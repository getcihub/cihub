# Deployment and infrastructure

## Installation target

The reference installation is one image, one Postgres 17 and one GitHub App. The image contains the
`cihub-server` binary; `cihub-server start` runs the four roles (`web`, `scheduler`, `worker`,
`notifier`) in one process; a deployment scales by identical replicas (`specs/ARCHITECTURE.md`, "Scaling"). The GitHub App is created by hand, according to `specs/GITHUB.md`. The Helm chart `helm/cihub` installs it on Kubernetes.

## Development

`compose.yaml`, at the root, is for development only: its credentials are fixed and public. Production installs
with the Helm chart (below). The configuration files of its services are in `.docker/`. Without a profile, it
starts only Postgres (`postgres:17.5-alpine`), published on `127.0.0.1:5432`. The superuser is `postgres`
(password `postgres`). The one-shot service `postgres-setup` runs `.docker/postgres-setup.sql`, which brings the
`cihub` role (password `cihub`), which is not a superuser and can create databases for the tests, and the `cihub`
database it owns, to the desired state. It runs at each start: a change to the file applies without erasing the
data.

| Profile         | Services                                                                                                                                                                                                                                  |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| (none)          | `postgres`, `postgres-setup`                                                                                                                                                                                                              |
| `server`        | `cihub-migrate` (one-shot `cihub-server migrate`), then `cihub-server` (the four roles, JSON logs) on `127.0.0.1:8080`; image `cihub-server:local`, built from the `Dockerfile`, or `CIHUB_IMAGE`                                         |

The `server` profile takes the variables of the test App from the environment, which mise fills from
`mise.local.toml` (below), as for `mise run dev`: `mise run server:up` starts it. The private key is a Compose
secret, read from the path of `CIHUB_GITHUB_PRIVATE_KEY_FILE`; the user of the image (uid 65532) must be able to
read it. Compose passes to the container only the variables that `compose.yaml` names: `CIHUB_DATABASE_URL` points
to the `postgres` service, and the logs stay in JSON. Compose reads every variable of the file, whatever the active
profiles: the variables of `server` have a `:-` default, so that Postgres starts without them, and `cihub-server`
stops at startup on a missing value.

```sh
mise run db:start                                  # Postgres, then postgres-setup
mise run db:reset                                  # erases the data, then mise run db:start (after a migration is rewritten)
mise run db:migrate                                # Postgres, then cihub-server migrate
mise run dev                                       # db:migrate, then cihub-server start with the four roles and pretty logs
mise run server:up                                 # cihub-server in a container, from the local Dockerfile
docker compose --profile "*" down                  # stops every service and keeps the volumes
```

`mise run dev` requires a test GitHub App, created according to `specs/GITHUB.md`. Its variables go in
`mise.local.toml`, at the root of the repository: `mise` loads it with `mise.toml`, and `.gitignore` excludes it.
The private key stays outside the repository, and the file gives only its path.

```toml
# mise.local.toml: never committed
[env]
CIHUB_GITHUB_APP_ID = "123456"
CIHUB_GITHUB_PRIVATE_KEY_FILE = "/Users/<you>/.config/cihub/cihub-test.private-key.pem"
CIHUB_GITHUB_WEBHOOK_SECRET = "<the webhook secret of the App>"
# With the sign-in: the client of the App, and a secret of at least 32 bytes
# (`openssl rand -hex 32`) that seals the tokens of the people who sign in.
CIHUB_GITHUB_CLIENT_ID = "Iv23li…"
CIHUB_GITHUB_CLIENT_SECRET = "<the client secret of the App>"
CIHUB_DATABASE_SECRET = "<64 hexadecimal digits>"
```

The first time, `mise trust mise.local.toml` authorizes the file. To receive webhooks locally, the webhook URL
of the App points to a tunnel that leads to port 8080.

`mise tasks` lists the project tasks and their description. A task of one part carries its prefix (`rust:`, `web:`,
`db:`, `server:`, `helm:`, `workflows:`); `fmt`, `lint`, `test` and `generate` run the tasks of the same name of each part, in parallel.

## Server configuration

`cihub-server` reads its configuration from a YAML file named by `--config <file>`, and from the `CIHUB_*`
variables. Without `--config`, it reads no file: a file forgotten in `/etc` never
changes the behavior. The documentation names the file `server.yaml`, never `cihub.yaml`, which would look like
`.cihub.yaml`, the configuration of a repository (`specs/CONFIGURATION.md`).

