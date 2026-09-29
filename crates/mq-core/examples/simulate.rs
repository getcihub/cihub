//! Plays a scenario on one queue: each line is a fact from the SCM, a job or a
//! user. After each fact, the example runs `plan`, applies its decisions and
//! runs `check`. At the end, it replays the log. `#` starts a comment.
//!
//! By default, the example prints one row per line of the scenario and one
//! column per PR: a cell shows the new state of the PR, `│` while the PR
//! waits in the queue, and the last column says what left the normal path.
//! With `-v`, it prints every decision with its reason instead.
//!
//! ```sh
//! cargo run -p cihub-mq-core --example simulate -- crates/mq-core/examples/valid.scenario
//! cargo run -p cihub-mq-core --example simulate -- -v crates/mq-core/examples/valid.scenario
//! cargo run -p cihub-mq-core --example simulate < my.scenario
//! ```
//!
//! Commands (`<pr>` is a PR number):
//!
//! | Command                                 | Fact                                                       |
//! | --------------------------------------- | ---------------------------------------------------------- |
//! | `settings window=N retries=N mode=M`    | queue settings; only before the first fact                 |
//! | `push <sha>`                            | a push from outside the queue on the target branch        |
//! | `submit <pr> [urgent\|high\|medium\|low\|N]` | `/cihub merge`, with a priority                        |
//! | `observe <pr> [word...]`                | the state of the PR: `mergeable` (default), `conflict`, `behind`, `blocked`, `unknown`, `closed`, `merged`, `draft`, `head=<sha>`, `base=<branch>` |
//! | `targets <pr> <target>...`              | the impacted targets of the PR head                        |
//! | `assemble <pr>` / `conflict <pr>`       | the assembly job of the run of the PR succeeds or fails    |
//! | `pass <pr>` / `fail <pr>`               | the required check `ci` on the test SHA of the PR          |
//! | `merge`                                 | the merge job merges every PR of the desired state         |
//! | `merge-failed <pr> <job_error>`         | the merge job fails, for example `head_changed`            |
//! | `cancel <pr>` / `restart <pr>`          | `/cihub cancel`, `/cihub restart`                          |
//! | `pause`, `resume`, `drain`              | an operator request on the queue                           |
//! | `lose-access`, `restore-access`         | CIHub loses or gets back its access to the repository      |
//! | `mode single\|parallel`                 | a change of the queue mode                                 |
//! | `wait <minutes>`                        | time passes                                                |
//! | `show`                                  | prints the queue and the desired state                     |

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line example writes its result to the terminal"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::process::ExitCode;
use std::str::FromStr;

use cihub_config::{Layer, QueueSettings, Setting};
use cihub_domain::{
    Actor, BlockReason, CheckConclusion, CheckKind, CheckReport, Event, EventId, EventKind,
    GroupId, ImpactedTargets, ItemId, ItemState, JobError, Mergeability, PrState, Priority,
    QueueControl, QueueMode, Reason, RequiredStatusesSource, ReviewDecision, RunId, SubmittedVia,
};
use cihub_mq_core::{Outcome, Request, Snapshot, apply, check, plan};
use time::{Duration, OffsetDateTime};
use ulid::Ulid;

/// The required status of every run in the scenario.
const REQUIRED: &str = "ci";

/// Reasons of the normal path, which the lane view does not repeat.
const NORMAL: [Reason; 5] = [
    Reason::Submitted,
    Reason::MergeItemNowReady,
    Reason::TestRunStarted,
    Reason::TestRunPassed,
    Reason::MergeGraphMerge,
];

