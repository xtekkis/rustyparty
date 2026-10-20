use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender, WeakSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::player::is_known_avatar;
use super::{Room, RoomState, clean_nickname, is_valid_points_to_win};
use crate::protocol::ServerMessage;

// How many commands can wait for a room before senders have to wait
const COMMAND_QUEUE_SIZE: usize = 64;

// How long a disconnected player keeps their seat
const DISCONNECT_GRACE: Duration = Duration::from_secs(90);

// A room with nobody connected for this long deletes itself
const EMPTY_ROOM_LIFETIME: Duration = Duration::from_secs(5 * 60);

// All rooms by code, shared by the whole server
pub type RoomMap = Arc<Mutex<HashMap<String, RoomHandle>>>;

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
    // Sent by the room to itself when it has been empty for a while
    EmptyTimeout {
        since: Instant,
    },
}

// How the rest of the server talks to a room
#[derive(Clone)]
pub struct RoomHandle {
    commands: mpsc::Sender<RoomCommand>,
}

impl RoomHandle {
    // Start the task for a new room
    pub fn spawn(code: String, rooms: RoomMap) -> RoomHandle {
        let (commands, receiver) = mpsc::channel(COMMAND_QUEUE_SIZE);
        tokio::spawn(run_room(
            Room::new(code),
            receiver,
            commands.downgrade(),
            rooms,
        ));
        RoomHandle { commands }
    }

    // Send a command, false if the room is gone
    pub async fn send(&self, command: RoomCommand) -> bool {
        self.commands.send(command).await.is_ok()
    }
}

// The room's task: handle commands one at a time, until the room deletes itself
async fn run_room(
    mut room: Room,
    mut commands: mpsc::Receiver<RoomCommand>,
    // Weak, so the room doesn't keep itself alive
    me: WeakSender<RoomCommand>,
    rooms: RoomMap,
) {
    // A new room starts empty
    room.watch_if_empty(&me);

    while let Some(command) = commands.recv().await {
        if let RoomCommand::EmptyTimeout { since } = command {
            // Still empty since that same moment, so nobody came back
            if room.empty_since == Some(since) {
                rooms.lock().unwrap().remove(&room.code);
                tracing::info!("Deleted empty room {}", room.code);
                break;
            }
            continue;
        }

        room.handle(command, &me);
        room.watch_if_empty(&me);
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
            // Handled in run_room
            RoomCommand::EmptyTimeout { .. } => {}
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
        if self.state != RoomState::Lobby {
            self.send_error(
                player_id,
                "not_in_lobby",
                "That can only be changed in the lobby",
            );
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

        remind(
            me,
            DISCONNECT_GRACE,
            RoomCommand::GraceOver { player_id, since },
        );
    }

    // Start the countdown when the last connected player is gone, stop it when someone is back
    fn watch_if_empty(&mut self, me: &WeakSender<RoomCommand>) {
        let anyone_connected = self
            .players
            .iter()
            .any(|player| player.disconnected_since.is_none());
        if anyone_connected {
            self.empty_since = None;
            return;
        }
        // Already counting down
        if self.empty_since.is_some() {
            return;
        }

        let since = Instant::now();
        self.empty_since = Some(since);
        remind(me, EMPTY_ROOM_LIFETIME, RoomCommand::EmptyTimeout { since });
    }

    fn send_error(&self, player_id: u32, code: &str, message: &str) {
        if let Some(player) = self.players.iter().find(|player| player.id == player_id) {
            send(&player.sender, ServerMessage::error(code, message));
        }
    }
}

// Send the room a command after a delay, unless the room is gone by then
fn remind(me: &WeakSender<RoomCommand>, delay: Duration, command: RoomCommand) {
    let me = me.clone();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        if let Some(commands) = me.upgrade() {
            let _ = commands.send(command).await;
        }
    });
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

    // A room registered in its own map, like the server does it
    fn spawn_room() -> (RoomHandle, RoomMap) {
        let rooms = RoomMap::default();
        let room = RoomHandle::spawn("KXTP".to_string(), rooms.clone());
        rooms
            .lock()
            .unwrap()
            .insert("KXTP".to_string(), room.clone());
        (room, rooms)
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

    #[test]
    fn points_to_win_only_change_in_the_lobby() {
        let mut room = Room::new("KXTP".to_string());
        let (connection, mut messages) = mpsc::unbounded_channel();
        let alex = room.add_player("Alex".to_string(), None, connection).id;

        room.state = RoomState::InGame;
        room.set_points_to_win(alex, 10);
        assert_eq!(error_code(messages.try_recv().ok()), "not_in_lobby");
        assert_eq!(room.settings.points_to_win, 7);

        room.state = RoomState::Lobby;
        room.set_points_to_win(alex, 10);
        assert_eq!(room.settings.points_to_win, 10);
    }

    #[tokio::test]
    async fn join_gets_welcome_then_lobby() {
        let (room, _rooms) = spawn_room();
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
        let (room, _rooms) = spawn_room();
        join(&room, "Alex", None).await;
        let (player_id, mut messages) = join(&room, "alex", None).await;

        assert_eq!(player_id, None);
        assert_eq!(error_code(messages.recv().await), "nickname_taken");
    }

    #[tokio::test]
    async fn only_the_host_can_kick() {
        let (room, _rooms) = spawn_room();
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
        let (room, _rooms) = spawn_room();
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

    #[tokio::test(start_paused = true)]
    async fn empty_room_deletes_itself_after_5_minutes() {
        let (room, rooms) = spawn_room();

        tokio::time::sleep(EMPTY_ROOM_LIFETIME - Duration::from_secs(1)).await;
        assert!(rooms.lock().unwrap().contains_key("KXTP"));

        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(!rooms.lock().unwrap().contains_key("KXTP"));
        assert!(
            !room.send(RoomCommand::Leave { player_id: 1 }).await,
            "the task has ended"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn room_with_a_connected_player_is_kept() {
        let (room, rooms) = spawn_room();
        let (_alex, _messages) = join(&room, "Alex", None).await;

        tokio::time::sleep(EMPTY_ROOM_LIFETIME * 2).await;
        assert!(rooms.lock().unwrap().contains_key("KXTP"));
    }

    #[tokio::test(start_paused = true)]
    async fn someone_joining_stops_the_countdown() {
        let (room, rooms) = spawn_room();
        tokio::time::sleep(Duration::from_secs(200)).await;

        // Alex is there from 200s to 300s, then the room is empty again
        let (alex, _messages) = join(&room, "Alex", None).await;
        tokio::time::sleep(Duration::from_secs(100)).await;
        room.send(RoomCommand::Leave {
            player_id: alex.unwrap(),
        })
        .await;

        // The first countdown would have ended at 300s, the new one ends at 600s
        tokio::time::sleep(Duration::from_secs(250)).await;
        assert!(rooms.lock().unwrap().contains_key("KXTP"));
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert!(!rooms.lock().unwrap().contains_key("KXTP"));
    }
}
