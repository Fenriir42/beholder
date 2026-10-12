# syntax=docker/dockerfile:1.7
# Server image: a static musl binary on `scratch`. Built in CI only; the VPS
# just pulls it (SPEC §2: never compile on the VPS).

FROM rust:1.99.0-alpine3.22 AS build
# C toolchain for the bundled SQLite, zstd and libwebp.
RUN apk add --no-cache build-base
WORKDIR /src
COPY . .
# The image already has the pinned toolchain; the repo's toolchain file would
# also pull the wasm32 target, which the server doesn't need.
RUN rm rust-toolchain.toml
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --profile server -p beholder-server \
    && cp target/server/beholder-server /beholder-server \
    && mkdir /data

FROM scratch
COPY --from=build /beholder-server /beholder-server
# A named volume copies this directory's ownership on first use.
COPY --from=build --chown=10001:10001 /data /data
USER 10001:10001
ENV BEHOLDER_BIND=0.0.0.0:8080 \
    BEHOLDER_DATA_DIR=/data \
    BEHOLDER_LOG=info
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/beholder-server"]
