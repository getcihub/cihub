use crate::{Attribute, Condition, Context, EvalError};

/// A list of conditions, as written under `queue_conditions`,
/// `merge_conditions` or a priority rule: all of them must hold.
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

    use cihub_domain::CheckConclusion;

    use super::*;
    use crate::{Check, Permission, ReviewDecision};

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
    fn compares_numbers_durations_and_choices() {
        let mut ctx = pr();
        assert!(holds("number >= 42", &ctx));
        assert!(holds("pr-age > 2h", &ctx));
        assert!(holds("github-review-decision = approved", &ctx));
        assert!(holds("github-review-decision != review_required", &ctx));

        ctx.sender = Some("bob".into());
        ctx.sender_permission = Some(Permission::Maintain);
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
