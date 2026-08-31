//! Erratic small wild bricks — deterministic spawn + movement.
//! Paddle shoves apply lasting velocity; wander + mild ball gravity when coasting.

use crate::fixed::FP_SCALE;
use crate::fixed::Vec2;
use crate::rules::{
    ARENA_H, ARENA_W, CORNER_PADDLE_KNOCK, WALL, WALL_KNOCK, WILD_BRICK_HALF, WILD_SPAWN_COOLDOWN,
    WorldState,
};

const COAST_NUM: i32 = 96;
const COAST_DEN: i32 = 100;
const WANDER_SPEED_CAP: i32 = 3 * FP_SCALE;
const BALL_PULL: i32 = FP_SCALE / 5;

pub fn tick_wild(state: &mut WorldState) -> Option<u8> {
    if state.wild_spawn_cd > 0 {
        state.wild_spawn_cd -= 1;
    } else if state.frame % 90 == 0 && rand_pct(state) < 45 {
        let spawned = try_spawn(state);
        if spawned && state.frame % 180 == 0 {
            try_spawn(state);
        }
        state.wild_spawn_cd = WILD_SPAWN_COOLDOWN;
    }

    // Shoved bricks can travel most of the court.
    let bound_x = ARENA_W / 2 - WALL - WILD_BRICK_HALF;
    let bound_y = ARENA_H / 2 - WALL - WILD_BRICK_HALF;
    let bx = state.ball.pos.x;
    let by = state.ball.pos.y;
    let mut corner_hit = None;
    for w in &mut state.wild_bricks {
        if !w.active {
            continue;
        }
        let speed = w.vx.abs().max(w.vy.abs());
        if speed < WANDER_SPEED_CAP {
            let phase = (state.frame + (w.x as u32 >> 3)) % 48;
            // More lively wander than before.
            w.vx = (((phase * 13 + 5) % 17) as i32 - 8) * FP_SCALE / 2;
            w.vy = (((phase * 9 + 7) % 15) as i32 - 7) * FP_SCALE / 2;
            // Small propensity to drift toward the ball.
            let dx = bx - w.x;
            let dy = by - w.y;
            let dist = crate::fixed::isqrt(dx as i64 * dx as i64 + dy as i64 * dy as i64).max(1);
            w.vx += ((dx as i64 * BALL_PULL as i64) / dist) as i32;
            w.vy += ((dy as i64 * BALL_PULL as i64) / dist) as i32;
        } else {
            w.vx = (w.vx as i64 * COAST_NUM as i64 / COAST_DEN as i64) as i32;
            w.vy = (w.vy as i64 * COAST_NUM as i64 / COAST_DEN as i64) as i32;
        }
        w.x += w.vx;
        w.y += w.vy;
        if let Some(corner) = crate::collision::resolve_body_corner_arcs(
            &mut w.x,
            &mut w.y,
            &mut w.vx,
            &mut w.vy,
            WILD_BRICK_HALF,
            CORNER_PADDLE_KNOCK,
            0,
        ) {
            corner_hit = Some(corner);
        }
        let in_corner = crate::collision::in_corner_arch_zone(Vec2::new(w.x, w.y));
        if !in_corner {
            if w.x <= -bound_x {
                w.x = -bound_x;
                w.vx = w.vx.unsigned_abs().max(WALL_KNOCK as u32) as i32;
            } else if w.x >= bound_x {
                w.x = bound_x;
                w.vx = -(w.vx.unsigned_abs().max(WALL_KNOCK as u32) as i32);
            }
            if w.y <= -bound_y {
                w.y = -bound_y;
                w.vy = w.vy.unsigned_abs().max(WALL_KNOCK as u32) as i32;
            } else if w.y >= bound_y {
                w.y = bound_y;
                w.vy = -(w.vy.unsigned_abs().max(WALL_KNOCK as u32) as i32);
            }
        }
        w.x = w.x.clamp(-bound_x, bound_x);
        w.y = w.y.clamp(-bound_y, bound_y);
    }
    corner_hit
}

fn try_spawn(state: &mut WorldState) -> bool {
    for slot in &mut state.wild_bricks {
        if slot.active {
            continue;
        }
        slot.active = true;
        let mix = state
            .seed
            .wrapping_mul(0x85EBCA6B)
            .wrapping_add(state.frame as u64)
            .wrapping_add(state.round_index as u64 * 17);
        slot.hp = match mix % 5 {
            0 => 1,
            1 | 2 => 2,
            _ => 3,
        };
        let r1 = (state.frame as i32 * 73 + state.seed as i32) % (ARENA_W / 2);
        let r2 = (state.frame as i32 * 41 + state.seed as i32) % (ARENA_H / 2);
        slot.x = r1 - ARENA_W / 4;
        slot.y = r2 - ARENA_H / 4;
        slot.vx = 0;
        slot.vy = 0;
        return true;
    }
    false
}

fn rand_pct(state: &WorldState) -> u32 {
    let x = state.seed ^ (state.frame as u64).wrapping_mul(0x9E3779B97F4A7C15);
    (x as u32) % 100
}

pub fn wild_aabb(w: &crate::rules::WildBrick) -> (i32, i32, i32, i32) {
    (
        w.x - WILD_BRICK_HALF,
        w.y - WILD_BRICK_HALF,
        w.x + WILD_BRICK_HALF,
        w.y + WILD_BRICK_HALF,
    )
}

pub fn circle_hits_wild(pos: Vec2, radius: i32, w: &crate::rules::WildBrick) -> bool {
    if !w.active {
        return false;
    }
    let (min_x, min_y, max_x, max_y) = wild_aabb(w);
    let cx = pos.x.clamp(min_x, max_x);
    let cy = pos.y.clamp(min_y, max_y);
    let dx = (pos.x - cx) as i64;
    let dy = (pos.y - cy) as i64;
    let r = radius as i64;
    dx * dx + dy * dy <= r * r
}
