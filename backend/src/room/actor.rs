use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender, WeakSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::player::is_known_avatar;
use super::vote::pick_winner;
use super::{
    DISCONNECT_GRACE, MAX_ROOM_PLAYERS, Room, RoomState, clean_nickname, is_valid_points_to_win,
    seconds_until,
};
use crate::games::{GameCtx, GameInfo, TimerId, find_game};
use crate::protocol::ServerMessage;
use serde_json::Value;

// How many commands can wait for a room before senders have to wait
const COMMAND_QUEUE_SIZE: usize = 64;

// The game vote needs at least this many players
const MIN_PLAYERS_TO_VOTE: usize = 3;

// How long players have to vote
pub(super) const VOTE_DURATION: Duration = Duration::from_secs(30);

// How long the final scores stay on screen
pub(super) const RESULTS_DURATION: Duration = Duration::from_secs(10);

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

    // Add a new player, or give a returning player their seat back
    pub(super) fn join(
        &mut self,
        nickname: String,
        avatar: Option<String>,
        token: Option<String>,
        connection: Connection,
        me: &WeakSender<RoomCommand>,
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
                self.broadcast_players();
                // Back mid-game: show them where things are right away
                self.send_game_view_to(id);
                return Some(id);
            }
        }

        let error = |code, message| {
            send(&connection, ServerMessage::error(code, message));
            None
        };
        // Players in their grace period still hold a seat
        if self.players.len() >= MAX_ROOM_PLAYERS {
            return error("room_full", "This room is full");
        }
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
        self.broadcast_players();
        self.tell_game_about_late_joiner(id, me);
        Some(id)
    }

    // The game decides when they start playing, usually at its next round
    fn tell_game_about_late_joiner(&mut self, player_id: u32, me: &WeakSender<RoomCommand>) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            return;
        };
        let number = *number;
        let mut ctx = GameCtx::default();
        game.player_joined(player_id, &mut ctx);
        self.run_game_ctx(ctx, number, me);
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

    pub(super) fn set_ready(&mut self, player_id: u32, ready: bool) {
        if !matches!(self.state, RoomState::Lobby) {
            self.send_error(
                player_id,
                "not_in_lobby",
                "That can only be changed in the lobby",
            );
            return;
        }
        if let Some(player) = self.player_mut(player_id) {
            player.ready = ready;
            tracing::debug!("Player {player_id} ready: {ready}");
            self.broadcast(&self.lobby_message());
        }
    }

    pub(super) fn vote(&mut self, player_id: u32, game_id: &str) {
        if !matches!(self.state, RoomState::Voting { .. }) {
            self.send_error(player_id, "not_voting", "Voting isn't open");
            return;
        }
        let Some(game) = find_game(self.games, game_id) else {
            self.send_error(player_id, "unknown_game", "That game doesn't exist");
            return;
        };
        if !game.fits(self.players.len()) {
            self.send_error(
                player_id,
                "game_not_available",
                "That game doesn't fit the number of players",
            );
            return;
        }

        if let RoomState::Voting { votes, .. } = &mut self.state {
            votes.insert(player_id, game.id);
        }
        self.broadcast(&self.vote_message());
    }

    // Pass a player's move to the game, or tell them why the game refused it
    pub(super) fn game_action(
        &mut self,
        player_id: u32,
        action: Value,
        me: &WeakSender<RoomCommand>,
    ) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            self.send_error(player_id, "not_in_game", "No game is running");
            return;
        };
        let number = *number;

        let mut ctx = GameCtx::default();
        match game.handle_action(player_id, action, &mut ctx) {
            Ok(()) => self.run_game_ctx(ctx, number, me),
            // A refused action has no effects, whatever the game wrote into ctx is dropped
            Err(error) => self.send_error(player_id, error.code, error.message),
        }
    }

    pub(super) fn set_points_to_win(&mut self, player_id: u32, points_to_win: u8) {
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
        if !matches!(self.state, RoomState::Lobby) {
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
    pub(super) fn remove_by_host(
        &mut self,
        player_id: u32,
        target_id: u32,
        ban: bool,
        me: &WeakSender<RoomCommand>,
    ) {
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
        self.broadcast_players();
        self.tell_game_about_leaver(target_id, me);
    }

    pub(super) fn leave(&mut self, player_id: u32, me: &WeakSender<RoomCommand>) {
        if self.remove_player(player_id) {
            tracing::info!("Player {player_id} left room {}", self.code);
            self.broadcast_players();
            self.tell_game_about_leaver(player_id, me);
        }
    }

    // Someone is gone for good, the game may need to cancel a round or end
    fn tell_game_about_leaver(&mut self, player_id: u32, me: &WeakSender<RoomCommand>) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            return;
        };
        let number = *number;
        let mut ctx = GameCtx::default();
        game.player_left(player_id, &mut ctx);
        self.run_game_ctx(ctx, number, me);
    }

    // Keep the seat for a while, the player can come back with their token
    pub(super) fn disconnect(
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

    // Nobody is ready again until they choose to be, or the vote would start straight away
    fn return_to_lobby(&mut self) {
        self.state = RoomState::Lobby;
        for player in &mut self.players {
            player.ready = false;
        }
        tracing::info!("Room {} is back in the lobby", self.code);
        self.broadcast_players();
    }

    // The game says it's over: show the results for a while
    pub(super) fn finish_game_if_over(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::InGame { game, number, .. } = &self.state else {
            return;
        };
        if !game.is_finished() {
            return;
        }

        let number = *number;
        self.state = RoomState::Results { number };
        tracing::info!("Game {number} finished in room {}", self.code);
        self.broadcast(&ServerMessage::Results {
            ends_in: RESULTS_DURATION.as_secs(),
        });
        remind(me, RESULTS_DURATION, RoomCommand::ResultsOver { number });
    }

    // Start a fresh game with everyone who has a seat
    pub(super) fn start_game(&mut self, info: &GameInfo, me: &WeakSender<RoomCommand>) {
        self.games_started += 1;
        let number = self.games_started;

        let mut game = (info.create)(&self.settings);
        let players: Vec<u32> = self.players.iter().map(|player| player.id).collect();
        let mut ctx = GameCtx::default();
        game.start(&players, &mut ctx);

        self.state = RoomState::InGame {
            game,
            number,
            ends_at: None,
        };
        tracing::info!("Game {} started in room {}", info.id, self.code);
        self.run_game_ctx(ctx, number, me);
    }

    // Everyone connected is ready and there are enough of them: start the game vote
    pub(super) fn start_vote_if_ready(&mut self, me: &WeakSender<RoomCommand>) {
        if !matches!(self.state, RoomState::Lobby) {
            return;
        }
        // Players in their grace period don't hold up the others
        let connected: Vec<_> = self
            .players
            .iter()
            .filter(|player| player.disconnected_since.is_none())
            .collect();
        if connected.len() < MIN_PLAYERS_TO_VOTE || !connected.iter().all(|player| player.ready) {
            return;
        }

        self.votes_started += 1;
        let number = self.votes_started;
        self.state = RoomState::Voting {
            votes: HashMap::new(),
            number,
            ends_at: Instant::now() + VOTE_DURATION,
        };
        tracing::info!("Voting started in room {}", self.code);
        self.broadcast(&self.vote_message());
        remind(me, VOTE_DURATION, RoomCommand::VoteTimeout { number });
    }

    // No need to wait out the timer once every connected player has picked a game
    pub(super) fn end_vote_if_everyone_voted(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::Voting { votes, .. } = &self.state else {
            return;
        };
        let everyone_voted = self
            .players
            .iter()
            .filter(|player| player.disconnected_since.is_none())
            .all(|player| votes.contains_key(&player.id));
        if everyone_voted {
            self.end_vote(me);
        }
    }

    // Announce the winner and start it, or go back to the lobby if nobody voted
    fn end_vote(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::Voting { votes, .. } = std::mem::replace(&mut self.state, RoomState::Lobby)
        else {
            return;
        };

        let Some(winner) = pick_winner(&votes, &mut rand::rng()) else {
            tracing::info!("Nobody voted in room {}", self.code);
            self.return_to_lobby();
            return;
        };

        tracing::info!("{} won the vote in room {}", winner.game, self.code);
        self.broadcast(&ServerMessage::VoteResult {
            game: winner.game.to_string(),
            tie_break: winner.tie_break,
        });
        if let Some(info) = find_game(self.games, winner.game) {
            self.start_game(info, me);
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

    // Carry out what a game asked for: send its messages and start its timers
    pub(super) fn run_game_ctx(
        &mut self,
        ctx: GameCtx,
        game_number: u32,
        me: &WeakSender<RoomCommand>,
    ) {
        self.send_game_messages(ctx.outgoing);
        for (timer, after) in ctx.timers {
            remind(me, after, RoomCommand::GameTimer { game_number, timer });
            if let RoomState::InGame { ends_at, .. } = &mut self.state {
                *ends_at = Some(Instant::now() + after);
            }
        }
        self.send_game_views();
    }

    // Every player gets the game as they are allowed to see it, never someone else's
    pub(super) fn send_game_views(&self) {
        for player in &self.players {
            self.send_game_view_to(player.id);
        }
    }

    fn send_game_view_to(&self, player_id: u32) {
        let RoomState::InGame { game, ends_at, .. } = &self.state else {
            return;
        };
        let view = game.view_for(player_id);
        // A finished game counts down to nothing, whatever timer was still running
        let ends_in = ends_at.filter(|_| !game.is_finished()).map(seconds_until);
        self.send_to_player(player_id, ServerMessage::GameView { view, ends_in });
    }

    // Send everyone what the game asked to send, each one wrapped as a game event
    pub(super) fn send_game_messages(&self, outgoing: Vec<Value>) {
        for event in outgoing {
            self.broadcast(&ServerMessage::GameEvent { event });
        }
    }

    fn send_to_player(&self, player_id: u32, message: ServerMessage) {
        if let Some(player) = self.players.iter().find(|player| player.id == player_id) {
            send(&player.sender, message);
        }
    }

    fn send_error(&self, player_id: u32, code: &str, message: &str) {
        self.send_to_player(player_id, ServerMessage::error(code, message));
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
