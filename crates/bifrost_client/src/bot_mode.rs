use bevy::prelude::*;
use bifrost_sim::{advance_bot, new_match, step, BotConfig, BotState, MatchPhase};

use crate::render::draw_world;
use crate::state::{AppState, LaunchConfig, SimSnapshot};

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
            Update,
            (bot_tick, draw_world).run_if(in_state(AppState::InGame)),
        );
}

fn start_bot_match(
    config: Res<LaunchConfig>,
    mut sim: ResMut<SimSnapshot>,
    mut bot: ResMut<LocalBot>,
) {
    let seed = 0xB1F05E_u64;
    sim.world = new_match(seed);
    sim.events.clear();
    bot.bot = BotState::default();
    bot.cfg = BotConfig::default();
    bot.recording.clear();
    if let Some(code) = &config.args.replay {
        if let Ok(replay) = bifrost_sim::decode_replay(code) {
            sim.world = bifrost_sim::simulate_frames(replay.seed, &replay.inputs);
        }
    }
}

fn bot_tick(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimSnapshot>,
    mut bot: ResMut<LocalBot>,
) {
    if sim.world.phase == MatchPhase::MatchOver {
        return;
    }
    let mut p0 = 0u8;
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        p0 |= bifrost_sim::INPUT_LEFT;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        p0 |= bifrost_sim::INPUT_RIGHT;
    }
    let cfg = bot.cfg;
    let p1 = advance_bot(&sim.world, &mut bot.bot, 1, cfg);
    let input = bifrost_sim::FrameInput { p0, p1 };
    bot.recording.push(input);
    let out = step(&mut sim.world, input);
    sim.events.extend(out.events);
}
