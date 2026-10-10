use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod room;

use room::{Room, generate_code};

// Shared by all requests: rooms by code
#[derive(Clone, Default)]
struct AppState {
    rooms: Arc<Mutex<HashMap<String, Room>>>,
}

#[tokio::main]
async fn main() {
    // Log level comes from RUST_LOG, with a default for local development
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("backend=debug,tower_http=debug")),
        )
        .init();

    let state = AppState::default();

    let app = Router::new()
        .route("/health", get(health))
        .route("/rooms", post(create_room))
        .route("/rooms/{code}", get(get_room))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Port from PORT, default 3000
    let port: u16 = match std::env::var("PORT") {
        Ok(value) => value.parse().expect("PORT must be a number between 0 and 65535"),
        Err(_) => 3000,
    };

    // Listen on all network interfaces so phones on the same Wi-Fi can connect
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// Health check
async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct CreateRoomResponse {
    code: String,
}

// Create a room with a new code
async fn create_room(State(state): State<AppState>) -> (StatusCode, Json<CreateRoomResponse>) {
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
struct RoomInfo {
    code: String,
    players: usize,
}

// Look up a room by code, null if it doesn't exist
async fn get_room(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Json<Option<RoomInfo>> {
    // Accept lowercase codes too
    let code = code.to_uppercase();
    let rooms = state.rooms.lock().unwrap();

    let info = match rooms.get(&code) {
        Some(room) => Some(RoomInfo {
            code: room.code.clone(),
            players: room.players.len(),
        }),
        None => None,
    };

    Json(info)
}
