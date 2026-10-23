use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use cihub_api_types::{
    BranchConfig, ConfigFileState, ConfigPreview, ConfigProposal, ConfigSettings, ErrorCode,
    PreviewConfig, ProposeConfig, ResolvedConfig, ResolvedQueue, SettingSource, SettingSources,
};
use cihub_config::{
    ConfigError, Layer, QueueSettings, RepoConfig, Source, diff_lines, write_document,
};
use cihub_domain::{ApiScope, ConfigProposalId, ConfigProposalReason, ConfigProposalState, RepoId};
use cihub_platform::{ApiError, ApiGrant, ApiJson, ApiPath, ApiQuery, GrantActor, enqueue_job};
use cihub_scm::RepoRef;
use cihub_store::{Postgres, Transaction};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

use super::repos::{MAX_SHOWN_TEXT_BYTES, config_file_state};
use super::{
    ApiState, BranchConfigErrors, PreviewErrors, ProposeErrors, RepoPath, audit, begin_repo,
};
use crate::proposal::{PROPOSE_CONFIG, propose_config_job};
use crate::target::RepoTarget;

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct BranchQuery {
    /// The default branch of the repository, or the target branch of one of
    /// its queues; the default branch without it.
    #[param(example = "release/2.0")]
    branch: Option<String>,
}

/// The last read of the file of a branch, and the checks that the rules of
/// the branch require, from the most recent setup read.
#[derive(sqlx::FromRow)]
struct FileRow {
    branch: String,
    head_sha: Option<String>,
    state: String,
    path: Option<String>,
    text: Option<String>,
    error: Option<String>,
    error_line: Option<i32>,
    error_column: Option<i32>,
    read_at: OffsetDateTime,
    required_checks: Option<Vec<String>>,
}

/// The last read of the file of `branch` of `repo`, the default branch
/// without one.
async fn read_file(
    tx: &mut Transaction<'static, Postgres>,
    repo: RepoId,
    branch: Option<&str>,
) -> Result<FileRow, ApiError> {
    // A queue reads the rules of its branch every hour, the setup of the
    // repository once a day: the latest read wins.
    let row: Option<FileRow> = sqlx::query_as(
        "SELECT f.branch, f.head_sha, f.state::text, f.path, f.text, f.error, f.error_line,
                f.error_column, f.read_at,
                CASE WHEN s.checked_at IS NULL OR k.checked_at > s.checked_at
                     THEN k.required_checks ELSE s.required_checks END AS required_checks
         FROM repos r
         JOIN config_files f ON f.repo_id = r.id AND f.branch = coalesce($2, r.default_branch)
         LEFT JOIN queues q ON q.repo_id = r.id AND q.target_branch = f.branch
         LEFT JOIN queue_setups k ON k.queue_id = q.id
         LEFT JOIN repo_setups s ON s.repo_id = r.id AND s.branch = f.branch
         WHERE r.id = $1 AND (f.branch = r.default_branch OR q.id IS NOT NULL)",
    )
    .bind(repo.to_string())
    .bind(branch)
    .fetch_optional(&mut **tx)
    .await?;

    row.ok_or_else(|| {
        let branch = branch.map_or_else(|| "the default branch".to_owned(), |b| format!("`{b}`"));
        ApiError::new(
            ErrorCode::ConfigNotFound,
            format!("no configuration of {branch}"),
        )
    })
}

/// Get the configuration of a branch
///
/// The configuration file of the branch, as CIHub last read it: what it sets,
/// the settings that follow with their sources, the checks that the branch
/// rules require, and the last proposal of a change that has not merged or
/// closed. Reads Postgres only.
#[utoipa::path(
    get,
    path = "/orgs/{org}/repos/{repo}/config",
    tag = "Repositories",
    params(RepoPath, BranchQuery),
    responses(
        (status = 200, description = "The configuration of the branch.", body = BranchConfig),
        BranchConfigErrors
    ),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn get_branch_config(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
    ApiQuery(query): ApiQuery<BranchQuery>,
) -> Result<Json<BranchConfig>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let row = read_file(&mut tx, repo, query.branch.as_deref()).await?;
    let target = RepoTarget::load(&mut tx, repo).await?;
    let proposal = last_proposal(&mut tx, &state, &target.repo, repo, &row.branch).await?;

    let file_state = config_file_state(row.state.parse()?);
    let config = match (file_state, &row.text) {
        (ConfigFileState::Valid, Some(text)) => Some(RepoConfig::parse(text)?),
        _ => None,
    };
    let (document, settings) = match file_state {
        ConfigFileState::Invalid => (None, None),
        _ => (
            Some(
                config
                    .as_ref()
                    .map(RepoConfig::document)
                    .unwrap_or_default(),
            ),
            Some(resolved_of(config.as_ref())?),
        ),
    };

    Ok(Json(BranchConfig {
        branch: row.branch,
        state: file_state,
        path: row.path,
        sha: row.head_sha,
        error: row.error,
        line: row.error_line.map(u32::try_from).transpose()?,
        column: row.error_column.map(u32::try_from).transpose()?,
        text: row.text.filter(|text| text.len() <= MAX_SHOWN_TEXT_BYTES),
        document,
        settings,
        required_checks: row.required_checks,
        proposal,
        read_at: row.read_at,
    }))
}

