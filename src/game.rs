use bevy::{
    prelude::*,
    sprite::{collide_aabb::{collide, Collision}},
    sprite::MaterialMesh2dBundle
};
use bevy_ggrs::*;
use crate::components::*;
use crate::input::*;
use crate::netcode::*;


// Breakout
// Multiplayer implementation of the classic game "Breakout".

// Defines the amount of time that should elapse between each physics step.
const TIME_STEP: f32 = 1.0 / 60.0;
const FPS_LIMIT: usize = 60;

// Player defaults
const PLAYER_MOVE_SPEED: f32 = 7.0;
const P1_START_POSITION: Vec3 = Vec3::new(-200.0, BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR, 0.0);
const P2_START_POSITION: Vec3 = Vec3::new(200.0, BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR, 0.0);

// Paddle
const PADDLE_SIZE: Vec3 = Vec3::new(120.0, 20.0, 0.0);
const GAP_BETWEEN_PADDLE_AND_FLOOR: f32 = 60.0;
// How close can the paddle get to the wall
const PADDLE_PADDING: f32 = 10.0;

// Ball
// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -50.0, 1.0);
const BALL_SIZE: Vec3 = Vec3::new(30.0, 30.0, 0.0);
const BALL_SPEED: f32 = 400.0;
const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);

// Walls
const WALL_THICKNESS: f32 = 10.0;
// x coordinates
const LEFT_WALL: f32 = -450.0;
const RIGHT_WALL: f32 = 450.0;
// y coordinates
const BOTTOM_WALL: f32 = -300.0;
const TOP_WALL: f32 = 300.0;
const DIVIDER_WALL: f32 = 0.0;

// Bricks
const BRICK_SIZE: Vec2 = Vec2::new(100.0, 30.0);
const GAP_BETWEEN_PADDLE_AND_BRICKS: f32 = 270.0;
const GAP_BETWEEN_BRICKS: f32 = 5.0;
const GAP_BETWEEN_BRICKS_AND_CEILING: f32 = 20.0;
const GAP_BETWEEN_BRICKS_AND_SIDES: f32 = 20.0;

// Scoreboard
const SCOREBOARD_FONT_SIZE: f32 = 40.0;
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

// Colors for game objects
const P1_COLOR: Color = Color::rgb(0.0, 0.47, 1.0);
const P2_COLOR: Color = Color::rgb(0.0, 0.4, 0.0);
const DIVIDER_COLOR: Color = Color::rgb(0.3, 0.3, 0.3);
const BACKGROUND_COLOR: Color = Color::rgb(0.2, 0.2, 0.2);
const BALL_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
const BRICK_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const WALL_COLOR: Color = Color::rgb(0.8, 0.8, 0.8);
const TEXT_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);


// This resource tracks the game's score
#[derive(Resource, Default, Reflect)]
struct Scoreboard {
    score: usize,
}

#[derive(Resource, Default, Reflect)]
struct CollisionSound(Handle<AudioSource>);

#[derive(Bundle)]
struct DividerBundle {
    sprite_bundle: SpriteBundle,
}

impl DividerBundle {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    fn new(location: WallLocation) -> DividerBundle {
        DividerBundle {
            sprite_bundle: SpriteBundle {
                transform: Transform {
                    // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                    // This is used to determine the order of our sprites
                    translation: location.position().extend(0.0),
                    // The z-scale of 2D objects must always be 1.0,
                    // or their ordering will be affected in surprising ways.
                    // See https://github.com/bevyengine/bevy/issues/4149
                    scale: location.size().extend(1.0),
                    ..default()
                },
                sprite: Sprite {
                    color: DIVIDER_COLOR,
                    ..default()
                },
                ..default()
            },
        }
    }
}

/// Which side of the arena is this wall located on?
enum WallLocation {
    Left,
    Right,
    Bottom,
    Top,
    Center
}

impl WallLocation {
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL, 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL, 0.),
            WallLocation::Bottom => Vec2::new(0., BOTTOM_WALL),
            WallLocation::Top => Vec2::new(0., TOP_WALL),
            WallLocation::Center => Vec2::new(0., DIVIDER_WALL),
        }
    }

    fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        // Make sure we haven't messed up our constants
        assert!(arena_height > 0.0);
        assert!(arena_width > 0.0);

        match self {
            WallLocation::Left | WallLocation::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            WallLocation::Bottom | WallLocation::Top => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
            WallLocation::Center => {
                Vec2::new(arena_width -5., WALL_THICKNESS)
            }
        }
    }
}

#[derive(Bundle)]
struct WallBundle {
    sprite_bundle: SpriteBundle,
    collider: Collider,
}

