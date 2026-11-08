pub mod actor;
pub mod code;
pub mod player;
pub mod vote;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::games::{GAMES, Game, GameInfo};
use crate::protocol::{LobbyPlayer, RoomSettings, ServerMessage, VoteOption};
use player::{Player, generate_token};

const NICKNAME_MAX_LENGTH: usize = 16;
// The most any game allows, so a room can't be flooded with extra players
pub const MAX_ROOM_PLAYERS: usize = 10;
// How long a disconnected player keeps their seat
pub const DISCONNECT_GRACE: Duration = Duration::from_secs(90);

const POINTS_TO_WIN_DEFAULT: u8 = 7;
const POINTS_TO_WIN_MIN: u8 = 3;
const POINTS_TO_WIN_MAX: u8 = 15;

pub fn is_valid_points_to_win(points: u8) -> bool {
    (POINTS_TO_WIN_MIN..=POINTS_TO_WIN_MAX).contains(&points)
}

// Trim spaces, then check the nickname is 1 to 16 characters
pub fn clean_nickname(nickname: &str) -> Option<String> {
    let nickname = nickname.trim();
    let length = nickname.chars().count();
    if (1..=NICKNAME_MAX_LENGTH).contains(&length) {
        Some(nickname.to_string())
    } else {
        None
    }
}

// Seconds until a moment, rounded up so half a second left still shows 1
pub fn seconds_until(moment: Instant) -> u64 {
    let left = moment.saturating_duration_since(Instant::now());
    left.as_secs() + u64::from(left.subsec_nanos() > 0)
}

// Where the room is in its cycle: Lobby, Voting, InGame, Results, then back to Lobby
pub enum RoomState {
    Lobby,
    Voting {
        // Player id to the id of the game they picked
        votes: HashMap<u32, &'static str>,
        // Counts up with every vote, so an old time limit can't end a new vote
        number: u32,
        ends_at: Instant,
    },
    InGame {
        game: Box<dyn Game>,
        // Counts up with every game, so a timer from an old game can't reach a new one
        number: u32,
        // When the timer the game started last runs out
        ends_at: Option<Instant>,
    },
    Results {
        // The game that just ended, so an old timer can't end a newer results screen
        number: u32,
    },
}

// A group of players sharing a room code
pub struct Room {
    pub code: String,
    pub state: RoomState,
    pub players: Vec<Player>,
    // Player id of the host, none while the room is empty
    pub host: Option<u32>,
    pub settings: RoomSettings,
    // Games this room can vote for
    pub games: &'static [GameInfo],
    // Tokens of banned players, they can't join this room again
    banned_tokens: HashSet<String>,
    // When the room last had nobody connected, none while someone is
    empty_since: Option<Instant>,
    next_player_id: u32,
    // How many votes and games this room has started, used to number them
    votes_started: u32,
    games_started: u32,
}

impl Room {
    pub fn new(code: String) -> Room {
        Room {
            code,
            state: RoomState::Lobby,
            players: Vec::new(),
            host: None,
            games: &GAMES,
            settings: RoomSettings {
                points_to_win: POINTS_TO_WIN_DEFAULT,
            },
            banned_tokens: HashSet::new(),
            empty_since: None,
            next_player_id: 1,
            votes_started: 0,
            games_started: 0,
        }
    }

    // Nicknames are unique per room, ignoring upper and lower case
    pub fn has_nickname(&self, nickname: &str) -> bool {
        let nickname = nickname.to_lowercase();
        self.players
            .iter()
            .any(|player| player.nickname.to_lowercase() == nickname)
    }

    // Add a player and return them
    pub fn add_player(
        &mut self,
        nickname: String,
        avatar: Option<String>,
        sender: UnboundedSender<ServerMessage>,
    ) -> &Player {
        let id = self.next_player_id;
        self.next_player_id += 1;

        // The first player to join is the host
        if self.host.is_none() {
            self.host = Some(id);
        }

        self.players.push(Player {
            id,
            nickname,
            avatar,
            ready: false,
            disconnected_since: None,
            token: generate_token(),
            sender,
        });
        self.players.last().expect("player was just added")
    }