/// Preview a configuration
///
/// The YAML that CIHub would write for a document, whether the queue of the
/// branch would take it, its settings with their sources, and its diff with
/// the file as CIHub read it last. Reads Postgres only.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/config/preview",
    tag = "Repositories",
    params(RepoPath),
    request_body = PreviewConfig,
    responses(
        (status = 200, description = "The file of the document.", body = ConfigPreview),
        PreviewErrors
    ),
    security(("token" = ["queues:read"]))
)]
pub(super) async fn preview_config(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
    ApiJson(request): ApiJson<PreviewConfig>,
) -> Result<Json<ConfigPreview>, ApiError> {
    grant.require(ApiScope::QueuesRead)?;

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let row = read_file(&mut tx, repo, request.branch.as_deref()).await?;
    drop(tx);

    let written = write_document(&request.document);
    let text = written
        .as_ref()
        .map_or_else(|_| String::new(), Clone::clone);
    refuse_too_large(&text)?;
    let checked = written.and_then(|text| checked_config(&text, &row.branch));
    let diff = diff_lines(row.text.as_deref().unwrap_or_default(), &text);

    let preview = match checked {
        Ok(config) => ConfigPreview {
            state: ConfigFileState::Valid,
            error: None,
            line: None,
            column: None,
            settings: Some(resolved_of(Some(&config))?),
            text,
            diff,
        },
        Err(error) => {
            let position = error.position;
            ConfigPreview {
                state: ConfigFileState::Invalid,
                error: Some(error.message),
                line: position.map(|at| u32::try_from(at.line)).transpose()?,
                column: position.map(|at| u32::try_from(at.column)).transpose()?,
                settings: None,
                text,
                diff,
            }
        }
    };
    Ok(Json(preview))
}

