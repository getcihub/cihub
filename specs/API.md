# APIs

This spec gives the rules of the public REST API under `/api/v1` and of the `cihub` command line that uses it. It
does not list the routes, the fields or the error codes: the OpenAPI document (`schema/openapi/v1.json`, served
without a token on `/api/v1/openapi.json`) and the types of `crates/api-types/src/lib.rs` are the reference.

## Authentication

Code: `crates/web/src/api.rs`, `crates/web/src/session.rs` (`SESSION_COOKIE`, `CSRF_COOKIE`),
`crates/domain/src/tenancy.rs` (`ApiScope`)

- **API-1** A caller is an API token of an organization, with its scopes (`queues:read`, `queues:write`,
  `prs:write`), or a session of a person: the cookie of the dashboard, or the session of the command line in
  `Authorization: Bearer`. The command line uses the same API as any client.
- **API-2** The role of a person in the organization of the URL gives the scopes of a session (`member`:
  `queues:read` and `prs:write`; `admin`: every scope). On a route of a repository, the permission of the person on
  it gives them instead (`specs/SECURITY.md`).
- **API-3** A token or a session that does not belong to the organization of the URL answers `404 org_not_found`,
  never `403`, and a repository that the person cannot read answers `404 repo_not_found`, because a caller must
  not learn that another organization or repository exists.
- **API-4** The check of access reads the decoded path parameters that the handler reads, never the raw path
  (`/repos/s%65cret` is `secret`).
- **API-5** A request of a cookie session that can change a state (any method but `GET`, `HEAD` and `OPTIONS`)
  carries `X-CSRF-Token` with the value of the CSRF cookie; otherwise `403 csrf_refused`, a code apart from
  `scope_missing`. A token and a session of the command line need no CSRF token.
- **API-6** A token stands for no person: the `/user` routes answer it `401`.
- **API-7** The routes that sign a person in are outside `/api/v1`, under `/auth/` (`specs/GITHUB.md`). The
  `next` parameter of the sign-in accepts a path of the instance only, so that the sign-in is not an open redirect.

## URLs

Code: `crates/mergequeue/src/api.rs`, `crates/client/src/lib.rs` (`RepoPath`, `QueuePath`)

- **API-8** A URL names the organization by its slug, the repository by its name, the queue by its target branch
  (percent-encoded, `release%2F2.0`) and a PR by its number, never by an identifier, so that a link or two
  browser tabs never depend on a hidden active organization. Names do not tell the case apart, as the SCM.
- **API-9** Everything that belongs to a repository is under it. No route names a queue without its repository,
  because a branch names a queue only in its repository (`acme/web` and `acme/api` each have `main`).
- **API-10** A route of the organization (`/orgs/{org}/queues`, `/orgs/{org}/prs`) only searches across its
  repositories, and takes no filter that a route of a repository gives.
- **API-11** An operation that the methods of HTTP do not name is a `POST` on a segment that names it, under the
  resource that it acts on (`…/sync`, `…/prs/{number}/cancel`). The segment never stands where an identifier
  can: `/orgs/{org}/repos/sync` would hide a repository named `sync`.
- **API-12** A cancellation is not a `DELETE`: the submission stays, `cancelled`, and its PR still reads it.
- **API-13** No route creates a queue: the first `/cihub merge` on a branch does (`specs/CONFIGURATION.md`).
- **API-14** A submission, a cancellation, a restart, a state request and a sync answer `202` without a body: the
  queue applies them at its next reconciliation. A PR already in a queue answers `409 pr_already_queued` at once;
  a refusal that needs a read of the SCM comes later, on the PR.
- **API-15** A read of a queue, a setup or a PR calls the SCM zero times: every field comes from Postgres.
- **API-16** The words of the SCM reads are fixed: a *sync* copies an object of the SCM into CIHub; a *setup* is
  what CIHub found about a repository (the rules of its default branch and its configuration file) with a
  verdict; a *check* is only a check of the CI. No route says "check" or "readiness".

## Resources and lists

Code: `crates/platform/src/page.rs` (`Page`, `cursor_of`)

- **API-17** Every list gives a page: `limit` (1 to 100, 50 by default) and `from`, the `cursor` of the previous
  page. The answer names its entries after the resource (`prs`, `queues`) and adds `has_more`, `count` (every
  page together, with the filters) and `cursor` (`null` on the last page).
- **API-18** A cursor is opaque to a client. It holds the name of the list and the sort key of the last entry, so
  that it stays valid when that entry changes, and a cursor of another list answers `400 invalid_cursor`.
