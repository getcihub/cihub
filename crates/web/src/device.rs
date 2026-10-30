//! The sign-in of the command line with a device code (`specs/API.md`): the
//! command line asks for a code, the person approves it on the page `/device`
//! of the dashboard, and the command line takes its session. This module
//! keeps the codes, and holds the routes and their times.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use cihub_api_types::{CliSession, DeviceCode, DeviceCodeToken, ErrorCode};
use cihub_domain::{
    DeviceCodeId, DeviceCodeState, SessionId, UserId, format_device_code, format_user_code,
    is_device_code, parse_user_code,
};
use cihub_platform::{ApiError, ApiJson, ApiPath, error_responses, profile_of};
use cihub_store::{PgPool, Postgres, StoreError, Transaction, draw_secret_bytes, secret_hash};
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use tracing::info;
use utoipa::openapi::{RefOr, Response as DocumentResponse};

use crate::api::{ApiState, session_of};
use crate::session::{
    CLI_SESSION_IDLE_LIFETIME, NewSession, SessionClient, SessionGrant, create_session,
    new_cli_session_secret,
};

const DEVICE_CODE_LIFETIME: Duration = Duration::minutes(15);
const POLL_INTERVAL: Duration = Duration::seconds(5);
/// A command line waits `POLL_INTERVAL` after an answer, so two of its
/// requests arrive a little more than that apart; the network can bring the
/// second one a little sooner.
const POLL_TOLERANCE: Duration = Duration::seconds(1);
const VERIFICATION_PATH: &str = "/device";
/// Two codes drawn at the same time almost never share a user code; a few
/// draws more cover a code that expired and that the maintenance has not
/// deleted yet.
const USER_CODE_DRAWS: usize = 5;

macro_rules! route_errors {
    ($name:ident: $codes:expr) => {
        struct $name;

        impl utoipa::IntoResponses for $name {
            fn responses() -> BTreeMap<String, RefOr<DocumentResponse>> {
                error_responses($codes)
            }
        }
    };
}

route_errors!(CreateErrors: &[ErrorCode::RateLimited, ErrorCode::SignInUnavailable]);
route_errors!(
    TokenErrors: &[
        ErrorCode::InvalidRequest,
        ErrorCode::RateLimited,
        ErrorCode::DeviceCodePending,
        ErrorCode::DeviceCodeExpired,
        ErrorCode::DeviceCodeDenied,
        ErrorCode::PollingTooFast,
    ]
);
route_errors!(AnswerErrors: &[ErrorCode::DeviceCodeNotFound]);

/// Start a sign-in of the command line
///
/// A device code, for the command line, and its user code, for the person,
/// who approves it on the page `verification_url`. The command line then
/// asks `POST /device-codes/token` every `interval` seconds.
#[utoipa::path(
    post,
    path = "/device-codes",
    tag = "Session",
    responses(
        (status = 200, description = "A sign-in that waits for its person.", body = DeviceCode),
        CreateErrors
    )
)]
pub(crate) async fn start_device_sign_in(
    State(state): State<ApiState>,
) -> Result<Json<DeviceCode>, ApiError> {
    let Some(public_url) = state.public_url.as_deref() else {
        return Err(ApiError::new(
            ErrorCode::SignInUnavailable,
            "the instance has no public address".into(),
        ));
    };
    // Without the cipher, nobody signs in on the dashboard, so nobody
    // approves the code.
    if state.cipher.is_none() {
        return Err(ApiError::new(
            ErrorCode::SignInUnavailable,
            "the instance has no database secret, so nobody signs in".into(),
        ));
    }

    let now = (state.now)();
    let device_code = new_device_code_secret()?;
    for _ in 0..USER_CODE_DRAWS {
        let user_code = new_user_code()?;
        #[expect(
            clippy::disallowed_methods,
            reason = "a sign-in of the command line has a new identifier"
        )]
        let id = DeviceCodeId::new();
        let code = NewDeviceCode {
            id,
            device_code: &device_code,
            user_code: &user_code,
            created_at: now,
            expires_at: now + DEVICE_CODE_LIFETIME,
        };

        if create_device_code(&state.pool, &code).await? {
            info!(device_code_id = %id, "device_code.created");
            return Ok(Json(DeviceCode {
                device_code: device_code.expose_secret().to_owned(),
                user_code,
                verification_url: format!("{public_url}{VERIFICATION_PATH}"),
                expires_in: whole_seconds(DEVICE_CODE_LIFETIME),
                interval: whole_seconds(POLL_INTERVAL),
            }));
        }
    }

    Err(ApiError::new(
        ErrorCode::Internal,
        "no free user code".into(),
    ))
}

