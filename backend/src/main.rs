use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod db;
mod error;
mod games;
mod protocol;
mod room;
mod routes;

use room::actor::RoomMap;

// Shared by all requests: how to reach each room, by code
#[derive(Clone, Default)]
struct AppState {
    rooms: RoomMap,
}

#[tokio::main]
async fn main() {
    // Settings from a .env file, if there is one. Real environment variables win
    dotenvy::dotenv().ok();

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

    // Kept open while the server runs. Later steps load cards and save finished games with it
    let db = db::connect().await;
    if let Some(pool) = &db
        && db::migrate(pool).await
    {
        // The deck file is where cards are written. The database gets any new ones
        let game_ids: Vec<&str> = games::GAMES.iter().map(|game| game.id).collect();
        match db::seed::seed_deck(pool, &game_ids, "bad_takes", deck).await {
            Ok(synced) if synced.prompts.is_empty() && synced.answers.is_empty() => {
                tracing::info!("Deck {} in the database is up to date", deck.name)
            }
            Ok(synced) => tracing::info!(
                "Deck {} saved. Prompts: {:?}. Answers: {:?}",
                deck.name,
                synced.prompts,
                synced.answers
            ),
            Err(error) => tracing::error!("Saving deck {} failed: {error}", deck.name),
        }
    }

    let state = AppState::default();

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
