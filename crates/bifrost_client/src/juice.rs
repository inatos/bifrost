//! Presentation juice: particles, camera shake, flashes, and SFX from confirmed events.

use bevy::prelude::*;
use bifrost_sim::ConfirmedEvent;

use crate::render::{ball_owner_color, P1_COLOR, P2_COLOR};
use crate::state::{AppState, SimSnapshot};

#[derive(Resource, Default)]
pub struct JuiceState {
    pub shake: f32,
    pub flash: f32,
    pub hitstop: f32,
    pub trail_cooldown: f32,
    /// Global SFX gate — seconds until next collision cue may play.
    pub sfx_cooldown: f32,
    /// Cadence for live snapback wave crescent trails.
    pub wave_vfx_cd: f32,
}

const SFX_MIN_GAP: f32 = 0.045;

#[derive(Component)]
pub(crate) struct JuiceParticle {
    vel: Vec2,
    life: f32,
    max_life: f32,
}

#[derive(Component)]
pub(crate) struct BallTrail {
    life: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<JuiceState>().add_systems(
        Update,
        (
            drain_confirmed_events,
            tick_juice,
            spawn_ball_trail,
            spawn_angle_wave_trail,
            tick_particles,
            apply_camera_shake,
        )
            .chain()
            .run_if(in_state(AppState::InGame)),
    );
}

