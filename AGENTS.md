# CIHub: how we work, for human and agent contributors

CIHub is an open-source, self-hosted merge queue for GitHub, written in Rust on top of Postgres. It takes the
approved, green PRs, orders them per target branch, retests each one on top of those ahead of it, and merges only
what still passes. `GOAL.md` says why; this file says how we work.

**The code is the source of truth (D1).** The documents state the intent and the shared vocabulary, and they can
lag behind. When a document and the code disagree, the code wins and we fix the document.

**No backward compatibility before the first published version.** When a decision changes a form, remove the old
one in the same change: code, routes, clients, tests, docs. No alias, no fallback, no "legacy". After the first
version, the event log stays replayable and the published migrations no longer change.

## How we work

- **Propose, then apply.** Every choice of architecture, dependency, crate, vocabulary, public format or name is
  proposed to the project lead and validated before it is written. A validated decision is recorded in
  `docs/DECISIONS.md` (skill `/decision`): append only, stable `Dn` identifier, date, name. An agent never adds a
  decision on its own. When in doubt, it is a decision.
- **One lot at a time.** Take the current lot in `docs/LOTS.md` (skill `/lot`), respect its frozen interfaces, do
  not anticipate the next lots. Its automated exit criterion defines "done". The status and the "Tracking" block
  of the lot are updated in the same change as the code (D3).
- **Verify before saying "done".** `mise run check` (lint, then tests); `mise run test-integration` when the
  change touches Postgres or a job; `mise run cover` for a pure crate. Show the output. Never claim that something
  works without having seen it pass.
- **Document in the same change.** A behavior change updates the document of its subject (map below).
  Everything in the repository is in English: documents, code, comments, logs, error messages, commits. Reply to a
  person in the language they write in.
- **Commits** follow Conventional Commits (`feat(mergequeue): …`); release-please turns them into versions. Do
  not commit unless asked. lefthook runs `cargo fmt` on commit and `mise run check` on push.
- **Explain simply.** A question to the project lead gives the context, the options, a recommendation, and a
  diagram when it helps. No jargon without a definition.

## The ten principles (what a PR is judged on)

One idea: **the queue is a log; everything else is a projection of the log or a convergence toward the state it
describes.** A violation means that the principle is wrong (new decision) or that the PR is; never "we will fix it
later". The details are in `docs/ARCHITECTURE.md`.

| #    | Principle                                                                                                                                                                 | The question to ask                                                        |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| I    | The log is the source of truth of the queue; `queue_items`, `test_groups`, `test_runs` are projections, rebuildable by replay                                             | "If I replay the log, do I get the same state?"                            |
| II   | The core is pure: `cihub-domain`, `cihub-conditions`, `cihub-config`, `cihub-mq-core` do no I/O and receive the clock and the identifiers as parameters                   | "Does this test run without Postgres or GitHub?"                           |
| III  | Converge, do not drive: a job describes a desired state, checks the real state before acting, and has no effect if the state is already reached                           | "If this job runs twice, or stops halfway, what happens?"                  |
| IV   | The target branch stays green: a PR merges only after a test on a commit that contains all its predecessors                                                               | "Which commit was tested, and is it exactly what lands?"                   |
| V    | No intent is lost: ingestion stores before deciding; a lost webhook is a delay, and the repair loop catches it up                                                         | "If this webhook never arrives, who catches it up, and when?"              |
| VI   | Every decision is explained by a reason from the closed vocabulary (`docs/DOMAIN.md`), never by a free string                                                             | "What does the user read on their PR?"                                     |
| VII  | The SCM budget is scheduled: every call goes through the `Scm` trait, with a class `interactive` > `merge` > `speculate` > `repair`                                       | "How many points per hour for 50 waiting PRs?"                             |
| VIII | Isolation per organization: every tenant table carries `org_id` and RLS; every scheduler query filters by `queue_id`                                                      | "Can an organization read or slow down another one?"                       |
| IX   | Modules do not know each other: `cihub-mergequeue` and `cihub-flakytests` talk through the traits of `cihub-contracts` and through events                                 | "Module disabled, does the queue behave exactly the same?"                 |
| X    | Keep CIHub small: Postgres only, a strategy is a table of default values, every dependency and every crate is validated (D2, D9)                                          | "Which line of `GOAL.md` does this change serve?"                          |

## Rules that the tools do not enforce

`Cargo.toml` (lints), `clippy.toml` and `deny.toml` already enforce: no `unwrap` or `expect` outside tests, no
standard output, no clock or randomness outside their local exception (D16, D20), no `tokio`, `sqlx`, `reqwest`
or `axum` in the pure crates, a single version per dependency in `[workspace.dependencies]`. When
one of these tools refuses, fix the cause, never the rule. The rest:

