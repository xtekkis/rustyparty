#[cfg(test)]
pub mod dummy;

use std::time::Duration;

use serde_json::Value;

use crate::protocol::RoomSettings;

pub type PlayerId = u32;
pub type TimerId = u32;

// Why a player's action was refused, sent back to them as an error
#[derive(Debug, PartialEq)]
pub struct GameError {
    pub code: &'static str,
    pub message: &'static str,
}

// Who a game message is for
#[derive(Debug, PartialEq)]
pub enum Recipient {
    Player(PlayerId),
    Everyone,
}

#[derive(Debug, PartialEq)]
pub struct Outgoing {
    pub to: Recipient,
    pub message: Value,
}

// What a game asks the room to do: messages to send and timers to start
#[derive(Default)]
pub struct GameCtx {
    pub outgoing: Vec<Outgoing>,
    pub timers: Vec<(TimerId, Duration)>,
}

impl GameCtx {
    pub fn send_to(&mut self, player: PlayerId, message: Value) {
        self.outgoing.push(Outgoing {
            to: Recipient::Player(player),
            message,
        });
    }

    pub fn broadcast(&mut self, message: Value) {
        self.outgoing.push(Outgoing {
            to: Recipient::Everyone,
            message,
        });
    }

    // The room calls handle_timer with this id once the time is up
    pub fn start_timer(&mut self, timer: TimerId, after: Duration) {
        self.timers.push((timer, after));
    }
}

// The rules of one game. A game never touches sockets, real timers or the database:
// it reacts to events and writes what should happen into the GameCtx
pub trait Game: Send {
    // Called once, when this game wins the vote
    fn start(&mut self, players: &[PlayerId], ctx: &mut GameCtx);

    // A player did something, like playing a card
    fn handle_action(
        &mut self,
        player: PlayerId,
        action: Value,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError>;

    // A timer this game asked for has run out
    fn handle_timer(&mut self, timer: TimerId, ctx: &mut GameCtx);

    // A late joiner, the game adds them at the start of its next round
    fn player_joined(&mut self, player: PlayerId, ctx: &mut GameCtx);

    // A player left for good: left, kicked, banned, or their grace period ran out
    fn player_left(&mut self, player: PlayerId, ctx: &mut GameCtx);

    // What this player is allowed to see right now
    fn view_for(&self, player: PlayerId) -> Value;

    fn is_finished(&self) -> bool;
}

// Facts about a game that don't change. The vote screen is built from these
pub struct GameInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub min_players: usize,
    pub max_players: usize,
    // Makes a fresh game when this one wins the vote
    pub create: fn(&RoomSettings) -> Box<dyn Game>,
}

impl GameInfo {
    // Whether this many players can play it
    pub fn fits(&self, players: usize) -> bool {
        (self.min_players..=self.max_players).contains(&players)
    }
}

// Every game players can vote for
pub static GAMES: &[GameInfo] = &[];

pub fn find_game<'a>(games: &'a [GameInfo], id: &str) -> Option<&'a GameInfo> {
    games.iter().find(|game| game.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ctx_collects_messages_and_timers_in_order() {
        let mut ctx = GameCtx::default();
        ctx.send_to(2, json!({ "hand": [12, 40] }));
        ctx.broadcast(json!({ "round": 1 }));
        ctx.start_timer(1, Duration::from_secs(60));

        assert_eq!(
            ctx.outgoing,
            [
                Outgoing {
                    to: Recipient::Player(2),
                    message: json!({ "hand": [12, 40] }),
                },
                Outgoing {
                    to: Recipient::Everyone,
                    message: json!({ "round": 1 }),
                },
            ]
        );
        assert_eq!(ctx.timers, [(1, Duration::from_secs(60))]);
    }

    fn test_info(id: &'static str, min_players: usize, max_players: usize) -> GameInfo {
        GameInfo {
            id,
            name: "Test",
            description: "A game for tests",
            min_players,
            max_players,
            create: |_| unreachable!("not started in these tests"),
        }
    }

    #[test]
    fn game_fits_its_player_limits() {
        let game = test_info("test", 3, 10);
        assert!(!game.fits(2));
        assert!(game.fits(3));
        assert!(game.fits(10));
        assert!(!game.fits(11));
    }

    #[test]
    fn games_are_found_by_id() {
        let games = [test_info("first", 3, 10), test_info("second", 2, 8)];
        assert_eq!(
            find_game(&games, "second").map(|game| game.id),
            Some("second")
        );
        assert!(find_game(&games, "missing").is_none());
    }
}
