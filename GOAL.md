# GOAL: what CIHub is for

Context: for everyone (humans and agents) who works on this repository. Read before `AGENTS.md`. This page says
**what we want**; `AGENTS.md` says **how we build it**. When a choice is not obvious, come back here and ask:
which option serves this goal best?

## The sentence

**An open-source, self-hosted merge queue for GitHub, which keeps the target branch always green with any CI,
installs in under thirty minutes with one image and one Postgres, and explains each of its decisions.**

## What this means

1. **The target branch stays green.** A PR merges only after its tests have passed on a commit that contains all
   the PRs ahead of it. The required statuses are read on the target branch, never from the PR, so that a PR
   cannot weaken its own gate.
2. **An efficient queue, not a single queue.** Several strategies chosen per queue (sequential, predictive,
   batched, parallel by affected targets), an adaptive speculation window, a failure budget that never removes a
   PR on a single failure, and four merge methods. The strategies are tables of default values, not code in the
   core.
3. **CI agnostic.** CIHub never runs the CI: it pushes a test branch and waits for the required statuses and check
   runs. GitHub Actions, Buildkite, CircleCI, Jenkins work without modification.
4. **Simple to install and operate.** One image, one Postgres 17, one GitHub App created according to
   `docs/GITHUB.md`; `docker compose up` is enough for one organization. Under thirty minutes from the first
   command to the first merged PR.
5. **Every decision is explained.** Every transition carries a reason from a closed vocabulary; an event log per
   PR answers "why is my PR still waiting" and feeds the dashboards.
6. **No intent is lost.** A submission, a command or a check result survives crashes, lost webhooks, outages and
   GitHub's rate limit: at worst it is delayed, never forgotten.
7. **Flaky tests do not reject PRs.** The `flakytests` module keeps the history of the results (native OTLP,
   converted JUnit) and the queue does not name a culprit on a test known to be flaky.
8. **Multi-tenant by construction.** One instance serves several GitHub organizations, or a third-party host; a
   noisy organization slows down its own queues, not those of others.
9. **Stay small.** Postgres is the only dependency in v1: no broker, no external cache. ClickHouse arrives behind
    an interface when a measurement justifies it.

## How we know we are there (acceptance)

| Claim                  | Bar                                                                                                                                                                                        |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Installation           | An outside person installs CIHub and merges a first PR in under 30 minutes by following the README                                                                                         |
| Green branch           | The CIHub repository merges through CIHub for 2 weeks without a red merge                                                                                                                  |
| Resilience             | Under the GitHub simulator (5xx, 429, lost, duplicated, out-of-order webhooks, killed processes), every green PR is merged exactly once, in graph order, and every red PR carries a reason |
| Lost webhook           | Costs at most one interval of the repair loop, never a blockage                                                                                                                            |
| Explainability         | Every queue exit has a reason from the closed vocabulary and a page                                                                                                                        |
| GitHub budget          | A repository with 50 waiting PRs stays under 300 GraphQL points per hour                                                                                                                   |
| Throughput (`batched`) | On a fixed simulator load of 20 PRs per hour, `batched` uses at least 30% fewer runs per merged PR than `pipelined`, with a p50 time in queue at most 1.5 times as long; bisection isolates a failure in ≤ log2(batch size) rounds of runs |
| Flaky tests            | On a repository with a known flaky test, zero PR wrongly evicted                                                                                                                           |
| Parallel               | A pilot monorepo merges disjoint PRs in parallel                                                                                                                                           |
| Scale                  | One instance holds 1,000 PRs per day with ClickHouse enabled                                                                                                                               |

## What we do **not** optimize (out of scope for v1)

- Hosting or running the CI.
- Code review, a browser extension, a Slack app beyond the outgoing webhooks, a Terraform provider.
- GitLab and Bitbucket: the `Scm` trait allows for them, v1 does not implement them.
- ClickHouse itself: it arrives when the volume justifies it, behind the `AnalyticsStore` trait.
- Uber-style probabilistic speculation: outside v1, but the log already keeps the useful attributes (`DECISIONS.md` D27).
