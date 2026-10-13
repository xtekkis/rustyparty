use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};

use crate::protocol::{ClientMessage, ServerMessage};

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
            Err(error) => {
                tracing::debug!("Received invalid JSON: {error}");
                let reply = ServerMessage::Error {
                    code: "invalid_message".to_string(),
                    message: "Could not read that message".to_string(),
                };
                if send_message(&mut socket, &reply).await.is_err() {
                    break;
                }
            }
        }
    }

    tracing::debug!("WebSocket disconnected");
}

// Send one message to this player as JSON
async fn send_message(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("server messages always turn into JSON");
    socket.send(Message::Text(text.into())).await
}
