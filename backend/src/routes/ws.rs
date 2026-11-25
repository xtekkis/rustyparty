use axum::{
    extract::{
        Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{
        StatusCode,
        header::{ORIGIN, RETRY_AFTER},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::IpAddr;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::AppState;
use crate::client_ip::{ClientIp, client_ip};
use crate::limits::{Bucket, MESSAGES};
use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::actor::{Connection, RoomCommand, RoomHandle};

// The biggest real message, a join, is well under 1 KB. Without a limit one message
// could be 64 MB, all of it read into memory before it's refused
const MAX_MESSAGE_SIZE: usize = 16 * 1024;

// Checked before a connection is opened.
// Browsers say which website opened it, and other websites are refused. Apps and scripts
// send no Origin and are let through: this only stops other websites.
// Each address may only open so many connections
pub async fn check_websocket(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(origin) = request.headers().get(ORIGIN)
        && !state.origins.allows(origin)
    {
        tracing::warn!("Refused a WebSocket opened by {origin:?}");
        return StatusCode::FORBIDDEN.into_response();
    }
    let ip = client_ip(request.headers(), request.extensions(), state.behind_proxy);
    if !state.limits.websockets.allow(ip) {
        tracing::warn!("Too many WebSockets opened from one address");
        let retry_after = state.limits.websockets.rate().retry_after();
        return (StatusCode::TOO_MANY_REQUESTS, [(RETRY_AFTER, retry_after)]).into_response();
    }
    next.run(request).await
}

// Upgrade the HTTP request to a WebSocket
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
) -> Response {
    ws.max_message_size(MAX_MESSAGE_SIZE)
        .max_frame_size(MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, state, ip))
}

// One player's connection: read from the phone and forward messages from their room
async fn handle_socket(mut socket: WebSocket, state: AppState, ip: IpAddr) {
    tracing::debug!("WebSocket connected");

    // The room sends to this player through the sender
    let (sender, mut receiver) = mpsc::unbounded_channel::<ServerMessage>();

    // The room and player id, once this connection has joined
    let mut joined: Option<(RoomHandle, u32)> = None;

    // Messages beyond the limit are dropped. The phone hears about it once, and again only
    // after the connection has calmed down enough for its bucket to fill up
    let mut messages = Bucket::full(MESSAGES, Instant::now());
    let mut warned = false;

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                // Every frame counts, pings and binary ones too
                let now = Instant::now();
                if messages.is_full(MESSAGES, now) {
                    warned = false;
                }
                if !matches!(message, Message::Close(_)) && !messages.take(MESSAGES, now) {
                    if !warned {
                        warned = true;
                        tracing::warn!("Dropping messages from a connection sending too fast");
                        if send_message(&mut socket, &ServerMessage::SlowDown).await.is_err() {
                            break;
                        }
                    }
                    continue;
                }
                // Only text messages carry JSON; pings and closing are handled automatically
                let Message::Text(text) = message else {
                    continue;
                };
                let reply = handle_text(&text, &state, ip, &mut joined, &sender).await;
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
    ip: IpAddr,
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
        } => {
            // A join uses a try like a lookup, and a real room gives it back. An address out
            // of tries is refused for every code alike, so it can't tell which ones exist
            if !state.limits.room_lookups.allow(ip) {
                return Some(ServerMessage::error(
                    "slow_down",
                    "Too many tries, wait a moment and try again",
                ));
            }
            let reply = join(room, nickname, avatar, token, state, joined, sender).await;
            let not_a_room = matches!(&reply, Some(ServerMessage::Error { code, .. }) if code == "room_not_found");
            if !not_a_room {
                state.limits.room_lookups.give_back(ip);
            }
            reply
        }
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
                ClientMessage::Vote { game } => RoomCommand::Vote { player_id, game },
                ClientMessage::GameAction { action } => {
                    RoomCommand::GameAction { player_id, action }
                }
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
                // Handled above
                ClientMessage::Join { .. } | ClientMessage::Leave => return None,
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
        return Some(room_not_found());
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
    match player_id.await {
        Ok(Some(player_id)) => {
            *joined = Some((room, player_id));
            None
        }
        Ok(None) => None,
        // The room deleted itself before it could answer
        Err(_) => Some(room_not_found()),
    }
}

fn room_not_found() -> ServerMessage {
    ServerMessage::error("room_not_found", "No room with that code")
}

fn not_joined() -> ServerMessage {
    ServerMessage::error("not_joined", "Join a room first")
}

// Send one message to this player as JSON
async fn send_message(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let text = serde_json::to_string(message).expect("server messages always turn into JSON");
    socket.send(Message::Text(text.into())).await
}
