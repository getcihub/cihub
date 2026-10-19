use std::fmt;

use crate::{Attribute, Condition, Context, EvalError};

/// The conditions of a map, as written under `queue_conditions`,
/// `merge_conditions`, a profile or a priority rule: all of them must hold.
#[derive(Clone, Debug, Default)]
pub struct Conditions(pub Vec<Expr>);

/// One condition of a map, or a block `all`, `any` or `none` of maps.
#[derive(Clone, Debug)]
pub enum Expr {
    Condition(Condition),
    All(Vec<Expr>),
    Any(Vec<Expr>),
    /// True when no element holds.
    None(Vec<Expr>),
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

    /// The text of each condition that does not hold, in the order of the
    /// file, to explain why a PR waits.
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

/// A condition as its path and its values; a block as its key and its
/// elements, separated by `;` as a value list uses `,`.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (key, list) = match self {
            Self::Condition(condition) => return condition.fmt(f),
            Self::All(list) => ("all", list),
            Self::Any(list) => ("any", list),
            Self::None(list) => ("none", list),
        };
        write!(f, "{key}: [")?;
        for (n, expr) in list.iter().enumerate() {
            if n > 0 {
                f.write_str("; ")?;
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
            Self::All(list) => all(list, ctx),
            Self::Any(list) => has(list, ctx, true),
            Self::None(list) => has(list, ctx, true).map(|any| !any),
        }
    }

    pub fn uses(&self, attribute: Attribute) -> bool {
        match self {
            Self::Condition(condition) => condition.attribute() == attribute,
            Self::All(list) | Self::Any(list) | Self::None(list) => {
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
    use crate::{Check, Operator};

    fn conditions(attribute: Attribute, operator: Operator, values: &[&str]) -> Vec<Expr> {
        let values: Vec<String> = values.iter().map(|&v| v.to_owned()).collect();
        Condition::build(attribute, operator, &values)
            .unwrap()
            .into_iter()
            .map(Expr::Condition)
            .collect()
    }

    fn condition(attribute: Attribute, operator: Operator, values: &[&str]) -> Expr {
        let mut list = conditions(attribute, operator, values);
        assert_eq!(list.len(), 1);
        list.remove(0)
    }

    fn holds(attribute: Attribute, operator: Operator, values: &[&str], ctx: &Context) -> bool {
        all(&conditions(attribute, operator, values), ctx).unwrap()
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
    fn list_operators() {
        use Attribute::Labels;
        let ctx = pr();

        assert!(holds(Labels, Operator::Is, &["ready", "area/ci"], &ctx));
        assert!(!holds(Labels, Operator::All, &["ready", "blocked"], &ctx));
        assert!(holds(Labels, Operator::Any, &["blocked", "ready"], &ctx));
        assert!(!holds(Labels, Operator::Any, &["blocked"], &ctx));
        assert!(holds(Labels, Operator::None, &["blocked", "wip"], &ctx));
        assert!(!holds(Labels, Operator::None, &["blocked", "ready"], &ctx));
        assert!(holds(Labels, Operator::Count, &["2"], &ctx));
        assert!(holds(Labels, Operator::CountMin, &["1"], &ctx));
        assert!(!holds(Labels, Operator::CountMax, &["1"], &ctx));
    }

    #[test]
    fn globs_match_any_every_or_no_element() {
        use Attribute::Files;
        let mut ctx = pr();

        assert!(holds(Files, Operator::AnyMatch, &["src/**/*.rs"], &ctx));
        assert!(!holds(Files, Operator::AnyMatch, &["src/*.rs"], &ctx));
        assert!(holds(Files, Operator::AllMatch, &["src/**", "*.md"], &ctx));
        assert!(!holds(Files, Operator::AllMatch, &["src/**"], &ctx));
        assert!(holds(Files, Operator::NoneMatch, &["migrations/**"], &ctx));
        assert!(!holds(Files, Operator::NoneMatch, &["*.md"], &ctx));

        ctx.files = Some(Vec::new());

        assert!(!holds(Files, Operator::AllMatch, &["**"], &ctx));
    }

    #[test]
    fn string_operators() {
        let ctx = pr();

        assert!(holds(
            Attribute::Author,
            Operator::Is,
            &["bob", "alice"],
            &ctx
        ));
        assert!(!holds(Attribute::Author, Operator::Not, &["alice"], &ctx));
        assert!(holds(Attribute::Base, Operator::Matches, &["ma*"], &ctx));
        assert!(holds(Attribute::Title, Operator::Regex, &["queue"], &ctx));
        assert!(!holds(Attribute::Title, Operator::Regex, &["^queue"], &ctx));
    }

    #[test]
    fn checks_are_grouped_by_conclusion() {
        let ctx = pr();

        assert!(holds(
            Attribute::CheckSuccess,
            Operator::Is,
            &["lint"],
            &ctx
        ));
        assert!(holds(
            Attribute::CheckFailure,
            Operator::Is,
            &["build"],
            &ctx
        ));
        assert!(holds(Attribute::CheckPending, Operator::Is, &["e2e"], &ctx));
        assert!(!holds(
            Attribute::CheckSuccess,
            Operator::Is,
            &["e2e"],
            &ctx
        ));
        assert!(holds(
            Attribute::CheckSkipped,
            Operator::Count,
            &["0"],
            &ctx
        ));
        assert!(holds(
            Attribute::CheckNeutral,
            Operator::Count,
            &["0"],
            &ctx
        ));
    }

    #[test]
    fn compares_numbers_durations_and_choices() {
        let mut ctx = pr();

        assert!(holds(Attribute::Number, Operator::Is, &["42"], &ctx));
        assert!(holds(Attribute::Number, Operator::Min, &["42"], &ctx));
        assert!(!holds(Attribute::Number, Operator::Max, &["41"], &ctx));
        assert!(holds(Attribute::PrAge, Operator::Min, &["2h"], &ctx));
        assert!(holds(
            Attribute::ReviewDecision,
            Operator::Is,
            &["approved"],
            &ctx
        ));
        assert!(!holds(
            Attribute::ReviewDecision,
            Operator::Is,
            &["none", "review_required"],
            &ctx
        ));

        ctx.sender = Some("bob".into());
        ctx.sender_permission = Some(Permission::Write);

        assert!(holds(
            Attribute::SenderPermission,
            Operator::Min,
            &["write"],
            &ctx
        ));
        assert!(!holds(
            Attribute::SenderPermission,
            Operator::Min,
            &["admin"],
            &ctx
        ));
        assert!(holds(Attribute::Sender, Operator::Is, &["bob"], &ctx));
    }

    #[test]
    fn absent_values_do_not_compare() {
        let ctx = pr();

        assert!(!holds(
            Attribute::SenderPermission,
            Operator::Max,
            &["admin"],
            &ctx
        ));
        assert!(!holds(
            Attribute::SenderPermission,
            Operator::Is,
            &["none"],
            &ctx
        ));
        assert!(!holds(Attribute::QueueAge, Operator::Max, &["1000d"], &ctx));
        assert!(!holds(
            Attribute::QueuePosition,
            Operator::Min,
            &["0"],
            &ctx
        ));
        assert!(holds(Attribute::Sender, Operator::Not, &["bob"], &ctx));
    }

    #[test]
    fn booleans() {
        let mut ctx = pr();

        assert!(holds(Attribute::Draft, Operator::Is, &["false"], &ctx));
        ctx.draft = true;
        assert!(holds(Attribute::Draft, Operator::Is, &["true"], &ctx));
        assert!(!holds(Attribute::Draft, Operator::Is, &["false"], &ctx));
    }

    #[test]
    fn unmet_names_each_condition_that_does_not_hold() {
        let mut list = conditions(Attribute::Labels, Operator::Is, &["ready", "blocked"]);
        list.push(Expr::Any(vec![
            condition(Attribute::Author, Operator::Is, &["bob"]),
            condition(Attribute::Number, Operator::Max, &["10"]),
        ]));
        list.push(Expr::None(vec![condition(
            Attribute::Author,
            Operator::Is,
            &["alice"],
        )]));
        list.push(Expr::All(vec![condition(
            Attribute::Number,
            Operator::Is,
            &["42"],
        )]));

        assert_eq!(
            Conditions(list).unmet(&pr()),
            Ok(vec![
                "labels: blocked".to_owned(),
                "any: [author: bob; number.max: 10]".to_owned(),
                "none: [author: alice]".to_owned(),
            ])
        );
        let threads = Conditions(conditions(
            Attribute::ReviewThreadsUnresolved,
            Operator::Count,
            &["0"],
        ));
        assert_eq!(
            threads.unmet(&pr()),
            Err(EvalError::NotLoaded(Attribute::ReviewThreadsUnresolved))
        );
    }

    #[test]
    fn evaluates_blocks() {
        let ctx = pr();
        let list = Conditions(vec![
            condition(Attribute::Draft, Operator::Is, &["false"]),
            Expr::Any(vec![
                condition(Attribute::Labels, Operator::Any, &["hotfix"]),
                condition(Attribute::Labels, Operator::Any, &["ready"]),
            ]),
            Expr::None(vec![
                condition(Attribute::Author, Operator::Is, &["dependabot[bot]"]),
                condition(Attribute::Labels, Operator::Any, &["blocked"]),
            ]),
        ]);

        assert_eq!(list.eval(&ctx), Ok(true));
        assert!(list.uses(Attribute::Author));
        assert!(!list.uses(Attribute::Files));
        let all = Conditions(vec![Expr::All(vec![
            condition(Attribute::Labels, Operator::Any, &["ready"]),
            condition(Attribute::Draft, Operator::Is, &["true"]),
        ])]);

        assert_eq!(all.eval(&ctx), Ok(false));
        assert_eq!(Conditions::default().eval(&ctx), Ok(true));
    }
}
