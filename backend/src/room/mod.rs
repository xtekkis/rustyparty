pub mod code;
pub mod player;

use std::collections::HashSet;

use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::protocol::{LobbyPlayer, RoomSettings, ServerMessage};
use player::{Player, generate_token};

const NICKNAME_MAX_LENGTH: usize = 16;

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

// A group of players sharing a room code
pub struct Room {
    pub code: String,
    pub players: Vec<Player>,
    // Player id of the host, none while the room is empty
    pub host: Option<u32>,
    pub settings: RoomSettings,
    // Tokens of banned players, they can't join this room again
    banned_tokens: HashSet<String>,
    next_player_id: u32,
}

impl Room {
    pub fn new(code: String) -> Room {
        Room {
            code,
            players: Vec::new(),
            host: None,
            settings: RoomSettings {
                points_to_win: POINTS_TO_WIN_DEFAULT,
            },
            banned_tokens: HashSet::new(),
            next_player_id: 1,
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
        player.sender = sender;
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
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    // Messages sent to it are dropped, tests here don't read them
    fn test_sender() -> UnboundedSender<ServerMessage> {
        mpsc::unbounded_channel().0
    }

    #[test]
    fn new_room_has_code_and_no_players() {
        let room = Room::new("KXTP".to_string());
        assert_eq!(room.code, "KXTP");
        assert!(room.players.is_empty());
    }

    #[test]
    fn players_get_increasing_ids() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
        assert_eq!((alex, sam), (1, 2));
        assert_eq!(room.players.len(), 2);
        assert_eq!(room.players[1].nickname, "Sam");
    }

    #[test]
    fn nickname_is_trimmed() {
        assert_eq!(clean_nickname("  Alex  "), Some("Alex".to_string()));
    }

    #[test]
    fn empty_nickname_is_rejected() {
        assert_eq!(clean_nickname(""), None);
        assert_eq!(clean_nickname("    "), None);
    }

    #[test]
    fn nickname_length_limits() {
        assert_eq!(clean_nickname("A"), Some("A".to_string()));
        assert!(clean_nickname(&"a".repeat(16)).is_some());
        assert_eq!(clean_nickname(&"a".repeat(17)), None);
    }

    #[test]
    fn nickname_length_counts_letters_not_bytes() {
        // 16 Greek letters take 32 bytes but are still allowed
        assert!(clean_nickname(&"λ".repeat(16)).is_some());
        assert_eq!(clean_nickname(&"λ".repeat(17)), None);
    }

    #[test]
    fn duplicate_nickname_ignores_case() {
        let mut room = Room::new("KXTP".to_string());
        room.add_player("Alex".to_string(), None, test_sender());
        assert!(room.has_nickname("Alex"));
        assert!(room.has_nickname("aLEX"));
        assert!(!room.has_nickname("Sam"));
    }

    #[test]
    fn first_player_is_host() {
        let mut room = Room::new("KXTP".to_string());
        assert_eq!(room.host, None);
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        room.add_player("Sam".to_string(), None, test_sender());
        assert_eq!(room.host, Some(alex));
    }

    #[test]
    fn find_player_by_id() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        assert_eq!(room.player_mut(alex).map(|player| player.id), Some(alex));
        assert!(room.player_mut(99).is_none());
    }

