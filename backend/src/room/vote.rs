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
