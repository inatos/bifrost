use crate::input::FrameInput;
use crate::{
    BRICK_COLS, BRICK_COUNT, FP_SCALE, OWNER_NEUTRAL, checksum, new_match, simulate_frames, step,
};

#[test]
fn identical_inputs_produce_identical_checksums() {
    let seed = 42u64;
    let inputs: Vec<FrameInput> = (0..240)
        .map(|i| FrameInput {
            p0: if i % 3 == 0 { 1 } else { 0 },
            p1: if i % 5 == 0 { 2 } else { 0 },
        })
        .collect();
    let a = checksum(&simulate_frames(seed, &inputs));
    let b = checksum(&simulate_frames(seed, &inputs));
    assert_eq!(a, b);
}

#[test]
fn replay_roundtrip() {
    let replay = crate::Replay {
        seed: 7,
        inputs: vec![FrameInput::default(); 10],
    };
    let code = crate::encode_replay(&replay);
    let decoded = crate::decode_replay(&code).expect("decode");
    assert_eq!(replay, decoded);
}

#[test]
fn checkpoint_resume_matches_continuous_run() {
    let seed = 99;
    let inputs: Vec<FrameInput> = (0..120)
        .map(|i| FrameInput {
            p0: if i % 4 == 0 { 1 } else { 0 },
            p1: if i % 6 == 0 { 2 } else { 0 },
        })
        .collect();

    let full = simulate_frames(seed, &inputs);

    let mut partial = new_match(seed);
    for inp in inputs.iter().take(60) {
        step(&mut partial, *inp);
    }
    let checkpoint = partial.clone();
    for inp in inputs.iter().skip(60) {
        step(&mut partial, *inp);
    }

    let mut resumed = checkpoint;
    for inp in inputs.iter().skip(60) {
        step(&mut resumed, *inp);
    }

    assert_eq!(checksum(&full), checksum(&partial));
    assert_eq!(checksum(&partial), checksum(&resumed));
}

#[test]
fn brick_tiers_differ_across_seeds() {
    let a = new_match(1);
    let b = new_match(2);
    assert_ne!(a.brick_max_hp, b.brick_max_hp);
    assert!(
        a.brick_max_hp
            .iter()
            .all(|h| *h == 0 || (1..=3).contains(h))
    );
    assert_eq!(a.brick_hp, a.brick_max_hp);
    // Face-off lane (center two columns) stays empty.
    let gap_lo = (BRICK_COLS as usize / 2).saturating_sub(1);
    let gap_hi = BRICK_COLS as usize / 2;
    for index in 0..BRICK_COUNT {
        let col = index % BRICK_COLS as usize;
        if col == gap_lo || col == gap_hi {
            assert_eq!(a.brick_hp[index], 0);
        }
    }
}

#[test]
fn neutral_ball_does_not_damage_bricks() {
    let mut state = new_match(123);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    let idx = state
        .brick_hp
        .iter()
        .position(|&h| h > 0)
        .expect("alive brick");
    let hp_before = state.brick_hp[idx];
    let center = state.brick_center(idx);
    state.ball.owner = OWNER_NEUTRAL;
    state.ball.pos = center;
    state.ball.vel = crate::Vec2::new(0, crate::rules::BALL_SPEED);
    let out = step(&mut state, FrameInput::default());
    assert_eq!(state.brick_hp[idx], hp_before);
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::BrickBounce { .. }))
    );
}

#[test]
fn charged_brick_hit_neutralizes_and_damages() {
    let mut state = new_match(55);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    let idx = state
        .brick_hp
        .iter()
        .position(|&h| h > 0)
        .expect("alive brick");
    let hp_before = state.brick_hp[idx];
    let center = state.brick_center(idx);
    state.ball.owner = 0;
    state.ball.pos = center;
    state.ball.vel = crate::Vec2::new(0, crate::rules::BALL_SPEED);
    let out = step(&mut state, FrameInput::default());
    assert_eq!(state.brick_hp[idx], hp_before - 1);
    assert_eq!(state.ball.owner, OWNER_NEUTRAL);
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::BallNeutralized))
    );
}

