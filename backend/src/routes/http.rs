use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use tokio::sync::oneshot;

use crate::AppState;
use crate::error::AppError;
use crate::room::actor::{RoomCommand, RoomHandle};
use crate::room::code::generate_code;

// Health check
pub async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    code: String,
}

// Create a room with a new code
pub async fn create_room(State(state): State<AppState>) -> (StatusCode, Json<CreateRoomResponse>) {
    // Check and insert under one lock so two requests can't take the same code
    let code = {
        let mut rooms = state.rooms.lock().unwrap();

        // Try new codes until one is free
        let code = loop {
            let code = generate_code();
            if !rooms.contains_key(&code) {
                break code;
            }
        };

        let room = RoomHandle::spawn(code.clone(), state.rooms.clone());
        rooms.insert(code.clone(), room);
        code
    };
    tracing::info!("Created room {code}");

    (StatusCode::CREATED, Json(CreateRoomResponse { code }))
}

#[derive(Serialize)]
pub struct RoomInfo {
    code: String,
    players: usize,
}

// Look up a room by code
pub async fn get_room(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfo>, AppError> {
    // Accept lowercase codes too
    let code = code.to_uppercase();
    let room = state.rooms.lock().unwrap().get(&code).cloned();
    let Some(room) = room else {
        return Err(AppError::RoomNotFound);
    };

    let (reply, players) = oneshot::channel();
    room.send(RoomCommand::PlayerCount { reply }).await;
    // No answer means the room just closed
    let players = players.await.map_err(|_| AppError::RoomNotFound)?;
    Ok(Json(RoomInfo { code, players }))
}