The variable of a key is `CIHUB_` followed by its path in capitals, with `_` as separator. A variable wins over
the file, the file over the default. An unknown key, a wrong type or an invalid value stops the process at
startup, with a message that names the key and the file, or the variable.

| Key                       | Variable                        | Default              | Role                                                                        |
| ------------------------- | ------------------------------- | -------------------- | --------------------------------------------------------------------------- |
| `database.url`            | `CIHUB_DATABASE_URL`            | none, required       | `postgres://user:password@host/database`, secret; the user is neither a superuser nor `BYPASSRLS` |
| `database.pool_size`      | `CIHUB_DATABASE_POOL_SIZE`      | `10`                 | Largest number of connections of each process to Postgres                   |
| `database.idle_timeout`   | `CIHUB_DATABASE_IDLE_TIMEOUT`   | `600`                | Seconds after which an unused connection is closed                          |
| `database.connection_timeout` | `CIHUB_DATABASE_CONNECTION_TIMEOUT` | `30`         | Seconds that a query waits for a connection of the pool before it fails     |
| `database.secret`         | `CIHUB_DATABASE_SECRET`         | none                 | Secret of at least 32 bytes that seals the tokens of the people who sign in (AES-256-GCM, `specs/SECURITY.md`); required with the sign-in; a lost or changed secret signs every person out |
| `internal.addr`           | `CIHUB_INTERNAL_ADDR`           | `127.0.0.1:8081`     | Address of `/healthz` and `/readyz`, not to be exposed (`specs/SECURITY.md`) |
| `http.addr`               | `CIHUB_HTTP_ADDR`               | `0.0.0.0:8080`       | Public address of the `web` role: webhooks on `POST /webhooks/github`, then the API |
| `http.public_url`         | `CIHUB_HTTP_PUBLIC_URL`         | none                 | Root address where people reach the instance, such as `https://ci.example.com`; the sign-in to the dashboard sends them back there, and without it nobody signs in; the status and the comment of a PR link to its page there |
| `log.filter`              | `CIHUB_LOG_FILTER`              | `info`               | Log filter (`specs/OBSERVABILITY.md`)                                        |
| `log.format`              | `CIHUB_LOG_FORMAT`              | `json`               | `json` or `pretty` (`specs/OBSERVABILITY.md`)                                |
| `github.url`              | `CIHUB_GITHUB_URL`              | `https://github.com` | github.com or the address of a GitHub Enterprise Server |
| `github.app_id`           | `CIHUB_GITHUB_APP_ID`           | none, required for the server | Identifier of the GitHub App; `cihub-server migrate` does not need it |
| `github.private_key`      | `CIHUB_GITHUB_PRIVATE_KEY`      | none                 | PEM private key of the App, secret; with `github.app_id`, this key or `github.private_key_file` is required |
| `github.private_key_file` | `CIHUB_GITHUB_PRIVATE_KEY_FILE` | none                 | File that contains the private key, instead of `github.private_key`         |
| `github.webhook_secret`   | `CIHUB_GITHUB_WEBHOOK_SECRET`   | none                 | Secret that signs the webhooks of the App; required with `github.app_id`   |
| `github.client_id`        | `CIHUB_GITHUB_CLIENT_ID`        | none                 | Client ID of the App, for the sign-in to the dashboard; with `github.client_secret` or not at all |
| `github.client_secret`    | `CIHUB_GITHUB_CLIENT_SECRET`    | none                 | Client secret of the App, secret; without both keys, nobody signs in to the dashboard |

```yaml
# /etc/cihub/server.yaml
database:
  url: postgres://cihub:…@db/cihub
  pool_size: 10
  idle_timeout: 600
  connection_timeout: 30
  secret: …
http:
  addr: 0.0.0.0:8080
internal:
  addr: 127.0.0.1:8081
log:
  filter: info
  format: json
github:
  url: https://github.com
  app_id: 123456
  private_key_file: /run/secrets/app.pem
  webhook_secret: …
  client_id: Iv23li…
  client_secret: …
```

One file serves every replica. On Kubernetes, a ConfigMap carries the
file without its secrets, and a Secret gives `CIHUB_DATABASE_URL`, `CIHUB_DATABASE_SECRET`,
`CIHUB_GITHUB_WEBHOOK_SECRET` and `CIHUB_GITHUB_CLIENT_SECRET`. A file that carries `database.url`, `database.secret`,
`github.private_key`, `github.webhook_secret` or `github.client_secret` is a secret (`specs/SECURITY.md`). The connections to Postgres
are `database.pool_size` for each replica: the sum over the replicas stays under the `max_connections` of Postgres.