#[test]
fn ball_bounces_off_brick_from_below() {
    let mut state = new_match(7);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    let idx = state
        .brick_hp
        .iter()
        .position(|&h| h > 0)
        .expect("alive brick");
    let center = state.brick_center(idx);
    let half_h = crate::rules::BRICK_H / 2;
    // Approach from below like a bottom-player serve.
    state.ball.owner = 1;
    state.ball.pos = crate::Vec2::new(
        center.x,
        center.y - half_h - crate::rules::BALL_R - FP_SCALE,
    );
    state.ball.vel = crate::Vec2::new(0, crate::rules::BALL_SPEED);
    let score_before = state.score[1];
    let _ = step(&mut state, FrameInput::default());
    assert!(
        state.ball.vel.y < 0,
        "ball should reverse Y after brick bounce, got {:?}",
        state.ball.vel
    );
    assert!(
        state.ball.pos.y < center.y,
        "ball should separate below the brick"
    );
    let _ = score_before;
}

#[test]
fn brick_break_awards_score_to_owner() {
    let mut state = new_match(11);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    let idx = state
        .brick_hp
        .iter()
        .position(|&h| h == 1)
        .expect("1-hp brick");
    let center = state.brick_center(idx);
    state.ball.owner = 0;
    state.ball.pos = center;
    state.ball.vel = crate::Vec2::new(0, crate::rules::BALL_SPEED);
    let before = state.score[0];
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::BrickBreak { .. }))
    );
    assert_eq!(state.score[0], before + 1);
}

#[test]
fn three_breaks_wins_round_early() {
    let mut state = new_match(29);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    state.round_timer = crate::rules::ROUND_DURATION_FRAMES;
    state.round_breaks = [3, 0];
    state.ball.owner = OWNER_NEUTRAL;
    state.ball.pos = crate::Vec2::new(0, 0);
    state.ball.vel = crate::Vec2::new(crate::rules::BALL_SPEED, 0);
    // Keep some bricks alive so this is not a board-clear win.
    assert!(state.brick_hp.iter().any(|&h| h > 0));
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::RoundWin { winner: 0 }))
    );
    assert_eq!(state.rounds_won[0], 1);
}

#[test]
fn timeout_most_breaks_wins_round() {
    let mut state = new_match(19);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    state.round_timer = 0;
    state.round_breaks = [3, 1];
    state.ball.owner = OWNER_NEUTRAL;
    state.ball.pos = crate::Vec2::new(0, 0);
    state.ball.vel = crate::Vec2::new(crate::rules::BALL_SPEED, 0);
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::RoundWin { winner: 0 }))
    );
    assert_eq!(state.rounds_won[0], 1);
    assert_eq!(state.phase, crate::MatchPhase::Serving);
}

#[test]
fn timeout_equal_breaks_ties_round() {
    let mut state = new_match(21);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    state.round_timer = 0;
    state.round_breaks = [2, 2];
    state.ball.owner = OWNER_NEUTRAL;
    state.ball.pos = crate::Vec2::new(0, 0);
    state.ball.vel = crate::Vec2::new(crate::rules::BALL_SPEED, 0);
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::RoundTie))
    );
    assert_eq!(state.rounds_won, [0, 0]);
    assert_eq!(state.phase, crate::MatchPhase::Serving);
}

#[test]
fn second_round_win_sets_match_over() {
    let mut state = new_match(23);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    state.rounds_won = [1, 0];
    state.round_timer = 0;
    state.round_breaks = [4, 0];
    state.ball.owner = OWNER_NEUTRAL;
    state.ball.pos = crate::Vec2::new(0, 0);
    state.ball.vel = crate::Vec2::new(crate::rules::BALL_SPEED, 0);
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::RoundWin { winner: 0 }))
    );
    assert_eq!(state.rounds_won[0], 2);
    assert_eq!(state.phase, crate::MatchPhase::MatchOver);
}

#[test]
fn paddle_shove_launches_wild_brick() {
    let mut state = new_match(3);
    state.phase = crate::MatchPhase::Rally;
    state.serve_timer = 0;
    state.wild_bricks[0].active = true;
    state.wild_bricks[0].hp = 1;
    state.wild_bricks[0].x = 0;
    state.wild_bricks[0].y = state.paddles[0].y;
    state.wild_bricks[0].vx = 0;
    state.wild_bricks[0].vy = 0;
    state.paddles[0].x = -crate::rules::WILD_BRICK_HALF;
    state.paddles[0].vx = crate::rules::PADDLE_SPEED * 2;
    let out = step(&mut state, FrameInput::default());
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, crate::ConfirmedEvent::WildPaddleKnock { player: 0, .. }))
    );
    assert!(
        state.wild_bricks[0].vx.abs() > 0 || state.wild_bricks[0].vy.abs() > 0,
        "tier-1 wild should receive shove velocity"
    );
}

