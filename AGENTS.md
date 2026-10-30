# CIHub

CIHub is an open-source, self-hosted merge queue for GitHub, written in Rust on top of Postgres. It takes the
approved, green PRs, orders them per target branch, retests each one on top of those ahead of it, and merges only
what still passes. `GOAL.md` says why.

**The code is the source of truth.** The specs in `specs/` state the intent and the vocabulary, and can lag
behind. When a spec and the code disagree, the code wins and we fix the spec.

**No backward compatibility before the first published version.** When a form changes, remove the old one in the
same change: code, routes, clients, tests, specs. No alias, no fallback, no "legacy".

## How we work

- **Propose what is costly to reverse.** A new dependency or crate, a change of the event log, of a public route
  or of a public format is proposed to the project lead and validated before it is written. The rest, the author
  decides and the review checks. The reason of a choice goes into the commit message, or into a comment when the
  code needs it.
- **One lot at a time.** Take the current lot in `specs/LOTS.md` (skill `/lot`), respect its frozen interfaces,
  do not anticipate the next lots. Its automated exit criterion defines "done". Update its status and its
  "Tracking" block in the same change as the code.
- **Verify before saying "done".** `mise run check` (lint, then tests); `mise run rust:integration` when the
  change touches Postgres or a job; `mise run rust:cover` for a pure crate. Show the output.
- **Update the spec of the subject in the same change** (map below). When an approach fails or is rejected for a
  reason that the code cannot show, add it to `specs/LESSONS.md`; read it before you propose a change.
- Everything in the repository is in English. Reply to a person in the language they write in.
- **Commits** follow Conventional Commits (`feat(mergequeue): …`). Do not commit unless asked.

## The ten principles (what a PR is judged on)

The queue is a log; everything else is a projection of the log or a convergence toward the state it describes.
A violation means that the principle or the PR is wrong; never "we will fix it later". Details in
`specs/ARCHITECTURE.md`.

| #    | Principle                                                                                                                                               | The question to ask                                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| I    | The log is the source of truth of the queue; `queue_items`, `test_groups`, `test_runs` are projections, rebuildable by replay                           | "If I replay the log, do I get the same state?"               |
| II   | The core is pure: `cihub-domain`, `cihub-conditions`, `cihub-config`, `cihub-mq-core` do no I/O and receive the clock and the identifiers as parameters | "Does this test run without Postgres or GitHub?"              |
| III  | Converge, do not drive: a job describes a desired state, checks the real state before acting, and has no effect if the state is already reached         | "If this job runs twice, or stops halfway, what happens?"     |
| IV   | The target branch stays green: a PR merges only after a test on a commit that contains all its predecessors                                             | "Which commit was tested, and is it exactly what lands?"      |
| V    | No intent is lost: ingestion stores before deciding; a lost webhook is a delay, and the repair loop catches it up                                       | "If this webhook never arrives, who catches it up, and when?" |
| VI   | Every decision is explained by a reason from the closed vocabulary (`specs/DOMAIN.md`), never by a free string                                          | "What does the user read on their PR?"                        |
| VII  | The SCM budget is scheduled: every call goes through the `Scm` trait, with a class `interactive` > `merge` > `speculate` > `repair`                     | "How many points per hour for 50 waiting PRs?"                |
| VIII | Isolation per organization: every tenant table carries `org_id` and RLS; every scheduler query filters by `queue_id`                                    | "Can an organization read or slow down another one?"          |
| IX   | Modules do not know each other: `cihub-mergequeue` and `cihub-flakytests` talk through the traits of `cihub-contracts` and through events               | "Module disabled, does the queue behave exactly the same?"    |
| X    | Keep CIHub small: Postgres only, a strategy is a table of default values, every dependency and every crate is validated                                 | "Which line of `GOAL.md` does this change serve?"             |

## Rules that the tools do not enforce

`mise run lint` already refuses: `unwrap`, standard output, the clock or randomness outside their exception, I/O
crates or `tracing` in the pure crates, `anyhow` outside the two binaries (`cihub-cli`, `cihub-server`),
`cihub-github` outside `cihub-server` and `cihub-web`, a block nested deeper than `clippy.toml` allows, a crate without its `//!` comment, a cited spec that
does not exist, `any` or `unknown` in the TypeScript (`unknown` stays only with a comment that says why no type can
be known). When a tool refuses, fix the cause, never the rule. A crate or a directory with its own rules has
its own `AGENTS.md`.

- **Neutral, closed vocabulary.** No SCM name outside `cihub-github`. CIHub takes no interface, command or format
  from another product.
- **Errors**: one `thiserror` enumeration per library crate, which keeps the cause.
- **Async**: every loop receives a `CancellationToken` and stops when it is cancelled; never hold a lock across an
  `.await`.
