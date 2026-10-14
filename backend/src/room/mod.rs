pub mod code;
pub mod player;

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
    next_player_id: u32,
}

impl Room {
    pub fn new(code: String) -> Room {
        Room {
            code,
            players: Vec::new(),
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
    pub fn add_player(&mut self, nickname: String) -> &Player {
        let id = self.next_player_id;
        self.next_player_id += 1;
        self.players.push(Player {
            id,
            nickname,
            token: generate_token(),
        });
        self.players.last().expect("player was just added")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_room_has_code_and_no_players() {
        let room = Room::new("KXTP".to_string());
        assert_eq!(room.code, "KXTP");
        assert!(room.players.is_empty());
    }

    #[test]
    fn players_get_increasing_ids() {
        let mut room = Room::new("KXTP".to_string());
        assert_eq!(room.add_player("Alex".to_string()).id, 1);
        assert_eq!(room.add_player("Sam".to_string()).id, 2);
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
        room.add_player("Alex".to_string());
        assert!(room.has_nickname("Alex"));
        assert!(room.has_nickname("aLEX"));
        assert!(!room.has_nickname("Sam"));
    }
}
