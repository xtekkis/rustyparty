use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender, WeakSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::player::is_known_avatar;
use super::{Room, clean_nickname, is_valid_points_to_win};
use crate::protocol::ServerMessage;

// How many commands can wait for a room before senders have to wait
const COMMAND_QUEUE_SIZE: usize = 64;

// How long a disconnected player keeps their seat
const DISCONNECT_GRACE: Duration = Duration::from_secs(90);

// Messages to one player's phone
pub type Connection = UnboundedSender<ServerMessage>;

// Everything a room can be asked to do
pub enum RoomCommand {
    // Join or reconnect, replies with the player id if it worked
    Join {
        nickname: String,
        avatar: Option<String>,
        token: Option<String>,
        connection: Connection,
        reply: oneshot::Sender<Option<u32>>,
    },
    SetAvatar {
        player_id: u32,
        avatar: Option<String>,
    },
    Ready {
        player_id: u32,
        ready: bool,
    },
    SetPointsToWin {
        player_id: u32,
        points_to_win: u8,
    },
    // Kick or ban another player
    Remove {
        player_id: u32,
        target_id: u32,
        ban: bool,
    },
    Leave {
        player_id: u32,
    },
    Disconnect {
        player_id: u32,
        connection: Connection,
    },
    // Sent by the room to itself when a grace period ends
    GraceOver {
        player_id: u32,
        since: Instant,
    },
    PlayerCount {
        reply: oneshot::Sender<usize>,
    },
    IsAbandoned {
        now: Instant,
        reply: oneshot::Sender<bool>,
    },
}

// How the rest of the server talks to a room
#[derive(Clone)]
pub struct RoomHandle {
    commands: mpsc::Sender<RoomCommand>,
}

impl RoomHandle {
    // Start the task for a new room
    pub fn spawn(code: String) -> RoomHandle {
        let (commands, receiver) = mpsc::channel(COMMAND_QUEUE_SIZE);
        tokio::spawn(run_room(Room::new(code), receiver, commands.downgrade()));
        RoomHandle { commands }
    }

    // Send a command, false if the room is gone
    pub async fn send(&self, command: RoomCommand) -> bool {
        self.commands.send(command).await.is_ok()
    }
}

// The room's task: handle commands one at a time, until nobody can send any more
async fn run_room(
    mut room: Room,
    mut commands: mpsc::Receiver<RoomCommand>,
    // Weak, so the room doesn't keep itself alive
    me: WeakSender<RoomCommand>,
) {
    while let Some(command) = commands.recv().await {
        room.handle(command, &me);
    }
}

impl Room {
    fn handle(&mut self, command: RoomCommand, me: &WeakSender<RoomCommand>) {
        match command {
            RoomCommand::Join {
                nickname,
                avatar,
                token,
                connection,
                reply,
            } => {
                let player_id = self.join(nickname, avatar, token, connection);
                let _ = reply.send(player_id);
            }
            RoomCommand::SetAvatar { player_id, avatar } => self.set_avatar(player_id, avatar),
            RoomCommand::Ready { player_id, ready } => self.set_ready(player_id, ready),
            RoomCommand::SetPointsToWin {
                player_id,
                points_to_win,
            } => self.set_points_to_win(player_id, points_to_win),
            RoomCommand::Remove {
                player_id,
                target_id,
                ban,
            } => self.remove_by_host(player_id, target_id, ban),
            RoomCommand::Leave { player_id } => self.leave(player_id),
            RoomCommand::Disconnect {
                player_id,
                connection,
            } => self.disconnect(player_id, &connection, me),
            RoomCommand::GraceOver { player_id, since } => {
                if self.remove_if_gone_since(player_id, since) {
                    tracing::info!(
                        "Player {player_id} didn't come back and was removed from room {}",
                        self.code
                    );
                    self.broadcast(&self.lobby_message());
                }
            }
            RoomCommand::PlayerCount { reply } => {
                let _ = reply.send(self.players.len());
            }
            RoomCommand::IsAbandoned { now, reply } => {
                let _ = reply.send(self.is_abandoned(now));
            }
        }
    }

