use crate::config::AppConfig;
use crate::rooms::{assert_protocol, RoomStore};
use crate::turn::turn_config;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bifrost_protocol::{
    CreateRoomRequest, CreateRoomResponse, HealthResponse, JoinRoomRequest, JoinRoomResponse,
    RoomInfoResponse, TurnCredentialsResponse, PROTOCOL_VERSION,
};
use std::sync::Arc;

pub async fn health(State(store): State<Arc<RoomStore>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        protocol_version: PROTOCOL_VERSION,
        active_rooms: store.active_count(),
    })
}

pub async fn ready(State(_store): State<Arc<RoomStore>>) -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn metrics(State(store): State<Arc<RoomStore>>) -> impl IntoResponse {
    format!(
        "# TYPE bifrost_active_rooms gauge\nbifrost_active_rooms {}\n",
        store.active_count()
    )
}

pub async fn create_room(
    State(store): State<Arc<RoomStore>>,
    Json(body): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, (StatusCode, String)> {
    assert_protocol(body.protocol_version).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let config = AppConfig::from_env();
    let room = store.create();
    Ok(Json(CreateRoomResponse {
        room_code: room.code.clone(),
        host_ticket: room.host_ticket.clone(),
        signal_url: config.room_signal_url(&room.code, &room.host_ticket),
        expires_at: room.expires_at.to_rfc3339(),
    }))
}

pub async fn join_room(
    State(store): State<Arc<RoomStore>>,
    Json(body): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, (StatusCode, String)> {
    assert_protocol(body.protocol_version).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let config = AppConfig::from_env();
    let code = body.room_code.to_uppercase();
    let (room, _role, ticket) = store.join(&code).map_err(|e| (StatusCode::CONFLICT, e))?;
    Ok(Json(JoinRoomResponse {
        guest_ticket: ticket.clone(),
        signal_url: config.room_signal_url(&room.code, &ticket),
        expires_at: room.expires_at.to_rfc3339(),
    }))
}

pub async fn room_info(
    State(store): State<Arc<RoomStore>>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfoResponse>, StatusCode> {
    let room = store.get(&code.to_uppercase()).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(RoomInfoResponse {
        room_code: room.code,
        players: store.players_in_room(&code.to_uppercase()),
        max_players: 2,
        expires_at: room.expires_at.to_rfc3339(),
    }))
}

pub async fn turn_credentials() -> Result<Json<TurnCredentialsResponse>, StatusCode> {
    let config = AppConfig::from_env();
    let (urls, username, credential) = turn_config(&config).ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(TurnCredentialsResponse {
        urls,
        username,
        credential,
        ttl_seconds: config.turn_ttl_secs,
    }))
}
