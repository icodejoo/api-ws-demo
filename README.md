# api-ws-demo

A Rust (axum) test server exposing a REST API, a raw WebSocket echo endpoint, and a minimal
in-memory STOMP 1.2 broker over WebSocket. Built by GitHub Actions into a Docker image published
to GHCR, and deployed on Render.com's free Web Service tier via the "Existing Image" deploy
source — Render just pulls and runs the prebuilt image, no build step on Render's side.

All plain JSON REST responses use a unified envelope: `{"code": 0, "data": ..., "message": "ok"}`
(non-zero `code` + an HTTP error status on failure). `/api/echo` is the one exception — it's a
byte/content-type passthrough, not a JSON response.

CORS is wide open (`Access-Control-Allow-Origin: *` etc.) on every route, including error
responses from the rate limiter/CPU breaker — this is a test server, not something serving
credentialed/cookie-based sessions, so permissive CORS has no real downside here.

## Endpoints

- `GET /health` — status/uptime JSON
- `GET /api/info` — server name/version/uptime JSON
- `POST /api/echo` — echoes the request body back with the same Content-Type (not enveloped)
- `GET|POST /api/mock` — universal controllable test response: `delay_ms` (clamped to 10s),
  `status`, `code`, `message`, `data` (POST body supports nested JSON; GET query params only
  support a flat string for `data`)
- `GET /api/compressed`, `/api/compressed-zstd`, `/api/compressed-mp`, `/api/compressed-mp-gzip`,
  `/api/compressed-mp-zstd` — the same five static pre-encoded/pre-compressed test payloads as the
  matching STOMP topics below, served over plain HTTP with correct `Content-Type`/`Content-Encoding`
  response headers (not enveloped — raw bytes), for testing HTTP client compression handling (e.g.
  `curl --compressed` auto-decodes the gzip ones transparently; zstd support varies by client, which
  is exactly what these exist to test).
- `POST /auth/register` — `{"username", "password"}`
- `POST /auth/login` — `{"username", "password"}` → `{access_token, refresh_token, token_type, expires_in}`
- `POST /auth/refresh` — `{"refresh_token"}` → rotates to a new access+refresh token pair,
  invalidating the old refresh token
- `POST /auth/logout` — `{"refresh_token"}` → revokes it
- `GET /api/me` — requires `Authorization: Bearer <access_token>`
- `GET /ws` — raw WebSocket, echoes back any text/binary frame sent, no auth
- `GET /ws/secure` — same echo behavior, requires `?token=<access_token>` at connect time
- `GET /stomp` — STOMP 1.2 over WebSocket: CONNECT, SUBSCRIBE/UNSUBSCRIBE, SEND (broadcasts to
  all subscribers of the destination), DISCONNECT, ERROR. No SockJS fallback.
  Destination-based auth: `/topic/public/*` is open to anyone; `/topic/secure/*` requires the
  CONNECT frame to carry a valid `Authorization: Bearer <access_token>` header (present-but-invalid
  tokens reject the CONNECT outright; an absent header just means anonymous/public-only access).
  There are also five special open topics that always broadcast the same fixed, build-time-generated
  payload regardless of what's SENT to them — for testing client-side decompression/decoding without
  any server-side CPU cost (nothing is ever compressed or encoded at request time, only read from a
  `include_bytes!`-embedded static asset in `assets/`):
  | Topic | Content-Type | Content-Encoding |
  |---|---|---|
  | `/topic/compressed` | `application/json` | `gzip` |
  | `/topic/compressed-zstd` | `application/json` | `zstd` |
  | `/topic/compressed-mp` | `application/msgpack` | _(none)_ |
  | `/topic/compressed-mp-gzip` | `application/msgpack` | `gzip` |
  | `/topic/compressed-mp-zstd` | `application/msgpack` | `zstd` |

### Rate limiting & CPU circuit breaker

- Per-IP rate limiting (via `tower_governor`, reading `X-Forwarded-For`/`X-Real-Ip` since Render
  sits in front as a proxy): `RATE_LIMIT_PER_SECOND` (default 5), `RATE_LIMIT_BURST` (default 10).
- CPU circuit breaker: if CPU usage (sampled from `/proc/stat`, Linux only — always reads 0% in
  local dev on Windows/macOS) is at or above `CPU_BREAKER_THRESHOLD_PCT` (default 90), every
  request immediately gets a `503` in the unified envelope, before even reaching the rate limiter.
- `JWT_SECRET` — HS256 signing secret for access tokens. If unset, a random secret is generated at
  startup (fine for this ephemeral test server — all sessions already reset on restart/redeploy).

## Local development

```powershell
cargo test
$env:PORT = "8080"
$env:RUST_LOG = "info,api_ws_demo=debug"
cargo run
```

```powershell
curl http://localhost:8080/health
curl http://localhost:8080/api/info
curl -X POST -H "Content-Type: application/json" -d '{"hello":"world"}' http://localhost:8080/api/echo
```

Raw WebSocket (using `websocat` or `wscat`):

```powershell
websocat ws://localhost:8080/ws
```

STOMP (Git-Bash/WSL, to emit the NUL frame terminator):

```bash
printf 'CONNECT\naccept-version:1.2\nhost:localhost\n\n\0' | websocat ws://localhost:8080/stomp
```

## Docker

```powershell
docker build -t api-ws-demo:local .
docker run --rm -p 8080:8080 -e PORT=8080 api-ws-demo:local
```

## Deployment (Render.com)

1. Push to `main` — GitHub Actions builds the image and pushes `ghcr.io/icodejoo/api-ws-demo`
   with tags `latest` and the short commit SHA.
2. One-time: on the GHCR package page, set visibility to **Public** (Settings → Danger Zone).
   GHCR packages pushed via the default `GITHUB_TOKEN` are created private regardless of the
   repo's own visibility, and there's no Actions-only way to force public at creation.
3. In the Render dashboard, create a Web Service from this repo's `render.yaml` Blueprint (or
   manually: New → Web Service → "Existing Image" → `ghcr.io/icodejoo/api-ws-demo:latest`,
   plan: Free).
4. Copy the service's Deploy Hook URL (Settings → Deploy Hook) and add it as the
   `RENDER_DEPLOY_HOOK_URL` secret in this repo's GitHub Actions settings. Render doesn't poll
   the registry for new tags, so the workflow calls this hook after each push to trigger a
   redeploy.