/// Propose a change of the configuration
///
/// Writes the YAML of the document and asks CIHub to open it as a PR from the
/// branch `cihub-config/<branch>`, on the head of the branch: CIHub never
/// pushes on the branch itself. The same text again answers its proposal;
/// another text while a proposal has not ended answers `409
/// config_proposal_open`. The proposal fails with `config_changed` when the
/// file changed since `base_sha`.
#[utoipa::path(
    post,
    path = "/orgs/{org}/repos/{repo}/config/proposals",
    tag = "Repositories",
    params(RepoPath),
    request_body = ProposeConfig,
    responses(
        (status = 202, description = "CIHub opens the PR.", body = ConfigProposal),
        ProposeErrors
    ),
    security(("token" = ["queues:write"]))
)]
pub(super) async fn propose_config(
    grant: ApiGrant,
    State(state): State<ApiState>,
    ApiPath(path): ApiPath<RepoPath>,
    ApiJson(request): ApiJson<ProposeConfig>,
) -> Result<(StatusCode, Json<ConfigProposal>), ApiError> {
    grant.require(ApiScope::QueuesWrite)?;
    let message = request.message.trim();
    for (field, value) in [("message", message), ("base_sha", request.base_sha.trim())] {
        if value.is_empty() {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                format!("`{field}` is empty"),
            ));
        }
    }

    let (mut tx, repo) = begin_repo(&state, &grant, &path.org, &path.repo).await?;
    let row = read_file(&mut tx, repo, request.branch.as_deref()).await?;
    let target = RepoTarget::load(&mut tx, repo).await?;
    let (text, written) = write_document(&request.document)
        .and_then(|text| checked_config(&text, &row.branch).map(|config| (text, config)))
        .map_err(|error| ApiError::new(ErrorCode::InvalidConfig, error.to_string()))?;
    refuse_too_large(&text)?;
    // The file may have comments or another form: the document says what it sets.
    let is_unchanged = row.state == "valid"
        && row
            .text
            .as_deref()
            .and_then(|file| RepoConfig::parse(file).ok())
            .is_some_and(|file| file.document() == written.document());
    let refused = match (&row.path, &row.text) {
        _ if is_unchanged => Some("the document sets what the file sets"),
        // `find_config_file` refuses several paths, and keeps none.
        (None, None) if row.state == "invalid" => {
            Some("several configuration files exist: keep only one before a proposal")
        }
        _ => None,
    };
    if let Some(refused) = refused {
        return Err(ApiError::new(ErrorCode::InvalidRequest, refused.to_owned()));
    }

    let now = (state.clock)();
    end_dead_proposals(&mut tx, repo, &row.branch, now).await?;
    if let Some(open) = last_proposal(&mut tx, &state, &target.repo, repo, &row.branch).await?
        && open.state != ConfigProposalState::Failed
    {
        let open_text: String =
            sqlx::query_scalar("SELECT text FROM config_proposals WHERE id = $1")
                .bind(&open.id)
                .fetch_one(&mut *tx)
                .await?;
        if open_text != text {
            return Err(ApiError::new(
                ErrorCode::ConfigProposalOpen,
                format!("the proposal {} of `{}` has not ended", open.id, row.branch),
            ));
        }
        return Ok((StatusCode::ACCEPTED, Json(open)));
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a new proposal gets a new identifier"
    )]
    let id = ConfigProposalId::new();
    let requested_by = match grant.actor {
        GrantActor::User(user_id) => Some(user_id.to_string()),
        GrantActor::Token(_) => None,
    };

    // Two proposals at once: the unique index keeps one, and the other one waits for it to end.
    let inserted = sqlx::query(
        "INSERT INTO config_proposals (id, org_id, repo_id, branch, base_sha, path, text, message,
                                       requested_by, state, requested_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'requested', $10, $10)
         ON CONFLICT DO NOTHING",
    )
    .bind(id.to_string())
    .bind(target.org_id.to_string())
    .bind(repo.to_string())
    .bind(&row.branch)
    .bind(&request.base_sha)
    .bind(row.path.as_deref().unwrap_or(".cihub.yaml"))
    .bind(&text)
    .bind(message)
    .bind(requested_by)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::new(
            ErrorCode::ConfigProposalOpen,
            format!("a proposal of `{}` has not ended", row.branch),
        ));
    }

    enqueue_job(&mut tx, &propose_config_job(&target, id), now, now).await?;
    let after = json!({ "branch": row.branch, "base_sha": request.base_sha });
    audit(&mut tx, &grant, "config.proposed", &id.to_string(), after).await?;
    let proposal = last_proposal(&mut tx, &state, &target.repo, repo, &row.branch)
        .await?
        .ok_or_else(|| ApiError::new(ErrorCode::Internal, "proposal not written".to_owned()))?;
    tx.commit().await?;

    Ok((StatusCode::ACCEPTED, Json(proposal)))
}

/// A file larger than the pages show is refused: its diff would cost too much.
fn refuse_too_large(text: &str) -> Result<(), ApiError> {
    if text.len() > MAX_SHOWN_TEXT_BYTES {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("the file would be larger than {MAX_SHOWN_TEXT_BYTES} bytes"),
        ));
    }
    Ok(())
}

/// Ends the proposals of `branch` whose job gave up: the SCM kept failing, so
/// they no longer hold the branch.
async fn end_dead_proposals(
    tx: &mut Transaction<'static, Postgres>,
    repo: RepoId,
    branch: &str,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE config_proposals p
         SET state = 'failed', state_reason = 'pr_not_opened', updated_at = $3
         WHERE p.repo_id = $1 AND p.branch = $2 AND p.state = 'requested'
           AND EXISTS (SELECT 1 FROM jobs j
                       WHERE j.idempotency_key = $4 || ':' || p.id || ':1'
                         AND j.state IN ('failed', 'dead'))",
    )
    .bind(repo.to_string())
    .bind(branch)
    .bind(now)
    .bind(PROPOSE_CONFIG)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// `text`, as the queue of `branch` would load it.
fn checked_config(text: &str, branch: &str) -> Result<RepoConfig, ConfigError> {
    let config = RepoConfig::parse(text)?;
    config.settings(branch)?;
    Ok(config)
}

/// A proposal as the database keeps it, with the login of its person.
#[derive(sqlx::FromRow)]
struct ProposalRow {
    id: String,
    branch: String,
    state: String,
    state_reason: Option<String>,
    pr_number: Option<i64>,
    login: Option<String>,
    requested_at: OffsetDateTime,
}

