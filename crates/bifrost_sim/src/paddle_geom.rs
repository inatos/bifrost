//! Trapezoid paddle geometry + jump helpers (deterministic).

use crate::fixed::Vec2;
use crate::fixed::FP_SCALE;
use crate::fixed::isqrt;
use crate::rules::{PaddleState, PADDLE_H, PADDLE_W};

/// Narrow width at the backboard edge; full width toward arena center.
pub const PADDLE_W_BACK: i32 = PADDLE_W * 7 / 10;
/// Mario-like arc — readable hang + strong visual scale at apex.
pub const JUMP_INITIAL_V: i32 = 30 * FP_SCALE;
pub const JUMP_GRAVITY: i32 = 16 * FP_SCALE / 10;
pub const JUMP_CLEAR_Z: i32 = 8 * FP_SCALE;
pub const MAX_JUMP_Z: i32 = 42 * FP_SCALE;

pub fn paddle_airborne(p: &PaddleState) -> bool {
    p.jump_z > JUMP_CLEAR_Z
}

/// Diegetic tech: pound anytime while airborne (SM64-style). Higher = stronger.
pub fn can_ground_pound(p: &PaddleState) -> bool {
    !p.ground_pounding && p.jump_z > 0
}

pub fn jump_scale_fixed(z: i32) -> i32 {
    if z <= 0 {
        return FP_SCALE;
    }
    let t = (z as i64 * 1000 / MAX_JUMP_Z as i64).clamp(0, 1000) as i32;
    // Up to +65% size at apex so jumps read clearly in 2D.
    FP_SCALE + (FP_SCALE * 65 / 100) * t / 1000
}

pub fn scaled_half_h(p: &PaddleState) -> i32 {
    (PADDLE_H as i64 * jump_scale_fixed(p.jump_z) as i64 / FP_SCALE as i64) as i32 / 2
}

/// Half-width at local y (−half_h bottom/toward center, +half_h top/back for player 0).
fn half_width_at_local_y(local_y: i32, toward_center_wide: bool) -> i32 {
    let half_h = PADDLE_H / 2;
    let t = ((local_y + half_h) as i64 * 1000 / PADDLE_H as i64).clamp(0, 1000) as i32;
    let narrow = PADDLE_W_BACK / 2;
    let wide = PADDLE_W / 2;
    if toward_center_wide {
        narrow + (wide - narrow) * t / 1000
    } else {
        wide - (wide - narrow) * t / 1000
    }
}

/// Player 0 (top): wide toward center (−y). Player 1 (bottom): wide toward center (+y).
fn toward_center_wide(player: usize) -> bool {
    player == 0
}

/// Integer degrees → (cos, sin) × FP_SCALE. Deterministic lookup (0.25° steps unused — 1°).
pub fn cos_sin_deg(deg: i32) -> (i32, i32) {
    let mut d = deg % 360;
    if d < 0 {
        d += 360;
    }
    let (c, s) = match d {
        0..=90 => cos_sin_q1(d),
        91..=180 => {
            let (c, s) = cos_sin_q1(180 - d);
            (-c, s)
        }
        181..=270 => {
            let (c, s) = cos_sin_q1(d - 180);
            (-c, -s)
        }
        _ => {
            let (c, s) = cos_sin_q1(360 - d);
            (c, -s)
        }
    };
    (c, s)
}

fn cos_sin_q1(deg: i32) -> (i32, i32) {
    // Precomputed cos(d°) * FP_SCALE for d=0..90 (rounded).
    const COS: [i32; 91] = [
        1000, 1000, 999, 999, 998, 996, 995, 993, 990, 988, 985, 982, 978, 974, 970, 966, 961, 956,
        951, 945, 940, 934, 927, 921, 914, 906, 899, 891, 883, 875, 866, 857, 848, 839, 829, 819,
        809, 799, 788, 777, 766, 755, 743, 731, 719, 707, 695, 682, 669, 656, 643, 629, 616, 602,
        588, 574, 559, 545, 530, 515, 500, 485, 469, 454, 438, 423, 407, 391, 375, 358, 342, 326,
        309, 292, 276, 259, 242, 225, 208, 191, 174, 156, 139, 122, 105, 87, 70, 52, 35, 17, 0,
    ];
    let d = deg.clamp(0, 90) as usize;
    (COS[d], COS[90 - d])
}

/// World → paddle-local (accounts for face tilt in degrees × FP_SCALE).
pub fn world_to_paddle_local(pos: Vec2, p: &PaddleState) -> Vec2 {
    let dx = pos.x - p.x;
    let dy = pos.y - p.y;
    let deg = p.angle / FP_SCALE;
    let (c, s) = cos_sin_deg(deg);
    // Inverse rotation R(-θ): [c s; -s c]
    Vec2::new(
        ((dx as i64 * c as i64 + dy as i64 * s as i64) / FP_SCALE as i64) as i32,
        ((-dx as i64 * s as i64 + dy as i64 * c as i64) / FP_SCALE as i64) as i32,
    )
}

fn local_to_world_delta(local: Vec2, p: &PaddleState) -> Vec2 {
    let deg = p.angle / FP_SCALE;
    let (c, s) = cos_sin_deg(deg);
    // R(θ): [c -s; s c]
    Vec2::new(
        ((local.x as i64 * c as i64 - local.y as i64 * s as i64) / FP_SCALE as i64) as i32,
        ((local.x as i64 * s as i64 + local.y as i64 * c as i64) / FP_SCALE as i64) as i32,
    )
}

