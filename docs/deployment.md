# Deployment

## Prerequisites

- A DNS name pointing at the host that will run Compose
- TLS email for ACME when Bifrost runs its **own** Caddy (`ACME_EMAIL`)
- Optional Coturn hostname for relay fallback (`TURN_URLS`, `TURN_SECRET`, `EXTERNAL_IP`)

Copy `deploy/.env.example` to `deploy/.env` and set `BIFROST_HOST`, `PUBLIC_ORIGIN`, and `PUBLIC_WS_ORIGIN`.

Cross-NAT / two-home play needs Coturn:

```bash
# in deploy/.env
TURN_SECRET=$(openssl rand -hex 32)
EXTERNAL_IP=<public-ipv4>
TURN_URLS=turn:bifrost.example.com:3478?transport=udp,turn:bifrost.example.com:3478?transport=tcp
# open ufw: 3478/tcp, 3478/udp, 49160:49200/udp
docker compose --profile turn -f docker-compose.yml -f docker-compose.edge.yml up -d --build
```

`GET /api/turn` must return 200; the WASM shell passes those credentials into Matchbox ICE.

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
| `caddy` | TLS termination (standalone mode only) |

## Production — edge mode (recommended on a shared VPS)

When Arathyll already owns `:80`/`:443`, run Bifrost **without** its Caddy and attach to Docker network `arathyll_edge`:

```bash
cd deploy
cp -n .env.example .env   # PUBLIC_ORIGIN=https://bifrost.example.com …
# Arathyll release stack must be up first (creates arathyll_edge)
docker compose -f docker-compose.yml -f docker-compose.edge.yml up -d --build
```

Arathyll's `docker/Caddyfile.production` proxies `bifrost.*` → aliases `bifrost_signal` / `bifrost_web`. See Arathyll [`docs/kvm-migration.md`](../../arathyll/docs/kvm-migration.md).

## Production — standalone Caddy

Only when Bifrost is the sole occupant of 80/443:

```bash
cd deploy
cp -n .env.example .env
docker compose --profile bifrost-own-caddy -f docker-compose.yml -f docker-compose.edge.yml up -d --build
# or: docker compose up -d --build   # base file includes caddy without edge overlay
```

## Health

```bash
curl -fsS "https://${BIFROST_HOST:-localhost}/healthz"
```

## Rollback deploy

Pin image digests in Compose overrides; `docker compose up -d` with previous tag if a release regresses.
