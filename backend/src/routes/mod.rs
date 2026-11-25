use std::path::{Path, PathBuf};

use axum::{
    Router,
    extract::Request,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::AppState;

pub mod http;
pub mod ws;

// All routes of the app, open to the websites in state.origins.
// With a frontend folder its files are served too, and other page addresses get index.html,
// so links like /join/KXTP open the app. The routes above always come first
pub fn router(state: AppState, frontend: Option<&Path>) -> Router {
    let cors = state.origins.cors();
    let ws = get(ws::ws_handler).route_layer(middleware::from_fn_with_state(
        state.clone(),
        ws::check_websocket,
    ));
    let mut app = Router::new()
        .route("/health", get(http::health))
        .route("/rooms", post(http::create_room))
        .route("/rooms/{code}", get(http::get_room))
        .route("/ws", ws);
    if let Some(dir) = frontend {
        let index = dir.join("index.html");
        let pages = Router::new()
            .fallback(move |request: Request| page_or_not_found(request, index.clone()));
        app = app.fallback_service(ServeDir::new(dir).fallback(pages));
    }
    app.layer(cors).with_state(state)
}

// An address that isn't a file: the app if it looks like a page, otherwise 404. A missing
// script must not get HTML back, and a wrong game server path must not look like it worked
async fn page_or_not_found(request: Request, index: PathBuf) -> Response {
    if !looks_like_page(request.uri().path()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match ServeFile::new(index).oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(never) => match never {},
    }
}

// Pages are addresses like / or /join/KXTP: no file extension, and not the game server's
fn looks_like_page(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or("");
    let server = ["/assets/", "/rooms", "/ws"]
        .iter()
        .any(|start| path.starts_with(start));
    !server && !last.contains('.')
}

#[cfg(test)]
mod tests;