/// The file named by `path`, or stdin.
fn read_input(path: Option<String>) -> Result<(String, String), String> {
    match path {
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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.first().is_some_and(|a| a == "-v" || a == "--verbose");
    if verbose {
        args.remove(0);
    }
    let (name, text) = match read_input(args.into_iter().next()) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut world = World {
        verbose,
        ..World::default()
    };
    for (index, line) in text.lines().enumerate() {
        let command = line.split('#').next().unwrap_or_default();
        if command.trim().is_empty() {
            continue;
        }
        if verbose {
            println!("> {}", command.trim());
        }
        world.begin();
        let result = world.run(command);
        if !command.trim_start().starts_with("settings") {
            world.end(index + 1, command.trim());
        }
        if let Err(e) = result {
            world.print_lanes();
            eprintln!("{name}:{}:{}: error: {}", index + 1, e.column, e.message);
            eprintln!("  | {line}\n  | {}^", " ".repeat(e.column - 1));
            return ExitCode::FAILURE;
        }
    }
    if verbose {
        world.show();
    } else {
        world.print_lanes();
        println!(
            "\nqueue {}, desired: {}",
            world.snapshot.state,
            world.desired()
        );
    }
    world.finish()
}

/// What the lane view knows of a PR.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Lane {
    state: ItemState,
    attempt: u32,
}

/// One line of the scenario in the lane view.
struct Row {
    line: usize,
    command: String,
    target: String,
    cells: BTreeMap<u64, String>,
    notes: Vec<String>,
}

const fn symbol(state: ItemState) -> &'static str {
    match state {
        ItemState::NotReady => "○",
        ItemState::Pending => "◌",
        ItemState::Testing => "●",
        ItemState::PendingFailure => "!",
        ItemState::TestsPassed => "✓",
        ItemState::Merged => "◆",
        ItemState::Failed => "✖",
        ItemState::Cancelled => "⊘",
    }
}

const fn block_name(block: &BlockReason) -> &'static str {
    match block {
        BlockReason::ReviewRequired { .. } => "review_required",
        BlockReason::ChangesRequested => "changes_requested",
        BlockReason::FailingRequiredCheck { .. } => "failing_required_check",
        BlockReason::AwaitingRequiredCheck { .. } => "awaiting_required_check",
        BlockReason::BehindBase => "behind_base",
        BlockReason::MergeConflict => "merge_conflict",
        BlockReason::StackNeedsRebase { .. } => "stack_needs_rebase",
        BlockReason::Undetermined => "undetermined",
    }
}

fn blocks(block_reasons: &[BlockReason]) -> String {
    let names: Vec<_> = block_reasons.iter().map(block_name).collect();
    names.join(", ")
}

/// Pads to `width` characters; `format!` pads by bytes for `│` and `●`.
fn pad(text: &str, width: usize) -> String {
    let n = text.chars().count();
    format!("{text}{}", " ".repeat(width.saturating_sub(n)))
}

/// An error in a line, at a 1-based column.
struct LineError {
    column: usize,
    message: String,
}

/// A word of the line with its column.
#[derive(Clone, Copy)]
struct Word<'a> {
    text: &'a str,
    column: usize,
}

impl Word<'_> {
    fn error(self, message: impl Into<String>) -> LineError {
        LineError {
            column: self.column,
            message: message.into(),
        }
    }

    fn parse<T: FromStr>(self, what: &str) -> Result<T, LineError> {
        self.text
            .parse()
            .map_err(|_| self.error(format!("`{}` is not {what}", self.text)))
    }
}

fn words(line: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices().chain([(line.len(), ' ')]) {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                words.push(Word {
                    text: &line[s..i],
                    column: line[..s].chars().count() + 1,
                });
                start = None;
            }
            _ => {}
        }
    }
    words
}

/// The queue, its log, and the names that the example prints.
struct World {
    snapshot: Snapshot,
    initial: Snapshot,
    settings: QueueSettings,
    layer: Layer,
    log: Vec<Event>,
    now: OffsetDateTime,
    facts: u128,
    heads: BTreeMap<u64, String>,
    item_prs: BTreeMap<ItemId, u64>,
    group_prs: BTreeMap<GroupId, u64>,
    run_attempts: BTreeMap<RunId, (u64, u32)>,
    /// The attempt of the latest run of each PR.
    pr_attempts: BTreeMap<u64, u32>,
    violations: usize,
    verbose: bool,
    lanes: BTreeMap<u64, Lane>,
    before: BTreeMap<u64, Lane>,
    target_before: String,
    touched: BTreeSet<u64>,
    notes: Vec<String>,
    rows: Vec<Row>,
}

