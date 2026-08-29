# Deployment

## Prerequisites

- DNS `bifrost.arathyll.com` → host running Compose
- TLS email for ACME (`ACME_EMAIL`)
- Optional Coturn at `turn.arathyll.com` for relay fallback

## Local preview

```bash
cd deploy
docker compose -f docker-compose.local.yml up -d --build
# http://127.0.0.1:1334/  ·  /healthz  ·  Create room / Play bot
```

Arathyll Dev Lab embeds this origin via `PUBLIC_BIFROST_ORIGIN` (default `http://127.0.0.1:1334` in local Compose override).

Services:

| Service | Role |
|---------|------|
| `signal` | Room API + WebSocket signaling edge |
| `web` | Trunk-built WASM + HTML shell (nginx) |
| `caddy` | TLS termination, `/api` + `/signal` proxy |

## Health

```bash
curl -fsS https://bifrost.arathyll.com/healthz
```

## Rollback deploy

Pin image digests in Compose overrides; `docker compose up -d` with previous tag if a release regresses.

## Not in Arathyll nexus

Do **not** add Bifrost to `web/arathyll` Compose/Caddy. The dev site only links to this host.
