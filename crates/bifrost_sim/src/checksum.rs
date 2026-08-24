use crate::WorldState;
use sha2::{Digest, Sha256};

pub fn checksum(state: &WorldState) -> u64 {
    let json = serde_json::to_vec(state).unwrap_or_default();
    let digest = Sha256::digest(json);
    u64::from_le_bytes(digest[0..8].try_into().unwrap())
}