pub fn circle_hits_paddle(pos: Vec2, radius: i32, p: &PaddleState, player: usize) -> bool {
    let half_h = scaled_half_h(p);
    let local = world_to_paddle_local(pos, p);
    if local.y < -half_h - radius || local.y > half_h + radius {
        return false;
    }
    let hw = half_width_at_local_y(local.y.clamp(-half_h, half_h), toward_center_wide(player));
    local.x.abs() <= hw + radius
}

/// Axis-aligned bounds that cover the rotated paddle (for coarse brick/wild tests).
pub fn paddle_aabb(p: &PaddleState) -> (i32, i32, i32, i32) {
    let half_h = scaled_half_h(p);
    let half_w = PADDLE_W / 2;
    let deg = p.angle / FP_SCALE;
    let (c, s) = cos_sin_deg(deg);
    // Rotate four corners of the unscaled AABB and take extents.
    let corners = [
        (-half_w, -half_h),
        (half_w, -half_h),
        (half_w, half_h),
        (-half_w, half_h),
    ];
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (lx, ly) in corners {
        let wx = ((lx as i64 * c as i64 - ly as i64 * s as i64) / FP_SCALE as i64) as i32;
        let wy = ((lx as i64 * s as i64 + ly as i64 * c as i64) / FP_SCALE as i64) as i32;
        min_x = min_x.min(wx);
        min_y = min_y.min(wy);
        max_x = max_x.max(wx);
        max_y = max_y.max(wy);
    }
    (p.x + min_x, p.y + min_y, p.x + max_x, p.y + max_y)
}

/// Reflect ball using collision normal from trapezoid surface (in paddle-local space).
pub fn bounce_ball_off_paddle(
    pos: &mut Vec2,
    vel: &mut Vec2,
    p: &PaddleState,
    player: usize,
    min_speed: i32,
) {
    let half_h = scaled_half_h(p);
    let local = world_to_paddle_local(*pos, p);
    let local_y = local.y.clamp(-half_h, half_h);
    let hw = half_width_at_local_y(local_y, toward_center_wide(player));
    let local_x = local.x;

    let (nx_l, ny_l) = if local_x > hw {
        (1, 0)
    } else if local_x < -hw {
        (-1, 0)
    } else if local_y > half_h - FP_SCALE / 20 {
        (0, 1)
    } else if local_y < -half_h + FP_SCALE / 20 {
        (0, -1)
    } else {
        let dx = local_x;
        let dy = local_y;
        let len = isqrt((dx as i64 * dx as i64 + dy as i64 * dy as i64) as i64).max(1);
        (
            (dx as i64 * FP_SCALE as i64 / len) as i32,
            (dy as i64 * FP_SCALE as i64 / len) as i32,
        )
    };

    // Local normal → world.
    let n_world = local_to_world_delta(Vec2::new(nx_l, ny_l), p);
    let nx = n_world.x;
    let ny = n_world.y;

    let gap = FP_SCALE / 35;
    let pushed = local_to_world_delta(
        Vec2::new(
            local_x.clamp(-hw, hw) + nx_l * gap,
            local_y.clamp(-half_h, half_h) + ny_l * gap,
        ),
        p,
    );
    pos.x = p.x + pushed.x;
    pos.y = p.y + pushed.y;

    let dot = (vel.x * nx + vel.y * ny) as i64;
    if dot < 0 {
        vel.x -= (2 * dot * nx as i64 / FP_SCALE as i64) as i32;
        vel.y -= (2 * dot * ny as i64 / FP_SCALE as i64) as i32;
    }

    let offset = (local_x as i64 * min_speed as i64 / hw.max(1) as i64) as i32;
    // Bias along paddle tangent (local X → world).
    let tangent = local_to_world_delta(Vec2::new(offset.clamp(-min_speed, min_speed), 0), p);
    vel.x = (vel.x * 2 + tangent.x) / 3;
    vel.y = (vel.y * 2 + tangent.y) / 3;

    // Face normal exit — angle dictates realistic leave trajectory.
    let face = local_to_world_delta(Vec2::new(0, if player == 0 { -1 } else { 1 }), p);
    let leave = min_speed + min_speed / 3;
    vel.x += face.x * leave / FP_SCALE.max(1);
    vel.y += face.y * leave / FP_SCALE.max(1);
    if vel.y.abs() < min_speed / 2 {
        vel.y = if player == 0 {
            -min_speed / 2
        } else {
            min_speed / 2
        };
    }

    // Spring wind-up residual slap — scales with tension charge.
    let strike = p.angle_strike;
    if strike != 0 {
        let kick = face;
        let power = strike.abs() + strike.abs() / 2;
        vel.x += (kick.x as i64 * power as i64 / FP_SCALE as i64) as i32;
        vel.y += (kick.y as i64 * power as i64 / FP_SCALE as i64) as i32;
        // Tangential peel from the wound face.
        let peel = local_to_world_delta(Vec2::new(p.angle.signum() * power / 3, 0), p);
        vel.x += peel.x;
        vel.y += peel.y;
    }

    enforce_min_speed(vel, min_speed);
}

fn enforce_min_speed(vel: &mut Vec2, min: i32) {
    let len_sq = vel.len_sq();
    let min_sq = (min as i64) * (min as i64);
    if len_sq >= min_sq {
        return;
    }
    if len_sq == 0 {
        vel.y = min;
        return;
    }
    let len = isqrt(len_sq).max(1) as i64;
    vel.x = ((vel.x as i64 * min as i64) / len) as i32;
    vel.y = ((vel.y as i64 * min as i64) / len) as i32;
}
