use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender, WeakSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::{Room, RoomState};
use crate::games::{GameCtx, TimerId};
use crate::protocol::ServerMessage;
use serde_json::Value;

// How many commands can wait for a room before senders have to wait
const COMMAND_QUEUE_SIZE: usize = 64;

// A room with nobody connected for this long deletes itself
pub(super) const EMPTY_ROOM_LIFETIME: Duration = Duration::from_secs(5 * 60);

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
    Vote {
        player_id: u32,
        game: String,
    },
    GameAction {
        player_id: u32,
        action: Value,
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
    // Sent by the room to itself when the results have been shown long enough
    ResultsOver {
        number: u32,
    },
    // Sent by the room to itself when the time to vote is up
    VoteTimeout {
        number: u32,
    },
    // Sent by the room to itself when a timer a game asked for runs out
    GameTimer {
        game_number: u32,
        timer: TimerId,
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
        room.finish_game_if_over(&me);
        room.start_vote_if_ready(&me);
        room.end_vote_if_everyone_voted(&me);
        room.watch_if_empty(&me);
    }
}

impl Room {
    pub(super) fn handle(&mut self, command: RoomCommand, me: &WeakSender<RoomCommand>) {
        match command {
            RoomCommand::Join {
                nickname,
                avatar,
                token,
                connection,
                reply,
            } => {
                let player_id = self.join(nickname, avatar, token, connection, me);
                let _ = reply.send(player_id);
            }
            RoomCommand::SetAvatar { player_id, avatar } => self.set_avatar(player_id, avatar),
            RoomCommand::Ready { player_id, ready } => self.set_ready(player_id, ready),
            RoomCommand::Vote { player_id, game } => self.vote(player_id, &game),
            RoomCommand::GameAction { player_id, action } => {
                self.game_action(player_id, action, me)
            }
            RoomCommand::SetPointsToWin {
                player_id,
                points_to_win,
            } => self.set_points_to_win(player_id, points_to_win),
            RoomCommand::Remove {
                player_id,
                target_id,
                ban,
            } => self.remove_by_host(player_id, target_id, ban, me),
            RoomCommand::Leave { player_id } => self.leave(player_id, me),
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
                    self.broadcast_players();
                    self.tell_game_about_leaver(player_id, me);
                }
            }
            RoomCommand::PlayerCount { reply } => {
                let _ = reply.send(self.players.len());
            }
            RoomCommand::ResultsOver { number } => {
                if let RoomState::Results { number: current } = self.state
                    && current == number
                {
                    self.return_to_lobby();
                }
            }
            RoomCommand::VoteTimeout { number } => {
                if let RoomState::Voting {
                    number: current, ..
                } = &self.state
                    && *current == number
                {
                    self.end_vote(me);
                }
            }
            RoomCommand::GameTimer { game_number, timer } => {
                if let RoomState::InGame { game, number, .. } = &mut self.state
                    && *number == game_number
                {
                    let mut ctx = GameCtx::default();
                    game.handle_timer(timer, &mut ctx);
                    self.run_game_ctx(ctx, game_number, me);
                }
            }
            // Handled in run_room
            RoomCommand::EmptyTimeout { .. } => {}
        }
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

    pub(super) fn send_to_player(&self, player_id: u32, message: ServerMessage) {
        if let Some(player) = self.players.iter().find(|player| player.id == player_id) {
            send(&player.sender, message);
        }
    }

    pub(super) fn send_error(&self, player_id: u32, code: &str, message: &str) {
        self.send_to_player(player_id, ServerMessage::error(code, message));
    }
}

// Send the room a command after a delay, unless the room is gone by then
pub(super) fn remind(me: &WeakSender<RoomCommand>, delay: Duration, command: RoomCommand) {
    let me = me.clone();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        if let Some(commands) = me.upgrade() {
            let _ = commands.send(command).await;
        }
    });
}

// A closed connection can't receive, so ignore the error
pub(super) fn send(connection: &Connection, message: ServerMessage) {
    let _ = connection.send(message);
}
