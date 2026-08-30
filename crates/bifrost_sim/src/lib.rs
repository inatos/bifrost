//! Deterministic arena simulation for competitive Breakout/Pong.

mod bot;
mod checksum;
mod collision;
mod fixed;
mod input;
mod paddle_geom;
mod replay;
mod rules;
mod wild_bricks;

pub use bot::{BotConfig, BotState};
pub use checksum::checksum;
pub use fixed::{FP_SCALE, Vec2};
pub use input::{
    FrameInput, INPUT_ANGLE_CCW, INPUT_ANGLE_CW, INPUT_DOWN, INPUT_JUMP, INPUT_LEFT, INPUT_RIGHT,
    INPUT_SPIN, INPUT_UP,
};
pub use paddle_geom::{
    JUMP_CLEAR_Z, MAX_JUMP_Z, PADDLE_W_BACK, can_ground_pound, jump_scale_fixed, paddle_airborne,
};
pub use replay::{Replay, decode_replay, encode_replay};
pub use rules::{
    ANGLE_WAVE_DURATION, BRICK_COLS, BRICK_COUNT, BRICK_ROWS, BallState, MAX_WILD_BRICKS,
    MatchPhase, MatchStats, OWNER_NEUTRAL, PADDLE_H, PADDLE_SPEED, PADDLE_W, PaddleState,
    SERVE_FRAMES, SPIN_CHARGE_MAX, TICKS_PER_SECOND, WildBrick, WorldState,
};

use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

/// One confirmed-frame effect for presentation (not part of rollback snapshots).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmedEvent {
    BrickBreak {
        index: u16,
        scorer: u8,
    },
    BrickDamage {
        index: u16,
        hp: u8,
    },
    BrickBounce {
        index: u16,
    },
    WildBrickBreak {
        slot: u8,
    },
    WildBrickHit {
        slot: u8,
    },
    WildBallBurst {
        slot: u8,
    },
    WildPaddleKnock {
        player: u8,
        slot: u8,
    },
    BallNeutralized,
    CornerBounce {
        corner: u8,
    },
    PaddleHit {
        player: u8,
    },
    Goal {
        scorer: u8,
    },
    RoundWin {
        winner: u8,
    },
    RoundTie,
    SpinRelease {
        player: u8,
        charge: u16,
    },
    GroundPound {
        player: u8,
        x: i32,
        y: i32,
    },
    /// Corner shockwave — presentation + knock already applied in sim.
    CornerPulse {
        corner: u8,
        x: i32,
        y: i32,
    },
    /// Snapback force-wave projectile spawn (presentation + knock applied over duration).
    AngleWave {
        player: u8,
        x: i32,
        y: i32,
        /// Beam direction × FP_SCALE (unit vector).
        nx: i32,
        ny: i32,
        power: i32,
        radius: i32,
    },
    /// Mutual attack cancel (spin / snapback clash).
    Clang {
        x: i32,
        y: i32,
    },
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
