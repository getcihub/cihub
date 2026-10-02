//! Parses one condition per line and evaluates each on a sample PR.
//! Blank lines are skipped.
//!
//! ```sh
//! cargo run -p cihub-conditions --example eval -- crates/conditions/examples/valid.conditions
//! cargo run -p cihub-conditions --example eval < my.conditions
//! ```

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line example writes its result to the terminal"
)]

use std::process::ExitCode;
use std::time::Duration;

use cihub_conditions::{Attribute, Check, Condition, Conditions, Context, Expr, ParseError};
use cihub_domain::{CheckConclusion, Permission, ReviewDecision};

/// The file named by the first argument, or stdin.
fn read_input() -> Result<(String, String), String> {
    match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(text) => Ok((path, text)),
            Err(e) => Err(format!("{path}: {e}")),
        },
        None => std::io::read_to_string(std::io::stdin())
            .map(|text| ("<stdin>".to_owned(), text))
            .map_err(|e| format!("stdin: {e}")),
    }
}

fn main() -> ExitCode {
    let (name, text) = match read_input() {
        Ok(input) => input,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let mut conditions = Vec::new();
    let mut failed = false;
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match line.parse::<Condition>() {
            Ok(condition) => conditions.push(condition),
            Err(e) => {
                report(&name, n + 1, line, &e);
                failed = true;
            }
        }
    }

    if failed {
        return ExitCode::FAILURE;
    }

    let list = Conditions(conditions.into_iter().map(Expr::Condition).collect());
    let ctx = sample_pr(&list);
    for expr in &list.0 {
        let Expr::Condition(condition) = expr else {
            unreachable!("the list holds only conditions");
        };
        match condition.eval(&ctx) {
            Ok(holds) => println!("{holds:<5}  {condition}"),
            Err(e) => {
                eprintln!("error   {condition}: {e}");
                failed = true;
            }
        }
    }

    match list.eval(&ctx) {
        Ok(holds) => println!("\nall conditions hold: {holds}"),
        Err(e) => eprintln!("\nerror: {e}"),
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints the error as a compiler does: position, the line, and a caret.
fn report(name: &str, line_number: usize, line: &str, error: &ParseError) {
    eprintln!(
        "{name}:{line_number}:{}: error: {}",
        error.column, error.kind
    );
    eprintln!("  | {line}\n  | {}^", " ".repeat(error.column - 1));
}

/// A PR that is approved, green on `lint` and `build`, and waits on `e2e`.
/// Attributes that are loaded on demand are loaded only when `list` uses
/// them, as the caller does.
fn sample_pr(list: &Conditions) -> Context {
    let check = |name: &str, conclusion| Check {
        name: name.to_owned(),
        conclusion,
    };

    Context {
        number: 42,
        title: "feat: add the merge queue".into(),
        body: "Closes #12".into(),
        review_decision: ReviewDecision::Approved,
        approved_reviews_by: vec!["alice".into(), "bob".into()],
        review_threads_unresolved: list.uses(Attribute::ReviewThreadsUnresolved).then(Vec::new),
        checks: vec![
            check("lint", Some(CheckConclusion::Success)),
            check("build", Some(CheckConclusion::Success)),
            check("e2e", None),
        ],
        labels: vec!["ready-to-merge".into(), "area/ci".into()],
        author: "carol".into(),
        sender: Some("carol".into()),
        sender_permission: Some(Permission::Write),
        files: list
            .uses(Attribute::Files)
            .then(|| vec!["src/queue/plan.rs".into(), "README.md".into()]),
        additions: 120,
        deletions: 30,
        base: "main".into(),
        head: "feat/merge-queue".into(),
        queue_name: Some("main".into()),
        queue_position: Some(0),
        queue_age: Some(Duration::from_mins(45)),
        pr_age: Duration::from_hours(26),
        ..Context::default()
    }
}
