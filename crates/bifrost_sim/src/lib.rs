//! Deterministic arena simulation for competitive Breakout/Pong.

mod bot;
mod checksum;
mod collision;
mod fixed;
mod input;
mod replay;
mod rules;

pub use bot::{BotConfig, BotState};
pub use checksum::checksum;
pub use fixed::{Vec2, FP_SCALE};
pub use input::{FrameInput, INPUT_LEFT, INPUT_RIGHT};
pub use replay::{decode_replay, encode_replay, Replay};
pub use rules::{MatchPhase, WorldState, BRICK_COLS, BRICK_ROWS, TICKS_PER_SECOND};

use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

/// One confirmed-frame effect for presentation (not part of rollback snapshots).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmedEvent {
    BrickBreak { index: u16 },
    PaddleHit { player: u8 },
    Goal { scorer: u8 },
    RoundWin { winner: u8 },
}

pub struct StepOutput {
    pub events: Vec<ConfirmedEvent>,
}

/// Advance one 60 Hz tick. Pure and deterministic given state + inputs.
pub fn step(state: &mut WorldState, input: FrameInput) -> StepOutput {
    rules::step(state, input)
}

pub fn new_match(seed: u64) -> WorldState {
    WorldState::new(seed)
}

pub fn advance_bot(state: &WorldState, bot: &mut BotState, player: usize, cfg: BotConfig) -> u8 {
    bot::choose_input(state, bot, player, cfg)
}

pub fn simulate_frames(seed: u64, inputs: &[FrameInput]) -> WorldState {
    let mut state = new_match(seed);
    for &inp in inputs {
        step(&mut state, inp);
    }
    state
}

pub fn rng_from_seed(seed: u64) -> Xoshiro256PlusPlus {
    Xoshiro256PlusPlus::seed_from_u64(seed)
}

#[cfg(test)]
mod tests;
