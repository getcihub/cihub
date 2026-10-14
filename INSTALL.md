# Install CIHub

From the first command to the first merged pull request in about 30 minutes.

You need Docker with Compose, a host that GitHub can reach over HTTPS, and admin rights on a GitHub organization.

## 1. Create the GitHub App

In the settings of your organization: *Settings → Developer settings → GitHub Apps → New GitHub App*. The full
procedure is in [`specs/GITHUB.md`](specs/GITHUB.md); in short:

- **Webhook**: active, URL `https://<your CIHub host>/webhooks/github`, and a random secret
  (`openssl rand -hex 32`).
- **Repository permissions**: Contents *write*, Pull requests *write*, Commit statuses *write*, Checks *read*,
  Metadata *read*, Administration *read*, Issues *write*; Organization: Members *read*. Actions *write* is
  optional.
- **Events**: Pull request, Pull request review, Issue comment, Push, Check run, Check suite, Status, Branch
  protection rule, Repository.
- **Where can this GitHub App be installed?**: *Only on this account*. Keep the App private: any account that
  installs a public App gets an organization in your CIHub.
- **Callback URL**, for the sign-in to the dashboard: `https://<your CIHub host>/auth/github/callback`.
- After the creation, note the **App ID** and generate a **private key** (`.pem` file); for the dashboard, note
  the **Client ID** and generate a **client secret**.

## 2. Configure and start

From a clone of this repository:

```sh
cp deploy/compose/.env.example deploy/compose/.env
# Fill in deploy/compose/.env: two Postgres passwords, the App ID, the webhook secret,
# the path of the private key and, for the dashboard, the address of this
# instance, the Client ID and the client secret.
docker compose -f deploy/compose/compose.yaml up -d
```

Compose starts Postgres 17, creates the database role of CIHub, applies the migrations (`cihub-migrate`), then
starts `cihub-server start` on port 8080. Put your HTTPS reverse proxy in front of this port.

On Kubernetes, the Helm chart replaces Compose: see [`deploy/charts/cihub/README.md`](deploy/charts/cihub/README.md).

## 3. Install the App and start your organization

Install the App on the repositories to serve (*Install App* on the page of the App). CIHub creates the
organization of the account (its slug is the login of the account) and reads its repositories by itself. To use
the API or the CLI, create a token:

```sh
docker compose -f deploy/compose/compose.yaml run --rm cihub-server token create --org <login of the account>
```

The command prints an API token **once**: keep it in a secret store.

## 4. Protect the target branch

Two rulesets on the target branch ([`specs/GITHUB.md`](specs/GITHUB.md)):

- one that restricts updates, with the App in its bypass list in mode **exempt**;
- one that holds the rules a PR must meet (reviews, required checks), without any bypass.

Any other ruleset must exclude the branches `cihub-temp/**/*` and `cihub-merge/**/*`. Turn off "Require branches
to be up to date before merging".

## 5. Merge a pull request

You do not create a queue. Comment on an approved, green pull request of the default branch:

```
/cihub merge
```

The first command creates the queue of the branch, with the default settings, and submits the pull request.
CIHub answers with a comment that it keeps up to date, and a commit status `CIHub Merge Queue (<branch>)`. Other
commands: `/cihub cancel`, `/cihub restart`, and options `--priority=urgent|high|medium|low|0-255`,
`--no-batch`, `--queue=<name>`. A queue on another branch, or a second queue, comes from `.cihub.yaml` (below).

CIHub checks the protection of the branch for you, once the queue exists:

```sh
curl https://<your CIHub host>/api/v1/orgs/<owner>/repos/<repo>/queues -H "Authorization: Bearer <token>"
curl https://<your CIHub host>/api/v1/orgs/<owner>/repos/<repo>/queues/<queue>/config-check -H "Authorization: Bearer <token>"
```

An empty list of `findings` means a sound configuration.

## 6. Use the CLI

In a clone of the repository:

```sh
cargo install --locked --git https://github.com/getcihub/cihub cihub-cli   # the cihub binary
cihub login --url https://<your CIHub host>   # asks for the token
cihub queue status
```

## Next: write your `.cihub.yaml` with an assistant

A queue works without a file. To change its behavior, or to add queues, add `.cihub.yaml` at the root of the
repository (or `.cihub/config.yaml`); CIHub reads it on the head of the target branch, never from a pull request. Copy this prompt into an
LLM assistant:

