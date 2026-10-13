pub mod code;

// A player in a room
#[derive(Debug)]
pub struct Player {
    pub id: u32,
    pub nickname: String,
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
