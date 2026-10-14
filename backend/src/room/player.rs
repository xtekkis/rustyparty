use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

// A player in a room
// No Debug on purpose, so the token can't end up in logs
pub struct Player {
    pub id: u32,
    pub nickname: String,
    pub token: String,
}

// Secret that lets a player reconnect to their seat
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_43_characters() {
        // 32 bytes in base64 without padding
        assert_eq!(generate_token().len(), 43);
    }

    #[test]
    fn tokens_are_different() {
        assert_ne!(generate_token(), generate_token());
    }
}
