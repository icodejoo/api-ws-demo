# api-ws-demo

A Rust (axum) test server exposing a REST API, a raw WebSocket echo endpoint, and a minimal
in-memory STOMP 1.2 broker over WebSocket. Built by GitHub Actions into a Docker image published
to GHCR, and deployed on Render.com's free Web Service tier via the "Existing Image" deploy
source — Render just pulls and runs the prebuilt image, no build step on Render's side.

## Endpoints

- `GET /health` — status/uptime JSON
- `GET /api/info` — server name/version/uptime JSON
- `POST /api/echo` — echoes the request body back with the same Content-Type
- `GET /ws` — raw WebSocket, echoes back any text/binary frame sent
- `GET /stomp` — STOMP 1.2 over WebSocket: CONNECT, SUBSCRIBE/UNSUBSCRIBE, SEND (broadcasts to
  all subscribers of the destination), DISCONNECT, ERROR. No SockJS fallback.

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