- **Neutral, closed vocabulary.** No SCM name outside `cihub-github`, its `CIHUB_GITHUB_*` variables and
  `docs/GITHUB.md` (D78). A new state, reason or event type is declared in `cihub-domain` (with its `///`
  comment) and in the Postgres enum, in the same change; a test compares the two. CIHub takes no interface,
  command or format from another product (D139).
- **Errors**: one `thiserror` enumeration per library crate, which keeps the cause; `anyhow` only in
  `cihub-cli`, the crate of the `cihub` binary.
- **Transitions guarded in SQL** (`UPDATE … WHERE id = $1 AND state = $2`) with a check of the row count.
- **Async**: the core and `plan` are synchronous; every loop receives a `CancellationToken` and stops when it is
  cancelled; never hold a lock across an `.await`.
- **Logs**: the writing rules are in `docs/OBSERVABILITY.md` (constant message, data in fields, level according
  to who must read). The pure crates write no logs: the caller writes their decisions.
- **Tests**: tables, `proptest` and `insta` for the pure crates; `#[sqlx::test]` against a real Postgres in the
  `tests/postgres.rs` files; the GitHub simulator (`cihub-scm`, feature `sim`) for every queue scenario. Never the
  real GitHub outside the staging end-to-end test.
- **Secrets**: never in the repository, the tests or the logs; a secret is a `secrecy::SecretString`.
- **Tools**: everything installs with `mise install`; a version change updates `mise.lock`.
- **Code**: every crate starts with a `//!` comment that says its role and what it does not do; a comment
  explains a constraint or a reason, not what the code does.

The agent configuration is in `.agents/`: the skills (`.claude/skills` points there) and the rules per part of the
code (`.agents/rules/`). Read the rules of a part before you change it:

| Part                                                                 | Rules                     |
| -------------------------------------------------------------------- | ------------------------- |
| `cihub-domain`, `cihub-conditions`, `cihub-config`, `cihub-mq-core`  | `.agents/rules/core.md`   |
| `cihub-store`, `cihub-platform`, `migrations/`                       | `.agents/rules/store.md`  |
| `cihub-scm`, `cihub-github`                                          | `.agents/rules/scm.md`    |

## Commands

`mise tasks` lists the tasks and their description. The most used:

```sh
mise run check             # lint (fmt, clippy -D warnings, cargo deny), then unit tests
mise run test-integration  # Postgres from compose.yaml, then the tests/postgres.rs files
mise run cover             # coverage of the pure crates, 90% at least
mise run dev               # migrations, then cihub server start locally with the four roles
mise run sim               # fault_injection on 1,000 seeds (long; the CI runs it at night)
mise run load              # cost, time and useless assemblies of `batched` on a simulated day (long)
```

The CI runs one task per job (`.github/workflows/ci.yml`); `Build Success` is the required status.

## Map of the documents

One place per fact: link, do not copy. `GOAL.md` is read first.

| Document                                   | When to read it                                                                                       |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| `GOAL.md`                                  | Always: the sentence, the acceptance table, what we do not optimize                                   |
| `docs/LOTS.md`                             | Before starting: phases, current lot, exit criteria; `docs/LOTS_DONE.md` for the history              |
| `docs/DECISIONS.md`                        | Before proposing a choice: the `Dn` decisions in force and the `Qn` open questions                    |
| `docs/ARCHITECTURE.md`                     | Before the first code change: the path of an event, roles, crates                                     |
| `docs/DOMAIN.md`                           | States, transitions, reasons: the vocabulary contract                                                 |
| `docs/ALGORITHMS.md`                       | The core (`cihub-mq-core`) and the reconciliation loop (`cihub-mergequeue`)                           |
| `docs/GITHUB.md`                           | `cihub-github`, permissions, webhooks, surfaces on the PR, creation of the App                        |
| `docs/CI.md`                               | The contract between the queue and the users' CI                                                      |
| `docs/DATA_MODEL.md`                       | A migration, a query, RLS                                                                             |
| `docs/CONFIGURATION.md`                    | A setting, a strategy, a key of `.cihub.yaml`                                                         |
| `docs/API.md`                              | A route, an error code, a CLI command                                                                 |
| `docs/RELIABILITY.md`                      | A job, a call to the SCM, the repair loop: every failure has a named treatment                        |
| `docs/SECURITY.md`                         | Authentication, tokens, secrets                                                                       |
| `docs/OBSERVABILITY.md`                    | A log, a metric, an alert                                                                             |
| `docs/DEPLOYMENT.md`                       | `compose.yaml`, the image, the migrations, the project CI, the `CIHUB_*` variables                    |
| `docs/ANALYTICS.md`, `docs/FLAKY_TESTS.md` | The modules to come (lots 11 and 14)                                                                  |
| `docs/COMPETITION.md`                      | Justify a deviation from Trunk, Mergify, GitHub, GitLab, bors, Zuul, Aviator, Graphite                |
