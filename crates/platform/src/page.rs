use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cihub_api_types::{DEFAULT_LIMIT, ErrorCode, MAX_LIMIT};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::ApiError;

/// The errors of every route that gives a page.
pub const PAGE_ERRORS: &[ErrorCode] = &[
    ErrorCode::InvalidRequest,
    ErrorCode::LimitOutOfRange,
    ErrorCode::InvalidCursor,
];

/// What a cursor holds: the list that gave it, so that a cursor of another
/// list is refused, and the sort key of the last entry of its page.
#[derive(Serialize, Deserialize)]
struct Cursor<K> {
    list: String,
    after: K,
}

/// The time in a sort key: read as RFC 3339 when the cursor is read, so that
/// a forged time is an invalid cursor and not an error of the database.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct CursorTime(#[serde(with = "time::serde::rfc3339")] pub OffsetDateTime);

/// The page of a list that a request asks for: at most `limit` entries,
/// after the entry whose sort key is `after`.
pub struct Page<K> {
    pub limit: u32,
    pub after: Option<K>,
}

impl<K: DeserializeOwned> Page<K> {
    /// The page of `list` that the query parameters `from` and `limit` ask
    /// for.
    ///
    /// # Errors
    ///
    /// `limit` is out of range, or `from` is not a cursor of `list`.
    pub fn read(list: &str, from: Option<&str>, limit: Option<u32>) -> Result<Self, ApiError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT);
        if limit == 0 || limit > MAX_LIMIT {
            return Err(ApiError::new(
                ErrorCode::LimitOutOfRange,
                format!("limit is from 1 to {MAX_LIMIT}"),
            ));
        }

        let after = from.map(|text| after_of(list, text)).transpose()?;

        Ok(Self { limit, after })
    }
}

impl<K> Page<K> {
    /// How many rows to read: one more than the page, to know whether
    /// another page follows.
    pub fn rows(&self) -> i64 {
        i64::from(self.limit) + 1
    }

    /// The entries of the page among the `rows` read, and whether another
    /// page follows.
    pub fn split<T>(&self, mut rows: Vec<T>) -> (Vec<T>, bool) {
        let limit = usize::try_from(self.limit).unwrap_or(usize::MAX);
        let has_more = rows.len() > limit;

        rows.truncate(limit);
        (rows, has_more)
    }
}

fn after_of<K: DeserializeOwned>(list: &str, text: &str) -> Result<K, ApiError> {
    let invalid = || {
        ApiError::new(
            ErrorCode::InvalidCursor,
            format!("`{text}` is not a cursor of this list"),
        )
    };
    let bytes = URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    // Postgres refuses a NUL in a text, so a forged key with one would fail
    // the query instead of the cursor.
    if holds_nul(&value) {
        return Err(invalid());
    }
    let cursor: Cursor<K> = serde_json::from_value(value).map_err(|_| invalid())?;

    if cursor.list != list {
        return Err(invalid());
    }

    Ok(cursor.after)
}

fn holds_nul(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains('\0'),
        Value::Array(values) => values.iter().any(holds_nul),
        Value::Object(fields) => fields
            .iter()
            .any(|(name, value)| name.contains('\0') || holds_nul(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// The cursor that gives the page after the entry whose sort key is `after`,
/// in `list`: opaque to a client, which only passes it back as `from`.
pub fn cursor_of<K: Serialize>(list: &str, after: K) -> String {
    let cursor = Cursor {
        list: list.to_owned(),
        after,
    };

    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn a_cursor_gives_back_its_key_in_its_list_only() {
        let cursor = cursor_of("prs", ("2026-10-11T09:30:00Z", "item_1"));

        let page = Page::<(String, String)>::read("prs", Some(&cursor), Some(10));
        let other = Page::<(String, String)>::read("queues", Some(&cursor), None);
        let broken = Page::<(String, String)>::read("prs", Some("not a cursor"), None);

        assert_eq!(
            page.ok().and_then(|page| page.after),
            Some(("2026-10-11T09:30:00Z".to_owned(), "item_1".to_owned()))
        );
        assert!(other.is_err_and(|err| err.code == ErrorCode::InvalidCursor));
        assert!(broken.is_err_and(|err| err.code == ErrorCode::InvalidCursor));
    }

    #[test]
    fn a_cursor_time_is_read_as_rfc_3339_only() {
        let valid = cursor_of("prs", ("2026-10-11T09:30:00Z", "item_1"));
        let forged = cursor_of("prs", ("garbage", "item_1"));

        let page = Page::<(CursorTime, String)>::read("prs", Some(&valid), None);
        let refused = Page::<(CursorTime, String)>::read("prs", Some(&forged), None);

        assert_eq!(
            page.ok().and_then(|page| page.after).map(|(at, _)| at.0),
            Some(datetime!(2026-10-11 09:30:00 UTC))
        );
        assert!(refused.is_err_and(|err| err.code == ErrorCode::InvalidCursor));
    }

    #[test]
    fn a_cursor_with_a_nul_character_is_refused() {
        let forged = cursor_of("prs", ("2026-10-11T09:30:00Z", "item\u{0}1"));

        let refused = Page::<(CursorTime, String)>::read("prs", Some(&forged), None);

        assert!(refused.is_err_and(|err| err.code == ErrorCode::InvalidCursor));
    }

    #[test]
    fn a_page_has_from_1_to_100_entries_and_tells_whether_more_follow() {
        let page = Page::<String>::read("prs", None, Some(2)).unwrap();

        assert!(Page::<String>::read("prs", None, Some(0)).is_err());
        assert!(Page::<String>::read("prs", None, Some(101)).is_err());
        assert_eq!(page.rows(), 3);
        assert_eq!(page.split(vec![1, 2, 3]), (vec![1, 2], true));
        assert_eq!(page.split(vec![1, 2]), (vec![1, 2], false));
    }
}