fn drain_confirmed_events(
    mut sim: ResMut<SimSnapshot>,
    mut juice: ResMut<JuiceState>,
    mut commands: Commands,
    interp: Res<crate::interp::InterpState>,
    time: Res<Time<Fixed>>,
) {
    if sim.events.is_empty() {
        return;
    }
    let origin = if interp.initialized {
        let snap = interp.sample(time.overstep_fraction());
        Vec2::new(snap.ball_x, snap.ball_y)
    } else {
        Vec2::ZERO
    };
    let events = std::mem::take(&mut sim.events);
    for event in events {
        match event {
            ConfirmedEvent::BrickBreak { .. } => {
                juice.shake = juice.shake.max(0.55);
                juice.flash = juice.flash.max(0.35);
                juice.hitstop = juice.hitstop.max(0.04);
                burst(&mut commands, origin, Color::srgb(0.72, 0.42, 0.95), 14, 220.0);
                try_play_sfx(&mut juice, "break", 0.55);
                try_haptic("break");
            }
            ConfirmedEvent::BrickDamage { .. } => {
                juice.shake = juice.shake.max(0.22);
                juice.flash = juice.flash.max(0.12);
                burst(&mut commands, origin, Color::srgb(0.55, 0.35, 0.78), 6, 140.0);
                try_play_sfx(&mut juice, "chip", 0.35);
                try_haptic("chip");
            }
            ConfirmedEvent::BrickBounce { .. } => {
                juice.shake = juice.shake.max(0.08);
                // Silent — bounce spam is the main offender.
            }
            ConfirmedEvent::WildBrickBreak { .. } => {
                juice.shake = juice.shake.max(0.4);
                juice.flash = juice.flash.max(0.25);
                burst(&mut commands, origin, Color::srgb(0.9, 0.35, 0.85), 10, 200.0);
                try_play_sfx(&mut juice, "wild", 0.45);
                try_haptic("break");
            }
            ConfirmedEvent::WildBrickHit { .. } => {
                juice.shake = juice.shake.max(0.25);
                try_play_sfx(&mut juice, "wild", 0.28);
            }
            ConfirmedEvent::WildPaddleKnock { player, .. } => {
                juice.shake = juice.shake.max(0.35);
                try_play_sfx(&mut juice, "knock", 0.4);
                if player == 0 {
                    try_haptic("shove");
                }
            }
            ConfirmedEvent::BallNeutralized => {
                juice.flash = juice.flash.max(0.2);
                burst(&mut commands, origin, Color::srgb(0.62, 0.4, 0.9), 8, 160.0);
                // Visual only — often stacks with brick/wild hit.
            }
            ConfirmedEvent::CornerBounce { corner: _ } => {
                juice.shake = juice.shake.max(0.12);
                try_play_sfx(&mut juice, "corner", 0.35);
            }
            ConfirmedEvent::PaddleHit { player } => {
                juice.shake = juice.shake.max(0.28);
                juice.hitstop = juice.hitstop.max(0.03);
                let color = if player == 0 { P1_COLOR } else { P2_COLOR };
                burst(&mut commands, origin, color, 8, 180.0);
                try_play_sfx(&mut juice, "paddle", 0.4);
                if player == 0 {
                    try_haptic("paddle");
                }
            }
            ConfirmedEvent::Goal { scorer } => {
                juice.shake = juice.shake.max(0.85);
                juice.flash = juice.flash.max(0.55);
                let color = if scorer == 0 { P1_COLOR } else { P2_COLOR };
                burst(&mut commands, origin, color, 22, 320.0);
                try_play_sfx(&mut juice, "goal", 0.7);
                try_haptic("goal");
            }
            ConfirmedEvent::SpinRelease { player, charge } => {
                let paddle = sim.world.paddles[player as usize];
                let (px, py) = bifrost_sim::Vec2::new(paddle.x, paddle.y).to_f();
                let color = if player == 0 { P1_COLOR } else { P2_COLOR };
                let power = 0.45 + charge as f32 / 120.0;
                juice.shake = juice.shake.max(0.45 + power * 0.35);
                juice.flash = juice.flash.max(0.28 + power * 0.25);
                juice.hitstop = juice.hitstop.max(0.04);
                ring_burst(
                    &mut commands,
                    Vec2::new(px, py),
                    color,
                    20 + (charge as usize / 6),
                    280.0 + charge as f32 * 2.5,
                );
                burst(
                    &mut commands,
                    Vec2::new(px, py),
                    Color::srgb(1.0, 0.92, 0.55),
                    14 + (charge as usize / 8),
                    220.0 + charge as f32 * 1.8,
                );
                try_play_sfx(&mut juice, "spin", 0.5 + charge as f32 / 180.0);
                try_haptic("spin");
            }
            ConfirmedEvent::WildBallBurst { .. } => {
                juice.shake = juice.shake.max(0.55);
                juice.flash = juice.flash.max(0.45);
                burst(&mut commands, origin, Color::srgb(0.95, 0.45, 0.85), 18, 280.0);
                try_play_sfx(&mut juice, "burst", 0.55);
                try_haptic("burst");
            }
            ConfirmedEvent::RoundWin { winner } => {
                juice.shake = juice.shake.max(1.0);
                juice.flash = juice.flash.max(0.7);
                try_play_sfx(&mut juice, "win", 0.85);
                try_haptic("win");
                let match_over = sim.world.phase == bifrost_sim::MatchPhase::MatchOver;
                notify_shell_round_win(winner, match_over);
            }
            ConfirmedEvent::RoundTie => {
                juice.shake = juice.shake.max(0.45);
                juice.flash = juice.flash.max(0.35);
                try_play_sfx(&mut juice, "win", 0.4);
                notify_shell_round_tie();
            }
            ConfirmedEvent::GroundPound { player, x, y } => {
                let (wx, wy) = bifrost_sim::Vec2::new(x, y).to_f();
                let color = if player == 0 {
                    P1_COLOR
                } else {
                    P2_COLOR
                };
                juice.shake = juice.shake.max(1.05);
                juice.flash = juice.flash.max(0.7);
                juice.hitstop = juice.hitstop.max(0.09);
                ring_burst(&mut commands, Vec2::new(wx, wy), color, 40, 420.0);
                sword_beam_burst(
                    &mut commands,
                    Vec2::new(wx, wy),
                    Vec2::new(0.0, -1.0),
                    color.with_alpha(0.85),
                    8,
                    220.0,
                    0.85,
                );
                try_play_sfx(&mut juice, "pound", 0.9);
                try_haptic("break");
            }
            ConfirmedEvent::CornerPulse { corner: _, x, y } => {
                let (wx, wy) = bifrost_sim::Vec2::new(x, y).to_f();
                juice.shake = juice.shake.max(0.7);
                juice.flash = juice.flash.max(0.4);
                ring_burst(
                    &mut commands,
                    Vec2::new(wx, wy),
                    Color::srgb(0.95, 0.75, 1.0),
                    36,
                    420.0,
                );
                try_play_sfx(&mut juice, "corner", 0.65);
                try_haptic("shove");
            }
            ConfirmedEvent::Clang { x, y } => {
                let (wx, wy) = bifrost_sim::Vec2::new(x, y).to_f();
                juice.shake = juice.shake.max(0.95);
                juice.flash = juice.flash.max(0.55);
                juice.hitstop = juice.hitstop.max(0.08);
                // Neutral hazard palette (not team cyan).
                ring_burst(
                    &mut commands,
                    Vec2::new(wx, wy),
                    Color::srgb(1.0, 0.55, 0.85),
                    24,
                    380.0,
                );
                try_play_sfx(&mut juice, "paddle", 0.85);
                try_haptic("shove");
            }
            ConfirmedEvent::AngleWave {
                player,
                x,
                y,
                nx,
                ny,
                power,
                radius,
            } => {
                let (wx, wy) = bifrost_sim::Vec2::new(x, y).to_f();
                let color = if player == 0 { P1_COLOR } else { P2_COLOR };
                let t = (power as f32 / (58.0 * 1000.0)).clamp(0.15, 1.0);
                let r = (radius as f32 / 1000.0).max(32.0);
                juice.shake = juice.shake.max(0.28 + t * 0.35);
                juice.flash = juice.flash.max(0.15 + t * 0.25);
                // Prefer event direction (survives rollback); fall back to world, then face.
                let mut dir = Vec2::new(
                    nx as f32 / bifrost_sim::FP_SCALE as f32,
                    ny as f32 / bifrost_sim::FP_SCALE as f32,
                );
                if dir.length_squared() < 0.01 {
                    dir = Vec2::new(
                        sim.world.angle_wave_nx as f32 / bifrost_sim::FP_SCALE as f32,
                        sim.world.angle_wave_ny as f32 / bifrost_sim::FP_SCALE as f32,
                    );
                }
                let dir = if dir.length_squared() < 0.01 {
                    Vec2::new(0.0, if player == 0 { -1.0 } else { 1.0 })
                } else {
                    dir.normalize()
                };
                sword_beam_burst(
                    &mut commands,
                    Vec2::new(wx, wy),
                    dir,
                    color,
                    5 + (t * 4.0) as usize,
                    280.0 + r * 1.8,
                    t,
                );
                try_play_sfx(&mut juice, "paddle", 0.35 + t * 0.4);
                if player == 0 {
                    try_haptic("paddle");
                }
            }
        }
    }
}

