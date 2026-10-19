use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

use crate::protocol::ServerMessage;

// A player in a room
// No Debug on purpose, so the token can't end up in logs
pub struct Player {
    pub id: u32,
    pub nickname: String,
    // Avatar id, none means show initials
    pub avatar: Option<String>,
    // Ready to start the game vote
    pub ready: bool,
    // When their phone disconnected, none while connected. The seat is kept for a while
    pub disconnected_since: Option<Instant>,
    pub token: String,
    // Messages for this player's connection
    pub sender: UnboundedSender<ServerMessage>,
}

// Avatar ids, matching the image names in frontend/public/avatars
pub const AVATARS: &[&str] = &["fox", "cat", "owl", "bear", "frog"];

// No avatar is fine, it means initials
pub fn is_known_avatar(avatar: Option<&str>) -> bool {
    avatar.is_none_or(|id| AVATARS.contains(&id))
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
    fn known_avatars_and_initials_are_allowed() {
        assert!(is_known_avatar(Some("fox")));
        assert!(is_known_avatar(None));
    }

    #[test]
    fn unknown_avatar_is_rejected() {
        assert!(!is_known_avatar(Some("dragon")));
        assert!(!is_known_avatar(Some("")));
    }

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
