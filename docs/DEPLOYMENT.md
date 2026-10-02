# Deployment and infrastructure

## Installation target

The reference installation is one image, one Postgres 17 and one GitHub App. The image contains the
`cihub-server` binary, which by default runs the four roles (`web`, `scheduler`, `worker`, `notifier`) in one
process; `CIHUB_ROLES` selects a subset of them to give each role its own processes
(`docs/ARCHITECTURE.md`, "Scaling"). The GitHub App is created by hand, according to `docs/GITHUB.md`
(`DECISIONS.md` D92). The Helm chart arrives in Lot 15.

## Development

`compose.yaml`, at the root, starts only Postgres (`postgres:17.5-alpine`), published on `127.0.0.1:5432`
(`DECISIONS.md` D12). The superuser is `postgres` (password `postgres`). The one-shot service
`postgres-setup` brings the `cihub` role (password `cihub`), which is not a superuser and can create
databases for the tests, and the `cihub` database it owns, to the desired state (D38, D52). It runs at each start:
a change to its command applies without erasing the data. These values are for development only.

```sh
mise run db            # Postgres, then postgres-setup
mise run db-reset      # erases the data, then mise run db (after a migration is rewritten)
mise run migrate       # Postgres, then cihub-server migrate
mise run dev           # migrate, then cihub-server with the four roles and pretty logs
docker compose down    # stops Postgres and keeps the volume
```

`mise run dev` requires a test GitHub App, created according to `docs/GITHUB.md` (D92). Its variables go in
`mise.local.toml`, at the root of the repository: `mise` loads it with `mise.toml`, and `.gitignore` excludes it.
The private key stays outside the repository, and the file gives only its path.

```toml
# mise.local.toml: never committed
[env]
CIHUB_GITHUB_APP_ID = "123456"
CIHUB_GITHUB_PRIVATE_KEY_FILE = "/Users/<you>/.config/cihub/cihub-test.private-key.pem"
CIHUB_GITHUB_WEBHOOK_SECRET = "<the webhook secret of the App>"
```

The first time, `mise trust mise.local.toml` authorizes the file. To receive webhooks locally, the webhook URL
of the App points to a tunnel that leads to port 8080.

`mise tasks` lists the project tasks and their description.

## Server variables

`cihub-server` reads its configuration from `CIHUB_*` variables (`DECISIONS.md` D31). An invalid value stops the
process at startup.

| Variable              | Default                             | Role                                                                        |
| --------------------- | ----------------------------------- | --------------------------------------------------------------------------- |
| `CIHUB_ROLES`         | `web,scheduler,worker,notifier`     | Roles of the process, separated by commas (`DECISIONS.md` D8)                  |
| `CIHUB_DATABASE_URL`  | none, required                      | `postgres://user:password@host/database`, secret; the user is neither a superuser nor `BYPASSRLS` |
| `CIHUB_INTERNAL_ADDR` | `127.0.0.1:8081`                    | Address of `/healthz` and `/readyz`, not to be exposed (`docs/SECURITY.md`) |
| `CIHUB_HTTP_ADDR`     | `0.0.0.0:8080`                      | Public address of the `web` role: webhooks on `POST /webhooks/github`, then the API (`DECISIONS.md` D82, D83) |
| `CIHUB_LOG`           | `info`                              | Log filter (`docs/OBSERVABILITY.md`)                                        |
| `CIHUB_LOG_FORMAT`    | `json`                              | `json` or `pretty` (`docs/OBSERVABILITY.md`)                                |
| `CIHUB_GITHUB_URL`    | `https://github.com`                | github.com or the address of a GitHub Enterprise Server (`DECISIONS.md` D79)   |
| `CIHUB_GITHUB_APP_ID` | none, required for the server       | Identifier of the GitHub App; `cihub-server migrate` does not need it (`DECISIONS.md` D92) |
| `CIHUB_GITHUB_PRIVATE_KEY` | none                           | PEM private key of the App, secret; required with `CIHUB_GITHUB_APP_ID`, or `CIHUB_GITHUB_PRIVATE_KEY_FILE` |
| `CIHUB_GITHUB_PRIVATE_KEY_FILE` | none                      | File that contains the private key, instead of `CIHUB_GITHUB_PRIVATE_KEY` |
| `CIHUB_GITHUB_WEBHOOK_SECRET` | none                        | Secret that signs the webhooks of the App; required with `CIHUB_GITHUB_APP_ID` |

