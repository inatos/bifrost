use crate::ConfirmedEvent;
use crate::fixed::{FP_SCALE, Vec2, isqrt};
use crate::paddle_geom::{bounce_ball_off_paddle, circle_hits_paddle, paddle_airborne};
use crate::rules::{
    ARENA_H, ARENA_W, BALL_HIT_KNOCK_DEN, BALL_HIT_KNOCK_NUM, BALL_MAX_SPEED, BALL_R, BALL_SPEED,
    BRICK_COUNT, BRICK_H, BRICK_W, CORNER_R, CORNER_TANGENT_KICK, CORNER_TRAMPOLINE_GAIN,
    CORNER_WALL_KNOCK, GOAL_DEPTH, OWNER_NEUTRAL, PADDLE_KNOCK_MAX, WALL, WALL_KNOCK,
    WILD_BALL_KNOCK, WILD_BRICK_HALF, WorldState,
};
use crate::wild_bricks::circle_hits_wild;

/// Substeps scale with ball speed so 7×+ play doesn't tunnel through bricks.
const BALL_SUBSTEPS: i32 = 8;

pub fn advance_ball(state: &mut WorldState) -> Vec<ConfirmedEvent> {
    let mut events = Vec::new();
    let mut pos = state.ball.pos;
    let mut vel = state.ball.vel;
    let mut paddle_hit = [false; 2];
    // Carry integer remainder so total displacement ≈ one tick of `vel`.
    let mut rem_x = 0i32;
    let mut rem_y = 0i32;

    for _ in 0..BALL_SUBSTEPS {
        let sx = vel.x / BALL_SUBSTEPS;
        let sy = vel.y / BALL_SUBSTEPS;
        rem_x += vel.x - sx * BALL_SUBSTEPS;
        rem_y += vel.y - sy * BALL_SUBSTEPS;
        let mut dx = sx;
        let mut dy = sy;
        if rem_x.abs() >= BALL_SUBSTEPS {
            dx += rem_x / BALL_SUBSTEPS;
            rem_x %= BALL_SUBSTEPS;
        }
        if rem_y.abs() >= BALL_SUBSTEPS {
            dy += rem_y / BALL_SUBSTEPS;
            rem_y %= BALL_SUBSTEPS;
        }
        pos = pos.add(Vec2::new(dx, dy));

        if let Some(corner) = resolve_corner_arcs(&mut pos, &mut vel) {
            events.push(ConfirmedEvent::CornerBounce { corner });
            events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
        }

        let bound_x = ARENA_W / 2 - WALL - BALL_R;
        if !in_corner_arch_zone(pos) {
            if pos.x < -bound_x {
                pos.x = -bound_x;
                bounce_off_wall(&mut vel, -1, 0, BALL_SPEED);
                events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
            } else if pos.x > bound_x {
                pos.x = bound_x;
                bounce_off_wall(&mut vel, 1, 0, BALL_SPEED);
                events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
            }
        }

        for player in 0..2 {
            if paddle_hit[player] || paddle_airborne(&state.paddles[player]) {
                continue;
            }
            let p = &state.paddles[player];
            if circle_hits_paddle(pos, BALL_R, p, player) {
                let impact = isqrt(vel.len_sq()).max(1) as i32;
                let pvx = p.vx;
                let pvy = p.vy;
                bounce_ball_off_paddle(&mut pos, &mut vel, p, player, BALL_SPEED);
                // Readable knockback without runaway dribble teleports.
                let kx = ((vel.x + pvx) * BALL_HIT_KNOCK_NUM / BALL_HIT_KNOCK_DEN)
                    .clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
                let ky = ((impact * BALL_HIT_KNOCK_NUM / (BALL_HIT_KNOCK_DEN * 2))
                    * if player == 0 { 1 } else { -1 })
                .clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
                state.paddles[player].vx =
                    (state.paddles[player].vx + kx).clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
                state.paddles[player].vy =
                    (state.paddles[player].vy + ky).clamp(-PADDLE_KNOCK_MAX, PADDLE_KNOCK_MAX);
                // Ball keeps a satisfying rebound boost from paddle motion.
                vel.x += pvx / 3;
                vel.y += pvy / 4;
                enforce_ball_speed(&mut vel);
                state.ball.owner = player as u8;
                paddle_hit[player] = true;
                events.push(ConfirmedEvent::PaddleHit {
                    player: player as u8,
                });
                events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
            }
        }

        for index in 0..BRICK_COUNT {
            if !state.brick_alive(index) {
                continue;
            }
            let center = state.brick_center(index);
            if circle_hits_aabb(pos, BALL_R, center, BRICK_W, BRICK_H) {
                let tier = state.brick_max_hp[index].max(1) as i32;
                let impact = isqrt(vel.len_sq()).max(BALL_SPEED as i64) as i32;
                let (nx, ny) = brick_normal_for_bounce(pos, vel, center, BRICK_W, BRICK_H);
                reflect_ball_from_brick(
                    &mut pos, &mut vel, nx, ny, center, BRICK_W, BRICK_H, BALL_SPEED,
                );
                // Heavier tiers throw the ball harder (momentum resolve).
                let kick = (impact * tier) / (4 + tier);
                vel.x += nx * kick;
                vel.y += ny * kick;
                enforce_ball_speed(&mut vel);
                if state.ball.owner == OWNER_NEUTRAL {
                    events.push(ConfirmedEvent::BrickBounce {
                        index: index as u16,
                    });
                } else {
                    let scorer = state.ball.owner;
                    let destroyed = state.damage_brick(index);
                    if state.neutralize_ball() {
                        events.push(ConfirmedEvent::BallNeutralized);
                    }
                    if destroyed {
                        if scorer < OWNER_NEUTRAL {
                            state.score[scorer as usize] =
                                state.score[scorer as usize].saturating_add(1);
                        }
                        events.push(ConfirmedEvent::BrickBreak {
                            index: index as u16,
                            scorer,
                        });
                    } else {
                        events.push(ConfirmedEvent::BrickDamage {
                            index: index as u16,
                            hp: state.brick_hp[index],
                        });
                    }
                }
                events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
                break;
            }
        }

        for (wi, w) in state.wild_bricks.iter_mut().enumerate() {
            if !w.active {
                continue;
            }
            if circle_hits_wild(pos, BALL_R, w) {
                let tier = w.hp.max(1) as i32;
                let knock = WILD_BALL_KNOCK + tier * FP_SCALE;
                let center = Vec2::new(w.x, w.y);
                let size = WILD_BRICK_HALF * 2;
                let impact = isqrt(vel.len_sq()).max(1) as i32;
                let (nx, ny) = brick_normal_for_bounce(pos, vel, center, size, size);
                reflect_ball_from_brick(&mut pos, &mut vel, nx, ny, center, size, size, BALL_SPEED);
                let boost = knock / 2 + impact * tier / 6;
                let burst = knock + impact * tier / 4;
                vel.x += nx * (boost + burst / 2);
                vel.y += ny * (boost + burst / 2);
                // Tile recoils opposite the ball — both fly apart.
                w.vx -= nx * burst;
                w.vy -= ny * burst;
                enforce_ball_speed(&mut vel);
                w.hp = w.hp.saturating_sub(1);
                if w.hp == 0 {
                    w.active = false;
                    events.push(ConfirmedEvent::WildBrickBreak { slot: wi as u8 });
                } else {
                    events.push(ConfirmedEvent::WildBrickHit { slot: wi as u8 });
                }
                events.push(ConfirmedEvent::WildBallBurst { slot: wi as u8 });
                events.push(ConfirmedEvent::DustImpact { x: pos.x, y: pos.y });
                if state.ball.owner != OWNER_NEUTRAL {
                    state.ball.owner = OWNER_NEUTRAL;
                    events.push(ConfirmedEvent::BallNeutralized);
                }
                break;
            }
        }

        let bound_y = ARENA_H / 2 - WALL - BALL_R;
        if !in_corner_arch_zone(pos) {
            if pos.y > bound_y {
                pos.y = bound_y - FP_SCALE / 25;
                bounce_off_wall(&mut vel, 0, -1, BALL_SPEED);
            } else if pos.y < -bound_y {
                pos.y = -bound_y + FP_SCALE / 25;
                bounce_off_wall(&mut vel, 0, 1, BALL_SPEED);
            }
        }
    }

    enforce_ball_speed(&mut vel);
    state.ball.pos = pos;
    state.ball.vel = vel;
    events
}

