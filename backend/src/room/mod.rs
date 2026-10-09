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
}

impl Room {
    pub fn new(code: String) -> Room {
        Room {
            code,
            players: Vec::new(),
        }
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
}
