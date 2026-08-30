use crate::fixed::{FP_SCALE, Vec2};
use crate::input::{FrameInput, INPUT_ANGLE_CCW, INPUT_ANGLE_CW, INPUT_JUMP, INPUT_SPIN};
use crate::paddle_geom::{JUMP_GRAVITY, JUMP_INITIAL_V, MAX_JUMP_Z, paddle_airborne};
use serde::{Deserialize, Serialize};

pub const ARENA_W: i32 = 1200 * FP_SCALE / 1;
pub const ARENA_H: i32 = 480 * FP_SCALE / 1;
pub const WALL: i32 = 10 * FP_SCALE / 1;
pub const PADDLE_W: i32 = 120 * FP_SCALE / 1;
pub const PADDLE_H: i32 = 16 * FP_SCALE / 1;
pub const BALL_R: i32 = 14 * FP_SCALE / 1;
pub const BRICK_W: i32 = 80 * FP_SCALE / 1;
pub const BRICK_H: i32 = 24 * FP_SCALE / 1;
pub const BRICK_GAP: i32 = 6 * FP_SCALE / 1;
pub const PADDLE_SPEED: i32 = 18 * FP_SCALE;
pub const BALL_SPEED: i32 = 3640 * FP_SCALE / 1000; // 7× prior 520
pub const GOAL_DEPTH: i32 = 8 * FP_SCALE / 1;
pub const SERVE_FRAMES: u32 = 90;
/// Breaks needed to win a round early (also timeout / board-clear).
pub const ROUND_TARGET: u8 = 3;
pub const TICKS_PER_SECOND: u32 = 60;
pub const PADDLE_PADDLE_KNOCK: i32 = 14 * FP_SCALE;
pub const PADDLE_BRICK_KNOCK: i32 = 8 * FP_SCALE;
pub const PADDLE_WILD_KNOCK: i32 = 11 * FP_SCALE;
pub const WILD_BALL_KNOCK: i32 = 18 * FP_SCALE;
pub const WALL_KNOCK: i32 = 9 * FP_SCALE;
pub const BALL_HIT_KNOCK_NUM: i32 = 48;
pub const BALL_HIT_KNOCK_DEN: i32 = 100;
/// Cap paddle knock velocity so dribble/strike never teleports across the arena.
pub const PADDLE_KNOCK_MAX: i32 = PADDLE_SPEED * 5 / 2;
pub const KNOCK_DECAY_NUM: i32 = 78;
pub const KNOCK_DECAY_DEN: i32 = 100;
/// Ball owner sentinel — purple/neutral, cannot break bricks until paddle reclaim.
pub const OWNER_NEUTRAL: u8 = 2;
/// Pinball-style concave corner arches (rounded-rect fillets).
pub const CORNER_R: i32 = 88 * FP_SCALE;
pub const BALL_MAX_SPEED: i32 = BALL_SPEED + BALL_SPEED / 2;
/// Extra kick along the ramp tangent (kept soft — hit also fires a force wave).
pub const CORNER_TANGENT_KICK: i32 = 5 * FP_SCALE;
/// Wall knock applied only on corner arc contact (softer than flat WALL_KNOCK).
pub const CORNER_WALL_KNOCK: i32 = 4 * FP_SCALE;

pub const BRICK_COLS: u8 = 8;
pub const BRICK_ROWS: u8 = 4;
pub const BRICK_COUNT: usize = (BRICK_COLS as usize) * (BRICK_ROWS as usize);

