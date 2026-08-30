use bevy::camera::{OrthographicProjection, ScalingMode};
use bevy::prelude::*;

use crate::args::Args;
use crate::bot_mode;
use crate::hud;
use crate::input_focus::InputFocus;
use crate::interp::InterpState;
use crate::juice;
use crate::online_mode;
use crate::render;
use crate::session_boot;
use crate::state::{AppState, LaunchConfig, SimSnapshot, UiChannel};

const ARENA_W: f32 = 1200.0;
const ARENA_H: f32 = 480.0;

pub fn run() {
    let args = Args::from_query_or_cli();
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.1)))
        .insert_resource(LaunchConfig { args: args.clone() })
        .init_resource::<SimSnapshot>()
        .init_resource::<InterpState>()
        .init_resource::<InputFocus>()
        .init_resource::<crate::input_focus::MouseAimAnchor>()
        .init_resource::<UiChannel>()
        .insert_resource(Time::<Fixed>::from_hz(bifrost_sim::TICKS_PER_SECOND as f64))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Bifrost".into(),
                        resolution: (1280, 720).into(),
                        canvas: Some("#bevy-canvas".into()),
                        // Do not use fit_canvas_to_parent — it collapses the Lab embed
                        // layout (height: 100% of indefinite parent) to a black zero-size canvas.
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // Trunk copies `assets/` to dist root; wasm loads from site root.
                    file_path: String::from("assets"),
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                .build(),
        )
        .init_state::<AppState>()
        .add_systems(Startup, (setup_camera, bootstrap_state))
        .add_systems(Update, (menu_input, sync_html_shell));

    render::plugin(&mut app);
    juice::plugin(&mut app);
    hud::plugin(&mut app);
    online_mode::plugin(&mut app);
    bot_mode::plugin(&mut app);
    session_boot::register(&mut app);

    app.run();
}

fn setup_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.area = Rect::new(-ARENA_W / 2.0, -ARENA_H / 2.0, ARENA_W / 2.0, ARENA_H / 2.0);
    // Zoom out so the board clears Lab chrome (lock banner / footer).
    projection.scaling_mode = ScalingMode::AutoMin {
        min_width: ARENA_W * 1.18,
        min_height: ARENA_H * 1.22,
    };
    commands.spawn((Camera2d, Projection::Orthographic(projection)));
}

fn bootstrap_state(config: Res<LaunchConfig>, mut next: ResMut<NextState<AppState>>) {
    if config.args.is_online() {
        next.set(AppState::Lobby);
    } else {
        next.set(AppState::InGame);
    }
}

fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut sim: ResMut<SimSnapshot>,
    mut interp: ResMut<InterpState>,
    config: Res<LaunchConfig>,
) {
    if config.args.is_online() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let seed = session_boot_seed();
        sim.world = bifrost_sim::new_match(seed);
        sim.events.clear();
        interp.reset_from(&sim.world);
        next.set(AppState::InGame);
        return;
    }
    if state.get() != &AppState::Menu {
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::InGame);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        sim.world = bifrost_sim::new_match(0xB1F05E);
        sim.events.clear();
        interp.reset_from(&sim.world);
        next.set(AppState::InGame);
    }
}

fn sync_html_shell(config: Res<LaunchConfig>, mut ui: ResMut<UiChannel>) {
    if !ui.status.is_empty() {
        return;
    }
    ui.status = if config.args.is_online() {
        format!(
            "Joining room {}…",
            config.args.room.clone().unwrap_or_default()
        )
    } else {
        "Bot match — WASD / arrows / mouse / gamepad · R restart".into()
    };
}

fn session_boot_seed() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64 ^ 0xB1F05E
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xB1F05E)
            ^ 0xB1F05E
    }
}
