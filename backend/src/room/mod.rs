use rand::RngExt;

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

// Room code letters, without I and O
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_LENGTH: usize = 4;

// Random room code like "KXTP"
pub fn generate_code() -> String {
    let mut rng = rand::rng();
    let mut code = String::new();
    for _ in 0..CODE_LENGTH {
        let index = rng.random_range(0..CODE_LETTERS.len());
        code.push(CODE_LETTERS[index] as char);
    }
    code
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

    // Codes are random, so check many of them
    #[test]
    fn code_has_four_uppercase_letters() {
        for _ in 0..1000 {
            let code = generate_code();
            assert_eq!(code.len(), 4, "wrong length: {code}");
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase()),
                "not uppercase: {code}"
            );
        }
    }

    #[test]
    fn code_skips_i_and_o() {
        for _ in 0..1000 {
            let code = generate_code();
            assert!(!code.contains('I'), "contains I: {code}");
            assert!(!code.contains('O'), "contains O: {code}");
        }
    }
}