- **API-19** A single resource is the object itself, without an envelope, with the same form in a list and alone.
  The one exception is a repository: its list gives the verdict of each setup (`Repo`), and the repository alone
  its whole setup (`RepoDetail`), because its file can weigh 64 KiB.
- **API-20** A client shows the sentence of a reason (`state_description`, `description`) and decides on its code
  (`state_reason`): the sentence can change without notice (principle VI).
- **API-21** The stream of an organization (`GET /orgs/{org}/stream`) only says that a queue changed, with the
  last sequence number of its log; the client reads the pages again. When the stream ends, the client opens it
  again and reads everything, so that a lost message is a delay and never a wrong page.

## Errors

Code: `crates/api-types/src/lib.rs` (`ErrorCode`, `ErrorCode::status`), `crates/web/src/api.rs`
(`REQUESTS_PER_MINUTE`)

- **API-22** An error is `application/problem+json` with a machine `code`; its `type` is
  `https://cihub.io/docs/api/errors#<code>`. A client decides on `code`, never on `detail` (principle VI).
- **API-23** The codes are closed. Each code names one cause and has one status.
- **API-24** The rate limit counts per token or session, and per client address on the routes without a token, in
  each replica of the server; a `429` carries `Retry-After`.

## OpenAPI document

Code: `crates/web/src/openapi.md`, `crates/platform/src/api.rs` (`error_responses`, `error_examples`),
`crates/server/tests/openapi.rs`

- **API-25** The OpenAPI 3.1 document is generated from the code. The TypeScript client of the dashboard is
  generated from `schema/openapi/v1.json` (`mise run console:generate-check` fails when it is old); the Rust
  client `cihub-client` shares the types of `cihub-api-types` directly.
- **API-26** A route declares every error that its handler can answer. `cihub-web` adds the errors of every route
  of its kind: `401`, `429` and `500` everywhere, `404 org_not_found` under `/orgs/{org}`, `403 scope_missing` with
  a scope, `403 csrf_refused` on a method that changes a state. The tests check the examples of the declared
  errors, not that a handler answers only those: a change that adds an error to a handler adds it to the route.
- **API-27** The example of an error code is written once, in `components.examples`, and every response refers
  to it.

## Command line

Code: `crates/cli/src/auth.rs`, `crates/cli/src/queue.rs` (the commands and their options),
`crates/cli/src/credentials.rs`

- **API-28** The `cihub` binary talks only to the public API, through `cihub-client`, and holds no server code. The
  `cihub-server` binary runs the server and talks to Postgres (`specs/DEPLOYMENT.md`).
- **API-29** The command line has one group per module (`cihub queue …`), apart from the binary of the server,
  no implicit verb, no shortcut at the top level, and no variable named after an SCM (`CIHUB_HOST`,
  `CIHUB_TOKEN`). CIHub takes the structure of other merge queue command lines, never their interface.
- **API-30** `--repo <owner/name>` falls back to the `origin` remote of git. The owner is the slug of the
  organization.
- **API-31** A command that cannot choose one queue or one host without doubt fails, lists the choices and does
  nothing; it never guesses.

## Sign-in of the command line

Code: `crates/web/src/device.rs`, `crates/web/src/session.rs` (`CLI_SESSION_IDLE_LIFETIME`)

- **API-32** The sign-in follows the device authorization grant (RFC 8628), on top of the sign-in of the
  dashboard.
- **API-33** The person types the code on the page `/device` of the dashboard; the address that the command line
  shows never carries the code, so that a link sent by someone else cannot sign their command line in as the
  person with one click.
- **API-34** A code uses letters without vowels, so that no code spells a word.
- **API-35** A session of the command line covers every organization of its person and acts as that person: a
  submission carries their account, and the job checks their permission on the repository.

## Output of the command line

Code: `crates/cli/src/output.rs`, `crates/cli/src/queue.rs`

- **API-36** In a terminal, a command that reads writes a table with a header, colors and relative durations. In a
  pipe, it writes the rows alone, with tabs between the columns, without colors and with RFC 3339 times.
  `NO_COLOR` turns the colors off.
- **API-37** `--json` writes what the API gives, without another form; `watch --json` writes one line per change.
- **API-38** A reason shows as its sentence, never as its JSON.
- **API-39** `cihub queue watch <pr>` ends with the PR: exit code 0 for `merged`, 1 with the state and its reason
  for `failed` or `cancelled`, so that a script can wait on it.
- **API-40** A watch reads again on each change of its queue on the stream and each time the stream opens again;
  when the server refuses one more stream, it reads at a fixed interval.
