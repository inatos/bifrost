//! In-memory session handoff from the HTML shell (no URL tickets, no reload).

use std::sync::{LazyLock, Mutex};

use bevy::prelude::*;

use bifrost_sim::MatchPhase;

use crate::args::Args;
use crate::bot_mode::LocalBot;
use crate::interp::InterpState;
use crate::state::{AppState, LaunchConfig, SimSnapshot};

#[derive(Clone)]
enum PendingSession {
    Bot,
    Online { room: String, ticket: String },
    /// Tear down GGRS/Matchbox and return to idle bot board.
    Leave,
}

static PENDING: LazyLock<Mutex<Option<PendingSession>>> =
    LazyLock::new(|| Mutex::new(None));

/// When true, bounce Menu → Lobby next frame so Matchbox OnEnter re-fires.
static DEFERRED_LOBBY: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

#[cfg(target_arch = "wasm32")]
mod wasm_exports {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn bifrost_start_bot() {
        *PENDING.lock().expect("session lock") = Some(PendingSession::Bot);
    }

    #[wasm_bindgen]
    pub fn bifrost_connect(room: String, ticket: String) {
        *PENDING.lock().expect("session lock") =
            Some(PendingSession::Online { room, ticket });
    }

    #[wasm_bindgen]
    pub fn bifrost_leave_match() {
        *PENDING.lock().expect("session lock") = Some(PendingSession::Leave);
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_exports::*;

pub fn register(app: &mut App) {
    app.add_systems(Update, (apply_pending_session, flush_deferred_lobby));
}

fn reset_bot_world(sim: &mut SimSnapshot, interp: &mut InterpState, bot: &mut LocalBot) {
    sim.world = bifrost_sim::new_match(fresh_seed());
    sim.world.phase = MatchPhase::Serving;
    sim.world.serve_timer = bifrost_sim::SERVE_FRAMES;
    sim.events.clear();
    interp.reset_from(&sim.world);
    bot.bot = bifrost_sim::BotState::default();
    bot.cfg = bifrost_sim::BotConfig::default();
    bot.recording.clear();
}

fn apply_pending_session(
    mut commands: Commands,
    mut config: ResMut<LaunchConfig>,
    mut next: ResMut<NextState<AppState>>,
    mut sim: ResMut<SimSnapshot>,
    mut interp: ResMut<InterpState>,
    mut bot: ResMut<LocalBot>,
    state: Res<State<AppState>>,
) {
    let pending = PENDING.lock().expect("session lock").take();
    match pending {
        None => {}
        Some(PendingSession::Bot) => {
            *DEFERRED_LOBBY.lock().expect("lobby lock") = false;
            config.args.room = None;
            config.args.ticket = None;
            config.args.bot = true;
            reset_bot_world(&mut sim, &mut interp, &mut bot);
            if state.get() == &AppState::InGame {
                // Already in-game — reset in place (OnEnter won't fire again).
            } else {
                next.set(AppState::InGame);
            }
        }
        Some(PendingSession::Leave) => {
            *DEFERRED_LOBBY.lock().expect("lobby lock") = false;
            config.args.room = None;
            config.args.ticket = None;
            config.args.bot = true;
            // Drop online session resources so RollbackWorld cannot overwrite Readying.
            commands.remove_resource::<bevy_ggrs::Session<crate::online_mode::BifrostConfig>>();
            commands.remove_resource::<bevy_matchbox::prelude::MatchboxSocket>();
            commands.remove_resource::<crate::online_mode::RollbackWorld>();
            reset_bot_world(&mut sim, &mut interp, &mut bot);
            next.set(AppState::InGame);
        }
        Some(PendingSession::Online { room, ticket }) => {
            // Page-origin Matchbox URL — never leave the clap default :3536.
            #[cfg(target_arch = "wasm32")]
            {
                config.args.signal = crate::args::wasm_signal_base();
            }
            config.args.room = Some(room);
            config.args.ticket = Some(ticket);
            config.args.bot = false;
            if state.get() == &AppState::Lobby {
                // Force OnEnter(Lobby) to rebuild the Matchbox socket.
                *DEFERRED_LOBBY.lock().expect("lobby lock") = true;
                next.set(AppState::Menu);
            } else {
                next.set(AppState::Lobby);
            }
        }
    }
}

fn flush_deferred_lobby(
    mut next: ResMut<NextState<AppState>>,
    state: Res<State<AppState>>,
    config: Res<LaunchConfig>,
) {
    let mut deferred = DEFERRED_LOBBY.lock().expect("lobby lock");
    if !*deferred {
        return;
    }
    if state.get() != &AppState::Menu {
        return;
    }
    if !is_online_args(&config.args) {
        *deferred = false;
        return;
    }
    *deferred = false;
    next.set(AppState::Lobby);
}

fn fresh_seed() -> u64 {
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

pub fn is_online_args(args: &Args) -> bool {
    args.room.is_some() && !args.bot && args.ticket.is_some()
}
