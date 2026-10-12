use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};

use crate::protocol::ClientMessage;

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

// Read messages from one player
async fn handle_socket(mut socket: WebSocket) {
    tracing::debug!("WebSocket connected");

    while let Some(Ok(message)) = socket.recv().await {
        // Only text messages carry JSON; pings and closing are handled automatically
        let Message::Text(text) = message else {
            continue;
        };

        // Don't log message contents, they can hold session tokens
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(_message) => tracing::debug!("Received a valid message"),
            Err(error) => tracing::debug!("Received invalid JSON: {error}"),
        }
    }

    tracing::debug!("WebSocket disconnected");
}