/// Get the session of the command line
///
/// The session, once the person approved the code; it is given once, and
/// the code is used. Before, the answer says to ask again after `interval`
/// seconds, 5 seconds more after each `polling_too_fast`.
#[utoipa::path(
    post,
    path = "/device-codes/token",
    tag = "Session",
    request_body = DeviceCodeToken,
    responses(
        (status = 200, description = "The session of the command line.", body = CliSession),
        TokenErrors
    )
)]
pub(crate) async fn get_device_session(
    State(state): State<ApiState>,
    ApiJson(request): ApiJson<DeviceCodeToken>,
) -> Result<Json<CliSession>, ApiError> {
    let now = (state.now)();
    let device_code = SecretString::from(request.device_code);
    let mut tx = state.pool.begin().await?;

    let poll = poll_device_code(&mut tx, &device_code, POLL_INTERVAL - POLL_TOLERANCE, now).await?;
    let user_id = match poll {
        DevicePoll::Approved(user_id) => user_id,
        DevicePoll::Pending => return Err(refusal(tx, ErrorCode::DeviceCodePending).await),
        DevicePoll::TooFast => return Err(refusal(tx, ErrorCode::PollingTooFast).await),
        DevicePoll::Denied => return Err(refusal(tx, ErrorCode::DeviceCodeDenied).await),
        DevicePoll::Expired => return Err(refusal(tx, ErrorCode::DeviceCodeExpired).await),
    };

    let secret = new_cli_session_secret()?;
    #[expect(
        clippy::disallowed_methods,
        reason = "a session of the command line has a new identifier"
    )]
    let id = SessionId::new();
    let session = NewSession {
        id,
        user_id,
        created_at: now,
        expires_at: now + CLI_SESSION_IDLE_LIFETIME,
    };
    create_session(&mut tx, &session, &secret).await?;
    tx.commit().await?;
    let profile = profile_of(&state.pool, user_id)
        .await?
        .ok_or_else(ApiError::unauthorized)?;

    info!(user_id = %user_id, session_id = %session.id, "command_line.signed_in");
    Ok(Json(CliSession {
        token: secret.expose_secret().to_owned(),
        login: profile.login,
    }))
}

/// Keeps what the poll wrote, when it was, then gives the error of `code`.
async fn refusal(tx: Transaction<'static, Postgres>, code: ErrorCode) -> ApiError {
    if let Err(err) = tx.commit().await {
        return err.into();
    }

    ApiError::new(code, code.sentence().to_owned())
}

/// Approve a sign-in of the command line
///
/// The person of the session approves the code that their command line
/// shows: the command line then signs in as them.
#[utoipa::path(
    post,
    path = "/device-codes/{user_code}/approve",
    tag = "Session",
    params(("user_code" = String, Path, description = "The code that the command line shows.", example = "WDJB-MJHT")),
    responses(
        (status = 204, description = "The command line signs in as the person."),
        AnswerErrors
    ),
    security(("session" = []))
)]
pub(crate) async fn approve_device_code(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
    ApiPath(user_code): ApiPath<String>,
) -> Result<StatusCode, ApiError> {
    answer(&state, session, &user_code, DeviceCodeState::Approved).await
}

/// Deny a sign-in of the command line
///
/// The person of the session denies a code that they did not ask for: no
/// command line signs in with it.
#[utoipa::path(
    post,
    path = "/device-codes/{user_code}/deny",
    tag = "Session",
    params(("user_code" = String, Path, description = "The code that the command line shows.", example = "WDJB-MJHT")),
    responses(
        (status = 204, description = "No command line signs in with the code."),
        AnswerErrors
    ),
    security(("session" = []))
)]
pub(crate) async fn deny_device_code(
    State(state): State<ApiState>,
    session: Option<Extension<SessionGrant>>,
    ApiPath(user_code): ApiPath<String>,
) -> Result<StatusCode, ApiError> {
    answer(&state, session, &user_code, DeviceCodeState::Denied).await
}

/// Only a session of the dashboard answers: a session of the command line
/// would sign another command line in by itself.
async fn answer(
    state: &ApiState,
    session: Option<Extension<SessionGrant>>,
    user_code: &str,
    answer: DeviceCodeState,
) -> Result<StatusCode, ApiError> {
    let session = session_of(session)?;
    if session.client != SessionClient::Dashboard {
        return Err(ApiError::new(
            ErrorCode::Unauthorized,
            "only a session of the dashboard answers a device code".into(),
        ));
    }

    let not_found = || {
        ApiError::new(
            ErrorCode::DeviceCodeNotFound,
            format!("no sign-in of the command line waits for the code `{user_code}`"),
        )
    };
    let user_code = parse_user_code(user_code).ok_or_else(not_found)?;
    let now = (state.now)();

    if !answer_device_code(&state.pool, &user_code, session.user_id, answer, now).await? {
        return Err(not_found());
    }

    info!(user_id = %session.user_id, answer = answer.as_str(), "device_code.answered");
    Ok(StatusCode::NO_CONTENT)
}