- **Logs**: constant message, data in fields, level according to who must read (`specs/OBSERVABILITY.md`). The
  caller of a pure crate writes its decisions.
- **Tests**: the GitHub simulator (`cihub-scm`, feature `sim`) for every queue scenario; never the real GitHub.
- **Secrets**: never in the repository, the tests or the logs; a secret is a `secrecy::SecretString`.
- **Readability**: separate the steps of a function (load, check, decide, act, return) and the parts of a test
  with one blank line, and follow a guard clause with one. A name says what the thing is in the domain, without
  abbreviation; a boolean reads as a question (`is_mergeable`). At most two levels of nesting in a new function.
  The `//!` comment of a crate says its role and what it does not do. A comment explains a constraint or a
  reason, never what the code does.

## Commands

`mise tasks` lists them all; every tool installs with `mise install`. A task is named `<domain>:<action>` (`rust:`,
`console:`, `website:`, `packages:`, `pnpm:`, `db:`, `server:`, `helm:`, `workflows:`); only the tasks that span domains have no prefix. `mise run check` (lint, then
unit tests), `mise run rust:integration` (the `tests/postgres.rs` files against the Postgres of `compose.yaml`),
`mise run rust:cover` (90% for the pure crates), `mise run test:all` (every suite, with Docker). Each suite writes
its JUnit report into `test-reports/<suite>/junit.xml`, which git ignores. The CI runs them in parallel jobs; the
last one, `Build Success`, is the required status.

## Migrations

- A migration is a pair `<YYYYMMDDHHMMSS>_<description>.up.sql` and `.down.sql`. `cihub-server migrate up`
  applies the up files, never at startup; `cihub-server migrate down --to <version>` runs the down files of the newer
  ones, the newest first.
- One migration creates one table, `create_<table>`, with the enums that only this table uses. An enum of
  several tables has its own, `create_type_<enum>`; the functions of row level security are in
  `create_tenancy_functions`. A later change of a table is a new migration that names it
  (`add_<column>_to_<table>`).
- A down gives back exactly the schema before its up: `each_down_migration_gives_back_the_schema_before_its_up`
  compares the catalog after each step. Postgres cannot remove a value from an enum, so the down of an added
  value recreates the type.
- An up published in a tag never changes, not even by one byte: a database that applied it refuses another file
  (`scripts/lint-rules.sh`). A change of the schema is a new migration; only a down may be fixed. Until `v0.5.0`
  is tagged, the baseline is rewritten instead, and `mise run db:reset` starts again from an empty database.
- A migration stays compatible with the version before it (expand now, contract in a later version), so that old
  and new processes run side by side during a deployment, and a rollback of the image needs no down.
- Every tenant table carries `org_id`, forced row level security and the `tenant` policy. Only `users`,
  `sessions` and `device_codes` are global: a session covers every organization of its user, and a device code
  belongs to no organization. The test
  `every_tenant_table_forces_row_level_security` fails otherwise.
- Identifiers are `text` in the typedid format. A Postgres enum mirrors its `cihub-domain` enumeration.
- Spec: `specs/DATA_MODEL.md`.


## Map of the specs

| Spec                                            | When to read it                                                    |
| ----------------------------------------------- | ------------------------------------------------------------------ |
| `GOAL.md`                                       | First: the sentence, the acceptance table, what we do not optimize |
| `specs/LOTS.md`                                 | Before starting: phases, current lot, exit criteria                |
| `specs/LESSONS.md`                              | Before proposing a change: what we tried and do not do             |
| `specs/ARCHITECTURE.md`                         | Before the first code change: the path of an event, roles, crates  |
| `specs/DOMAIN.md`                               | States, transitions, reasons: the vocabulary                       |
| `specs/ALGORITHMS.md`                           | The core and the reconciliation loop                               |
| `specs/GITHUB.md`, `specs/CI.md`                | The GitHub App and webhooks; the contract with the users' CI       |
| `specs/DATA_MODEL.md`, `specs/SCHEMA_PLAN.md`   | A migration, a query, RLS                                          |
| `specs/CONFIGURATION.md`, `specs/API.md`        | A setting or a key of `.cihub.yaml`; a route, an error, a command  |
| `specs/RELIABILITY.md`, `specs/SECURITY.md`     | A job or an SCM call; authentication, tokens, secrets              |
| `specs/OBSERVABILITY.md`, `specs/DEPLOYMENT.md` | A log, a metric; the image, the chart, the `CIHUB_*` variables     |
| `specs/ANALYTICS.md`, `specs/FLAKY_TESTS.md`    | The modules to come (lots 11 and 14)                               |
| `specs/COMPETITION.md`                          | Justify a deviation from the other merge queues                    |
