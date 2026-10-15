pub mod code;
pub mod player;

use tokio::sync::mpsc::UnboundedSender;

use crate::protocol::{LobbyPlayer, ServerMessage};
use player::{Player, generate_token};

const NICKNAME_MAX_LENGTH: usize = 16;

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
    next_player_id: u32,
}

impl Room {
    pub fn new(code: String) -> Room {
        Room {
            code,
            players: Vec::new(),
            host: None,
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
            token: generate_token(),
            sender,
        });
        self.players.last().expect("player was just added")
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
            })
            .collect();
        ServerMessage::Lobby {
            players,
            host: self.host,
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
