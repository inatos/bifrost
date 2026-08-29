//! GGRS + Matchbox P2P online session.

use bevy::input::gamepad::Gamepad;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_ggrs::ggrs::{DesyncDetection, GgrsEvent, SessionBuilder};
use bevy_ggrs::prelude::*;
use bevy_ggrs::{LocalInputs, LocalPlayers};
use bevy_matchbox::matchbox_socket::PeerId;
use bevy_matchbox::prelude::*;
use bifrost_net::{BifrostInput, DEFAULT_INPUT_DELAY, DEFAULT_MAX_PREDICTION, FPS};
use bifrost_sim::{new_match, step, FrameInput, MatchPhase, WorldState};

use crate::local_input::local_input_mask;

use crate::interp::InterpState;
use crate::input_focus::InputFocus;
use crate::net_mode::NetSession;
use crate::session_boot;
use crate::state::{AppState, LaunchConfig, SimSnapshot, UiChannel};

fn online_session_active(config: Res<LaunchConfig>) -> bool {
    session_boot::is_online_args(&config.args)
}

pub type BifrostConfig = GgrsConfig<BifrostInput, PeerId>;

const NUM_PLAYERS: usize = 2;

#[derive(Resource, Clone, PartialEq, Eq)]
pub struct RollbackWorld(pub WorldState);

pub fn plugin(app: &mut App) {
    app.add_plugins(GgrsPlugin::<BifrostConfig>::default())
        .insert_resource(RollbackFrameRate(FPS))
        .rollback_resource_with_clone::<RollbackWorld>()
        .checksum_resource(|world: &RollbackWorld| bifrost_sim::checksum(&world.0))
        .insert_resource(NetSession::new())
        .add_systems(ReadInputs, read_local_inputs.run_if(online_session_active))
        .add_systems(
            OnEnter(AppState::Lobby),
            start_matchbox_socket.run_if(online_session_active),
        )
        .add_systems(
            Update,
            lobby_system
                .run_if(in_state(AppState::Lobby))
                .run_if(online_session_active),
        )
        .add_systems(
            OnEnter(AppState::InGame),
            sync_initial_sim.run_if(online_session_active),
        )
        .add_systems(
            Update,
            (
                poll_ggrs_events,
                sync_sim_snapshot,
                refresh_mouse_aim_anchor,
                sync_diagnostics_to_ui,
            )
                .run_if(in_state(AppState::InGame))
                .run_if(online_session_active),
        )
        .add_systems(GgrsSchedule, ggrs_step.run_if(online_session_active));
}

pub fn matchbox_room_url(args: &crate::args::Args) -> String {
    let room = args.room.as_ref().expect("room required for online play");
    let ticket = args.ticket.as_ref().expect("ticket required for online play");
    let base = args.signal.trim_end_matches('/');
    format!("{base}/{room}?ticket={ticket}&next=2")
}

fn seed_from_room(room: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    room.to_uppercase().hash(&mut hasher);
    hasher.finish() ^ 0xB1F05E
}

fn start_matchbox_socket(
    mut commands: Commands,
    config: Res<LaunchConfig>,
    mut ui: ResMut<UiChannel>,
) {
    let url = matchbox_room_url(&config.args);
    info!("connecting to matchbox at {url}");
    if let Some(code) = &config.args.room {
        ui.room_code = Some(code.clone());
        ui.status = format!("Connecting to room {code}…");
        ui.lobby_phase = "host_wait".into();
        ui.lobby_waiting = true;
        ui.lobby_peers = 1;
    }
    commands.insert_resource(MatchboxSocket::new_unreliable(url));
}