fn try_play_sfx(juice: &mut JuiceState, kind: &str, volume: f32) {
    if juice.sfx_cooldown > 0.0 {
        return;
    }
    juice.sfx_cooldown = SFX_MIN_GAP;
    play_sfx(kind, volume);
}

fn try_haptic(kind: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(hook) = js_sys::Reflect::get(&window, &"bifrostHaptic".into()) {
            if let Ok(func) = hook.dyn_into::<js_sys::Function>() {
                let _ = func.call1(&window, &wasm_bindgen::JsValue::from_str(kind));
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = kind;
    }
}

fn notify_shell_round_win(winner: u8, match_over: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(hook) = js_sys::Reflect::get(&window, &"bifrostRoundWin".into()) {
            if let Ok(func) = hook.dyn_into::<js_sys::Function>() {
                let _ = func.call2(
                    &window,
                    &wasm_bindgen::JsValue::from_f64(winner as f64),
                    &wasm_bindgen::JsValue::from_bool(match_over),
                );
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (winner, match_over);
    }
}

fn notify_shell_round_tie() {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(hook) = js_sys::Reflect::get(&window, &"bifrostRoundTie".into()) {
            if let Ok(func) = hook.dyn_into::<js_sys::Function>() {
                let _ = func.call0(&window);
            }
        }
    }
}

fn play_sfx(kind: &str, volume: f32) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        // Prefer window hook; fall back to globalThis (some embedders differ).
        let hook = js_sys::Reflect::get(&window, &"bifrostPlaySfx".into())
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null())
            .or_else(|| {
                let global = js_sys::global();
                js_sys::Reflect::get(&global, &"bifrostPlaySfx".into())
                    .ok()
                    .filter(|v| !v.is_undefined() && !v.is_null())
            });
        if let Some(hook) = hook {
            if let Ok(func) = hook.dyn_into::<js_sys::Function>() {
                let _ = func.call2(
                    &window,
                    &wasm_bindgen::JsValue::from_str(kind),
                    &wasm_bindgen::JsValue::from_f64(volume as f64),
                );
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (kind, volume);
    }
}

fn burst(commands: &mut Commands, origin: Vec2, color: Color, count: usize, speed: f32) {
    for i in 0..count.min(18) {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU + (i as f32 * 0.37);
        let spd = speed * (0.45 + (i % 5) as f32 * 0.12);
        let vel = Vec2::new(angle.cos(), angle.sin()) * spd;
        let life = 0.28 + (i % 4) as f32 * 0.05;
        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::splat(5.0 + (i % 3) as f32)),
                ..default()
            },
            Transform::from_xyz(origin.x, origin.y, 8.0),
            JuiceParticle {
                vel,
                life,
                max_life: life,
            },
        ));
    }
}

