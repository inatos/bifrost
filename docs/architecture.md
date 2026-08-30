# Architecture

```text
web/shell.js ── REST ──► bifrost_signal (/api/rooms, /signal)
       │                        │
       ▼                        ▼
 bifrost_client (Bevy WASM)   ephemeral room tickets + TURN creds
       │
       ├── bifrost_sim (deterministic WorldState, fixed-point)
       ├── bifrost_net (diagnostics, GGRS config helpers)
       └── bevy_ggrs + bevy_matchbox (P2P inputs, rollback schedule)
```

## Crates

| Path | Role |
|------|------|
| `crates/bifrost_sim` | Authoritative rules, collision, bot, replay codec |
| `crates/bifrost_protocol` | Shared room/ticket DTOs |
| `crates/bifrost_net` | Rollback diagnostics and input packing |
| `crates/bifrost_client` | Bevy renderer + bot mode + online hooks |
| `services/bifrost_signal` | Ephemeral room API, metrics, TURN minting |

## Hosting boundary

Deploy from this repository with your own DNS name (`BIFROST_HOST` / `PUBLIC_ORIGIN` / `PUBLIC_WS_ORIGIN`). Keep the game on its own origin if a parent site embeds or links to it (CSP `frame-ancestors` + separate host).

Typical production shape (operator view):

1. Edge TLS terminates for the game hostname and proxies HTTP to the static WASM shell, `/api/*` to the room service, and `/signal/*` to Matchbox.
2. On a shared VPS, Bifrost containers join the site’s edge Docker network (aliases for signal + web) instead of binding their own public :80/:443 — see `deploy/docker-compose.edge.yml`.
3. Optional Coturn (`--profile turn`) supplies TURN when STUN-only ICE fails; the API mints short-lived REST credentials (`GET /api/turn`). Do not commit secrets; set `TURN_SECRET` / `EXTERNAL_IP` / `TURN_URLS` in `deploy/.env`.

## Determinism boundary

Only `WorldState` advances during rollback. Audio/VFX read **confirmed** `ConfirmedEvent` values after the frame is accepted. Rendering mirrors `WorldState` outside the GGRS schedule.
