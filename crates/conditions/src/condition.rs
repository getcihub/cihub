//! One condition: an attribute, an operator and its values, as the keys of
//! the file write them (`labels` → `none` → `wip`).

use std::fmt;

use regex::Regex;

use crate::attribute::Kind;
use crate::context::{Value, saturate};
use crate::{
    Attribute, ConditionError, ConditionErrorKind, Context, EvalError, Glob, Operator, parse,
};

/// A built condition. It keeps its values as written, which explanations
/// show.
#[derive(Clone, Debug)]
pub struct Condition {
    attribute: Attribute,
    operator: Operator,
    values: Vec<String>,
    predicate: Predicate,
}

/// What the condition asks of the value of its attribute.
#[derive(Clone, Debug)]
enum Predicate {
    Flag(bool),
    /// An element is one of the values of the condition.
    Has,
    /// No element is one of the values of the condition.
    HasNone,
    Match(Matching, Vec<Glob>),
    Regex(Regex),
    /// Integers, durations in seconds, ordered choices by rank and the number
    /// of elements of a list, inclusive.
    Range(i64, i64),
    /// Choices by rank.
    OneOf(Vec<i64>),
}

#[derive(Clone, Copy, Debug)]
enum Matching {
    Any,
    /// Every element, and at least one.
    All,
    None,
}

impl Condition {
    /// The conditions that `values` under `attribute` and `operator` write:
    /// one per value when each value must hold on its own (`all`, `none`, and
    /// the values of a list attribute), else one.
    ///
    /// # Errors
    ///
    /// The operator does not fit the type of the attribute, or a value is
    /// invalid; `value` gives its index.
    pub fn build(
        attribute: Attribute,
        operator: Operator,
        values: &[String],
    ) -> Result<Vec<Self>, ConditionError> {
        let kind = attribute.kind();
        if !operator.is_allowed_on(kind) {
            let error = match operator {
                Operator::Is => ConditionErrorKind::KeyExpected { attribute },
                _ => ConditionErrorKind::OperatorNotAllowed { attribute },
            };
            return Err(ConditionError::whole(error));
        }

        if values.is_empty() {
            return Err(ConditionError::whole(ConditionErrorKind::MissingValue));
        }

        if values.len() > 1 && operator.takes_one_value(kind) {
            return Err(ConditionError::at(1, ConditionErrorKind::OneValue));
        }

        if !operator.splits_values(kind) {
            return Self::new(attribute, operator, values).map(|condition| vec![condition]);
        }

        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                Self::new(attribute, operator, std::slice::from_ref(value))
                    .map_err(|error| ConditionError::at(index, error.kind))
            })
            .collect()
    }

    fn new(
        attribute: Attribute,
        operator: Operator,
        values: &[String],
    ) -> Result<Self, ConditionError> {
        let predicate = predicate(attribute, operator, values)?;

        Ok(Self {
            attribute,
            operator,
            values: values.to_vec(),
            predicate,
        })
    }

    pub fn attribute(&self) -> Attribute {
        self.attribute
    }

    pub fn operator(&self) -> Operator {
        self.operator
    }

    /// The values as the file writes them.
    pub fn values(&self) -> &[String] {
        &self.values
    }

    /// The inclusive bounds that the condition accepts, when it is a range.
    pub fn bounds(&self) -> Option<(i64, i64)> {
        match self.predicate {
            Predicate::Range(min, max) => Some((min, max)),
            _ => None,
        }
    }

    /// # Errors
    ///
    /// `NotLoaded` if the attribute is loaded on demand and `ctx` does not
    /// have it.
    pub fn eval(&self, ctx: &Context) -> Result<bool, EvalError> {
        let attribute = self.attribute;
        Ok(match (&self.predicate, ctx.value(attribute)?) {
            (Predicate::Flag(expected), Value::Flag(flag)) => flag == *expected,
            (Predicate::Has, Value::Texts(items)) => self.is_any_value_in(&items),
            (Predicate::HasNone, Value::Texts(items)) => !self.is_any_value_in(&items),
            (Predicate::Match(matching, globs), Value::Texts(items)) => {
                matches(*matching, globs, &items)
            }
            (Predicate::Regex(regex), Value::Texts(items)) => {
                items.iter().any(|item| regex.is_match(item))
            }
            (Predicate::Range(min, max), Value::Texts(items)) => {
                (*min..=*max).contains(&saturate(items.len()))
            }
            (Predicate::Range(min, max), Value::Number(actual)) => {
                actual.is_some_and(|actual| (*min..=*max).contains(&actual))
            }
            (Predicate::OneOf(ranks), Value::Number(actual)) => {
                actual.is_some_and(|actual| ranks.contains(&actual))
            }
            _ => unreachable!("`build` checks the kind of `{attribute}`"),
        })
    }

    fn is_any_value_in(&self, items: &[&str]) -> bool {
        items
            .iter()
            .any(|item| self.values.iter().any(|value| value == item))
    }
}