```text
Help me write the .cihub.yaml file of CIHub, a merge queue for GitHub. Ask me
questions first, one at a time: which branches need a queue, how long my CI
takes and how often it fails, which checks are required, which labels or
reviews must block a merge, and which pull requests must go first.

Then write the file, with only the keys below, and explain each choice.

Rules of the file (version 1):
- It starts with `version: 1`. Top-level keys: `queues` (a list),
  `priority_rules` (a list) and `defaults` (any queue setting, without `name`,
  `branch` or conditions). An unknown key is an error.
- Each queue has `name` (unique in the file) and `branch`. Several queues can
  target the same branch: a pull request goes to the first one, in file
  order, whose `route_conditions` pass (a queue without them takes every
  pull request), else to the first queue of the branch.
- If the file has no queue for the default branch, that branch has a queue
  named after it. Other branches have a queue only with an entry.
- `strategy`: `sequential` (one test at a time), `pipelined` (default: tests
  in parallel, each on top of the ones ahead) or `batched` (`pipelined` with
  several pull requests per test; a failed batch is split to find the
  cause). Do not use `parallel`: it is not ready yet.
- `batching: { enabled, size: 2-20, max_wait_minutes: 1-60 }`: on with
  `batched` (size 4, wait 5 by default), off otherwise.
- `merge_method`: `squash` (default), `merge_commit`, `rebase`, or
  `fast_forward` (only with `strategy: sequential`).
- `trigger`: `draft_pr` (default: a draft pull request runs the CI of each
  test) or `push` (the push of the test branch runs it).
- `window: { floor: 1-100, ceiling: 1-100 }`, floor <= ceiling (default 2 /
  20, 1 / 1 with `sequential`), or `testing_concurrency: 1-100` instead.
- `failure_budget: { retries: 0-10 }` (default 1): retries before a pull
  request fails.
- `testing_timeout_minutes` (10-1440, default 300), `not_ready_timeout_hours`
  (1-720, default 168), `keep_test_branches_minutes` (0-1440, default 0).
- `merge_commit_title`: `pr_title` (default) or `pr_title_and_number`.
- Booleans: `comments_enabled`, `commands_enabled`, `status_check_enabled`
  (default true), `state_labels_enabled`, `resubmit_on_push` (default false).
- `label_commands: { enabled: true, enqueue_label: <label> }`: a label that
  submits the pull request (default `cihub-merge-queue-submit`).
- `direct_merge_mode`: `off` (default) or `always` (when no test runs, merge
  the first pull request without a test branch).
- `branch_protection_injection`: `both` (default), `merge` or `none`: whether
  the checks required by the branch rules also count before admission and on
  the test commit.
- `required_statuses`: a list of check names for the test commit; without
  the key, they come from the branch rules. With no required check at all,
  every test fails.
- `route_conditions`: which pull requests go to this queue.
  `queue_conditions`: what a pull request must meet to enter the queue.
  `merge_conditions`: only `check-success = <name>` items, which add
  required checks on the test commit.
- `priority_rules`: a list of `{ name, conditions, priority }`, where priority
  is `urgent` (0), `high` (10), `medium` (100, default), `low` (200) or 0-255;
  a lower number goes first; the first rule that matches wins.

Conditions: `[-][#]attribute [operator value]`, where `-` negates and `#`
counts the items of a list (`#approved-reviews-by >= 2`); nested `and:`,
`or:` and `not:` blocks take a list. Operators: `=`, `!=`, `<`, `<=`, `>`,
`>=` (numbers, durations, counts, sender-permission), `~=` (regular
expression) and `*=` (glob) on text and lists. A list matches when one item
matches; `!=` means no item is equal. Attributes: draft, conflict, from-fork,
behind-base, number, title, body, review-decision (approved,
changes_requested, review_required, none; `=` or `!=`), approved-reviews-by,
changes-requested-reviews-by, review-requested, review-threads-unresolved,
check-success, check-neutral, check-skipped, check-failure, check-pending,
label, author, sender, sender-permission (none < read < write < admin), files,
additions, deletions, base, head, queue-name, pr-age. Durations are `<n>m`,
`<n>h` or `<n>d`. A boolean attribute alone is true, without an operator
(`-draft` means "not a draft"). In YAML, quote a condition that starts with
`#` or holds `[` or `]`.
```

The reference of every key is [`specs/CONFIGURATION.md`](specs/CONFIGURATION.md).

