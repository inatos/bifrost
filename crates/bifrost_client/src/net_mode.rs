//! Rollback diagnostics shared by online play.

use bevy::prelude::*;
use bifrost_net::{RollbackDiagnostics, DEFAULT_INPUT_DELAY};

#[derive(Resource)]
pub struct NetSession {
    pub diagnostics: RollbackDiagnostics,
    pub input_delay: usize,
    /// Set once when a GGRS peer disconnects mid-match.
    pub peer_disconnected: bool,
}

impl NetSession {
    pub fn new() -> Self {
        Self {
            diagnostics: RollbackDiagnostics::default(),
            input_delay: DEFAULT_INPUT_DELAY,
            peer_disconnected: false,
        }
    }
}

impl Default for NetSession {
    fn default() -> Self {
        Self::new()
    }
}

pub fn plugin(_app: &mut App) {
    // Online mode registers NetSession directly.
}
