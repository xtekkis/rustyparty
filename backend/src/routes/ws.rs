use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::AppState;
use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::clean_nickname;

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// One player's connection: read from the phone and forward messages from the server
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::debug!("WebSocket connected");

    // Other parts of the server send to this player through the sender
    let (sender, mut receiver) = mpsc::unbounded_channel::<ServerMessage>();

    // Room code and player id, once this connection has joined
    let mut joined: Option<(String, u32)> = None;

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                // Only text messages carry JSON; pings and closing are handled automatically
                let Message::Text(text) = message else {
                    continue;
                };

                let reply = handle_text(&text, &state, &mut joined, &sender);
                if let Some(reply) = reply
                    && send_message(&mut socket, &reply).await.is_err()
                {
                    break;
                }
            }
            Some(outgoing) = receiver.recv() => {
                if send_message(&mut socket, &outgoing).await.is_err() {
                    break;
                }
            }
        }
    }

    tracing::debug!("WebSocket disconnected");
}

// Handle one message from the phone and return the reply, if any
fn handle_text(
    text: &str,
    state: &AppState,
    joined: &mut Option<(String, u32)>,
    sender: &UnboundedSender<ServerMessage>,
) -> Option<ServerMessage> {
    // Don't log message contents, they can hold session tokens
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::Join {
            room: code,
            nickname,
            avatar,
            ..
        }) => {
            if joined.is_some() {
                return Some(error_message("already_joined", "You are already in a room"));
            }
            let Some(nickname) = clean_nickname(&nickname) else {
                return Some(error_message(
                    "invalid_nickname",
                    "Nickname must be 1 to 16 characters",
                ));
            };

            // Accept lowercase codes too
            let code = code.to_uppercase();
            // Check and add under one lock so two players can't take the same nickname
            let result = {
                let mut rooms = state.rooms.lock().unwrap();
                match rooms.get_mut(&code) {
                    None => Err(error_message("room_not_found", "No room with that code")),
                    Some(room) if room.has_nickname(&nickname) => {
                        Err(error_message("nickname_taken", "That nickname is taken"))
                    }
                    Some(room) => {
                        let player = room.add_player(nickname, avatar, sender.clone());
                        let new_player = (player.id, player.token.clone());
                        room.broadcast(&room.lobby_message());
                        Ok(new_player)
                    }
                }
            };

            match result {
                Ok((id, token)) => {
                    tracing::info!("Player {id} joined room {code}");
                    *joined = Some((code, id));
                    Some(ServerMessage::Welcome {
                        player_id: id,
                        token,
                    })
                }
                Err(reply) => Some(reply),
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
    }
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