fn whole_seconds(duration: Duration) -> u64 {
    u64::try_from(duration.whole_seconds()).unwrap_or_default()
}

pub struct NewDeviceCode<'a> {
    pub id: DeviceCodeId,
    pub device_code: &'a SecretString,
    pub user_code: &'a str,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

/// What the command line learns when it asks for its session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicePoll {
    /// The person has not answered yet.
    Pending,
    /// The command line asks sooner than it should.
    TooFast,
    /// The person denied the code; the code is used.
    Denied,
    /// The code is unknown, used or expired.
    Expired,
    /// The person approved the code; the code is used.
    Approved(UserId),
}

/// # Errors
///
/// The system has no random source.
pub fn new_device_code_secret() -> Result<SecretString, StoreError> {
    Ok(format_device_code(&draw_secret_bytes()?).into())
}

/// # Errors
///
/// The system has no random source.
pub fn new_user_code() -> Result<String, StoreError> {
    Ok(format_user_code(&draw_secret_bytes()?))
}

/// Stores `code` with the hash of its device code. `false` when another
/// code, maybe expired but not deleted yet, has the same user code: the
/// caller draws another one.
///
/// # Errors
///
/// Postgres fails.
pub async fn create_device_code(
    pool: &PgPool,
    code: &NewDeviceCode<'_>,
) -> Result<bool, StoreError> {
    let created = sqlx::query(
        "INSERT INTO device_codes (id, device_code_hash, user_code, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_code) DO NOTHING",
    )
    .bind(code.id.to_string())
    .bind(secret_hash(code.device_code))
    .bind(code.user_code)
    .bind(code.created_at)
    .bind(code.expires_at)
    .execute(pool)
    .await
    .map_err(StoreError::Query)?
    .rows_affected();

    Ok(created == 1)
}

/// The answer `state` of `user_id` to the pending code `user_code`. `false`
/// when no pending code has this user code at `now`.
///
/// # Errors
///
/// Postgres fails.
pub async fn answer_device_code(
    pool: &PgPool,
    user_code: &str,
    user_id: UserId,
    state: DeviceCodeState,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    let answered = sqlx::query(
        "UPDATE device_codes SET state = $3::device_code_state, user_id = $4
         WHERE user_code = $1 AND state = 'pending' AND expires_at > $2",
    )
    .bind(user_code)
    .bind(now)
    .bind(state.as_str())
    .bind(user_id.to_string())
    .execute(pool)
    .await
    .map_err(StoreError::Query)?
    .rows_affected();

    Ok(answered == 1)
}

/// Where the code of `device_code` stands at `now`, for a command line that
/// asks again no sooner than `interval`. An answered code is used: its row
/// goes, so that the caller creates the session in the same transaction.
///
/// # Errors
///
/// Postgres fails, or the row does not match `cihub-domain`.
pub async fn poll_device_code(
    conn: &mut PgConnection,
    device_code: &SecretString,
    interval: Duration,
    now: OffsetDateTime,
) -> Result<DevicePoll, StoreError> {
    if !is_device_code(device_code.expose_secret()) {
        return Ok(DevicePoll::Expired);
    }

    let row: Option<(String, String, Option<String>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, state::text, user_id, polled_at FROM device_codes
         WHERE device_code_hash = $1 AND expires_at > $2
         FOR UPDATE",
    )
    .bind(secret_hash(device_code))
    .bind(now)
    .fetch_optional(&mut *conn)
    .await
    .map_err(StoreError::Query)?;
    let Some((id, state, user_id, polled_at)) = row else {
        return Ok(DevicePoll::Expired);
    };

    let state: DeviceCodeState = state
        .parse()
        .map_err(StoreError::read("device code", &id))?;
    let is_too_fast = polled_at.is_some_and(|at| now - at < interval);
    sqlx::query("UPDATE device_codes SET polled_at = $2 WHERE id = $1")
        .bind(&id)
        .bind(now)
        .execute(&mut *conn)
        .await
        .map_err(StoreError::Query)?;

    if is_too_fast {
        return Ok(DevicePoll::TooFast);
    }

    let poll = match (state, user_id) {
        (DeviceCodeState::Pending, _) => return Ok(DevicePoll::Pending),
        (DeviceCodeState::Denied, _) => DevicePoll::Denied,
        (DeviceCodeState::Approved, Some(user_id)) => DevicePoll::Approved(
            user_id
                .parse()
                .map_err(StoreError::read("device code", &id))?,
        ),
        (DeviceCodeState::Approved, None) => return Ok(DevicePoll::Expired),
    };
    sqlx::query("DELETE FROM device_codes WHERE id = $1")
        .bind(&id)
        .execute(&mut *conn)
        .await
        .map_err(StoreError::Query)?;
    Ok(poll)
}
