# api-ws-demo

Rust (axum) test server for developers debugging REST/WebSocket/STOMP clients. Deployed to
Render.com's free Web Service tier via GitHub Actions → GHCR → Render "Existing Image" (no build
step on Render's side). Live at `https://api-ws-demo-latest.onrender.com`.

## Hard constraints — read before adding anything

- **Stay lightweight.** This runs on a free-tier container with a real cgroup-enforced 512MB RAM
  cap and weak/shared CPU. Every dependency choice in this repo was picked against that
  constraint: hand-rolled HS256 JWT instead of `jsonwebtoken` (its only pure-Rust backend drags in
  `rsa`/`ed25519-dalek`/`p256` etc.), fast HMAC-SHA256 password hashing instead of argon2/bcrypt
  (deliberately slow algorithms cost real CPU here for no real security benefit on test data),
  hand-rolled `/proc/stat` CPU sampling instead of `sysinfo`, `tower_governor = "0.5"` pinned
  specifically to avoid the latest version's `governor 0.10.x` → `getrandom 0.3` →
  `wasm-bindgen`/`js-sys` chain. Before adding a new crate, check whether it pulls in something
  heavy transitively (`cargo tree` or a scratch-project `cargo add --dry-run` against the same
  registry) — this has bitten us before.
- **No third-party STOMP crate.** Already researched — the only Rust STOMP server option (`romp`)
  has been unmaintained since 2020 and isn't designed as an embeddable axum component. The STOMP
  implementation in `src/stomp/` is hand-rolled and stays that way.
- **Static test assets are pre-computed, never compressed/encoded at request time.** The 5
  compressed-topic assets under `assets/` (see the `regenerate-compressed-assets` skill) are
  generated offline and embedded via `include_bytes!` — this is a deliberate design to keep
  per-request CPU cost at zero regardless of how many clients hit those endpoints.

## Architecture

- `src/main.rs` — binds `0.0.0.0:$PORT` (Render injects `PORT`), spawns the CPU sampler, builds
  the router.
- `src/routes.rs` — all route registration + layer ordering (CORS outermost, then rate limiter,
  then the CPU-breaker middleware, then tracing — see the comment there for why this order
  matters).
- `src/response.rs` — the `{code, data, message}` envelope (`ApiResponse<T>`, `AppError`) used by
  every JSON REST handler except `/api/echo` (which is a raw byte/content-type passthrough by
  design) and the `/api/compressed*` endpoints (raw bytes with real `Content-Encoding` headers).
- `src/auth/` — hand-rolled JWT (access token) + opaque-UUID refresh tokens (server-revocable,
  rotated on every `/auth/refresh` call) + in-memory user store. State resets on every
  restart/redeploy — this is intentional for an ephemeral test server, not a bug.
- `src/stomp/` — hand-rolled STOMP 1.2 broker. `frame.rs` parses/serializes frames (`OutgoingItem`
  wraps either a full frame or a bare heartbeat `\n` byte — heartbeats are NOT frames per spec).
  `broker.rs` holds subscriptions + a per-destination last-message cache (used by the delayed
  auto-push). `connection.rs` is a `tokio::select!` loop juggling: incoming WS messages, an
  outgoing-heartbeat interval, an incoming-heartbeat-timeout check, and a 3-minute hard connection
  TTL. Every SUBSCRIBE also spawns an independent 3-second delayed push (static asset for the 5
  compressed topics, `{"response": <cached>|"ready"}` for anything else).
- `src/cpu.rs` — background `/proc/stat` sampler (Linux-only, degrades to 0% elsewhere) backing
  both `/api/stats`'s `cpu_percent` and the CPU circuit breaker middleware.
- `src/stats.rs` — `/api/stats`. Memory prefers the container's actual cgroup v2/v1 limit over
  `/proc/meminfo` (which on a shared host reports host-wide memory, wildly overstating headroom).
- `src/compressed_assets.rs` / `src/compressed_http.rs` — the 5 static pre-compressed test assets,
  shared between the HTTP endpoints and the matching STOMP topics.

## Testing

`cargo test` covers pure-logic pieces (STOMP frame round-trips, heartbeat negotiation, CPU delta
math, JWT sign/verify, usage-percent rounding) — this project follows a pattern of factoring
timing/protocol math into small standalone functions specifically so they're unit-testable without
a real socket/filesystem. There's no integration test suite; end-to-end behavior (auth flow,
STOMP heartbeat/ACK/TTL/auto-push, rate limiting, CPU breaker) is verified manually per-change with
throwaway Node `.mjs` scripts against a locally-running `cargo run` instance, then deleted — see
recent commit messages for the kinds of checks that matter before considering a STOMP/auth change
done.

Docker isn't available in the usual dev environment this project is worked in — `docker build`
correctness is only ever confirmed via the GitHub Actions run after pushing, not locally.

## Deployment

Push to `main` → GitHub Actions builds a Docker image (`rust:1-alpine` builder with `RUN
--mount=type=cache` for the cargo registry/target dirs, bridged across separate CI runs via
`buildkit-cache-dance` since BuildKit cache mounts are otherwise invisible to the `gha` layer-cache
backend — this is why adding a new dependency doesn't force a full from-scratch rebuild) → pushes
to `ghcr.io/icodejoo/api-ws-demo` → calls the Render deploy hook. See `README.md` for the full
endpoint reference and the one-time manual setup steps (GHCR package visibility, Render service
creation, deploy hook secret).
