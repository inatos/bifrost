use crate::input::{INPUT_LEFT, INPUT_RIGHT};
use crate::rules::{MatchPhase, WorldState, PADDLE_W};
use crate::fixed::FP_SCALE;

#[derive(Copy, Clone, Debug)]
pub struct BotConfig {
    pub reaction_frames: u32,
    pub aim_error: i32,
    pub aggressiveness: i32,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            reaction_frames: 4,
            aim_error: 8 * FP_SCALE / 100,
            aggressiveness: 60,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct BotState {
    pub cooldown: u32,
}

pub fn choose_input(
    state: &WorldState,
    bot: &mut BotState,
    player: usize,
    cfg: BotConfig,
) -> u8 {
    if state.phase == MatchPhase::MatchOver {
        return 0;
    }
    if bot.cooldown > 0 {
        bot.cooldown -= 1;
        return 0;
    }
    bot.cooldown = cfg.reaction_frames;

    let target_x = predict_intercept_x(state, player, cfg.aim_error);
    let dx = target_x - state.paddles[player].x;
    let threshold = PADDLE_W / 8;
    if dx > threshold {
        INPUT_RIGHT
    } else if dx < -threshold {
        INPUT_LEFT
    } else {
        0
    }
}

fn predict_intercept_x(state: &WorldState, player: usize, error: i32) -> i32 {
    let ball_x = state.ball.pos.x;
    let toward_bot = if player == 0 {
        state.ball.vel.y < 0
    } else {
        state.ball.vel.y > 0
    };
    if !toward_bot {
        return 0;
    }
    ball_x + error * if (state.frame % 2) == 0 { 1 } else { -1 }
}
