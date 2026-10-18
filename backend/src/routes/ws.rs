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
use crate::room::{clean_nickname, is_valid_points_to_win, player::is_known_avatar};

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
                // A kicked player's connection is closed by the server
                if matches!(outgoing, ServerMessage::Kicked) {
                    let _ = socket.send(Message::Close(None)).await;
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
            room,
            nickname,
            avatar,
            token,
        }) => handle_join(room, nickname, avatar, token, state, joined, sender),
        Ok(ClientMessage::SetAvatar { avatar }) => handle_set_avatar(avatar, state, joined),
        Ok(ClientMessage::Ready { ready }) => handle_ready(ready, state, joined),
        Ok(ClientMessage::RoomSettings { points_to_win }) => {
            handle_room_settings(points_to_win, state, joined)
        }
        Ok(ClientMessage::Kick { player_id }) => handle_kick(player_id, state, joined),
        Ok(ClientMessage::Leave) => handle_leave(state, joined),
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

// Add this connection's player to a room
fn handle_join(
    code: String,
    nickname: String,
    avatar: Option<String>,
    token: Option<String>,
    state: &AppState,
    joined: &mut Option<(String, u32)>,
    sender: &UnboundedSender<ServerMessage>,
) -> Option<ServerMessage> {
    if joined.is_some() {
        return Some(error_message("already_joined", "You are already in a room"));
    }
    let Some(nickname) = clean_nickname(&nickname) else {
        return Some(error_message(
            "invalid_nickname",
            "Nickname must be 1 to 16 characters",
        ));
    };
    if !is_known_avatar(avatar.as_deref()) {
        return Some(error_message("unknown_avatar", "That avatar doesn't exist"));
    }

    // Accept lowercase codes too
    let code = code.to_uppercase();
    // Check and add under one lock so two players can't take the same nickname
    let result = {
        let mut rooms = state.rooms.lock().unwrap();
        match rooms.get_mut(&code) {
            None => Err(error_message("room_not_found", "No room with that code")),
            Some(room) if token.as_deref().is_some_and(|token| room.is_kicked(token)) => Err(
                error_message("kicked_from_room", "You were removed from this room"),
            ),
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

// Change this connection's avatar and show it to everyone
fn handle_set_avatar(
    avatar: Option<String>,
    state: &AppState,
    joined: &Option<(String, u32)>,
) -> Option<ServerMessage> {
    let Some((code, player_id)) = joined else {
        return Some(error_message("not_joined", "Join a room first"));
    };
    if !is_known_avatar(avatar.as_deref()) {
        return Some(error_message("unknown_avatar", "That avatar doesn't exist"));
    }

    let mut rooms = state.rooms.lock().unwrap();
    if let Some(room) = rooms.get_mut(code)
        && let Some(player) = room.player_mut(*player_id)
    {
        player.avatar = avatar;
        room.broadcast(&room.lobby_message());
    }
    None
}

// Mark this connection's player as ready or not
fn handle_ready(
    ready: bool,
    state: &AppState,
    joined: &Option<(String, u32)>,
) -> Option<ServerMessage> {
    let Some((code, player_id)) = joined else {
        return Some(error_message("not_joined", "Join a room first"));
    };

    let mut rooms = state.rooms.lock().unwrap();
    if let Some(room) = rooms.get_mut(code)
        && let Some(player) = room.player_mut(*player_id)
    {
        player.ready = ready;
        tracing::debug!("Player {player_id} ready: {ready}");
        room.broadcast(&room.lobby_message());
    }
    None
}

// Host changes how many points win the game
fn handle_room_settings(
    points_to_win: u8,
    state: &AppState,
    joined: &Option<(String, u32)>,
) -> Option<ServerMessage> {
    let Some((code, player_id)) = joined else {
        return Some(error_message("not_joined", "Join a room first"));
    };
    if !is_valid_points_to_win(points_to_win) {
        return Some(error_message(
            "invalid_points_to_win",
            "Points to win must be 3 to 15",
        ));
    }

    let mut rooms = state.rooms.lock().unwrap();
    let room = rooms.get_mut(code)?;
    if room.host != Some(*player_id) {
        return Some(error_message("not_host", "Only the host can do that"));
    }

    room.settings.points_to_win = points_to_win;
    room.broadcast(&room.lobby_message());
    None
}

// Host removes another player from the room
fn handle_kick(
    target_id: u32,
    state: &AppState,
    joined: &Option<(String, u32)>,
) -> Option<ServerMessage> {
    let Some((code, player_id)) = joined else {
        return Some(error_message("not_joined", "Join a room first"));
    };

    let mut rooms = state.rooms.lock().unwrap();
    let room = rooms.get_mut(code)?;
    if room.host != Some(*player_id) {
        return Some(error_message("not_host", "Only the host can do that"));
    }
    if target_id == *player_id {
        return Some(error_message(
            "cannot_kick_yourself",
            "You can't kick yourself",
        ));
    }
    if !room.kick_player(target_id) {
        return Some(error_message(
            "player_not_found",
            "That player isn't in the room",
        ));
    }

    tracing::info!("Player {target_id} was kicked from room {code}");
    room.broadcast(&room.lobby_message());
    None
}

// Take this connection's player out of their room
fn handle_leave(state: &AppState, joined: &mut Option<(String, u32)>) -> Option<ServerMessage> {
    // take() empties joined, so this connection can join a room again
    let Some((code, player_id)) = joined.take() else {
        return Some(error_message("not_joined", "Join a room first"));
    };

    let mut rooms = state.rooms.lock().unwrap();
    if let Some(room) = rooms.get_mut(&code)
        && room.remove_player(player_id)
    {
        tracing::info!("Player {player_id} left room {code}");
        room.broadcast(&room.lobby_message());
    }
    None
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
