use crate::rooms::RoomStore;
use matchbox_signaling::SignalingServer;
use std::{net::Ipv4Addr, sync::Arc};

pub async fn run(store: Arc<RoomStore>, port: u16) -> Result<(), matchbox_signaling::Error> {
    let store_cb = store.clone();
    let server = SignalingServer::full_mesh_builder((Ipv4Addr::UNSPECIFIED, port))
        .on_connection_request(move |meta| {
            let room = meta
                .path
                .as_deref()
                .unwrap_or("")
                .trim_matches('/')
                .to_uppercase();
            if room.is_empty() {
                return Ok(false);
            }
            let ticket = meta
                .query_params
                .get("ticket")
                .map(String::as_str)
                .unwrap_or("");
            if ticket.is_empty() {
                return Ok(false);
            }
            Ok(store_cb.validate_ticket(&room, ticket).is_some())
        })
        .cors()
        .trace()
        .build();
    tracing::info!("matchbox signaling listening on 0.0.0.0:{port}");
    server.serve().await
}
