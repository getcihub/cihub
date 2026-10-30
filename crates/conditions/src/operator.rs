use std::fmt;

use crate::Attribute;
use crate::attribute::Kind;

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

/// The operators written as a key directly under an attribute, in the order
/// that an error message lists them. `Is` has no key; `CountMin` and
/// `CountMax` are `min` and `max` under `count`.
const DIRECT_KEYS: [Operator; 12] = [
    Operator::Not,
    Operator::Matches,
    Operator::Regex,
    Operator::All,
    Operator::Any,
    Operator::None,
    Operator::AllMatch,
    Operator::AnyMatch,
    Operator::NoneMatch,
    Operator::Min,
    Operator::Max,
    Operator::Count,
];

impl Operator {
    /// The operator of a key directly under an attribute.
    pub fn from_key(key: &str) -> Option<Self> {
        DIRECT_KEYS
            .into_iter()
            .find(|operator| operator.key() == Some(key))
    }

    /// The path of the operator under its attribute (`count.min`); `None`
    /// for `Is`, which has no key.
    pub const fn key(self) -> Option<&'static str> {
        match self {
            Self::Is => None,
            Self::Not => Some("not"),
            Self::Matches => Some("matches"),
            Self::Regex => Some("regex"),
            Self::All => Some("all"),
            Self::Any => Some("any"),
            Self::None => Some("none"),
            Self::AllMatch => Some("all_match"),
            Self::AnyMatch => Some("any_match"),
            Self::NoneMatch => Some("none_match"),
            Self::Min => Some("min"),
            Self::Max => Some("max"),
            Self::Count => Some("count"),
            Self::CountMin => Some("count.min"),
            Self::CountMax => Some("count.max"),
        }
    }

    /// The keys that `attribute` accepts directly under it, for an error
    /// message.
    pub fn keys(attribute: Attribute) -> impl Iterator<Item = &'static str> {
        let kind = attribute.kind();
        DIRECT_KEYS
            .into_iter()
            .filter(move |operator| operator.is_allowed_on(kind))
            .filter_map(Self::key)
    }

    pub(crate) const fn is_allowed_on(self, kind: Kind) -> bool {
        match kind {
            Kind::Bool | Kind::Choice { ordered: false, .. } => matches!(self, Self::Is),
            Kind::Duration => matches!(self, Self::Min | Self::Max),
            Kind::Int | Kind::Choice { ordered: true, .. } => {
                matches!(self, Self::Is | Self::Min | Self::Max)
            }
            Kind::Text => matches!(self, Self::Is | Self::Not | Self::Matches | Self::Regex),
            Kind::List => matches!(
                self,
                Self::Is
                    | Self::All
                    | Self::Any
                    | Self::None
                    | Self::AllMatch
                    | Self::AnyMatch
                    | Self::NoneMatch
                    | Self::Count
                    | Self::CountMin
                    | Self::CountMax
            ),
        }
    }

    /// Whether each value holds on its own, so that an explanation names
    /// only the values that fail.
    pub(crate) const fn splits_values(self, kind: Kind) -> bool {
        match self {
            Self::All | Self::None => true,
            Self::Is => matches!(kind, Kind::List),
            _ => false,
        }
    }

    pub(crate) const fn takes_one_value(self, kind: Kind) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
