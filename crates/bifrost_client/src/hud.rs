//! HTML-shell HUD snapshot (SSBM-style nametags overlay the canvas).

use std::sync::{LazyLock, Mutex};

use bevy::prelude::*;
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::interp::{InterpState, VisualSnapshot};
use crate::session_boot;
use crate::state::{AppState, LaunchConfig, SimSnapshot, UiChannel};

static HUD: LazyLock<Mutex<HudSnapshot>> = LazyLock::new(|| Mutex::new(HudSnapshot::default()));

#[derive(Clone, Debug, Default, Serialize)]
struct HudPlayer {
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, Default, Serialize)]
struct HudViewport {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug, Default, Serialize)]
struct HudStats {
    bricks_broken: [u16; 2],
    goals: [u8; 2],
    paddle_hits: [u16; 2],
    wild_burst: u16,
    spins: u16,
    longest_rally: u32,
    match_frames: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
struct HudLobby {
    /// idle | host_wait | guest_wait | ready | match
    phase: String,
    waiting: bool,
    peers: u8,
    room: Option<String>,
    status: String,
}

#[derive(Clone, Debug, Default, Serialize)]
struct HudSnapshot {
    in_game: bool,
    bot: bool,
    p0: HudPlayer,
    p1: HudPlayer,
    score: [u8; 2],
    rounds: [u8; 2],
    owner: u8,
    match_over: bool,
    /// Winner player index when match_over; otherwise unused.
    winner: u8,
    view: HudViewport,
    stats: HudStats,
    p0_spin_charge: u16,
    phase: String,
    ready: [bool; 2],
    round_remain: u32,
    round_breaks: [u16; 2],
    lobby: HudLobby,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, publish_hud);
}

fn publish_hud(
    sim: Res<SimSnapshot>,
    config: Res<LaunchConfig>,
    interp: Res<InterpState>,
    time: Res<Time<Fixed>>,
    ui: Res<UiChannel>,
    app_state: Res<State<AppState>>,
    windows: Query<&Window>,
    cameras: Query<&Camera>,
) {
    let bot = !session_boot::is_online_args(&config.args);
    let in_game = *app_state.get() == AppState::InGame;
    let lobby = HudLobby {
        phase: ui.lobby_phase.clone(),
        waiting: ui.lobby_waiting,
        peers: ui.lobby_peers,
        room: ui.room_code.clone().or_else(|| config.args.room.clone()),
        status: ui.status.clone(),
    };

    if !in_game {
        *HUD.lock().expect("hud lock") = HudSnapshot {
            in_game: false,
            bot,
            lobby,
            ..Default::default()
        };
        return;
    }

    let snap = if interp.initialized {
        interp.sample(time.overstep_fraction())
    } else {
        VisualSnapshot::from_world(&sim.world)
    };
    let view = viewport_norm(&windows, &cameras);
    let (p0x, p0y) = snap.arena_norm(snap.paddle_world(0).0, snap.paddle_world(0).1);
    let (p1x, p1y) = snap.arena_norm(snap.paddle_world(1).0, snap.paddle_world(1).1);
    let match_over = sim.world.phase == bifrost_sim::MatchPhase::MatchOver;
    let winner = if sim.world.rounds_won[0] > sim.world.rounds_won[1] {
        0
    } else {
        1
    };
    let phase = match sim.world.phase {
        bifrost_sim::MatchPhase::Readying => "readying",
        bifrost_sim::MatchPhase::Serving => "serving",
        bifrost_sim::MatchPhase::Rally => "rally",
        bifrost_sim::MatchPhase::GoalPause => "goal_pause",
        bifrost_sim::MatchPhase::MatchOver => "match_over",
    };
    let snap = HudSnapshot {
        in_game: true,
        bot,
        p0: HudPlayer {
            x: p0x,
            y: (p0y - 0.055).clamp(0.02, 0.98),
        },
        p1: HudPlayer {
            x: p1x,
            y: (p1y - 0.055).clamp(0.02, 0.98),
        },
        score: sim.world.score,
        rounds: sim.world.rounds_won,
        owner: snap.ball_owner,
        match_over,
        winner,
        view,
        stats: HudStats {
            bricks_broken: sim.world.stats.bricks_broken,
            goals: sim.world.stats.goals,
            paddle_hits: sim.world.stats.paddle_hits,
            wild_burst: sim.world.stats.wild_burst,
            spins: sim.world.stats.spins,
            longest_rally: sim.world.stats.longest_rally,
            match_frames: sim.world.frame,
        },
        p0_spin_charge: sim.world.paddles[0].spin_charge,
        phase: phase.into(),
        ready: sim.world.ready,
        round_remain: sim.world.round_timer,
        round_breaks: sim.world.round_breaks,
        lobby: HudLobby {
            phase: if ui.lobby_phase == "disconnected" {
                "disconnected".into()
            } else {
                ui.lobby_phase.clone()
            },
            waiting: ui.lobby_waiting,
            peers: ui.lobby_peers,
            room: lobby.room,
            status: ui.status.clone(),
        },
    };
    *HUD.lock().expect("hud lock") = snap;
}

fn viewport_norm(windows: &Query<&Window>, cameras: &Query<&Camera>) -> HudViewport {
    let Ok(window) = windows.single() else {
        return HudViewport {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        };
    };
    let Ok(camera) = cameras.single() else {
        return HudViewport {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        };
    };
    let logical = window.resolution.width();
    let logical_h = window.resolution.height();
    if logical <= 0.0 || logical_h <= 0.0 {
        return HudViewport {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        };
    }
    if let Some(rect) = camera.logical_viewport_rect() {
        return HudViewport {
            left: rect.min.x / logical,
            top: rect.min.y / logical_h,
            width: rect.width() / logical,
            height: rect.height() / logical_h,
        };
    }
    HudViewport {
        left: 0.0,
        top: 0.0,
        width: 1.0,
        height: 1.0,
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn bifrost_hud() -> String {
    serde_json::to_string(&*HUD.lock().expect("hud lock")).unwrap_or_else(|_| "{}".into())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bifrost_hud_json() -> String {
    serde_json::to_string(&*HUD.lock().expect("hud lock")).unwrap_or_else(|_| "{}".into())
}
