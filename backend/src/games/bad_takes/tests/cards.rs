// Where the cards go: refills, the discard pile and reshuffles
use super::*;

#[test]
fn hands_are_refilled_when_the_next_round_starts() {
    let mut game = started_game(&[1, 2, 3]);
    // A pick 2 prompt, so Sam and Kim each play two cards
    game.prompt = Some(starter_prompt(5));
    for player in [2, 3] {
        let cards = [game.hands[&player][0].id, game.hands[&player][1].id];
        submit(&mut game, player, &cards).unwrap();
    }
    finish_round(&mut game);

    for player in [1, 2, 3] {
        assert_eq!(game.hands[&player].len(), HAND_SIZE);
    }
    assert_eq!(game.answers.len(), 140 - 3 * HAND_SIZE - 4);
    assert_eq!(cards_in_play(&game), 140);
}

#[test]
fn the_discard_pile_becomes_the_new_deck() {
    let mut game = started_game(&[1, 2, 3]);
    // Every card not in a hand has already been played
    while let Some(card) = game.answers.draw() {
        game.discard.push(card);
    }
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    for player in [1, 2, 3] {
        assert_eq!(game.hands[&player].len(), HAND_SIZE);
    }
    assert!(game.discard.is_empty());
    assert_eq!(game.answers.len(), 140 - 3 * HAND_SIZE);
    assert_eq!(cards_in_play(&game), 140);
}

#[test]
fn the_deck_is_only_reshuffled_once_it_runs_out() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    assert_eq!(game.discard.len(), 2, "last round's cards wait in the pile");
    assert_eq!(game.answers.len(), 140 - 3 * HAND_SIZE - 2);
}

#[test]
fn used_prompts_are_shuffled_into_a_new_deck() {
    let mut game = started_game(&[1, 2, 3]);
    while let Some(prompt) = game.prompts.draw() {
        game.used_prompts.push(prompt);
    }
    game.handle_timer(game.timer_id(Timer::Submit), &mut GameCtx::default());

    assert_eq!(game.round, 2);
    assert!(game.prompt.is_some(), "a prompt from the new deck");
    assert!(game.used_prompts.is_empty());
    assert_eq!(game.prompts.len(), 45 - 1);
}

#[test]
fn every_prompt_is_somewhere() {
    let mut game = started_game(&[1, 2, 3]);
    for _ in 0..50 {
        game.handle_timer(game.timer_id(Timer::Submit), &mut GameCtx::default());
        let in_play = usize::from(game.prompt.is_some());
        assert_eq!(game.prompts.len() + game.used_prompts.len() + in_play, 45);
    }
}

#[test]
fn played_cards_are_not_dealt_again() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    for card in &game.discard {
        assert!(game.hands.values().flatten().all(|held| held != card));
    }
}

#[test]
fn played_cards_stay_on_the_table_for_the_reveal() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    assert!(game.discard.is_empty());
    assert_eq!(game.cards_to_judge().len(), 2);
}

#[test]
fn played_cards_are_discarded_when_the_next_round_starts() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let played: HashSet<CardId> = game
        .submissions
        .values()
        .flatten()
        .map(|card| card.id)
        .collect();
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    game.start_round(&mut GameCtx::default());
    let discarded: HashSet<CardId> = game.discard.iter().map(|card| card.id).collect();
    assert_eq!(discarded, played);
    assert!(game.submissions.is_empty(), "everyone can play again");
    assert!(game.cards_to_judge().is_empty());
    assert_eq!(cards_in_play(&game), 140);
}
