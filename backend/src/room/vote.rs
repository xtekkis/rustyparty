use std::collections::HashMap;
use std::time::Duration;

use rand::{Rng, RngExt};
use tokio::sync::mpsc::WeakSender;
use tokio::time::Instant;

use super::actor::{RoomCommand, remind};
use super::{Room, RoomState};
use crate::games::find_game;
use crate::protocol::ServerMessage;

// The game vote needs at least this many players
const MIN_PLAYERS_TO_VOTE: usize = 3;

// How long players have to vote
pub(super) const VOTE_DURATION: Duration = Duration::from_secs(30);

// The game that won the vote
#[derive(Debug, PartialEq)]
pub struct Winner {
    pub game: &'static str,
    // True when several games had the most votes and one was picked at random
    pub tie_break: bool,
}

// The game with the most votes, a random one of them on a tie, none if nobody voted
pub fn pick_winner(votes: &HashMap<u32, &'static str>, rng: &mut impl Rng) -> Option<Winner> {
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for &game in votes.values() {
        *counts.entry(game).or_default() += 1;
    }

    let most = counts.values().copied().max()?;
    let mut leaders: Vec<&'static str> = counts
        .into_iter()
        .filter(|&(_, count)| count == most)
        .map(|(game, _)| game)
        .collect();
    // A map has no fixed order, sort so the same random number always picks the same game
    leaders.sort();

    let game = leaders[rng.random_range(0..leaders.len())];
    Some(Winner {
        game,
        tie_break: leaders.len() > 1,
    })
}

impl Room {
    pub(super) fn vote(&mut self, player_id: u32, game_id: &str) {
        if !matches!(self.state, RoomState::Voting { .. }) {
            self.send_error(player_id, "not_voting", "Voting isn't open");
            return;
        }
        let Some(game) = find_game(self.games, game_id) else {
            self.send_error(player_id, "unknown_game", "That game doesn't exist");
            return;
        };
        if !game.fits(self.players.len()) {
            self.send_error(
                player_id,
                "game_not_available",
                "That game doesn't fit the number of players",
            );
            return;
        }

        if let RoomState::Voting { votes, .. } = &mut self.state {
            votes.insert(player_id, game.id);
        }
        self.broadcast(&self.vote_message());
    }

    // Everyone connected is ready and there are enough of them: start the game vote
    pub(super) fn start_vote_if_ready(&mut self, me: &WeakSender<RoomCommand>) {
        if !matches!(self.state, RoomState::Lobby) {
            return;
        }
        // Players in their grace period don't hold up the others
        let connected: Vec<_> = self
            .players
            .iter()
            .filter(|player| player.disconnected_since.is_none())
            .collect();
        if connected.len() < MIN_PLAYERS_TO_VOTE || !connected.iter().all(|player| player.ready) {
            return;
        }

        self.votes_started += 1;
        let number = self.votes_started;
        self.state = RoomState::Voting {
            votes: HashMap::new(),
            number,
            ends_at: Instant::now() + VOTE_DURATION,
        };
        tracing::info!("Voting started in room {}", self.code);
        self.broadcast(&self.vote_message());
        remind(me, VOTE_DURATION, RoomCommand::VoteTimeout { number });
    }

    // No need to wait out the timer once every connected player has picked a game
    pub(super) fn end_vote_if_everyone_voted(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::Voting { votes, .. } = &self.state else {
            return;
        };
        let everyone_voted = self
            .players
            .iter()
            .filter(|player| player.disconnected_since.is_none())
            .all(|player| votes.contains_key(&player.id));
        if everyone_voted {
            self.end_vote(me);
        }
    }

    // Announce the winner and start it, or go back to the lobby if nobody voted
    pub(super) fn end_vote(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::Voting { votes, .. } = std::mem::replace(&mut self.state, RoomState::Lobby)
        else {
            return;
        };

        let Some(winner) = pick_winner(&votes, &mut rand::rng()) else {
            tracing::info!("Nobody voted in room {}", self.code);
            self.return_to_lobby();
            return;
        };

        tracing::info!("{} won the vote in room {}", winner.game, self.code);
        self.broadcast(&ServerMessage::VoteResult {
            game: winner.game.to_string(),
            tie_break: winner.tie_break,
        });
        if let Some(info) = find_game(self.games, winner.game) {
            self.start_game(info, me);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    // Player 1 voted for the first game, player 2 for the second, and so on
    fn votes(games: &[&'static str]) -> HashMap<u32, &'static str> {
        (1..).zip(games.iter().copied()).collect()
    }

    fn rng(seed: u64) -> StdRng {
        StdRng::seed_from_u64(seed)
    }

    #[test]
    fn nobody_voted_means_no_winner() {
        assert_eq!(pick_winner(&votes(&[]), &mut rng(1)), None);
    }

    #[test]
    fn most_votes_wins() {
        let votes = votes(&["bad_takes", "trivia", "bad_takes"]);
        assert_eq!(
            pick_winner(&votes, &mut rng(1)),
            Some(Winner {
                game: "bad_takes",
                tie_break: false,
            })
        );
    }

    #[test]
    fn a_single_vote_wins() {
        let winner = pick_winner(&votes(&["trivia"]), &mut rng(1)).unwrap();
        assert_eq!(winner.game, "trivia");
        assert!(!winner.tie_break);
    }

    #[test]
    fn tie_is_broken_and_says_so() {
        let votes = votes(&["bad_takes", "trivia", "drawing", "drawing", "bad_takes"]);
        let winner = pick_winner(&votes, &mut rng(1)).unwrap();

        assert!(["bad_takes", "drawing"].contains(&winner.game));
        assert!(winner.tie_break);
    }

    #[test]
    fn same_seed_picks_the_same_game() {
        let votes = votes(&["bad_takes", "trivia"]);
        let first = pick_winner(&votes, &mut rng(42));
        let second = pick_winner(&votes, &mut rng(42));
        assert_eq!(first, second);
    }

    #[test]
    fn tie_break_is_fair() {
        let votes = votes(&["bad_takes", "trivia"]);
        let mut rng = rng(7);
        let rounds = 1000;
        let bad_takes_wins = (0..rounds)
            .filter(|_| pick_winner(&votes, &mut rng).unwrap().game == "bad_takes")
            .count();

        // Each should win about half the time
        assert!(
            (400..=600).contains(&bad_takes_wins),
            "bad_takes won {bad_takes_wins} of {rounds}"
        );
    }
}