/// Pinball launcher arches — concave quarter-fillets flush with the walls.
/// Ball rides the inside of the arc (rounded-rectangle corner).
/// Returns the corner index (0..=3) when a bounce was applied.
fn resolve_corner_arcs(pos: &mut Vec2, vel: &mut Vec2) -> Option<u8> {
    let hit = resolve_body_corner_arcs(
        &mut pos.x,
        &mut pos.y,
        &mut vel.x,
        &mut vel.y,
        BALL_R,
        CORNER_WALL_KNOCK,
        CORNER_TANGENT_KICK,
    );
    if hit.is_some() {
        enforce_ball_speed(vel);
    }
    hit
}

/// Corner trampoline for any circular body (ball, paddle approx, wild brick).
/// Reflects inbound normal momentum, then adds `base_knock + inbound * GAIN`.
pub(crate) fn resolve_body_corner_arcs(
    pos_x: &mut i32,
    pos_y: &mut i32,
    vel_x: &mut i32,
    vel_y: &mut i32,
    body_r: i32,
    base_knock: i32,
    tangent_kick: i32,
) -> Option<u8> {
    let edge_x = ARENA_W / 2 - WALL;
    let edge_y = ARENA_H / 2 - WALL;
    let r = CORNER_R;
    // Playable side of the arc: body center stays at dist <= r - body_r from C.
    let max_dist = r - body_r;
    if max_dist <= 0 {
        return None;
    }
    let corners = [
        (-edge_x + r, -edge_y + r, -1, -1),
        (edge_x - r, -edge_y + r, 1, -1),
        (-edge_x + r, edge_y - r, -1, 1),
        (edge_x - r, edge_y - r, 1, 1),
    ];
    let max_sq = (max_dist as i64) * (max_dist as i64);
    for (idx, (cx, cy, ox, oy)) in corners.into_iter().enumerate() {
        // Outer corner pocket (toward the wall tip) — where the arch lives.
        if (ox < 0 && *pos_x > cx) || (ox > 0 && *pos_x < cx) {
            continue;
        }
        if (oy < 0 && *pos_y > cy) || (oy > 0 && *pos_y < cy) {
            continue;
        }
        let dx = *pos_x - cx;
        let dy = *pos_y - cy;
        let dist_sq = (dx as i64) * (dx as i64) + (dy as i64) * (dy as i64);
        if dist_sq == 0 || dist_sq <= max_sq {
            continue;
        }
        let dist = isqrt(dist_sq).max(1);
        // Inward (into play) unit from wall: toward C.
        let nx = -((dx as i64 * FP_SCALE as i64) / dist) as i32;
        let ny = -((dy as i64 * FP_SCALE as i64) / dist) as i32;
        *pos_x = cx + ((dx as i64 * max_dist as i64) / dist) as i32;
        *pos_y = cy + ((dy as i64 * max_dist as i64) / dist) as i32;
        let dot = (*vel_x as i64 * nx as i64 + *vel_y as i64 * ny as i64) / FP_SCALE as i64;
        let inbound = if dot < 0 { (-dot) as i32 } else { 0 };
        if dot < 0 {
            *vel_x -= ((2 * dot * nx as i64) / FP_SCALE as i64) as i32;
            *vel_y -= ((2 * dot * ny as i64) / FP_SCALE as i64) as i32;
        }
        let boost = base_knock
            + ((inbound as i64 * CORNER_TRAMPOLINE_GAIN as i64) / FP_SCALE as i64) as i32;
        *vel_x += ((nx as i64 * boost as i64) / FP_SCALE as i64) as i32;
        *vel_y += ((ny as i64 * boost as i64) / FP_SCALE as i64) as i32;
        if tangent_kick != 0 {
            let tx = -ny;
            let ty = nx;
            let tdot = *vel_x as i64 * tx as i64 + *vel_y as i64 * ty as i64;
            let (tx, ty) = if tdot >= 0 { (tx, ty) } else { (-tx, -ty) };
            *vel_x += ((tx as i64 * tangent_kick as i64) / FP_SCALE as i64) as i32;
            *vel_y += ((ty as i64 * tangent_kick as i64) / FP_SCALE as i64) as i32;
        }
        return Some(idx as u8);
    }
    None
}