impl WallBundle {
    fn new(location: WallLocation) -> WallBundle {
        WallBundle {
            sprite_bundle: SpriteBundle {
                transform: Transform {
                    // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                    // This is used to determine the order of our sprites
                    translation: location.position().extend(0.0),
                    // The z-scale of 2D objects must always be 1.0,
                    // or their ordering will be affected in surprising ways.
                    // See https://github.com/bevyengine/bevy/issues/4149
                    scale: location.size().extend(1.0),
                    ..default()
                },
                sprite: Sprite {
                    color: WALL_COLOR,
                    ..default()
                },
                ..default()
            },
            collider: Collider,
        }
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * TIME_STEP;
        transform.translation.y += velocity.y * TIME_STEP;
    }
}

fn update_scoreboard(scoreboard: Res<Scoreboard>, mut query: Query<&mut Text>) {
    let mut text = query.single_mut();
    text.sections[1].value = scoreboard.score.to_string();
}

fn check_for_collisions(
    mut commands: Commands,
    mut scoreboard: ResMut<Scoreboard>,
    mut ball_query: Query<(&mut Velocity, &Transform), With<Ball>>,
    collider_query: Query<(Entity, &Transform, Option<&Brick>), With<Collider>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.single_mut();
    let ball_size = ball_transform.scale.truncate();

    // Check wall collision
    for (collider_entity, transform, maybe_brick) in &collider_query {
        let collision = collide(
            ball_transform.translation,
            ball_size,
            transform.translation,
            transform.scale.truncate(), 
        );

        if let Some(collision) = collision { 
            // Sends a collision event so that other systems can react to the collision
            collision_events.send_default();

            

            // Bricks should be despawned and increment the scoreboard on collision
            if maybe_brick.is_some() {
                scoreboard.score += 1;
                commands.entity(collider_entity).despawn();
            }

            // Reflect the ball when it collides
            let mut reflect_x = false;
            let mut reflect_y = false;

            // Only reflect if the ball's velocity is going in the opposite direction of the collision
            match collision {
                Collision::Left => reflect_x = ball_velocity.x > 0.0,
                Collision::Right => reflect_x = ball_velocity.x < 0.0,
                Collision::Top => reflect_y = ball_velocity.y < 0.0,
                Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
                Collision::Inside => { /* do nothing */ }
            }

            // Reflect velocity on the x-axis if we hit something on the x-axis
            if reflect_x {
                ball_velocity.x = -ball_velocity.x;
            }

            // Reflect velocity on the y-axis if we hit something on the y-axis
            if reflect_y {
                ball_velocity.y = -ball_velocity.y;
            }
        }
    }
}

fn play_collision_sound(
    collision_events: EventReader<CollisionEvent>,
    audio: Res<Audio>,
    sound: Res<CollisionSound>,
) {
    // Play a sound once per frame if a collision occurred.
    if !collision_events.is_empty() {
        // This prevents events staying active on the next frame.
        collision_events.clear();
        audio.play(sound.0.clone());
    }
}

fn spawn_realm(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Sound
    let ball_collision_sound = asset_server.load("../assets/sounds/oof.ogg");
    commands.insert_resource(CollisionSound(ball_collision_sound));

    // Ball
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::default().into()).into(),
            material: materials.add(ColorMaterial::from(BALL_COLOR)),
            transform: Transform::from_translation(BALL_STARTING_POSITION).with_scale(BALL_SIZE),
            ..default()
        },
        Ball,
        Velocity(INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED),
    ));

    // Scoreboard
    commands.spawn(
        TextBundle::from_sections([
            TextSection::new(
                "Score: ",
                TextStyle {
                    font: asset_server.load("../assets/fonts/FiraSans-Bold.ttf"),
                    font_size: SCOREBOARD_FONT_SIZE,
                    color: TEXT_COLOR,
                },
            ),
            TextSection::from_style(TextStyle {
                font: asset_server.load("../assets/fonts/FiraMono-Medium.ttf"),
                font_size: SCOREBOARD_FONT_SIZE,
                color: SCORE_COLOR,
            }),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            position: UiRect {
                top: SCOREBOARD_TEXT_PADDING,
                left: SCOREBOARD_TEXT_PADDING,
                ..default()
            },
            ..default()
        }),
    );

    // Walls
    commands.spawn(WallBundle::new(WallLocation::Left));
    commands.spawn(WallBundle::new(WallLocation::Right));
    commands.spawn(WallBundle::new(WallLocation::Bottom));
    commands.spawn(WallBundle::new(WallLocation::Top));
    commands.spawn(DividerBundle::new(WallLocation::Center));

    // Bricks
    // Negative scales result in flipped sprites / meshes,
    // which is definitely not what we want here
    assert!(BRICK_SIZE.x > 0.0);
    assert!(BRICK_SIZE.y > 0.0);

    let paddle_y = BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR;

    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // Given the space available, compute how many rows and columns of bricks we can fit
    let n_columns = (total_width_of_bricks / (BRICK_SIZE.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (BRICK_SIZE.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // Because we need to round the number of columns,
    // the space on the top and sides of the bricks only captures a lower bound, not an exact value
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        // Space taken up by the bricks
        - (n_columns as f32 / 2.0 * BRICK_SIZE.x)
        // Space taken up by the gaps
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // In Bevy, the `translation` of an entity describes the center point,
    // not its bottom-left corner
    let offset_x = left_edge_of_bricks + BRICK_SIZE.x / 2.;
    let offset_y = bottom_edge_of_bricks + BRICK_SIZE.y / 2.;

    for row in 0..n_rows {
        for column in 0..n_columns {
            let brick_position = Vec2::new(
                offset_x + column as f32 * (BRICK_SIZE.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (BRICK_SIZE.y + GAP_BETWEEN_BRICKS),
            );

            // brick
            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: BRICK_COLOR,
                        ..default()
                    },
                    transform: Transform {
                        translation: brick_position.extend(0.0),
                        scale: Vec3::new(BRICK_SIZE.x, BRICK_SIZE.y, 1.0),
                        ..default()
                    },
                    ..default()
                },
                Brick,
                Collider,
            ));
        }
    }
}