```sh
cihub-server migrate --config /etc/cihub/server.yaml
cihub-server start --config /etc/cihub/server.yaml    # the four roles, as many replicas as needed
```

`cihub-server` alone shows its help: a typo never starts a server. `cihub-server start` takes no argument, and
refuses one.

In a container, `CIHUB_INTERNAL_ADDR` must listen on `0.0.0.0` so that the probes reach it. `/healthz`
answers `200` as long as the process runs; `/readyz` answers `503` when Postgres does not answer within 2 s.

## Image

`Dockerfile`, at the root, builds the server image in three stages: the dashboard on
`node:24.21.0-bookworm-slim` (the Node of `mise.toml`, pnpm by corepack from `web/package.json`), compilation on
`rust:1.98.1-slim-bookworm` (the Rust of `mise.toml`, with the glibc of Debian 12), which embeds `web/dist` in the
binary (`crates/web/build.rs`), then execution on `gcr.io/distroless/cc-debian12:nonroot`. The images are pinned
by their digest, with the tag as a comment. A build of the Rust code without `web/dist` serves no dashboard: its
pages answer `404`, and the API works. The binary is `/usr/local/bin/cihub-server`, its entry point; the default command is `start`, so the
image starts the four roles, and an argument replaces it (`migrate`). The image does not contain the `cihub`
command line. The project
license is in `/usr/share/doc/cihub/`.
The image listens on `0.0.0.0:8080` (public address) and sets `CIHUB_INTERNAL_ADDR` to `0.0.0.0:8081`, so that the
probes reach `/healthz` and `/readyz`. On Linux, the TLS client reads the root certificates of the system, which
the runtime image provides: the binary does not embed a list of certificates. The
`release` profile of `Cargo.toml` (`lto = "thin"`, `codegen-units = 1`) is used for this image.

`.github/workflows/image.yaml` publishes the image to `ghcr.io/getcihub/server` at each release, with the tags
`X.Y.Z`, `X.Y` and `latest`, and on demand, with the tag `sha-<commit>`. The image is for `linux/amd64` and
`linux/arm64`: each architecture is built on a native runner hosted by GitHub (`ubuntu-26.04`,
`ubuntu-26.04-arm`), then the two are joined in one manifest. The workflow takes the owner of the image from the
repository.

## Installation on Kubernetes

`helm/cihub` is the Helm chart. It installs:

| Resource                    | Role                                                                                                     |
| --------------------------- | -------------------------------------------------------------------------------------------------------- |
| Job `<release>-migrate`     | `cihub-server migrate`, as a `pre-install` and `pre-upgrade` hook; it reads only the environment, as the ConfigMap does not exist yet when it runs |
| ConfigMap                   | `server.yaml` without secrets; a change rolls the pods                                                    |
| Deployment                  | `cihub-server start --config /etc/cihub/server.yaml`, `replicas` identical replicas               |
| Service                     | The public address (8080) only; the probes reach `/healthz` and `/readyz` on the internal address of the pod (8081) |
| Ingress or HTTPRoute        | Optional, to the Service; it must not buffer the stream `/api/v1/orgs/{org}/stream` (CIHub sends `X-Accel-Buffering: no`) and must keep it open past 15 seconds of silence, the ping; HTTP/2 to the browser, else each open tab holds one of the 6 connections that a browser opens to a site |
| PodDisruptionBudget         | When `replicas` is above 1                                                                               |
| Pod of `helm test`          | Posts an unsigned delivery to the Service and expects `401`                                               |

The secrets come from two existing Secrets: `database.existingSecret` gives `CIHUB_DATABASE_URL` and, when
`github.clientId` is set, `CIHUB_DATABASE_SECRET` (key `database.secretKey`, `secret`), and `github.existingSecret`
gives `CIHUB_GITHUB_WEBHOOK_SECRET`, the private key of the App, mounted as a file, and, when `github.clientId` is
set, `CIHUB_GITHUB_CLIENT_SECRET`; `publicUrl` gives `http.public_url`, and `database.poolSize`,
`database.idleTimeout` and `database.connectionTimeout` give the settings of the pool. The
role of the URL owns the database, as the migration Job uses the same URL. The pods mount no ServiceAccount token:
CIHub does not call the Kubernetes API. `helm/cihub/README.md`, generated, lists the values.

