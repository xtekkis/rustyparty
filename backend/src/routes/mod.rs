use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::AppState;

pub mod http;
pub mod ws;

// All routes of the app, open to the websites in state.origins
pub fn router(state: AppState) -> Router {
    let cors = state.origins.cors();
    let ws = get(ws::ws_handler).route_layer(middleware::from_fn_with_state(
        state.clone(),
        ws::check_origin,
    ));
    Router::new()
        .route("/health", get(http::health))
        .route("/rooms", post(http::create_room))
        .route("/rooms/{code}", get(http::get_room))
        .route("/ws", ws)
        .layer(cors)
        .with_state(state)
}

#[cfg(test)]
mod tests;
