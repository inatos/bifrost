use bevy::prelude::*;
use bifrost_sim::{MatchPhase, WorldState, BRICK_COLS, BRICK_ROWS, FP_SCALE};

use crate::state::SimSnapshot;

const ARENA_W: f32 = 900.0;
const ARENA_H: f32 = 600.0;

#[derive(Component)]
pub(crate) struct ArenaSprite;

pub fn draw_world(
    mut commands: Commands,
    sim: Res<SimSnapshot>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing: Query<Entity, With<ArenaSprite>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    spawn_world(&mut commands, &mut meshes, &mut materials, &sim.world);
}

fn spawn_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    world: &WorldState,
) {
    let wall_color = Color::srgb(0.75, 0.78, 0.9);
    let bg = Color::srgb(0.08, 0.07, 0.14);
    commands.spawn((
        Sprite {
            color: bg,
            custom_size: Some(Vec2::new(ARENA_W + 40.0, ARENA_H + 40.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
        ArenaSprite,
    ));

    for x in [-ARENA_W / 2.0, ARENA_W / 2.0] {
        commands.spawn((
            Sprite {
                color: wall_color,
                custom_size: Some(Vec2::new(10.0, ARENA_H)),
                ..default()
            },
            Transform::from_xyz(x, 0.0, 0.0),
            ArenaSprite,
        ));
    }

    for y in [-ARENA_H / 2.0, ARENA_H / 2.0] {
        commands.spawn((
            Sprite {
                color: Color::srgba(0.3, 0.9, 0.95, 0.35),
                custom_size: Some(Vec2::new(ARENA_W - 40.0, 4.0)),
                ..default()
            },
            Transform::from_xyz(0.0, y, 0.0),
            ArenaSprite,
        ));
    }

    for index in 0..(BRICK_COLS as usize * BRICK_ROWS as usize) {
        if !world.brick_alive(index) {
            continue;
        }
        let (cx, cy) = world.brick_center(index).to_f();
        commands.spawn((
            Sprite {
                color: Color::srgb(0.45, 0.65, 1.0),
                custom_size: Some(Vec2::new(76.0, 22.0)),
                ..default()
            },
            Transform::from_xyz(cx, cy, 0.0),
            ArenaSprite,
        ));
    }

    let paddle_colors = [Color::srgb(0.2, 0.85, 0.95), Color::srgb(0.95, 0.55, 0.25)];
    for player in 0..2 {
        let px = world.paddles[player].x as f32 / FP_SCALE as f32;
        let py = world.paddle_y(player) as f32 / FP_SCALE as f32;
        commands.spawn((
            Sprite {
                color: paddle_colors[player],
                custom_size: Some(Vec2::new(120.0, 16.0)),
                ..default()
            },
            Transform::from_xyz(px, py, 1.0),
            ArenaSprite,
        ));
    }

    let (bx, by) = world.ball.pos.to_f();
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(14.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(1.0, 0.85, 0.55)))),
        Transform::from_xyz(bx, by, 2.0),
        ArenaSprite,
    ));

    if world.phase == MatchPhase::MatchOver {
        commands.spawn((
            Text2d::new("Match over — Enter to restart"),
            TextFont {
                font_size: 28.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, ARENA_H / 2.0 - 40.0, 5.0),
            ArenaSprite,
        ));
    }
}
