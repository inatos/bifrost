use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bifrost_sim::{
    BRICK_COLS, BRICK_ROWS, FP_SCALE, MAX_WILD_BRICKS, MatchPhase, OWNER_NEUTRAL, PADDLE_H,
    PADDLE_W, PADDLE_W_BACK, can_ground_pound, jump_scale_fixed, paddle_airborne,
};

use crate::interp::{InterpState, VisualSnapshot};
use crate::state::{AppState, LaunchConfig};

const ARENA_W: f32 = 1200.0;
const ARENA_H: f32 = 480.0;
const CORNER_R: f32 = 88.0;

pub const P1_COLOR: Color = Color::srgb(0.2, 0.85, 0.95);
pub const P2_COLOR: Color = Color::srgb(0.95, 0.55, 0.25);
pub const NEUTRAL_BALL: Color = Color::srgb(0.62, 0.42, 0.92);
const WILD_BASE: Color = Color::srgb(0.78, 0.28, 0.82);

#[derive(Resource)]
pub struct ArenaAssets {
    pub font: Handle<Font>,
    pub paddle_mesh: Handle<Mesh>,
    pub wild_mesh: Handle<Mesh>,
    pub ball_mesh: Handle<Mesh>,
    pub brick_mesh: Handle<Mesh>,
    pub paddle_materials: [Handle<ColorMaterial>; 2],
    pub wild_materials: [Handle<ColorMaterial>; 3],
    pub ball_materials: [Handle<ColorMaterial>; 3],
    pub brick_materials: [Handle<ColorMaterial>; 3],
    pub shadow_material: Handle<ColorMaterial>,
    pub corner_material: Handle<ColorMaterial>,
    pub corner_hi_material: Handle<ColorMaterial>,
    pub crack_material: Handle<ColorMaterial>,
    pub corner_mesh: Handle<Mesh>,
}

#[derive(Component)]
pub(crate) struct ArenaRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum ArenaPart {
    Background,
    FloorShade,
    SideWall,
    BackboardTop,
    BackboardBottom,
    Brick(u16),
    BrickExtrude(u16),
    BrickShadow(u16),
    BrickCrack(u16),
    WildBrick(u8),
    WildShadow(u8),
    Paddle(u8),
    Ball,
    BallHighlight,
    BallShadow,
    LabelName(u8),
    LabelHint(u8),
    ScoreSide(u8),
    /// Diegetic match stock (best-of-3). `slot` is 0 or 1.
    RoundStock {
        player: u8,
        slot: u8,
    },
    MatchOverText,
    FlashOverlay,
    FaceOffRing,
    CornerRamp(u8),
    /// Snapback force-wave projectile (synced from sim).
    AngleWave,
    /// Grappleshot tether segment (player 0 / 1).
    GrappleLine(u8),
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load_arena_assets)
        .add_systems(OnEnter(AppState::InGame), spawn_arena)
        .add_systems(OnExit(AppState::InGame), despawn_arena)
        .add_systems(
            Update,
            sync_arena_visuals.run_if(in_state(AppState::InGame)),
        );
}

pub fn ball_owner_color(owner: u8) -> Color {
    match owner {
        0 => P1_COLOR,
        1 => P2_COLOR,
        _ => NEUTRAL_BALL,
    }
}

fn load_arena_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let paddle_mesh = meshes.add(rounded_trapezoid_mesh(
        PADDLE_W_BACK as f32 / FP_SCALE as f32 / 2.0,
        PADDLE_W as f32 / FP_SCALE as f32 / 2.0,
        PADDLE_H as f32 / FP_SCALE as f32 / 2.0,
    ));
    let wild_mesh = meshes.add(rounded_rect_mesh(20.0, 20.0, 3.0));
    let ball_mesh = meshes.add(circle_mesh(14.0, 24));
    let corner_mesh = meshes.add(corner_arch_mesh(CORNER_R, 20));
    let brick_mesh = meshes.add(rounded_rect_mesh(76.0, 22.0, 3.5));
    let paddle_materials = [
        materials.add(ColorMaterial::from(P1_COLOR)),
        materials.add(ColorMaterial::from(P2_COLOR)),
    ];
    let wild_materials = [
        materials.add(ColorMaterial::from(wild_tint(1))),
        materials.add(ColorMaterial::from(wild_tint(2))),
        materials.add(ColorMaterial::from(wild_tint(3))),
    ];
    let ball_materials = [
        materials.add(ColorMaterial::from(P1_COLOR)),
        materials.add(ColorMaterial::from(P2_COLOR)),
        materials.add(ColorMaterial::from(NEUTRAL_BALL)),
    ];
    let brick_materials = [
        materials.add(ColorMaterial::from(brick_base(1))),
        materials.add(ColorMaterial::from(brick_base(2))),
        materials.add(ColorMaterial::from(brick_base(3))),
    ];
    let shadow_material = materials.add(ColorMaterial::from(Color::srgba(0.02, 0.01, 0.05, 0.45)));
    let corner_material = materials.add(ColorMaterial::from(Color::srgb(0.72, 0.55, 0.92)));
    let corner_hi_material = materials.add(ColorMaterial::from(Color::srgba(0.9, 0.75, 1.0, 0.4)));
    let crack_material = materials.add(ColorMaterial::from(Color::srgba(0.85, 0.92, 1.0, 0.0)));
    commands.insert_resource(ArenaAssets {
        font,
        paddle_mesh,
        wild_mesh,
        ball_mesh,
        brick_mesh,
        paddle_materials,
        wild_materials,
        ball_materials,
        brick_materials,
        shadow_material,
        corner_material,
        corner_hi_material,
        crack_material,
        corner_mesh,
    });
}