```sh
mise run helm:lint       # helm lint with helm/cihub/ci/lint-values.yaml
mise run helm:unittest   # the tests of helm/cihub/tests (installs the helm-unittest plugin when it is missing)
mise run helm:generate   # helm/cihub/README.md and values.schema.json, from Chart.yaml and values.yaml
mise run helm:smoke      # the chart in a throwaway kind cluster (needs Docker)
```

`mise run helm:smoke` (`scripts/smoke.sh`) builds the image, creates a kind cluster, installs CloudNativePG and a
Postgres 17 whose owner is the `cihub` role, then the chart with two replicas (`scripts/smoke/values.yaml`). It
fails when the migration Job fails, when a pod is not ready or restarts, when `helm test` fails, or when a signed
delivery does not get `202`. The App does not exist and `github.url` does not resolve: the test calls no GitHub.
`CIHUB_SMOKE_IMAGE` gives an image already built, `CIHUB_SMOKE_CLUSTER` the name of the cluster (`cihub-smoke`), and
`CIHUB_SMOKE_KEEP=1` keeps the cluster at the end; a failure shows the pods, the logs and the state of Postgres. The
kubeconfig of the cluster is `$TMPDIR/kind-cihub-smoke.kubeconfig` (`/tmp` without `TMPDIR`), never `~/.kube`. A run
on a kept cluster keeps its Secrets, and restarts the replicas, as the tag of the image does not change.

With `CIHUB_SMOKE_GITHUB=1`, the script uses the development App of `mise.local.toml` instead of a false App, and
also fails when no installation of the App is synced within 2 minutes. It is a development use, as `mise run dev`,
never a test of the CI. It then takes from the environment every variable of the server configuration that the chart
carries: `CIHUB_GITHUB_APP_ID`, `CIHUB_GITHUB_WEBHOOK_SECRET`, `CIHUB_GITHUB_PRIVATE_KEY_FILE` or
`CIHUB_GITHUB_PRIVATE_KEY`, and, when they are set, `CIHUB_GITHUB_URL`, `CIHUB_GITHUB_CLIENT_ID` with
`CIHUB_GITHUB_CLIENT_SECRET` for the sign-in, `CIHUB_HTTP_PUBLIC_URL`, `CIHUB_LOG_FILTER` and `CIHUB_LOG_FORMAT`. The
cluster has its own Postgres and the chart fixes the addresses of the pod: `CIHUB_DATABASE_URL`, `CIHUB_HTTP_ADDR` and
`CIHUB_INTERNAL_ADDR` stay out, and the script draws `CIHUB_DATABASE_SECRET` into the Secret of Postgres. With `CIHUB_SMOKE_KEEP=1`, it ends with the commands that bring the webhooks of the
App to the cluster: a port-forward to the Service, a tunnel to it, and the tunnel as the webhook URL of the App; with
the sign-in, the Callback URL and the Setup URL that the App must have.

```sh
CIHUB_SMOKE_GITHUB=1 CIHUB_SMOKE_KEEP=1 mise run helm:smoke
export KUBECONFIG="$TMPDIR/kind-cihub-smoke.kubeconfig"
kubectl -n cihub get pods
kind delete cluster --name cihub-smoke
```

## Organizations and API tokens

Installing the App on an account is enough: CIHub syncs the installations of its App, at the start of the server,
every 24 hours and at each `installation` delivery, and creates the organization of a new installation (its slug
is the login of the account), its connection and the read of its repositories (`specs/GITHUB.md`). Keep the App private (*Only on this account*): any account that installs a public App gets an
organization.

The sync creates no API token. A command creates one for an existing organization, without the dashboard:

```sh
cihub-server token create --org <slug of the organization>
```

It needs only the database: it creates a token named `token` with every scope, writes `token.created` in the
audit log, and prints the token once on the standard output, and nothing else. Run again, it creates another
token: it also serves to recover an organization without a token. An unknown slug stops the command with an
error.

## Migrations

Migrations are explicit:

- `cihub-server migrate` applies the embedded migrations that are missing, creates the partitions, then stops.
- At startup, `cihub-server start` applies nothing: it stops if an embedded migration is missing in the database. A
  migration in the database that the binary does not know comes from a newer version and does not prevent startup.
- A published migration stays backward compatible for one version, so that old and new processes
  coexist during a deployment.

| Environment                     | What runs `cihub-server migrate`                       | When                                     |
| ------------------------------- | ------------------------------------------------------ | ---------------------------------------- |
| Development                     | `mise run db:migrate`                                  | By hand                                  |
| Development, `server` profile   | `cihub-migrate` service, on which `cihub-server` depends | Before `cihub-server`                  |
| Kubernetes                      | `Job` as a Helm `pre-install` and `pre-upgrade` hook   | Before the deployment of the new pods    |

