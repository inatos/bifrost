# Deployment

## Prerequisites

- DNS `bifrost.arathyll.com` → host running Compose
- TLS email for ACME (`ACME_EMAIL`)
- Optional Coturn at `turn.arathyll.com` for relay fallback

## Compose

```bash
cp .env.example deploy/.env
# edit secrets
cd deploy
docker compose up -d --build
```

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
