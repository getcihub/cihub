use cihub_domain::{
    DeviceCodeId, DeviceCodeState, UserId, format_device_code, format_user_code, is_device_code,
};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};

use crate::StoreError;
use crate::tokens::{draw_secret_bytes, hash};

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
    .bind(hash(code.device_code))
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
    .bind(hash(device_code))
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