pub fn spawn_players(mut commands: Commands, mut rip: ResMut<RollbackIdProvider>) {

    // Player 1
    commands.spawn((
        Player { handle: 0 },
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform{
                translation: P1_START_POSITION,
                scale: PADDLE_SIZE,
                ..default()
            },
            sprite: Sprite {
                color: P1_COLOR,
                //custom_size: Some(PLAYER_SPRITE_SIZE),
                ..default()
            },
            ..default()
        },
        Paddle,
        Collider,
    ));

    // Player 2
    commands.spawn((
        Player { handle: 1 },
        Rollback::new(rip.next_id()),
        SpriteBundle {
            transform: Transform{
                translation: P2_START_POSITION,
                scale: PADDLE_SIZE,
                ..default()
            },
            sprite: Sprite {
                color: P2_COLOR,
                //custom_size: Some(PLAYER_SPRITE_SIZE),
                ..default()
            },
            ..default()
        },
        Paddle,
        Collider,
    ));
}

fn move_players(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut player_query: Query<(&mut Transform, &Player), With<Rollback>>,
) {
    for (mut transform, player) in player_query.iter_mut() {
        let direction = direction(inputs[player.handle].0);

        if direction == Vec2::ZERO {
            continue;
        }

        let move_speed = PLAYER_MOVE_SPEED;
        let move_delta = (direction * move_speed).extend(0.0);

        transform.translation += move_delta;

        // Calculate the new horizontal paddle position based on player input
        let new_paddle_position_x = transform.translation.x;
        let new_paddle_position_y = transform.translation.y;

        // Update the paddle position,
        // making sure it doesn't cause the paddle to leave the arena
        let left_bound = LEFT_WALL + WALL_THICKNESS / 2.0 + PADDLE_SIZE.x / 2.0 + PADDLE_PADDING;
        let right_bound = RIGHT_WALL - WALL_THICKNESS / 2.0 - PADDLE_SIZE.x / 2.0 - PADDLE_PADDING;
        transform.translation.x = new_paddle_position_x.clamp(left_bound, right_bound);

        let top_bound = DIVIDER_WALL - WALL_THICKNESS / 2.0 - PADDLE_SIZE.y / 2.0 - PADDLE_PADDING;
        let bottom_bound = BOTTOM_WALL + WALL_THICKNESS / 2.0 + PADDLE_SIZE.y / 2.0 + PADDLE_PADDING;
        transform.translation.y = new_paddle_position_y.clamp(bottom_bound, top_bound);

    }
}


/// Builds shared and local contexts of the game.
pub fn build_app(app: &mut App) {

    // Build shared state
    GGRSPlugin::<GgrsConfig>::new()
        .with_update_frequency(FPS_LIMIT)
        .with_input_system(input)
        .with_rollback_schedule(Schedule::default().with_stage(
            "ROLLBACK_STAGE",
            SystemStage::single_threaded()
                .with_system(move_players)
                .with_system(apply_velocity)
                .with_system(check_for_collisions)
                .with_system(play_collision_sound.after(check_for_collisions))
                .with_system(increase_frame_system)
        ))
        .register_rollback_component::<Transform>()
        .register_rollback_component::<Velocity>() 
        .register_rollback_component::<CollisionEvent>()
        .register_rollback_resource::<Scoreboard>()
        .register_rollback_resource::<FrameCount>()
        .register_rollback_resource::<CollisionSound>()
        .build(app);

    // Build local state
    app.insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_startup_system(spawn_realm)
        .add_startup_system(start_matchbox_socket)
        .add_startup_system(spawn_players)
        .add_system(wait_for_players)
        .add_system(update_scoreboard)
        .add_system(bevy::window::close_on_esc)
        .insert_resource(FrameCount { frame: 0 })
        .insert_resource(Scoreboard { score: 0 })
        .add_event::<CollisionEvent>();
}