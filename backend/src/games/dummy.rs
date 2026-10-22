use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Value, json};

use super::{Game, GameCtx, GameError, GameInfo, PlayerId, TimerId};
use crate::protocol::RoomSettings;

// First to press this many times wins
const PRESSES_TO_WIN: u32 = 3;

const TIME_LIMIT_TIMER: TimerId = 1;
const TIME_LIMIT: Duration = Duration::from_secs(30);

pub static DUMMY_INFO: GameInfo = GameInfo {
    id: "dummy",
    name: "Dummy",
    description: "Press the button, first to 3 wins",
    min_players: 3,
    max_players: 10,
    create,
};

pub fn create(_settings: &RoomSettings) -> Box<dyn Game> {
    Box::new(DummyGame::default())
}

// A tiny game that uses every part of the engine, for tests
#[derive(Default)]
pub struct DummyGame {
    presses: HashMap<PlayerId, u32>,
    finished: bool,
}

impl Game for DummyGame {
    fn start(&mut self, players: &[PlayerId], ctx: &mut GameCtx) {
        for &player in players {
            self.presses.insert(player, 0);
        }
        ctx.broadcast(json!({ "event": "started" }));
        ctx.start_timer(TIME_LIMIT_TIMER, TIME_LIMIT);
    }

    fn handle_action(
        &mut self,
        player: PlayerId,
        action: Value,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        if self.finished {
            return Err(GameError {
                code: "game_over",
                message: "The game is over",
            });
        }
        if action["kind"] != "press" {
            return Err(GameError {
                code: "unknown_action",
                message: "That action doesn't exist in this game",
            });
        }
        let Some(presses) = self.presses.get_mut(&player) else {
            return Err(GameError {
                code: "not_playing",
                message: "You aren't in this game",
            });
        };

        *presses += 1;
        ctx.send_to(player, json!({ "presses": *presses }));
        if *presses == PRESSES_TO_WIN {
            self.finished = true;
            ctx.broadcast(json!({ "event": "won", "winner": player }));
        }
        Ok(())
    }

    fn handle_timer(&mut self, timer: TimerId, ctx: &mut GameCtx) {
        // Ignore the time limit if someone already won
        if timer == TIME_LIMIT_TIMER && !self.finished {
            self.finished = true;
            ctx.broadcast(json!({ "event": "time_up" }));
        }
    }

    fn player_joined(&mut self, player: PlayerId, _ctx: &mut GameCtx) {
        self.presses.insert(player, 0);
    }

    fn player_left(&mut self, player: PlayerId, _ctx: &mut GameCtx) {
        self.presses.remove(&player);
    }

    // Each player only sees their own presses
    fn view_for(&self, player: PlayerId) -> Value {
        json!({ "presses": self.presses.get(&player) })
    }

    fn is_finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{Outgoing, Recipient};

    fn started_game() -> DummyGame {
        let mut game = DummyGame::default();
        game.start(&[1, 2, 3], &mut GameCtx::default());
        game
    }

    fn press(game: &mut DummyGame, player: PlayerId, ctx: &mut GameCtx) -> Result<(), GameError> {
        game.handle_action(player, json!({ "kind": "press" }), ctx)
    }

    #[test]
    fn start_tells_everyone_and_starts_the_time_limit() {
        let mut game = DummyGame::default();
        let mut ctx = GameCtx::default();
        game.start(&[1, 2, 3], &mut ctx);

        assert_eq!(
            ctx.outgoing,
            [Outgoing {
                to: Recipient::Everyone,
                message: json!({ "event": "started" }),
            }]
        );
        assert_eq!(ctx.timers, [(TIME_LIMIT_TIMER, TIME_LIMIT)]);
    }

    #[test]
    fn a_press_is_only_told_to_that_player() {
        let mut game = started_game();
        let mut ctx = GameCtx::default();
        press(&mut game, 2, &mut ctx).unwrap();

        assert_eq!(
            ctx.outgoing,
            [Outgoing {
                to: Recipient::Player(2),
                message: json!({ "presses": 1 }),
            }]
        );
    }

    #[test]
    fn third_press_wins() {
        let mut game = started_game();
        let mut ctx = GameCtx::default();
        press(&mut game, 1, &mut ctx).unwrap();
        press(&mut game, 1, &mut ctx).unwrap();
        assert!(!game.is_finished());
        press(&mut game, 1, &mut ctx).unwrap();

        assert!(game.is_finished());
        assert_eq!(
            ctx.outgoing.last().map(|outgoing| &outgoing.message),
            Some(&json!({ "event": "won", "winner": 1 }))
        );
        assert_eq!(press(&mut game, 2, &mut ctx).unwrap_err().code, "game_over");
    }

    #[test]
    fn wrong_actions_are_refused() {
        let mut game = started_game();
        let mut ctx = GameCtx::default();

        let dance = game.handle_action(1, json!({ "kind": "dance" }), &mut ctx);
        assert_eq!(dance.unwrap_err().code, "unknown_action");
        assert_eq!(
            press(&mut game, 99, &mut ctx).unwrap_err().code,
            "not_playing"
        );
        assert!(ctx.outgoing.is_empty());
    }

    #[test]
    fn time_limit_ends_the_game() {
        let mut game = started_game();
        let mut ctx = GameCtx::default();
        game.handle_timer(TIME_LIMIT_TIMER, &mut ctx);

        assert!(game.is_finished());
        assert_eq!(ctx.outgoing[0].message, json!({ "event": "time_up" }));
    }

    #[test]
    fn late_joiner_can_play_and_leaver_cannot() {
        let mut game = started_game();
        let mut ctx = GameCtx::default();
        game.player_joined(4, &mut ctx);
        game.player_left(1, &mut ctx);

        assert!(press(&mut game, 4, &mut ctx).is_ok());
        assert_eq!(
            press(&mut game, 1, &mut ctx).unwrap_err().code,
            "not_playing"
        );
    }

    #[test]
    fn views_are_private() {
        let mut game = started_game();
        press(&mut game, 1, &mut GameCtx::default()).unwrap();

        assert_eq!(game.view_for(1), json!({ "presses": 1 }));
        assert_eq!(game.view_for(2), json!({ "presses": 0 }));
    }

    #[test]
    fn registry_entry_makes_a_fresh_game() {
        let settings = RoomSettings { points_to_win: 7 };
        let game = (DUMMY_INFO.create)(&settings);
        assert!(!game.is_finished());
        assert!(DUMMY_INFO.fits(3));
    }
}