#[test]
fn corner_trampoline_preserves_and_boosts_inbound_momentum() {
    use crate::collision::resolve_body_corner_arcs;
    use crate::rules::{
        ARENA_H, ARENA_W, BALL_R, CORNER_R, CORNER_WALL_KNOCK, WALL, WILD_BRICK_HALF,
    };

    let edge_x = ARENA_W / 2 - WALL;
    let edge_y = ARENA_H / 2 - WALL;
    let r = CORNER_R;
    let cx = -edge_x + r;
    let cy = -edge_y + r;
    let max_dist = r - BALL_R;
    // Deep in the BL pocket (outside the playable arc).
    let mut x = cx - max_dist - 4 * FP_SCALE;
    let mut y = cy - max_dist - 4 * FP_SCALE;
    let inbound = 20 * FP_SCALE;
    let mut vx = -inbound;
    let mut vy = -inbound;
    let before = ((vx as i64) * (vx as i64) + (vy as i64) * (vy as i64)) as i128;
    let hit = resolve_body_corner_arcs(
        &mut x,
        &mut y,
        &mut vx,
        &mut vy,
        BALL_R,
        CORNER_WALL_KNOCK,
        0,
    );
    assert_eq!(hit, Some(0));
    let after = ((vx as i64) * (vx as i64) + (vy as i64) * (vy as i64)) as i128;
    assert!(
        after > before,
        "trampoline should boost speed: before={before} after={after} vel=({vx},{vy})"
    );
    // Outward into play from BL → +x/+y dominant.
    assert!(vx > 0 && vy > 0, "bounce should leave toward midcourt");

    // Wild brick radius also trampolines.
    let max_w = r - WILD_BRICK_HALF;
    let mut wx = cx - max_w - 3 * FP_SCALE;
    let mut wy = cy - max_w - 3 * FP_SCALE;
    let mut wvx = -15 * FP_SCALE;
    let mut wvy = -15 * FP_SCALE;
    let w_before = (wvx as i64).abs() + (wvy as i64).abs();
    assert!(
        resolve_body_corner_arcs(
            &mut wx,
            &mut wy,
            &mut wvx,
            &mut wvy,
            WILD_BRICK_HALF,
            crate::rules::CORNER_PADDLE_KNOCK,
            0,
        )
        .is_some()
    );
    let w_after = (wvx as i64).abs() + (wvy as i64).abs();
    assert!(w_after > w_before, "wild brick trampoline should boost");
}

#[test]
fn paddle_corner_reflects_inbound_velocity() {
    use crate::collision::resolve_body_corner_arcs;
    use crate::rules::{
        ARENA_H, ARENA_W, CORNER_PADDLE_KNOCK, CORNER_R, CORNER_TANGENT_KICK, PADDLE_H, PADDLE_W,
        WALL,
    };

    let edge_x = ARENA_W / 2 - WALL;
    let edge_y = ARENA_H / 2 - WALL;
    let r = CORNER_R;
    let cx = -edge_x + r;
    let cy = -edge_y + r;
    let body_r = (PADDLE_W / 4).max(PADDLE_H);
    let max_dist = r - body_r;
    let mut x = cx - max_dist - 2 * FP_SCALE;
    let mut y = cy - max_dist - 2 * FP_SCALE;
    let mut vx = -12 * FP_SCALE;
    let mut vy = -12 * FP_SCALE;
    assert!(
        resolve_body_corner_arcs(
            &mut x,
            &mut y,
            &mut vx,
            &mut vy,
            body_r,
            CORNER_PADDLE_KNOCK,
            CORNER_TANGENT_KICK / 2,
        )
        .is_some()
    );
    assert!(vx > 0 && vy > 0, "paddle should bounce out of the corner");
}

#[test]
fn match_over_rematch_votes_return_to_readying() {
    use crate::input::INPUT_JUMP;
    let mut state = new_match(41);
    state.phase = crate::MatchPhase::MatchOver;
    state.rounds_won = [2, 0];
    state.ready = [false, false];
    let _ = step(
        &mut state,
        FrameInput {
            p0: INPUT_JUMP,
            p1: INPUT_JUMP,
        },
    );
    assert_eq!(state.phase, crate::MatchPhase::Readying);
    assert_eq!(state.rounds_won, [0, 0]);
    assert_eq!(state.ready, [false, false]);
}