In a container, `CIHUB_INTERNAL_ADDR` must listen on `0.0.0.0` so that the probes reach it. `/healthz`
answers `200` as long as the process runs; `/readyz` answers `503` when Postgres does not answer within 2 s.

## Image

`Dockerfile`, at the root, builds the `cihub-server` image in two stages: compilation on
`rust:1.98.1-slim-bookworm` (the Rust of `mise.toml`, with the glibc of Debian 12), then execution on
`gcr.io/distroless/cc-debian12:nonroot`. Both images are pinned by their digest, with the tag as a
comment. The binary is `/usr/local/bin/cihub-server`, the project license is in `/usr/share/doc/cihub/`.
The image listens on `0.0.0.0:8080` (public address) and sets `CIHUB_INTERNAL_ADDR` to `0.0.0.0:8081`, so that the
probes reach `/healthz` and `/readyz`. On Linux, the TLS client reads the root certificates of the system, which
the runtime image provides: the binary does not embed a list of certificates (`DECISIONS.md` D81). The
`release` profile of `Cargo.toml` (`lto = "thin"`, `codegen-units = 1`) is used for this image.

`.github/workflows/image.yaml` publishes the image to `ghcr.io/getcihub/cihub-server` at each release
(`DECISIONS.md` D19), with the tags `X.Y.Z`, `X.Y` and `latest`, and on demand, with the tag `sha-<commit>`. One job
per architecture (`linux/amd64` on `ubuntu-26.04`, `linux/arm64` on `ubuntu-26.04-arm`) builds the image on a
native runner and pushes it by its digest; a last job joins the two digests in a manifest.

## Installation with Docker Compose

`deploy/compose.yaml` is the installation `docker compose`. It starts four services:

| Service          | Role                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------ |
| `postgres`       | Postgres 17, with a data volume; no port published on the host                             |
| `postgres-setup` | One-shot: brings the `cihub` role (neither superuser nor `BYPASSRLS`) and the `cihub` database to the desired state, at each start (D38, D52) |
| `cihub-migrate`  | One-shot: `cihub-server migrate` (D14)                                                     |
| `cihub-server`   | The four roles; starts after `cihub-migrate` ends without error; publishes port 8080       |

The values come from `deploy/.env`, to be created from `deploy/.env.example`: the Postgres passwords,
`CIHUB_GITHUB_APP_ID`, `CIHUB_GITHUB_WEBHOOK_SECRET` and the path of the private key of the App. The key is mounted
read-only and given by `CIHUB_GITHUB_PRIVATE_KEY_FILE`; the user of the image (uid 65532) must be able to
read it. `deploy/.env` must never be committed (`.gitignore` excludes it). The internal port 8081 is not published on
the host (`docs/SECURITY.md`). The complete procedure is in `README.md`.

```sh
docker compose -f deploy/compose.yaml up -d
docker compose -f deploy/compose.yaml run --rm cihub-server admin bootstrap --installation <id>
```

## Bootstrap an organization

After the App is installed on an account, a command links the installation to a new organization, without
the dashboard (`DECISIONS.md` D154, D155):

```sh
cihub-server admin bootstrap --installation <identifier of the installation>
```