fn lobby_system(
    mut commands: Commands,
    config: Res<LaunchConfig>,
    mut socket: ResMut<MatchboxSocket>,
    mut next: ResMut<NextState<AppState>>,
    mut ui: ResMut<UiChannel>,
    mut net: Option<ResMut<NetSession>>,
) {
    let Ok(peer_changes) = socket.try_update_peers() else {
        ui.error = Some("Signaling connection lost".into());
        ui.status = "Signaling connection lost — refresh to retry".into();
        return;
    };

    for (peer, state) in peer_changes {
        if state == PeerState::Connected {
            info!("peer {peer} connected");
        }
    }

    let connected = socket.connected_peers().count();
    let peers = (connected + 1) as u8;
    let remaining = NUM_PLAYERS.saturating_sub(connected + 1);
    ui.lobby_peers = peers;
    ui.lobby_waiting = remaining > 0;
    ui.lobby_phase = if remaining > 0 {
        "host_wait".into()
    } else {
        "ready".into()
    };
    ui.status = if remaining > 0 {
        format!("Waiting for {remaining} more player(s)…")
    } else {
        "Starting match…".into()
    };

    if remaining > 0 {
        return;
    }

    let players = socket.players();
    if players.len() < NUM_PLAYERS {
        return;
    }

    let input_delay = config.args.lag_frames as usize + DEFAULT_INPUT_DELAY;
    let mut builder = SessionBuilder::<BifrostConfig>::new()
        .with_num_players(NUM_PLAYERS)
        .with_max_prediction_window(DEFAULT_MAX_PREDICTION)
        .with_input_delay(input_delay)
        .with_desync_detection_mode(DesyncDetection::On { interval: 30 });

    for (i, player) in players.into_iter().enumerate() {
        builder = builder
            .add_player(player, i)
            .expect("failed to add GGRS player");
    }

    let channel = socket.take_channel(0).expect("GGRS channel missing");
    let sess = builder
        .start_p2p_session(channel)
        .expect("failed to start GGRS session");

    let room = config.args.room.as_deref().unwrap_or("");
    commands.insert_resource(RollbackWorld(new_match(seed_from_room(room))));
    commands.insert_resource(Session::P2P(sess));
    if let Some(mut net) = net {
        net.peer_disconnected = false;
    }
    ui.status = "In match — WASD / arrows / mouse / gamepad".into();
    ui.error = None;
    ui.lobby_phase = "match".into();
    ui.lobby_waiting = false;
    ui.lobby_peers = 2;
    next.set(AppState::InGame);
}

fn read_local_inputs(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform, &Transform)>,
    world: Option<Res<RollbackWorld>>,
    local_players: Option<Res<LocalPlayers>>,
    anchor: Option<Res<crate::input_focus::MouseAimAnchor>>,
    mut focus: ResMut<InputFocus>,
) {
    if local_players.is_none() {
        return;
    }
    let local_players = local_players.unwrap();
    let local_seat = local_players.0.first().copied().unwrap_or(0) as usize;
    let paddle = world
        .as_ref()
        .map(|w| w.0.paddles[local_seat.min(1)])
        .unwrap_or(bifrost_sim::PaddleState {
            x: 0,
            y: 0,
            vx: 0,
            vy: 0,
            jump_z: 0,
            jump_vz: 0,
            spin_charge: 0,
            spin_dir_x: 0,
            spin_dir_y: 0,
            spin_remain: 0,
            spin_theta: 0,
            jump_was_held: false,
            ground_pounding: false,
            angle: 0,
            angle_was_held: false,
            angle_strike: 0,
            jump_peak_z: 0,
        });
    // Prefer visual anchor so rollback / input-delay does not thrash mouse chase.
    let (aim_x, aim_y) = anchor
        .filter(|a| a.valid)
        .map(|a| (a.x, a.y))
        .unwrap_or((paddle.x, paddle.y));
    let mask = local_input_mask(
        &keys,
        &mouse,
        &gamepads,
        &windows,
        &camera_q,
        aim_x,
        aim_y,
        &mut focus,
    );
    let mut local_inputs = HashMap::new();
    for handle in &local_players.0 {
        local_inputs.insert(*handle, BifrostInput { mask });
    }
    commands.insert_resource(LocalInputs::<BifrostConfig>(local_inputs));
}

fn ggrs_step(
    mut world: ResMut<RollbackWorld>,
    inputs: Res<PlayerInputs<BifrostConfig>>,
    mut sim: ResMut<SimSnapshot>,
    mut last: Local<i32>,
) {
    if world.0.phase == MatchPhase::MatchOver {
        return;
    }
    let p0 = inputs[0].0.mask;
    let p1 = inputs[1].0.mask;
    let out = step(&mut world.0, FrameInput { p0, p1 });
    // Only forward juice when the sim frame advances (skip rollback re-sim noise).
    let frame = world.0.frame as i32;
    if frame > *last {
        sim.events.extend(out.events);
        *last = frame;
    }
}