/// The last proposal of `branch`, unless it merged or closed.
async fn last_proposal(
    tx: &mut Transaction<'static, Postgres>,
    state: &ApiState,
    repo_ref: &RepoRef,
    repo: RepoId,
    branch: &str,
) -> Result<Option<ConfigProposal>, ApiError> {
    let row: Option<ProposalRow> = sqlx::query_as(
        "SELECT p.id, p.branch,
                CASE WHEN j.state IN ('failed', 'dead') AND p.state = 'requested'
                     THEN 'failed' ELSE p.state::text END AS state,
                CASE WHEN j.state IN ('failed', 'dead') AND p.state = 'requested'
                     THEN 'pr_not_opened' ELSE p.state_reason::text END AS state_reason,
                p.pr_number, u.login, p.requested_at
         FROM config_proposals p
         LEFT JOIN users u ON u.id = p.requested_by
         LEFT JOIN jobs j ON j.idempotency_key = $3 || ':' || p.id || ':1'
         WHERE p.repo_id = $1 AND p.branch = $2
         ORDER BY p.requested_at DESC, p.id DESC LIMIT 1",
    )
    .bind(repo.to_string())
    .bind(branch)
    .bind(PROPOSE_CONFIG)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let proposal_state: ConfigProposalState = row.state.parse()?;
    if matches!(
        proposal_state,
        ConfigProposalState::Merged | ConfigProposalState::Closed
    ) {
        return Ok(None);
    }

    let reason: Option<ConfigProposalReason> =
        row.state_reason.as_deref().map(str::parse).transpose()?;
    let pr_number = row.pr_number.map(u64::try_from).transpose()?;
    Ok(Some(ConfigProposal {
        id: row.id,
        state_description: reason.map(|reason| reason.describe(&row.branch)),
        branch: row.branch,
        state: proposal_state,
        state_reason: reason,
        pr_url: pr_number.map(|number| state.scm.pr_url(repo_ref, number)),
        pr_number,
        requested_by: row.login,
        requested_at: row.requested_at,
    }))
}

/// The settings of `config`, each with its source: the values of CIHub
/// without a file.
fn resolved_of(config: Option<&RepoConfig>) -> Result<ResolvedConfig, ApiError> {
    let Some(config) = config else {
        let defaults = QueueSettings::resolve(&Layer::default(), &Layer::default())?;
        return Ok(ResolvedConfig {
            defaults: settings_of(&defaults),
            queues: Vec::new(),
        });
    };

    let defaults = QueueSettings::resolve(&Layer::default(), &config.defaults)?;
    let queues = config
        .queues
        .iter()
        .map(|queue| {
            let settings = QueueSettings::resolve(&queue.settings, &config.defaults)?;
            let profiles = queue
                .profiles
                .iter()
                .map(|profile| {
                    QueueSettings::resolve_profile(
                        &profile.settings,
                        &queue.settings,
                        &config.defaults,
                    )
                    .map(|settings| settings_of(&settings))
                })
                .collect::<Result<_, _>>()?;

            Ok(ResolvedQueue {
                settings: settings_of(&settings),
                profiles,
            })
        })
        .collect::<Result<_, ApiError>>()?;

    Ok(ResolvedConfig {
        defaults: settings_of(&defaults),
        queues,
    })
}

fn source_of(source: Source) -> SettingSource {
    match source {
        Source::Default => SettingSource::Default,
        Source::Strategy => SettingSource::Strategy,
        Source::Defaults => SettingSource::Defaults,
        Source::Queue => SettingSource::Queue,
        Source::Profile => SettingSource::Profile,
    }
}

/// The value and the source of each setting of `settings`.
macro_rules! settings_of {
    ($settings:expr, $($field:ident),+ $(,)?) => {
        ConfigSettings {
            $($field: $settings.$field.value.clone(),)+
            sources: SettingSources {
                $($field: source_of($settings.$field.source),)+
            },
        }
    };
}

fn settings_of(settings: &QueueSettings) -> ConfigSettings {
    settings_of!(
        settings,
        strategy,
        mode,
        window_floor,
        window_ceiling,
        bisection_concurrency,
        testing_timeout_minutes,
        not_ready_timeout_hours,
        merge_method,
        merge_commit_title,
        trigger,
        update_method,
        batching_enabled,
        batching_size,
        batching_max_wait_minutes,
        failure_budget_retries,
        optimistic_merging,
        pending_failure_depth,
        direct_merge_mode,
        optimization_mode,
        comments_enabled,
        commands_enabled,
        status_check_enabled,
        label_commands_enabled,
        enqueue_label,
        state_labels_enabled,
        resubmit_on_push,
        keep_test_branches_minutes,
        branch_prefix,
        branch_protection_injection,
        required_statuses,
    )
}