It requires the variables of the server and of the GitHub App, and an up-to-date database. It creates the
organization (its name is the login of the account), enables the merge queue, requests the read of the
repositories of the installation and shows once an API token of the organization with all the scopes. The
identifier of the installation is at the end of the URL of the installation page (*Settings → GitHub Apps →
Configure*). Run again on an installation already bootstrapped, the command creates only a new token: it also
serves to recover an organization without a token.

## Migrations

Migrations are explicit (`DECISIONS.md` D14):

- `cihub-server migrate` applies the embedded migrations that are missing, then stops.
- At startup, `cihub-server` applies nothing: it stops if an embedded migration is missing in the database. A
  migration in the database that the binary does not know comes from a newer version and does not prevent startup.
- A published migration stays backward compatible for one version, so that old and new processes
  coexist during a deployment.

| Environment                     | What runs `cihub-server migrate`                       | When                                     |
| ------------------------------- | ------------------------------------------------------ | ---------------------------------------- |
| Development                     | `mise run migrate`                                     | By hand                                  |
| Installation `docker compose`   | `cihub-migrate` service, on which `cihub-server` depends | Before `cihub-server`                  |
| Kubernetes                      | `Job` as a Helm `pre-install` and `pre-upgrade` hook   | Before the deployment of the new pods    |

## Tools

`mise install` installs the whole suite (`DECISIONS.md` D17): Rust (version pinned in `mise.toml`, with `clippy`,
`rustfmt` and `llvm-tools`, through `rustup`), `cargo-deny`, `cargo-nextest`, `cargo-llvm-cov`, `actionlint`, `zizmor`
and `lefthook`, then installs the git hooks of `.lefthook.toml` (D166): `cargo fmt`, `actionlint` and `zizmor` at
commit, `mise run check` at push; `LEFTHOOK=0 git push` skips them. `mise.lock` pins the versions and the
checksums for Linux and macOS, on x64 and arm64; after a version change, `mise lock` updates it.

## Project CI

`.github/workflows/ci.yml` follows the organization of `home-operations/external-dns-unifi-webhook` (`DECISIONS.md` D18):

| Job             | Task                      |
| --------------- | ------------------------- |
| `Rust Lint`     | `mise run lint`           |
| `Rust Test`     | `mise run test`           |
| `Rust Coverage` | `mise run cover`          |
| `Workflow Lint` | `mise run lint-workflows` |
| `Build Success` | joins the jobs above      |

`Build Success` is the required status of the `main` branch: it fails if a job failed or was cancelled. The CI
runs on pushes to `main` (except a Markdown-only change), PRs to `main`, `merge_group` and on
demand. Each job installs the tools with `mise install --locked`. Each action is pinned by the SHA of its
commit, with the version as a comment. `.github/actionlint.yaml` makes `actionlint` ignore the label
`ubuntu-26.04`, which it does not know yet. A Postgres service is added in Lot 2, for
`mise run test-integration`.

## Versions

release-please (`.github/workflows/release-please.yaml`, `DECISIONS.md` D19) maintains a release PR from the
commit titles in Conventional Commits format. When this PR is merged, it creates the tag `vX.Y.Z` and the release,
and updates `CHANGELOG.md`, `[workspace.package].version` in `Cargo.toml` and the versions of the `cihub-*` crates
in `Cargo.lock`. The CI does not run on the release-please PRs, which change only these files. The workflow
takes a token of the bot GitHub App of the project (secrets `BOT_CLIENT_ID` and `BOT_APP_PRIVATE_KEY`). The release
triggers the publication of the image ("Image").

## Dependency updates

Renovate (`.github/workflows/renovate.yaml`, `DECISIONS.md` D21) runs every hour, on a push that modifies
`.renovaterc.json5`, and on demand (with a dry-run option). It opens PRs for the GitHub actions,
the tools of `mise.toml` (with `mise.lock`) and the crates of `Cargo.toml`, and maintains a dashboard in an
issue. No PR is merged automatically. It uses the same bot GitHub App as release-please.
