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
            Self::Any(list) => any(list, ctx),
            Self::None(list) => any(list, ctx).map(|holds| !holds),
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

/// Stops at the first element that does not hold.
fn all(list: &[Expr], ctx: &Context) -> Result<bool, EvalError> {
    for expr in list {
        if !expr.eval(ctx)? {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Stops at the first element that holds.
fn any(list: &[Expr], ctx: &Context) -> Result<bool, EvalError> {
    for expr in list {
        if expr.eval(ctx)? {
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
        let values: Vec<String> = values.iter().map(|&value| value.to_owned()).collect();
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

    /// Each case is an attribute, an operator, its values, and whether the
    /// condition holds on `ctx`.
    fn assert_cases(ctx: &Context, cases: &[(Attribute, Operator, &[&str], bool)]) {
        for &(attribute, operator, values, expected) in cases {
            assert_eq!(
                holds(attribute, operator, values, ctx),
                expected,
                "{attribute}.{operator}: {values:?}"
            );
        }
    }

    #[test]
    fn list_operators() {
        assert_cases(
            &pr(),
            &[
                (Attribute::Labels, Operator::Is, &["ready", "area/ci"], true),
                (
                    Attribute::Labels,
                    Operator::All,
                    &["ready", "blocked"],
                    false,
                ),
                (
                    Attribute::Labels,
                    Operator::Any,
                    &["blocked", "ready"],
                    true,
                ),
                (Attribute::Labels, Operator::Any, &["blocked"], false),
                (Attribute::Labels, Operator::None, &["blocked", "wip"], true),
                (
                    Attribute::Labels,
                    Operator::None,
                    &["blocked", "ready"],
                    false,
                ),
                (Attribute::Labels, Operator::Count, &["2"], true),
                (Attribute::Labels, Operator::CountMin, &["1"], true),
                (Attribute::Labels, Operator::CountMax, &["1"], false),
            ],
        );
    }

    #[test]
    fn globs_match_any_every_or_no_element() {
        let mut ctx = pr();

        assert_cases(
            &ctx,
            &[
                (Attribute::Files, Operator::AnyMatch, &["src/**/*.rs"], true),
                (Attribute::Files, Operator::AnyMatch, &["src/*.rs"], false),
                (
                    Attribute::Files,
                    Operator::AllMatch,
                    &["src/**", "*.md"],
                    true,
                ),
                (Attribute::Files, Operator::AllMatch, &["src/**"], false),
                (
                    Attribute::Files,
                    Operator::NoneMatch,
                    &["migrations/**"],
                    true,
                ),
                (Attribute::Files, Operator::NoneMatch, &["*.md"], false),
            ],
        );

        ctx.files = Some(Vec::new());

        assert!(!holds(Attribute::Files, Operator::AllMatch, &["**"], &ctx));
    }

    #[test]
    fn string_operators() {
        assert_cases(
            &pr(),
            &[
                (Attribute::Author, Operator::Is, &["bob", "alice"], true),
                (Attribute::Author, Operator::Not, &["alice"], false),
                (Attribute::Base, Operator::Matches, &["ma*"], true),
                (Attribute::Title, Operator::Regex, &["queue"], true),
                (Attribute::Title, Operator::Regex, &["^queue"], false),
            ],
        );
    }

    #[test]
    fn checks_are_grouped_by_conclusion() {
        assert_cases(
            &pr(),
            &[
                (Attribute::CheckSuccess, Operator::Is, &["lint"], true),
                (Attribute::CheckFailure, Operator::Is, &["build"], true),
                (Attribute::CheckPending, Operator::Is, &["e2e"], true),
                (Attribute::CheckSuccess, Operator::Is, &["e2e"], false),
                (Attribute::CheckSkipped, Operator::Count, &["0"], true),
                (Attribute::CheckNeutral, Operator::Count, &["0"], true),
            ],
        );
    }

    #[test]
    fn compares_numbers_durations_and_choices() {
        let mut ctx = pr();

        assert_cases(
            &ctx,
            &[
                (Attribute::Number, Operator::Is, &["42"], true),
                (Attribute::Number, Operator::Min, &["42"], true),
                (Attribute::Number, Operator::Max, &["41"], false),
                (Attribute::PrAge, Operator::Min, &["2h"], true),
                (Attribute::ReviewDecision, Operator::Is, &["approved"], true),
                (
                    Attribute::ReviewDecision,
                    Operator::Is,
                    &["none", "review_required"],
                    false,
                ),
            ],
        );

        ctx.sender = Some("bob".into());
        ctx.sender_permission = Some(Permission::Write);
        ctx.queue_position = Some(2);

        assert_cases(
            &ctx,
            &[
                (Attribute::SenderPermission, Operator::Min, &["write"], true),
                (
                    Attribute::SenderPermission,
                    Operator::Min,
                    &["admin"],
                    false,
                ),
                (Attribute::Sender, Operator::Is, &["bob"], true),
                (Attribute::QueuePosition, Operator::Max, &["2"], true),
                (Attribute::QueuePosition, Operator::Max, &["1"], false),
            ],
        );
    }

    #[test]
    fn absent_values_do_not_compare() {
        assert_cases(
            &pr(),
            &[
                (
                    Attribute::SenderPermission,
                    Operator::Max,
                    &["admin"],
                    false,
                ),
                (Attribute::SenderPermission, Operator::Is, &["none"], false),
                (Attribute::QueueAge, Operator::Max, &["1000d"], false),
                (Attribute::QueuePosition, Operator::Min, &["0"], false),
                (Attribute::QueuePosition, Operator::Max, &["3"], false),
                (Attribute::QueuePosition, Operator::Is, &["0"], false),
                (Attribute::Sender, Operator::Not, &["bob"], true),
            ],
        );
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