    // Add a new player, or give a returning player their seat back
    fn join(
        &mut self,
        nickname: String,
        avatar: Option<String>,
        token: Option<String>,
        connection: Connection,
    ) -> Option<u32> {
        // A saved token means this phone was here before
        if let Some(token) = token {
            if self.is_banned(&token) {
                send(
                    &connection,
                    ServerMessage::error("banned_from_room", "You were banned from this room"),
                );
                return None;
            }
            if let Some(id) = self.reconnect(&token, connection.clone()) {
                tracing::info!("Player {id} reconnected to room {}", self.code);
                send(
                    &connection,
                    ServerMessage::Welcome {
                        player_id: id,
                        token,
                    },
                );
                self.broadcast(&self.lobby_message());
                return Some(id);
            }
        }

        let error = |code, message| {
            send(&connection, ServerMessage::error(code, message));
            None
        };
        let Some(nickname) = clean_nickname(&nickname) else {
            return error("invalid_nickname", "Nickname must be 1 to 16 characters");
        };
        if !is_known_avatar(avatar.as_deref()) {
            return error("unknown_avatar", "That avatar doesn't exist");
        }
        if self.has_nickname(&nickname) {
            return error("nickname_taken", "That nickname is taken");
        }

        let player = self.add_player(nickname, avatar, connection.clone());
        let (id, token) = (player.id, player.token.clone());
        tracing::info!("Player {id} joined room {}", self.code);
        // Welcome first, so the phone knows its id before the lobby arrives
        send(
            &connection,
            ServerMessage::Welcome {
                player_id: id,
                token,
            },
        );
        self.broadcast(&self.lobby_message());
        Some(id)
    }

    fn set_avatar(&mut self, player_id: u32, avatar: Option<String>) {
        if !is_known_avatar(avatar.as_deref()) {
            self.send_error(player_id, "unknown_avatar", "That avatar doesn't exist");
            return;
        }
        if let Some(player) = self.player_mut(player_id) {
            player.avatar = avatar;
            self.broadcast(&self.lobby_message());
        }
    }

    fn set_ready(&mut self, player_id: u32, ready: bool) {
        if let Some(player) = self.player_mut(player_id) {
            player.ready = ready;
            tracing::debug!("Player {player_id} ready: {ready}");
            self.broadcast(&self.lobby_message());
        }
    }

    fn set_points_to_win(&mut self, player_id: u32, points_to_win: u8) {
        if !is_valid_points_to_win(points_to_win) {
            self.send_error(
                player_id,
                "invalid_points_to_win",
                "Points to win must be 3 to 15",
            );
            return;
        }
        if self.host != Some(player_id) {
            self.send_error(player_id, "not_host", "Only the host can do that");
            return;
        }
        self.settings.points_to_win = points_to_win;
        self.broadcast(&self.lobby_message());
    }

    // Host kicks or bans another player
    fn remove_by_host(&mut self, player_id: u32, target_id: u32, ban: bool) {
        if self.host != Some(player_id) {
            self.send_error(player_id, "not_host", "Only the host can do that");
            return;
        }
        if target_id == player_id {
            self.send_error(
                player_id,
                "cannot_remove_yourself",
                "You can't remove yourself",
            );
            return;
        }

        let removed = if ban {
            self.ban_player(target_id)
        } else {
            self.kick_player(target_id)
        };
        if !removed {
            self.send_error(
                player_id,
                "player_not_found",
                "That player isn't in the room",
            );
            return;
        }

        let action = if ban { "banned" } else { "kicked" };
        tracing::info!("Player {target_id} was {action} from room {}", self.code);
        self.broadcast(&self.lobby_message());
    }

    fn leave(&mut self, player_id: u32) {
        if self.remove_player(player_id) {
            tracing::info!("Player {player_id} left room {}", self.code);
            self.broadcast(&self.lobby_message());
        }
    }

