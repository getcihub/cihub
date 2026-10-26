use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{
    ConfigFile, ConfigFileState, ErrorCode, Permission, Repo, RepoDetail, RepoList, RepoSetup,
    RuleCheck, SetupState, SetupSummary,
};
use cihub_config::RepoConfig;
use cihub_domain::{ApiScope, Permission as Level};
use cihub_platform::{ApiError, ApiGrant, ApiPath, ApiQuery, GrantActor, Page, cursor_of};
use cihub_scm::{ConnectionRef, RepoRef};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use sqlx::types::Json as Stored;
use time::OffsetDateTime;

use super::queues::settings_summary;
use super::{ApiState, RepoErrors, RepoListErrors, RepoPath, audit, begin_repo, name_part};
use crate::repo_setup::sync_repo_setup;
use crate::target::queue_settings;

/// The text of a file that is not valid, which a setup shows up to this size.
const MAX_SHOWN_TEXT_BYTES: usize = 64 * 1024;

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct RepoQuery {
    /// Only the repositories whose setup is in these states, separated by
    /// commas: `started`, `checking`, `ready`, `ready_with_warnings`,
    /// `cannot_merge`, `invalid_config`.
    #[serde(default, deserialize_with = "setup_states")]
    #[param(value_type = Option<String>, example = "checking,ready,cannot_merge")]
    state: Vec<SetupState>,
    /// Only the repositories whose name holds this text, whatever its case;
    /// from 1 to 100 characters.
    #[param(example = "web")]
    q: Option<String>,
    /// The `cursor` of the previous page.
    from: Option<String>,
    /// How many repositories, from 1 to 100; 50 by default.
    limit: Option<u32>,
}

fn setup_states<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<SetupState>, D::Error> {
    let text = String::deserialize(deserializer)?;

    text.split(',')
        .map(|name| {
            name.parse()
                .map_err(|_| D::Error::custom(format!("unknown state `{name}`")))
        })
        .collect()
}

/// A repository, its queue on its default branch, its last setup read, and
/// the read of its setup that an admin asked for and that waits.
const SETUP_SOURCE: &str = "repos r
     JOIN scm_connections c ON c.id = r.connection_id
     LEFT JOIN queues q ON q.repo_id = r.id AND q.target_branch = r.default_branch
     LEFT JOIN repo_setups s ON s.repo_id = r.id
     LEFT JOIN LATERAL (
         SELECT min(j.created_at) AS requested_at FROM jobs j
         WHERE j.type = 'read_repo_setup' AND j.class = 'interactive'
           AND j.state IN ('queued', 'running') AND j.payload ->> 'repo_id' = r.id
     ) w ON true";

/// The verdict of a setup, in the order of `SetupState`: the queue of the
/// default branch first, then a read that waits, the file, the rules that
/// block the merges or a branch that is gone, the other rules.
const SETUP_STATE: &str = "CASE
     WHEN q.id IS NOT NULL THEN 'started'
     WHEN s.repo_id IS NULL OR w.requested_at IS NOT NULL THEN 'checking'
     WHEN s.config_state = 'invalid' THEN 'invalid_config'
     WHEN s.head_sha IS NULL
       OR s.findings @> '[{\"code\": \"app_not_exempt\"}]'
       OR s.findings @> '[{\"code\": \"test_branches_not_excluded\"}]' THEN 'cannot_merge'
     WHEN jsonb_array_length(s.findings) > 0 THEN 'ready_with_warnings'
     ELSE 'ready'
 END";

/// A repository of a list: its identity, its verdict, when its setup was read
/// and asked for, and the permission of the person of the request on it.
#[derive(sqlx::FromRow)]
struct RepoRow {
    id: String,
    owner: String,
    name: String,
    default_branch: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    state: String,
    checked_at: Option<OffsetDateTime>,
    requested_at: Option<OffsetDateTime>,
    permission: Option<String>,
}

