# Rollback model

Bifrost uses [GGRS](https://github.com/gschup/ggrs) through `bevy_ggrs` with a single compact `WorldState` resource.

## What rolls back

- Paddle positions, ball position/velocity, brick bitset, scores, serve phase

## What does not

- Meshes/sprites (rebuilt from `WorldState` each frame)
- Audio and particles (driven from confirmed events once)

## Input delay

Default **2 frames** at 60 Hz. The HTML shell “Lag Forge” adds local delay for demos only; it does not affect remote peers.

## Limits (honest)

Rollback reduces *felt* input lag but cannot remove prediction windows entirely. Under sustained loss or very high RTT, play will hitch or stall while the session reconciles. This demo is not ranked or cheat-resistant P2P.

## Verification

`cargo test -p bifrost_sim` includes checksum stability and checkpoint-resume tests that mirror rollback resimulation.
