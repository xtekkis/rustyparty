// Bad Takes: an adult fill-in-the-blank card game
pub mod deck;

use serde_json::{Value, json};

use super::{Game, GameCtx, GameError, GameInfo, PlayerId, TimerId};
use crate::protocol::RoomSettings;

pub const HAND_SIZE: usize = 7;
const MIN_PLAYERS: usize = 3;
const MAX_PLAYERS: usize = 10;

// The registry entry, built at startup because the player limit depends on the deck
pub fn info() -> GameInfo {
    GameInfo {
        id: "bad_takes",
        name: "Bad Takes",
        description: "Fill in the blank with your funniest card. The judge picks the winner.",
        min_players: MIN_PLAYERS,
        max_players: max_players(deck::starter_deck().answers.len()),
        create,
    }
}

// Half the answers stay in the draw pile, so hands get fresh cards instead of the ones just played
fn max_players(answer_count: usize) -> usize {
    (answer_count / (HAND_SIZE * 2)).min(MAX_PLAYERS)
}

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
    fn max_players_keeps_half_the_answers_in_reserve() {
        assert_eq!(max_players(70), 5);
        assert_eq!(max_players(83), 5);
        assert_eq!(max_players(84), 6);
        assert_eq!(max_players(140), 10);
        // Never more than 10, however big the deck
        assert_eq!(max_players(500), 10);
    }

    #[test]
    fn bad_takes_is_in_the_registry() {
        let info = find_game(&GAMES, "bad_takes").expect("Bad Takes is registered");
        assert_eq!(info.name, "Bad Takes");
        // The 140 answer starter deck allows 3 to 10 players
        assert!(!info.fits(2));
        assert!(info.fits(3));
        assert!(info.fits(10));
        assert!(!info.fits(11));
    }

    #[test]
    fn registry_makes_a_game_with_the_room_settings() {
        let mut game = (info().create)(&RoomSettings { points_to_win: 5 });
        game.start(&[1, 2, 3], &mut GameCtx::default());
        assert_eq!(game.view_for(1), json!({ "points_to_win": 5 }));
        assert!(!game.is_finished());
    }
}
