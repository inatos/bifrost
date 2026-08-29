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

## Determinism boundary

Only `WorldState` advances during rollback. Audio/VFX read **confirmed** `ConfirmedEvent` values after the frame is accepted. Rendering mirrors `WorldState` outside the GGRS schedule.

## Hosting boundary

Deploy from this repository with your own DNS name (`BIFROST_HOST`). Keep the game on its own origin if a parent site embeds or links to it (CSP `frame-ancestors` + separate host).
