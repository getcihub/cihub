//! The values of a condition as the file writes them, read into what the
//! evaluation compares.

use regex::Regex;

use crate::attribute::Kind;
use crate::{Attribute, ConditionErrorKind};

pub(crate) fn boolean(value: &str) -> Result<bool, ConditionErrorKind> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConditionErrorKind::InvalidBoolean(value.into())),
    }
}

/// An integer, a duration in seconds, or the rank of an ordered choice.
pub(crate) fn bound(attribute: Attribute, value: &str) -> Result<i64, ConditionErrorKind> {
    match attribute.kind() {
        Kind::Duration => {
            duration(value).ok_or_else(|| ConditionErrorKind::InvalidDuration(value.into()))
        }
        Kind::Choice { .. } => rank(attribute, value),
        Kind::Bool | Kind::Int | Kind::Text | Kind::List => count(value),
    }
}

pub(crate) fn count(value: &str) -> Result<i64, ConditionErrorKind> {
    value
        .parse::<u32>()
        .map(i64::from)
        .map_err(|_| ConditionErrorKind::InvalidNumber(value.into()))
}

pub(crate) fn rank(attribute: Attribute, value: &str) -> Result<i64, ConditionErrorKind> {
    let Kind::Choice { values, .. } = attribute.kind() else {
        unreachable!("`{attribute}` is not a choice");
    };

    values
        .iter()
        .position(|choice| *choice == value)
        .and_then(|rank| i64::try_from(rank).ok())
        .ok_or_else(|| ConditionErrorKind::InvalidChoice {
            attribute,
            value: value.into(),
        })
}

pub(crate) fn regex(value: &str) -> Result<Regex, ConditionErrorKind> {
    Regex::new(value).map_err(|error| ConditionErrorKind::InvalidRegex(regex_cause(&error)))
}

/// The last line of a `regex` error. The lines before it repeat the pattern
/// with a caret, which breaks a one-line report.
fn regex_cause(error: &regex::Error) -> String {
    let message = error.to_string();
    let cause = message.lines().last().unwrap_or_default();
    cause.strip_prefix("error: ").unwrap_or(cause).to_owned()
}

/// Seconds in `<digits>m`, `<digits>h` or `<digits>d`.
fn duration(value: &str) -> Option<i64> {
    let unit = match value.chars().last()? {
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return None,
    };
    let digits = &value[..value.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    digits.parse::<i64>().ok()?.checked_mul(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations_in_seconds() {
        assert_eq!(duration("30m"), Some(1_800));
        assert_eq!(duration("2h"), Some(7_200));
        assert_eq!(duration("7d"), Some(604_800));
        assert_eq!(duration("m"), None);
        assert_eq!(duration("+1h"), None);
        assert_eq!(duration("99999999999999999d"), None);
    }
}
