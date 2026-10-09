use std::fmt;

use crate::{Attribute, Condition, Context, EvalError};

/// A list of conditions, as written under `route_conditions`,
/// `queue_conditions`, `merge_conditions` or a priority rule: all of them must hold.
#[derive(Clone, Debug, Default)]
pub struct Conditions(pub Vec<Expr>);

/// One element of a condition list: a condition, or a block with one key.
#[derive(Clone, Debug)]
pub enum Expr {
    Condition(Condition),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    /// True when the whole list does not hold.
    Not(Vec<Expr>),
}

impl Conditions {
    /// # Errors
    ///
    /// `NotLoaded` if a condition uses an attribute that `ctx` does not have.
    pub fn eval(&self, ctx: &Context) -> Result<bool, EvalError> {
        all(&self.0, ctx)
    }

    /// Tells the caller to load an attribute that is loaded on demand.
    pub fn uses(&self, attribute: Attribute) -> bool {
        self.0.iter().any(|expr| expr.uses(attribute))
    }

    /// The text of each element of the list that does not hold, in order,
    /// to explain why a PR waits (`DECISIONS.md` D142).
    ///
    /// # Errors
    ///
    /// `NotLoaded` if a condition uses an attribute that `ctx` does not have.
    pub fn unmet(&self, ctx: &Context) -> Result<Vec<String>, EvalError> {
        let mut unmet = Vec::new();
        for expr in &self.0 {
            if !expr.eval(ctx)? {
                unmet.push(expr.to_string());
            }
        }
        Ok(unmet)
    }
}

/// A condition as the file writes it; a block as its key and its list.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (key, list) = match self {
            Self::Condition(condition) => return f.write_str(condition.as_str()),
            Self::And(list) => ("and", list),
            Self::Or(list) => ("or", list),
            Self::Not(list) => ("not", list),
        };
        write!(f, "{key}: [")?;
        for (n, expr) in list.iter().enumerate() {
            if n > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{expr}")?;
        }
        f.write_str("]")
    }
}

impl Expr {
    /// # Errors
    ///
    /// `NotLoaded` if a condition uses an attribute that `ctx` does not have.
    pub fn eval(&self, ctx: &Context) -> Result<bool, EvalError> {
        match self {
            Self::Condition(condition) => condition.eval(ctx),
            Self::And(list) => all(list, ctx),
            Self::Or(list) => has(list, ctx, true),
            Self::Not(list) => has(list, ctx, false),
        }
    }

    pub fn uses(&self, attribute: Attribute) -> bool {
        match self {
            Self::Condition(condition) => condition.attribute() == attribute,
            Self::And(list) | Self::Or(list) | Self::Not(list) => {
                list.iter().any(|expr| expr.uses(attribute))
            }
        }
    }
}

fn all(list: &[Expr], ctx: &Context) -> Result<bool, EvalError> {
    has(list, ctx, false).map(|failed| !failed)
}

