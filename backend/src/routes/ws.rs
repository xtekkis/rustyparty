use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
};

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_socket)
}

// Send every message straight back
async fn handle_socket(mut socket: WebSocket) {
    tracing::debug!("WebSocket connected");

    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(_) | Message::Binary(_) => {
                if socket.send(message).await.is_err() {
                    break;
                }
            }
            // Pings and closing are handled automatically
            _ => {}
        }
    }

    tracing::debug!("WebSocket disconnected");
}