/// True when the body center is in a corner arch pocket (skip flat wall clamps there).
pub(crate) fn in_corner_arch_zone(pos: Vec2) -> bool {
    let edge_x = ARENA_W / 2 - WALL;
    let edge_y = ARENA_H / 2 - WALL;
    let r = CORNER_R;
    let corners = [
        (-edge_x + r, -edge_y + r, -1, -1),
        (edge_x - r, -edge_y + r, 1, -1),
        (-edge_x + r, edge_y - r, -1, 1),
        (edge_x - r, edge_y - r, 1, 1),
    ];
    for (cx, cy, ox, oy) in corners {
        if (ox < 0 && pos.x > cx) || (ox > 0 && pos.x < cx) {
            continue;
        }
        if (oy < 0 && pos.y > cy) || (oy > 0 && pos.y < cy) {
            continue;
        }
        return true;
    }
    false
}

/// Outward normal (brick → free space). Uses incoming velocity when overlapping the core.
fn brick_normal_for_bounce(pos: Vec2, vel: Vec2, center: Vec2, w: i32, h: i32) -> (i32, i32) {
    let dx = pos.x - center.x;
    let dy = pos.y - center.y;
    let half_w = w / 2;
    let half_h = h / 2;
    let deep = dx.abs() < half_w && dy.abs() < half_h;
    let use_x = if deep {
        vel.x.abs() >= vel.y.abs()
    } else {
        dx.abs() * h > dy.abs() * w
    };
    if use_x {
        let nx = if dx != 0 {
            if dx > 0 { 1 } else { -1 }
        } else if vel.x > 0 {
            -1
        } else {
            1
        };
        (nx, 0)
    } else {
        let ny = if dy != 0 {
            if dy > 0 { 1 } else { -1 }
        } else if vel.y > 0 {
            -1
        } else {
            1
        };
        (0, ny)
    }
}