/// Whether an element of `list` evaluates to `value`. Stops at the first one.
fn has(list: &[Expr], ctx: &Context, value: bool) -> Result<bool, EvalError> {
    for expr in list {
        if expr.eval(ctx)? == value {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use cihub_domain::{CheckConclusion, Permission, ReviewDecision};

    use super::*;
    use crate::Check;

    fn condition(s: &str) -> Expr {
        Expr::Condition(s.parse().unwrap())
    }

    fn holds(condition: &str, ctx: &Context) -> bool {
        condition.parse::<Condition>().unwrap().eval(ctx).unwrap()
    }

    fn pr() -> Context {
        Context {
            number: 42,
            title: "feat: add queue".into(),
            review_decision: ReviewDecision::Approved,
            labels: vec!["ready".into(), "area/ci".into()],
            author: "alice".into(),
            files: Some(vec!["src/queue/plan.rs".into(), "README.md".into()]),
            checks: vec![
                Check {
                    name: "lint".into(),
                    conclusion: Some(CheckConclusion::Success),
                },
                Check {
                    name: "build".into(),
                    conclusion: Some(CheckConclusion::TimedOut),
                },
                Check {
                    name: "e2e".into(),
                    conclusion: None,
                },
            ],
            base: "main".into(),
            pr_age: Duration::from_hours(3),
            ..Context::default()
        }
    }

    #[test]
    fn list_operators_match_any_element_and_ne_matches_none() {
        let ctx = pr();
        assert!(holds("label = ready", &ctx));
        assert!(!holds("label = blocked", &ctx));
        assert!(holds("label != blocked", &ctx));
        assert!(!holds("label != ready", &ctx));
        assert!(holds("-label = do-not-merge", &ctx));
        assert!(holds("label ~= ^area/", &ctx));
        assert!(holds("#label = 2", &ctx));
        assert!(holds("-#label > 2", &ctx));
    }

    #[test]
    fn glob_star_stays_in_one_segment() {
        let ctx = pr();
        assert!(holds("files *= src/**/*.rs", &ctx));
        assert!(!holds("files *= src/*.rs", &ctx));
        assert!(holds("files *= *.md", &ctx));
    }

    #[test]
    fn regex_matches_anywhere() {
        let ctx = pr();
        assert!(holds("title ~= queue", &ctx));
        assert!(!holds("title ~= ^queue", &ctx));
    }

    #[test]
    fn checks_are_grouped_by_conclusion() {
        let ctx = pr();
        assert!(holds("check-success = lint", &ctx));
        assert!(holds("check-failure = build", &ctx));
        assert!(holds("check-pending = e2e", &ctx));
        assert!(!holds("check-success = e2e", &ctx));
    }

    #[test]
    fn unmet_names_each_element_that_does_not_hold() {
        let conditions = Conditions(vec![
            condition("label = ready"),
            condition("label = blocked"),
            Expr::Or(vec![condition("author = bob"), condition("number < 10")]),
            Expr::Not(vec![condition("author = alice")]),
            Expr::And(vec![condition("number = 42")]),
        ]);
        assert_eq!(
            conditions.unmet(&pr()),
            Ok(vec![
                "label = blocked".to_owned(),
                "or: [author = bob, number < 10]".to_owned(),
                "not: [author = alice]".to_owned(),
            ])
        );
        let threads = Conditions(vec![condition("#review-threads-unresolved = 0")]);
        assert_eq!(
            threads.unmet(&pr()),
            Err(EvalError::NotLoaded(Attribute::ReviewThreadsUnresolved))
        );
    }

    #[test]
    fn compares_numbers_durations_and_choices() {
        let mut ctx = pr();
        assert!(holds("number >= 42", &ctx));
        assert!(holds("pr-age > 2h", &ctx));
        assert!(holds("review-decision = approved", &ctx));
        assert!(holds("review-decision != review_required", &ctx));

        ctx.sender = Some("bob".into());
        ctx.sender_permission = Some(Permission::Write);

        assert!(holds("sender-permission >= write", &ctx));
        assert!(!holds("sender-permission >= admin", &ctx));
    }

    #[test]
    fn absent_values_do_not_compare() {
        let ctx = pr();
        assert!(!holds("sender-permission >= none", &ctx));
        assert!(!holds("sender-permission != admin", &ctx));
        assert!(!holds("queue-age < 1000d", &ctx));
        assert!(holds("queue-position = -1", &ctx));
        assert!(!holds("queue-name = main", &ctx));
        assert!(holds("queue-name != main", &ctx));
    }

    #[test]
    fn booleans_and_negation() {
        let mut ctx = pr();
        assert!(holds("-draft", &ctx));
        ctx.draft = true;
        assert!(holds("draft", &ctx));
    }

    #[test]
    fn attributes_loaded_on_demand_fail_when_missing() {
        let ctx = pr();
        let condition: Condition = "#review-threads-unresolved = 0".parse().unwrap();
        assert_eq!(
            condition.eval(&ctx),
            Err(EvalError::NotLoaded(Attribute::ReviewThreadsUnresolved))
        );
    }

    #[test]
    fn evaluates_blocks() {
        let ctx = pr();
        let list = Conditions(vec![
            condition("-draft"),
            Expr::Or(vec![
                condition("label = hotfix"),
                condition("label = ready"),
            ]),
            Expr::Not(vec![
                condition("author = dependabot[bot]"),
                condition("label = ready"),
            ]),
        ]);

        assert_eq!(list.eval(&ctx), Ok(true));
        assert!(list.uses(Attribute::Author));
        assert!(!list.uses(Attribute::Files));
        let and = Conditions(vec![Expr::And(vec![
            condition("label = ready"),
            condition("draft"),
        ])]);

        assert_eq!(and.eval(&ctx), Ok(false));
        assert_eq!(Conditions::default().eval(&ctx), Ok(true));
    }
}