fn rounded_trapezoid_mesh(back_half: f32, front_half: f32, half_h: f32) -> Mesh {
    let bevel = 2.8;
    let bh = back_half;
    let fh = front_half;
    let hh = half_h;
    let positions = vec![
        [-bh + bevel * 0.4, hh - bevel, 0.0],
        [bh - bevel * 0.4, hh - bevel, 0.0],
        [bh, hh - bevel * 1.6, 0.0],
        [fh - bevel, -hh + bevel, 0.0],
        [fh, -hh + bevel * 0.5, 0.0],
        [fh - bevel, -hh, 0.0],
        [-fh + bevel, -hh, 0.0],
        [-fh, -hh + bevel * 0.5, 0.0],
        [-fh + bevel, -hh + bevel, 0.0],
        [-bh, hh - bevel * 1.6, 0.0],
    ];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(vec![
        0, 1, 2, 0, 2, 9, 2, 3, 4, 2, 4, 1, 4, 3, 5, 4, 5, 6, 4, 6, 7, 4, 7, 8, 4, 8, 9, 4, 9, 2,
    ]));
    mesh
}

fn circle_mesh(radius: f32, segments: usize) -> Mesh {
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut indices = Vec::new();
    for i in 0..segments {
        let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
        positions.push([a.cos() * radius, a.sin() * radius, 0.0]);
    }
    for i in 0..segments {
        let next = if i + 1 == segments { 1 } else { i + 2 };
        indices.extend_from_slice(&[0, (i + 1) as u32, next as u32]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Pinball corner arch solid: tip + arc (π..3π/2 in BL local). Flush with both walls.
fn corner_arch_mesh(radius: f32, segments: usize) -> Mesh {
    let segs = segments.max(6);
    let mut positions = vec![[-radius, -radius, 0.0]]; // tip
    let mut indices = Vec::new();
    for i in 0..=segs {
        let a = std::f32::consts::PI + (i as f32 / segs as f32) * std::f32::consts::FRAC_PI_2;
        positions.push([a.cos() * radius, a.sin() * radius, 0.0]);
    }
    for i in 0..segs {
        indices.extend_from_slice(&[0, (i + 1) as u32, (i + 2) as u32]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn rounded_rect_mesh(w: f32, h: f32, _r: f32) -> Mesh {
    let hw = w / 2.0;
    let hh = h / 2.0;
    let positions = vec![
        [-hw, -hh, 0.0],
        [hw, -hh, 0.0],
        [hw, hh, 0.0],
        [-hw, hh, 0.0],
    ];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

fn spawn_arena(
    mut commands: Commands,
    assets: Res<ArenaAssets>,
    interp: Res<InterpState>,
    config: Res<LaunchConfig>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let bot_match = !config.args.is_online();
    let snap = if interp.initialized {
        interp.curr.clone()
    } else {
        VisualSnapshot::default()
    };

    commands.spawn(ArenaRoot);
    spawn_static_geometry(&mut commands, &assets);
    spawn_bricks(&mut commands, &assets, &snap, &mut materials);
    spawn_wild_bricks(&mut commands, &assets);
    spawn_paddles_and_ball(&mut commands, &assets, &snap);
    spawn_labels_and_scores(&mut commands, &assets, bot_match);
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.85, 1.0, 0.0),
            custom_size: Some(Vec2::new(ARENA_W + 80.0, ARENA_H + 80.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 9.0),
        ArenaPart::FlashOverlay,
    ));
}

fn spawn_static_geometry(commands: &mut Commands, assets: &ArenaAssets) {
    let wall_color = Color::srgb(0.55, 0.42, 0.72);
    let bg = Color::srgb(0.07, 0.05, 0.12);
    let floor = Color::srgb(0.12, 0.08, 0.18);

    commands.spawn((
        Sprite {
            color: bg,
            custom_size: Some(Vec2::new(ARENA_W + 60.0, ARENA_H + 60.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -2.0),
        ArenaPart::Background,
    ));
    // Soft “lit” floor plane for 2.5D read.
    commands.spawn((
        Sprite {
            color: floor,
            custom_size: Some(Vec2::new(ARENA_W - 20.0, ARENA_H - 20.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -12.0, -1.5),
        ArenaPart::FloorShade,
    ));

    for x in [-ARENA_W / 2.0, ARENA_W / 2.0] {
        // Side rails stop at the arch junctions (don't cut through corners).
        let wall_h = (ARENA_H - 20.0 - 2.0 * CORNER_R).max(40.0);
        commands.spawn((
            Sprite {
                color: wall_color,
                custom_size: Some(Vec2::new(8.0, wall_h)),
                ..default()
            },
            Transform::from_xyz(x, 0.0, 0.0),
            ArenaPart::SideWall,
        ));
    }

    // Pinball corner arches — one solid fillet per corner (flush with walls).
    let edge_x = ARENA_W / 2.0 - 10.0;
    let edge_y = ARENA_H / 2.0 - 10.0;
    let corners = [
        (-1.0_f32, -1.0, 0.0),
        (1.0, -1.0, std::f32::consts::FRAC_PI_2),
        (-1.0, 1.0, -std::f32::consts::FRAC_PI_2),
        (1.0, 1.0, std::f32::consts::PI),
    ];
    for (i, (sx, sy, rot)) in corners.iter().enumerate() {
        let cx = sx * (edge_x - CORNER_R);
        let cy = sy * (edge_y - CORNER_R);
        commands.spawn((
            Mesh2d(assets.corner_mesh.clone()),
            MeshMaterial2d(assets.corner_material.clone()),
            Transform::from_xyz(cx, cy, -0.5).with_rotation(Quat::from_rotation_z(*rot)),
            ArenaPart::CornerRamp(i as u8),
        ));
    }

    // Face-off drop zone (hockey center ice).
    commands.spawn((
        Sprite {
            color: Color::srgba(0.55, 0.75, 0.95, 0.14),
            custom_size: Some(Vec2::splat(86.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -0.8),
        ArenaPart::FaceOffRing,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgba(0.7, 0.85, 1.0, 0.35),
            custom_size: Some(Vec2::splat(28.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -0.7),
        ArenaPart::FaceOffRing,
    ));

    commands.spawn((
        Sprite {
            color: wall_color,
            custom_size: Some(Vec2::new((ARENA_W - 20.0 - 2.0 * CORNER_R).max(40.0), 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, ARENA_H / 2.0, 0.0),
        ArenaPart::BackboardTop,
    ));
    commands.spawn((
        Sprite {
            color: wall_color,
            custom_size: Some(Vec2::new((ARENA_W - 20.0 - 2.0 * CORNER_R).max(40.0), 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -ARENA_H / 2.0, 0.0),
        ArenaPart::BackboardBottom,
    ));
}

fn spawn_bricks(
    commands: &mut Commands,
    assets: &ArenaAssets,
    snap: &VisualSnapshot,
    materials: &mut Assets<ColorMaterial>,
) {
    for index in 0..(BRICK_COLS as usize * BRICK_ROWS as usize) {
        let max_hp = snap.brick_max_hp[index].max(1);
        let tint = brick_tint(snap.brick_hp[index].max(1).min(max_hp), max_hp);
        // Per-brick materials so HP tint updates don't clobber siblings.
        let face = materials.add(ColorMaterial::from(tint));
        let extrude = materials.add(ColorMaterial::from(Color::mix(
            &tint.with_alpha(0.85),
            &Color::BLACK,
            0.35,
        )));
        let crack = materials.add(ColorMaterial::from(Color::srgba(0.85, 0.92, 1.0, 0.0)));
        commands.spawn((
            Mesh2d(assets.brick_mesh.clone()),
            MeshMaterial2d(assets.shadow_material.clone()),
            Transform::from_xyz(0.0, 0.0, -0.2),
            Visibility::Hidden,
            ArenaPart::BrickShadow(index as u16),
        ));
        commands.spawn((
            Mesh2d(assets.brick_mesh.clone()),
            MeshMaterial2d(extrude),
            Transform::from_xyz(0.0, 0.0, -0.05),
            Visibility::Hidden,
            ArenaPart::BrickExtrude(index as u16),
        ));
        commands.spawn((
            Mesh2d(assets.brick_mesh.clone()),
            MeshMaterial2d(face),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Visibility::Hidden,
            ArenaPart::Brick(index as u16),
        ));
        commands.spawn((
            Mesh2d(assets.brick_mesh.clone()),
            MeshMaterial2d(crack),
            Transform::from_xyz(0.0, 0.0, 0.15),
            Visibility::Hidden,
            ArenaPart::BrickCrack(index as u16),
        ));
    }
}

fn spawn_wild_bricks(commands: &mut Commands, assets: &ArenaAssets) {
    for slot in 0..MAX_WILD_BRICKS {
        commands.spawn((
            Mesh2d(assets.wild_mesh.clone()),
            MeshMaterial2d(assets.shadow_material.clone()),
            Transform::from_xyz(0.0, 0.0, 0.3),
            Visibility::Hidden,
            ArenaPart::WildShadow(slot as u8),
        ));
        commands.spawn((
            Mesh2d(assets.wild_mesh.clone()),
            MeshMaterial2d(assets.wild_materials[0].clone()),
            Transform::from_xyz(0.0, 0.0, 0.5),
            Visibility::Hidden,
            ArenaPart::WildBrick(slot as u8),
        ));
    }
}

fn spawn_paddles_and_ball(commands: &mut Commands, assets: &ArenaAssets, snap: &VisualSnapshot) {
    for player in 0..2 {
        commands.spawn((
            Mesh2d(assets.paddle_mesh.clone()),
            MeshMaterial2d(assets.paddle_materials[player].clone()),
            Transform::from_xyz(0.0, 0.0, 1.0),
            ArenaPart::Paddle(player as u8),
        ));
    }

    commands.spawn((
        Mesh2d(assets.ball_mesh.clone()),
        MeshMaterial2d(assets.shadow_material.clone()),
        Transform::from_xyz(snap.ball_x, snap.ball_y - 6.0, 1.5),
        ArenaPart::BallShadow,
    ));
    commands.spawn((
        Mesh2d(assets.ball_mesh.clone()),
        MeshMaterial2d(assets.ball_materials[2].clone()),
        Transform::from_xyz(snap.ball_x, snap.ball_y, 2.0),
        ArenaPart::Ball,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 1.0, 1.0, 0.55),
            custom_size: Some(Vec2::splat(7.0)),
            ..default()
        },
        Transform::from_xyz(snap.ball_x - 4.0, snap.ball_y + 4.0, 2.2),
        ArenaPart::BallHighlight,
    ));
    commands.spawn((
        Sprite {
            color: Color::srgba(1.0, 0.92, 0.45, 0.0),
            custom_size: Some(Vec2::splat(48.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 2.4),
        Visibility::Hidden,
        ArenaPart::AngleWave,
    ));
    for player in 0u8..2 {
        let color = if player == 0 { P1_COLOR } else { P2_COLOR };
        commands.spawn((
            Sprite {
                color: color.with_alpha(0.0),
                custom_size: Some(Vec2::new(8.0, 4.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 2.35),
            Visibility::Hidden,
            ArenaPart::GrappleLine(player),
        ));
    }
}

fn spawn_labels_and_scores(commands: &mut Commands, assets: &ArenaAssets, bot_match: bool) {
    let labels: [(u8, &str); 2] = [
        (0, "1  P1"),
        (1, if bot_match { "2  CPU" } else { "2  P2" }),
    ];

    for (player, name) in labels {
        commands.spawn((
            Text2d::new(name),
            TextFont {
                font: assets.font.clone(),
                font_size: 24.0,
                ..default()
            },
            TextColor(if player == 0 { P1_COLOR } else { P2_COLOR }),
            TextLayout::new_with_justify(Justify::Center),
            Transform::from_xyz(0.0, 0.0, 4.0),
            ArenaPart::LabelName(player),
        ));
        commands.spawn((
            Text2d::new("0"),
            TextFont {
                font: assets.font.clone(),
                font_size: 56.0,
                ..default()
            },
            TextColor(if player == 0 { P1_COLOR } else { P2_COLOR }),
            TextLayout::new_with_justify(Justify::Center),
            Transform::from_xyz(-ARENA_W / 2.0 + 52.0, 0.0, 5.0),
            ArenaPart::ScoreSide(player),
        ));
        // Match stocks (best of 3) — solid quads (font may lack ◆/◇).
        for slot in 0u8..2 {
            commands.spawn((
                Sprite {
                    color: if player == 0 {
                        Color::srgba(0.35, 0.85, 1.0, 0.35)
                    } else {
                        Color::srgba(1.0, 0.55, 0.25, 0.35)
                    },
                    custom_size: Some(Vec2::new(14.0, 14.0)),
                    ..default()
                },
                Transform::from_xyz(-ARENA_W / 2.0 + 88.0 + slot as f32 * 22.0, 0.0, 5.0),
                ArenaPart::RoundStock { player, slot },
            ));
        }
    }

    commands.spawn((
        Text2d::new("Match over — Enter / R to restart"),
        TextFont {
            font: assets.font.clone(),
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        Transform::from_xyz(0.0, ARENA_H / 2.0 - 40.0, 5.0),
        Visibility::Hidden,
        ArenaPart::MatchOverText,
    ));
}

fn despawn_arena(
    mut commands: Commands,
    roots: Query<Entity, With<ArenaRoot>>,
    parts: Query<Entity, With<ArenaPart>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
}

fn brick_xy(index: usize) -> Vec2 {
    let col = (index % BRICK_COLS as usize) as f32;
    let row = (index / BRICK_COLS as usize) as f32;
    let brick_w = 80.0;
    let brick_h = 24.0;
    let gap = 6.0;
    let grid_w = BRICK_COLS as f32 * brick_w + (BRICK_COLS as f32 - 1.0) * gap;
    let start_x = -grid_w / 2.0 + brick_w / 2.0;
    let start_y = -((BRICK_ROWS as f32 - 1.0) * (brick_h + gap)) / 2.0 + row * (brick_h + gap);
    Vec2::new(start_x + col * (brick_w + gap), start_y)
}

fn sync_arena_visuals(
    time: Res<Time<Fixed>>,
    interp: Res<InterpState>,
    sim: Res<crate::state::SimSnapshot>,
    juice: Option<Res<crate::juice::JuiceState>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    assets: Res<ArenaAssets>,
    mut parts: Query<
        (
            &ArenaPart,
            &mut Transform,
            Option<&mut Visibility>,
            Option<&mut Sprite>,
            Option<&mut MeshMaterial2d<ColorMaterial>>,
        ),
        (
            Without<Text2d>,
            Without<Camera2d>,
            Without<crate::juice::JuiceParticle>,
            Without<crate::juice::BallTrail>,
        ),
    >,
    mut text_entities: Query<
        (
            &ArenaPart,
            &mut Text2d,
            &mut Transform,
            Option<&mut Visibility>,
        ),
        (Without<Camera2d>, Without<Sprite>),
    >,
) {
    if !interp.initialized {
        return;
    }
    let alpha = time.overstep_fraction();
    let snap = interp.sample(alpha);
    let flash = juice.as_ref().map(|j| j.flash).unwrap_or(0.0);
    let pulse_t = sim.world.corner_pulse_t;
    let pulse_id = sim.world.corner_pulse_id;

    for (part, mut transform, vis, sprite, mut mat) in &mut parts {
        match *part {
            ArenaPart::Paddle(player) => {
                let (px, py, pz) = snap.paddle_world(player as usize);
                let paddle = snap.paddles[player as usize];
                let scale = jump_scale_fixed(paddle.jump_z) as f32 / FP_SCALE as f32;
                let y_flip = if player == 1 { -1.0 } else { 1.0 };
                let airborne = paddle_airborne(&paddle) || paddle.jump_z > 0;
                // Strong screen-space lift so jumps are obvious (pz is world units ~0..42).
                let lift = pz * 2.15;
                transform.translation = Vec3::new(px, py + lift, 1.0 + pz * 0.05);
                let angle_deg = paddle.angle as f32 / FP_SCALE as f32;
                let full_charge = paddle.angle.abs() >= 180 * FP_SCALE - FP_SCALE;
                let teeter = if full_charge && paddle.angle_was_held {
                    (sim.world.frame as f32 * 0.55).sin() * 4.5
                } else {
                    0.0
                };
                let angle_rad = -((angle_deg
                    + teeter
                    + if paddle.spin_remain > 0 {
                        paddle.spin_theta as f32 / FP_SCALE as f32
                    } else {
                        0.0
                    })
                .to_radians());
                transform.rotation = Quat::from_rotation_z(angle_rad);
                transform.scale = Vec3::new(scale, scale * y_flip, 1.0);
                let base = if player == 0 { P1_COLOR } else { P2_COLOR };
                if let Some(mat_h) = mat.as_mut() {
                    if let Some(m) = materials.get_mut(&mat_h.0) {
                        let spinning = paddle.spin_remain > 0;
                        let pound_ready = can_ground_pound(&paddle);
                        let wind_t =
                            (paddle.angle.abs() as f32 / (180.0 * FP_SCALE as f32)).clamp(0.0, 1.0);
                        // Tech window: lighter wash so pound is readable at apex.
                        // Wind-up: warm tension tint; full charge → lightning glow.
                        m.color = if full_charge && paddle.angle_was_held {
                            let pulse = 0.55 + 0.45 * (sim.world.frame as f32 * 0.4).sin().abs();
                            Color::srgba(
                                (0.55 + 0.45 * pulse).min(1.0),
                                (0.75 + 0.25 * pulse).min(1.0),
                                1.0,
                                1.0,
                            )
                        } else if pound_ready {
                            Color::srgba(
                                (base.to_srgba().red * 0.35 + 0.65).min(1.0),
                                (base.to_srgba().green * 0.35 + 0.7).min(1.0),
                                (base.to_srgba().blue * 0.35 + 0.75).min(1.0),
                                1.0,
                            )
                        } else if spinning {
                            Color::srgba(
                                (base.to_srgba().red * 0.4 + 0.6).min(1.0),
                                (base.to_srgba().green * 0.45 + 0.55).min(1.0),
                                (base.to_srgba().blue * 0.35 + 0.65).min(1.0),
                                1.0,
                            )
                        } else if wind_t > 0.05 {
                            Color::srgba(
                                (base.to_srgba().red * (1.0 - wind_t * 0.35) + wind_t * 1.0)
                                    .min(1.0),
                                (base.to_srgba().green * (1.0 - wind_t * 0.2) + wind_t * 0.55)
                                    .min(1.0),
                                (base.to_srgba().blue * (1.0 - wind_t * 0.45)).max(0.15),
                                1.0,
                            )
                        } else if airborne {
                            Color::srgba(
                                (base.to_srgba().red * 0.85 + 0.1).min(1.0),
                                (base.to_srgba().green * 0.85 + 0.1).min(1.0),
                                (base.to_srgba().blue * 0.85 + 0.15).min(1.0),
                                1.0,
                            )
                        } else {
                            base
                        };
                    }
                }
            }
            ArenaPart::Ball => {
                transform.translation = Vec3::new(snap.ball_x, snap.ball_y, 2.0);
                let mat_idx = match snap.ball_owner {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                };
                if let Some(mat_h) = mat.as_mut() {
                    mat_h.0 = assets.ball_materials[mat_idx].clone();
                }
            }
            ArenaPart::BallShadow => {
                transform.translation = Vec3::new(snap.ball_x + 3.0, snap.ball_y - 7.0, 1.55);
                transform.scale = Vec3::new(1.05, 0.75, 1.0);
            }
            ArenaPart::BallHighlight => {
                transform.translation = Vec3::new(snap.ball_x - 4.0, snap.ball_y + 4.5, 2.2);
                if let Some(mut sprite) = sprite {
                    let glow = if snap.ball_owner == OWNER_NEUTRAL {
                        0.35
                    } else {
                        0.65
                    };
                    sprite.color = Color::srgba(1.0, 1.0, 1.0, glow);
                }
            }
            ArenaPart::AngleWave => {
                // Filled slab removed — VFX comes from juice crescent trails.
                if let Some(mut v) = vis {
                    *v = Visibility::Hidden;
                }
            }
            ArenaPart::GrappleLine(player) => {
                let p = snap.paddles[player as usize];
                let charging = p.grapple_phase == 0 && p.grapple_charge > 0;
                let tethered = p.grapple_phase != 0;
                if let Some(mut v) = vis {
                    *v = if charging || tethered {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                if charging || tethered {
                    let (px, py, _) = snap.paddle_world(player as usize);
                    let (ax, ay) = if charging {
                        let reach = 70.0 + (p.grapple_charge as f32 / 90.0) * 210.0;
                        (
                            px + p.grapple_dir_x as f32 / FP_SCALE as f32 * reach,
                            py + p.grapple_dir_y as f32 / FP_SCALE as f32 * reach,
                        )
                    } else {
                        (
                            p.grapple_ax as f32 / FP_SCALE as f32,
                            p.grapple_ay as f32 / FP_SCALE as f32,
                        )
                    };
                    let dx = ax - px;
                    let dy = ay - py;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    let angle = dy.atan2(dx);
                    transform.translation = Vec3::new(px + dx * 0.5, py + dy * 0.5, 2.35);
                    transform.rotation = Quat::from_rotation_z(angle);
                    transform.scale = Vec3::ONE;
                    if let Some(mut sprite) = sprite {
                        let base = if player == 0 { P1_COLOR } else { P2_COLOR };
                        let a = if charging { 0.35 } else { 0.85 };
                        sprite.color = base.with_alpha(a);
                        sprite.custom_size = Some(Vec2::new(len, if charging { 3.0 } else { 5.0 }));
                    }
                }
            }
            ArenaPart::Brick(index)
            | ArenaPart::BrickExtrude(index)
            | ArenaPart::BrickShadow(index)
            | ArenaPart::BrickCrack(index) => {
                let idx = index as usize;
                let hp = snap.brick_hp[idx];
                let alive = hp > 0;
                let max_hp = snap.brick_max_hp[idx].max(1);
                let crack_a = brick_crack_alpha(hp, max_hp);
                if let Some(mut v) = vis {
                    *v = if alive {
                        if matches!(*part, ArenaPart::BrickCrack(_)) && crack_a < 0.05 {
                            Visibility::Hidden
                        } else {
                            Visibility::Visible
                        }
                    } else {
                        Visibility::Hidden
                    };
                }
                let xy = brick_xy(idx);
                let depth = (max_hp as f32) * 1.6;
                match *part {
                    ArenaPart::Brick(_) => {
                        transform.translation = Vec3::new(xy.x, xy.y, 0.1);
                        if let Some(mat_h) = mat.as_mut() {
                            if let Some(m) = materials.get_mut(&mat_h.0) {
                                m.color = brick_tint(hp, max_hp);
                            }
                        }
                    }
                    ArenaPart::BrickExtrude(_) => {
                        transform.translation = Vec3::new(xy.x + 2.0, xy.y - depth, 0.02);
                        if let Some(mat_h) = mat.as_mut() {
                            if let Some(m) = materials.get_mut(&mat_h.0) {
                                m.color = brick_tint(hp, max_hp).with_alpha(0.85);
                                m.color = Color::mix(&m.color, &Color::BLACK, 0.35);
                            }
                        }
                    }
                    ArenaPart::BrickShadow(_) => {
                        transform.translation = Vec3::new(xy.x + 5.0, xy.y - depth - 4.0, -0.15);
                        transform.scale = Vec3::new(1.05, 0.9, 1.0);
                    }
                    ArenaPart::BrickCrack(_) => {
                        // Frosted crack plate — denser veins as HP drops.
                        let vein = 0.92 + crack_a * 0.08;
                        transform.translation = Vec3::new(xy.x, xy.y, 0.18);
                        transform.scale = Vec3::new(vein, vein * (0.55 + crack_a * 0.45), 1.0);
                        if let Some(mat_h) = mat.as_mut() {
                            if let Some(m) = materials.get_mut(&mat_h.0) {
                                m.color = Color::srgba(0.82, 0.9, 1.0, crack_a);
                            }
                        }
                    }
                    _ => {}
                }
            }
            ArenaPart::WildBrick(slot) => {
                let w = snap.wild_bricks[slot as usize];
                if let Some(mut v) = vis {
                    *v = if w.active {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                if w.active {
                    let x = w.x as f32 / FP_SCALE as f32;
                    let y = w.y as f32 / FP_SCALE as f32;
                    transform.translation = Vec3::new(x, y, 0.55);
                    let tier = w.hp.max(1).min(3) as usize - 1;
                    if let Some(mat_h) = mat.as_mut() {
                        mat_h.0 = assets.wild_materials[tier].clone();
                    }
                }
            }
            ArenaPart::WildShadow(slot) => {
                let w = snap.wild_bricks[slot as usize];
                if let Some(mut v) = vis {
                    *v = if w.active {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                if w.active {
                    let x = w.x as f32 / FP_SCALE as f32;
                    let y = w.y as f32 / FP_SCALE as f32;
                    transform.translation = Vec3::new(x + 3.0, y - 5.0, 0.35);
                }
            }
            ArenaPart::CornerRamp(id) => {
                let active = pulse_t > 0 && id == pulse_id;
                let t = if active { pulse_t as f32 / 28.0 } else { 0.0 };
                let wave = (t * std::f32::consts::PI).sin().abs();
                let scale = 1.0 + wave * 0.4;
                transform.scale = Vec3::new(scale, scale, 1.0);
                if let Some(mat_h) = mat.as_mut() {
                    if let Some(m) = materials.get_mut(&mat_h.0) {
                        if pulse_t > 0 {
                            m.color = Color::srgb(0.55 + wave * 0.4, 0.75 + wave * 0.2, 1.0);
                        } else {
                            m.color = Color::srgb(0.72, 0.55, 0.92);
                        }
                    }
                }
            }
            ArenaPart::FlashOverlay => {
                if let Some(mut sprite) = sprite {
                    sprite.color = Color::srgba(0.85, 0.7, 1.0, flash * 0.35);
                }
            }
            ArenaPart::FloorShade => {
                // Subtle breathing light on the floor.
                let pulse = 0.92 + (time.overstep_fraction() * 0.0);
                transform.scale = Vec3::new(1.0, pulse, 1.0);
            }
            ArenaPart::RoundStock { player, slot } => {
                let won = snap.rounds_won[player as usize];
                let y = if player == 0 {
                    ARENA_H / 2.0 - 72.0
                } else {
                    -ARENA_H / 2.0 + 72.0
                };
                transform.translation =
                    Vec3::new(-ARENA_W / 2.0 + 88.0 + slot as f32 * 20.0, y, 5.0);
                if let Some(mut sprite) = sprite {
                    let filled = won > slot;
                    sprite.color = if player == 0 {
                        if filled {
                            P1_COLOR
                        } else {
                            Color::srgba(0.35, 0.85, 1.0, 0.28)
                        }
                    } else if filled {
                        P2_COLOR
                    } else {
                        Color::srgba(1.0, 0.55, 0.25, 0.28)
                    };
                }
            }
            _ => {}
        }
    }

    for (part, mut text, mut transform, vis) in &mut text_entities {
        match *part {
            ArenaPart::LabelName(player) => {
                let (px, py, pz) = snap.paddle_world(player as usize);
                let lift = pz * 2.15;
                let dy = if player == 0 { 28.0 } else { -28.0 };
                transform.translation = Vec3::new(px, py + lift + dy, 4.0);
            }
            ArenaPart::LabelHint(_) => {
                if let Some(mut v) = vis {
                    *v = Visibility::Hidden;
                }
            }
            ArenaPart::ScoreSide(player) => {
                text.0 = snap.score[player as usize].to_string();
                let y = if player == 0 {
                    ARENA_H / 2.0 - 72.0
                } else {
                    -ARENA_H / 2.0 + 72.0
                };
                transform.translation = Vec3::new(-ARENA_W / 2.0 + 52.0, y, 5.0);
            }
            ArenaPart::MatchOverText => {
                // HTML shell owns match-end UI (results + rematch vote).
                if let Some(mut v) = vis {
                    *v = Visibility::Hidden;
                }
            }
            _ => {}
        }
    }
}

fn brick_base(max_hp: u8) -> Color {
    match max_hp {
        3 => Color::srgb(0.38, 0.18, 0.68),
        2 => Color::srgb(0.52, 0.28, 0.78),
        _ => Color::srgb(0.66, 0.44, 0.88),
    }
}

fn brick_tint(hp: u8, max_hp: u8) -> Color {
    if hp == 0 {
        return Color::srgba(0.2, 0.14, 0.28, 0.4);
    }
    let base = brick_base(max_hp);
    let damage = (max_hp.saturating_sub(hp)) as f32 / max_hp.max(1) as f32;
    // Fresh = saturated purple; mid hits shift cyan-white; critical → cracked frost.
    let mid = Color::srgb(0.72, 0.82, 0.95);
    let critical = Color::srgb(0.92, 0.95, 1.0);
    if damage < 0.34 {
        Color::mix(&base, &mid, damage * 1.4)
    } else if damage < 0.67 {
        Color::mix(&mid, &critical, (damage - 0.34) / 0.33)
    } else {
        Color::mix(
            &critical,
            &Color::srgb(0.55, 0.62, 0.78),
            (damage - 0.67) / 0.33,
        )
    }
}

fn brick_crack_alpha(hp: u8, max_hp: u8) -> f32 {
    if hp == 0 || max_hp == 0 {
        return 0.0;
    }
    let damage = (max_hp.saturating_sub(hp)) as f32 / max_hp.max(1) as f32;
    (damage * 0.85).clamp(0.0, 0.85)
}

fn wild_tint(hp: u8) -> Color {
    match hp {
        3 => Color::srgb(0.95, 0.25, 0.75),
        2 => Color::srgb(0.85, 0.32, 0.82),
        _ => WILD_BASE,
    }
}

/// Legacy entry — arena now uses persistent entities.
pub fn draw_world() {}