fn matches(matching: Matching, globs: &[Glob], items: &[&str]) -> bool {
    let is_hit = |item: &&str| globs.iter().any(|glob| glob.is_match(item));
    match matching {
        Matching::Any => items.iter().any(is_hit),
        Matching::All => !items.is_empty() && items.iter().all(is_hit),
        Matching::None => !items.iter().any(is_hit),
    }
}

/// The path of the condition and its values, as the file writes them:
/// `labels.none: wip`, `author: alice, bob`.
impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.attribute)?;
        if let Some(key) = self.operator.key() {
            write!(f, ".{key}")?;
        }
        write!(f, ": {}", self.values.join(", "))
    }
}

/// The predicate of `values`, which `build` has checked against the
/// operator: not empty, and one value when the operator takes one.
fn predicate(
    attribute: Attribute,
    operator: Operator,
    values: &[String],
) -> Result<Predicate, ConditionError> {
    let first = values.first().map(String::as_str).unwrap_or_default();
    let at_first = |error| ConditionError::at(0, error);

    match operator {
        Operator::Is => equal_to(attribute, values),
        Operator::Not | Operator::None => Ok(Predicate::HasNone),
        Operator::All | Operator::Any => Ok(Predicate::Has),
        Operator::Matches | Operator::AnyMatch => globs(Matching::Any, values),
        Operator::AllMatch => globs(Matching::All, values),
        Operator::NoneMatch => globs(Matching::None, values),
        Operator::Regex => parse::regex(first).map(Predicate::Regex).map_err(at_first),
        Operator::Min => parse::bound(attribute, first)
            .map(|min| Predicate::Range(min, i64::MAX))
            .map_err(at_first),
        Operator::Max => parse::bound(attribute, first)
            .map(|max| Predicate::Range(i64::MIN, max))
            .map_err(at_first),
        Operator::Count => parse::count(first)
            .map(|count| Predicate::Range(count, count))
            .map_err(at_first),
        Operator::CountMin => parse::count(first)
            .map(|min| Predicate::Range(min, i64::MAX))
            .map_err(at_first),
        Operator::CountMax => parse::count(first)
            .map(|max| Predicate::Range(i64::MIN, max))
            .map_err(at_first),
    }
}

/// The predicate of the values written directly under the attribute.
fn equal_to(attribute: Attribute, values: &[String]) -> Result<Predicate, ConditionError> {
    let first = values.first().map(String::as_str).unwrap_or_default();
    let at_first = |error| ConditionError::at(0, error);

    match attribute.kind() {
        Kind::Bool => parse::boolean(first).map(Predicate::Flag).map_err(at_first),
        Kind::Int | Kind::Duration => parse::bound(attribute, first)
            .map(|number| Predicate::Range(number, number))
            .map_err(at_first),
        Kind::Choice { .. } => {
            each_value(values, |value| parse::rank(attribute, value)).map(Predicate::OneOf)
        }
        Kind::Text | Kind::List => Ok(Predicate::Has),
    }
}

fn globs(matching: Matching, values: &[String]) -> Result<Predicate, ConditionError> {
    each_value(values, Glob::new).map(|globs| Predicate::Match(matching, globs))
}