    #[test]
    fn new_player_is_not_ready() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender());
        assert!(!alex.ready);
    }

    #[test]
    fn lobby_shows_ready_state() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        room.add_player("Sam".to_string(), None, test_sender());
        room.player_mut(alex).expect("Alex is in the room").ready = true;

        let ServerMessage::Lobby { players, .. } = room.lobby_message() else {
            panic!("expected a lobby message");
        };
        let ready: Vec<bool> = players.iter().map(|player| player.ready).collect();
        assert_eq!(ready, [true, false]);
    }

    #[test]
    fn points_to_win_starts_at_7() {
        let room = Room::new("KXTP".to_string());
        assert_eq!(room.settings.points_to_win, 7);
    }

    #[test]
    fn points_to_win_limits() {
        assert!(!is_valid_points_to_win(2));
        assert!(is_valid_points_to_win(3));
        assert!(is_valid_points_to_win(15));
        assert!(!is_valid_points_to_win(16));
    }

    #[test]
    fn player_can_be_removed() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        room.add_player("Sam".to_string(), None, test_sender());

        assert!(room.remove_player(alex));
        assert_eq!(room.players.len(), 1);
        assert!(!room.has_nickname("Alex"));
        assert!(!room.remove_player(alex));
    }

    #[test]
    fn host_passes_to_next_earliest_player() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
        let kim = room.add_player("Kim".to_string(), None, test_sender()).id;

        room.remove_player(kim);
        assert_eq!(room.host, Some(alex), "a guest leaving keeps the host");

        room.remove_player(alex);
        assert_eq!(room.host, Some(sam));

        room.remove_player(sam);
        assert_eq!(room.host, None, "an empty room has no host");
    }

    #[test]
    fn kicked_player_is_told_and_can_rejoin() {
        let mut room = Room::new("KXTP".to_string());
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let sam = room.add_player("Sam".to_string(), None, sender);
        let (sam_id, sam_token) = (sam.id, sam.token.clone());

        assert!(room.kick_player(sam_id));
        assert!(matches!(receiver.try_recv(), Ok(ServerMessage::Kicked)));
        assert!(room.players.is_empty());
        assert!(!room.is_banned(&sam_token));
        assert!(!room.kick_player(sam_id));
    }

    #[test]
    fn banned_player_is_told_and_blocked() {
        let mut room = Room::new("KXTP".to_string());
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let sam = room.add_player("Sam".to_string(), None, sender);
        let (sam_id, sam_token) = (sam.id, sam.token.clone());

        assert!(room.ban_player(sam_id));
        assert!(matches!(receiver.try_recv(), Ok(ServerMessage::Banned)));
        assert!(room.players.is_empty());
        assert!(room.is_banned(&sam_token));
        assert!(!room.is_banned("some-other-token"));
        assert!(!room.ban_player(sam_id));
    }

    #[test]
    fn lobby_shows_disconnected_players() {
        let mut room = Room::new("KXTP".to_string());
        let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
        room.add_player("Sam".to_string(), None, test_sender());
        room.player_mut(alex)
            .expect("Alex is in the room")
            .disconnected_since = Some(Instant::now());

        let ServerMessage::Lobby { players, .. } = room.lobby_message() else {
            panic!("expected a lobby message");
        };
        let connected: Vec<bool> = players.iter().map(|player| player.connected).collect();
        assert_eq!(connected, [false, true]);
    }

    #[test]
    fn reconnect_restores_the_seat() {
        let mut room = Room::new("KXTP".to_string());
        let sam = room.add_player("Sam".to_string(), Some("owl".to_string()), test_sender());
        let (sam_id, sam_token) = (sam.id, sam.token.clone());
        room.player_mut(sam_id)
            .expect("Sam is in the room")
            .disconnected_since = Some(Instant::now());

        let (new_sender, mut new_receiver) = mpsc::unbounded_channel();
        assert_eq!(room.reconnect(&sam_token, new_sender), Some(sam_id));

        let sam = &room.players[0];
        assert!(sam.disconnected_since.is_none());
        assert_eq!(sam.avatar.as_deref(), Some("owl"));
        room.broadcast(&room.lobby_message());
        assert!(
            new_receiver.try_recv().is_ok(),
            "messages reach the new connection"
        );
    }

    #[test]
    fn unknown_token_does_not_reconnect() {
        let mut room = Room::new("KXTP".to_string());
        room.add_player("Sam".to_string(), None, test_sender());
        assert_eq!(room.reconnect("not-a-real-token", test_sender()), None);
    }

    // Disconnect a player now and return when
    fn disconnect(room: &mut Room, player_id: u32) -> Instant {
        let now = Instant::now();
        room.player_mut(player_id)
            .expect("player is in the room")
            .disconnected_since = Some(now);
        now
    }

    #[test]
    fn player_still_gone_is_removed() {
        let mut room = Room::new("KXTP".to_string());
        let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
        let since = disconnect(&mut room, sam);

        assert!(room.remove_if_gone_since(sam, since));
        assert!(room.players.is_empty());
    }

    #[test]
    fn player_who_came_back_is_kept() {
        let mut room = Room::new("KXTP".to_string());
        let sam = room.add_player("Sam".to_string(), None, test_sender());
        let (sam_id, sam_token) = (sam.id, sam.token.clone());
        let since = disconnect(&mut room, sam_id);
        room.reconnect(&sam_token, test_sender());

        assert!(!room.remove_if_gone_since(sam_id, since));
        assert_eq!(room.players.len(), 1);
    }

    #[test]
    fn old_timer_ignores_a_newer_disconnect() {
        let mut room = Room::new("KXTP".to_string());
        let sam = room.add_player("Sam".to_string(), None, test_sender());
        let (sam_id, sam_token) = (sam.id, sam.token.clone());
        let first = disconnect(&mut room, sam_id);
        room.reconnect(&sam_token, test_sender());
        // Set a clearly later time, two calls to now() can be equal
        let second = first + Duration::from_secs(1);
        room.player_mut(sam_id)
            .expect("Sam is in the room")
            .disconnected_since = Some(second);

        assert!(!room.remove_if_gone_since(sam_id, first));
        assert!(room.remove_if_gone_since(sam_id, second));
    }

    #[test]
    fn broadcast_reaches_every_player() {
        let mut room = Room::new("KXTP".to_string());
        let (alex_sender, mut alex_receiver) = mpsc::unbounded_channel();
        let (sam_sender, mut sam_receiver) = mpsc::unbounded_channel();
        room.add_player("Alex".to_string(), Some("fox".to_string()), alex_sender);
        room.add_player("Sam".to_string(), None, sam_sender);

        room.broadcast(&room.lobby_message());

        for receiver in [&mut alex_receiver, &mut sam_receiver] {
            match receiver.try_recv() {
                Ok(ServerMessage::Lobby { players, .. }) => {
                    let names: Vec<&str> = players.iter().map(|p| p.nickname.as_str()).collect();
                    assert_eq!(names, ["Alex", "Sam"]);
                    assert_eq!(players[0].avatar.as_deref(), Some("fox"));
                    assert_eq!(players[1].avatar, None);
                }
                _ => panic!("expected a lobby message"),
            }
        }
    }
}
