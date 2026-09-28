//! Checks a `.cihub.yaml` and prints the effective settings of each queue,
//! with the source of each value.
//!
//! ```sh
//! cargo run -p cihub-config --example check -- crates/config/examples/valid.cihub.yaml
//! cargo run -p cihub-config --example check < .cihub.yaml
//! ```

use std::fmt::Debug;
use std::process::ExitCode;

use cihub_config::{ConfigError, Layer, QueueSettings, RepoConfig, Sourced};

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
    match check(&text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            report(&name, &text, &e);
            ExitCode::FAILURE
        }
    }
}

fn check(text: &str) -> Result<(), ConfigError> {
    let config = RepoConfig::parse(text)?;
    for queue in &config.queues {
        println!("queue `{}` on branch `{}`", queue.name, queue.branch);
        println!(
            "  queue_conditions: {}, merge_conditions: {}",
            queue.queue_conditions.0.len(),
            queue.merge_conditions.0.len()
        );
        print_settings(&QueueSettings::resolve(
            &Layer::default(),
            &queue.settings,
            &config.defaults,
        )?);
        println!();
    }
    for rule in &config.priority_rules {
        println!(
            "priority rule `{}`: priority {} if {} condition(s) hold",
            rule.name,
            rule.priority.0,
            rule.conditions.0.len()
        );
    }
    Ok(())
}

/// Prints the error as a compiler does: position, the line, and a caret.
fn report(name: &str, text: &str, error: &ConfigError) {
    match error.position {
        Some(at) => {
            eprintln!("{name}:{}:{}: error: {}", at.line, at.column, error.message);
            let line = usize::try_from(at.line)
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|n| text.lines().nth(n));
            if let Some(line) = line {
                let pad = usize::try_from(at.column).unwrap_or(1).saturating_sub(1);
                eprintln!("  | {line}\n  | {}^", " ".repeat(pad));
            }
        }
        None => eprintln!("{name}: error: {}", error.message),
    }
    eprintln!("  = test runs fail with `{}`", error.reason());
}

fn print_settings(s: &QueueSettings) {
    macro_rules! rows {
        ($($field:ident),+ $(,)?) => { $(row(stringify!($field), &s.$field);)+ };
    }
    rows!(
        strategy,
        mode,
        window_floor,
        window_ceiling,
        bisection_concurrency,
        testing_timeout_minutes,
        not_ready_timeout_hours,
        merge_method,
        merge_commit_title,
        trigger,
        update_method,
        batching_enabled,
        batching_size,
        batching_max_wait_minutes,
        failure_budget_retries,
        optimistic_merging,
        pending_failure_depth,
        direct_merge_mode,
        optimization_mode,
        comments_enabled,
        commands_enabled,
        status_check_enabled,
        label_commands_enabled,
        enqueue_label,
        state_labels_enabled,
        keep_test_branches_minutes,
        branch_prefix,
        branch_protection_injection,
        required_statuses,
    );
}

fn row<T: Debug>(name: &str, setting: &Sourced<T>) {
    let at = setting
        .at
        .map(|at| format!(" (line {})", at.line))
        .unwrap_or_default();
    let value = format!("{:?}", setting.value);
    println!("  {name:<28} {value:<32} {:?}{at}", setting.source);
}
