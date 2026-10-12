use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod error;
mod room;
mod routes;

use room::Room;

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
    let app = routes::router(state).layer(TraceLayer::new_for_http());

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
