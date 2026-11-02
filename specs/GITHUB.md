# GitHub integration

This spec gives the rules of the GitHub App of CIHub: its permissions and events, its creation, the reads of a PR,
the merge, what the queue writes on a PR, the commands, the rulesets, the sign-in and the onboarding. The calls
are in `crates/github/src/` and the jobs that use them in `crates/platform/src/` and `crates/mergequeue/src/`.

## The App

Code: `crates/github/src/client.rs` (`TOKEN_MARGIN`), `crates/github/src/apps.rs`

- **GH-1** CIHub is one GitHub App, which the operator creates by hand. A call uses an installation token from the
  private key of the App, or the token of a person; CIHub never needs to write the administration of a repository.

| Permission      | Level                  | Usage                                                                  |
| --------------- | ---------------------- | ---------------------------------------------------------------------- |
| Contents        | write                  | create and delete the test refs, merge, read the configuration file    |
| Pull requests   | write                  | read PRs and mergeability, open and close draft PRs, merge, comment    |
| Commit statuses | write                  | publish the queue status and the conditions status                     |
| Checks          | read                   | observe the check runs on the PR heads and the test SHAs               |
| Metadata        | read                   | list the repositories                                                  |
| Administration  | read                   | read the protections and rulesets: required checks and setup           |
| Issues          | write                  | labels on PRs (submission, state), reactions on command comments       |
| Members         | read                   | map GitHub logins to the roles of the dashboard                        |
| Actions         | read; write (optional) | read: `workflow_job`; write: cancel old workflows, restart failed jobs |

## Events

Code: `crates/github/src/translate.rs` (`translate`), `crates/github/src/webhook.rs`

- **GH-2** The App subscribes to `pull_request`, `pull_request_review`, `issue_comment` (commands), `push`,
  `check_run`, `status`, `workflow_job`, `branch_protection_rule` and `repository`. `installation` and
  `installation_repositories` arrive without a subscription. Every other event is ignored.
- **GH-3** GitHub sends no `check_run` when a check run goes from `queued` to `in_progress`: the `in_progress`
  action of `workflow_job` gives that change for a GitHub Actions job. Without `actions: read`, a check shows
  `queued` until it ends.
- **GH-4** `POST /webhooks/github` verifies the HMAC signature, stores the delivery without routing it and answers
  `202` in under 2 seconds. A delivery is deduplicated by `X-GitHub-Delivery` while it waits in the inbox
  (principle V).

## Creating the App

The operator creates the App in the GitHub settings of their organization (*Settings → Developer settings →
GitHub Apps → New GitHub App*) or of their account:

1. Any name, and homepage URL: the address of the instance.
2. *Webhook* active, with the URL `<CIHub public address>/webhooks/github` and a random secret (for example
   `openssl rand -hex 32`).
3. The permissions of the table above, with `Actions: read`; `Actions: write` only to cancel and restart the CI.
4. The events of GH-2.
5. *Where can this GitHub App be installed?*: *Only on this account*, unless it serves several accounts. Any account
   that installs a public App gets an organization in CIHub.
6. After creation: note the *App ID* and generate a private key (`.pem` file).
7. For the sign-in to the dashboard: the *Callback URL* `<CIHub public address>/auth/github/callback`, then note the
   *Client ID* and generate a *client secret*. *Request user authorization (OAuth) during installation* stays off:
   on, GitHub sends the person to the Callback URL and skips the Setup URL.
8. The *Setup URL* `<CIHub public address>/auth/github/installed`, with *Redirect on update* checked: after an
   install or a change of its repositories, GitHub sends the person back to CIHub.

The server then receives the `CIHUB_GITHUB_*` variables (`specs/DEPLOYMENT.md`), and the operator installs the App
on the repositories to serve. An App that already exists is suitable if its permissions, events and webhook URL
are these. Without the client ID and secret, nobody signs in to the dashboard, and the API tokens still work.

## Routing

Code: `crates/platform/src/routing.rs` (`find_route`), `crates/platform/src/sweep.rs` (`run_sweep`)

- **GH-5** The router finds the queue of a delivery only when a single one is certain: a PR by its base branch, a
  push by its branch, a check by the name of its test branch or by its test SHA (`specs/CI.md`). Any other
  delivery goes to its connection only.
