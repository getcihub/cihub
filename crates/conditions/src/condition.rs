//! One condition: `[-] [#] attribute [operator value]`.

use std::fmt;
use std::str::FromStr;

use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;

use crate::attribute::{Attribute, Kind};
use crate::context::{Context, EvalError, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    Eq,
    Ne,
    Regex,
    Glob,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Two-character symbols first, so that `<=` is not read as `<`.
const OPERATORS: [(&str, Operator); 8] = [
    ("!=", Operator::Ne),
    ("~=", Operator::Regex),
    ("*=", Operator::Glob),
    ("<=", Operator::Le),
    (">=", Operator::Ge),
    ("=", Operator::Eq),
    ("<", Operator::Lt),
    (">", Operator::Gt),
];

impl Operator {
    pub fn symbol(self) -> &'static str {
        OPERATORS
            .iter()
            .find_map(|&(symbol, op)| (op == self).then_some(symbol))
            .unwrap_or_default()
    }

    const fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// A parsed condition. It keeps its source text, which explanations show
/// as written.
#[derive(Clone, Debug)]
pub struct Condition {
    source: String,
    negated: bool,
    test: Test,
}

#[derive(Clone, Debug)]
enum Test {
    Flag(Attribute),
    Text {
        attribute: Attribute,
        op: Operator,
        pattern: Pattern,
    },
    /// Integers, durations in seconds, choices by rank, and `#` list lengths.
    Compare {
        attribute: Attribute,
        op: Operator,
        value: i64,
    },
}

#[derive(Clone, Debug)]
enum Pattern {
    Exact(String),
    Regex(Regex),
    Glob(GlobMatcher),
}

impl Condition {
    pub fn attribute(&self) -> Attribute {
        match self.test {
            Test::Flag(attribute)
            | Test::Text { attribute, .. }
            | Test::Compare { attribute, .. } => attribute,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// The name of the check when the condition is `check-success = <name>`,
    /// the only form of `merge_conditions` for now (`DECISIONS.md` D146).
    pub fn required_check(&self) -> Option<&str> {
        match &self.test {
            Test::Text {
                attribute: Attribute::CheckSuccess,
                op: Operator::Eq,
                pattern: Pattern::Exact(name),
            } if !self.negated => Some(name),
            _ => None,
        }
    }

    /// # Errors
    ///
    /// `NotLoaded` if the attribute is loaded on demand and `ctx` does not
    /// have it.
    pub fn eval(&self, ctx: &Context) -> Result<bool, EvalError> {
        let attribute = self.attribute();
        let matched = match (&self.test, ctx.value(attribute)?) {
            (Test::Flag(_), Value::Flag(flag)) => flag,
            (Test::Text { op, pattern, .. }, Value::Texts(items)) => {
                text_matches(*op, pattern, &items)
            }
            (Test::Compare { op, value, .. }, Value::Texts(items)) => {
                compare(*op, i64::try_from(items.len()).unwrap_or(i64::MAX), *value)
            }
            (Test::Compare { op, value, .. }, Value::Number(actual)) => {
                actual.is_some_and(|actual| compare(*op, actual, *value))
            }
            _ => unreachable!("the parser checks the kind of `{attribute}`"),
        };
        Ok(matched != self.negated)
    }
}

fn text_matches(op: Operator, pattern: &Pattern, items: &[&str]) -> bool {
    match pattern {
        Pattern::Exact(value) => {
            let any = items.contains(&value.as_str());
            if op == Operator::Ne { !any } else { any }
        }
        Pattern::Regex(regex) => items.iter().any(|item| regex.is_match(item)),
        Pattern::Glob(glob) => items.iter().any(|item| glob.is_match(item)),
    }
}

fn compare(op: Operator, actual: i64, value: i64) -> bool {
    match op {
        Operator::Eq => actual == value,
        Operator::Ne => actual != value,
        Operator::Lt => actual < value,
        Operator::Le => actual <= value,
        Operator::Gt => actual > value,
        Operator::Ge => actual >= value,
        Operator::Regex | Operator::Glob => unreachable!("the parser rejects `{op}` here"),
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

/// A parse error, at a 1-based character column of the condition text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub column: usize,
    pub kind: ParseErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    Empty,
    UnknownAttribute(String),
    CountNeedsList(Attribute),
    ExpectedOperator,
    MissingOperator(Attribute),
    OperatorOnBoolean(Attribute),
    OperatorNotAllowed {
        attribute: Attribute,
        operator: Operator,
    },
    MissingValue,
    UnterminatedQuote,
    InvalidEscape(char),
    TrailingCharacters,
    InvalidNumber(String),
    InvalidDuration(String),
    InvalidChoice {
        attribute: Attribute,
        value: String,
    },
    InvalidRegex(String),
    InvalidGlob(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.kind)
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ParseErrorKind as K;
        match self {
            K::Empty => f.write_str("empty condition"),
            K::UnknownAttribute(name) => write!(f, "unknown attribute `{name}`"),
            K::CountNeedsList(a) => write!(f, "`#` needs a list, and `{a}` is a {}", a.kind()),
            K::ExpectedOperator => f.write_str("expected an operator"),
            K::MissingOperator(a) => write!(f, "`{a}` needs an operator and a value"),
            K::OperatorOnBoolean(a) => {
                write!(f, "`{a}` is a boolean: write `{a}` or `-{a}`")
            }
            K::OperatorNotAllowed {
                attribute,
                operator,
            } => write!(
                f,
                "operator `{operator}` is not allowed on `{attribute}` ({})",
                attribute.kind()
            ),
            K::MissingValue => f.write_str("missing value"),
            K::UnterminatedQuote => f.write_str("unterminated quoted value"),
            K::InvalidEscape(c) => write!(f, "invalid escape `\\{c}`: use `\\\"` or `\\\\`"),
            K::TrailingCharacters => f.write_str("unexpected text after the quoted value"),
            K::InvalidNumber(v) => write!(f, "`{v}` is not an integer"),
            K::InvalidDuration(v) => {
                write!(f, "`{v}` is not a duration such as `30m`, `2h` or `7d`")
            }
            K::InvalidChoice { attribute, value } => {
                write!(
                    f,
                    "`{value}` is not valid for `{attribute}`: expected {}",
                    attribute.kind()
                )
            }
            K::InvalidRegex(e) => write!(f, "invalid regular expression: {e}"),
            K::InvalidGlob(e) => write!(f, "invalid glob: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl FromStr for Condition {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (negated, test) = Cursor { s, pos: 0 }.condition()?;
        Ok(Self {
            source: s.trim().to_owned(),
            negated,
            test,
        })
    }
}

struct Cursor<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn at_end(&self) -> bool {
        self.pos == self.s.len()
    }

    fn skip_whitespace(&mut self) {
        self.pos = self.s.len() - self.rest().trim_start().len();
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.rest().starts_with(c);
        if found {
            self.pos += c.len_utf8();
        }
        found
    }

    fn error(&self, at: usize, kind: ParseErrorKind) -> ParseError {
        ParseError {
            column: self.s[..at].chars().count() + 1,
            kind,
        }
    }

    fn condition(mut self) -> Result<(bool, Test), ParseError> {
        self.skip_whitespace();
        if self.at_end() {
            return Err(self.error(self.pos, ParseErrorKind::Empty));
        }
        let negated = self.eat('-');
        let count_at = self.pos;
        let count = self.eat('#');

        let name_at = self.pos;
        let name_len = self
            .rest()
            .find(|c: char| c.is_whitespace() || "=!~*<>".contains(c))
            .unwrap_or(self.rest().len());
        let name = &self.rest()[..name_len];
        let attribute = Attribute::from_name(name).ok_or_else(|| {
            self.error(name_at, ParseErrorKind::UnknownAttribute(name.to_owned()))
        })?;
        self.pos += name_len;
        let kind = attribute.kind();
        if count && kind != Kind::List {
            return Err(self.error(count_at, ParseErrorKind::CountNeedsList(attribute)));
        }

        self.skip_whitespace();
        let op_at = self.pos;
        let Some(op) = self.operator() else {
            if !self.at_end() {
                return Err(self.error(op_at, ParseErrorKind::ExpectedOperator));
            }
            if kind != Kind::Bool {
                return Err(self.error(op_at, ParseErrorKind::MissingOperator(attribute)));
            }
            return Ok((negated, Test::Flag(attribute)));
        };
        if kind == Kind::Bool {
            return Err(self.error(op_at, ParseErrorKind::OperatorOnBoolean(attribute)));
        }
        if !allows(kind, count, op) {
            return Err(self.error(
                op_at,
                ParseErrorKind::OperatorNotAllowed {
                    attribute,
                    operator: op,
                },
            ));
        }

        self.skip_whitespace();
        let value_at = self.pos;
        let value = self.value()?;
        let invalid = |kind| self.error(value_at, kind);
        let test = if !count && matches!(kind, Kind::Text | Kind::List) {
            Test::Text {
                attribute,
                op,
                pattern: pattern(op, &value).map_err(invalid)?,
            }
        } else {
            let (number, error) = match kind {
                Kind::Duration => (
                    parse_duration(&value),
                    ParseErrorKind::InvalidDuration(value),
                ),
                Kind::Choice { values, .. } => (
                    values
                        .iter()
                        .position(|v| *v == value)
                        .and_then(|rank| i64::try_from(rank).ok()),
                    ParseErrorKind::InvalidChoice { attribute, value },
                ),
                _ => (value.parse().ok(), ParseErrorKind::InvalidNumber(value)),
            };
            Test::Compare {
                attribute,
                op,
                value: number.ok_or_else(|| invalid(error))?,
            }
        };
        Ok((negated, test))
    }

    fn operator(&mut self) -> Option<Operator> {
        let (symbol, op) = OPERATORS
            .iter()
            .find(|(symbol, _)| self.rest().starts_with(symbol))?;
        self.pos += symbol.len();
        Some(*op)
    }

    /// The rest of the text without its trailing whitespace, or a quoted
    /// string with `\"` and `\\` escapes.
    fn value(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        if !self.eat('"') {
            let value = self.rest().trim_end();
            if value.is_empty() {
                return Err(self.error(start, ParseErrorKind::MissingValue));
            }
            self.pos = self.s.len();
            return Ok(value.to_owned());
        }
        let mut value = String::new();
        let mut chars = self.rest().char_indices();
        loop {
            match chars.next() {
                None => return Err(self.error(start, ParseErrorKind::UnterminatedQuote)),
                Some((i, '"')) => {
                    self.pos += i + 1;
                    break;
                }
                Some((i, '\\')) => match chars.next() {
                    Some((_, c @ ('"' | '\\'))) => value.push(c),
                    Some((_, c)) => {
                        return Err(self.error(self.pos + i, ParseErrorKind::InvalidEscape(c)));
                    }
                    None => return Err(self.error(start, ParseErrorKind::UnterminatedQuote)),
                },
                Some((_, c)) => value.push(c),
            }
        }
        self.skip_whitespace();
        if !self.at_end() {
            return Err(self.error(self.pos, ParseErrorKind::TrailingCharacters));
        }
        Ok(value)
    }
}

fn allows(kind: Kind, count: bool, op: Operator) -> bool {
    match kind {
        _ if count => op.is_comparison(),
        Kind::Bool => false,
        Kind::Int | Kind::Duration | Kind::Choice { ordered: true, .. } => op.is_comparison(),
        Kind::Choice { ordered: false, .. } => matches!(op, Operator::Eq | Operator::Ne),
        Kind::Text | Kind::List => matches!(
            op,
            Operator::Eq | Operator::Ne | Operator::Regex | Operator::Glob
        ),
    }
}

fn pattern(op: Operator, value: &str) -> Result<Pattern, ParseErrorKind> {
    match op {
        Operator::Regex => Regex::new(value)
            .map(Pattern::Regex)
            .map_err(|e| ParseErrorKind::InvalidRegex(regex_cause(&e))),
        Operator::Glob => GlobBuilder::new(value)
            .literal_separator(true)
            .build()
            .map(|glob| Pattern::Glob(glob.compile_matcher()))
            .map_err(|e| ParseErrorKind::InvalidGlob(e.kind().to_string())),
        _ => Ok(Pattern::Exact(value.to_owned())),
    }
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

    fn error(s: &str) -> ParseError {
        s.parse::<Condition>().unwrap_err()
    }

    #[test]
    fn parses_the_spec_examples() {
        for s in [
            "review-decision = approved",
            "-draft",
            "-label = do-not-merge",
            "check-success = lint",
            "label = hotfix",
            "sender-permission >= write",
            "author = dependabot[bot]",
            "#approved-reviews-by >= 2",
            "files *= src/**/*.rs",
            "title ~= ^feat",
            "queue-age > 2h",
        ] {
            let condition: Condition = s.parse().unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_eq!(condition.as_str(), s);
        }
    }

    #[test]
    fn reads_the_value_to_the_end_or_between_quotes() {
        let Test::Text { pattern, .. } = "title = fix: a  b ".parse::<Condition>().unwrap().test
        else {
            panic!("expected a text test");
        };
        assert!(matches!(pattern, Pattern::Exact(v) if v == "fix: a  b"));

        let Test::Text { pattern, .. } =
            r#"title = "a \"b\" \\ c""#.parse::<Condition>().unwrap().test
        else {
            panic!("expected a text test");
        };
        assert!(matches!(pattern, Pattern::Exact(v) if v == r#"a "b" \ c"#));
    }

    #[test]
    fn reports_errors_with_their_column() {
        use ParseErrorKind as K;
        let cases = [
            ("   ", 4, K::Empty),
            ("lable = x", 1, K::UnknownAttribute("lable".into())),
            ("label_x = 1", 1, K::UnknownAttribute("label_x".into())),
            ("#title = 2", 1, K::CountNeedsList(Attribute::Title)),
            ("label", 6, K::MissingOperator(Attribute::Label)),
            ("label : x", 7, K::ExpectedOperator),
            ("-draft = true", 8, K::OperatorOnBoolean(Attribute::Draft)),
            (
                "number ~= 3",
                8,
                K::OperatorNotAllowed {
                    attribute: Attribute::Number,
                    operator: Operator::Regex,
                },
            ),
            (
                "review-decision > approved",
                17,
                K::OperatorNotAllowed {
                    attribute: Attribute::ReviewDecision,
                    operator: Operator::Gt,
                },
            ),
            ("label =   ", 11, K::MissingValue),
            (r#"label = "a"#, 9, K::UnterminatedQuote),
            (r#"label = "a\n""#, 11, K::InvalidEscape('n')),
            (r#"label = "a" b"#, 13, K::TrailingCharacters),
            ("number = 3.5", 10, K::InvalidNumber("3.5".into())),
            ("queue-age > 2x", 13, K::InvalidDuration("2x".into())),
            ("pr-age > -2h", 10, K::InvalidDuration("-2h".into())),
            (
                "sender-permission >= god",
                22,
                K::InvalidChoice {
                    attribute: Attribute::SenderPermission,
                    value: "god".into(),
                },
            ),
        ];
        for (s, column, kind) in cases {
            assert_eq!(error(s), ParseError { column, kind }, "{s}");
        }
        assert!(matches!(error("title ~= (").kind, K::InvalidRegex(_)));
        assert!(matches!(error("files *= a[").kind, K::InvalidGlob(_)));
    }

    #[test]
    fn counts_columns_in_characters() {
        assert_eq!(error("title = \"é\\x\"").column, 11);
    }

    #[test]
    fn error_messages_name_the_problem() {
        assert_eq!(
            error("-draft = true").to_string(),
            "column 8: `draft` is a boolean: write `draft` or `-draft`"
        );
        assert_eq!(
            error("sender-permission >= god").to_string(),
            "column 22: `god` is not valid for `sender-permission`: expected one of none, read, write, admin"
        );
        assert_eq!(
            error("title ~= (").to_string(),
            "column 10: invalid regular expression: unclosed group"
        );
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
