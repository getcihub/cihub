# Security and multi-tenancy

This spec gives the security rules of CIHub: how tenants stay apart, who is authenticated and how, what each
grant reaches, how secrets are kept. The routes and the scopes are in `schema/openapi/v1.json` and `specs/API.md`;
the row level security is in `specs/DATA_MODEL.md`; the values are the constants that each rule names.

## Tenancy layers

Code: `migrations/20261026100000_create_tenancy_functions.up.sql`, `crates/web/src/api.rs`

- **SEC-1** Tenancy holds on three layers: row level security in Postgres (`specs/DATA_MODEL.md`, DATA-6),
  organization-scoped identifiers at the API, and one installation token per installation, never shared between
  organizations.
- **SEC-2** Nothing of one tenant can be inferred from the data of another, including through response time:
  the reconcilers work per queue and the workers filter every query by `queue_id`.
- **SEC-3** Each instance is self-contained and keeps its data where it runs; a host that serves several regions
  deploys several instances, without replication between them.

## Authentication

Code: `crates/web/src/sign_in.rs` (`SESSION_LIFETIME`), `crates/web/src/session.rs` (`CLI_SESSION_IDLE_LIFETIME`),
`crates/web/src/device.rs` (`DEVICE_CODE_LIFETIME`), `crates/domain/src/token.rs`

- **SEC-4** A person signs in through the user authorization of the GitHub App, with PKCE (`specs/GITHUB.md`). A
  sign-in in progress keeps its `state`, its verifier and the next page in the cookie `__Host-cihub_sign_in`
  (`HttpOnly`, 10 minutes). The next page is a path of the instance, never another site.
- **SEC-5** The Setup URL of the App only starts a sign-in again: its `installation_id` proves nothing and gives no
  role; only the installations that the token of the person lists count.
- **SEC-6** Every secret that CIHub gives (`cht_` API token, `chs_` dashboard session, `chc_` command-line
  session, `chd_` device code) is a prefix and 20 random bytes in base32, shown once; Postgres keeps only its
  SHA-256 fingerprint.
- **SEC-7** A dashboard session lives in the cookie `__Host-cihub_session` (`HttpOnly`, `Secure`, `SameSite=Lax`,
  `Path=/`, no `Domain`), so that another subdomain cannot plant one; a request that carries it twice is refused.
  It lasts `SESSION_LIFETIME`; the sign-out deletes it.
- **SEC-8** A dashboard session is accepted only in its cookie, and a command-line session only in
  `Authorization: Bearer`, so that one never stands for the other. Neither is accepted as an API token.
- **SEC-9** A command-line session ends `CLI_SESSION_IDLE_LIFETIME` after its last use; a use pushes `expires_at`
  back at most once an hour, so that a read does not write each time.
- **SEC-10** A device code is two secrets: `device_code`, which only the command line knows, and `user_code`,
  8 letters out of 20 (about 34 bits), which the person types. Both expire after `DEVICE_CODE_LIFETIME` and serve
  once.
- **SEC-11** The address that the command line shows never carries the user code: the person types it on
  `/device`, then approves with a dashboard session. A phishing then needs more than one click, and a guessed code
  only gives its session to the guesser.
- **SEC-12** Every request of a dashboard session but `GET`, `HEAD` and `OPTIONS` carries `X-CSRF-Token`: the
  hexadecimal SHA-256 of a label and the session secret, which the server keeps nowhere and gives to the scripts
  in the cookie `__Host-cihub_csrf`.
- **SEC-13** The pages of the dashboard carry `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` and
  `Content-Security-Policy: frame-ancestors 'none'`, because no other site may frame a page whose buttons act.

## Authorization

Code: `crates/web/src/api.rs` (`by_session`), `crates/platform/src/persons.rs` (`ACCESS_LIFETIME`)

- **SEC-14** The roles come from the SCM membership of the connection (owners are `admin`, members `member`),
  refreshed every hour. The role in the organization of the URL gives the scopes of its routes; an organization
  where the user has no role answers `404`.
- **SEC-15** A person reads only the repositories that the SCM shows them. On a route of a repository, the
  permission of the person gives the scopes (`read`: `queues:read`; `write`: `prs:write` too; `admin`:
  `queues:write` too); a repository without `read`, or outside the organization, answers `404 repo_not_found`.
- **SEC-16** The middleware decides from the path parameters that the router matched and decoded, the ones that
  the handler reads, never from the raw path (`/repos/s%65cret` is `secret`).