impl Default for World {
    fn default() -> Self {
        let layer = Layer::default();
        let settings = resolve(&layer).unwrap_or_else(|_| unreachable!("the defaults are valid"));
        let snapshot = new_snapshot(settings.mode.value);
        Self {
            initial: snapshot.clone(),
            snapshot,
            settings,
            layer,
            log: Vec::new(),
            now: OffsetDateTime::UNIX_EPOCH + Duration::days(20_000),
            facts: 0,
            heads: BTreeMap::new(),
            item_prs: BTreeMap::new(),
            group_prs: BTreeMap::new(),
            run_attempts: BTreeMap::new(),
            pr_attempts: BTreeMap::new(),
            violations: 0,
            verbose: false,
            lanes: BTreeMap::new(),
            before: BTreeMap::new(),
            target_before: String::new(),
            touched: BTreeSet::new(),
            notes: Vec::new(),
            rows: Vec::new(),
        }
    }
}

fn resolve(layer: &Layer) -> Result<QueueSettings, cihub_config::ConfigError> {
    QueueSettings::resolve(layer, &Layer::default(), &Layer::default())
}

fn new_snapshot(mode: QueueMode) -> Snapshot {
    Snapshot::new(
        cihub_domain::OrgId::from_ulid(Ulid(1)),
        cihub_domain::QueueId::from_ulid(Ulid(2)),
        "main".into(),
        mode,
    )
}

