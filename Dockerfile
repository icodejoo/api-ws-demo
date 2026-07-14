# ---- Builder ----
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Cache dependency compilation as its own layer.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

# Now build the real binary.
COPY src ./src
COPY assets ./assets
RUN touch src/main.rs \
    && cargo build --release --locked

# ---- Runtime ----
FROM scratch AS runtime

COPY --from=builder /app/target/release/api-ws-demo /api-ws-demo

ENV RUST_LOG=info
EXPOSE 8080

ENTRYPOINT ["/api-ws-demo"]