fn sync_initial_sim(
    world: Res<RollbackWorld>,
    mut sim: ResMut<SimSnapshot>,
    mut interp: ResMut<InterpState>,
) {
    sim.world = world.0.clone();
    sim.events.clear();
    interp.reset_from(&sim.world);
}

fn sync_sim_snapshot(
    world: Res<RollbackWorld>,
    mut sim: ResMut<SimSnapshot>,
    mut interp: ResMut<InterpState>,
) {
    sim.world = world.0.clone();
    interp.advance(&sim.world);
}

fn refresh_mouse_aim_anchor(
    interp: Res<InterpState>,
    sim: Res<SimSnapshot>,
    local_players: Option<Res<LocalPlayers>>,
    mut anchor: ResMut<crate::input_focus::MouseAimAnchor>,
) {
    let seat = local_players
        .and_then(|lp| lp.0.first().copied())
        .unwrap_or(0) as usize;
    let seat = seat.min(1);
    let paddle = if interp.initialized {
        &interp.curr.paddles[seat]
    } else {
        &sim.world.paddles[seat]
    };
    anchor.x = paddle.x;
    anchor.y = paddle.y;
    anchor.valid = true;
}

fn poll_ggrs_events(
    mut session: ResMut<Session<BifrostConfig>>,
    mut net: ResMut<NetSession>,
    mut ui: ResMut<UiChannel>,
    world: Res<RollbackWorld>,
) {
    let Session::P2P(sess) = session.as_mut() else {
        return;
    };

    for event in sess.events() {
        match event {
            GgrsEvent::DesyncDetected { .. } => error!("GGRS desync: {event:?}"),
            GgrsEvent::Disconnected { .. } => {
                warn!("GGRS peer disconnected: {event:?}");
                mark_peer_disconnected(&mut net, &mut ui, "Opponent disconnected.");
            }
            GgrsEvent::NetworkInterrupted { disconnect_timeout, .. } => {
                // Warning only — GGRS will emit Disconnected after disconnect_timeout.
                // Treating this as a hard drop aborts lobby sync / Ready Up on flaky UDP.
                warn!(
                    "GGRS network interrupted ({disconnect_timeout}ms to drop): {event:?}"
                );
                if !net.peer_disconnected {
                    ui.status = format!(
                        "Connection unstable — reconnecting ({disconnect_timeout}ms)…"
                    );
                }
            }
            GgrsEvent::NetworkResumed { .. } => {
                info!("GGRS network resumed");
                if !net.peer_disconnected && ui.lobby_phase == "match" {
                    ui.status = "In match".into();
                }
            }
            _ => {}
        }
    }

    let frame = sess.current_frame();
    net.diagnostics.update_from_state(&world.0, frame as u32);
    if let Ok(stats) = sess.network_stats(0) {
        net.diagnostics.rtt_ms = stats.ping as f32;
    }
}

fn mark_peer_disconnected(net: &mut NetSession, ui: &mut UiChannel, message: &str) {
    if net.peer_disconnected {
        return;
    }
    net.peer_disconnected = true;
    ui.lobby_phase = "disconnected".into();
    ui.status = message.into();
    ui.lobby_waiting = false;
    ui.lobby_peers = 1;
    notify_shell_opponent_left(message);
}

fn notify_shell_opponent_left(message: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(hook) = js_sys::Reflect::get(&window, &"bifrostOpponentLeft".into()) {
            if let Ok(func) = hook.dyn_into::<js_sys::Function>() {
                let _ = func.call1(&window, &wasm_bindgen::JsValue::from_str(message));
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = message;
    }
}

fn sync_diagnostics_to_ui(net: Res<NetSession>, mut ui: ResMut<UiChannel>) {
    if net.peer_disconnected {
        return;
    }
    if net.diagnostics.confirmed_frame > 0 {
        ui.status = format!(
            "frame {} · rollback max {} · rtt {:.0}ms",
            net.diagnostics.confirmed_frame,
            net.diagnostics.max_rollback_depth,
            net.diagnostics.rtt_ms
        );
    }
}