/// Expanding radial shockwave for ground pound.
fn ring_burst(commands: &mut Commands, origin: Vec2, color: Color, count: usize, speed: f32) {
    burst(commands, origin, color, count.min(18), speed);
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let spd = speed * (0.85 + (i % 3) as f32 * 0.08);
        let vel = Vec2::new(angle.cos(), angle.sin()) * spd;
        let life = 0.42 + (i % 5) as f32 * 0.04;
        commands.spawn((
            Sprite {
                color: color.with_alpha(0.85),
                custom_size: Some(Vec2::splat(7.0 + (i % 4) as f32 * 1.5)),
                ..default()
            },
            Transform::from_xyz(origin.x, origin.y, 9.0),
            JuiceParticle {
                vel,
                life,
                max_life: life,
            },
        ));
    }
}

/// Forward crescent arcs + spark trail (Link sword-beam feel).
fn sword_beam_burst(
    commands: &mut Commands,
    origin: Vec2,
    dir: Vec2,
    color: Color,
    arcs: usize,
    speed: f32,
    power_t: f32,
) {
    let perp = Vec2::new(-dir.y, dir.x);
    for i in 0..arcs.max(3) {
        let along = 18.0 + i as f32 * 22.0;
        let spread = (i as f32 - arcs as f32 * 0.5) * 7.0;
        let pos = origin + dir * along + perp * spread * 0.15;
        let life = 0.35 + power_t * 0.25 + (i % 3) as f32 * 0.04;
        let w = 28.0 + power_t * 18.0 + (i % 2) as f32 * 6.0;
        let h = 5.0 + power_t * 3.0;
        let angle = dir.y.atan2(dir.x);
        commands.spawn((
            Sprite {
                color: color.with_alpha(0.75 - i as f32 * 0.06),
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 9.2).with_rotation(Quat::from_rotation_z(angle)),
            JuiceParticle {
                vel: dir * (speed * (0.55 + i as f32 * 0.08)),
                life,
                max_life: life,
            },
        ));
        // Soft ripple twin offset.
        let ripple = origin + dir * (along + 10.0) + perp * ((i as f32 % 3.0) - 1.0) * 10.0;
        let rlife = life * 0.85;
        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 0.95, 0.7, 0.45),
                custom_size: Some(Vec2::new(w * 0.7, h * 0.7)),
                ..default()
            },
            Transform::from_xyz(ripple.x, ripple.y, 9.0)
                .with_rotation(Quat::from_rotation_z(angle + 0.12)),
            JuiceParticle {
                vel: dir * (speed * 0.4) + perp * ((i % 2) as f32 * 2.0 - 1.0) * 40.0,
                life: rlife,
                max_life: rlife,
            },
        ));
    }
    // Spark motes.
    for i in 0..(6 + (power_t * 8.0) as usize).min(14) {
        let jitter = perp * ((i as f32 * 0.73).sin() * 55.0);
        let vel = dir * (speed * (0.35 + (i % 4) as f32 * 0.1)) + jitter * 0.4;
        let life = 0.22 + (i % 4) as f32 * 0.05;
        commands.spawn((
            Sprite {
                color: color.with_alpha(0.9),
                custom_size: Some(Vec2::splat(3.0 + (i % 3) as f32)),
                ..default()
            },
            Transform::from_xyz(origin.x, origin.y, 9.5),
            JuiceParticle {
                vel,
                life,
                max_life: life,
            },
        ));
    }
}

