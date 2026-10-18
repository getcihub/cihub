//! The sign-in of the command line with a device code (`specs/API.md`): the
//! command line asks for a code, the person approves it on the page `/device`
//! of the dashboard, and the command line takes its session. The store keeps
//! the codes; this module holds the routes and their times.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use cihub_api_types::{CliSession, DeviceCode, DeviceCodeToken, ErrorCode};
use cihub_domain::{DeviceCodeId, DeviceCodeState, SessionId, parse_user_code};
use cihub_platform::{ApiError, ApiJson, ApiPath, error_responses};
use cihub_store::{
    CLI_SESSION_IDLE_LIFETIME, DevicePoll, NewDeviceCode, NewSession, Postgres, SessionClient,
    SessionGrant, Transaction, answer_device_code, create_device_code, create_session,
    new_cli_session_secret, new_device_code_secret, new_user_code, poll_device_code, profile_of,
};
use secrecy::{ExposeSecret, SecretString};
use time::Duration;
use tracing::info;
use utoipa::openapi::{RefOr, Response as DocumentResponse};

use crate::api::{ApiState, session_of};

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

route_errors!(CreateErrors: &[ErrorCode::SignInUnavailable]);
route_errors!(
    TokenErrors: &[
        ErrorCode::InvalidRequest,
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
            info!(device_code_id = %id, "device code created");
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

    info!(user_id = %user_id, session_id = %session.id, "command line signed in");
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

    info!(user_id = %session.user_id, answer = answer.as_str(), "device code answered");
    Ok(StatusCode::NO_CONTENT)
}

fn whole_seconds(duration: Duration) -> u64 {
    u64::try_from(duration.whole_seconds()).unwrap_or_default()
}