/// List the repositories
///
/// The repositories of the organization that the App reaches, by owner then
/// name, each with what the caller may do on it and the verdict of its
/// setup. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos",
    tag = "Repositories",
    params(
        ("org" = String, Path, description = "The slug of the organization.", example = "acme"),
        RepoQuery
    ),
    responses(
        (status = 200, description = "A page of repositories.", body = RepoList),
        RepoListErrors
    ),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn list_repos(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(org): ApiPath<String>,
    ApiQuery(query): ApiQuery<RepoQuery>,
) -> Result<Json<RepoList>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;
    let page: Page<(String, String, String)> =
        Page::read("repos", query.from.as_deref(), query.limit)?;
    let name = name_part(query.q)?;
    let states: Vec<&str> = query.state.iter().map(|state| (*state).into()).collect();
    let (repos, user) = (grant.repo_filter(), user_of(&grant));

    // `$4` names the person of the request, whose permission a row gives.
    let listed = format!(
        "WITH listed AS (
             SELECT r.id, r.owner, r.name, r.default_branch, r.created_at, r.updated_at,
                    {SETUP_STATE} AS state,
                    s.checked_at, w.requested_at, p.permission::text AS permission
             FROM {SETUP_SOURCE}
             LEFT JOIN repo_permissions p ON p.repo_id = r.id AND p.user_id = $4
             WHERE r.removed_at IS NULL
               AND ($1::text IS NULL OR strpos(lower(r.name), lower($1)) > 0)
               AND ($3::text[] IS NULL OR r.id = ANY($3))
         )"
    );
    let filter = "(cardinality($2::text[]) = 0 OR state = ANY($2))";
    let mut tx = grant.begin_in_org(&state.pool, &org).await?;
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "{listed} SELECT count(*) FROM listed WHERE {filter}"
    )))
    .bind(&name)
    .bind(&states)
    .bind(&repos)
    .bind(&user)
    .fetch_one(&mut *tx)
    .await?;
    let (after_owner, after_name, id) = match page.after.clone() {
        Some((owner, name, id)) => (Some(owner), Some(name), Some(id)),
        None => (None, None, None),
    };
    // A repository deleted then created again keeps its row: the identifier
    // tells two repositories of the same name apart.
    let rows: Vec<RepoRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{listed}
         SELECT id, owner, name, default_branch, created_at, updated_at, state, checked_at,
                requested_at, permission
         FROM listed
         WHERE {filter} AND ($5::text IS NULL OR (owner, name, id) > ($5, $6, $7))
         ORDER BY owner, name, id LIMIT $8"
    )))
    .bind(&name)
    .bind(&states)
    .bind(&repos)
    .bind(&user)
    .bind(after_owner)
    .bind(after_name)
    .bind(id)
    .bind(page.rows())
    .fetch_all(&mut *tx)
    .await?;

    let repos = rows
        .into_iter()
        .map(|row| repo_of(row, &grant))
        .collect::<Result<Vec<_>, _>>()?;
    let (repos, has_more): (Vec<Repo>, bool) = page.split(repos);
    let cursor = repos
        .last()
        .filter(|_| has_more)
        .map(|repo| cursor_of("repos", (&repo.owner, &repo.name, &repo.id)));
    Ok(Json(RepoList {
        repos,
        has_more,
        count: u64::try_from(count)?,
        cursor,
    }))
}

/// A repository and the whole of its last setup read.
#[derive(sqlx::FromRow)]
struct DetailRow {
    id: String,
    owner: String,
    name: String,
    default_branch: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    external_id: String,
    connection_id: String,
    connection_external_id: String,
    queue: Option<String>,
    state: String,
    is_read: bool,
    head_sha: Option<String>,
    findings: Option<Stored<Vec<StoredFinding>>>,
    config_state: Option<String>,
    config_path: Option<String>,
    config_error: Option<String>,
    config_line: Option<i32>,
    config_column: Option<i32>,
    config_text: Option<String>,
    entries: Option<Vec<String>>,
    checked_at: Option<OffsetDateTime>,
    requested_at: Option<OffsetDateTime>,
}

/// A finding as `read_repo_setup` keeps it.
#[derive(Deserialize)]
struct StoredFinding {
    code: String,
}

