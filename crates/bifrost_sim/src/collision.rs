use crate::fixed::Vec2;
use crate::rules::{
    WorldState, ARENA_H, ARENA_W, BALL_R, BALL_SPEED, BRICK_COUNT, BRICK_H, BRICK_W, GOAL_DEPTH,
    PADDLE_H, PADDLE_W, WALL,
};
use crate::ConfirmedEvent;

struct Aabb {
    min: Vec2,
    max: Vec2,
}

impl Aabb {
    fn contains_center(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

pub fn advance_ball(state: &mut WorldState) -> Vec<ConfirmedEvent> {
    let mut events = Vec::new();
    let mut pos = state.ball.pos;
    let mut vel = state.ball.vel;

    // Sub-step once per axis in fixed order for stability.
    for _ in 0..2 {
        pos = pos.add(vel);

        // Side walls
        let bound_x = ARENA_W / 2 - WALL - BALL_R;
        if pos.x < -bound_x {
            pos.x = -bound_x;
            vel.x = vel.x.abs();
        } else if pos.x > bound_x {
            pos.x = bound_x;
            vel.x = -vel.x.abs();
        }

        // Paddle collisions (player 0 top, 1 bottom)
        for player in 0..2 {
            let paddle = paddle_aabb(state, player);
            if circle_aabb_hit(pos, BALL_R, &paddle) {
                vel.y = if player == 0 { vel.y.abs() } else { -vel.y.abs() };
                let offset = ((pos.x - state.paddles[player].x) as i64 * BALL_SPEED as i64
                    / (PADDLE_W as i64 / 2)) as i32;
                vel.x = offset.clamp(-BALL_SPEED, BALL_SPEED);
                events.push(ConfirmedEvent::PaddleHit { player: player as u8 });
            }
        }

        // Bricks — iterate in row-major order for determinism
        for index in 0..BRICK_COUNT {
            if !state.brick_alive(index) {
                continue;
            }
            let center = state.brick_center(index);
            let brick = brick_aabb(center);
            if circle_aabb_hit(pos, BALL_R, &brick) {
                state.break_brick(index);
                vel.y = -vel.y;
                events.push(ConfirmedEvent::BrickBreak { index: index as u16 });
                break;
            }
        }
    }

    state.ball.pos = pos;
    state.ball.vel = vel;
    events
}

pub fn check_goal(state: &WorldState) -> Option<u8> {
    let top_goal = ARENA_H / 2 - GOAL_DEPTH;
    let bottom_goal = -ARENA_H / 2 + GOAL_DEPTH;
    if state.ball.pos.y > top_goal + BALL_R {
        return Some(1);
    }
    if state.ball.pos.y < bottom_goal - BALL_R {
        return Some(0);
    }
    None
}

fn paddle_aabb(state: &WorldState, player: usize) -> Aabb {
    let y = state.paddle_y(player);
    Aabb {
        min: Vec2::new(
            state.paddles[player].x - PADDLE_W / 2,
            y - PADDLE_H / 2,
        ),
        max: Vec2::new(
            state.paddles[player].x + PADDLE_W / 2,
            y + PADDLE_H / 2,
        ),
    }
}

fn brick_aabb(center: Vec2) -> Aabb {
    Aabb {
        min: Vec2::new(center.x - BRICK_W / 2, center.y - BRICK_H / 2),
        max: Vec2::new(center.x + BRICK_W / 2, center.y + BRICK_H / 2),
    }
}

fn circle_aabb_hit(center: Vec2, radius: i32, aabb: &Aabb) -> bool {
    let cx = center.x.clamp(aabb.min.x, aabb.max.x);
    let cy = center.y.clamp(aabb.min.y, aabb.max.y);
    let dx = (center.x - cx) as i64;
    let dy = (center.y - cy) as i64;
    let r = radius as i64;
    dx * dx + dy * dy <= r * r
}