    // Find a player by id to change them
    pub fn player_mut(&mut self, player_id: u32) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| player.id == player_id)
    }

    // Take a player out of the room, false if they weren't in it
    pub fn remove_player(&mut self, player_id: u32) -> bool {
        let count_before = self.players.len();
        self.players.retain(|player| player.id != player_id);

        // Someone who left doesn't get a say
        if let RoomState::Voting { votes, .. } = &mut self.state {
            votes.remove(&player_id);
        }

        // Players are kept in join order, so the first one left joined earliest
        if self.host == Some(player_id) {
            self.host = self.players.first().map(|player| player.id);
        }

        self.players.len() < count_before
    }

    // Give a returning player their seat back, with messages going to their new connection
    pub fn reconnect(
        &mut self,
        token: &str,
        sender: UnboundedSender<ServerMessage>,
    ) -> Option<u32> {
        let player = self
            .players
            .iter_mut()
            .find(|player| player.token == token)?;
        player.disconnected_since = None;

        // The newest connection wins, an older one still open is told and closes
        let old_sender = std::mem::replace(&mut player.sender, sender);
        let _ = old_sender.send(ServerMessage::Replaced);

        Some(player.id)
    }

    // Remove a player whose grace period ran out, unless they came back since
    pub fn remove_if_gone_since(&mut self, player_id: u32, since: Instant) -> bool {
        let still_gone = self
            .players
            .iter()
            .any(|player| player.id == player_id && player.disconnected_since == Some(since));
        still_gone && self.remove_player(player_id)
    }

    // Remove a player, they can rejoin
    pub fn kick_player(&mut self, player_id: u32) -> bool {
        self.send_and_remove(player_id, ServerMessage::Kicked)
            .is_some()
    }

    // Remove a player and block their token from rejoining
    pub fn ban_player(&mut self, player_id: u32) -> bool {
        match self.send_and_remove(player_id, ServerMessage::Banned) {
            Some(token) => {
                self.banned_tokens.insert(token);
                true
            }
            None => false,
        }
    }

    pub fn is_banned(&self, token: &str) -> bool {
        self.banned_tokens.contains(token)
    }

    // Tell a player why they're removed, then remove them and return their token
    fn send_and_remove(&mut self, player_id: u32, message: ServerMessage) -> Option<String> {
        let player = self.players.iter().find(|player| player.id == player_id)?;
        // Their connection closes when this arrives
        let _ = player.sender.send(message);
        let token = player.token.clone();
        self.remove_player(player_id);
        Some(token)
    }

    // The games on offer, as everyone sees them
    pub fn vote_message(&self) -> ServerMessage {
        // Everyone with a seat counts, players in their grace period will be in the game too
        let player_count = self.players.len();
        let options = self
            .games
            .iter()
            .map(|game| VoteOption {
                id: game.id,
                name: game.name,
                description: game.description,
                min_players: game.min_players,
                max_players: game.max_players,
                available: game.fits(player_count),
            })
            .collect();

        let mut votes: BTreeMap<&'static str, Vec<u32>> = BTreeMap::new();
        let mut ends_in = 0;
        if let RoomState::Voting {
            votes: picks,
            ends_at,
            ..
        } = &self.state
        {
            for (&player_id, &game_id) in picks {
                votes.entry(game_id).or_default().push(player_id);
            }
            ends_in = seconds_until(*ends_at);
        }
        // Same order every time, so the phones don't reshuffle the bubbles
        for voters in votes.values_mut() {
            voters.sort();
        }

        ServerMessage::VoteState {
            options,
            votes,
            ends_in,
        }
    }

    // Tell everyone who's in the room, and the vote too while one is running
    pub fn broadcast_players(&self) {
        self.broadcast(&self.lobby_message());
        if matches!(self.state, RoomState::Voting { .. }) {
            self.broadcast(&self.vote_message());
        }
    }

    // The lobby as everyone sees it
    pub fn lobby_message(&self) -> ServerMessage {
        let players = self
            .players
            .iter()
            .map(|player| LobbyPlayer {
                id: player.id,
                nickname: player.nickname.clone(),
                avatar: player.avatar.clone(),
                ready: player.ready,
                connected: player.disconnected_since.is_none(),
                reconnect_in: player
                    .disconnected_since
                    .map(|since| seconds_until(since + DISCONNECT_GRACE)),
            })
            .collect();
        ServerMessage::Lobby {
            players,
            host: self.host,
            settings: self.settings,
        }
    }

    // Send a message to every player in the room
    pub fn broadcast(&self, message: &ServerMessage) {
        for player in &self.players {
            // A closed connection can't receive, so ignore the error
            let _ = player.sender.send(message.clone());
        }
    }
}

#[cfg(test)]
mod tests;
