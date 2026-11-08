// Bad Takes tests, one file per part of the game. The helpers they share are here
use super::*;
use crate::games::{GAMES, find_game};
use rand::SeedableRng;
use std::collections::HashSet;

mod auto_pull;
mod cards;
mod game_end;
mod judging;
mod late_joiners;
mod leaving;
mod privacy;
mod reading;
mod rounds;
mod setup;
mod submitting;

// A game with the starter deck, dealt the same way every run
fn started_game(players: &[PlayerId]) -> BadTakes {
    let cards = deck::starter_deck();
    let mut game = BadTakes::new(
        7,
        cards.prompts.clone(),
        cards.answers.clone(),
        StdRng::seed_from_u64(1),
    );
    game.start(players, &mut GameCtx::default());
    // A known pick 1 prompt, so tests don't depend on what the shuffle revealed
    game.prompt = Some(starter_prompt(2));
    game
}

fn starter_prompt(id: CardId) -> PromptCard {
    deck::starter_deck()
        .prompts
        .iter()
        .find(|prompt| prompt.id == id)
        .cloned()
        .expect("the starter deck has this prompt")
}

fn submit(game: &mut BadTakes, player: PlayerId, cards: &[CardId]) -> Result<GameCtx, GameError> {
    let mut ctx = GameCtx::default();
    game.handle_action(
        player,
        json!({ "kind": "submit", "cards": cards }),
        &mut ctx,
    )?;
    Ok(ctx)
}

// Everyone but the judge plays their first cards, as many as the prompt needs
fn everyone_submits(game: &mut BadTakes, ctx: &mut GameCtx) {
    let gaps = usize::from(game.prompt.as_ref().expect("a prompt is out").pick);
    let players: Vec<PlayerId> = game.players.clone();
    for player in players {
        if Some(player) != game.judge {
            let cards: Vec<CardId> = game.hands[&player][..gaps]
                .iter()
                .map(|card| card.id)
                .collect();
            game.handle_action(player, json!({ "kind": "submit", "cards": cards }), ctx)
                .unwrap();
        }
    }
}

// The judge pulls the next answer out of the bag
fn pull(game: &mut BadTakes, player: PlayerId) -> Result<GameCtx, GameError> {
    let mut ctx = GameCtx::default();
    game.handle_action(player, json!({ "kind": "pull" }), &mut ctx)?;
    Ok(ctx)
}

// The judge shows every answer together
fn show_all(game: &mut BadTakes, player: PlayerId) -> Result<GameCtx, GameError> {
    let mut ctx = GameCtx::default();
    game.handle_action(player, json!({ "kind": "show_all" }), &mut ctx)?;
    Ok(ctx)
}

// The judge pulls out every answer still in the bag, then shows them all
fn pull_and_show_all(game: &mut BadTakes, ctx: &mut GameCtx) {
    if game.phase != Phase::Reading {
        return;
    }
    let judge = game.judge.expect("a round has a judge");
    while game.in_bag() > 0 {
        game.handle_action(judge, json!({ "kind": "pull" }), ctx)
            .unwrap();
    }
    game.handle_action(judge, json!({ "kind": "show_all" }), ctx)
        .unwrap();
}

// Everyone plays, then the judge pulls out every answer and shows them all, ready to pick
fn all_answers_out(game: &mut BadTakes, ctx: &mut GameCtx) {
    everyone_submits(game, ctx);
    pull_and_show_all(game, ctx);
}

fn pick(game: &mut BadTakes, player: PlayerId, card: CardId) -> Result<GameCtx, GameError> {
    let mut ctx = GameCtx::default();
    game.handle_action(player, json!({ "kind": "pick", "card": card }), &mut ctx)?;
    Ok(ctx)
}

// The judge pulls out and shows every answer, picks the first, then the reveal runs out
fn finish_round(game: &mut BadTakes) {
    pull_and_show_all(game, &mut GameCtx::default());
    let first = game.reading_order[0];
    let card = game.submissions[&first][0].id;
    let judge = game.judge.expect("a round has a judge");
    pick(game, judge, card).unwrap();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut GameCtx::default());
}

// Every answer card is somewhere: in the deck, a hand, on the table or discarded
fn cards_in_play(game: &BadTakes) -> usize {
    game.answers.len()
        + game.hands.values().map(Vec::len).sum::<usize>()
        + game.submissions.values().map(Vec::len).sum::<usize>()
        + game.discard.len()
}
