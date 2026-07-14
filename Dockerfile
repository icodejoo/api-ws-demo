# syntax=docker/dockerfile:1
# ---- Builder ----
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets

# Cargo's own registry + target caches live in these mounts. BuildKit keeps
# them across builds (persisted across CI runs via buildkit-cache-dance in
# .github/workflows/docker-publish.yml), so cargo only recompiles the crates
# that actually changed — including when a *new* dependency is added, unlike
# whole-layer Docker caching, which would otherwise force a full from-scratch
# recompile of every dependency just because Cargo.lock's hash changed. The
# binary is copied out to a plain (non-cache-mount) path before the RUN ends,
# since cache mount contents never become part of the resulting image layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked && \
    cp target/release/api-ws-demo /api-ws-demo-bin

# ---- Runtime ----
FROM scratch AS runtime

COPY --from=builder /api-ws-demo-bin /api-ws-demo

ENV RUST_LOG=info
EXPOSE 8080

ENTRYPOINT ["/api-ws-demo"]
