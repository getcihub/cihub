# Deployment and infrastructure

This spec gives the rules of how CIHub is configured, packaged, installed, tested and released. The values are in
the files that it points to: the keys in `crates/server/src/config.rs`, the image in `Dockerfile`, the chart in
`helm/cihub/`, the tasks in `mise.toml`, the CI in `.github/workflows/`.

## Installation target

- **DEP-1** The reference installation is one image, one Postgres 17 and one GitHub App. `cihub-server start` runs
  the four roles (`web`, `scheduler`, `worker`, `notifier`) in one process, and a deployment scales by identical
  replicas (`specs/ARCHITECTURE.md`).
- **DEP-2** The GitHub App is created by hand (`specs/GITHUB.md`) and stays private (*Only on this account*),
  because any account that installs a public App gets an organization.
- **DEP-3** `compose.yaml` is for development only: its credentials are fixed and public. Production installs with
  the Helm chart. The local settings of a developer, such as the test App, go in `mise.local.toml`, which git
  ignores; a private key stays outside the repository and the file gives only its path.
- **DEP-33** `mise run dev` runs the code on the machine, not in an image, so that a change applies in seconds:
  Vite reloads the console in place, and `watchexec` applies the migrations and starts `cihub-server` again at
  each change of the Rust code. The tunnel of the profile `tunnel` then reaches Vite, which sends `/api`, `/auth`
  and `/webhooks` to the server, so that the sign-in and the console share one address.

## Configuration rules

Code: `crates/server/src/config.rs` (`KEYS`, the `///` comments of `Config`)

- **DEP-4** `cihub-server` reads a YAML file only when `--config <file>` names it, so that a file forgotten in
  `/etc` never changes the behavior. The documentation names it `server.yaml`, never `cihub.yaml`, which would
  look like `.cihub.yaml`, the file of a repository (`specs/CONFIGURATION.md`).
- **DEP-5** The variable of a key is `CIHUB_` followed by its path in capitals, with `_` as separator. A variable
  wins over the file, the file over the default.
- **DEP-6** An unknown key, a wrong type or an invalid value stops the process at startup, with a message that
  names the key and the file or the variable.
- **DEP-7** `cihub-server` without a command shows its help, and `start` refuses an argument, so that a typo never
  starts a server.
- **DEP-8** One file serves every replica. The pools of the replicas share the `max_connections` of Postgres:
  `database.pool_size` times the replicas stays under it.
- **DEP-9** `/healthz` answers `200` while the process runs; `/readyz` answers `503` when Postgres does not answer
  in time. In a container, `internal.addr` listens on `0.0.0.0` so that the probes reach it.

## Image rules

Code: `Dockerfile`, `.github/workflows/image.yaml`

- **DEP-10** The image runs `cihub-server` on a distroless `nonroot` base; its entry point is the binary and its
  default command `start`. It does not contain the `cihub` command line.
- **DEP-11** The build stages use the Node and the Rust of `mise.toml`, on the glibc of the runtime stage; every
  base image is pinned by its tag and its digest, so that Renovate updates both in the PR of the `mise.toml` tool.
- **DEP-12** The TLS client reads the root certificates of the system, which the runtime image provides; the binary
  embeds no list.
- **DEP-13** The dashboard is embedded in the binary at build time (`crates/web/build.rs`); a build without
  `apps/console/dist` serves the API and answers `404` on the pages.
- **DEP-14** The image is built for `linux/amd64` and `linux/arm64`, each on a native runner, then joined in one
  manifest.

## Chart rules

Code: `helm/cihub/templates/`, `helm/cihub/README.md` (generated: the values)

- **DEP-15** The migration runs as a Job with the `pre-install` and `pre-upgrade` hooks, before the new pods. It
  reads only the environment, because the ConfigMap does not exist yet when it runs.
- **DEP-16** The ConfigMap carries `server.yaml` without its secrets; the secrets come from existing Secrets, as
  variables or a mounted file (`specs/SECURITY.md`, SEC-22).
- **DEP-17** The pods mount no ServiceAccount token, because CIHub does not call the Kubernetes API.
- **DEP-18** A proxy in front of the Service must not buffer the stream `/api/v1/orgs/{org}/stream` (CIHub sends
  `X-Accel-Buffering: no`) and must keep it open past the 15 seconds between two pings. It serves HTTP/2 to the
  browser, else each open tab holds one of the 6 connections that a browser opens to a site.
- **DEP-19** `trustedProxyHops` is the number of proxies of the Ingress path; else every client behind them shares
  one limit (`specs/SECURITY.md`, SEC-30).
- **DEP-20** `values.schema.json` refuses a key that the chart does not know, so that a typo fails the install.
- **DEP-21** The smoke test (`scripts/smoke.sh`) installs the chart with two replicas in a kind cluster and calls
  no GitHub; with `CIHUB_SMOKE_GITHUB=1` it uses the development App, as a development use, never in the CI.

## Migrations and rollback

Code: `crates/server/src/migrate.rs`, `crates/store/src/lib.rs` (`missing_migrations`)

- **DEP-22** The rules to write a migration are in `AGENTS.md`, "Migrations".
- **DEP-23** `cihub-server migrate up` applies the missing migrations and creates the partitions. `start` applies
  nothing: it stops if an embedded migration is missing in the database, and starts when the database has a newer
  migration that it does not know.
- **DEP-24** A binary knows only the down files of its own migrations. To go back to an older version whose schema
  differs, the newer image runs `migrate down --to` the newest migration of the older version (the last file of
  `migrations/` in its tag), then the older image is deployed. A rollback without a change of schema needs no down.
- **DEP-25** Version 0.5.0 starts from a new baseline: a database of version 0.4 or older does not upgrade to it.

## CI and release rules

Code: `.github/workflows/ci.yaml`, `.github/workflows/image.yaml`, `.github/workflows/cli.yaml`,
`.github/workflows/release-please.yaml`, `.renovaterc.json5`

- **DEP-26** `mise install` installs every tool, pinned with its checksums in `mise.lock`. `watchexec` is a tool
  of the `server:watch` task alone, which installs it at its first run, so that the jobs of the CI do not download
  it.
- **DEP-27** The CI jobs run in parallel; the last one, `Build Success`, is the required status of `main` and fails
  when another job failed or was cancelled.
- **DEP-28** Every workflow runs on the runners hosted by GitHub, never on a self-hosted runner, because the
  repository is public and a self-hosted runner would run the code of the pull requests of forks.
- **DEP-29** Each job installs its tools with `mise install --locked`, and each action and image is pinned by its
  SHA or its digest.
- **DEP-30** release-please makes the release PR from the Conventional Commits titles; its merge creates the tag,
  which publishes the image, the chart and the `cihub` binaries. The image and the chart are signed with cosign,
  keyless, with the OIDC identity of the workflow.
- **DEP-31** The `cihub` Linux binaries link musl statically, so that they run on any distribution.
- **DEP-32** Renovate opens PRs for the actions, the tools of `mise.toml`, the crates, the images of the
  `Dockerfile` and of the chart; no PR merges automatically.

## Pointers

- Profiles and services of development: `compose.yaml`; the tasks and their descriptions: `mise tasks`.
- Every key, variable and default of `server.yaml`: `crates/server/src/config.rs`.
- Every value of the chart: `helm/cihub/README.md`; the smoke variables: `scripts/smoke.sh`.
- The token command: `cihub-server token create --org <slug>` (`crates/server/src/token.rs`).
