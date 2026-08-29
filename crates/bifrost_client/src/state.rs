use bevy::prelude::*;
use bifrost_sim::WorldState;

use crate::args::Args;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Menu,
    Lobby,
    InGame,
    Replay,
}

#[derive(Resource, Clone)]
pub struct LaunchConfig {
    pub args: Args,
}

#[derive(Resource, Clone)]
pub struct SimSnapshot {
    pub world: WorldState,
    pub events: Vec<bifrost_sim::ConfirmedEvent>,
}

impl Default for SimSnapshot {
    fn default() -> Self {
        Self {
            world: bifrost_sim::new_match(0),
            events: Vec::new(),
        }
    }
}

#[derive(Resource, Default)]
pub struct UiChannel {
    pub status: String,
    pub room_code: Option<String>,
    pub invite_url: Option<String>,
    pub error: Option<String>,
    /// idle | host_wait | guest_wait | ready | match
    pub lobby_phase: String,
    pub lobby_waiting: bool,
    /// Connected peer count including self (1 while waiting, 2 when ready).
    pub lobby_peers: u8,
}
