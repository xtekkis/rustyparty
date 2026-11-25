use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod client_ip;
mod error;
mod games;
mod limits;
mod origins;
mod protocol;
mod room;
mod routes;

use limits::Limits;
use origins::AllowedOrigins;
use room::actor::RoomMap;

// Shared by all requests: how to reach each room by code, which websites may call,
// how often each address may do things, and whether a proxy says who's calling
#[derive(Clone, Default)]
struct AppState {
    rooms: RoomMap,
    origins: AllowedOrigins,
    limits: Arc<Limits>,
    behind_proxy: bool,
    // Logs what the proxy sends on each /health, to check once where player addresses come from
    log_proxy_headers: bool,
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
    // Render's proxy says who's calling in X-Forwarded-For. Without a proxy, anyone could
    // write that header, so it's only read when this is set
    let behind_proxy = match std::env::var("BEHIND_PROXY").as_deref() {
        Ok("true") => true,
        Ok("false") | Err(_) => false,
        Ok(other) => panic!("BEHIND_PROXY must be true or false, not {other:?}"),
    };
    if behind_proxy {
        tracing::info!("Behind a proxy: player addresses come from X-Forwarded-For");
    }

    // Off unless asked for: the lines it writes hold visitors' addresses
    let log_proxy_headers = std::env::var("LOG_PROXY_HEADERS").as_deref() == Ok("true");
    if log_proxy_headers {
        tracing::warn!(
            "LOG_PROXY_HEADERS is on: each /health logs the proxy headers. Turn it off after checking"
        );
    }

    let state = AppState {
        origins,
        behind_proxy,
        log_proxy_headers,
        ..AppState::default()
    };

    // The built frontend, set in the Docker image. In development Vite serves it instead
    let frontend = std::env::var_os("FRONTEND_DIR").map(PathBuf::from);
    match &frontend {
        Some(dir) if !dir.join("index.html").is_file() => {
            panic!("FRONTEND_DIR has no index.html: {}", dir.display())
        }
        Some(dir) => {
            tracing::info!("Serving the frontend from {}", dir.display());
            if matches!(state.origins, AllowedOrigins::Only(_)) {
                tracing::info!(
                    "The pages come from this server, so ALLOWED_ORIGINS must include its own address"
                );
            }
        }
        None => tracing::info!(
            "FRONTEND_DIR is not set, so only the game server runs (Vite serves the frontend in development)"
        ),
    }

    let app = routes::router(state, frontend.as_deref()).layer(TraceLayer::new_for_http());

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

    // Each connection's own address, for the limits when there's no proxy
    let app = app.into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await.unwrap();
}
