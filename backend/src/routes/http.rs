use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::AppState;
use crate::error::AppError;
use crate::room::{Room, code::generate_code};

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

        rooms.insert(code.clone(), Room::new(code.clone()));
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
    let rooms = state.rooms.lock().unwrap();

    match rooms.get(&code) {
        Some(room) => Ok(Json(RoomInfo {
            code: room.code.clone(),
            players: room.players.len(),
        })),
        None => Err(AppError::RoomNotFound),
    }
}