pub(crate) fn enforce_ball_speed(vel: &mut Vec2) {
    let len = isqrt(vel.len_sq());
    if len == 0 {
        *vel = Vec2::new(0, BALL_SPEED);
        return;
    }
    let target = if len < BALL_SPEED as i64 {
        BALL_SPEED
    } else if len > BALL_MAX_SPEED as i64 {
        BALL_MAX_SPEED
    } else {
        return;
    };
    *vel = vel.normalize().scale(target, FP_SCALE);
}

fn bounce_off_wall(vel: &mut Vec2, nx: i32, ny: i32, min_speed: i32) {
    // Arena wall normals point out of playable space (±1 unit axes).
    let dot = vel.x * nx + vel.y * ny;
    if dot > 0 {
        vel.x -= 2 * nx * dot;
        vel.y -= 2 * ny * dot;
    }
    vel.x += nx * WALL_KNOCK;
    vel.y += ny * WALL_KNOCK;
    if nx != 0 {
        let away = if nx < 0 {
            min_speed / 2
        } else {
            -min_speed / 2
        };
        if vel.x * nx > 0 {
            vel.x = away;
        }
    }
    if ny != 0 {
        let away = if ny < 0 {
            min_speed / 2
        } else {
            -min_speed / 2
        };
        if vel.y * ny > 0 {
            vel.y = away;
        }
    }
    enforce_ball_speed(vel);
}

/// `nx,ny` are outward normals (brick → free space). Separates + reflects into free space.
fn reflect_ball_from_brick(
    pos: &mut Vec2,
    vel: &mut Vec2,
    nx: i32,
    ny: i32,
    center: Vec2,
    w: i32,
    h: i32,
    min_speed: i32,
) {
    let half_w = w / 2;
    let half_h = h / 2;
    if nx != 0 {
        pos.x = center.x + nx * (half_w + BALL_R + FP_SCALE / 30);
        // Approaching brick ⇒ vel · outward_n < 0; flip into free space.
        if vel.x * nx < 0 {
            vel.x = -vel.x;
        }
        if vel.x.abs() < min_speed / 2 {
            vel.x = nx * (min_speed / 2);
        } else if vel.x * nx < 0 {
            vel.x = nx * vel.x.abs();
        }
        vel.x += nx * WALL_KNOCK;
    } else {
        pos.y = center.y + ny * (half_h + BALL_R + FP_SCALE / 30);
        if vel.y * ny < 0 {
            vel.y = -vel.y;
        }
        if vel.y.abs() < min_speed / 2 {
            vel.y = ny * (min_speed / 2);
        } else if vel.y * ny < 0 {
            vel.y = ny * vel.y.abs();
        }
        vel.y += ny * WALL_KNOCK;
    }
    enforce_ball_speed(vel);
}

pub fn check_goal_crossing(state: &WorldState, prev_y: i32) -> Option<u8> {
    let top_goal = ARENA_H / 2 - GOAL_DEPTH;
    let bottom_goal = -ARENA_H / 2 + GOAL_DEPTH;
    if prev_y <= top_goal + BALL_R && state.ball.pos.y > top_goal + BALL_R {
        return Some(0);
    }
    if prev_y >= bottom_goal - BALL_R && state.ball.pos.y < bottom_goal - BALL_R {
        return Some(1);
    }
    None
}

#[allow(dead_code)] // retained for arena variants / future goal modes
pub fn check_goal(state: &WorldState) -> Option<u8> {
    let top_goal = ARENA_H / 2 - GOAL_DEPTH;
    let bottom_goal = -ARENA_H / 2 + GOAL_DEPTH;
    if state.ball.pos.y > top_goal + BALL_R {
        return Some(0);
    }
    if state.ball.pos.y < bottom_goal - BALL_R {
        return Some(1);
    }
    None
}

fn circle_hits_aabb(center: Vec2, radius: i32, brick_center: Vec2, w: i32, h: i32) -> bool {
    let min_x = brick_center.x - w / 2;
    let max_x = brick_center.x + w / 2;
    let min_y = brick_center.y - h / 2;
    let max_y = brick_center.y + h / 2;
    let cx = center.x.clamp(min_x, max_x);
    let cy = center.y.clamp(min_y, max_y);
    let dx = (center.x - cx) as i64;
    let dy = (center.y - cy) as i64;
    let r = radius as i64;
    dx * dx + dy * dy <= r * r
}
