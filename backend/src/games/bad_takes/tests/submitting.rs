// Playing cards: what can be played, and what's refused
use super::*;

#[test]
fn a_pick_1_prompt_takes_exactly_one_card() {
    let mut game = started_game(&[1, 2, 3]);
    let hand: Vec<CardId> = game.hands[&2].iter().map(|card| card.id).collect();

    for cards in [vec![], vec![hand[0], hand[1]]] {
        let result = submit(&mut game, 2, &cards);
        assert_eq!(
            result.err().map(|error| error.code),
            Some("wrong_card_count")
        );
    }
    assert_eq!(game.hands[&2].len(), HAND_SIZE);
}

#[test]
fn a_pick_2_prompt_takes_two_cards_in_order() {
    let mut game = started_game(&[1, 2, 3]);
    // "Tonight's main event: ____ vs. ____."
    game.prompt = Some(starter_prompt(5));
    let first = game.hands[&2][4].clone();
    let second = game.hands[&2][1].clone();

    let one_card = submit(&mut game, 2, &[first.id]);
    assert_eq!(
        one_card.err().map(|error| error.code),
        Some("wrong_card_count")
    );

    submit(&mut game, 2, &[first.id, second.id]).unwrap();
    assert_eq!(game.submissions[&2], [first, second]);
}

#[test]
fn submitted_cards_leave_the_hand() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&2][0].clone();

    submit(&mut game, 2, &[card.id]).unwrap();
    assert_eq!(game.hands[&2].len(), HAND_SIZE - 1);
    assert!(!game.hands[&2].contains(&card));
    assert_eq!(game.submissions[&2], [card]);
}

#[test]
fn everyone_sees_who_submitted_but_not_what() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&2][0].clone();
    let ctx = submit(&mut game, 2, &[card.id]).unwrap();

    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "submitted", "player": 2 })
    );
    let kim_sees = game.view_for(3);
    assert_eq!(kim_sees["submitted"], json!([2]));
    assert!(!kim_sees.to_string().contains(&card.text));
    assert_eq!(game.view_for(2)["my_submission"], json!([card]));
}

#[test]
fn the_judge_cannot_submit() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&1][0].id;

    let result = submit(&mut game, 1, &[card]);
    assert_eq!(
        result.err().map(|error| error.code),
        Some("judge_cannot_submit")
    );
    assert_eq!(game.hands[&1].len(), HAND_SIZE, "the judge keeps the card");
    assert!(game.submissions.is_empty());
}

#[test]
fn cards_you_dont_hold_are_refused() {
    let mut game = started_game(&[1, 2, 3]);
    game.prompt = Some(starter_prompt(5));
    let mine = game.hands[&2][0].id;
    let kims = game.hands[&3][0].id;

    let result = submit(&mut game, 2, &[mine, kims]);
    assert_eq!(
        result.err().map(|error| error.code),
        Some("card_not_in_hand")
    );
    assert_eq!(game.hands[&2].len(), HAND_SIZE, "nothing left the hand");
    assert_eq!(game.hands[&3].len(), HAND_SIZE, "Kim still has her card");
    assert!(game.submissions.is_empty());
}

#[test]
fn the_same_card_twice_is_refused() {
    let mut game = started_game(&[1, 2, 3]);
    game.prompt = Some(starter_prompt(5));
    let card = game.hands[&2][0].id;

    let result = submit(&mut game, 2, &[card, card]);
    assert_eq!(
        result.err().map(|error| error.code),
        Some("card_not_in_hand")
    );
    assert_eq!(game.hands[&2].len(), HAND_SIZE);
}

#[test]
fn no_cards_once_judging_has_started() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());
    assert_eq!(game.phase, Phase::Judging);

    // Nobody can play now, not even someone who somehow still could
    game.submissions.remove(&4);
    let card = game.hands[&4][0].id;
    let result = submit(&mut game, 4, &[card]);
    assert_eq!(result.err().map(|error| error.code), Some("not_submitting"));
}

#[test]
fn nothing_to_judge_until_everyone_has_played() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();
    assert_eq!(game.view_for(1)["cards_to_judge"], json!([]));
}

#[test]
fn when_time_is_up_the_judge_reads_what_was_played() {
    let mut game = started_game(&[1, 2, 3, 4]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Submit), &mut ctx);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(game.reading_order, [2], "Kim and Lee sit this round out");
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "reading_started", "answers": 1 })
    );
}

#[test]
fn time_up_after_everyone_has_played_changes_nothing() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let order = game.reading_order.clone();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Submit), &mut ctx);
    assert_eq!(game.reading_order, order, "not shuffled again");
    assert!(ctx.outgoing.is_empty());
}

#[test]
fn someone_without_a_hand_cannot_submit() {
    let mut game = started_game(&[1, 2, 3]);
    let result = submit(&mut game, 99, &[1]);

    assert_eq!(result.err().map(|error| error.code), Some("not_playing"));
    assert!(!game.hands.contains_key(&99), "no hand is made for them");
}

#[test]
fn no_prompt_means_no_submissions() {
    let mut game = started_game(&[1, 2, 3]);
    game.prompt = None;
    let card = game.hands[&2][0].id;

    let result = submit(&mut game, 2, &[card]);
    assert_eq!(
        result.err().map(|error| error.code),
        Some("wrong_card_count")
    );
}

#[test]
fn a_malformed_submit_is_refused() {
    let mut game = started_game(&[1, 2, 3]);
    let mut ctx = GameCtx::default();
    let result = game.handle_action(2, json!({ "kind": "submit", "cards": "12" }), &mut ctx);
    assert_eq!(result.err().map(|error| error.code), Some("unknown_action"));
}

#[test]
fn a_refused_submission_changes_nothing() {
    let mut game = started_game(&[1, 2, 3]);
    let sams_card = game.hands[&2][0].id;
    let kims_card = game.hands[&3][0].id;
    let hands_before = game.hands.clone();

    // The judge, a card Sam doesn't hold, too many cards, and someone not playing
    let attempts: [(PlayerId, Vec<CardId>); 4] = [
        (1, vec![game.hands[&1][0].id]),
        (2, vec![kims_card]),
        (2, vec![sams_card, sams_card]),
        (99, vec![sams_card]),
    ];
    for (player, cards) in attempts {
        let mut ctx = GameCtx::default();
        let result = game.handle_action(
            player,
            json!({ "kind": "submit", "cards": cards }),
            &mut ctx,
        );
        assert!(result.is_err());
        assert!(ctx.outgoing.is_empty(), "a refused submission tells nobody");
    }
    assert_eq!(game.hands, hands_before);
    assert!(game.submissions.is_empty());
}

#[test]
fn submitting_twice_is_refused() {
    let mut game = started_game(&[1, 2, 3]);
    let first = game.hands[&2][0].id;
    let second = game.hands[&2][1].id;
    submit(&mut game, 2, &[first]).unwrap();

    let again = submit(&mut game, 2, &[second]);
    assert_eq!(
        again.err().map(|error| error.code),
        Some("already_submitted")
    );
    assert_eq!(
        game.hands[&2].len(),
        HAND_SIZE - 1,
        "the second card stays in the hand"
    );
}
