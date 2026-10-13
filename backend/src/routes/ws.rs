use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};

use crate::AppState;
use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::clean_nickname;

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// Read messages from one player
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::debug!("WebSocket connected");

    // Room code and player id, once this connection has joined
    let mut joined: Option<(String, u32)> = None;

    while let Some(Ok(message)) = socket.recv().await {
        // Only text messages carry JSON; pings and closing are handled automatically
        let Message::Text(text) = message else {
            continue;
        };

        // Don't log message contents, they can hold session tokens
        let reply = match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Join {
                room: code,
                nickname,
                ..
            }) => {
                if joined.is_some() {
                    Some(error_message("already_joined", "You are already in a room"))
                } else if let Some(nickname) = clean_nickname(&nickname) {
                    // Accept lowercase codes too
                    let code = code.to_uppercase();
                    let player_id = {
                        let mut rooms = state.rooms.lock().unwrap();
                        rooms.get_mut(&code).map(|room| room.add_player(nickname))
                    };

                    match player_id {
                        Some(id) => {
                            tracing::info!("Player {id} joined room {code}");
                            joined = Some((code, id));
                            None
                        }
                        None => Some(error_message("room_not_found", "No room with that code")),
                    }
                } else {
                    Some(error_message(
                        "invalid_nickname",
                        "Nickname must be 1 to 16 characters",
                    ))
                }
            }
            // Other messages are handled in later steps
            Ok(_) => None,
            Err(error) => {
                tracing::debug!("Received invalid JSON: {error}");
                Some(error_message(
                    "invalid_message",
                    "Could not read that message",
                ))
            }
        };

        if let Some(reply) = reply
            && send_message(&mut socket, &reply).await.is_err()
        {
            break;
        }
    }

    tracing::debug!("WebSocket disconnected");
}

// Error message with a fixed code and a text to show
fn error_message(code: &str, message: &str) -> ServerMessage {
    ServerMessage::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

// Send one message to this player as JSON
async fn send_message(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("server messages always turn into JSON");
    socket.send(Message::Text(text.into())).await
}
