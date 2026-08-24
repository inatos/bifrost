use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(name = "bifrost", about = "Bifrost rollback Breakout/Pong")]
pub struct Args {
    /// Matchbox / signaling base (`ws://127.0.0.1:3536` dev, `wss://host/signal` prod)
    #[arg(long, default_value = "ws://127.0.0.1:3536")]
    pub signal: String,

    /// Room code for private match (from /api/rooms)
    #[arg(long)]
    pub room: Option<String>,

    /// Join ticket for guest
    #[arg(long)]
    pub ticket: Option<String>,

    /// Play locally against bot (no networking)
    #[arg(long, default_value_t = false)]
    pub bot: bool,

    /// Replay code to watch deterministically
    #[arg(long)]
    pub replay: Option<String>,

    /// Simulated input delay frames (Lag Forge)
    #[arg(long, default_value_t = 0)]
    pub lag_frames: u32,
}

impl Args {
    pub fn from_query_or_cli() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(args) = query_args() {
                return args;
            }
        }
        Self::parse()
    }
}

#[cfg(target_arch = "wasm32")]
fn query_args() -> Option<Args> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window()?;
    let location = window.location();
    let search = location.search().ok()?;
    if search.is_empty() {
        return None;
    }
    let mut args: Args = serde_qs::from_str(&search).ok()?;
    if args.room.is_some() && !args.bot {
        args.signal = wasm_signal_base();
    }
    Some(args)
}

#[cfg(target_arch = "wasm32")]
fn wasm_signal_base() -> String {
    let window = web_sys::window().expect("window");
    let loc = window.location();
    let scheme = if loc.protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = loc.host().unwrap_or_default();
    format!("{scheme}//{host}/signal")
}