fn spawn_angle_wave_trail(mut commands: Commands, mut juice: ResMut<JuiceState>, sim: Res<SimSnapshot>) {
    if sim.world.angle_wave_t == 0 {
        return;
    }
    if juice.wave_vfx_cd > 0.0 {
        return;
    }
    juice.wave_vfx_cd = 0.05;
    let (wx, wy) = bifrost_sim::Vec2::new(sim.world.angle_wave_x, sim.world.angle_wave_y).to_f();
    let nx = sim.world.angle_wave_nx as f32 / bifrost_sim::FP_SCALE as f32;
    let ny = sim.world.angle_wave_ny as f32 / bifrost_sim::FP_SCALE as f32;
    let mut dir = Vec2::new(nx, ny);
    if dir.length_squared() < 0.01 {
        return;
    }
    dir = dir.normalize();
    let color = if sim.world.angle_wave_player == 0 {
        P1_COLOR
    } else {
        P2_COLOR
    };
    let life = (sim.world.angle_wave_t as f32 / bifrost_sim::ANGLE_WAVE_DURATION.max(1) as f32)
        .clamp(0.12, 0.55);
    let angle = dir.y.atan2(dir.x);
    let perp = Vec2::new(-dir.y, dir.x);
    // Rippling crescent at the wave front.
    commands.spawn((
        Sprite {
            color: color.with_alpha(0.2 + life * 0.55),
            custom_size: Some(Vec2::new(34.0 + life * 20.0, 6.0)),
            ..default()
        },
        Transform::from_xyz(wx, wy, 9.1).with_rotation(Quat::from_rotation_z(angle)),
        JuiceParticle {
            vel: dir * 160.0,
            life: 0.28 + life * 0.2,
            max_life: 0.28 + life * 0.2,
        },
    ));
    for side in [-1.0_f32, 1.0] {
        let p = Vec2::new(wx, wy) + perp * side * 14.0;
        commands.spawn((
            Sprite {
                color: color.with_alpha(0.35 + life * 0.3),
                custom_size: Some(Vec2::new(3.5, 3.5)),
                ..default()
            },
            Transform::from_xyz(p.x, p.y, 9.3),
            JuiceParticle {
                vel: dir * 120.0 + perp * side * 50.0,
                life: 0.2 + life * 0.15,
                max_life: 0.2 + life * 0.15,
            },
        ));
    }
}

fn tick_juice(time: Res<Time>, mut juice: ResMut<JuiceState>) {
    let dt = time.delta_secs();
    juice.shake = (juice.shake - dt * 2.4).max(0.0);
    juice.flash = (juice.flash - dt * 2.8).max(0.0);
    juice.hitstop = (juice.hitstop - dt).max(0.0);
    juice.trail_cooldown = (juice.trail_cooldown - dt).max(0.0);
    juice.sfx_cooldown = (juice.sfx_cooldown - dt).max(0.0);
    juice.wave_vfx_cd = (juice.wave_vfx_cd - dt).max(0.0);
}

fn spawn_ball_trail(
    mut commands: Commands,
    mut juice: ResMut<JuiceState>,
    sim: Res<SimSnapshot>,
    interp: Res<crate::interp::InterpState>,
    time: Res<Time<Fixed>>,
) {
    if !interp.initialized || juice.trail_cooldown > 0.0 {
        return;
    }
    if sim.world.ball_is_neutral() {
        return;
    }
    juice.trail_cooldown = 0.028;
    let snap = interp.sample(time.overstep_fraction());
    let color = ball_owner_color(snap.ball_owner).with_alpha(0.45);
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::splat(18.0)),
            ..default()
        },
        Transform::from_xyz(snap.ball_x, snap.ball_y, 1.6),
        BallTrail { life: 0.18 },
    ));
}

fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<
        (Entity, &mut Transform, &mut Sprite, &mut JuiceParticle),
        (Without<BallTrail>, Without<Camera2d>, Without<crate::render::ArenaPart>),
    >,
    mut trails: Query<
        (Entity, &mut Transform, &mut Sprite, &mut BallTrail),
        (Without<JuiceParticle>, Without<Camera2d>, Without<crate::render::ArenaPart>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut sprite, mut p) in &mut particles {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation.x += p.vel.x * dt;
        transform.translation.y += p.vel.y * dt;
        p.vel *= 0.92;
        let t = (p.life / p.max_life).clamp(0.0, 1.0);
        sprite.color = sprite.color.with_alpha(t);
        let s = 0.6 + t * 0.8;
        transform.scale = Vec3::splat(s);
    }
    for (entity, mut transform, mut sprite, mut trail) in &mut trails {
        trail.life -= dt;
        if trail.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let t = (trail.life / 0.18).clamp(0.0, 1.0);
        sprite.color = sprite.color.with_alpha(0.35 * t);
        transform.scale = Vec3::splat(0.7 + t * 0.4);
    }
}

fn apply_camera_shake(
    juice: Res<JuiceState>,
    time: Res<Time>,
    mut cameras: Query<&mut Transform, (With<Camera2d>, Without<crate::render::ArenaPart>)>,
) {
    let Ok(mut transform) = cameras.single_mut() else {
        return;
    };
    if juice.shake <= 0.001 {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
        transform.rotation = Quat::IDENTITY;
        return;
    }
    let t = time.elapsed_secs();
    let amp = juice.shake * 10.0;
    transform.translation.x = (t * 57.0).sin() * amp;
    transform.translation.y = (t * 71.0).cos() * amp * 0.7;
    transform.rotation = Quat::IDENTITY;
}
