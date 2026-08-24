use bifrost_protocol::{PlayerRole, PROTOCOL_VERSION, MAX_PLAYERS};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rand::Rng;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RoomRecord {
    pub code: String,
    pub host_ticket: String,
    pub guest_ticket: Option<String>,
    pub created: Instant,
    pub expires_at: DateTime<Utc>,
}

pub struct RoomStore {
    rooms: DashMap<String, RoomRecord>,
    ttl: Duration,
}

impl RoomStore {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            rooms: DashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn cleanup(&self) {
        let ttl = self.ttl;
        self.rooms.retain(|_, room| room.created.elapsed() < ttl);
    }

    pub fn create(&self) -> RoomRecord {
        self.cleanup();
        let code = random_code(8);
        let host_ticket = random_token(24);
        let expires_at = Utc::now() + chrono::Duration::seconds(self.ttl.as_secs() as i64);
        let record = RoomRecord {
            code: code.clone(),
            host_ticket,
            guest_ticket: None,
            created: Instant::now(),
            expires_at,
        };
        self.rooms.insert(code, record.clone());
        record
    }

    pub fn get(&self, code: &str) -> Option<RoomRecord> {
        self.cleanup();
        self.rooms.get(code).map(|r| r.clone())
    }

    pub fn join(&self, code: &str) -> Result<(RoomRecord, PlayerRole, String), String> {
        self.cleanup();
        let mut entry = self
            .rooms
            .get_mut(code)
            .ok_or_else(|| "room not found".to_string())?;
        if entry.created.elapsed() > self.ttl {
            return Err("room expired".into());
        }
        if entry.guest_ticket.is_some() {
            return Err("room full".into());
        }
        let ticket = random_token(24);
        entry.guest_ticket = Some(ticket.clone());
        Ok((entry.clone(), PlayerRole::Guest, ticket))
    }

    pub fn validate_ticket(&self, code: &str, ticket: &str) -> Option<PlayerRole> {
        let room = self.get(code)?;
        if room.host_ticket == ticket {
            return Some(PlayerRole::Host);
        }
        if room.guest_ticket.as_deref() == Some(ticket) {
            return Some(PlayerRole::Guest);
        }
        None
    }

    pub fn active_count(&self) -> usize {
        self.cleanup();
        self.rooms.len()
    }

    pub fn players_in_room(&self, code: &str) -> u8 {
        self.get(code)
            .map(|r| if r.guest_ticket.is_some() { 2 } else { 1 })
            .unwrap_or(0)
    }
}

fn random_code(len: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut out = String::with_capacity(len);
    let mut rng = rand::rng();
    for _ in 0..len {
        let idx = rng.random_range(0..ALPHABET.len());
        out.push(ALPHABET[idx] as char);
    }
    out
}

fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

use base64::Engine;

pub fn assert_protocol(version: u32) -> Result<(), String> {
    if version != PROTOCOL_VERSION {
        return Err(format!("unsupported protocol version {version}"));
    }
    Ok(())
}

pub const MAX_PLAYERS_CONST: usize = MAX_PLAYERS;

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_protocol::{PlayerRole, PROTOCOL_VERSION};

    #[test]
    fn room_create_and_join() {
        let store = RoomStore::new(900);
        let room = store.create();
        assert_eq!(room.code.len(), 8);
        let (joined, role, ticket) = store.join(&room.code).expect("guest");
        assert_eq!(role, PlayerRole::Guest);
        assert!(store.validate_ticket(&joined.code, &ticket).is_some());
        assert!(store.join(&room.code).is_err());
    }

    #[test]
    fn protocol_version_gate() {
        assert!(assert_protocol(PROTOCOL_VERSION).is_ok());
        assert!(assert_protocol(0).is_err());
    }
}
