use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

pub mod http;
pub mod ws;

// All routes of the app
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(http::health))
        .route("/rooms", post(http::create_room))
        .route("/rooms/{code}", get(http::get_room))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
}