- **SEC-17** A role or a permission older than `ACCESS_LIFETIME` is checked again in the request, on the SCM, with
  the token of the person, never with an installation token. Without an answer, `503 scm_unavailable`; a refused
  token ends every session of the person, `401`.
- **SEC-18** An API token belongs to one organization and reads every repository of it, within its scopes.
- **SEC-19** A command in a PR comment, or a submission from a session, is honored only for a user with `write`
  on the repository; the job checks it. A check of the submission box by a bot is ignored
  (`crates/github/src/translate.rs`), so that CIHub does not answer its own comment.
- **SEC-20** A stream of an organization checks its grant when it opens, then ends after `STREAM_LIFETIME`
  (`crates/web/src/stream.rs`), so that a revoked token, an ended session or a removed member stops hearing the
  changes within that time.

## Secrets and sealing

Code: `crates/store/src/cipher.rs`, `crates/platform/src/persons.rs` (`TOKEN_MARGIN`)

- **SEC-21** The secrets of the App and of the database come from the environment or the configuration file and
  are read at startup into a `SecretString`. Installation tokens are cached in memory only.
- **SEC-22** A configuration file that carries a secret key (`database.url`, `database.secret`, `github.*_secret`,
  `github.private_key`) is a secret: a Kubernetes Secret rather than a ConfigMap, or mode 0600.
- **SEC-23** The token and the refresh token of a person are kept on `users`, sealed with AES-256-GCM under a key
  that HKDF-SHA256 derives from `database.secret`, each bound to its row and column, so that a copy of the database
  alone opens none of them.
- **SEC-24** CIHub refreshes a person token that ends within `TOKEN_MARGIN`, one refresh at a time per person,
  because a refresh token works once.
- **SEC-25** CIHub forgets the token of a person at the sign-out of their last session, at the maintenance once
  each of their sessions has expired, and, with their permissions, when the SCM refuses it.
- **SEC-26** The token of a person only reads what the person reaches; CIHub never acts on the SCM with it.
- **SEC-27** The permissions outlive the sign-out, so that the next sign-in shows the repositories at once; the
  maintenance deletes them after `PERMISSION_RETENTION` (`specs/DATA_MODEL.md`, DATA-26).

## Abuse limits

Code: `crates/web/src/api.rs` (`REQUESTS_PER_MINUTE`, `MAX_BODY_BYTES`, `SIGN_IN_REQUESTS_PER_MINUTE`,
`SIGN_IN_MAX_BODY_BYTES`, `client_address`), `crates/web/src/stream.rs` (`STREAMS_PER_CALLER`)

- **SEC-28** Each token or session has a rate limit per minute, and each request body a cap; a limit holds in each
  replica, and a refused request gets `429` with `Retry-After`.
- **SEC-29** The two routes without a token, `POST /device-codes` and `POST /device-codes/token`, have their own
  lower limits per client address, and a command line that polls sooner than `interval` gets
  `429 polling_too_fast`.
- **SEC-30** The client address is the address of the connection, or, with `http.trusted_proxy_hops = N`, the
  `N`-th address from the right of `X-Forwarded-For`, which the nearest trusted proxy appended. A bad header gives
  the address of the connection, never a value that the client chooses.
- **SEC-31** A token or a person keeps at most `STREAMS_PER_CALLER` streams open; the next one answers `429`.

## Network surface

Code: `crates/server/src/health.rs`, `crates/web/src/lib.rs`

- **SEC-32** The internal address (`internal.addr`) serves only `/healthz` and `/readyz`, which give the name and
  the version of the binary. It is separate from the public routes and must not be exposed on the Internet.
- **SEC-33** A webhook is verified with HMAC-SHA256 before it is parsed.
- **SEC-34** CIHub serves plain HTTP: the operator ends TLS in the proxy in front of it. The cookies are `Secure`,
  so the dashboard works only over HTTPS.

## Audit

Code: `migrations/20261026100900_create_audit_log.up.sql`, `crates/mergequeue/src/api.rs` (`audit`),
`crates/platform/src/tokens.rs`

- **SEC-35** Each action of a person or a token through the API that changes a queue, a PR or a sync, and each
  token creation, writes one row of `audit_log` with its actor, its target and its `after` value, in the
  transaction of the action.
- **SEC-36** Reading the audit log needs `queues:write`: an `admin` of the organization, or a token with that
  scope.
