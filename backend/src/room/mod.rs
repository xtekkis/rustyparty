pub mod code;

// A player in a room
#[derive(Debug)]
pub struct Player {
    pub id: u32,
    pub nickname: String,
}

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
#[derive(Debug)]
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

    // Add a player and return their new id
    pub fn add_player(&mut self, nickname: String) -> u32 {
        let id = self.next_player_id;
        self.next_player_id += 1;
        self.players.push(Player { id, nickname });
        id
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
        assert_eq!(room.add_player("Alex".to_string()), 1);
        assert_eq!(room.add_player("Sam".to_string()), 2);
        assert_eq!(room.players.len(), 2);
        assert_eq!(room.players[1].nickname, "Sam");
    }
}
