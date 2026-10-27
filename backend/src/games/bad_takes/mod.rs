// Bad Takes: an adult fill-in-the-blank card game
pub mod deck;

use serde_json::{Value, json};

use super::{Game, GameCtx, GameError, GameInfo, PlayerId, TimerId};
use crate::protocol::RoomSettings;

pub const INFO: GameInfo = GameInfo {
    id: "bad_takes",
    name: "Bad Takes",
    description: "Fill in the blank with your funniest card. The judge picks the winner.",
    min_players: 3,
    max_players: 10,
    create,
};

fn create(settings: &RoomSettings) -> Box<dyn Game> {
    Box::new(BadTakes::new(settings.points_to_win))
}

pub struct BadTakes {
    points_to_win: u8,
    players: Vec<PlayerId>,
}

impl BadTakes {
    pub fn new(points_to_win: u8) -> BadTakes {
        BadTakes {
            points_to_win,
            players: Vec::new(),
        }
    }
}

impl Game for BadTakes {
    fn start(&mut self, players: &[PlayerId], _ctx: &mut GameCtx) {
        self.players = players.to_vec();
    }

    fn handle_action(
        &mut self,
        _player: PlayerId,
        _action: Value,
        _ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        Err(GameError {
            code: "unknown_action",
            message: "That action doesn't exist in this game",
        })
    }

    fn handle_timer(&mut self, _timer: TimerId, _ctx: &mut GameCtx) {}

    fn player_joined(&mut self, _player: PlayerId, _ctx: &mut GameCtx) {}

    fn player_left(&mut self, player: PlayerId, _ctx: &mut GameCtx) {
        self.players.retain(|&id| id != player);
    }

    fn view_for(&self, _player: PlayerId) -> Value {
        json!({ "points_to_win": self.points_to_win })
    }

    fn is_finished(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{GAMES, find_game};

    #[test]
    fn bad_takes_is_in_the_registry() {
        let info = find_game(GAMES, "bad_takes").expect("Bad Takes is registered");
        assert_eq!(info.name, "Bad Takes");
        assert!(!info.fits(2));
        assert!(info.fits(3));
        assert!(info.fits(10));
        assert!(!info.fits(11));
    }

    #[test]
    fn registry_makes_a_game_with_the_room_settings() {
        let mut game = (INFO.create)(&RoomSettings { points_to_win: 5 });
        game.start(&[1, 2, 3], &mut GameCtx::default());
        assert_eq!(game.view_for(1), json!({ "points_to_win": 5 }));
        assert!(!game.is_finished());
    }
}
