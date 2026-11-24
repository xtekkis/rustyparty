use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use tokio::sync::oneshot;

use crate::AppState;
use crate::client_ip::ClientIp;
use crate::error::AppError;
use crate::room::actor::{RoomCommand, RoomHandle};
use crate::room::code::{MAX_CODE_ATTEMPTS, free_code};

// The most rooms at once, so many addresses together can't fill the server's memory.
// Empty rooms close after a few minutes
const MAX_ROOMS: usize = 1000;

// Health check
pub async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    code: String,
}

// Create a room with a new code
pub async fn create_room(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
) -> Result<(StatusCode, Json<CreateRoomResponse>), AppError> {
    // Check and insert under one lock so two requests can't take the same code
    let code = {
        let mut rooms = state.rooms.lock().unwrap();
        if rooms.len() >= MAX_ROOMS {
            tracing::warn!("The server has {MAX_ROOMS} rooms, no more for now");
            return Err(AppError::NoFreeRoomCode);
        }

        // Nearly every code in use means someone is creating rooms nonstop
        let Some(code) = free_code(|code| rooms.contains_key(code)) else {
            tracing::warn!("No free room code after {MAX_CODE_ATTEMPTS} tries");
            return Err(AppError::NoFreeRoomCode);
        };

        // Last, so a refusal for another reason doesn't use up a try
        if !state.limits.create_rooms.allow(ip) {
            tracing::warn!("Too many rooms created from one address");
            let retry_after = state.limits.create_rooms.rate().retry_after();
            return Err(AppError::TooManyRequests { retry_after });
        }

        let room = RoomHandle::spawn(code.clone(), state.rooms.clone());
        rooms.insert(code.clone(), room);
        code
    };
    tracing::info!("Created room {code}");

    Ok((StatusCode::CREATED, Json(CreateRoomResponse { code })))
}

#[derive(Serialize)]
pub struct RoomInfo {
    code: String,
    players: usize,
}

// Look up a room by code
pub async fn get_room(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Path(code): Path<String>,
) -> Result<Json<RoomInfo>, AppError> {
    // Accept lowercase codes too
    let code = code.to_uppercase();
    let room = state.rooms.lock().unwrap().get(&code).cloned();
    let Some(room) = room else {
        // Slows down anyone guessing codes. A real room is never refused
        if !state.limits.room_lookups.allow(ip) {
            let retry_after = state.limits.room_lookups.rate().retry_after();
            return Err(AppError::TooManyRequests { retry_after });
        }
        return Err(AppError::RoomNotFound);
    };

    let (reply, players) = oneshot::channel();
    room.send(RoomCommand::PlayerCount { reply }).await;
    // No answer means the room just closed
    let players = players.await.map_err(|_| AppError::RoomNotFound)?;
    Ok(Json(RoomInfo { code, players }))
}
