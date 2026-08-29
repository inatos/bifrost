//! Shared DTOs between the Bifrost client and signaling service.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_PLAYERS: usize = 2;
pub const ROOM_CODE_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerRole {
    Host,
    Guest,
}

fn empty_name() -> String {
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub protocol_version: u32,
    #[serde(default = "empty_name")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomResponse {
    pub room_code: String,
    pub host_ticket: String,
    pub signal_url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomRequest {
    pub protocol_version: u32,
    pub room_code: String,
    #[serde(default = "empty_name")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomResponse {
    pub guest_ticket: String,
    pub signal_url: String,
    pub expires_at: String,
    #[serde(default = "empty_name")]
    pub host_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfoResponse {
    pub room_code: String,
    pub players: u8,
    pub max_players: u8,
    pub expires_at: String,
    #[serde(default = "empty_name")]
    pub host_name: String,
    #[serde(default = "empty_name")]
    pub guest_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRoomRequest {
    pub protocol_version: u32,
    pub room_code: String,
    pub ticket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRoomResponse {
    /// True when the host left and the room was deleted.
    pub lobby_closed: bool,
    pub role: PlayerRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnCredentialsResponse {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
    pub ttl_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub protocol_version: u32,
    pub active_rooms: usize,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("invalid room code")]
    InvalidRoomCode,
    #[error("{0}")]
    Message(String),
}

impl ProtocolError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}
