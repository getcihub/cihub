//! One condition: an attribute, an operator and its values, as the keys of
//! the file write them (`labels` → `none` → `wip`).

use std::fmt;

use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;

use crate::attribute::{Attribute, Kind};
use crate::context::{Context, EvalError, Value};

/// The key under an attribute; `Is` when the attribute takes its values
/// directly (`draft: false`, `author: [a, b]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    Is,
    Not,
    Matches,
    Regex,
    All,
    Any,
    None,
    AllMatch,
    AnyMatch,
    NoneMatch,
    Min,
    Max,
    Count,
    CountMin,
    CountMax,
}

const KEYS: [(&str, Operator); 14] = [
    ("not", Operator::Not),
    ("matches", Operator::Matches),
    ("regex", Operator::Regex),
    ("all", Operator::All),
    ("any", Operator::Any),
    ("none", Operator::None),
    ("all_match", Operator::AllMatch),
    ("any_match", Operator::AnyMatch),
    ("none_match", Operator::NoneMatch),
    ("min", Operator::Min),
    ("max", Operator::Max),
    ("count", Operator::Count),
    ("count.min", Operator::CountMin),
    ("count.max", Operator::CountMax),
];

impl Operator {
    /// The operator of a key directly under an attribute. `count.min` and
    /// `count.max` are not keys: they are `min` and `max` under `count`.
    pub fn from_key(key: &str) -> Option<Self> {
        KEYS.iter()
            .find_map(|&(name, op)| (name == key && !name.contains('.')).then_some(op))
    }

    /// The operator of a path under an attribute, `count.min` included;
    /// `None` for an unknown path.
    pub fn from_path(path: &str) -> Option<Self> {
        KEYS.iter()
            .find_map(|&(name, op)| (name == path).then_some(op))
    }

    /// Each value of the operator holds on its own on `attribute`: two
    /// conditions of it are one condition with both values.
    pub fn holds_per_value(self, attribute: Attribute) -> bool {
        self.splits(attribute.kind())
    }

    /// The path of the operator under its attribute (`count.min`); `None`
    /// for `Is`, which has no key.
    pub fn key(self) -> Option<&'static str> {
        KEYS.iter()
            .find_map(|&(name, op)| (op == self).then_some(name))
    }

    /// The keys that `attribute` accepts directly under it, for an error
    /// message.
    pub fn keys(attribute: Attribute) -> impl Iterator<Item = &'static str> {
        let kind = attribute.kind();
        KEYS.iter()
            .filter(move |(name, op)| !name.contains('.') && allows(kind, *op))
            .map(|(name, _)| *name)
    }

    /// An operator whose values each hold on their own, so that an
    /// explanation names only the values that fail.
    const fn splits(self, kind: Kind) -> bool {
        match self {
            Self::All | Self::None => true,
            Self::Is => matches!(kind, Kind::List),
            _ => false,
        }
    }

    /// An operator that takes one value, never a list.
    const fn takes_one(self, kind: Kind) -> bool {
        match self {
            Self::Regex | Self::Min | Self::Max | Self::Count | Self::CountMin | Self::CountMax => {
                true
            }
            Self::Is => matches!(kind, Kind::Bool | Kind::Int),
            _ => false,
        }
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key().unwrap_or("a value"))
    }
}

/// A built condition. It keeps its values as written, which explanations
/// show.
#[derive(Clone, Debug)]
pub struct Condition {
    attribute: Attribute,
    operator: Operator,
    values: Vec<String>,
    test: Test,
}