/// Get a repository
///
/// The repository, whatever the case of its name, with what the caller may
/// do on it and its whole setup. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}",
    tag = "Repositories",
    params(RepoPath),
    responses((status = 200, description = "The repository.", body = RepoDetail), RepoErrors),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_repo(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
) -> Result<Json<RepoDetail>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let mut tx = grant.begin_in_org(&state.pool, &path.org).await?;
    // The SCM does not tell names apart by their case.
    let row: Option<DetailRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT r.id, r.owner, r.name, r.default_branch, r.created_at, r.updated_at,
                r.external_id,
                c.id AS connection_id, c.external_id AS connection_external_id,
                q.target_branch AS queue, {SETUP_STATE} AS state, s.repo_id IS NOT NULL AS is_read,
                s.head_sha, s.findings, s.config_state::text, s.config_path, s.config_error,
                s.config_line, s.config_column, s.config_text, s.entries, s.checked_at,
                w.requested_at
         FROM {SETUP_SOURCE}
         WHERE r.removed_at IS NULL AND lower(r.name) = lower($1) ORDER BY r.id LIMIT 1"
    )))
    .bind(&path.repo)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Err(ApiError::new(
            ErrorCode::RepoNotFound,
            format!("repository `{}` not found", path.repo),
        ));
    };

    let repo = RepoRef {
        connection: ConnectionRef {
            id: row.connection_id.parse()?,
            external_id: row.connection_external_id.clone(),
        },
        external_id: row.external_id.clone(),
        owner: row.owner.clone(),
        name: row.name.clone(),
    };
    let setup = RepoSetup {
        state: row.state.parse()?,
        rules: rules_of(&row),
        target_branch_missing: row.is_read && row.head_sha.is_none(),
        rules_url: state.scm.branch_rules_url(&repo),
        config: config_of(&row)?,
        checked_at: row.checked_at,
        requested_at: row.requested_at,
        queue: row.queue,
    };
    Ok(Json(RepoDetail {
        id: row.id,
        owner: row.owner,
        name: row.name,
        default_branch: row.default_branch,
        created_at: row.created_at,
        updated_at: row.updated_at,
        permission: permission_of(&grant),
        setup,
    }))
}

/// The four rules that a queue needs, in the order of the findings; none
/// before a read of the default branch.
fn rules_of(row: &DetailRow) -> Vec<RuleCheck> {
    let (Some(findings), Some(_)) = (&row.findings, &row.head_sha) else {
        return Vec::new();
    };
    let branch = &row.default_branch;

    RULES
        .iter()
        .map(|rule| {
            let passed = !findings.0.iter().any(|finding| finding.code == rule.code);
            RuleCheck {
                code: rule.code.to_owned(),
                passed,
                blocks_merge: rule.blocks_merge,
                title: if passed {
                    (rule.passed)(branch)
                } else {
                    (rule.failed)(branch)
                },
                message: (!passed).then(|| (rule.change)(branch)),
            }
        })
        .collect()
}

/// A rule of the SCM that a queue needs: what it says when it passes and
/// when it fails, and what to change.
struct Rule {
    code: &'static str,
    blocks_merge: bool,
    passed: fn(&str) -> String,
    failed: fn(&str) -> String,
    change: fn(&str) -> String,
}

const RULES: [Rule; 4] = [
    Rule {
        code: "update_restriction_missing",
        blocks_merge: false,
        passed: |branch| format!("A rule restricts the updates of `{branch}`"),
        failed: |branch| format!("No rule restricts the updates of `{branch}`"),
        change: |branch| {
            format!(
                "add a ruleset that restricts the updates of `{branch}`, with the App of CIHub in \
                 its bypass list, in exempt mode: without it, a PR can merge without the queue"
            )
        },
    },
    Rule {
        code: "app_not_exempt",
        blocks_merge: true,
        passed: |branch| format!("The App of CIHub may update `{branch}`"),
        failed: |branch| {
            format!("A rule restricts the updates of `{branch}` without an exemption for the App")
        },
        change: |_| {
            "add the App of CIHub to the bypass list of the ruleset, in exempt mode".to_owned()
        },
    },
    Rule {
        code: "test_branches_not_excluded",
        blocks_merge: true,
        passed: |_| "No rule covers the test branches of CIHub".to_owned(),
        failed: |_| "A rule covers `cihub-temp/**` or `cihub-merge/**`".to_owned(),
        change: |_| {
            "exclude `cihub-temp/**/*` and `cihub-merge/**/*` from every ruleset".to_owned()
        },
    },
    Rule {
        code: "up_to_date_required",
        blocks_merge: false,
        passed: |branch| format!("`{branch}` asks for no branch up to date"),
        failed: |branch| format!("`{branch}` asks for branches up to date"),
        change: |branch| {
            format!(
                "turn off \"Require branches to be up to date before merging\" on `{branch}`, \
                 unless the queue merges directly"
            )
        },
    },
];

