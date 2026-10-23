use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc::{self, UnboundedSender, WeakSender};
use tokio::sync::oneshot;
use tokio::time::Instant;

use super::player::is_known_avatar;
use super::{Room, RoomState, clean_nickname, is_valid_points_to_win};
use crate::games::{GameCtx, GameInfo, Outgoing, Recipient, TimerId, find_game};
use crate::protocol::ServerMessage;

// How many commands can wait for a room before senders have to wait
const COMMAND_QUEUE_SIZE: usize = 64;

// How long a disconnected player keeps their seat
const DISCONNECT_GRACE: Duration = Duration::from_secs(90);

// The game vote needs at least this many players
const MIN_PLAYERS_TO_VOTE: usize = 3;

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
    Vote {
        player_id: u32,
        game: String,
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
        room.start_vote_if_ready();
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
            RoomCommand::Vote { player_id, game } => self.vote(player_id, &game),
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
                    self.broadcast_players();
                }
            }
            RoomCommand::PlayerCount { reply } => {
                let _ = reply.send(self.players.len());
            }
            RoomCommand::GameTimer { game_number, timer } => {
                if let RoomState::InGame { game, number } = &mut self.state
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
                self.broadcast_players();
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
        self.broadcast_players();
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

    fn vote(&mut self, player_id: u32, game_id: &str) {
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

        if let RoomState::Voting { votes } = &mut self.state {
            votes.insert(player_id, game.id);
        }
        self.broadcast(&self.vote_message());
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
        self.broadcast_players();
    }

    fn leave(&mut self, player_id: u32) {
        if self.remove_player(player_id) {
            tracing::info!("Player {player_id} left room {}", self.code);
            self.broadcast_players();
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

    // Start a fresh game with everyone who has a seat
    fn start_game(&mut self, info: &GameInfo, me: &WeakSender<RoomCommand>) {
        self.games_started += 1;
        let number = self.games_started;

        let mut game = (info.create)(&self.settings);
        let players: Vec<u32> = self.players.iter().map(|player| player.id).collect();
        let mut ctx = GameCtx::default();
        game.start(&players, &mut ctx);

        self.state = RoomState::InGame { game, number };
        tracing::info!("Game {} started in room {}", info.id, self.code);
        self.run_game_ctx(ctx, number, me);
    }

    // Everyone connected is ready and there are enough of them: start the game vote
    fn start_vote_if_ready(&mut self) {
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

        self.state = RoomState::Voting {
            votes: HashMap::new(),
        };
        tracing::info!("Voting started in room {}", self.code);
        self.broadcast(&self.vote_message());
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
    fn run_game_ctx(&self, ctx: GameCtx, game_number: u32, me: &WeakSender<RoomCommand>) {
        self.send_game_messages(ctx.outgoing);
        for (timer, after) in ctx.timers {
            remind(me, after, RoomCommand::GameTimer { game_number, timer });
        }
    }

    // Send what a game asked to send, each one wrapped as a game event
    fn send_game_messages(&self, outgoing: Vec<Outgoing>) {
        for Outgoing { to, message } in outgoing {
            let message = ServerMessage::GameEvent { event: message };
            match to {
                Recipient::Player(player_id) => self.send_to_player(player_id, message),
                Recipient::Everyone => self.broadcast(&message),
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GameInfo;
    use crate::games::dummy::{DUMMY_INFO, DummyGame};
    use serde_json::json;
    use std::collections::BTreeMap;
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

    #[tokio::test(start_paused = true)]
    async fn starting_a_game_runs_its_start() {
        let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
        let (commands, mut receiver) = mpsc::channel(8);
        room.start_game(&DUMMY_INFO, &commands.downgrade());

        assert!(matches!(room.state, RoomState::InGame { number: 1, .. }));
        for phone in &mut phones {
            let mut got_started = false;
            while let Ok(message) = phone.try_recv() {
                if let ServerMessage::GameEvent { event } = message {
                    got_started = event == json!({ "event": "started" });
                }
            }
            assert!(got_started, "every player hears the game start");
        }

        // The dummy game's 30 second time limit was scheduled for game 1
        tokio::time::sleep(Duration::from_secs(31)).await;
        assert!(matches!(
            receiver.try_recv(),
            Ok(RoomCommand::GameTimer {
                game_number: 1,
                timer: 1
            })
        ));
    }

    #[tokio::test]
    async fn each_game_gets_a_new_number() {
        let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
        let (commands, _receiver) = mpsc::channel(8);
        room.start_game(&DUMMY_INFO, &commands.downgrade());
        room.start_game(&DUMMY_INFO, &commands.downgrade());
        assert!(matches!(room.state, RoomState::InGame { number: 2, .. }));
    }

    #[tokio::test(start_paused = true)]
    async fn game_timer_fires_after_its_delay() {
        let room = Room::new("KXTP".to_string());
        let (commands, mut receiver) = mpsc::channel(8);

        let mut ctx = GameCtx::default();
        ctx.start_timer(7, Duration::from_secs(60));
        room.run_game_ctx(ctx, 1, &commands.downgrade());

        tokio::time::sleep(Duration::from_secs(59)).await;
        assert!(receiver.try_recv().is_err(), "not yet");
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(matches!(
            receiver.try_recv(),
            Ok(RoomCommand::GameTimer {
                game_number: 1,
                timer: 7
            })
        ));
    }

    #[test]
    fn game_timer_only_reaches_the_game_that_asked() {
        let mut room = Room::new("KXTP".to_string());
        let (connection, mut messages) = mpsc::unbounded_channel();
        room.add_player("Alex".to_string(), None, connection);
        room.state = RoomState::InGame {
            game: Box::new(DummyGame::default()),
            number: 2,
        };
        let (commands, _receiver) = mpsc::channel(8);
        let me = commands.downgrade();

        // A timer from an earlier game is ignored
        room.handle(
            RoomCommand::GameTimer {
                game_number: 1,
                timer: 1,
            },
            &me,
        );
        assert!(messages.try_recv().is_err());

        room.handle(
            RoomCommand::GameTimer {
                game_number: 2,
                timer: 1,
            },
            &me,
        );
        assert!(matches!(
            messages.try_recv(),
            Ok(ServerMessage::GameEvent { event }) if event == json!({ "event": "time_up" })
        ));
    }

    // A room with these players in it, and their phones' messages
    fn room_with(nicknames: &[&str]) -> (Room, Vec<UnboundedReceiver<ServerMessage>>) {
        let mut room = Room::new("KXTP".to_string());
        room.games = std::slice::from_ref(&DUMMY_INFO);
        let mut phones = Vec::new();
        for nickname in nicknames {
            let (connection, messages) = mpsc::unbounded_channel();
            room.add_player(nickname.to_string(), None, connection);
            phones.push(messages);
        }
        (room, phones)
    }

    fn got_vote_state(messages: &mut UnboundedReceiver<ServerMessage>) -> bool {
        // Skip lobby updates
        while let Ok(message) = messages.try_recv() {
            if matches!(message, ServerMessage::VoteState { .. }) {
                return true;
            }
        }
        false
    }

    #[test]
    fn vote_starts_when_everyone_is_ready() {
        let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
        room.set_ready(1, true);
        room.set_ready(2, true);
        room.start_vote_if_ready();
        assert!(
            matches!(room.state, RoomState::Lobby),
            "Kim isn't ready yet"
        );

        room.set_ready(3, true);
        room.start_vote_if_ready();
        assert!(matches!(room.state, RoomState::Voting { .. }));
        for phone in &mut phones {
            assert!(got_vote_state(phone));
        }
    }

    #[test]
    fn two_players_are_not_enough() {
        let (mut room, _phones) = room_with(&["Alex", "Sam"]);
        room.set_ready(1, true);
        room.set_ready(2, true);
        room.start_vote_if_ready();
        assert!(matches!(room.state, RoomState::Lobby));
    }

    #[test]
    fn disconnected_player_does_not_hold_up_the_vote() {
        let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
        room.player_mut(4)
            .expect("Lee is in the room")
            .disconnected_since = Some(Instant::now());
        for player_id in 1..=3 {
            room.set_ready(player_id, true);
        }
        room.start_vote_if_ready();
        assert!(matches!(room.state, RoomState::Voting { .. }));
    }

    // The votes in the newest vote_state this phone got
    fn latest_votes(
        messages: &mut UnboundedReceiver<ServerMessage>,
    ) -> Option<BTreeMap<&'static str, Vec<u32>>> {
        let mut latest = None;
        while let Ok(message) = messages.try_recv() {
            if let ServerMessage::VoteState { votes, .. } = message {
                latest = Some(votes);
            }
        }
        latest
    }

    fn voting_room(nicknames: &[&str]) -> (Room, Vec<UnboundedReceiver<ServerMessage>>) {
        let (mut room, phones) = room_with(nicknames);
        room.state = RoomState::Voting {
            votes: HashMap::new(),
        };
        (room, phones)
    }

    #[test]
    fn votes_are_shown_to_everyone() {
        let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
        room.vote(3, "dummy");
        room.vote(1, "dummy");

        for phone in &mut phones {
            let votes = latest_votes(phone).expect("a vote_state");
            assert_eq!(votes, BTreeMap::from([("dummy", vec![1, 3])]));
        }
    }

    #[test]
    fn wrong_votes_are_refused() {
        let (mut room, mut phones) = voting_room(&["Alex", "Sam"]);
        room.vote(1, "chess");
        assert_eq!(error_code(phones[0].try_recv().ok()), "unknown_game");
        // The dummy game needs 3 players
        room.vote(1, "dummy");
        assert_eq!(error_code(phones[0].try_recv().ok()), "game_not_available");

        let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
        room.vote(1, "dummy");
        assert_eq!(error_code(phones[0].try_recv().ok()), "not_voting");
    }

    #[test]
    fn leaving_takes_back_the_vote() {
        let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim", "Lee"]);
        room.vote(4, "dummy");
        room.leave(4);

        let votes = latest_votes(&mut phones[0]).expect("a vote_state");
        assert!(votes.is_empty());
    }

    #[test]
    fn joining_during_the_vote_shows_the_vote() {
        let (mut room, _phones) = voting_room(&["Alex", "Sam", "Kim"]);
        room.vote(1, "dummy");

        let (connection, mut messages) = mpsc::unbounded_channel();
        let lee = room.join("Lee".to_string(), None, None, connection);
        assert!(lee.is_some());
        let votes = latest_votes(&mut messages).expect("Lee gets the vote");
        assert_eq!(votes, BTreeMap::from([("dummy", vec![1])]));
    }

    // Two games that both fit 3 to 10 players
    static TWO_GAMES: [GameInfo; 2] = [
        GameInfo {
            id: "first",
            name: "First",
            description: "A game for tests",
            min_players: 3,
            max_players: 10,
            create: crate::games::dummy::create,
        },
        GameInfo {
            id: "second",
            name: "Second",
            description: "Another game for tests",
            min_players: 3,
            max_players: 10,
            create: crate::games::dummy::create,
        },
    ];

    #[test]
    fn changing_a_vote_moves_it() {
        let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
        room.games = &TWO_GAMES;
        room.vote(1, "first");
        room.vote(2, "first");
        room.vote(1, "second");

        let votes = latest_votes(&mut phones[2]).expect("a vote_state");
        assert_eq!(
            votes,
            BTreeMap::from([("first", vec![2]), ("second", vec![1])])
        );
    }

    #[test]
    fn votes_are_refused_once_voting_ends() {
        let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
        room.vote(1, "dummy");
        latest_votes(&mut phones[0]);

        room.state = RoomState::Lobby;
        room.vote(1, "dummy");
        assert_eq!(error_code(phones[0].try_recv().ok()), "not_voting");
    }

    #[test]
    fn ready_only_changes_in_the_lobby() {
        let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
        room.state = RoomState::Voting {
            votes: HashMap::new(),
        };
        room.set_ready(1, true);
        assert_eq!(error_code(phones[0].try_recv().ok()), "not_in_lobby");
    }

    #[tokio::test]
    async fn ready_players_get_the_vote_through_the_room_task() {
        let (room, _rooms) = spawn_room();
        let mut phones = Vec::new();
        for nickname in ["Alex", "Sam", "Kim"] {
            let (player_id, messages) = join(&room, nickname, None).await;
            phones.push(messages);
            room.send(RoomCommand::Ready {
                player_id: player_id.unwrap(),
                ready: true,
            })
            .await;
        }
        // Ask something so every command above has been handled
        player_count(&room).await;
        for phone in &mut phones {
            assert!(got_vote_state(phone));
        }
    }

    #[test]
    fn game_messages_reach_the_right_players() {
        let mut room = Room::new("KXTP".to_string());
        let (alex_connection, mut alex_messages) = mpsc::unbounded_channel();
        let (sam_connection, mut sam_messages) = mpsc::unbounded_channel();
        let alex = room
            .add_player("Alex".to_string(), None, alex_connection)
            .id;
        room.add_player("Sam".to_string(), None, sam_connection);

        let mut ctx = GameCtx::default();
        ctx.send_to(alex, json!({ "hand": [12] }));
        ctx.broadcast(json!({ "round": 1 }));
        room.send_game_messages(ctx.outgoing);

        let event = |message| match message {
            Ok(ServerMessage::GameEvent { event }) => event,
            _ => panic!("expected a game event"),
        };
        assert_eq!(event(alex_messages.try_recv()), json!({ "hand": [12] }));
        assert_eq!(event(alex_messages.try_recv()), json!({ "round": 1 }));
        assert_eq!(event(sam_messages.try_recv()), json!({ "round": 1 }));
        assert!(
            sam_messages.try_recv().is_err(),
            "Sam doesn't get Alex's hand"
        );
    }

    #[test]
    fn points_to_win_only_change_in_the_lobby() {
        let mut room = Room::new("KXTP".to_string());
        let (connection, mut messages) = mpsc::unbounded_channel();
        let alex = room.add_player("Alex".to_string(), None, connection).id;

        room.state = RoomState::Voting {
            votes: HashMap::new(),
        };
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