#[derive(Clone, Debug)]
enum Test {
    Flag(bool),
    /// An element is one of the values.
    Has(Vec<String>),
    /// No element is one of the values.
    HasNone(Vec<String>),
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

/// A glob as `matches` reads it: `*` and `?` stay in one `/` segment, `**`
/// crosses them.
#[derive(Clone, Debug)]
pub struct Glob(GlobMatcher);

impl Glob {
    /// # Errors
    ///
    /// `pattern` is not a valid glob.
    pub fn new(pattern: &str) -> Result<Self, ConditionErrorKind> {
        GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map(|glob| Self(glob.compile_matcher()))
            .map_err(|e| ConditionErrorKind::InvalidGlob(e.kind().to_string()))
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
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
        let error = |value, kind| ConditionError { value, kind };
        if !allows(kind, operator) {
            let kind = match operator {
                Operator::Is => ConditionErrorKind::KeyExpected { attribute },
                _ => ConditionErrorKind::OperatorNotAllowed { attribute },
            };
            return Err(error(None, kind));
        }

        if values.is_empty() {
            return Err(error(None, ConditionErrorKind::MissingValue));
        }

        if values.len() > 1 && operator.takes_one(kind) {
            return Err(error(Some(1), ConditionErrorKind::OneValue));
        }

        if !operator.splits(kind) {
            let test =
                test(attribute, operator, values).map_err(|(n, kind)| error(Some(n), kind))?;
            return Ok(vec![Self::new(attribute, operator, values, test)]);
        }

        values
            .iter()
            .enumerate()
            .map(|(n, value)| {
                let one = std::slice::from_ref(value);
                test(attribute, operator, one)
                    .map(|test| Self::new(attribute, operator, one, test))
                    .map_err(|(_, kind)| error(Some(n), kind))
            })
            .collect()
    }

    fn new(attribute: Attribute, operator: Operator, values: &[String], test: Test) -> Self {
        Self {
            attribute,
            operator,
            values: values.to_vec(),
            test,
        }
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
        match self.test {
            Test::Range(min, max) => Some((min, max)),
            _ => None,
        }
    }

    /// The name of the check when the condition is one name of
    /// `checks.success`, the only form of `merge_conditions` for now.
    pub fn required_check(&self) -> Option<&str> {
        match (&self.test, self.attribute, self.operator) {
            (Test::Has(names), Attribute::CheckSuccess, Operator::Is | Operator::All) => {
                names.first().map(String::as_str)
            }
            _ => None,
        }
    }

