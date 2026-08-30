//! Rollback diagnostics and session configuration helpers.

use bifrost_sim::{FrameInput, WorldState, checksum};
use serde::{Deserialize, Serialize};

pub const DEFAULT_INPUT_DELAY: usize = 2;
pub const DEFAULT_MAX_PREDICTION: usize = 12;
pub const FPS: usize = 60;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BifrostInput {
    pub mask: u8,
}

impl From<BifrostInput> for u8 {
    fn from(v: BifrostInput) -> Self {
        v.mask
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RollbackDiagnostics {
    pub predicted_frame: u32,
    pub confirmed_frame: u32,
    pub rollback_count: u32,
    pub max_rollback_depth: u32,
    pub last_checksum: u64,
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub send_queue: u32,
}

impl RollbackDiagnostics {
    pub fn record_rollback(&mut self, depth: u32) {
        self.rollback_count += 1;
        self.max_rollback_depth = self.max_rollback_depth.max(depth);
    }

    pub fn update_from_state(&mut self, state: &WorldState, confirmed: u32) {
        self.last_checksum = checksum(state);
        self.confirmed_frame = confirmed;
        self.predicted_frame = state.frame;
    }
}

pub fn pack_inputs(p0: u8, p1: u8) -> FrameInput {
    FrameInput { p0, p1 }
}

pub fn unpack_for_player(input: FrameInput, players: usize) -> Vec<BifrostInput> {
    (0..players)
        .map(|i| BifrostInput {
            mask: input.for_player(i),
        })
        .collect()
}
