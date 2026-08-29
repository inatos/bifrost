# Deployment

## Prerequisites

- A DNS name pointing at the host that will run Compose
- TLS email for ACME (`ACME_EMAIL`)
- Optional Coturn hostname for relay fallback (`TURN_URLS`)

Copy `deploy/.env.example` to `deploy/.env` and set `BIFROST_HOST`, `PUBLIC_ORIGIN`, and `PUBLIC_WS_ORIGIN` to your public hostname.

## Local preview

```bash
cd deploy
docker compose -f docker-compose.local.yml up -d --build
# http://127.0.0.1:1334/  ·  /healthz  ·  Create room / Play bot
```

Parent sites can embed this origin (local Compose defaults to `http://127.0.0.1:1334`). Adjust Caddy `frame-ancestors` if you iframe from another host.

Services:

| Service | Role |
|---------|------|
| `signal` | Room API + WebSocket signaling edge |
| `web` | Trunk-built WASM + HTML shell (nginx) |
| `caddy` | TLS termination, `/api` + `/signal` proxy |

## Production

```bash
cd deploy
cp -n .env.example .env   # edit host + ACME email
docker compose up -d --build
```

## Health

```bash
curl -fsS "https://${BIFROST_HOST:-localhost}/healthz"
```

## Rollback deploy

Pin image digests in Compose overrides; `docker compose up -d` with previous tag if a release regresses.