    /// # Errors
    ///
    /// `NotLoaded` if the attribute is loaded on demand and `ctx` does not
    /// have it.
    pub fn eval(&self, ctx: &Context) -> Result<bool, EvalError> {
        let attribute = self.attribute;
        Ok(match (&self.test, ctx.value(attribute)?) {
            (Test::Flag(expected), Value::Flag(flag)) => flag == *expected,
            (Test::Has(values), Value::Texts(items)) => has(&items, values),
            (Test::HasNone(values), Value::Texts(items)) => !has(&items, values),
            (Test::Match(matching, globs), Value::Texts(items)) => {
                matches(*matching, globs, &items)
            }
            (Test::Regex(regex), Value::Texts(items)) => items.iter().any(|i| regex.is_match(i)),
            (Test::Range(min, max), Value::Texts(items)) => {
                (*min..=*max).contains(&i64::try_from(items.len()).unwrap_or(i64::MAX))
            }
            (Test::Range(min, max), Value::Number(actual)) => {
                actual.is_some_and(|actual| (*min..=*max).contains(&actual))
            }
            (Test::OneOf(ranks), Value::Number(actual)) => {
                actual.is_some_and(|actual| ranks.contains(&actual))
            }
            _ => unreachable!("`build` checks the kind of `{attribute}`"),
        })
    }
}

fn has(items: &[&str], values: &[String]) -> bool {
    items
        .iter()
        .any(|item| values.iter().any(|value| value == item))
}

fn matches(matching: Matching, globs: &[Glob], items: &[&str]) -> bool {
    let hit = |item: &&str| globs.iter().any(|glob| glob.is_match(item));
    match matching {
        Matching::Any => items.iter().any(hit),
        Matching::All => !items.is_empty() && items.iter().all(hit),
        Matching::None => !items.iter().any(hit),
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

/// An error in a condition; `value` is the index of the faulty value, `None`
/// when the operator itself is faulty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionError {
    pub value: Option<usize>,
    pub kind: ConditionErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionErrorKind {
    OperatorNotAllowed {
        attribute: Attribute,
    },
    /// A value where the attribute takes only keys (a duration).
    KeyExpected {
        attribute: Attribute,
    },
    MissingValue,
    OneValue,
    InvalidBoolean(String),
    InvalidNumber(String),
    InvalidDuration(String),
    InvalidChoice {
        attribute: Attribute,
        value: String,
    },
    InvalidRegex(String),
    InvalidGlob(String),
}

impl fmt::Display for ConditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl fmt::Display for ConditionErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ConditionErrorKind as K;
        match self {
            K::OperatorNotAllowed { attribute } => {
                write!(f, "not allowed on this attribute ({})", attribute.kind())
            }
            K::KeyExpected { attribute } => {
                let keys = Operator::keys(*attribute).collect::<Vec<_>>().join(", ");
                write!(f, "expected a key ({keys}), not a value")
            }
            K::MissingValue => f.write_str("missing value"),
            K::OneValue => f.write_str("takes one value, not a list"),
            K::InvalidBoolean(v) => write!(f, "`{v}` is not `true` or `false`"),
            K::InvalidNumber(v) => write!(f, "`{v}` is not an integer from 0 to {}", u32::MAX),
            K::InvalidDuration(v) => {
                write!(f, "`{v}` is not a duration such as `30m`, `2h` or `7d`")
            }
            K::InvalidChoice { attribute, value } => {
                write!(f, "`{value}` is not valid: expected {}", attribute.kind())
            }
            K::InvalidRegex(e) => write!(f, "invalid regular expression: {e}"),
            K::InvalidGlob(e) => write!(f, "invalid glob: {e}"),
        }
    }
}

impl std::error::Error for ConditionError {}

fn allows(kind: Kind, operator: Operator) -> bool {
    use Operator as O;
    match kind {
        Kind::Bool | Kind::Choice { ordered: false, .. } => operator == O::Is,
        Kind::Duration => matches!(operator, O::Min | O::Max),
        Kind::Int | Kind::Choice { ordered: true, .. } => {
            matches!(operator, O::Is | O::Min | O::Max)
        }
        Kind::Text => matches!(operator, O::Is | O::Not | O::Matches | O::Regex),
        Kind::List => matches!(
            operator,
            O::Is
                | O::All
                | O::Any
                | O::None
                | O::AllMatch
                | O::AnyMatch
                | O::NoneMatch
                | O::Count
                | O::CountMin
                | O::CountMax
        ),
    }
}

/// The test of `values`, which `build` has checked against the operator; an
/// error gives the index of its value.
fn test(
    attribute: Attribute,
    operator: Operator,
    values: &[String],
) -> Result<Test, (usize, ConditionErrorKind)> {
    use Operator as O;
    let first = values.first().map(String::as_str).unwrap_or_default();
    let at_first = |kind| (0, kind);
    let globs = |matching| {
        values
            .iter()
            .enumerate()
            .map(|(n, value)| Glob::new(value).map_err(|kind| (n, kind)))
            .collect::<Result<_, _>>()
            .map(|globs| Test::Match(matching, globs))
    };

    match (attribute.kind(), operator) {
        (Kind::Bool, _) => match first {
            "true" => Ok(Test::Flag(true)),
            "false" => Ok(Test::Flag(false)),
            _ => Err(at_first(ConditionErrorKind::InvalidBoolean(
                first.to_owned(),
            ))),
        },
        (Kind::Choice { .. }, O::Is) => values
            .iter()
            .enumerate()
            .map(|(n, value)| rank(attribute, value).map_err(|kind| (n, kind)))
            .collect::<Result<_, _>>()
            .map(Test::OneOf),
        (Kind::Int | Kind::Duration | Kind::Choice { .. }, _)
        | (_, O::Count | O::CountMin | O::CountMax) => {
            let bound = match operator {
                O::Count | O::CountMin | O::CountMax => count(first),
                _ => number(attribute, first),
            }
            .map_err(at_first)?;
            Ok(match operator {
                O::Min | O::CountMin => Test::Range(bound, i64::MAX),
                O::Max | O::CountMax => Test::Range(i64::MIN, bound),
                _ => Test::Range(bound, bound),
            })
        }
        (_, O::Regex) => Regex::new(first)
            .map(Test::Regex)
            .map_err(|e| at_first(ConditionErrorKind::InvalidRegex(regex_cause(&e)))),
        (_, O::Matches | O::AnyMatch) => globs(Matching::Any),
        (_, O::AllMatch) => globs(Matching::All),
        (_, O::NoneMatch) => globs(Matching::None),
        (_, O::Not | O::None) => Ok(Test::HasNone(values.to_vec())),
        _ => Ok(Test::Has(values.to_vec())),
    }
}

/// An integer, a duration in seconds, or the rank of an ordered choice.
fn number(attribute: Attribute, value: &str) -> Result<i64, ConditionErrorKind> {
    match attribute.kind() {
        Kind::Duration => {
            parse_duration(value).ok_or_else(|| ConditionErrorKind::InvalidDuration(value.into()))
        }
        Kind::Choice { .. } => rank(attribute, value),
        _ => count(value),
    }
}

fn count(value: &str) -> Result<i64, ConditionErrorKind> {
    value
        .parse::<u32>()
        .map(i64::from)
        .map_err(|_| ConditionErrorKind::InvalidNumber(value.into()))
}

fn rank(attribute: Attribute, value: &str) -> Result<i64, ConditionErrorKind> {
    let Kind::Choice { values, .. } = attribute.kind() else {
        unreachable!("`{attribute}` is not a choice");
    };
    values
        .iter()
        .position(|v| *v == value)
        .and_then(|rank| i64::try_from(rank).ok())
        .ok_or_else(|| ConditionErrorKind::InvalidChoice {
            attribute,
            value: value.into(),
        })
}

/// The last line of a `regex` error. The lines before it repeat the pattern
/// with a caret, which breaks a one-line report.
fn regex_cause(error: &regex::Error) -> String {
    let message = error.to_string();
    let cause = message.lines().last().unwrap_or_default();
    cause.strip_prefix("error: ").unwrap_or(cause).to_owned()
}

/// Seconds in `<digits>m`, `<digits>h` or `<digits>d`.
fn parse_duration(value: &str) -> Option<i64> {
    let unit = match value.chars().last()? {
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return None,
    };
    let digits = &value[..value.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok()?.checked_mul(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_operator_knows_whether_each_value_holds_on_its_own() {
        assert_eq!(Operator::from_path("count.min"), Some(Operator::CountMin));
        assert_eq!(Operator::from_path("some"), None);
        assert!(Operator::None.holds_per_value(Attribute::Labels));
        assert!(Operator::Is.holds_per_value(Attribute::Labels));
        assert!(!Operator::Any.holds_per_value(Attribute::Labels));
        assert!(!Operator::Is.holds_per_value(Attribute::Author));

        let built = build(Attribute::Labels, Operator::None, &["a"]);
        let [condition] = built.as_slice() else {
            panic!("one condition");
        };
        assert_eq!(condition.operator(), Operator::None);
        assert_eq!(condition.values(), ["a"]);
    }

    fn values(list: &[&str]) -> Vec<String> {
        list.iter().map(|&v| v.to_owned()).collect()
    }

    fn build(attribute: Attribute, operator: Operator, list: &[&str]) -> Vec<Condition> {
        Condition::build(attribute, operator, &values(list)).unwrap()
    }

    fn error(attribute: Attribute, operator: Operator, list: &[&str]) -> ConditionError {
        Condition::build(attribute, operator, &values(list)).unwrap_err()
    }

    #[test]
    fn operator_keys_round_trip() {
        for &attribute in Attribute::ALL {
            for key in Operator::keys(attribute) {
                assert_eq!(Operator::from_key(key).and_then(Operator::key), Some(key));
            }
        }
        assert_eq!(Operator::Is.key(), None);
        assert_eq!(Operator::from_key("is"), None);
        assert_eq!(Operator::from_key("count.min"), None);
        assert_eq!(Operator::CountMin.key(), Some("count.min"));
        assert_eq!(
            Operator::keys(Attribute::Additions).collect::<Vec<_>>(),
            ["min", "max"]
        );
        assert_eq!(Operator::keys(Attribute::Draft).count(), 0);
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
    fn only_one_name_of_checks_success_is_a_required_check() {
        let names = |attribute, operator, list: &[&str]| {
            build(attribute, operator, list)
                .iter()
                .map(|c| c.required_check().map(str::to_owned))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            names(Attribute::CheckSuccess, Operator::All, &["a", "b"]),
            [Some("a".into()), Some("b".into())]
        );
        assert_eq!(
            names(Attribute::CheckSuccess, Operator::Any, &["a"]),
            [None]
        );
        assert_eq!(names(Attribute::Labels, Operator::Is, &["a"]), [None]);
        assert_eq!(
            names(Attribute::CheckSuccess, Operator::None, &["a"]),
            [None]
        );
    }

    #[test]
    fn a_glob_stays_in_one_segment_unless_double() {
        let release = Glob::new("release/*").unwrap();
        assert!(release.is_match("release/2.0"));
        assert!(!release.is_match("release/2.0/fix"));
        assert!(!release.is_match("main"));
        assert!(Glob::new("team/**").unwrap().is_match("team/a/b"));
        assert!(matches!(
            Glob::new("release/[").unwrap_err(),
            ConditionErrorKind::InvalidGlob(_)
        ));
    }

    #[test]
    fn refuses_an_operator_that_does_not_fit_the_type() {
        use Attribute as A;
        use Operator as O;

        for (attribute, operator) in [
            (A::Draft, O::Not),
            (A::Number, O::Regex),
            (A::ReviewDecision, O::Min),
            (A::Title, O::Count),
            (A::Labels, O::Regex),
        ] {
            assert_eq!(
                error(attribute, operator, &["x"]),
                ConditionError {
                    value: None,
                    kind: ConditionErrorKind::OperatorNotAllowed { attribute },
                },
                "{attribute}.{operator}"
            );
        }
    }

    #[test]
    fn reports_errors_with_the_index_of_their_value() {
        use Attribute as A;
        use ConditionErrorKind as K;
        use Operator as O;
        let choice = |attribute, value: &str| K::InvalidChoice {
            attribute,
            value: value.into(),
        };
        let cases = [
            (A::Labels, O::None, vec![], None, K::MissingValue),
            (A::Additions, O::Max, vec!["1", "2"], Some(1), K::OneValue),
            (
                A::Draft,
                O::Is,
                vec!["yes"],
                Some(0),
                K::InvalidBoolean("yes".into()),
            ),
            (
                A::Number,
                O::Is,
                vec!["3.5"],
                Some(0),
                K::InvalidNumber("3.5".into()),
            ),
            (
                A::Files,
                O::Count,
                vec!["-1"],
                Some(0),
                K::InvalidNumber("-1".into()),
            ),
            (
                A::QueueAge,
                O::Min,
                vec!["2x"],
                Some(0),
                K::InvalidDuration("2x".into()),
            ),
            (
                A::ReviewDecision,
                O::Is,
                vec!["approved", "ok"],
                Some(1),
                choice(A::ReviewDecision, "ok"),
            ),
            (
                A::SenderPermission,
                O::Min,
                vec!["god"],
                Some(0),
                choice(A::SenderPermission, "god"),
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
            error(A::Title, O::Regex, &["("]).kind,
            K::InvalidRegex(_)
        ));
        let glob = error(A::Files, O::AnyMatch, &["src/**", "a["]);
        assert_eq!(glob.value, Some(1));
        assert!(matches!(glob.kind, K::InvalidGlob(_)));
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

    #[test]
    fn parses_durations_in_seconds() {
        assert_eq!(parse_duration("30m"), Some(1_800));
        assert_eq!(parse_duration("2h"), Some(7_200));
        assert_eq!(parse_duration("7d"), Some(604_800));
        assert_eq!(parse_duration("m"), None);
        assert_eq!(parse_duration("+1h"), None);
        assert_eq!(parse_duration("99999999999999999d"), None);
    }
}
