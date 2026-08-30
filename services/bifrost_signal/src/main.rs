mod config;
mod matchbox_server;
mod rooms;
mod routes;
mod turn;

use axum::{
    Router,
    routing::{get, post},
};
use clap::Parser;
use config::AppConfig;
use rooms::RoomStore;
use std::{net::SocketAddr, sync::Arc};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "bifrost_signal")]
struct Cli {
    #[arg(long, env = "BIND_ADDR", default_value = "0.0.0.0")]
    bind: String,
    #[arg(long, env = "PORT", default_value = "8787")]
    port: u16,
    #[arg(long, env = "MATCHBOX_PORT", default_value = "3536")]
    matchbox_port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("bifrost_signal=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    let config = AppConfig::from_env();
    tracing::info!(
        public_origin = %config.public_origin,
        public_ws_origin = %config.public_ws_origin,
        "bifrost_signal config"
    );
    let store = Arc::new(RoomStore::new(config.room_ttl().as_secs()));

    let matchbox_store = store.clone();
    let matchbox_port = cli.matchbox_port;
    tokio::spawn(async move {
        if let Err(err) = matchbox_server::run(matchbox_store, matchbox_port).await {
            tracing::error!("matchbox signaling failed: {err}");
        }
    });

    let app = Router::new()
        .route("/healthz", get(routes::health))
        .route("/readyz", get(routes::ready))
        .route("/metrics", get(routes::metrics))
        .route("/api/rooms", post(routes::create_room))
        .route("/api/rooms/join", post(routes::join_room))
        .route("/api/rooms/leave", post(routes::leave_room))
        .route("/api/rooms/{code}", get(routes::room_info))
        .route("/api/turn", get(routes::turn_credentials))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(store);

    let addr: SocketAddr = format!("{}:{}", cli.bind, cli.port)
        .parse()
        .expect("bind addr");
    tracing::info!("bifrost_signal API listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
