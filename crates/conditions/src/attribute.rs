use std::fmt;

use cihub_domain::{Permission, ReviewDecision};

/// Value type of an attribute. It decides which operators are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bool,
    Int,
    Duration,
    Text,
    List,
    /// A closed set of values, in rank order; `ordered` allows `<`, `<=`,
    /// `>` and `>=`.
    Choice {
        values: &'static [&'static str],
        ordered: bool,
    },
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("boolean"),
            Self::Int => f.write_str("integer"),
            Self::Duration => f.write_str("duration"),
            Self::Text => f.write_str("string"),
            Self::List => f.write_str("list"),
            Self::Choice { values, .. } => write!(f, "one of {}", values.join(", ")),
        }
    }
}

macro_rules! attributes {
    ($($variant:ident => $name:literal, $kind:expr;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Attribute { $($variant),+ }

        impl Attribute {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }

            pub(crate) const fn kind(self) -> Kind {
                match self { $(Self::$variant => $kind),+ }
            }
        }
    };
}

attributes! {
    Draft => "draft", Kind::Bool;
    Conflict => "conflict", Kind::Bool;
    FromFork => "from-fork", Kind::Bool;
    BehindBase => "behind-base", Kind::Bool;
    Number => "number", Kind::Int;
    Title => "title", Kind::Text;
    Body => "body", Kind::Text;
    ReviewDecision => "review-decision", Kind::Choice { values: REVIEW_DECISION_NAMES, ordered: false };
    ApprovedReviewsBy => "approved-reviews-by", Kind::List;
    ChangesRequestedReviewsBy => "changes-requested-reviews-by", Kind::List;
    ReviewRequested => "review-requested", Kind::List;
    ReviewThreadsUnresolved => "review-threads-unresolved", Kind::List;
    CheckSuccess => "check-success", Kind::List;
    CheckNeutral => "check-neutral", Kind::List;
    CheckSkipped => "check-skipped", Kind::List;
    CheckFailure => "check-failure", Kind::List;
    CheckPending => "check-pending", Kind::List;
    Label => "label", Kind::List;
    Author => "author", Kind::Text;
    Sender => "sender", Kind::Text;
    SenderPermission => "sender-permission", Kind::Choice { values: PERMISSION_NAMES, ordered: true };
    Files => "files", Kind::List;
    Additions => "additions", Kind::Int;
    Deletions => "deletions", Kind::Int;
    Base => "base", Kind::Text;
    Head => "head", Kind::Text;
    QueueName => "queue-name", Kind::Text;
    QueuePosition => "queue-position", Kind::Int;
    QueueAge => "queue-age", Kind::Duration;
    PrAge => "pr-age", Kind::Duration;
}

impl Attribute {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Names of the values of `review-decision`, in the order of
/// `review_decision_rank`.
const REVIEW_DECISION_NAMES: &[&str] =
    &["approved", "changes_requested", "review_required", "none"];

pub(crate) const fn review_decision_rank(decision: ReviewDecision) -> i64 {
    match decision {
        ReviewDecision::Approved => 0,
        ReviewDecision::ChangesRequested => 1,
        ReviewDecision::ReviewRequired => 2,
        ReviewDecision::None => 3,
    }
}

/// Names of the values of `sender-permission`, in the order of
/// `permission_rank` (`AGENTS.md` D145).
const PERMISSION_NAMES: &[&str] = &["none", "read", "write", "admin"];

pub(crate) const fn permission_rank(permission: Permission) -> i64 {
    match permission {
        Permission::None => 0,
        Permission::Read => 1,
        Permission::Write => 2,
        Permission::Admin => 3,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_and_round_trip() {
        let names: HashSet<_> = Attribute::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(names.len(), Attribute::ALL.len());
        for &attribute in Attribute::ALL {
            assert_eq!(Attribute::from_name(attribute.name()), Some(attribute));
        }
    }

    #[test]
    fn choice_names_follow_the_rank() {
        for decision in [
            ReviewDecision::Approved,
            ReviewDecision::ChangesRequested,
            ReviewDecision::ReviewRequired,
            ReviewDecision::None,
        ] {
            let rank = usize::try_from(review_decision_rank(decision)).unwrap();
            assert_eq!(REVIEW_DECISION_NAMES[rank], decision.as_str());
        }
        for permission in [
            Permission::None,
            Permission::Read,
            Permission::Write,
            Permission::Admin,
        ] {
            let rank = usize::try_from(permission_rank(permission)).unwrap();
            assert_eq!(PERMISSION_NAMES[rank], permission.as_str());
        }
        assert_eq!(REVIEW_DECISION_NAMES.len(), 4);
    }
}
