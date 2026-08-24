use std::time::Duration;

#[derive(Clone)]
pub struct AppConfig {
    pub public_origin: String,
    /// WebSocket origin clients use for Matchbox (room path + `?ticket=` appended).
    pub public_ws_origin: String,
    pub room_ttl_secs: u64,
    pub turn_secret: Option<String>,
    pub turn_urls: Vec<String>,
    pub turn_ttl_secs: u32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            public_origin: std::env::var("PUBLIC_ORIGIN")
                .unwrap_or_else(|_| "http://127.0.0.1:8787".into()),
            public_ws_origin: std::env::var("PUBLIC_WS_ORIGIN")
                .unwrap_or_else(|_| "ws://127.0.0.1:3536".into()),
            room_ttl_secs: std::env::var("ROOM_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            turn_secret: std::env::var("TURN_SECRET").ok(),
            turn_urls: std::env::var("TURN_URLS")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            turn_ttl_secs: std::env::var("TURN_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(600),
        }
    }

    pub fn room_ttl(&self) -> Duration {
        Duration::from_secs(self.room_ttl_secs)
    }

    pub fn room_signal_url(&self, room_code: &str, ticket: &str) -> String {
        format!(
            "{}/{}?ticket={}",
            self.public_ws_origin.trim_end_matches('/'),
            room_code,
            ticket
        )
    }
}
