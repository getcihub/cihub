# The image of cihub-server (docs/DEPLOYMENT.md). The build stage uses the
# Rust of mise.toml on Debian 12, the glibc of the runtime stage. On Linux,
# the TLS client reads the root certificates of the system, which the runtime
# image provides: the binary embeds no list of its own (DECISIONS.md D81).

# rust:1.98.1-slim-bookworm
FROM docker.io/library/rust@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS build
WORKDIR /src
COPY . .
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
