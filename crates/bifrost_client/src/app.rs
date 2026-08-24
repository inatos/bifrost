use bevy::prelude::*;

use crate::args::Args;
use crate::bot_mode;
use crate::online_mode;
use crate::state::{AppState, LaunchConfig, SimSnapshot, UiChannel};

pub fn run() {
    let args = Args::from_query_or_cli();
    let online = args.room.is_some() && !args.bot;
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.1)))
        .insert_resource(LaunchConfig { args: args.clone() })
        .init_resource::<SimSnapshot>()
        .init_resource::<UiChannel>()
        .init_state::<AppState>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Bifrost".into(),
                        resolution: (1280, 720).into(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .build(),
        )
        .add_systems(Startup, (setup_camera, bootstrap_state))
        .add_systems(Update, (menu_input, sync_html_shell));

    if online {
        online_mode::plugin(&mut app);
    } else {
        bot_mode::plugin(&mut app);
    }

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn bootstrap_state(config: Res<LaunchConfig>, mut next: ResMut<NextState<AppState>>) {
    if config.args.room.is_some() && !config.args.bot {
        next.set(AppState::Lobby);
    }
}

fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut sim: ResMut<SimSnapshot>,
    config: Res<LaunchConfig>,
) {
    if config.args.room.is_some() && !config.args.bot {
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
        next.set(AppState::InGame);
    }
}

fn sync_html_shell(config: Res<LaunchConfig>, mut ui: ResMut<UiChannel>) {
    if !ui.status.is_empty() {
        return;
    }
    ui.status = if config.args.bot || config.args.room.is_none() {
        "Press Enter — bot match · R — restart".into()
    } else {
        format!(
            "Joining room {}…",
            config.args.room.clone().unwrap_or_default()
        )
    };
}
