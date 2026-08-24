use crate::input::FrameInput;
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub seed: u64,
    pub inputs: Vec<FrameInput>,
}

pub fn encode_replay(replay: &Replay) -> String {
    let json = serde_json::to_vec(replay).expect("replay encode");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

pub fn decode_replay(code: &str) -> Result<Replay, String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(code.trim())
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