    // Keep the seat for a while, the player can come back with their token
    fn disconnect(
        &mut self,
        player_id: u32,
        connection: &Connection,
        me: &WeakSender<RoomCommand>,
    ) {
        let since = Instant::now();
        let Some(player) = self.player_mut(player_id) else {
            return;
        };
        // Skip if a newer connection has taken over this seat
        if !player.sender.same_channel(connection) {
            return;
        }
        player.disconnected_since = Some(since);
        tracing::info!("Player {player_id} disconnected from room {}", self.code);
        self.broadcast(&self.lobby_message());

        // Remind the room when the grace period ends
        let me = me.clone();
        tokio::spawn(async move {
            tokio::time::sleep(DISCONNECT_GRACE).await;
            if let Some(commands) = me.upgrade() {
                let _ = commands
                    .send(RoomCommand::GraceOver { player_id, since })
                    .await;
            }
        });
    }

    fn send_error(&self, player_id: u32, code: &str, message: &str) {
        if let Some(player) = self.players.iter().find(|player| player.id == player_id) {
            send(&player.sender, ServerMessage::error(code, message));
        }
    }
}

// A closed connection can't receive, so ignore the error
fn send(connection: &Connection, message: ServerMessage) {
    let _ = connection.send(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::UnboundedReceiver;

    // Join a player and return their id and the messages their phone gets
    async fn join(
        room: &RoomHandle,
        nickname: &str,
        token: Option<String>,
    ) -> (Option<u32>, UnboundedReceiver<ServerMessage>) {
        let (connection, messages) = mpsc::unbounded_channel();
        let (reply, player_id) = oneshot::channel();
        room.send(RoomCommand::Join {
            nickname: nickname.to_string(),
            avatar: None,
            token,
            connection,
            reply,
        })
        .await;
        (player_id.await.unwrap(), messages)
    }

    async fn player_count(room: &RoomHandle) -> usize {
        let (reply, count) = oneshot::channel();
        room.send(RoomCommand::PlayerCount { reply }).await;
        count.await.unwrap()
    }

    fn error_code(message: Option<ServerMessage>) -> String {
        match message {
            Some(ServerMessage::Error { code, .. }) => code,
            _ => panic!("expected an error"),
        }
    }

    #[tokio::test]
    async fn join_gets_welcome_then_lobby() {
        let room = RoomHandle::spawn("KXTP".to_string());
        let (alex, mut messages) = join(&room, "Alex", None).await;

        assert_eq!(alex, Some(1));
        assert!(matches!(
            messages.recv().await,
            Some(ServerMessage::Welcome { player_id: 1, .. })
        ));
        assert!(matches!(
            messages.recv().await,
            Some(ServerMessage::Lobby { .. })
        ));
    }

    #[tokio::test]
    async fn taken_nickname_is_refused() {
        let room = RoomHandle::spawn("KXTP".to_string());
        join(&room, "Alex", None).await;
        let (player_id, mut messages) = join(&room, "alex", None).await;

        assert_eq!(player_id, None);
        assert_eq!(error_code(messages.recv().await), "nickname_taken");
    }

    #[tokio::test]
    async fn only_the_host_can_kick() {
        let room = RoomHandle::spawn("KXTP".to_string());
        join(&room, "Alex", None).await;
        let (sam, mut sam_messages) = join(&room, "Sam", None).await;
        let sam = sam.unwrap();
        // Skip Sam's welcome and lobby
        sam_messages.recv().await;
        sam_messages.recv().await;

        room.send(RoomCommand::Remove {
            player_id: sam,
            target_id: 1,
            ban: false,
        })
        .await;
        assert_eq!(error_code(sam_messages.recv().await), "not_host");
        assert_eq!(player_count(&room).await, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn disconnected_player_is_removed_after_grace() {
        let room = RoomHandle::spawn("KXTP".to_string());
        let (connection, _messages) = mpsc::unbounded_channel();
        let (reply, player_id) = oneshot::channel();
        room.send(RoomCommand::Join {
            nickname: "Sam".to_string(),
            avatar: None,
            token: None,
            connection: connection.clone(),
            reply,
        })
        .await;
        let sam = player_id.await.unwrap().unwrap();

        room.send(RoomCommand::Disconnect {
            player_id: sam,
            connection,
        })
        .await;
        tokio::time::sleep(DISCONNECT_GRACE - Duration::from_secs(1)).await;
        assert_eq!(
            player_count(&room).await,
            1,
            "still within the grace period"
        );

        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(player_count(&room).await, 0);
    }
}
