use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use tokio::sync::{mpsc, oneshot};

use crate::AppState;
use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::actor::{Connection, RoomCommand, RoomHandle};

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// One player's connection: read from the phone and forward messages from their room
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::debug!("WebSocket connected");

    // The room sends to this player through the sender
    let (sender, mut receiver) = mpsc::unbounded_channel::<ServerMessage>();

    // The room and player id, once this connection has joined
    let mut joined: Option<(RoomHandle, u32)> = None;

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

                let reply = handle_text(&text, &state, &mut joined, &sender).await;
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
                // Kicked, banned or replaced connections are closed by the server
                if matches!(
                    outgoing,
                    ServerMessage::Kicked | ServerMessage::Banned | ServerMessage::Replaced
                ) {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    }

    tracing::debug!("WebSocket disconnected");

    // Keep the seat, the player can come back with their token
    if let Some((room, player_id)) = joined {
        room.send(RoomCommand::Disconnect {
            player_id,
            connection: sender,
        })
        .await;
    }
}

// Turn one message from the phone into a command for their room.
// The room sends its own replies, only connection errors are returned here
async fn handle_text(
    text: &str,
    state: &AppState,
    joined: &mut Option<(RoomHandle, u32)>,
    sender: &Connection,
) -> Option<ServerMessage> {
    // Don't log message contents, they can hold session tokens
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            tracing::debug!("Received invalid JSON: {error}");
            return Some(ServerMessage::error(
                "invalid_message",
                "Could not read that message",
            ));
        }
    };

    match message {
        ClientMessage::Join {
            room,
            nickname,
            avatar,
            token,
        } => join(room, nickname, avatar, token, state, joined, sender).await,
        ClientMessage::Leave => {
            // take() empties joined, so this connection can join a room again
            let Some((room, player_id)) = joined.take() else {
                return Some(not_joined());
            };
            room.send(RoomCommand::Leave { player_id }).await;
            None
        }
        message => {
            let Some((room, player_id)) = joined else {
                return Some(not_joined());
            };
            let player_id = *player_id;
            let command = match message {
                ClientMessage::SetAvatar { avatar } => RoomCommand::SetAvatar { player_id, avatar },
                ClientMessage::Ready { ready } => RoomCommand::Ready { player_id, ready },
                ClientMessage::RoomSettings { points_to_win } => RoomCommand::SetPointsToWin {
                    player_id,
                    points_to_win,
                },
                ClientMessage::Kick {
                    player_id: target_id,
                } => RoomCommand::Remove {
                    player_id,
                    target_id,
                    ban: false,
                },
                ClientMessage::Ban {
                    player_id: target_id,
                } => RoomCommand::Remove {
                    player_id,
                    target_id,
                    ban: true,
                },
                // Other messages are handled in later steps
                _ => return None,
            };
            room.send(command).await;
            None
        }
    }
}

// Find the room and ask it to let this connection in
async fn join(
    code: String,
    nickname: String,
    avatar: Option<String>,
    token: Option<String>,
    state: &AppState,
    joined: &mut Option<(RoomHandle, u32)>,
    sender: &Connection,
) -> Option<ServerMessage> {
    if joined.is_some() {
        return Some(ServerMessage::error(
            "already_joined",
            "You are already in a room",
        ));
    }

    // Accept lowercase codes too
    let code = code.to_uppercase();
    let room = state.rooms.lock().unwrap().get(&code).cloned();
    let Some(room) = room else {
        return Some(ServerMessage::error(
            "room_not_found",
            "No room with that code",
        ));
    };

    // The room sends the welcome or an error to the phone itself
    let (reply, player_id) = oneshot::channel();
    room.send(RoomCommand::Join {
        nickname,
        avatar,
        token,
        connection: sender.clone(),
        reply,
    })
    .await;
    if let Ok(Some(player_id)) = player_id.await {
        *joined = Some((room, player_id));
    }
    None
}

fn not_joined() -> ServerMessage {
    ServerMessage::error("not_joined", "Join a room first")
}

// Send one message to this player as JSON
async fn send_message(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("server messages always turn into JSON");
    socket.send(Message::Text(text.into())).await
}
