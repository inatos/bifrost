# Contributing

1. `cargo test -p bifrost_sim` before opening a PR
2. `cargo fmt --all` and `cargo clippy` on touched crates
3. Keep simulation changes deterministic — add checksum or replay tests
4. Do not claim latency guarantees without inspector evidence

## MSRV

Rust **1.90** (see `rust-toolchain.toml`).
