//! GGRS + Matchbox P2P online session.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_ggrs::ggrs::{DesyncDetection, GgrsEvent, SessionBuilder};
use bevy_ggrs::prelude::*;
use bevy_ggrs::{LocalInputs, LocalPlayers};
use bevy_matchbox::matchbox_socket::PeerId;
use bevy_matchbox::prelude::*;
use bifrost_net::{BifrostInput, DEFAULT_INPUT_DELAY, DEFAULT_MAX_PREDICTION, FPS};
use bifrost_sim::{
    new_match, step, FrameInput, MatchPhase, WorldState, INPUT_LEFT, INPUT_RIGHT,
};

use crate::net_mode::NetSession;
use crate::render::draw_world;
use crate::state::{AppState, LaunchConfig, SimSnapshot, UiChannel};

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
        .add_systems(ReadInputs, read_local_inputs)
        .add_systems(OnEnter(AppState::Lobby), start_matchbox_socket)
        .add_systems(Update, lobby_system.run_if(in_state(AppState::Lobby)))
        .add_systems(OnEnter(AppState::InGame), sync_initial_sim)
        .add_systems(
            Update,
            (
                poll_ggrs_events,
                sync_sim_snapshot,
                draw_world,
                sync_diagnostics_to_ui,
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(GgrsSchedule, ggrs_step);
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
    }
    commands.insert_resource(MatchboxSocket::new_unreliable(url));
}

fn lobby_system(
    mut commands: Commands,
    config: Res<LaunchConfig>,
    mut socket: ResMut<MatchboxSocket>,
    mut next: ResMut<NextState<AppState>>,
    mut ui: ResMut<UiChannel>,
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
    let remaining = NUM_PLAYERS.saturating_sub(connected + 1);
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
    ui.status = "In match — A/D or ←/→ to move".into();
    ui.error = None;
    next.set(AppState::InGame);
}

fn read_local_inputs(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    local_players: Res<LocalPlayers>,
) {
    let mut local_inputs = HashMap::new();
    for handle in &local_players.0 {
        let mut mask = 0u8;
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            mask |= INPUT_LEFT;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            mask |= INPUT_RIGHT;
        }
        local_inputs.insert(*handle, BifrostInput { mask });
    }
    commands.insert_resource(LocalInputs::<BifrostConfig>(local_inputs));
}

fn ggrs_step(mut world: ResMut<RollbackWorld>, inputs: Res<PlayerInputs<BifrostConfig>>) {
    if world.0.phase == MatchPhase::MatchOver {
        return;
    }
    let p0 = inputs[0].0.mask;
    let p1 = inputs[1].0.mask;
    step(&mut world.0, FrameInput { p0, p1 });
}

fn sync_initial_sim(world: Res<RollbackWorld>, mut sim: ResMut<SimSnapshot>) {
    sim.world = world.0.clone();
    sim.events.clear();
}

fn sync_sim_snapshot(world: Res<RollbackWorld>, mut sim: ResMut<SimSnapshot>) {
    sim.world = world.0.clone();
}

fn poll_ggrs_events(
    mut session: ResMut<Session<BifrostConfig>>,
    mut net: ResMut<NetSession>,
    world: Res<RollbackWorld>,
) {
    let Session::P2P(sess) = session.as_mut() else {
        return;
    };

    for event in sess.events() {
        match event {
            GgrsEvent::DesyncDetected { .. } => error!("GGRS desync: {event:?}"),
            GgrsEvent::Disconnected { .. } | GgrsEvent::NetworkInterrupted { .. } => {
                warn!("GGRS network: {event:?}");
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

fn sync_diagnostics_to_ui(net: Res<NetSession>, mut ui: ResMut<UiChannel>) {
    if net.diagnostics.confirmed_frame > 0 {
        ui.status = format!(
            "frame {} · rollback max {} · rtt {:.0}ms",
            net.diagnostics.confirmed_frame,
            net.diagnostics.max_rollback_depth,
            net.diagnostics.rtt_ms
        );
    }
}
