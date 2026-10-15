use serde::{Deserialize, Serialize};

// Messages from a player's phone to the server
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join {
        room: String,
        nickname: String,
        avatar: Option<String>,
        token: Option<String>,
    },
    SetAvatar {
        avatar: Option<String>,
    },
    Ready {
        ready: bool,
    },
    Vote {
        game: String,
    },
    RoomSettings {
        points_to_win: u8,
    },
    GameAction {
        action: serde_json::Value,
    },
    Kick {
        player_id: u32,
    },
    Leave,
}

// A player as everyone in the lobby sees them
#[derive(Debug, Clone, Serialize)]
pub struct LobbyPlayer {
    pub id: u32,
    pub nickname: String,
    pub avatar: Option<String>,
}

// Messages from the server to a player's phone
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
        token: String,
    },
    Lobby {
        players: Vec<LobbyPlayer>,
        host: Option<u32>,
    },
    VoteResult {
        game: String,
        tie_break: bool,
    },
    GameView {
        view: serde_json::Value,
    },
    Error {
        code: String,
        message: String,
    },
    Replaced,
    Kicked,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_join() {
        let text = r#"{ "type": "join", "room": "KXTP", "nickname": "Alex", "avatar": "fox", "token": null }"#;
        let message: ClientMessage = serde_json::from_str(text).unwrap();

        match message {
            ClientMessage::Join {
                room,
                nickname,
                avatar,
                token,
            } => {
                assert_eq!(room, "KXTP");
                assert_eq!(nickname, "Alex");
                assert_eq!(avatar.as_deref(), Some("fox"));
                assert_eq!(token, None);
            }
            other => panic!("expected join, got {other:?}"),
        }
    }

    #[test]
    fn parses_message_without_fields() {
        let message: ClientMessage = serde_json::from_str(r#"{ "type": "leave" }"#).unwrap();
        assert!(matches!(message, ClientMessage::Leave));
    }

    #[test]
    fn rejects_unknown_type() {
        let result = serde_json::from_str::<ClientMessage>(r#"{ "type": "dance" }"#);
        assert!(result.is_err());
    }

    #[test]
    fn writes_error() {
        let message = ServerMessage::Error {
            code: "nickname_taken".to_string(),
            message: "That nickname is taken".to_string(),
        };
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(
            value,
            json!({ "type": "error", "code": "nickname_taken", "message": "That nickname is taken" })
        );
    }

    #[test]
    fn writes_message_without_fields() {
        let value = serde_json::to_value(&ServerMessage::Kicked).unwrap();
        assert_eq!(value, json!({ "type": "kicked" }));
    }
}
