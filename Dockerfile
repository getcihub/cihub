# The server image of CIHub, with the cihub-server binary (specs/DEPLOYMENT.md). The build stage uses the
# Rust of mise.toml on Debian 12, the glibc of the runtime stage. On Linux,
# the TLS client reads the root certificates of the system, which the runtime
# image provides: the binary embeds no list of its own.

# The dashboard (apps/console/), built with the Node and the pnpm of mise.toml; the
# build stage embeds apps/console/dist in the binary (crates/web/build.rs).
# node:24.21.0-bookworm-slim
FROM docker.io/library/node@sha256:d6aa754f16b3197301076f047b5def2f02ea1dbbc2ca920407d46d7ec7f87b20 AS console
WORKDIR /src
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml tsconfig.base.json ./
COPY apps/console/package.json apps/console/
COPY packages/ui/package.json packages/ui/
RUN corepack enable && pnpm install --frozen-lockfile --filter @cihub/console...
COPY apps/console/ apps/console/
COPY packages/ui/ packages/ui/
RUN pnpm --dir apps/console build

# rust:1.98.1-slim-bookworm
FROM docker.io/library/rust@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS build
WORKDIR /src
COPY . .
COPY --from=console /src/apps/console/dist apps/console/dist
RUN cargo build --release --locked -p cihub-server

# gcr.io/distroless/cc-debian12:nonroot
FROM gcr.io/distroless/cc-debian12@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f

COPY --from=build /src/target/release/cihub-server /usr/local/bin/cihub-server
COPY --from=build /src/LICENSE /usr/share/doc/cihub/LICENSE

# The probes reach the internal address from outside the container.
ENV CIHUB_INTERNAL_ADDR=0.0.0.0:8081
EXPOSE 8080 8081
USER nonroot

ENTRYPOINT ["/usr/local/bin/cihub-server"]
CMD ["start"]
