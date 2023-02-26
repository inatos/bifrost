
use bevy::{
    prelude::*,
    tasks::IoTaskPool
};
use bevy_ggrs::{*, ggrs::PlayerType};
use matchbox_socket::WebRtcSocket;

const INPUT_DELAY: usize = 2;
const MAX_PLAYER_CONNECTIONS: usize = 2;
const HOST: &str = "ws://127.0.0.1";
const PORT: &str = "3536";
const ROOM: &str = "bifrost";

#[derive(Resource)]
pub struct P2PSession {
    socket: Option<WebRtcSocket>,
}

#[derive(Resource)]
struct LocalPlayerHandle(usize);

#[derive(Resource, Default, Reflect, Hash)]
#[reflect(Hash)]
pub struct FrameCount {
    pub frame: u32,
}

pub struct GgrsConfig;

impl ggrs::Config for GgrsConfig {
    // 4-directions + fire fits easily in a single byte
    type Input = u8;
    type State = u8;
    // Matchbox' WebRtcSocket addresses are strings
    type Address = String;
}


/// Increases the frame count by 1 every update step. If loading and saving resources works correctly,
/// you should see this resource rolling back, counting back up and finally increasing by 1 every update step.
pub fn increase_frame_system(mut frame_count: ResMut<FrameCount>) {
    frame_count.frame += 1;
}

/// Opens a WebRTC socket where players can connect to.
pub fn start_matchbox_socket(mut commands: Commands) {
    let room_url = format!("{}:{}/{}?next={}", HOST, PORT, ROOM, MAX_PLAYER_CONNECTIONS);
    info!("Connecting to matchbox server: {:?}", room_url);
    let (socket, message_loop) = WebRtcSocket::new(room_url);

    // The message loop needs to be awaited, or nothing will happen.
    // We do this here using bevy's task system.
    IoTaskPool::get().spawn(message_loop).detach();

    commands.insert_resource(P2PSession {
        socket: Some(socket),
    });
}

/// Creates a P2P Session between players.
pub fn wait_for_players(mut commands: Commands, mut session: ResMut<P2PSession>) {
    let Some(socket) = &mut session.socket else {
        // If there is no socket we've already started the game
        return;
    };

    // Check for new connections
    socket.accept_new_connections();
    let players = socket.players();
    if players.len() < MAX_PLAYER_CONNECTIONS {
        return; // Wait for more players
    }

    info!("All players have connected!");
    
    // Create a GGRS P2P session
    let mut session_builder = ggrs::SessionBuilder::<GgrsConfig>::new()
        .with_num_players(MAX_PLAYER_CONNECTIONS)
        .with_input_delay(INPUT_DELAY);

    for (i, player) in players.into_iter().enumerate() {
        if player == PlayerType::Local {
            commands.insert_resource(LocalPlayerHandle(i));
        }

        session_builder = session_builder
            .add_player(player, i)
            .expect("Player failed to join.");
    }

    // Move the socket out of the resource (required because GGRS takes ownership of it)
    let socket = session.socket.take().unwrap();

    // Start session
    let ggrs_session = session_builder
        .start_p2p_session(socket)
        .expect("Session failed to start.");

    commands.insert_resource(bevy_ggrs::Session::P2PSession(ggrs_session));
}