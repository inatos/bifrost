use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use bifrost_sim::{BotConfig, BotState, MatchPhase, advance_bot, new_match, step};

use crate::input_focus::InputFocus;
use crate::interp::InterpState;
use crate::local_input::local_input_mask;
use crate::session_boot;
use crate::state::{AppState, LaunchConfig, SimSnapshot};

fn bot_session_active(config: Res<LaunchConfig>) -> bool {
    !session_boot::is_online_args(&config.args)
}

#[derive(Resource)]
pub struct LocalBot {
    pub bot: BotState,
    pub cfg: BotConfig,
    pub recording: Vec<bifrost_sim::FrameInput>,
}

impl FromWorld for LocalBot {
    fn from_world(_world: &mut World) -> Self {
        Self {
            bot: BotState::default(),
            cfg: BotConfig::default(),
            recording: Vec::new(),
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<LocalBot>()
        .add_systems(OnEnter(AppState::InGame), start_bot_match)
        .add_systems(
            FixedUpdate,
            bot_tick
                .run_if(in_state(AppState::InGame))
                .run_if(bot_session_active),
        );
}

fn start_bot_match(
    config: Res<LaunchConfig>,
    mut sim: ResMut<SimSnapshot>,
    mut bot: ResMut<LocalBot>,
    mut interp: ResMut<InterpState>,
) {
    if session_boot::is_online_args(&config.args) {
        return;
    }
    let seed = {
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
    };
    sim.world = new_match(seed);
    sim.world.phase = MatchPhase::Serving;
    sim.world.serve_timer = bifrost_sim::SERVE_FRAMES;
    sim.events.clear();
    interp.reset_from(&sim.world);
    bot.bot = BotState::default();
    bot.cfg = BotConfig::default();
    bot.recording.clear();
    if let Some(code) = &config.args.replay {
        if let Ok(replay) = bifrost_sim::decode_replay(code) {
            sim.world = bifrost_sim::simulate_frames(replay.seed, &replay.inputs);
            interp.reset_from(&sim.world);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn shell_sim_paused() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"__bifrostPaused".into()).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn shell_sim_paused() -> bool {
    false
}

fn bot_tick(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform, &Transform)>,
    mut sim: ResMut<SimSnapshot>,
    mut bot: ResMut<LocalBot>,
    mut interp: ResMut<InterpState>,
    mut focus: ResMut<InputFocus>,
    mut anchor: ResMut<crate::input_focus::MouseAimAnchor>,
) {
    if shell_sim_paused() {
        interp.advance(&sim.world);
        return;
    }
    let paddle = sim.world.paddles[0];
    // Keep visual anchor fresh for mouse (same path as online).
    anchor.x = if interp.initialized {
        interp.curr.paddles[0].x
    } else {
        paddle.x
    };
    anchor.y = if interp.initialized {
        interp.curr.paddles[0].y
    } else {
        paddle.y
    };
    anchor.valid = true;
    let p0 = local_input_mask(
        &keys, &mouse, &gamepads, &windows, &camera_q, anchor.x, anchor.y, &mut focus,
    );
    let cfg = bot.cfg;
    let p1 = advance_bot(&sim.world, &mut bot.bot, 1, cfg);
    let input = bifrost_sim::FrameInput { p0, p1 };
    bot.recording.push(input);
    let out = step(&mut sim.world, input);
    sim.events.extend(out.events);
    interp.advance(&sim.world);
}
