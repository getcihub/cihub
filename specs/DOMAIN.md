# Domain model: the vocabulary contract

This spec defines the terms that humans and agents use, and the rules of the vocabulary that the code does not
say in one place. The values of each closed vocabulary, their meaning and the allowed transitions are in
`cihub-domain`, with a `///` comment on each value; that code is the source of truth.

## Entities

Code: `crates/domain/src/` (`OrgId`, `ConnectionId`, `RepoId`, `Queue`, `Item`, `TestGroup`, `TestRun`,
`CheckObservation`, `ImpactedTargets`, `Event`)

- **DOM-1** The vocabulary is that of CIHub, not that of an SCM: each adapter (`cihub-github`) translates the
  values of its SCM, and the core sees none of them. A **PR** is any change request.
- **DOM-2** An organization installs the App on repositories through a connection; a repository has one queue per
  target branch; a queue contains items, one per submission of a PR; items form test groups (one PR, a batch, or
  a half of a bisection), which own test runs. Lanes, bisections and stacks follow from these.
- **DOM-3** A submission after a cancellation creates a new item; `is_current` names the live item of a PR.
- **DOM-4** The organization is the tenancy boundary: every table carries `org_id` except `users`, `sessions` and
  `device_codes`, which are global (`specs/DATA_MODEL.md`).
- **DOM-5** Identifiers are typed ULIDs in the `typedid` text format. An SCM identifier is an `external_id`,
  unique within the deployment. The text value (`snake_case`) of a closed vocabulary is the same in the API, the
  webhooks, the analytics and Postgres.

## Log

Code: `crates/domain/src/event.rs` (`Event`, `EventKind`, `EventType::is_decision`, `Actor`),
`crates/mq-core/src/apply.rs` (`apply`), `crates/mq-core/src/snapshot.rs` (`Request`)

- **DOM-6** **Log**: the append-only sequence of the events of a queue, the source of truth of the queue. The state
  tables are projections of the log.
- **DOM-7** **Fact**: what GitHub, the CI or a user established ("PR 12 has a new head SHA"). A fact changes only
  the observed state.
- **DOM-8** **Decision**: what the queue chose ("the item of PR 12 moves to `cancelled`, reason `pr_pushed_to`").
  A decision changes only the state of the queue and carries the fact that caused it (`cause_id`).
- **DOM-9** **Request**: a fact that waits for a decision (submission, cancellation, restart, job result).
- **DOM-10** **`apply`**: the pure function that applies an event to the state; the replay uses only it.
  **`plan`**: the pure function that reads the state and the time, and gives decisions and the desired state.
- **DOM-11** **Reason**: the cause of a transition, from a closed vocabulary; each transition carries one.
  **Actor**: who caused an event: an SCM user account, the App account, or CIHub.

## Reasons and states

Code: `crates/domain/src/reason.rs` (`Reason`, `Reason::describe`), `crates/domain/src/transition.rs`
(`Lifecycle`, `check_transition`), `crates/domain/src/item.rs` (`ItemState`, `BlockReason`, `Priority`),
`crates/domain/src/pr.rs` (`Mergeability`, `PrState`)

- **DOM-12** One `Reason` enumeration serves every state machine. For each one, a table gives the reasons that
  each (start state, end state) allows.
- **DOM-13** The reason code is the stable contract. Its sentence (`Reason::describe`) is a free rendering for a
  person, which the PR, the API and the CLI show, and which can change without a decision.
- **DOM-14** A `not_ready` item also carries block reasons: what prevents its admission.
- **DOM-15** Only the priority value orders the queue; the names are shortcuts (`specs/ALGORITHMS.md`, ALG-6).
- **DOM-19** The state of a queue (`QueueState`) is the choice of the admin and of a mode switch; the access to
  the repository is apart. `access.lost` stops the queue whatever its state, which stays as it is, and
  `access.restored` lets it go on in that state. A request of an operator without the access changes the state,
  and the queue stays stopped until the access comes back.

## Groups, runs, checks and jobs

Code: `crates/domain/src/group.rs` (`GroupState`), `crates/domain/src/run.rs` (`RunStatus`),
`crates/domain/src/check.rs` (`CheckObservation::is_regressed_by`), `crates/domain/src/job.rs` (`JobError`)

- **DOM-16** A group has its own state, apart from that of its items, because a split or dissolved group has no
  items left, and the members of a batch move to `merged` one by one.
- **DOM-17** A check report never takes a check back to an earlier status, unless it started after the end of
  that check, as a rerun does. GitHub does not order its deliveries: a late `queued` or `in_progress` of the same
  execution is ignored.
- **DOM-18** A job that fails reports an error (`JobError`), never a reason: `plan` chooses the reason.
