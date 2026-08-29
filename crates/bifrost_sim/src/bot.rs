use crate::fixed::{isqrt, Vec2, FP_SCALE};
use crate::input::{INPUT_DOWN, INPUT_JUMP, INPUT_LEFT, INPUT_RIGHT, INPUT_SPIN, INPUT_UP};
use crate::paddle_geom::paddle_airborne;
use crate::rules::{
    MatchPhase, PaddleState, WorldState, ARENA_H, ARENA_W, BALL_R, BALL_SPEED, BRICK_COUNT, BRICK_H,
    BRICK_W, PADDLE_H, PADDLE_W, WALL, WILD_BRICK_HALF, OWNER_NEUTRAL,
};

/// Frames between bot jumps — stops hop spam.
const JUMP_COOLDOWN_FRAMES: u32 = 48;

#[derive(Copy, Clone, Debug)]
pub struct BotConfig {
    pub reaction_frames: u32,
    pub aim_error: i32,
    pub aggressiveness: i32,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            reaction_frames: 0,
            aim_error: FP_SCALE / 100,
            aggressiveness: 130,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct BotState {
    pub cooldown: u32,
    /// 0 idle · 1 leap over ball · 2 flank around · 3 strike clear
    pub escape_phase: u8,
    pub escape_timer: u32,
    pub flank_sign: i8,
    pub jump_cd: u32,
    pub escape_jumped: bool,
    /// Idle frames after faceoff (~1s) so the bot doesn't cheat the drop.
    pub post_serve_idle: u32,
}

/// Hold still for one second after the ball drops.
const POST_SERVE_IDLE_FRAMES: u32 = 60;

pub fn choose_input(
    state: &WorldState,
    bot: &mut BotState,
    player: usize,
    cfg: BotConfig,
) -> u8 {
    if bot.jump_cd > 0 {
        bot.jump_cd -= 1;
    }
    if state.phase == MatchPhase::Readying {
        return INPUT_JUMP;
    }
    if state.phase == MatchPhase::MatchOver {
        bot.escape_phase = 0;
        bot.escape_jumped = false;
        bot.post_serve_idle = 0;
        return 0;
    }
    // Don't move during serve countdown — player gets a fair look.
    if state.phase == MatchPhase::Serving {
        bot.post_serve_idle = POST_SERVE_IDLE_FRAMES;
        bot.escape_phase = 0;
        return 0;
    }
    if bot.post_serve_idle > 0 {
        bot.post_serve_idle -= 1;
        return 0;
    }

    // Clear trapped / stalled balls without hop-spamming.
    if bot.escape_phase > 0 || ball_needs_clear(state, player) {
        return escape_maneuver(state, bot, player);
    }

    if bot.cooldown > 0 {
        bot.cooldown -= 1;
        return 0;
    }
    bot.cooldown = cfg.reaction_frames;

    let (target_x, target_y) = predict_intercept(state, player, cfg.aim_error, cfg.aggressiveness);
    let mut mask = steer_toward(state, bot, player, target_x, target_y);
    // Press the attack: spin when the ball is in contest range / owned by us.
    let ball = state.ball;
    let paddle = state.paddles[player];
    let dx = ball.pos.x - paddle.x;
    let dy = ball.pos.y - paddle.y;
    let near = dx.abs() < PADDLE_W * 2 && dy.abs() < PADDLE_H * 5;
    let contest = ball.owner == player as u8
        || ball.owner == OWNER_NEUTRAL
        || (ball.owner != player as u8 && near);
    if contest && near {
        mask |= INPUT_SPIN;
    }
    // Chase jump more often when closing on the ball.
    if should_jump(state, player, target_x) || (near && bot.jump_cd == 0 && (state.frame % 37) < 4)
    {
        mask = try_jump(bot, mask);
    }
    mask
}

fn escape_maneuver(state: &WorldState, bot: &mut BotState, player: usize) -> u8 {
    let paddle = state.paddles[player];
    let ball = state.ball.pos;

    if bot.escape_phase == 0 {
        bot.escape_phase = 1;
        bot.escape_timer = 18;
        bot.escape_jumped = false;
        let wall_x = ARENA_W / 2 - WALL;
        let toward_center = if ball.x.abs() > wall_x / 2 {
            -ball.x.signum()
        } else if paddle.x >= ball.x {
            1
        } else {
            -1
        };
        bot.flank_sign = toward_center.clamp(-1, 1) as i8;
        if bot.flank_sign == 0 {
            bot.flank_sign = if (state.frame & 1) == 0 { 1 } else { -1 };
        }
    }

    if bot.escape_timer > 0 {
        bot.escape_timer -= 1;
    }

    let flank = (bot.flank_sign as i32) * (PADDLE_W + BALL_R * 2);
    let clear_y = if player == 0 {
        ball.y - PADDLE_H * 2
    } else {
        ball.y + PADDLE_H * 2
    };

    let (target_x, target_y, want_jump) = match bot.escape_phase {
        1 => {
            // One leap at the start of the clear, then commit to the flank.
            let tx = keep_off_walls(ball.x + flank / 2);
            let ty = paddle.y;
            let jump = !bot.escape_jumped && !paddle_airborne(&paddle) && bot.jump_cd == 0;
            if jump {
                bot.escape_jumped = true;
            }
            if bot.escape_timer == 0 || paddle_airborne(&paddle) || bot.escape_jumped {
                bot.escape_phase = 2;
                bot.escape_timer = 22;
            }
            (tx, ty, jump)
        }
        2 => {
            let tx = keep_off_walls(ball.x + flank);
            let ty = clear_y;
            if bot.escape_timer == 0
                || ((paddle.x - tx).abs() < PADDLE_W / 3 && (paddle.y - ty).abs() < PADDLE_H)
            {
                bot.escape_phase = 3;
                bot.escape_timer = 28;
            }
            // Flank on the ground — no jumps unless a wild tile is right on us.
            (tx, ty, wild_blocking(state, &paddle, tx))
        }
        _ => {
            let toward_opp_y = if player == 0 {
                ball.y - PADDLE_H * 3
            } else {
                ball.y + PADDLE_H * 3
            };
            let aim_x = keep_off_walls(ball.x - (bot.flank_sign as i32) * (PADDLE_W / 3));
            let done = bot.escape_timer == 0
                || !ball_needs_clear(state, player)
                || state.ball.vel.y.abs() > BALL_SPEED / 3;
            if done {
                bot.escape_phase = 0;
                bot.escape_timer = 0;
                bot.escape_jumped = false;
            }
            (aim_x, toward_opp_y, false)
        }
    };

    let mut mask = steer_move(paddle, target_x, target_y);
    if want_jump {
        mask = try_jump(bot, mask);
    }
    mask
}

fn steer_toward(
    state: &WorldState,
    bot: &mut BotState,
    player: usize,
    target_x: i32,
    target_y: i32,
) -> u8 {
    let paddle = state.paddles[player];
    let mut mask = steer_move(paddle, target_x, target_y);
    if should_jump(state, player, target_x) {
        mask = try_jump(bot, mask);
    }
    mask
}

fn steer_move(paddle: PaddleState, target_x: i32, target_y: i32) -> u8 {
    let dx = target_x - paddle.x;
    let dy = target_y - paddle.y;
    let threshold_x = PADDLE_W / 10;
    let threshold_y = PADDLE_H / 3;
    let mut mask = 0u8;
    if dx > threshold_x {
        mask |= INPUT_RIGHT;
    } else if dx < -threshold_x {
        mask |= INPUT_LEFT;
    }
    if dy > threshold_y {
        mask |= INPUT_UP;
    } else if dy < -threshold_y {
        mask |= INPUT_DOWN;
    }
    mask
}

fn try_jump(bot: &mut BotState, mask: u8) -> u8 {
    if bot.jump_cd > 0 {
        return mask;
    }
    bot.jump_cd = JUMP_COOLDOWN_FRAMES;
    mask | INPUT_JUMP
}

fn ball_needs_clear(state: &WorldState, player: usize) -> bool {
    let paddle = state.paddles[player];
    let ball = state.ball.pos;
    let close = (ball.x - paddle.x).abs() < PADDLE_W * 2 + BALL_R
        && (ball.y - paddle.y).abs() < PADDLE_H * 5 + BALL_R;
    if !close {
        return false;
    }
    let speed = isqrt(state.ball.vel.len_sq()) as i32;
    let stalled = speed < BALL_SPEED / 3;
    let wall_x = ARENA_W / 2 - WALL - PADDLE_W;
    let near_wall = ball.x.abs() > wall_x * 2 / 3 || paddle.x.abs() > wall_x * 2 / 3;
    let pinned_y = if player == 0 {
        ball.y >= paddle.y - PADDLE_H / 2 && ball.y <= paddle.y + PADDLE_H * 2
    } else {
        ball.y <= paddle.y + PADDLE_H / 2 && ball.y >= paddle.y - PADDLE_H * 2
    };
    (stalled && close) || (near_wall && close && stalled) || (pinned_y && stalled)
}

fn should_jump(state: &WorldState, player: usize, target_x: i32) -> bool {
    let paddle = state.paddles[player];
    if paddle_airborne(&paddle) {
        return false;
    }
    // Only jump for genuine blockers — not for casual pathing.
    if wild_blocking(state, &paddle, target_x) {
        return true;
    }
    brick_blocks_path(state, paddle, target_x)
}

fn wild_blocking(state: &WorldState, paddle: &PaddleState, target_x: i32) -> bool {
    for w in &state.wild_bricks {
        if !w.active {
            continue;
        }
        let near_x = (w.x - paddle.x).abs() < PADDLE_W / 2 + WILD_BRICK_HALF;
        let between = if target_x >= paddle.x {
            w.x >= paddle.x - WILD_BRICK_HALF && w.x <= target_x + WILD_BRICK_HALF
        } else {
            w.x <= paddle.x + WILD_BRICK_HALF && w.x >= target_x - WILD_BRICK_HALF
        };
        let near_y = (w.y - paddle.y).abs() < PADDLE_H + WILD_BRICK_HALF;
        if near_x && near_y && between {
            return true;
        }
    }
    false
}

fn brick_blocks_path(state: &WorldState, paddle: PaddleState, target_x: i32) -> bool {
    // Only hop if a brick sits in the immediate step toward the target.
    let step = (target_x - paddle.x).clamp(-PADDLE_W, PADDLE_W);
    if step.abs() < PADDLE_W / 4 {
        return false;
    }
    let probe_x = paddle.x + step;
    let band = PADDLE_H + BRICK_H / 2;
    for index in 0..BRICK_COUNT {
        if !state.brick_alive(index) {
            continue;
        }
        let c = state.brick_center(index);
        if (c.y - paddle.y).abs() > band {
            continue;
        }
        let half = BRICK_W / 2;
        if (c.x - probe_x).abs() <= half + PADDLE_W / 4 {
            return true;
        }
    }
    false
}

fn predict_intercept(
    state: &WorldState,
    player: usize,
    error: i32,
    aggressiveness: i32,
) -> (i32, i32) {
    let paddle = state.paddles[player];
    let err = error * if (state.frame % 2) == 0 { 1 } else { -1 };
    let ball = state.ball.pos;

    let toward_bot = if player == 0 {
        state.ball.vel.y > 0
    } else {
        state.ball.vel.y < 0
    };
    let own_half = if player == 0 {
        ball.y > 0
    } else {
        ball.y < 0
    };

    if state.ball.owner == player as u8 {
        // Own the ball → drive toward the opponent's goal (score first, bricks second).
        let goal_lane = keep_off_walls(err / 2);
        let brick_bias = nearest_alive_brick(state, ball)
            .map(|(bx, _)| (bx - ball.x) / 6)
            .unwrap_or(0);
        let aim_x = keep_off_walls(ball.x * 2 / 3 + goal_lane + brick_bias);
        let press_y = if player == 0 {
            (ball.y - PADDLE_H * 3).min(ARENA_H / 5)
        } else {
            (ball.y + PADDLE_H * 3).max(-ARENA_H / 5)
        };
        return (aim_x, press_y);
    }

    // CPU hunts the human paddle when the ball is loose and close.
    if player == 1 && state.ball.owner == OWNER_NEUTRAL && aggressiveness > 90 {
        let human = state.paddles[0];
        let close = (human.x - paddle.x).abs() < PADDLE_W * 4
            && (human.y - paddle.y).abs() < PADDLE_H * 8;
        if close && (state.frame / 45) % 2 == 0 {
            return (human.x + err / 2, human.y);
        }
    }

    if toward_bot || own_half {
        let lead = state.ball.vel.x / 2;
        let center_pull = -ball.x / 10;
        let aim_x = keep_off_walls(ball.x + lead + center_pull + err);
        // Meet early, then follow through toward opponent half.
        let meet_y = if player == 0 {
            let base = ball.y.max(paddle.y - PADDLE_H);
            if aggressiveness > 80 {
                base.min(ball.y - PADDLE_H)
            } else {
                base
            }
        } else {
            let base = ball.y.min(paddle.y + PADDLE_H);
            if aggressiveness > 80 {
                base.max(ball.y + PADDLE_H)
            } else {
                base
            }
        };
        return (aim_x, meet_y);
    }

    // Ball going away: cut center and threaten the goal mouth.
    if aggressiveness > 60 {
        if let Some((bx, by)) = nearest_alive_brick(state, ball) {
            let hunt_x = keep_off_walls((ball.x + bx) / 2 + err);
            let hunt_y = if player == 0 {
                by.min(ARENA_H / 5)
            } else {
                by.max(-ARENA_H / 5)
            };
            return (hunt_x, hunt_y);
        }
    }
    let press = if player == 0 { ARENA_H / 8 } else { -ARENA_H / 8 };
    (keep_off_walls(ball.x / 3 + err), press)
}

fn keep_off_walls(x: i32) -> i32 {
    let limit = ARENA_W / 2 - WALL - PADDLE_W;
    x.clamp(-limit + PADDLE_W / 2, limit - PADDLE_W / 2)
}

fn nearest_alive_brick(state: &WorldState, from: impl HasXy) -> Option<(i32, i32)> {
    let (fx, fy) = from.xy();
    let mut best: Option<(i64, i32, i32)> = None;
    for index in 0..BRICK_COUNT {
        if !state.brick_alive(index) {
            continue;
        }
        let c = state.brick_center(index);
        let dx = (c.x - fx) as i64;
        let dy = (c.y - fy) as i64;
        let d = dx * dx + dy * dy;
        if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
            best = Some((d, c.x, c.y));
        }
    }
    best.map(|(_, x, y)| (x, y))
}

trait HasXy {
    fn xy(&self) -> (i32, i32);
}

impl HasXy for Vec2 {
    fn xy(&self) -> (i32, i32) {
        (self.x, self.y)
    }
}

impl HasXy for PaddleState {
    fn xy(&self) -> (i32, i32) {
        (self.x, self.y)
    }
}