impl World {
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per command, each short and independent"
    )]
    fn run(&mut self, line: &str) -> Result<(), LineError> {
        let words = words(line);
        let (command, args) = words.split_first().ok_or_else(|| LineError {
            column: 1,
            message: "empty command".into(),
        })?;
        let arg = |i: usize, what: &str| {
            args.get(i).copied().ok_or_else(|| LineError {
                column: line.trim_end().chars().count() + 1,
                message: format!("`{}` needs {what}", command.text),
            })
        };
        let user = Actor::User {
            login: "octocat".into(),
        };
        let kind = match command.text {
            "settings" => return self.settings(*command, args),
            "show" => {
                if self.verbose {
                    self.show();
                } else {
                    let desired = self.desired();
                    self.note(format!("desired: {desired}"));
                }
                return Ok(());
            }
            "wait" => {
                let minutes: i64 = arg(0, "a number of minutes")?.parse("a number")?;
                self.now += Duration::minutes(minutes);
                self.reconcile();
                return Ok(());
            }
            "merge" => return self.merge(),
            "push" => {
                let after = arg(0, "a SHA")?.text.to_owned();
                if self.fact(user, self.push(after))? == Outcome::Applied
                    && self.snapshot.target.recent_heads.back().is_some()
                {
                    self.note("push from outside the queue".into());
                }
                return Ok(());
            }
            "submit" => {
                let pr = pr_number(arg(0, "a PR number")?)?;
                let priority = match args.get(1) {
                    None => Priority::MEDIUM,
                    Some(w) => match Priority::from_name(w.text) {
                        Some(p) => p,
                        None => Priority(w.parse("a priority")?),
                    },
                };
                EventKind::ItemSubmitted {
                    pr_number: pr,
                    head_sha: self.head(pr),
                    priority,
                    no_batch: false,
                    direct: false,
                    submitted_via: SubmittedVia::Comment,
                    author: "octocat".into(),
                    additions: 10,
                    deletions: 2,
                    changed_files: 1,
                }
            }
            "observe" => self.observe(arg(0, "a PR number")?, &args[1..])?,
            "targets" => {
                let pr = pr_number(arg(0, "a PR number")?)?;
                let targets: BTreeSet<String> =
                    args[1..].iter().map(|w| w.text.to_owned()).collect();
                EventKind::TargetsReceived {
                    pr_number: pr,
                    head_sha: self.head(pr),
                    targets: if targets.is_empty() || targets.contains("ALL") {
                        ImpactedTargets::All
                    } else {
                        ImpactedTargets::Targets(targets)
                    },
                }
            }
            "assemble" => {
                let run_id = self.run_of(arg(0, "a PR number")?)?;
                EventKind::TestBranchAssembled {
                    run_id,
                    test_sha: test_sha(run_id),
                    draft_pr_number: None,
                    required_statuses: vec![REQUIRED.into()],
                    required_statuses_source: RequiredStatusesSource::RepoConfig,
                }
            }
            "conflict" => EventKind::TestBranchAssemblyFailed {
                run_id: self.run_of(arg(0, "a PR number")?)?,
                error: JobError::MergeConflict,
                conflict_culprit: None,
            },
            "pass" | "fail" => {
                let run_id = self.run_of(arg(0, "a PR number")?)?;
                let conclusion = if command.text == "pass" {
                    CheckConclusion::Success
                } else {
                    CheckConclusion::Failure
                };
                EventKind::CheckObserved {
                    test_sha: test_sha(run_id),
                    check: CheckReport {
                        name: REQUIRED.into(),
                        kind: CheckKind::Check,
                        conclusion: Some(conclusion),
                        details_url: None,
                    },
                }
            }
            "merge-failed" => {
                let item_id = self.item_of(arg(0, "a PR number")?)?;
                EventKind::MergeFailed {
                    item_id,
                    direct: false,
                    error: arg(1, "a job error")?.parse("a job error")?,
                }
            }
            "cancel" => EventKind::ItemCancelRequested {
                item_id: self.item_of(arg(0, "a PR number")?)?,
                by_admin: false,
            },
            "restart" => EventKind::ItemRestartRequested {
                item_id: self.item_of(arg(0, "a PR number")?)?,
                by_api: false,
            },
            "pause" | "resume" | "drain" => EventKind::QueueControlRequested {
                control: command.parse::<QueueControl>("a queue control")?,
            },
            "lose-access" => EventKind::AccessLost,
            "restore-access" => EventKind::AccessRestored,
            "mode" => EventKind::QueueSettingsChanged {
                before: self.snapshot.requested_mode,
                after: arg(0, "a mode")?.parse("a mode")?,
            },
            other => return Err(command.error(format!("unknown command `{other}`"))),
        };
        self.fact(Actor::System, kind).map(|_| ())
    }

    fn settings(&mut self, command: Word, args: &[Word]) -> Result<(), LineError> {
        if !self.log.is_empty() {
            return Err(command.error("`settings` must come before the first fact"));
        }
        for word in args {
            let (key, value) = word
                .text
                .split_once('=')
                .ok_or_else(|| word.error("expected `key=value`"))?;
            let value_word = Word {
                text: value,
                column: word.column + key.len() + 1,
            };
            match key {
                "window" => {
                    let n: u32 = value_word.parse("a number")?;
                    self.layer.window_floor = Some(Setting::new(n));
                    self.layer.window_ceiling = Some(Setting::new(n));
                }
                "retries" => {
                    self.layer.failure_budget_retries =
                        Some(Setting::new(value_word.parse("a number")?));
                }
                "timeout" => {
                    self.layer.testing_timeout_minutes =
                        Some(Setting::new(value_word.parse("a number")?));
                }
                "mode" => self.layer.mode = Some(Setting::new(value_word.parse("a mode")?)),
                _ => return Err(word.error(format!("unknown setting `{key}`"))),
            }
        }
        self.settings = resolve(&self.layer).map_err(|e| command.error(e.message))?;
        self.snapshot = new_snapshot(self.settings.mode.value);
        self.initial = self.snapshot.clone();
        Ok(())
    }

    fn head(&self, pr: u64) -> String {
        self.heads
            .get(&pr)
            .cloned()
            .unwrap_or_else(|| format!("head-{pr}"))
    }

    fn item_of(&self, word: Word) -> Result<ItemId, LineError> {
        let pr = pr_number(word)?;
        self.snapshot
            .current_item(pr)
            .map(|i| i.id)
            .ok_or_else(|| word.error(format!("PR {pr} has no live item")))
    }

    fn run_of(&self, word: Word) -> Result<RunId, LineError> {
        let pr = pr_number(word)?;
        self.snapshot
            .current_item(pr)
            .and_then(|i| i.group_id)
            .and_then(|g| self.snapshot.live_run(g))
            .map(|r| r.id)
            .ok_or_else(|| word.error(format!("PR {pr} has no run in progress")))
    }

    fn observe(&mut self, pr_word: Word, words: &[Word]) -> Result<EventKind, LineError> {
        let pr = pr_number(pr_word)?;
        let (mut state, mut draft, mut base) = (PrState::Open, false, "main".to_owned());
        let mut mergeability = Mergeability::Mergeable;
        for word in words {
            match word.text.split_once('=') {
                Some(("head", sha)) => {
                    self.heads.insert(pr, sha.to_owned());
                }
                Some(("base", branch)) => branch.clone_into(&mut base),
                Some(_) => return Err(word.error(format!("unknown field in `{}`", word.text))),
                None => match word.text {
                    "draft" => draft = true,
                    "open" | "closed" | "merged" => state = word.parse("a PR state")?,
                    _ => mergeability = word.parse("a PR state or a mergeability")?,
                },
            }
        }
        Ok(EventKind::PrObserved {
            pr_number: pr,
            state,
            head_sha: self.head(pr),
            base_branch: base,
            draft,
            labels: BTreeSet::new(),
            mergeability,
            review_decision: ReviewDecision::Approved,
            checks: Vec::new(),
        })
    }

    fn push(&self, after: String) -> EventKind {
        EventKind::TargetPushed {
            before: self.snapshot.target.head.as_ref().map(|h| h.value.clone()),
            after,
            sender: "octocat".into(),
        }
    }

    /// Merges until the desired state has no merge left: a merge can make
    /// the next PR ready.
    fn merge(&mut self) -> Result<(), LineError> {
        let mut count = 0;
        loop {
            let merges = plan(&self.snapshot, &self.settings, self.now)
                .map(|p| p.desired.merges)
                .unwrap_or_default();
            let Some(merge) = merges.first() else { break };
            let sha = format!("merge-{}", merge.pr_number);
            let kind = EventKind::MergeSucceeded {
                item_id: merge.item_id,
                merged_sha: sha.clone(),
            };
            if self.fact(Actor::Bot, kind)? == Outcome::Ignored {
                break;
            }
            self.fact(Actor::Bot, self.push(sha))?;
            count += 1;
        }
        if count == 0 {
            if self.verbose {
                println!("  nothing to merge");
            }
            self.note("nothing to merge".into());
        }
        Ok(())
    }

    /// Writes a fact, then reconciles.
    fn fact(&mut self, actor: Actor, kind: EventKind) -> Result<Outcome, LineError> {
        self.facts += 1;
        self.now += Duration::seconds(1);
        let ms = u64::try_from(self.now.unix_timestamp_nanos() / 1_000_000).unwrap_or(0);
        let event = Event {
            id: EventId::from_ulid(Ulid::from_parts(ms, self.facts)),
            org_id: self.snapshot.org_id,
            queue_id: self.snapshot.queue_id,
            actor,
            occurred_at: self.now,
            cause_id: None,
            kind,
        };
        let outcome = apply(&mut self.snapshot, &event).map_err(|e| LineError {
            column: 1,
            message: e.to_string(),
        })?;
        match outcome {
            Outcome::Applied => self.log.push(event),
            Outcome::Ignored => {
                if self.verbose {
                    println!(
                        "  {} ignored: it changes nothing, or no decision can answer it",
                        event.kind.event_type()
                    );
                }
                self.note(format!("{} ignored", event.kind.event_type()));
            }
        }
        self.reconcile();
        Ok(outcome)
    }

    /// Runs `plan`, applies and prints its decisions, then runs `check`.
    fn reconcile(&mut self) {
        let plan = match plan(&self.snapshot, &self.settings, self.now) {
            Ok(plan) => plan,
            Err(e) => {
                self.error(format!("plan error: {e}"));
                return;
            }
        };
        for event in plan.decisions {
            self.name(&event);
            self.track(&event);
            if self.verbose {
                println!("  {}", self.describe(&event));
            }
            if let Err(e) = apply(&mut self.snapshot, &event) {
                self.error(format!("apply error: {e}"));
            }
            self.log.push(event);
        }
        for violation in check(&self.snapshot, &self.settings) {
            self.error(format!("violation: {violation}"));
        }
    }

    fn error(&mut self, message: String) {
        if self.verbose {
            println!("  {message}");
        }
        self.note(message);
        self.violations += 1;
    }

    fn note(&mut self, note: String) {
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    fn target_head(&self) -> String {
        self.snapshot
            .target
            .head
            .as_ref()
            .map(|h| h.value.clone())
            .unwrap_or_default()
    }

    fn begin(&mut self) {
        self.before = self.lanes.clone();
        self.target_before = self.target_head();
        self.touched.clear();
        self.notes.clear();
    }

    fn end(&mut self, line: usize, command: &str) {
        let target = self.target_head();
        let target = if target == self.target_before {
            if target.is_empty() { "" } else { "│" }.to_owned()
        } else {
            target
        };
        let cells = self
            .lanes
            .iter()
            .map(|(pr, lane)| {
                let changed = self.before.get(pr) != Some(lane) || self.touched.contains(pr);
                let text = if changed {
                    let again = if self.before.get(pr) == Some(lane) {
                        "↺ "
                    } else {
                        ""
                    };
                    let attempt = if lane.state == ItemState::Testing && lane.attempt > 1 {
                        format!(" #{}", lane.attempt)
                    } else {
                        String::new()
                    };
                    format!("{again}{} {}{attempt}", symbol(lane.state), lane.state)
                } else if lane.state.is_terminal() {
                    String::new()
                } else {
                    "│".to_owned()
                };
                (*pr, text)
            })
            .collect();
        self.rows.push(Row {
            line,
            command: command.to_owned(),
            target,
            cells,
            notes: std::mem::take(&mut self.notes),
        });
    }

    /// Updates the lanes and the notes with a decision.
    fn track(&mut self, event: &Event) {
        let pr_of = |id: &ItemId| self.item_prs.get(id).copied();
        match &event.kind {
            EventKind::ItemTransitioned {
                item_id,
                to,
                reason,
                ..
            } => {
                let Some(pr) = pr_of(item_id) else { return };
                let attempt = self.pr_attempts.get(&pr).copied().unwrap_or(1);
                self.lanes.insert(
                    pr,
                    Lane {
                        state: *to,
                        attempt,
                    },
                );
                self.touched.insert(pr);
                if !NORMAL.contains(reason) {
                    self.note(format!("PR {pr} {reason}"));
                }
            }
            EventKind::ItemReadinessChanged {
                item_id,
                block_reasons,
                ..
            } if !block_reasons.is_empty() => {
                if let Some(pr) = pr_of(item_id) {
                    self.note(format!("PR {pr} blocked: {}", blocks(block_reasons)));
                }
            }
            EventKind::GroupFormed {
                item_ids,
                predecessor_ids,
                ..
            } if !predecessor_ids.is_empty() => {
                let prs: Vec<String> = predecessor_ids
                    .iter()
                    .filter_map(pr_of)
                    .map(|p| format!("PR {p}"))
                    .collect();
                if let Some(pr) = item_ids.first().and_then(pr_of) {
                    self.note(format!("PR {pr} tested on top of {}", prs.join("+")));
                }
            }
            EventKind::QueueTransitioned { to, reason, .. } => {
                self.note(format!("queue {to} ({reason})"));
            }
            _ => {}
        }
    }

    /// The desired state in one line: each test branch as its base and PRs.
    fn desired(&self) -> String {
        let Ok(plan) = plan(&self.snapshot, &self.settings, self.now) else {
            return "(plan error)".into();
        };
        let pr_of = |sha: &String| {
            self.snapshot
                .items
                .values()
                .find(|i| i.head_sha == *sha)
                .map_or_else(|| sha.clone(), |i| i.pr_number.to_string())
        };
        let mut parts: Vec<String> = plan
            .desired
            .test_branches
            .iter()
            .map(|b| {
                let prs: Vec<String> = b.head_shas.iter().map(pr_of).collect();
                format!("test {} on {}", prs.join("+"), b.base_sha)
            })
            .collect();
        parts.extend(
            plan.desired
                .merges
                .iter()
                .map(|m| format!("merge PR {}", m.pr_number)),
        );
        if parts.is_empty() {
            "nothing".into()
        } else {
            parts.join("; ")
        }
    }

    fn print_lanes(&self) {
        if self.verbose {
            return;
        }
        let prs: Vec<u64> = self.lanes.keys().copied().collect();
        let width = |header: &str, cells: &mut dyn Iterator<Item = usize>| {
            cells.chain([header.chars().count()]).max().unwrap_or(0)
        };
        let command_w = width(
            "fact",
            &mut self.rows.iter().map(|r| r.command.chars().count()),
        );
        let target_w = width(
            "main",
            &mut self.rows.iter().map(|r| r.target.chars().count()),
        );
        let pr_w: Vec<usize> = prs
            .iter()
            .map(|pr| {
                width(
                    &format!("PR {pr}"),
                    &mut self
                        .rows
                        .iter()
                        .map(|r| r.cells.get(pr).map_or(0, |c| c.chars().count())),
                )
            })
            .collect();
        let legend = [
            ItemState::NotReady,
            ItemState::Pending,
            ItemState::Testing,
            ItemState::PendingFailure,
            ItemState::TestsPassed,
            ItemState::Merged,
            ItemState::Failed,
            ItemState::Cancelled,
        ]
        .map(|s| format!("{} {s}", symbol(s)))
        .join("  ");
        println!("{legend}  ↺ restarted  │ waits\n");
        let mut header = format!(
            "line  {}  {}",
            pad("fact", command_w),
            pad("main", target_w)
        );
        for (pr, w) in prs.iter().zip(&pr_w) {
            let _ = write!(header, "  {}", pad(&format!("PR {pr}"), *w));
        }
        println!("{header}  what left the normal path");
        println!("{}", "─".repeat(header.chars().count() + 27));
        for row in &self.rows {
            let mut text = format!(
                "{:>4}  {}  {}",
                row.line,
                pad(&row.command, command_w),
                pad(&row.target, target_w)
            );
            for (pr, w) in prs.iter().zip(&pr_w) {
                let cell = row.cells.get(pr).map_or("", String::as_str);
                let _ = write!(text, "  {}", pad(cell, *w));
            }
            text.push_str("  ");
            text.push_str(&row.notes.join("; "));
            println!("{}", text.trim_end());
        }
    }

    /// Keeps the PR of each item, group and run, which may leave the
    /// snapshot.
    fn name(&mut self, event: &Event) {
        match &event.kind {
            EventKind::ItemTransitioned {
                item_id,
                from: None,
                ..
            } => {
                let submission = event.cause_id.and_then(|c| self.snapshot.requests.get(&c));
                if let Some(Request::Submit { pr_number, .. }) = submission {
                    self.item_prs.insert(*item_id, *pr_number);
                }
            }
            EventKind::GroupFormed {
                group_id, item_ids, ..
            } => {
                if let Some(pr) = item_ids.first().and_then(|i| self.item_prs.get(i)) {
                    self.group_prs.insert(*group_id, *pr);
                }
            }
            EventKind::RunTransitioned {
                run_id,
                group_id,
                from,
                attempt,
                ..
            } => {
                if let Some(pr) = self.group_prs.get(group_id) {
                    self.run_attempts.insert(*run_id, (*pr, *attempt));
                    if from.is_none() {
                        self.pr_attempts.insert(*pr, *attempt);
                    }
                }
            }
            _ => {}
        }
    }

    fn describe(&self, event: &Event) -> String {
        let item = |id: &ItemId| {
            self.item_prs
                .get(id)
                .map_or_else(|| id.to_string(), |pr| format!("PR {pr}"))
        };
        let from = |from: Option<String>| from.unwrap_or_else(|| "(new)".into());
        match &event.kind {
            EventKind::ItemTransitioned {
                item_id,
                from: f,
                to,
                reason,
                position,
                ..
            } => {
                let at = position
                    .map(|p| format!(", position {p}"))
                    .unwrap_or_default();
                format!(
                    "item   {:<6} {} -> {to} ({reason}{at})",
                    item(item_id),
                    from(f.map(|s| s.to_string()))
                )
            }
            EventKind::ItemReadinessChanged {
                item_id,
                mergeability,
                block_reasons,
            } => format!(
                "item   {:<6} readiness {mergeability}, blocked by [{}]",
                item(item_id),
                blocks(block_reasons)
            ),
            EventKind::QueueRenumbered { positions } => {
                format!("queue  renumbered {} items", positions.len())
            }
            EventKind::GroupFormed {
                item_ids,
                predecessor_ids,
                base_sha,
                ..
            } => format!(
                "group  {:<6} formed on {base_sha}, on top of [{}]",
                item_ids.first().map(item).unwrap_or_default(),
                predecessor_ids
                    .iter()
                    .map(item)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            EventKind::GroupTransitioned {
                group_id,
                from,
                to,
                reason,
            } => format!(
                "group  {:<6} {from} -> {to} ({reason})",
                self.group_prs
                    .get(group_id)
                    .map(|pr| format!("PR {pr}"))
                    .unwrap_or_default()
            ),
            EventKind::RunTransitioned {
                run_id,
                from: f,
                to,
                reason,
                attempt,
                test_branch,
                ..
            } => {
                let pr = self.run_attempts.get(run_id).map(|(pr, _)| *pr);
                let branch = if f.is_none() {
                    format!(", branch {test_branch}")
                } else {
                    String::new()
                };
                format!(
                    "run    {:<6} attempt {attempt}: {} -> {to} ({reason}{branch})",
                    pr.map(|pr| format!("PR {pr}")).unwrap_or_default(),
                    from(f.map(|s| s.to_string()))
                )
            }
            EventKind::QueueTransitioned { from, to, reason } => {
                format!("queue  {from} -> {to} ({reason})")
            }
            other => format!("{}", other.event_type()),
        }
    }

    fn show(&self) {
        let s = &self.snapshot;
        println!("\nqueue: {}, mode {}", s.state, s.mode);
        let head = s.target.head.as_ref().map_or("(unknown)", |h| &h.value);
        println!("target head: {head}");
        println!("items in queue order:");
        for item in s.queue_order() {
            println!(
                "  PR {:<4} position {:<6} {:<16} {}",
                item.pr_number,
                item.position.unwrap_or_default(),
                item.state.to_string(),
                item.state_reason
            );
        }
        for item in s.items.values().filter(|i| i.position.is_none()) {
            println!(
                "  PR {:<4} (not admitted)  {:<16} [{}]",
                item.pr_number,
                item.state.to_string(),
                blocks(&item.block_reasons)
            );
        }
        if let Ok(plan) = plan(s, &self.settings, self.now) {
            println!("desired test branches:");
            for branch in &plan.desired.test_branches {
                println!(
                    "  {}: {} + [{}]",
                    branch.name,
                    branch.base_sha,
                    branch.head_shas.join(", ")
                );
            }
            println!("desired merges:");
            for merge in &plan.desired.merges {
                println!("  PR {} at {}", merge.pr_number, merge.head_sha);
            }
        }
        println!();
    }

    fn finish(&self) -> ExitCode {
        let mut replay = self.initial.clone();
        let replayed = self
            .log
            .iter()
            .all(|e| apply(&mut replay, e) == Ok(Outcome::Applied))
            && replay == self.snapshot;
        let facts = self
            .log
            .iter()
            .filter(|e| !e.kind.event_type().is_decision())
            .count();
        println!(
            "log: {} events ({facts} facts, {} decisions); replay gives the same snapshot: {replayed}",
            self.log.len(),
            self.log.len() - facts
        );
        println!("violations: {}", self.violations);
        if replayed && self.violations == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}

fn pr_number(word: Word) -> Result<u64, LineError> {
    word.text
        .trim_start_matches('#')
        .parse()
        .map_err(|_| word.error(format!("`{}` is not a PR number", word.text)))
}

fn test_sha(run_id: RunId) -> String {
    format!("test-{run_id}")
}
