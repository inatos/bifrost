# Bifrost

**Live demo:** [bifrost.arathyll.com](https://bifrost.arathyll.com)

Bifrost is a compact competitive Breakout/Pong arena used to demonstrate **GGRS rollback** over **WebRTC** in the browser. The simulation is deterministic and Bevy-free at the core; rendering and networking wrap the same `WorldState` snapshot used for rollback.

## What you can try

- **Play bot** — instant local match, no signaling required
- **Private rooms** — ephemeral two-player codes via `/api/rooms`
- **Rollback inspector** — RTT, input delay, rollback depth, checksum (toggle in shell)
- **Replay codes** — deterministic input tapes shareable without server storage

## Stack (pinned)

| Crate | Version | Notes |
|-------|---------|-------|
| Bevy | 0.18.1 | WebGL2 WASM client |
| bevy_ggrs | 0.20 | Rollback schedule |
| Matchbox | 0.14 | WebRTC signaling transport |
| Rust | 1.90 | MSRV for Bevy 0.18 deps |

Bevy 0.19 is deferred until Matchbox’s GGRS stack publishes a compatible release train.

## Quick start (native)

```bash
rustup toolchain install 1.90.0
cargo run -p bifrost_client -- --bot
# Enter → bot match · WASD / arrows · R restart
```

## Signaling service

```bash
cargo run -p bifrost_signal
curl -s localhost:8787/healthz | jq
curl -s -X POST localhost:8787/api/rooms -H 'content-type: application/json' -d '{"protocol_version":1}'
```

## WASM / web shell

```bash
cargo install trunk
trunk serve --open
# http://127.0.0.1:1334/
```

## Tests

```bash
cargo test -p bifrost_sim
cargo fmt --all -- --check
cargo clippy -p bifrost_sim -p bifrost_signal -- -D warnings
```

## Deploy

See [docs/deployment.md](docs/deployment.md). Production runs as an **independent** stack at `bifrost.arathyll.com` (not embedded in the Arathyll nexus Compose).

## Docs

- [Architecture](docs/architecture.md)
- [Rollback model](docs/rollback.md)
- [Room protocol](docs/protocol.md)
- [Privacy & security](docs/privacy-security.md)
- [Contributing](CONTRIBUTING.md)

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