- **GH-6** A delivery of an installation that CIHub does not know, and an `installation` delivery, ask for a sync
  of the installation.
- **GH-7** `sweep_deliveries` asks GitHub again for the deliveries that no attempt answered with a 2xx. CIHub
  answers `202` only after it stores a delivery, so a lost delivery is a delay.

## Reads of a PR

Code: `crates/github/src/pulls.rs` (`read_prs`, `mergeability`), `crates/github/src/checks.rs` (`check_reports`)

- **GH-8** Mergeability is read, never computed: one GraphQL request reads up to 50 PRs of a repository.
  `UNSTABLE` and `HAS_HOOKS` admit the PR, as the branch protection of GitHub does.
- **GH-9** `UNKNOWN` asks for a read again after a short delay, because GitHub computes the value lazily; the item
  has the block reason `undetermined` in the meantime.
- **GH-10** For a `blocked` PR, the reason names the required check that fails or has not concluded;
  `undetermined` stays when no required check explains it.
- **GH-11** When the only unsatisfied required checks are the statuses of CIHub for the queue itself and no review
  is missing, the PR is `mergeable`. Otherwise a repository that requires the queue status could never enter the
  queue.
- **GH-12** Events drive the reads; the polling of the tick is a safety net. A check on the head of a `not_ready`
  item asks a read only once it has concluded. The polling alone keeps a repository with 50 waiting PRs under 300
  GraphQL points per hour (principle VII).

## Merge

Code: `crates/github/src/pulls.rs` (`merge_pr`, `merge_pr_async`)

- **GH-13** `merge_pr` merges with `PUT /pulls/{n}/merge` and the `sha` guard, the only merge endpoint of GitHub
  Enterprise Server. When GitHub refuses a PR of a stack with a `403` that names its asynchronous endpoint, the job
  asks `merge-async` with the same `sha` and message. The job keeps no identifier of that request: a pending merge
  is asked again until the merge attempts end, then the item fails with `merge_already_in_progress`.

## What the queue writes on a PR

Code: `crates/mergequeue/src/jobs/mod.rs` (`queue_status_context`, `conditions_status_context`),
`crates/mergequeue/src/surfaces.rs` (`converge_surfaces`), `crates/mergequeue/src/wording.rs`,
`crates/mergequeue/src/conditions_status.rs` (`observe`)

- **GH-14** The reconciliation writes the desired rendering of a PR in `pr_surfaces`, and `publish_surfaces`
  publishes it (principle III).
- **GH-15** The queue status, `CIHub Merge Queue (<target branch>)` on the PR head, is `pending` while the item is
  live and untested, `success` from `tests_passed` on, `failure` on `failed` and `error` on `cancelled`. The
  `success` comes before the merge call, so that a rule that requires the status holds when the queue merges.
- **GH-16** A team can require the queue status, so that nothing merges without a test of the queue; the ruleset
  that restricts updates (GH-25) is what prevents a merge outside the queue.
- **GH-17** CIHub writes one comment per PR, in English, and edits it in place at each change. It posts it when the
  PR opens out of draft on a branch with a queue, at the next change of an open PR without one, or at the
  submission.
- **GH-18** The comment links where the test runs (the draft PR, or the test commit), never the test branch. For a
  batch that waits, it gives when the batch closes, never when its test starts. It writes a login in a code span,
  so that no one is mentioned.
- **GH-19** State labels are optional. CIHub keeps them up to date on a PR that already carries one, even after the
  setting is turned off.
- **GH-20** Each open PR on a branch with a queue, submitted or not, carries the conditions status, `CIHub
  Conditions (<target branch>)`: whether the PR would enter its queue now. It is `pending` when the only reasons
  are `undetermined` or a check that has not concluded, so that a running CI never shows red. It never says
  whether the PR passed a test of the queue: the queue status does.
- **GH-21** The conditions never see the statuses of CIHub, so that a `checks.failure` condition cannot fail on the
  status that CIHub writes.
- **GH-22** The conditions status must not be a required check: a PR whose conditions pass would then merge outside
  the queue. `read_setup` reports it as `conditions_status_required`.
