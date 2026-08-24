use crate::config::AppConfig;
use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub fn mint_turn_credentials(
    secret: &str,
    username: &str,
    ttl_secs: u32,
) -> (String, String) {
    let expiry = chrono::Utc::now().timestamp() as u64 + ttl_secs as u64;
    let user = format!("{expiry}:{username}");
    let mut mac = HmacSha1::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(user.as_bytes());
    let sig = mac.finalize().into_bytes();
    (user, base64::engine::general_purpose::STANDARD.encode(sig))
}

pub fn turn_config(config: &AppConfig) -> Option<(Vec<String>, String, String)> {
    let secret = config.turn_secret.as_ref()?;
    if config.turn_urls.is_empty() {
        return None;
    }
    let (username, credential) = mint_turn_credentials(secret, "bifrost", config.turn_ttl_secs);
    Some((config.turn_urls.clone(), username, credential))
}

use base64::Engine as _;
