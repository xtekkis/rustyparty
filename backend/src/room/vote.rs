use std::collections::HashMap;

use rand::{Rng, RngExt};

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