/// The configuration file of the last read, with the strategy and the
/// settings of the default branch: from a valid file, or the defaults.
fn config_of(row: &DetailRow) -> Result<Option<ConfigFile>, ApiError> {
    let Some(state) = row.config_state.as_deref() else {
        return Ok(None);
    };
    let state = match state {
        "valid" => ConfigFileState::Valid,
        "invalid" => ConfigFileState::Invalid,
        _ => ConfigFileState::None,
    };
    let config = match (state, &row.config_text) {
        (ConfigFileState::Valid, Some(text)) => Some(RepoConfig::parse(text)?),
        _ => None,
    };
    let settings = queue_settings(config.as_ref(), &row.default_branch)?;
    let text = row
        .config_text
        .as_ref()
        .filter(|text| state == ConfigFileState::Invalid && text.len() <= MAX_SHOWN_TEXT_BYTES);

    Ok(Some(ConfigFile {
        state,
        path: row.config_path.clone(),
        sha: row.head_sha.clone(),
        error: row.config_error.clone(),
        line: row.config_line.map(u32::try_from).transpose()?,
        column: row.config_column.map(u32::try_from).transpose()?,
        text: text.cloned(),
        entries: row.entries.clone().unwrap_or_default(),
        strategy: settings.strategy.value,
        settings: settings_summary(&settings),
    }))
}

/// Sync the setup of a repository
///
/// Asks CIHub to read the setup of the repository from the SCM now, at most
/// once a minute; its `requested_at` shows the read until it ends.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/sync",
    tag = "Repositories",
    params(RepoPath),
    responses((status = 202, description = "CIHub takes the request."), RepoErrors),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn sync_repo(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
) -> Result<StatusCode, ApiError> {
    grant.require(ApiScope::QueuesWrite)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let now = (state.clock)();

    sync_repo_setup(&mut tx, repo, now).await?;
    audit(&mut tx, &grant, "repo.sync", &repo.to_string(), json!({})).await?;
    tx.commit().await?;

    Ok(StatusCode::ACCEPTED)
}

/// The person of the request, whose permission a list gives; `None` for a
/// token.
fn user_of(grant: &ApiGrant) -> Option<String> {
    match grant.actor {
        GrantActor::User(user_id) => Some(user_id.to_string()),
        GrantActor::Token(_) => None,
    }
}

/// What the scopes of the caller let it do on a repository of the
/// organization: on a route of a repository, those of the permission of a
/// person on it.
fn permission_of(grant: &ApiGrant) -> Permission {
    Permission {
        read: grant.scopes.contains(&ApiScope::QueuesRead),
        write: grant.scopes.contains(&ApiScope::PrsWrite),
        admin: grant.scopes.contains(&ApiScope::QueuesWrite),
    }
}

/// A repository of a list: a person gets their permission on it, a token
/// that of its scopes.
fn repo_of(row: RepoRow, grant: &ApiGrant) -> Result<Repo, ApiError> {
    let permission = match (grant.actor, row.permission) {
        (GrantActor::User(_), Some(level)) => permission_of_level(level.parse()?),
        (GrantActor::User(_), None) => permission_of_level(Level::None),
        (GrantActor::Token(_), _) => permission_of(grant),
    };

    Ok(Repo {
        id: row.id,
        owner: row.owner,
        name: row.name,
        default_branch: row.default_branch,
        created_at: row.created_at,
        updated_at: row.updated_at,
        permission,
        setup: SetupSummary {
            state: row.state.parse()?,
            checked_at: row.checked_at,
            requested_at: row.requested_at,
        },
    })
}

fn permission_of_level(level: Level) -> Permission {
    Permission {
        read: level >= Level::Read,
        write: level >= Level::Write,
        admin: level >= Level::Admin,
    }
}

#[cfg(test)]
mod tests {
    use crate::repo_setup::READ_REPO_SETUP;

    #[test]
    fn the_sql_names_the_job_of_the_setup() {
        assert_eq!(
            READ_REPO_SETUP, "read_repo_setup",
            "the name that the SQL reads"
        );
    }
}
