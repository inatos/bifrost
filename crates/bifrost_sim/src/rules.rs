use crate::fixed::{Vec2, FP_SCALE};
use crate::input::FrameInput;
use serde::{Deserialize, Serialize};

pub const ARENA_W: i32 = 900 * FP_SCALE / 1;
pub const ARENA_H: i32 = 600 * FP_SCALE / 1;
pub const WALL: i32 = 10 * FP_SCALE / 1;
pub const PADDLE_W: i32 = 120 * FP_SCALE / 1;
pub const PADDLE_H: i32 = 16 * FP_SCALE / 1;
pub const BALL_R: i32 = 14 * FP_SCALE / 1;
pub const BRICK_W: i32 = 80 * FP_SCALE / 1;
pub const BRICK_H: i32 = 24 * FP_SCALE / 1;
pub const BRICK_GAP: i32 = 6 * FP_SCALE / 1;
pub const PADDLE_SPEED: i32 = 420 * FP_SCALE / 1000; // per tick at 60hz
pub const BALL_SPEED: i32 = 380 * FP_SCALE / 1000;
pub const GOAL_DEPTH: i32 = 8 * FP_SCALE / 1;
pub const SERVE_FRAMES: u32 = 90;
pub const ROUND_TARGET: u8 = 3;
pub const TICKS_PER_SECOND: u32 = 60;

pub const BRICK_COLS: u8 = 8;
pub const BRICK_ROWS: u8 = 4;
pub const BRICK_COUNT: usize = (BRICK_COLS as usize) * (BRICK_ROWS as usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchPhase {
    Serving,
    Rally,
    GoalPause,
    MatchOver,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaddleState {
    pub x: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BallState {
    pub pos: Vec2,
    pub vel: Vec2,
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
    pub bricks: u64,
    pub score: [u8; 2],
    pub rounds_won: [u8; 2],
}

impl WorldState {
    pub fn new(seed: u64) -> Self {
        let mut state = Self {
            seed,
            frame: 0,
            phase: MatchPhase::Serving,
            serve_timer: SERVE_FRAMES,
            server: 0,
            paddles: [
                PaddleState { x: 0 },
                PaddleState { x: 0 },
            ],
            ball: BallState {
                pos: Vec2::new(0, 0),
                vel: Vec2::ZERO,
            },
            bricks: 0,
            score: [0, 0],
            rounds_won: [0, 0],
        };
        state.reset_round();
        state
    }

    pub fn reset_round(&mut self) {
        self.paddles[0].x = 0;
        self.paddles[1].x = 0;
        self.bricks = default_brick_mask();
        self.score = [0, 0];
        self.phase = MatchPhase::Serving;
        self.serve_timer = SERVE_FRAMES;
        self.place_serve_ball();
    }

    fn place_serve_ball(&mut self) {
        let dir = if self.server == 0 { -1 } else { 1 };
        self.ball.pos = Vec2::new(0, dir * ARENA_H / 6);
        self.ball.vel = Vec2::new(BALL_SPEED / 2, dir * BALL_SPEED).normalize().scale(BALL_SPEED, FP_SCALE);
    }

    pub fn paddle_y(&self, player: usize) -> i32 {
        if player == 0 {
            ARENA_H / 2 - PADDLE_H - GOAL_DEPTH
        } else {
            -ARENA_H / 2 + PADDLE_H + GOAL_DEPTH
        }
    }

    pub fn brick_center(&self, index: usize) -> Vec2 {
        let col = (index % BRICK_COLS as usize) as i32;
        let row = (index / BRICK_COLS as usize) as i32;
        let grid_w = BRICK_COLS as i32 * BRICK_W + (BRICK_COLS as i32 - 1) * BRICK_GAP;
        let start_x = -grid_w / 2 + BRICK_W / 2;
        let start_y = -((BRICK_ROWS as i32 - 1) * (BRICK_H + BRICK_GAP)) / 2
            + row * (BRICK_H + BRICK_GAP);
        Vec2::new(start_x + col * (BRICK_W + BRICK_GAP), start_y)
    }

    pub fn brick_alive(&self, index: usize) -> bool {
        (self.bricks >> index) & 1 == 1
    }

    pub fn break_brick(&mut self, index: usize) {
        self.bricks &= !(1u64 << index);
    }
}

pub fn default_brick_mask() -> u64 {
    (1u64 << BRICK_COUNT) - 1
}

pub fn step(state: &mut WorldState, input: FrameInput) -> crate::StepOutput {
    use crate::ConfirmedEvent;

    let mut events = Vec::new();
    state.frame = state.frame.saturating_add(1);

    match state.phase {
        MatchPhase::MatchOver => return crate::StepOutput { events },
        MatchPhase::Serving => {
            move_paddles(state, &input);
            if state.serve_timer > 0 {
                state.serve_timer -= 1;
            }
            if state.serve_timer == 0 {
                state.phase = MatchPhase::Rally;
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

    move_paddles(state, &input);
    let hit = super::collision::advance_ball(state);
    events.extend(hit.into_iter());

    if let Some(scorer) = super::collision::check_goal(state) {
        state.score[scorer as usize] += 1;
        events.push(ConfirmedEvent::Goal { scorer });

        if state.score[scorer as usize] >= ROUND_TARGET {
            state.rounds_won[scorer as usize] += 1;
            events.push(ConfirmedEvent::RoundWin { winner: scorer });
            if state.rounds_won[scorer as usize] >= 2 {
                state.phase = MatchPhase::MatchOver;
            } else {
                state.reset_round();
                state.server = scorer;
            }
        } else {
            state.phase = MatchPhase::GoalPause;
            state.serve_timer = SERVE_FRAMES;
            state.server = 1 - scorer;
            state.place_serve_ball();
        }
    }

    crate::StepOutput { events }
}

fn move_paddles(state: &mut WorldState, input: &FrameInput) {
    for player in 0..2 {
        let dir = FrameInput::direction_x(input.for_player(player));
        if dir == 0 {
            continue;
        }
        let bound = ARENA_W / 2 - WALL - PADDLE_W / 2;
        let next = state.paddles[player].x + dir * PADDLE_SPEED;
        state.paddles[player].x = next.clamp(-bound, bound);
    }
}
