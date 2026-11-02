FROM docker.io/library/node:24.22.2-trixie-slim@sha256:54f84b4cf8432b21aaffd5d3ce8d41e12cea0d10d33c09d2f3c9879d49d6b446 AS console

WORKDIR /src

COPY package.json pnpm-workspace.yaml pnpm-lock.yaml tsconfig.base.json ./
COPY apps/console/package.json apps/console/
COPY packages/theme/package.json packages/theme/
COPY packages/icons/package.json packages/icons/
COPY packages/utils/package.json packages/utils/
COPY packages/ui/package.json packages/ui/

# The caches speed up a local build; a CI runner starts with them empty.
RUN --mount=type=cache,id=pnpm-store,target=/pnpm/store \
    corepack enable && pnpm install --frozen-lockfile --store-dir /pnpm/store --filter @cihub/console...

COPY apps/console/ apps/console/
COPY packages/ ./packages/

RUN pnpm --dir apps/console build

FROM docker.io/library/rust:1.99.1-slim-trixie@sha256:b8e062b1c3a80183df83d9ba187cadd2b5ab84bc78d453dbfeb2fccd5f982247 AS build

ARG TARGETPLATFORM

WORKDIR /src

COPY . .
COPY --from=console /src/apps/console/dist apps/console/dist

RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cihub-target-${TARGETPLATFORM},target=/src/target \
    cargo build --release --locked -p cihub-server \
    && cp target/release/cihub-server /usr/local/bin/cihub-server

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:08476c244888bfb425bae1ae4dd7c17f093e972aab8426883d97f3c6f280f7c1

COPY --from=build /usr/local/bin/cihub-server /usr/local/bin/cihub-server
COPY --from=build /src/LICENSE /usr/share/doc/cihub/LICENSE

ENV CIHUB_INTERNAL_ADDR=0.0.0.0:8081

EXPOSE 8080 8081
USER nonroot

ENTRYPOINT ["/usr/local/bin/cihub-server"]
CMD ["start"]