- **GH-23** The conditions status is read in the class `repair`, so that it never slows the queue, and written
  without an event: the log does not change. There is no polling: a lost delivery leaves the status behind until
  the next event of the PR or a `…/sync`.

## Commands

Code: `crates/mergequeue/src/command.rs`, `crates/mergequeue/src/jobs/submit_pr.rs`

- **GH-24** A comment holds `/cihub merge`, `/cihub cancel` or `/cihub restart`; no command of another product is
  accepted. The author needs the `write` permission on the repository. The submission label and the checkbox of
  the comment of CIHub (checked by a person other than the bot) submit through the same path as `/cihub merge`.
  A refused command gets a comment with its cause; an accepted one gets a `+1` reaction.

## Rulesets and setup

Code: `crates/scm/src/repo.rs` (`SetupFinding`), `crates/mergequeue/src/config.rs` (`read_setup`),
`crates/mergequeue/src/repo_setup.rs` (`read_repo_setup`)

- **GH-25** The target branch has two rulesets: one restricts updates, with the App in its bypass list in
  **exempt** mode (the `always` mode does not cover the branch updates of an App); the other carries the rules of
  mergeability, without a bypass.
- **GH-26** Every other ruleset excludes `cihub-temp/**/*` and `cihub-merge/**/*`. "Require branches to be up to
  date" stays off, unless the queue merges directly.
- **GH-27** `read_setup` reads the setup of a queue every hour and at a `…/sync` (class `repair`, about 5 REST
  points per queue and per hour): the four points of `SetupFinding` on the target branch,
  `conditions_status_required`, and `target_branch_missing` when the branch is gone. A page reads it without a call.
- **GH-28** `read_repo_setup` reads the setup of a repository at its discovery, once a day and at
  `POST …/repos/{repo}/sync`: the four points of `SetupFinding` on the default branch, and the configuration file at
  its head. It does not check `conditions_status_required`, and a default branch that is gone keeps no finding.

## Sign-in and roles

Code: `crates/github/src/people.rs`, `crates/web/src/sign_in.rs`, `crates/platform/src/persons.rs` (`role_now`),
`crates/platform/src/members.rs` (`run_sync_members`)

- **GH-29** A person signs in through the user authorization of the App, with PKCE `S256` and a `state`. GitHub
  shows no account picker to a GitHub App: to use another account, a person signs out of GitHub.
- **GH-30** An active membership of an organization gives `admin` or `member`; on the account of a person, only its
  owner is `admin`; another kind of account gives no role.
- **GH-31** A permission or a role older than an hour is read again in the request, with the token of the person,
  never with the token of an installation (`specs/SECURITY.md`).
- **GH-32** The `installation_id` of the Setup URL proves nothing, since anyone can write it: it only chooses the
  page to open after the sign-in.
- **GH-33** The sign-in deletes no membership, because `/user/installations` lists only the installations that the
  person reaches through a repository, and GitHub can refuse the read of a membership (single sign-on, allow list
  of addresses). The hourly `sync_members` decides who left.
- **GH-34** When a role is read again in a request, a `404` or a membership that is not `active` deletes the
  membership; a `403` is an error, so the request answers `503` and the membership stays.
- **GH-35** A member gives its avatar, never its name: the name changes only at a sign-in.
- **GH-36** The calls of the members are written from the documentation of GitHub: the contract suite reads them on
  the simulator only, until a recording holds them.

## Onboarding

Code: `crates/platform/src/installations.rs` (`run_sync_installation`), `crates/platform/src/connections.rs`
(`run_sync_repos`)

- **GH-37** Installing the App is enough. GitHub is the source of truth of the installations; a webhook is only a
  signal to read them again.
- **GH-38** An installation that CIHub does not know becomes an organization whose slug is the login of the
  account. When another organization has this slug, the sync fails and creates nothing.
- **GH-39** A new installation on an account whose connection exists is the App installed again: the connection
  keeps its organization and follows the new installation.
- **GH-40** A suspended or deleted installation, or a repository that its connection no longer lists, gives
  `access.lost` to its queues, which stop and keep their state; the return gives `access.restored`. The end of a
  suspension restores no queue of a repository that the connection no longer lists.
- **GH-41** A repository belongs to each connection that lists it. When it moves to another installation, it gets a
  new row in the organization of that installation; the old row and its queues keep their history.
