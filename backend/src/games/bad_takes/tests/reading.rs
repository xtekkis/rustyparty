// The bag: the judge pulls the answers out, then shows them all
use super::*;

#[test]
fn answers_go_in_the_bag_when_everyone_has_played() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();
    assert_eq!(game.phase, Phase::Submitting, "Kim hasn't played yet");

    let card = game.hands[&3][0].id;
    let ctx = submit(&mut game, 3, &[card]).unwrap();
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(
        ctx.outgoing.last(),
        Some(&json!({ "event": "reading_started", "answers": 2 }))
    );
    assert_eq!(
        ctx.timers,
        [(game.pull_timer_id(), PULL_DURATION)],
        "the pick timer waits for the answers to be shown"
    );
    let view = game.view_for(1);
    assert_eq!(view["phase"], "reading");
    assert_eq!(view["cards_to_judge"], json!([]));
    assert_eq!(view["in_bag"], 2);
}

#[test]
fn the_judge_pulls_the_answers_out_one_at_a_time() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());

    let first = game.submissions[&game.reading_order[0]].clone();
    let ctx = pull(&mut game, 1).unwrap();
    assert_eq!(ctx.outgoing.len(), 1);
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "answer_pulled", "cards": first })
    );
    for player in [1, 2, 3, 4] {
        let view = game.view_for(player);
        assert_eq!(view["cards_to_judge"], json!([first]));
        assert_eq!(view["in_bag"], 2);
    }
    assert_eq!(game.phase, Phase::Reading, "two answers to go");
}

#[test]
fn the_pick_timer_starts_when_the_judge_shows_all_the_answers() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();

    // The last card gets read out like the others, with no rush to pick yet
    let last = game.submissions[&game.reading_order[1]].clone();
    let ctx = pull(&mut game, 1).unwrap();
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(ctx.outgoing.len(), 1);
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "answer_pulled", "cards": last })
    );
    assert_eq!(ctx.timers, [(game.pull_timer_id(), PULL_DURATION)]);
    assert_eq!(game.view_for(2)["in_bag"], 0);

    let ctx = show_all(&mut game, 1).unwrap();
    assert_eq!(game.phase, Phase::Judging);
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "judging_started", "judge_for": 45 })
    );
    assert_eq!(ctx.timers, [(game.timer_id(Timer::Judge), JUDGE_DURATION)]);
    assert_eq!(game.view_for(2)["phase"], "judging");
}

#[test]
fn only_the_judge_can_show_the_answers() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();

    let result = show_all(&mut game, 2);
    assert_eq!(result.err().map(|error| error.code), Some("not_the_judge"));
    assert_eq!(game.phase, Phase::Reading);
}

#[test]
fn answers_cant_be_shown_with_cards_still_in_the_bag() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();

    let result = show_all(&mut game, 1);
    assert_eq!(result.err().map(|error| error.code), Some("cards_in_bag"));
    assert_eq!(game.phase, Phase::Reading);
}

#[test]
fn pulling_from_an_empty_bag_is_refused() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();

    let result = pull(&mut game, 1);
    assert_eq!(result.err().map(|error| error.code), Some("bag_empty"));
    assert_eq!(game.pulled, 2);
}

#[test]
fn nothing_to_show_outside_the_reading() {
    let mut game = started_game(&[1, 2, 3]);
    let before = show_all(&mut game, 1);
    assert_eq!(before.err().map(|error| error.code), Some("not_reading"));

    all_answers_out(&mut game, &mut GameCtx::default());
    let after = show_all(&mut game, 1);
    assert_eq!(after.err().map(|error| error.code), Some("not_reading"));
}

#[test]
fn only_the_judge_can_pull() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());

    let result = pull(&mut game, 2);
    assert_eq!(result.err().map(|error| error.code), Some("not_the_judge"));
    assert_eq!(game.pulled, 0);
}

#[test]
fn nothing_to_pull_outside_the_reading() {
    let mut game = started_game(&[1, 2, 3]);
    let before = pull(&mut game, 1);
    assert_eq!(before.err().map(|error| error.code), Some("not_reading"));

    all_answers_out(&mut game, &mut GameCtx::default());
    let after = pull(&mut game, 1);
    assert_eq!(after.err().map(|error| error.code), Some("not_reading"));
}

#[test]
fn no_picking_until_the_answers_are_shown() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    let pulled = game.submissions[&game.reading_order[0]][0].id;

    // Not with a card still in the bag, and not with the bag empty either
    for _ in 0..2 {
        pull(&mut game, 1).unwrap();
        let result = pick(&mut game, 1, pulled);
        assert_eq!(result.err().map(|error| error.code), Some("not_judging"));
    }
    assert_eq!(game.winner, None);
}

#[test]
fn everyone_sees_the_cards_but_not_whose_they_are() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let in_reading_order: Vec<_> = game
        .reading_order
        .iter()
        .map(|player| &game.submissions[player])
        .collect();
    for player in [1, 2, 3, 4] {
        let view = game.view_for(player);
        assert_eq!(view["cards_to_judge"], json!(in_reading_order));
    }
    // Each entry is just the cards, with no player attached
    let entry = &game.view_for(1)["cards_to_judge"][0];
    assert_eq!(entry, &json!(game.submissions[&game.reading_order[0]]));
}

#[test]
fn the_reading_order_is_shuffled() {
    let players: Vec<PlayerId> = (1..=8).collect();
    let mut game = started_game(&players);
    all_answers_out(&mut game, &mut GameCtx::default());

    let mut sorted = game.reading_order.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        (2..=8).collect::<Vec<_>>(),
        "everyone but the judge"
    );
    assert_ne!(game.reading_order, sorted, "not in player order");
}
