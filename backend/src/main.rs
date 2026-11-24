use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod error;
mod games;
mod origins;
mod protocol;
mod room;
mod routes;

use origins::AllowedOrigins;
use room::actor::RoomMap;

// Shared by all requests: how to reach each room, by code, and which websites may call
#[derive(Clone, Default)]
struct AppState {
    rooms: RoomMap,
    origins: AllowedOrigins,
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

    // A broken card file stops the server here, not in the middle of a game
    let deck = games::bad_takes::deck::starter_deck();
    tracing::info!(
        "Loaded deck {}: {} prompts, {} answers",
        deck.name,
        deck.prompts.len(),
        deck.answers.len()
    );

    let origins = AllowedOrigins::from_env();
    match &origins {
        AllowedOrigins::Any => tracing::warn!(
            "ALLOWED_ORIGINS is not set, so any website may call this server. Set it when deployed"
        ),
        AllowedOrigins::Only(list) => tracing::info!("Only these websites may call: {list:?}"),
    }
    let state = AppState {
        origins,
        ..AppState::default()
    };

    let app = routes::router(state).layer(TraceLayer::new_for_http());

    // Port from PORT, default 3000
    let port: u16 = match std::env::var("PORT") {
        Ok(value) => value
            .parse()
            .expect("PORT must be a number between 0 and 65535"),
        Err(_) => 3000,
    };

    // Listen on all network interfaces so phones on the same Wi-Fi can connect
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