/// `read` applied to each value; an error gives the index of its value.
fn each_value<T>(
    values: &[String],
    read: impl Fn(&str) -> Result<T, ConditionErrorKind>,
) -> Result<Vec<T>, ConditionError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| read(value).map_err(|error| ConditionError::at(index, error)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(list: &[&str]) -> Vec<String> {
        list.iter().map(|&value| value.to_owned()).collect()
    }

    fn build(attribute: Attribute, operator: Operator, list: &[&str]) -> Vec<Condition> {
        Condition::build(attribute, operator, &values(list)).unwrap()
    }

    fn error(attribute: Attribute, operator: Operator, list: &[&str]) -> ConditionError {
        Condition::build(attribute, operator, &values(list)).unwrap_err()
    }

    #[test]
    fn shows_the_path_and_the_values() {
        let shown = |attribute, operator, list: &[&str]| {
            build(attribute, operator, list)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            shown(Attribute::Draft, Operator::Is, &["false"]),
            ["draft: false"]
        );
        assert_eq!(
            shown(Attribute::Author, Operator::Is, &["a", "b"]),
            ["author: a, b"]
        );
        assert_eq!(
            shown(Attribute::Labels, Operator::None, &["wip", "blocked"]),
            ["labels.none: wip", "labels.none: blocked"]
        );
        assert_eq!(
            shown(Attribute::CheckSuccess, Operator::Is, &["lint", "build"]),
            ["checks.success: lint", "checks.success: build"]
        );
        assert_eq!(
            shown(Attribute::Labels, Operator::Any, &["urgent", "hotfix"]),
            ["labels.any: urgent, hotfix"]
        );
        assert_eq!(
            shown(Attribute::Files, Operator::CountMax, &["3"]),
            ["files.count.max: 3"]
        );
    }

    #[test]
    fn refuses_an_operator_that_does_not_fit_the_type() {
        for (attribute, operator) in [
            (Attribute::Draft, Operator::Not),
            (Attribute::Number, Operator::Regex),
            (Attribute::ReviewDecision, Operator::Min),
            (Attribute::Title, Operator::Count),
            (Attribute::Labels, Operator::Regex),
        ] {
            assert_eq!(
                error(attribute, operator, &["x"]),
                ConditionError::whole(ConditionErrorKind::OperatorNotAllowed { attribute }),
                "{attribute}.{operator}"
            );
        }
    }

    #[test]
    fn reports_errors_with_the_index_of_their_value() {
        let choice = |attribute, value: &str| ConditionErrorKind::InvalidChoice {
            attribute,
            value: value.into(),
        };
        let cases = [
            (
                Attribute::Labels,
                Operator::None,
                vec![],
                None,
                ConditionErrorKind::MissingValue,
            ),
            (
                Attribute::Additions,
                Operator::Max,
                vec!["1", "2"],
                Some(1),
                ConditionErrorKind::OneValue,
            ),
            (
                Attribute::Draft,
                Operator::Is,
                vec!["yes"],
                Some(0),
                ConditionErrorKind::InvalidBoolean("yes".into()),
            ),
            (
                Attribute::Number,
                Operator::Is,
                vec!["3.5"],
                Some(0),
                ConditionErrorKind::InvalidNumber("3.5".into()),
            ),
            (
                Attribute::Files,
                Operator::Count,
                vec!["-1"],
                Some(0),
                ConditionErrorKind::InvalidNumber("-1".into()),
            ),
            (
                Attribute::QueueAge,
                Operator::Min,
                vec!["2x"],
                Some(0),
                ConditionErrorKind::InvalidDuration("2x".into()),
            ),
            (
                Attribute::ReviewDecision,
                Operator::Is,
                vec!["approved", "ok"],
                Some(1),
                choice(Attribute::ReviewDecision, "ok"),
            ),
            (
                Attribute::SenderPermission,
                Operator::Min,
                vec!["god"],
                Some(0),
                choice(Attribute::SenderPermission, "god"),
            ),
        ];

        for (attribute, operator, list, value, kind) in cases {
            assert_eq!(
                error(attribute, operator, &list),
                ConditionError { value, kind },
                "{attribute}.{operator}"
            );
        }

        assert!(matches!(
            error(Attribute::Title, Operator::Regex, &["("]).kind,
            ConditionErrorKind::InvalidRegex(_)
        ));
        let glob = error(Attribute::Files, Operator::AnyMatch, &["src/**", "a["]);
        assert_eq!(glob.value, Some(1));
        assert!(matches!(glob.kind, ConditionErrorKind::InvalidGlob(_)));
    }

    #[test]
    fn error_messages_name_the_problem() {
        let shown =
            |attribute, operator, list: &[&str]| error(attribute, operator, list).to_string();

        assert_eq!(
            shown(Attribute::Draft, Operator::Not, &["x"]),
            "not allowed on this attribute (boolean)"
        );
        assert_eq!(
            shown(Attribute::Draft, Operator::Is, &["true", "false"]),
            "takes one value, not a list"
        );
        assert_eq!(
            shown(Attribute::Title, Operator::Regex, &["a", "b"]),
            "takes one value, not a list"
        );
        assert_eq!(
            shown(Attribute::SenderPermission, Operator::Min, &["god"]),
            "`god` is not valid: expected one of none, read, write, admin"
        );
        assert_eq!(
            shown(Attribute::Title, Operator::Regex, &["("]),
            "invalid regular expression: unclosed group"
        );
        assert_eq!(
            shown(Attribute::PrAge, Operator::Max, &["-2h"]),
            "`-2h` is not a duration such as `30m`, `2h` or `7d`"
        );
        assert_eq!(
            shown(Attribute::Number, Operator::Is, &["x"]),
            "`x` is not an integer from 0 to 4294967295"
        );
        assert_eq!(
            shown(Attribute::Labels, Operator::Any, &[]),
            "missing value"
        );
        assert!(shown(Attribute::Files, Operator::AnyMatch, &["a["]).starts_with("invalid glob: "));
    }

    #[test]
    fn a_duration_takes_keys_not_a_value() {
        for attribute in [Attribute::QueueAge, Attribute::PrAge] {
            let failure = error(attribute, Operator::Is, &["2h"]);

            assert_eq!(failure.value, None);
            assert_eq!(
                failure.to_string(),
                "expected a key (min, max), not a value"
            );
        }
    }

    #[test]
    fn gives_the_bounds_of_a_range() {
        let bounds =
            |attribute, operator, list: &[&str]| build(attribute, operator, list)[0].bounds();

        assert_eq!(
            bounds(Attribute::Additions, Operator::Min, &["3"]),
            Some((3, i64::MAX))
        );
        assert_eq!(
            bounds(Attribute::Additions, Operator::Max, &["5"]),
            Some((i64::MIN, 5))
        );
        assert_eq!(bounds(Attribute::Labels, Operator::Any, &["a"]), None);
    }
}