pub const MAX_WILD_BRICKS: usize = 7;
pub const WILD_BRICK_HALF: i32 = 10 * FP_SCALE / 1;
pub const WILD_SPAWN_COOLDOWN: u32 = 90;
pub const SPIN_CHARGE_MAX: u16 = 90;
pub const SPIN_MAX_KNOCK: i32 = 28 * FP_SCALE;
/// LTTP-style 360° sweep duration (frames @ 60Hz ≈ 0.5s).
pub const SPIN_SWEEP_FRAMES: u16 = 30;
pub const SPIN_SWEEP_RADIUS: i32 = PADDLE_W + PADDLE_H;
/// Slam velocity once ground-pound starts (downward).
pub const GROUND_POUND_V: i32 = -(72 * FP_SCALE);
pub const GROUND_POUND_RADIUS: i32 = PADDLE_W * 3;
pub const GROUND_POUND_KNOCK: i32 = 42 * FP_SCALE;
/// Paddle face tilt (±180°) — full omnidirectional snapback wind.
pub const PADDLE_ANGLE_MAX: i32 = 180 * FP_SCALE;
/// Wind-up rate — snappy charge toward max.
pub const PADDLE_ANGLE_SPEED: i32 = 6 * FP_SCALE;
/// Base spring return; scales up with wind-up (rubber band).
pub const PADDLE_ANGLE_SPRING: i32 = 22 * FP_SCALE;
/// Strike impulse at full wind-up (±180°): hard rubber slap (ball / wave).
pub const PADDLE_ANGLE_STRIKE_MAX: i32 = 58 * FP_SCALE;
/// Snapback force-wave projectile duration (frames @ 60Hz ≈ 2.5s).
pub const ANGLE_WAVE_DURATION: u32 = 150;
pub const ANGLE_WAVE_SPEED: i32 = 14 * FP_SCALE;
/// Soft push when a paddle nests in a corner arch.
pub const CORNER_PADDLE_KNOCK: i32 = 10 * FP_SCALE;
/// One-minute round clock (most brick breaks wins; ties OK).
pub const ROUND_DURATION_FRAMES: u32 = TICKS_PER_SECOND * 60;
/// Occasional corner shockwave.
pub const CORNER_PULSE_COOLDOWN_MIN: u32 = 180;
pub const CORNER_PULSE_COOLDOWN_SPAN: u32 = 240;
pub const CORNER_PULSE_DURATION: u32 = 28;
pub const CORNER_PULSE_RADIUS: i32 = 220 * FP_SCALE;
pub const CORNER_PULSE_KNOCK: i32 = 18 * FP_SCALE;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchPhase {
    /// Both players must ready (jump/confirm) before Serving. CPU always readies.
    Readying,
    Serving,
    Rally,
    GoalPause,
    MatchOver,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaddleState {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub jump_z: i32,
    pub jump_vz: i32,
    /// Spin charge frames (0..SPIN_CHARGE_MAX) while X/RT is held.
    pub spin_charge: u16,
    pub spin_dir_x: i8,
    pub spin_dir_y: i8,
    /// Remaining LTTP sweep frames (0 = idle).
    pub spin_remain: u16,
    /// Sweep angle in degrees × FP_SCALE (0..360).
    pub spin_theta: i32,
    /// Prior-frame jump bit for rising-edge jump / ground-pound.
    pub jump_was_held: bool,
    /// True while slamming down from an apex ground-pound.
    pub ground_pounding: bool,
    /// Paddle face tilt (−PADDLE_ANGLE_MAX..=PADDLE_ANGLE_MAX).
    pub angle: i32,
    /// Prior-frame angle input held (for spring-release strike edge).
    pub angle_was_held: bool,
    /// Latched move/aim dir while winding (−1/0/1); used as beam on release.
    pub snap_aim_x: i8,
    pub snap_aim_y: i8,
    /// One-shot strike impulse residual after spring release (decays).
    pub angle_strike: i32,
    /// Peak jump_z reached this airtime (scales ground-pound).
    pub jump_peak_z: i32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MatchStats {
    pub bricks_broken: [u16; 2],
    pub goals: [u8; 2],
    pub paddle_hits: [u16; 2],
    pub wild_burst: u16,
    pub spins: u16,
    pub longest_rally: u32,
    pub rally_frames: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BallState {
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct WildBrick {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub hp: u8,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldState {
    pub seed: u64,
    pub frame: u32,
    pub phase: MatchPhase,
    pub serve_timer: u32,
    pub server: u8,
    pub paddles: [PaddleState; 2],
    pub ball: BallState,
    pub brick_hp: [u8; BRICK_COUNT],
    /// Per-brick max HP for this match (random tiers from seed).
    pub brick_max_hp: [u8; BRICK_COUNT],
    pub score: [u8; 2],
    pub rounds_won: [u8; 2],
    pub wild_bricks: [WildBrick; MAX_WILD_BRICKS],
    pub wild_spawn_cd: u32,
    /// Increments each round reset so brick tiers reshuffle within a match.
    pub round_index: u32,
    pub stats: MatchStats,
    /// Ready-up latch for Readying phase (player 0 / 1).
    pub ready: [bool; 2],
    /// Frames until the next random corner shockwave may fire.
    pub corner_pulse_cd: u32,
    /// Remaining active frames of a corner pulse (0 = idle).
    pub corner_pulse_t: u32,
    /// Which corner is pulsing: 0=BL 1=BR 2=TL 3=TR.
    pub corner_pulse_id: u8,
    /// Frames remaining in the current round clock (Rally only).
    pub round_timer: u32,
    /// Brick breaks this round (timeout winner / tie).
    pub round_breaks: [u16; 2],
    /// Snapback force-wave projectile (0 = idle).
    pub angle_wave_t: u32,
    pub angle_wave_player: u8,
    pub angle_wave_x: i32,
    pub angle_wave_y: i32,
    pub angle_wave_nx: i32,
    pub angle_wave_ny: i32,
    pub angle_wave_power: i32,
    pub angle_wave_radius: i32,
}

impl WorldState {
    pub fn new(seed: u64) -> Self {
        let mut state = Self {
            seed,
            frame: 0,
            phase: MatchPhase::Readying,
            serve_timer: 0,
            server: 0,
            paddles: [default_paddle(), default_paddle()],
            ball: BallState {
                pos: Vec2::new(0, 0),
                vel: Vec2::ZERO,
                owner: 0,
            },
            brick_hp: [0; BRICK_COUNT],
            brick_max_hp: [0; BRICK_COUNT],
            score: [0, 0],
            rounds_won: [0, 0],
            wild_bricks: [WildBrick::default(); MAX_WILD_BRICKS],
            wild_spawn_cd: 0,
            round_index: 0,
            stats: MatchStats::default(),
            ready: [false, false],
            corner_pulse_cd: CORNER_PULSE_COOLDOWN_MIN,
            corner_pulse_t: 0,
            corner_pulse_id: 0,
            round_timer: ROUND_DURATION_FRAMES,
            round_breaks: [0, 0],
            angle_wave_t: 0,
            angle_wave_player: 0,
            angle_wave_x: 0,
            angle_wave_y: 0,
            angle_wave_nx: 0,
            angle_wave_ny: 0,
            angle_wave_power: 0,
            angle_wave_radius: 0,
        };
        state.reset_round();
        // reset_round enters Serving — re-park in Readying for pre-match.
        state.phase = MatchPhase::Readying;
        state.serve_timer = 0;
        state.ready = [false, false];
        state
    }

    pub fn reset_round(&mut self) {
        let margin = GOAL_DEPTH + PADDLE_H / 2;
        self.paddles[0] = PaddleState {
            x: 0,
            y: ARENA_H / 2 - margin,
            vx: 0,
            vy: 0,
            jump_z: 0,
            jump_vz: 0,
            spin_charge: 0,
            spin_dir_x: 0,
            spin_dir_y: 0,
            spin_remain: 0,
            spin_theta: 0,
            jump_was_held: false,
            ground_pounding: false,
            angle: 0,
            angle_was_held: false,
            snap_aim_x: 0,
            snap_aim_y: 0,
            angle_strike: 0,
            jump_peak_z: 0,
        };
        self.paddles[1] = PaddleState {
            x: 0,
            y: -ARENA_H / 2 + margin,
            vx: 0,
            vy: 0,
            jump_z: 0,
            jump_vz: 0,
            spin_charge: 0,
            spin_dir_x: 0,
            spin_dir_y: 0,
            spin_remain: 0,
            spin_theta: 0,
            jump_was_held: false,
            ground_pounding: false,
            angle: 0,
            angle_was_held: false,
            snap_aim_x: 0,
            snap_aim_y: 0,
            angle_strike: 0,
            jump_peak_z: 0,
        };
        init_brick_hp(
            self.seed,
            self.round_index,
            &mut self.brick_hp,
            &mut self.brick_max_hp,
        );
        self.round_index = self.round_index.saturating_add(1);
        self.wild_bricks = [WildBrick::default(); MAX_WILD_BRICKS];
        self.wild_spawn_cd = 60;
        self.score = [0, 0];
        self.round_breaks = [0, 0];
        self.round_timer = ROUND_DURATION_FRAMES;
        self.angle_wave_t = 0;
        self.angle_wave_power = 0;
        self.phase = MatchPhase::Serving;
        self.serve_timer = SERVE_FRAMES;
        self.place_serve_ball();
    }

    pub fn ball_is_neutral(&self) -> bool {
        self.ball.owner == OWNER_NEUTRAL
    }

    pub fn neutralize_ball(&mut self) -> bool {
        if self.ball.owner == OWNER_NEUTRAL {
            return false;
        }
        self.ball.owner = OWNER_NEUTRAL;
        true
    }

    fn place_serve_ball(&mut self) {
        // Face-off drop zone at center ice (cleared brick lane).
        self.ball.pos = Vec2::ZERO;
        self.ball.vel = Vec2::ZERO;
        self.ball.owner = OWNER_NEUTRAL;
    }

    fn drop_faceoff_ball(&mut self) {
        let dir_y = if self.server == 0 { -1 } else { 1 };
        let dir_x = if (self.frame.wrapping_add(self.seed as u32) % 2) == 0 {
            1
        } else {
            -1
        };
        self.ball.pos = Vec2::ZERO;
        self.ball.vel = Vec2::new(dir_x * BALL_SPEED / 2, dir_y * BALL_SPEED)
            .normalize()
            .scale(BALL_SPEED, FP_SCALE);
        self.ball.owner = self.server;
    }

    pub fn brick_center(&self, index: usize) -> Vec2 {
        let col = (index % BRICK_COLS as usize) as i32;
        let row = (index / BRICK_COLS as usize) as i32;
        let grid_w = BRICK_COLS as i32 * BRICK_W + (BRICK_COLS as i32 - 1) * BRICK_GAP;
        let start_x = -grid_w / 2 + BRICK_W / 2;
        let start_y =
            -((BRICK_ROWS as i32 - 1) * (BRICK_H + BRICK_GAP)) / 2 + row * (BRICK_H + BRICK_GAP);
        Vec2::new(start_x + col * (BRICK_W + BRICK_GAP), start_y)
    }

    pub fn brick_alive(&self, index: usize) -> bool {
        self.brick_hp[index] > 0
    }

    pub fn damage_brick(&mut self, index: usize) -> bool {
        if self.brick_hp[index] == 0 {
            return false;
        }
        self.brick_hp[index] -= 1;
        self.brick_hp[index] == 0
    }
}

fn default_paddle() -> PaddleState {
    PaddleState {
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        jump_z: 0,
        jump_vz: 0,
        spin_charge: 0,
        spin_dir_x: 0,
        spin_dir_y: 0,
        spin_remain: 0,
        spin_theta: 0,
        jump_was_held: false,
        ground_pounding: false,
        angle: 0,
        angle_was_held: false,
        snap_aim_x: 0,
        snap_aim_y: 0,
        angle_strike: 0,
        jump_peak_z: 0,
    }
}

fn init_brick_hp(
    seed: u64,
    round_index: u32,
    hp: &mut [u8; BRICK_COUNT],
    max_hp: &mut [u8; BRICK_COUNT],
) {
    // Hockey-style face-off lane: clear the two center columns.
    let gap_lo = (BRICK_COLS as usize / 2).saturating_sub(1);
    let gap_hi = BRICK_COLS as usize / 2;
    for index in 0..BRICK_COUNT {
        let col = index % BRICK_COLS as usize;
        if col == gap_lo || col == gap_hi {
            max_hp[index] = 0;
            hp[index] = 0;
            continue;
        }
        let mix = seed
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add((index as u64).wrapping_mul(0xC2B2AE3D27D4EB4F))
            .wrapping_add(round_index as u64);
        // Tiers 1..=3, biased slightly toward mid so boards stay readable.
        let tier = match (mix >> 4) % 5 {
            0 => 1,
            1 | 2 => 2,
            _ => 3,
        };
        max_hp[index] = tier;
        hp[index] = tier;
    }
}

pub fn step(state: &mut WorldState, input: FrameInput) -> crate::StepOutput {
    use crate::ConfirmedEvent;

    let mut events = Vec::new();
    state.frame = state.frame.saturating_add(1);

    match state.phase {
        MatchPhase::MatchOver => return crate::StepOutput { events },
        MatchPhase::Readying => {
            for player in 0..2 {
                let mask = input.for_player(player);
                if (mask & INPUT_JUMP) != 0 {
                    state.ready[player] = true;
                }
            }
            // Allow gentle positioning before the serve.
            move_paddles(state, &input, &mut events);
            if state.ready[0] && state.ready[1] {
                // Fresh board + parked paddles once both confirm — sandbox wander is wiped.
                begin_match_from_ready(state);
            }
            return crate::StepOutput { events };
        }
        MatchPhase::Serving => {
            move_paddles(state, &input, &mut events);
            tick_angle_wave(state, &mut events);
            resolve_paddle_corners(state);
            resolve_paddle_collisions(state);
            super::wild_bricks::tick_wild(state);
            if state.serve_timer > 0 {
                state.serve_timer -= 1;
            }
            if state.serve_timer == 0 {
                state.phase = MatchPhase::Rally;
                state.drop_faceoff_ball();
            }
            return crate::StepOutput { events };
        }
        MatchPhase::GoalPause => {
            if state.serve_timer > 0 {
                state.serve_timer -= 1;
            } else {
                state.phase = MatchPhase::Serving;
                state.serve_timer = SERVE_FRAMES;
                state.server = 1 - state.server;
                state.place_serve_ball();
            }
            return crate::StepOutput { events };
        }
        MatchPhase::Rally => {}
    }

    move_paddles(state, &input, &mut events);
    tick_corner_pulses(state, &mut events);
    tick_angle_wave(state, &mut events);
    resolve_paddle_corners(state);
    let prev_ball_y = state.ball.pos.y;
    let hit = super::collision::advance_ball(state);
    for e in &hit {
        if let ConfirmedEvent::CornerBounce { corner } = e {
            fire_corner_hit_pulse(state, &mut events, *corner);
        }
    }
    events.extend(hit.into_iter());
    resolve_paddle_collisions(state);
    events.extend(resolve_paddle_wilds(state));
    super::wild_bricks::tick_wild(state);

    if state.phase == MatchPhase::Rally {
        if state.round_timer > 0 {
            state.round_timer -= 1;
        }
        if let Some(goal_side) = super::collision::check_goal_crossing(state, prev_ball_y) {
            let scorer = match state.ball.owner {
                0 | 1 if state.ball.owner != goal_side => state.ball.owner,
                _ => 1 - goal_side,
            };
            state.score[scorer as usize] += 1;
            events.push(ConfirmedEvent::Goal { scorer });
        }
        try_finish_round(state, &mut events);
        if state.phase == MatchPhase::Rally {
            state.stats.rally_frames = state.stats.rally_frames.saturating_add(1);
            if state.stats.rally_frames > state.stats.longest_rally {
                state.stats.longest_rally = state.stats.rally_frames;
            }
        }
    }

    record_event_stats(state, &events);

    crate::StepOutput { events }
}

fn record_event_stats(state: &mut WorldState, events: &[crate::ConfirmedEvent]) {
    use crate::ConfirmedEvent;
    for event in events {
        match event {
            ConfirmedEvent::BrickBreak { scorer, .. } => {
                if *scorer < OWNER_NEUTRAL {
                    state.stats.bricks_broken[*scorer as usize] =
                        state.stats.bricks_broken[*scorer as usize].saturating_add(1);
                    state.round_breaks[*scorer as usize] =
                        state.round_breaks[*scorer as usize].saturating_add(1);
                }
            }
            ConfirmedEvent::Goal { scorer } => {
                state.stats.goals[*scorer as usize] =
                    state.stats.goals[*scorer as usize].saturating_add(1);
                state.stats.rally_frames = 0;
            }
            ConfirmedEvent::PaddleHit { player } => {
                state.stats.paddle_hits[*player as usize] =
                    state.stats.paddle_hits[*player as usize].saturating_add(1);
            }
            ConfirmedEvent::WildBallBurst { .. } => {
                state.stats.wild_burst = state.stats.wild_burst.saturating_add(1);
            }
            ConfirmedEvent::SpinRelease { player, .. } => {
                state.stats.spins = state.stats.spins.saturating_add(1);
                let _ = player;
            }
            _ => {}
        }
    }
}

fn try_finish_round(state: &mut WorldState, events: &mut Vec<crate::ConfirmedEvent>) {
    if state.phase != MatchPhase::Rally {
        return;
    }
    // Early finish: first to ROUND_TARGET ball breaks.
    if state.round_breaks[0] >= ROUND_TARGET as u16 {
        apply_round_win(state, events, 0);
        return;
    }
    if state.round_breaks[1] >= ROUND_TARGET as u16 {
        apply_round_win(state, events, 1);
        return;
    }
    // Early finish: clear the board (all bricks gone).
    let any_alive = state.brick_hp.iter().any(|&h| h > 0);
    if !any_alive {
        let (b0, b1) = (state.round_breaks[0], state.round_breaks[1]);
        if b0 > b1 {
            apply_round_win(state, events, 0);
        } else if b1 > b0 {
            apply_round_win(state, events, 1);
        } else {
            apply_round_tie(state, events);
        }
        return;
    }
    if state.round_timer > 0 {
        return;
    }
    // Timeout: most breaks wins; equal breaks → tie.
    let (b0, b1) = (state.round_breaks[0], state.round_breaks[1]);
    if b0 > b1 {
        apply_round_win(state, events, 0);
    } else if b1 > b0 {
        apply_round_win(state, events, 1);
    } else {
        apply_round_tie(state, events);
    }
}

fn apply_round_win(state: &mut WorldState, events: &mut Vec<crate::ConfirmedEvent>, scorer: u8) {
    use crate::ConfirmedEvent;
    state.rounds_won[scorer as usize] += 1;
    events.push(ConfirmedEvent::RoundWin { winner: scorer });
    if state.rounds_won[scorer as usize] >= 2 {
        state.phase = MatchPhase::MatchOver;
    } else {
        state.reset_round();
        state.server = scorer;
    }
}

fn apply_round_tie(state: &mut WorldState, events: &mut Vec<crate::ConfirmedEvent>) {
    use crate::ConfirmedEvent;
    events.push(ConfirmedEvent::RoundTie);
    state.reset_round();
}

fn corner_origin(id: u8) -> Vec2 {
    let hx = ARENA_W / 2 - WALL;
    let hy = ARENA_H / 2 - WALL;
    match id {
        0 => Vec2::new(-hx, -hy),
        1 => Vec2::new(hx, -hy),
        2 => Vec2::new(-hx, hy),
        _ => Vec2::new(hx, hy),
    }
}

fn apply_radial_knock(ox: i32, oy: i32, tx: i32, ty: i32, strength: i32) -> (i32, i32) {
    apply_radial_knock_r(ox, oy, tx, ty, strength, CORNER_PULSE_RADIUS)
}

fn apply_radial_knock_r(
    ox: i32,
    oy: i32,
    tx: i32,
    ty: i32,
    strength: i32,
    radius: i32,
) -> (i32, i32) {
    let dx = tx - ox;
    let dy = ty - oy;
    let d2 = dx as i64 * dx as i64 + dy as i64 * dy as i64;
    let r2 = (radius as i64) * (radius as i64);
    if d2 == 0 || d2 > r2 {
        return (0, 0);
    }
    let dist = crate::fixed::isqrt(d2).max(1) as i32;
    let nx = dx * FP_SCALE / dist;
    let ny = dy * FP_SCALE / dist;
    let falloff = ((r2 - d2) * strength as i64 / r2) as i32;
    (nx * falloff / FP_SCALE, ny * falloff / FP_SCALE)
}

/// Active corner hitbox → force wave (replaces/augments rare random pulses).
fn clamp_paddle_knock(p: &mut PaddleState) {
    p.vx = p.vx.clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
    p.vy = p.vy.clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
}

fn begin_match_from_ready(state: &mut WorldState) {
    let margin = GOAL_DEPTH + PADDLE_H / 2;
    state.paddles[0].x = 0;
    state.paddles[0].y = ARENA_H / 2 - margin;
    state.paddles[0].vx = 0;
    state.paddles[0].vy = 0;
    state.paddles[0].jump_z = 0;
    state.paddles[0].jump_vz = 0;
    state.paddles[0].ground_pounding = false;
    state.paddles[0].spin_charge = 0;
    state.paddles[0].spin_remain = 0;
    state.paddles[0].angle = 0;
    state.paddles[0].angle_strike = 0;
    state.paddles[1].x = 0;
    state.paddles[1].y = -ARENA_H / 2 + margin;
    state.paddles[1].vx = 0;
    state.paddles[1].vy = 0;
    state.paddles[1].jump_z = 0;
    state.paddles[1].jump_vz = 0;
    state.paddles[1].ground_pounding = false;
    state.paddles[1].spin_charge = 0;
    state.paddles[1].spin_remain = 0;
    state.paddles[1].angle = 0;
    state.paddles[1].angle_strike = 0;
    init_brick_hp(
        state.seed,
        state.round_index,
        &mut state.brick_hp,
        &mut state.brick_max_hp,
    );
    state.wild_bricks = [WildBrick::default(); MAX_WILD_BRICKS];
    state.angle_wave_t = 0;
    state.angle_wave_power = 0;
    state.round_breaks = [0, 0];
    state.round_timer = ROUND_DURATION_FRAMES;
    state.phase = MatchPhase::Serving;
    state.serve_timer = SERVE_FRAMES;
    state.place_serve_ball();
}

fn tick_angle_wave(state: &mut WorldState, events: &mut Vec<crate::ConfirmedEvent>) {
    use crate::collision::enforce_ball_speed;
    if state.angle_wave_t == 0 {
        return;
    }
    state.angle_wave_t -= 1;
    // nx/ny are FP_SCALE unit vectors — divide once.
    state.angle_wave_x += state.angle_wave_nx * ANGLE_WAVE_SPEED / FP_SCALE.max(1);
    state.angle_wave_y += state.angle_wave_ny * ANGLE_WAVE_SPEED / FP_SCALE.max(1);
    let strength = state.angle_wave_power * (state.angle_wave_t as i32 + 1)
        / ANGLE_WAVE_DURATION.max(1) as i32;
    // Projectile bloom: radius grows as the wave travels, fade via strength.
    let age = ANGLE_WAVE_DURATION.saturating_sub(state.angle_wave_t);
    let radius = state.angle_wave_radius
        + (state.angle_wave_radius as i64 * age as i64 / ANGLE_WAVE_DURATION.max(1) as i64 / 2)
            as i32;
    let ox = state.angle_wave_x;
    let oy = state.angle_wave_y;
    let owner = state.angle_wave_player;

    let (bx, by) =
        apply_radial_knock_r(ox, oy, state.ball.pos.x, state.ball.pos.y, strength, radius);
    if bx != 0 || by != 0 {
        state.ball.vel.x += bx;
        state.ball.vel.y += by;
        enforce_ball_speed(&mut state.ball.vel);
        state.ball.owner = owner;
    }

    let other = 1usize.saturating_sub(owner as usize);
    let other_attacking =
        state.paddles[other].spin_remain > 0 || state.paddles[other].angle_strike > FP_SCALE;
    let (kx, ky) = apply_radial_knock_r(
        ox,
        oy,
        state.paddles[other].x,
        state.paddles[other].y,
        strength,
        radius,
    );
    if kx != 0 || ky != 0 {
        if other_attacking {
            // Snapback vs spin/snapback residual → clang, cancel the beam.
            state.paddles[owner as usize].vx -= state.angle_wave_nx * strength / FP_SCALE.max(1);
            state.paddles[owner as usize].vy -= state.angle_wave_ny * strength / FP_SCALE.max(1);
            state.paddles[other].vx += kx;
            state.paddles[other].vy += ky;
            state.paddles[other].spin_remain = state.paddles[other].spin_remain.min(4);
            state.paddles[other].angle_strike = 0;
            state.paddles[owner as usize].angle_strike = 0;
            clamp_paddle_knock(&mut state.paddles[owner as usize]);
            clamp_paddle_knock(&mut state.paddles[other]);
            state.angle_wave_t = 0;
            events.push(crate::ConfirmedEvent::Clang { x: ox, y: oy });
        } else {
            state.paddles[other].vx += kx;
            state.paddles[other].vy += ky;
            clamp_paddle_knock(&mut state.paddles[other]);
        }
    }

    for w in &mut state.wild_bricks {
        if !w.active {
            continue;
        }
        let (kx, ky) = apply_radial_knock_r(ox, oy, w.x, w.y, strength, radius);
        w.vx += kx;
        w.vy += ky;
    }
}

/// Push paddles out of corner arches (same hitboxes that bounce the ball).
fn resolve_paddle_corners(state: &mut WorldState) {
    let edge_x = ARENA_W / 2 - WALL;
    let edge_y = ARENA_H / 2 - WALL;
    let r = CORNER_R;
    let max_dist = r - PADDLE_H / 2;
    if max_dist <= 0 {
        return;
    }
    let corners = [
        (-edge_x + r, -edge_y + r, -1, -1),
        (edge_x - r, -edge_y + r, 1, -1),
        (-edge_x + r, edge_y - r, -1, 1),
        (edge_x - r, edge_y - r, 1, 1),
    ];
    let max_sq = (max_dist as i64) * (max_dist as i64);
    for p in &mut state.paddles {
        for &(cx, cy, ox, oy) in &corners {
            if (ox < 0 && p.x > cx) || (ox > 0 && p.x < cx) {
                continue;
            }
            if (oy < 0 && p.y > cy) || (oy > 0 && p.y < cy) {
                continue;
            }
            let dx = p.x - cx;
            let dy = p.y - cy;
            let dist_sq = dx as i64 * dx as i64 + dy as i64 * dy as i64;
            if dist_sq == 0 || dist_sq <= max_sq {
                continue;
            }
            let dist = crate::fixed::isqrt(dist_sq).max(1);
            let nx = -((dx as i64 * FP_SCALE as i64) / dist) as i32;
            let ny = -((dy as i64 * FP_SCALE as i64) / dist) as i32;
            p.x = cx + ((dx as i64 * max_dist as i64) / dist) as i32;
            p.y = cy + ((dy as i64 * max_dist as i64) / dist) as i32;
            p.vx += nx * CORNER_PADDLE_KNOCK / FP_SCALE.max(1);
            p.vy += ny * CORNER_PADDLE_KNOCK / FP_SCALE.max(1);
            clamp_paddle_knock(p);
            break;
        }
    }
}

fn fire_corner_hit_pulse(
    state: &mut WorldState,
    events: &mut Vec<crate::ConfirmedEvent>,
    corner: u8,
) {
    use crate::ConfirmedEvent;
    // Don't stack pulses; refresh if already pulsing the same corner.
    if state.corner_pulse_t > 0 && state.corner_pulse_id == corner {
        state.corner_pulse_t = CORNER_PULSE_DURATION;
        return;
    }
    if state.corner_pulse_t > 0 {
        return;
    }
    state.corner_pulse_id = corner.min(3);
    state.corner_pulse_t = CORNER_PULSE_DURATION;
    state.corner_pulse_cd = CORNER_PULSE_COOLDOWN_MIN / 2;
    let origin = corner_origin(state.corner_pulse_id);
    events.push(ConfirmedEvent::CornerPulse {
        corner: state.corner_pulse_id,
        x: origin.x,
        y: origin.y,
    });
}

fn tick_corner_pulses(state: &mut WorldState, events: &mut Vec<crate::ConfirmedEvent>) {
    use crate::ConfirmedEvent;
    use crate::collision::enforce_ball_speed;

    if state.phase != MatchPhase::Rally && state.phase != MatchPhase::Serving {
        return;
    }

    if state.corner_pulse_t > 0 {
        state.corner_pulse_t -= 1;
        let origin = corner_origin(state.corner_pulse_id);
        let strength =
            CORNER_PULSE_KNOCK * (state.corner_pulse_t as i32 + 1) / CORNER_PULSE_DURATION as i32;
        let (bx, by) = apply_radial_knock(
            origin.x,
            origin.y,
            state.ball.pos.x,
            state.ball.pos.y,
            strength,
        );
        state.ball.vel.x += bx;
        state.ball.vel.y += by;
        enforce_ball_speed(&mut state.ball.vel);
        for p in &mut state.paddles {
            let (kx, ky) = apply_radial_knock(origin.x, origin.y, p.x, p.y, strength);
            p.vx += kx;
            p.vy += ky;
        }
        for w in &mut state.wild_bricks {
            if !w.active {
                continue;
            }
            let (kx, ky) = apply_radial_knock(origin.x, origin.y, w.x, w.y, strength);
            w.vx += kx;
            w.vy += ky;
        }
        return;
    }

    if state.corner_pulse_cd > 0 {
        state.corner_pulse_cd -= 1;
        return;
    }

    let mix = state
        .seed
        .wrapping_mul(0xD1B54A32D192ED03)
        .wrapping_add(state.frame as u64);
    if (mix % 7) != 0 {
        state.corner_pulse_cd = 30;
        return;
    }
    state.corner_pulse_id = ((mix >> 8) % 4) as u8;
    state.corner_pulse_t = CORNER_PULSE_DURATION;
    state.corner_pulse_cd =
        CORNER_PULSE_COOLDOWN_MIN + (mix % CORNER_PULSE_COOLDOWN_SPAN as u64) as u32;
    let origin = corner_origin(state.corner_pulse_id);
    events.push(ConfirmedEvent::CornerPulse {
        corner: state.corner_pulse_id,
        x: origin.x,
        y: origin.y,
    });
}

fn move_paddles(
    state: &mut WorldState,
    input: &FrameInput,
    events: &mut Vec<crate::ConfirmedEvent>,
) {
    let bound_x = ARENA_W / 2 - WALL - PADDLE_W / 2;
    let bound_y = ARENA_H / 2 - WALL - PADDLE_H / 2;
    let scrape = FP_SCALE; // near-wall band for skate assist
    for player in 0..2 {
        let mask = input.for_player(player);
        let dx = FrameInput::direction_x(mask);
        let dy = FrameInput::direction_y(mask);
        let mut angle_wave_spawn: Option<(i32, i32, i32, i32, u32)> = None;
        let spin_release = {
            let p = &mut state.paddles[player];
            let jump_down = (mask & INPUT_JUMP) != 0;
            let jump_edge = jump_down && !p.jump_was_held;
            if jump_edge && state.phase != MatchPhase::Readying {
                if p.jump_z == 0 && p.jump_vz == 0 && !p.ground_pounding {
                    p.jump_vz = JUMP_INITIAL_V;
                    p.jump_peak_z = 0;
                    // Preserve / boost horizontal momentum into the leap.
                    if dx != 0 {
                        p.vx += dx * PADDLE_SPEED * 3 / 4;
                    }
                    if dy != 0 {
                        p.vy += dy * PADDLE_SPEED * 3 / 4;
                    }
                    // Keep existing knock/slide momentum rather than hard-resetting.
                    clamp_paddle_knock(p);
                } else if super::paddle_geom::can_ground_pound(p) {
                    // SM64-style: second jump in air → slam (overwrite upward vel).
                    p.ground_pounding = true;
                    p.jump_vz = GROUND_POUND_V;
                    if p.jump_peak_z < p.jump_z {
                        p.jump_peak_z = p.jump_z;
                    }
                }
            }
            p.jump_was_held = jump_down;

            let angle_dir = FrameInput::angle_dir(mask);
            // Diagonals can set both CW+CCW (angle_dir==0) but still count as held.
            let angle_held = (mask & (INPUT_ANGLE_CCW | INPUT_ANGLE_CW)) != 0;
            if angle_held {
                if angle_dir != 0 {
                    p.angle = (p.angle + angle_dir * PADDLE_ANGLE_SPEED)
                        .clamp(-PADDLE_ANGLE_MAX, PADDLE_ANGLE_MAX);
                }
                // Latch stick/cursor/arrow aim while winding (clears on release frame).
                if dx != 0 || dy != 0 {
                    p.snap_aim_x = dx.clamp(-1, 1) as i8;
                    p.snap_aim_y = dy.clamp(-1, 1) as i8;
                }
            } else if p.angle_was_held && p.angle.abs() > FP_SCALE {
                // Release spring → strike residual + outward force-wave projectile.
                let t = p.angle.abs() as i64 * 1000 / PADDLE_ANGLE_MAX.max(1) as i64;
                let tension = (t * t) / 1000;
                let strike = (tension * PADDLE_ANGLE_STRIKE_MAX as i64 / 1000) as i32;
                p.angle_strike = strike.max(PADDLE_ANGLE_STRIKE_MAX / 5);
                let face_y = if player == 0 { -1 } else { 1 };
                let (c, s) = super::paddle_geom::cos_sin_deg(p.angle / FP_SCALE);
                // Fire opposite the release/push vector (stick at 6:00 → beam at 12:00).
                // Prefer latched aim, then same-frame move, else opposite face-normal.
                let (nx, ny) = if p.snap_aim_x != 0 || p.snap_aim_y != 0 {
                    let ax = p.snap_aim_x as i32;
                    let ay = p.snap_aim_y as i32;
                    let len = crate::fixed::isqrt((ax * ax + ay * ay) as i64).max(1) as i32;
                    (-ax * FP_SCALE / len, -ay * FP_SCALE / len)
                } else if dx != 0 || dy != 0 {
                    let len = crate::fixed::isqrt((dx * dx + dy * dy) as i64).max(1) as i32;
                    (-dx * FP_SCALE / len, -dy * FP_SCALE / len)
                } else {
                    (face_y * s, -face_y * c)
                };
                // Blowback opposite the beam, proportional to charge.
                let recoil = (strike as i64 * 7 / 10).max((3 * FP_SCALE) as i64) as i32;
                p.vx -= nx * recoil / FP_SCALE.max(1);
                p.vy -= ny * recoil / FP_SCALE.max(1);
                clamp_paddle_knock(p);
                // Light charges: short reach; full charge: full radius.
                let radius =
                    (PADDLE_W / 10) + (PADDLE_W as i64 * tension * tension / 1_400_000) as i32;
                let power = (PADDLE_ANGLE_STRIKE_MAX / 6)
                    + (tension as i32 * PADDLE_ANGLE_STRIKE_MAX / 1000);
                let wave_life = (ANGLE_WAVE_DURATION as i64 * (180 + tension * 3 / 4) / 1000)
                    .clamp(28, ANGLE_WAVE_DURATION as i64) as u32;
                angle_wave_spawn = Some((nx, ny, radius, power, wave_life));
                p.snap_aim_x = 0;
                p.snap_aim_y = 0;
            } else if p.angle != 0 {
                // Elastic snap: base spring × wind-up (near-full charge returns in ~2–3 frames).
                let wind = p.angle.abs() as i64 * PADDLE_ANGLE_SPRING as i64 * 4
                    / PADDLE_ANGLE_MAX.max(1) as i64;
                let step =
                    (PADDLE_ANGLE_SPRING as i64 + wind).max(PADDLE_ANGLE_SPRING as i64) as i32;
                if p.angle.abs() <= step {
                    p.angle = 0;
                    p.snap_aim_x = 0;
                    p.snap_aim_y = 0;
                } else {
                    p.angle -= p.angle.signum() * step;
                }
            } else {
                p.snap_aim_x = 0;
                p.snap_aim_y = 0;
            }
            p.angle_was_held = angle_held;
            if p.angle_strike > 0 {
                p.angle_strike = (p.angle_strike as i64 * 72 / 100) as i32;
                if p.angle_strike < FP_SCALE / 4 {
                    p.angle_strike = 0;
                }
            }

            let spin_held = (mask & INPUT_SPIN) != 0;
            let mut release = None::<u16>;
            if p.spin_remain > 0 {
                // Mid-sweep: ignore new charge.
            } else if spin_held {
                if p.spin_charge == 0 {
                    let sx = FrameInput::direction_x(mask);
                    let mut sy = FrameInput::direction_y(mask);
                    if sx == 0 && sy == 0 {
                        sy = if player == 0 { -1 } else { 1 };
                    }
                    p.spin_dir_x = sx.clamp(-1, 1) as i8;
                    p.spin_dir_y = sy.clamp(-1, 1) as i8;
                }
                p.spin_charge = p.spin_charge.saturating_add(1).min(SPIN_CHARGE_MAX);
            } else if p.spin_charge > 0 {
                release = Some(p.spin_charge);
                p.spin_charge = 0;
            }
            release
        };
        if let Some((nx, ny, radius, power, wave_life)) = angle_wave_spawn {
            let origin = state.paddles[player];
            state.angle_wave_t = wave_life.max(1);
            state.angle_wave_player = player as u8;
            state.angle_wave_x = origin.x + nx * PADDLE_H / FP_SCALE.max(1);
            state.angle_wave_y = origin.y + ny * PADDLE_H / FP_SCALE.max(1);
            state.angle_wave_nx = nx;
            state.angle_wave_ny = ny;
            state.angle_wave_power = power;
            state.angle_wave_radius = radius;
            events.push(crate::ConfirmedEvent::AngleWave {
                player: player as u8,
                x: state.angle_wave_x,
                y: state.angle_wave_y,
                nx,
                ny,
                power,
                radius,
            });
        }
        if let Some(charge) = spin_release {
            release_spin(state, player, charge, events);
        }
        tick_spin_sweep(state, player, events);

        let (pounded, pound_peak) = {
            let p = &mut state.paddles[player];
            let jump_held = (mask & INPUT_JUMP) != 0;

            // Aim cardinals ride on move bits while winding — don't walk the paddle.
            let winding = (mask & (INPUT_ANGLE_CCW | INPUT_ANGLE_CW)) != 0;
            if !winding {
                if dx != 0 {
                    p.x += dx * PADDLE_SPEED;
                }
                if dy != 0 {
                    p.y += dy * PADDLE_SPEED;
                }
            }
            p.x += p.vx;
            p.y += p.vy;
            p.vx = (p.vx as i64 * KNOCK_DECAY_NUM as i64 / KNOCK_DECAY_DEN as i64) as i32;
            p.vy = (p.vy as i64 * KNOCK_DECAY_NUM as i64 / KNOCK_DECAY_DEN as i64) as i32;
            clamp_paddle_knock(p);
            if p.vx.abs() < FP_SCALE / 50 {
                p.vx = 0;
            }
            if p.vy.abs() < FP_SCALE / 50 {
                p.vy = 0;
            }

            let mut pounded = false;
            let mut pound_peak = 0;
            if p.jump_vz != 0 || p.jump_z > 0 {
                // SM64 variable jump: hold jump while rising → float higher.
                let hold_float = jump_held && p.jump_vz > 0 && !p.ground_pounding;
                if hold_float {
                    p.jump_vz -= JUMP_GRAVITY / 2;
                } else {
                    p.jump_vz -= JUMP_GRAVITY;
                }
                if p.ground_pounding {
                    // Faster slam.
                    p.jump_vz -= JUMP_GRAVITY;
                }
                p.jump_z += p.jump_vz;
                if p.jump_z > p.jump_peak_z {
                    p.jump_peak_z = p.jump_z;
                }
                if p.jump_z <= 0 {
                    pounded = p.ground_pounding;
                    pound_peak = p.jump_peak_z;
                    p.jump_z = 0;
                    p.jump_vz = 0;
                    p.ground_pounding = false;
                    p.jump_peak_z = 0;
                } else if p.jump_z > MAX_JUMP_Z {
                    p.jump_z = MAX_JUMP_Z;
                    if p.jump_vz > 0 {
                        p.jump_vz = 0;
                    }
                }
            }

            // Wall scrape → skate: into-wall thrust becomes a little parallel slide.
            let at_left = p.x <= -bound_x + scrape;
            let at_right = p.x >= bound_x - scrape;
            let at_bot = p.y <= -bound_y + scrape;
            let at_top = p.y >= bound_y - scrape;
            if at_left && dx < 0 && dy != 0 {
                p.y += dy * (PADDLE_SPEED / 3);
            } else if at_right && dx > 0 && dy != 0 {
                p.y += dy * (PADDLE_SPEED / 3);
            }
            if at_bot && dy < 0 && dx != 0 {
                p.x += dx * (PADDLE_SPEED / 3);
            } else if at_top && dy > 0 && dx != 0 {
                p.x += dx * (PADDLE_SPEED / 3);
            }

            // Soft clamp — kill into-wall velocity, keep along-wall slide.
            if p.x < -bound_x {
                p.x = -bound_x;
                if p.vx < 0 {
                    p.vx = 0;
                }
            } else if p.x > bound_x {
                p.x = bound_x;
                if p.vx > 0 {
                    p.vx = 0;
                }
            }
            if p.y < -bound_y {
                p.y = -bound_y;
                if p.vy < 0 {
                    p.vy = 0;
                }
            } else if p.y > bound_y {
                p.y = bound_y;
                if p.vy > 0 {
                    p.vy = 0;
                }
            }
            (pounded, pound_peak)
        };

        if pounded {
            apply_ground_pound(state, player, pound_peak, events);
        }
    }
}

fn release_spin(
    state: &mut WorldState,
    player: usize,
    charge: u16,
    events: &mut Vec<crate::ConfirmedEvent>,
) {
    use crate::ConfirmedEvent;
    // Floor so a quick tap still reads as a spin (LTTP charged spin).
    let charge = charge.max(18);
    let p = &mut state.paddles[player];
    // Longer charge → slightly longer / stronger sweep.
    let bonus = ((charge as u32 * 12) / SPIN_CHARGE_MAX as u32) as u16;
    p.spin_remain = SPIN_SWEEP_FRAMES + bonus;
    p.spin_theta = 0;
    events.push(ConfirmedEvent::SpinRelease {
        player: player as u8,
        charge,
    });
}

fn tick_spin_sweep(state: &mut WorldState, player: usize, events: &mut Vec<crate::ConfirmedEvent>) {
    use crate::ConfirmedEvent;
    use crate::collision::enforce_ball_speed;
    if state.paddles[player].spin_remain == 0 {
        return;
    }
    let charge_scale = {
        let rem = state.paddles[player].spin_remain;
        let total = SPIN_SWEEP_FRAMES.max(1);
        // Front-load power early in the sweep like LTTP.
        ((rem as i32 * SPIN_MAX_KNOCK) / total as i32).max(SPIN_MAX_KNOCK / 3)
    };
    {
        let p = &mut state.paddles[player];
        let step_deg = (360 * FP_SCALE) / SPIN_SWEEP_FRAMES.max(1) as i32;
        p.spin_theta = (p.spin_theta + step_deg) % (360 * FP_SCALE);
        p.spin_remain = p.spin_remain.saturating_sub(1);
    }
    let origin = state.paddles[player];
    let reach = SPIN_SWEEP_RADIUS as i64;
    let dx = state.ball.pos.x - origin.x;
    let dy = state.ball.pos.y - origin.y;
    if dx as i64 * dx as i64 + dy as i64 * dy as i64 <= reach * reach {
        // Tangential shove around the paddle (sword arc).
        let (c, s) = super::paddle_geom::cos_sin_deg(origin.spin_theta / FP_SCALE);
        let tx = -s; // tangent of sweep circle
        let ty = c;
        state.ball.vel.x += (tx as i64 * charge_scale as i64 / FP_SCALE as i64) as i32;
        state.ball.vel.y += (ty as i64 * charge_scale as i64 / FP_SCALE as i64) as i32;
        // Also push outward a little.
        if dx != 0 || dy != 0 {
            let len = crate::fixed::isqrt(dx as i64 * dx as i64 + dy as i64 * dy as i64).max(1);
            state.ball.vel.x += ((dx as i64 * charge_scale as i64 / 2) / len) as i32;
            state.ball.vel.y += ((dy as i64 * charge_scale as i64 / 2) / len) as i32;
        }
        enforce_ball_speed(&mut state.ball.vel);
        state.ball.owner = player as u8;
        events.push(ConfirmedEvent::PaddleHit {
            player: player as u8,
        });
    }
    // Bricks are ball-only — spin never damages the board.
}

fn apply_ground_pound(
    state: &mut WorldState,
    player: usize,
    peak_z: i32,
    events: &mut Vec<crate::ConfirmedEvent>,
) {
    use crate::ConfirmedEvent;
    use crate::collision::enforce_ball_speed;
    let origin = state.paddles[player];
    events.push(ConfirmedEvent::GroundPound {
        player: player as u8,
        x: origin.x,
        y: origin.y,
    });

    // Higher peak → larger AoE and stronger knock (SM64 feel).
    let height_t = (peak_z as i64 * 1000 / MAX_JUMP_Z.max(1) as i64).clamp(250, 1000);
    let radius = (GROUND_POUND_RADIUS as i64 * (700 + height_t * 3 / 10) / 1000) as i32;
    let knock = (GROUND_POUND_KNOCK as i64 * (550 + height_t * 45 / 100) / 1000) as i32;
    let r2 = (radius as i64) * (radius as i64);

    let dx = state.ball.pos.x - origin.x;
    let dy = state.ball.pos.y - origin.y;
    let d2 = dx as i64 * dx as i64 + dy as i64 * dy as i64;
    if d2 <= r2 && d2 > 0 {
        let dist = crate::fixed::isqrt(d2).max(1) as i32;
        let nx = dx * FP_SCALE / dist;
        let ny = dy * FP_SCALE / dist;
        let falloff = ((r2 - d2) * knock as i64 / r2) as i32;
        state.ball.vel.x += nx * falloff / FP_SCALE;
        state.ball.vel.y += ny * falloff / FP_SCALE;
        enforce_ball_speed(&mut state.ball.vel);
        state.ball.owner = player as u8;
    }

    let other = 1 - player;
    let ox = state.paddles[other].x - origin.x;
    let oy = state.paddles[other].y - origin.y;
    let od2 = ox as i64 * ox as i64 + oy as i64 * oy as i64;
    if od2 <= r2 && od2 > 0 {
        let dist = crate::fixed::isqrt(od2).max(1) as i32;
        let nx = ox * FP_SCALE / dist;
        let ny = oy * FP_SCALE / dist;
        let falloff = ((r2 - od2) * knock as i64 / r2) as i32;
        state.paddles[other].vx += nx * falloff / FP_SCALE;
        state.paddles[other].vy += ny * falloff / FP_SCALE;
        clamp_paddle_knock(&mut state.paddles[other]);
    }

    for slot in 0..MAX_WILD_BRICKS {
        let w = &mut state.wild_bricks[slot];
        if !w.active {
            continue;
        }
        let wx = w.x - origin.x;
        let wy = w.y - origin.y;
        let wd2 = wx as i64 * wx as i64 + wy as i64 * wy as i64;
        if wd2 <= r2 && wd2 > 0 {
            let dist = crate::fixed::isqrt(wd2).max(1) as i32;
            let nx = wx * FP_SCALE / dist;
            let ny = wy * FP_SCALE / dist;
            let falloff = ((r2 - wd2) * knock as i64 / r2) as i32;
            w.vx += nx * falloff / FP_SCALE;
            w.vy += ny * falloff / FP_SCALE;
        }
    }
}

fn resolve_paddle_collisions(state: &mut WorldState) {
    resolve_paddle_paddle(state);
    resolve_paddle_bricks(state);
}

fn resolve_paddle_wilds(state: &mut WorldState) -> Vec<crate::ConfirmedEvent> {
    use crate::ConfirmedEvent;
    let mut events = Vec::new();
    for player in 0..2 {
        // Airborne paddles can still slam / shoot wild tiles.
        let p = state.paddles[player];
        let (p_min_x, p_min_y, p_max_x, p_max_y) = super::paddle_geom::paddle_aabb(&p);
        for (slot, w) in state.wild_bricks.iter_mut().enumerate() {
            if !w.active {
                continue;
            }
            let (w_min_x, w_min_y, w_max_x, w_max_y) = super::wild_bricks::wild_aabb(w);
            let (ox, oy) = aabb_overlap(
                p_min_x, p_min_y, p_max_x, p_max_y, w_min_x, w_min_y, w_max_x, w_max_y,
            );
            if ox == 0 || oy == 0 {
                continue;
            }
            let tier = w.hp.max(1).min(3);
            let impact = (p.vx.abs() + p.vy.abs()).max(PADDLE_SPEED / 2);
            let sep_x = if p.x <= w.x { -1 } else { 1 };
            let sep_y = if p.y <= w.y { -1 } else { 1 };

            // Separate paddle from brick.
            if ox <= oy {
                state.paddles[player].x += sep_x * ox;
            } else {
                state.paddles[player].y += sep_y * oy;
            }

            // Tiered shove / shoot — momentum goes INTO the wild tile.
            match tier {
                1 => {
                    // Glass: easy launch, light paddle recoil.
                    let launch = impact * 3 / 2 + PADDLE_WILD_KNOCK;
                    w.vx += -sep_x * launch;
                    w.vy += -sep_y * launch / 2;
                    state.paddles[player].vx += sep_x * (PADDLE_WILD_KNOCK / 3);
                    state.paddles[player].vy += sep_y * (PADDLE_WILD_KNOCK / 4);
                }
                2 => {
                    // Sticky: damp paddle, modest shove + lateral smear.
                    let launch = impact + PADDLE_WILD_KNOCK / 2;
                    w.vx += -sep_x * launch + p.vx / 2;
                    w.vy += -sep_y * launch / 2 + p.vy / 3;
                    state.paddles[player].vx =
                        state.paddles[player].vx * 45 / 100 + sep_x * PADDLE_WILD_KNOCK / 2;
                    state.paddles[player].vy =
                        state.paddles[player].vy * 45 / 100 + sep_y * PADDLE_WILD_KNOCK / 3;
                }
                _ => {
                    // Heavy: high-speed contact shoots toward the opponent goal.
                    let shoot_thresh = PADDLE_SPEED + PADDLE_SPEED / 2;
                    if impact >= shoot_thresh || paddle_airborne(&p) {
                        let toward_opp = if player == 0 { -1 } else { 1 };
                        let shot = PADDLE_WILD_KNOCK * 3 + impact;
                        w.vx = p.vx / 2 + -sep_x * shot / 2;
                        w.vy = toward_opp * shot;
                    } else {
                        let launch = impact + PADDLE_WILD_KNOCK;
                        w.vx += -sep_x * launch / 2 + p.vx / 3;
                        w.vy += -sep_y * launch / 2 + p.vy / 3;
                    }
                    state.paddles[player].vx += sep_x * PADDLE_WILD_KNOCK;
                    state.paddles[player].vy += sep_y * PADDLE_WILD_KNOCK;
                }
            }

            events.push(ConfirmedEvent::WildPaddleKnock {
                player: player as u8,
                slot: slot as u8,
            });
        }
        clamp_paddles(state);
    }
    events
}

fn resolve_paddle_paddle(state: &mut WorldState) {
    let (a_min_x, a_min_y, a_max_x, a_max_y) = super::paddle_geom::paddle_aabb(&state.paddles[0]);
    let (b_min_x, b_min_y, b_max_x, b_max_y) = super::paddle_geom::paddle_aabb(&state.paddles[1]);
    let (ox, oy) = aabb_overlap(
        a_min_x, a_min_y, a_max_x, a_max_y, b_min_x, b_min_y, b_max_x, b_max_y,
    );
    if ox == 0 || oy == 0 {
        return;
    }
    let a_wave = state.angle_wave_t > 0 && state.angle_wave_player == 0;
    let b_wave = state.angle_wave_t > 0 && state.angle_wave_player == 1;
    let a_attack =
        state.paddles[0].spin_remain > 0 || state.paddles[0].angle_strike > FP_SCALE || a_wave;
    let b_attack =
        state.paddles[1].spin_remain > 0 || state.paddles[1].angle_strike > FP_SCALE || b_wave;
    let clang = a_attack && b_attack;

    // Attack clang (spin / snapback) works airborne; body checks stay grounded.
    if !clang && (paddle_airborne(&state.paddles[0]) || paddle_airborne(&state.paddles[1])) {
        return;
    }

    let rel_x = (state.paddles[0].vx - state.paddles[1].vx).abs();
    let rel_y = (state.paddles[0].vy - state.paddles[1].vy).abs();
    let knock = if clang {
        PADDLE_PADDLE_KNOCK * 2 + PADDLE_ANGLE_STRIKE_MAX / 8
    } else {
        PADDLE_PADDLE_KNOCK
    };

    if ox <= oy {
        let push = (ox + 1) / 2;
        let dir = if state.paddles[0].x <= state.paddles[1].x {
            -1
        } else {
            1
        };
        let impulse = knock + rel_x / 2;
        state.paddles[0].x += dir * push;
        state.paddles[1].x -= dir * push;
        state.paddles[0].vx += dir * impulse;
        state.paddles[1].vx -= dir * impulse;
        if !clang {
            state.paddles[0].vx += dir * PADDLE_PADDLE_KNOCK / 3;
            state.paddles[0].vy += if state.paddles[1].y > state.paddles[0].y {
                PADDLE_PADDLE_KNOCK / 4
            } else {
                -PADDLE_PADDLE_KNOCK / 4
            };
        }
    } else {
        let push = (oy + 1) / 2;
        let dir = if state.paddles[0].y <= state.paddles[1].y {
            -1
        } else {
            1
        };
        let impulse = knock + rel_y / 2;
        state.paddles[0].y += dir * push;
        state.paddles[1].y -= dir * push;
        state.paddles[0].vy += dir * impulse;
        state.paddles[1].vy -= dir * impulse;
        if !clang {
            state.paddles[0].vx += if state.paddles[1].x > state.paddles[0].x {
                PADDLE_PADDLE_KNOCK / 4
            } else {
                -PADDLE_PADDLE_KNOCK / 4
            };
            state.paddles[0].vy += dir * PADDLE_PADDLE_KNOCK / 3;
        }
    }
    if clang {
        state.paddles[0].angle_strike = 0;
        state.paddles[1].angle_strike = 0;
        state.paddles[0].spin_remain = state.paddles[0].spin_remain.min(4);
        state.paddles[1].spin_remain = state.paddles[1].spin_remain.min(4);
        state.angle_wave_t = 0;
    }
    clamp_paddles(state);
}

fn resolve_paddle_bricks(state: &mut WorldState) {
    for player in 0..2 {
        if paddle_airborne(&state.paddles[player]) {
            continue;
        }
        let p = state.paddles[player];
        let (p_min_x, p_min_y, p_max_x, p_max_y) = super::paddle_geom::paddle_aabb(&p);
        for index in 0..BRICK_COUNT {
            if !state.brick_alive(index) {
                continue;
            }
            let c = state.brick_center(index);
            let (ox, oy) = aabb_overlap(
                p_min_x,
                p_min_y,
                p_max_x,
                p_max_y,
                c.x - BRICK_W / 2,
                c.y - BRICK_H / 2,
                c.x + BRICK_W / 2,
                c.y + BRICK_H / 2,
            );
            if ox == 0 || oy == 0 {
                continue;
            }
            let tier = state.brick_max_hp[index].max(1) as i32;
            let knock = PADDLE_BRICK_KNOCK * tier / 2 + PADDLE_BRICK_KNOCK / 2;
            let rel = if ox <= oy {
                (p.vx - 0).abs()
            } else {
                (p.vy - 0).abs()
            };
            let impulse = knock + rel * tier / 4;
            if ox <= oy {
                let dir = if p.x <= c.x { -1 } else { 1 };
                state.paddles[player].x += dir * ox;
                state.paddles[player].vx += dir * impulse;
            } else {
                let dir = if p.y <= c.y { -1 } else { 1 };
                state.paddles[player].y += dir * oy;
                state.paddles[player].vy += dir * impulse;
            }
        }
        clamp_paddles(state);
    }
}

fn clamp_paddles(state: &mut WorldState) {
    let bound_x = ARENA_W / 2 - WALL - PADDLE_W / 2;
    let bound_y = ARENA_H / 2 - WALL - PADDLE_H / 2;
    for p in &mut state.paddles {
        if p.x <= -bound_x {
            p.x = -bound_x;
            if p.vx < 0 {
                p.vx = 0;
            }
        } else if p.x >= bound_x {
            p.x = bound_x;
            if p.vx > 0 {
                p.vx = 0;
            }
        }
        if p.y <= -bound_y {
            p.y = -bound_y;
            if p.vy < 0 {
                p.vy = 0;
            }
        } else if p.y >= bound_y {
            p.y = bound_y;
            if p.vy > 0 {
                p.vy = 0;
            }
        }
        p.x = p.x.clamp(-bound_x, bound_x);
        p.y = p.y.clamp(-bound_y, bound_y);
    }
}

fn aabb_overlap(
    a_min_x: i32,
    a_min_y: i32,
    a_max_x: i32,
    a_max_y: i32,
    b_min_x: i32,
    b_min_y: i32,
    b_max_x: i32,
    b_max_y: i32,
) -> (i32, i32) {
    let ox = (a_max_x.min(b_max_x) - a_min_x.max(b_min_x)).max(0);
    let oy = (a_max_y.min(b_max_y) - a_min_y.max(b_min_y)).max(0);
    (ox, oy)
}