## Tools

`mise install` installs the whole suite: Rust (version pinned in `mise.toml`, with `clippy`,
`rustfmt` and `llvm-tools`, through `rustup`), `cargo-deny`, `cargo-nextest`, `cargo-llvm-cov`, `actionlint`, `zizmor`,
`lefthook`, the chart tools (`helm`, `helm-docs`, `helm-schema`, `kind`, `kubectl`) and `shellcheck`, then installs the git hooks of `.lefthook.toml`: `cargo fmt`, `actionlint` and `zizmor` at
commit, `mise run lint` at push; `LEFTHOOK=0 git push` skips them. `mise.lock` pins the versions and the
checksums for Linux and macOS, on x64 and arm64; after a version change, `mise lock` updates it.

## Project CI

`.github/workflows/ci.yaml` runs its jobs in parallel: `Rust Lint` (`mise run rust:lint`, then `mise run
web:generate-check`), `Web Lint` (`mise run web:lint`), `Rust Test` (`mise run
rust:test`), `Web Test` (`mise run web:test`), `Rust Integration` (`mise run rust:integration`), `Rust Coverage` (`mise
run rust:cover`), `Workflow Lint` (`mise run workflows:lint`), `Helm Lint` (`mise run helm:lint`, then `mise run
helm:generate-check`), `Helm Unittest` (`mise run helm:unittest`) and `Smoke` (`mise run helm:smoke`). The last job, `Build
Success`, is the required status of the `main` branch: it fails when one of these jobs failed or was cancelled.
The test jobs upload their JUnit report (`test-reports/<suite>/junit.xml`) as the artifact `junit-<suite>`, even
when the tests fail; `Dashboard E2E` adds the traces of the failed tests.

The CI runs on pushes to `main` (except a Markdown-only change), PRs to `main`, `merge_group` and on demand. Each
job installs the tools with `mise install --locked`. Each action and each image is pinned by its SHA or its
digest, with the version as a comment or a tag.

Every workflow runs on the standard runners hosted by GitHub (`ubuntu-26.04`), never on a self-hosted runner: the
repository is public, and a self-hosted runner would run the code of the pull requests of forks. The runner of a
job is new for each run, so `ci.yaml` turns off incremental compilation (`CARGO_INCREMENTAL=0`) and keeps only the
line numbers of the debug information (`CARGO_PROFILE_DEV_DEBUG=line-tables-only`). The Rust jobs cache the
Cargo registry and `target/` with `actions/cache`, under a key per job and per content of `Cargo.lock` and
`mise.lock`; `Rust Test` and `Rust Integration` share their key, as they build the same test binaries. `Rust
Integration` and `Fault Injection` (`sim.yaml`, once a night) start the Postgres of `compose.yaml`, as in
development. `.github/actionlint.yaml` makes `actionlint` ignore the label `ubuntu-26.04`, which it does not know
yet.

## Versions

release-please (`.github/workflows/release-please.yaml`) maintains a release PR from the
commit titles in Conventional Commits format. When this PR is merged, it creates the tag `vX.Y.Z` and the release,
and updates `CHANGELOG.md`, `[workspace.package].version` in `Cargo.toml` and the versions of the `cihub-*` crates
in `Cargo.lock`, and the version and the `appVersion` of `helm/cihub/Chart.yaml`. The jobs of the CI are skipped on the release-please PRs, which change only these files, and `Build Success` passes. The workflow
takes a token of the bot GitHub App of the project (secrets `BOT_CLIENT_ID` and `BOT_APP_PRIVATE_KEY`). The release
triggers the publication of the image ("Image"), then of the chart: `image.yaml` packages `helm/cihub` with the
version of the release and pushes it to `oci://ghcr.io/getcihub/charts/cihub`.

## Dependency updates

Renovate (`.github/workflows/renovate.yaml`) runs on a push that modifies `.renovaterc.json5`, on demand
(with a dry-run option), and when a person edits its dashboard or one of its PRs (a checkbox) or closes one of
its PRs; another issue or PR does not run it. It opens PRs for the GitHub actions, the tools of `mise.toml`
(with `mise.lock`) and the crates of `Cargo.toml`, and maintains a dashboard in an issue. No PR is merged
automatically. TypeScript stays below 7 until `typescript-eslint` supports it. It uses the same bot GitHub App
as release-please.
