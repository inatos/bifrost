//! Interpolation between fixed sim ticks for smooth visuals.

use bevy::prelude::*;
use bifrost_sim::{FP_SCALE, MatchPhase, PaddleState, WildBrick, WorldState, BRICK_COUNT, MAX_WILD_BRICKS};

const ARENA_W: f32 = 1200.0;
const ARENA_H: f32 = 480.0;

#[derive(Clone, Debug)]
pub struct VisualSnapshot {
    pub paddles: [PaddleState; 2],
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_owner: u8,
    pub brick_hp: [u8; BRICK_COUNT],
    pub brick_max_hp: [u8; BRICK_COUNT],
    pub wild_bricks: [WildBrick; MAX_WILD_BRICKS],
    pub score: [u8; 2],
    pub rounds_won: [u8; 2],
    pub phase: MatchPhase,
}

#[derive(Resource, Default)]
pub struct InterpState {
    pub prev: VisualSnapshot,
    pub curr: VisualSnapshot,
    pub initialized: bool,
}

impl InterpState {
    pub fn reset_from(&mut self, world: &WorldState) {
        let snap = VisualSnapshot::from_world(world);
        self.prev = snap.clone();
        self.curr = snap;
        self.initialized = true;
    }

    pub fn advance(&mut self, world: &WorldState) {
        if !self.initialized {
            self.reset_from(world);
            return;
        }
        self.prev = self.curr.clone();
        self.curr = VisualSnapshot::from_world(world);
    }

    pub fn sample(&self, alpha: f32) -> VisualSnapshot {
        let t = alpha.clamp(0.0, 1.0);
        VisualSnapshot {
            paddles: [
                lerp_paddle(&self.prev.paddles[0], &self.curr.paddles[0], t),
                lerp_paddle(&self.prev.paddles[1], &self.curr.paddles[1], t),
            ],
            ball_x: lerp_f32(self.prev.ball_x, self.curr.ball_x, t),
            ball_y: lerp_f32(self.prev.ball_y, self.curr.ball_y, t),
            ball_owner: self.curr.ball_owner,
            brick_hp: self.curr.brick_hp,
            brick_max_hp: self.curr.brick_max_hp,
            wild_bricks: self.curr.wild_bricks,
            score: self.curr.score,
            rounds_won: self.curr.rounds_won,
            phase: self.curr.phase,
        }
    }
}

impl Default for VisualSnapshot {
    fn default() -> Self {
        Self {
            paddles: [default_paddle(), default_paddle()],
            ball_x: 0.0,
            ball_y: 0.0,
            ball_owner: 0,
            brick_hp: [0; BRICK_COUNT],
            brick_max_hp: [0; BRICK_COUNT],
            wild_bricks: [WildBrick::default(); MAX_WILD_BRICKS],
            score: [0, 0],
            rounds_won: [0, 0],
            phase: MatchPhase::Serving,
        }
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
        angle_strike: 0,
        jump_peak_z: 0,
    }
}

impl VisualSnapshot {
    pub fn from_world(world: &WorldState) -> Self {
        let (bx, by) = world.ball.pos.to_f();
        Self {
            paddles: world.paddles,
            ball_x: bx,
            ball_y: by,
            ball_owner: world.ball.owner,
            brick_hp: world.brick_hp,
            brick_max_hp: world.brick_max_hp,
            wild_bricks: world.wild_bricks,
            score: world.score,
            rounds_won: world.rounds_won,
            phase: world.phase,
        }
    }

    pub fn paddle_world(&self, player: usize) -> (f32, f32, f32) {
        let p = self.paddles[player];
        (
            p.x as f32 / FP_SCALE as f32,
            p.y as f32 / FP_SCALE as f32,
            p.jump_z as f32 / FP_SCALE as f32,
        )
    }

    pub fn arena_norm(&self, wx: f32, wy: f32) -> (f32, f32) {
        let nx = ((wx + ARENA_W / 2.0) / ARENA_W).clamp(0.04, 0.96);
        let ny = (1.0 - (wy + ARENA_H / 2.0) / ARENA_H).clamp(0.04, 0.96);
        (nx, ny)
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_paddle(a: &PaddleState, b: &PaddleState, t: f32) -> PaddleState {
    PaddleState {
        x: lerp_i32(a.x, b.x, t),
        y: lerp_i32(a.y, b.y, t),
        vx: b.vx,
        vy: b.vy,
        jump_z: lerp_i32(a.jump_z, b.jump_z, t),
        jump_vz: b.jump_vz,
        spin_charge: b.spin_charge,
        spin_dir_x: b.spin_dir_x,
        spin_dir_y: b.spin_dir_y,
        spin_remain: b.spin_remain,
        spin_theta: lerp_i32(a.spin_theta, b.spin_theta, t),
        jump_was_held: b.jump_was_held,
        ground_pounding: b.ground_pounding,
        angle: lerp_i32(a.angle, b.angle, t),
        angle_was_held: b.angle_was_held,
        angle_strike: lerp_i32(a.angle_strike, b.angle_strike, t),
        jump_peak_z: lerp_i32(a.jump_peak_z, b.jump_peak_z, t),
    }
}

fn lerp_i32(a: i32, b: i32, t: f32) -> i32 {
    a + ((b - a) as f32 * t).round() as i32
}
