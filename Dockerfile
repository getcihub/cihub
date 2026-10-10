# The image of CIHub, with the cihub binary (specs/DEPLOYMENT.md). The build stage uses the
# Rust of mise.toml on Debian 12, the glibc of the runtime stage. On Linux,
# the TLS client reads the root certificates of the system, which the runtime
# image provides: the binary embeds no list of its own.

# The dashboard (web/), built with the Node and the pnpm of mise.toml; the
# build stage embeds web/dist in the binary (crates/web/build.rs).
# node:24.21.0-bookworm-slim
FROM docker.io/library/node@sha256:d6aa754f16b3197301076f047b5def2f02ea1dbbc2ca920407d46d7ec7f87b20 AS web
WORKDIR /src/web
COPY web/package.json web/pnpm-lock.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# rust:1.98.1-slim-bookworm
FROM docker.io/library/rust@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS build
WORKDIR /src
COPY . .
COPY --from=web /src/web/dist web/dist
RUN cargo build --release --locked -p cihub-cli

# gcr.io/distroless/cc-debian12:nonroot
FROM gcr.io/distroless/cc-debian12@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f

COPY --from=build /src/target/release/cihub /usr/local/bin/cihub
COPY --from=build /src/LICENSE /usr/share/doc/cihub/LICENSE

# The probes reach the internal address from outside the container.
ENV CIHUB_INTERNAL_ADDR=0.0.0.0:8081
EXPOSE 8080 8081
USER nonroot

ENTRYPOINT ["/usr/local/bin/cihub"]
CMD ["server", "start"]
