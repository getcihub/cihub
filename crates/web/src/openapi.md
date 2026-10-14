The API of a CIHub instance: read the queues, the PRs in them and their runs; submit, cancel and restart a PR; pause, drain and resume a queue. The dashboard and the `cihub` command line use it too. The API is at `/api/v1` on the address of the instance, next to the dashboard.

## Authentication

Send an API token of the organization in the header `Authorization: Bearer <token>`. `cihub-server token create --org <slug>` creates the first one. A token has scopes:

| Scope          | Allows                                                     |
| -------------- | ---------------------------------------------------------- |
| `queues:read`  | Read the queues, their PRs and their runs                  |
| `queues:write` | Pause, drain, resume and read again a queue; read the audit log |
| `prs:write`    | Submit, cancel and restart a PR                            |

The dashboard uses the session of a person instead: the cookie `__Host-cihub_session`. The role of the person in the organization of the URL gives the scopes: `admin` has every scope, `member` has `queues:read` and `prs:write`. A request of a session that can change a state (any method but `GET`, `HEAD` and `OPTIONS`) also sends the header `X-CSRF-Token`, with the value of the cookie `__Host-cihub_csrf`.

The `cihub` command line acts as a person too: `cihub auth login` asks `POST /device-codes` for a code, the person approves it on the page `/device` of the dashboard, and `POST /device-codes/token` then gives a session of the command line, `chc_…`, which goes in `Authorization: Bearer` as a token, with the scopes of the role of the person and no CSRF token.

## Names in the URL

A route names the organization by its slug, then the repository and the queue by their names: `/orgs/acme/repos/web/queues/main`. A PR belongs to its repository: `/orgs/acme/repos/web/prs/118`; its base branch gives its queue. The case of the slug and of the repository does not matter. An organization that the token or the person does not belong to answers `404 org_not_found`, as one that does not exist.

## Requests that the queue does later

A submission, a cancellation, a restart, a change of the state of a queue and a new read of a queue answer `202 Accepted`, without a body: the queue applies them at its next reconciliation, within seconds. `GET /orgs/{org}/stream` tells when a queue changed.

## Pages

A list gives a page. `limit` asks for 1 to 100 entries (50 by default), and `from` takes the `cursor` of the previous page. The answer names its entries after the resource (`prs`, `queues`, …) and adds:

| Field      | Meaning                                                              |
| ---------- | -------------------------------------------------------------------- |
| `has_more` | More entries follow this page                                        |
| `count`    | How many entries the list has, every page together                   |
| `cursor`   | The `from` of the next page; `null` on the last page. It is opaque: pass it back as it is |

A cursor of another list, or one that does not read, answers `400 invalid_cursor`.

## Rate limits

A token or a session makes at most 600 requests per minute on each replica of the server; past it, the answer is `429 rate_limited` with the header `Retry-After`. A token or a person keeps at most 8 streams open.

## Errors

An error is a `application/problem+json` document (RFC 9457). Decide on its `code`, never on `detail`, whose text can change:

```json
{
  "type": "about:blank",
  "title": "Not Found",
  "status": 404,
  "detail": "queue `main` not found",
  "code": "queue_not_found"
}
```

Each route lists the codes that it can answer. Every code:
