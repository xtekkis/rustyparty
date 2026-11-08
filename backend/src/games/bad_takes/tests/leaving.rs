// Players and judges leaving mid-round
use super::*;

#[test]
fn a_leaver_no_longer_holds_up_judging() {
    let mut game = started_game(&[1, 2, 3, 4]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();
    let card = game.hands[&3][0].id;
    submit(&mut game, 3, &[card]).unwrap();
    assert_eq!(game.phase, Phase::Submitting, "Lee hasn't played");

    let mut ctx = GameCtx::default();
    game.player_left(4, &mut ctx);
    assert_eq!(game.phase, Phase::Reading);
}

#[test]
fn a_leaver_takes_their_cards_with_them() {
    let mut game = started_game(&[1, 2, 3, 4]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();

    game.player_left(2, &mut GameCtx::default());
    assert!(!game.submissions.contains_key(&2));
    assert_eq!(game.phase, Phase::Submitting);
}

#[test]
fn a_leaver_drops_out_of_the_reading_order() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());

    game.player_left(3, &mut GameCtx::default());
    assert!(!game.reading_order.contains(&3));
    assert_eq!(game.cards_to_judge().len(), 2);
}

#[test]
fn a_leaver_leaves_the_scoreboard() {
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_left(4, &mut GameCtx::default());
    assert_eq!(
        game.view_for(1)["scores"],
        json!({ "1": 0, "2": 0, "3": 0 })
    );
}

#[test]
fn a_leaver_is_skipped_in_the_rotation() {
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_left(2, &mut GameCtx::default());
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);
    assert_eq!(game.judge, Some(3));
}

#[test]
fn the_round_is_skipped_when_every_answer_has_left() {
    let mut game = started_game(&[1, 2, 3, 4, 5]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();
    game.handle_timer(game.timer_id(Timer::Submit), &mut GameCtx::default());
    assert_eq!(game.reading_order, [2]);

    let mut ctx = GameCtx::default();
    game.player_left(2, &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_skipped" }));
    assert_eq!(game.round, 2);
    assert_eq!(game.phase, Phase::Submitting);
}

#[test]
fn the_judge_leaving_cancels_the_round() {
    let mut game = started_game(&[1, 2, 3, 4]);
    let sams_card = game.hands[&2][0].clone();
    submit(&mut game, 2, &[sams_card.id]).unwrap();

    let mut ctx = GameCtx::default();
    game.player_left(1, &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_cancelled" }));
    assert_eq!(game.round, 2);
    assert_eq!(game.judge, Some(2));
    assert!(game.hands[&2].contains(&sams_card), "Sam has his card back");
    assert_eq!(game.hands[&2].len(), HAND_SIZE);
    assert_eq!(game.discard.len(), HAND_SIZE, "only the judge's hand");
    assert_eq!(cards_in_play(&game), 140);
}

#[test]
fn the_judge_leaving_while_reading_cancels_the_round() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());

    game.player_left(1, &mut GameCtx::default());
    assert_eq!(game.phase, Phase::Submitting);
    assert!(game.submissions.is_empty());
    for player in [2, 3, 4] {
        assert_eq!(game.hands[&player].len(), HAND_SIZE);
    }
    assert_eq!(game.scores.values().sum::<u8>(), 0, "no point");
}

#[test]
fn the_judge_leaving_while_picking_cancels_the_round() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.player_left(1, &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_cancelled" }));
    assert_eq!(game.phase, Phase::Submitting);
    assert_eq!(game.pulled, 0);
    assert!(game.submissions.is_empty());
    assert_eq!(game.scores.values().sum::<u8>(), 0, "no point");
}

#[test]
fn a_leaver_whose_answer_is_out_doesnt_pull_the_next_card() {
    let mut game = started_game(&[1, 2, 3, 4, 5]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();
    let order = game.reading_order.clone();
    let still_out = game.submissions[&order[1]].clone();
    let next = game.submissions[&order[2]].clone();

    game.player_left(order[0], &mut GameCtx::default());
    let view = game.view_for(1);
    assert_eq!(view["cards_to_judge"], json!([still_out]));
    assert_eq!(view["in_bag"], 2);

    let ctx = pull(&mut game, 1).unwrap();
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "answer_pulled", "cards": next })
    );
}

#[test]
fn a_leaver_whose_answer_is_in_the_bag_is_never_shown() {
    let mut game = started_game(&[1, 2, 3, 4, 5]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    let order = game.reading_order.clone();
    let first = game.submissions[&order[0]].clone();
    let second = game.submissions[&order[1]].clone();
    let fourth = game.submissions[&order[3]].clone();

    game.player_left(order[2], &mut GameCtx::default());
    let view = game.view_for(1);
    assert_eq!(view["cards_to_judge"], json!([first]));
    assert_eq!(view["in_bag"], 2);

    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();
    assert_eq!(
        game.view_for(1)["cards_to_judge"],
        json!([first, second, fourth])
    );
    assert_eq!(game.view_for(1)["in_bag"], 0);
}

#[test]
fn the_judge_can_show_the_answers_when_the_last_card_in_the_bag_leaves() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();

    game.player_left(game.reading_order[2], &mut GameCtx::default());
    assert_eq!(game.phase, Phase::Reading, "the judge can still read");
    assert_eq!(game.view_for(1)["in_bag"], 0);

    show_all(&mut game, 1).unwrap();
    assert_eq!(game.phase, Phase::Judging);
}

#[test]
fn the_auto_pull_carries_on_after_a_leaver() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    pull(&mut game, 1).unwrap();
    game.player_left(game.reading_order[2], &mut GameCtx::default());

    // Nothing left in the bag, so the waiting timer shows the answers
    game.handle_timer(game.pull_timer_id(), &mut GameCtx::default());
    assert_eq!(game.phase, Phase::Judging);
}

#[test]
fn the_round_is_skipped_when_the_only_answer_leaves_after_being_pulled() {
    let mut game = started_game(&[1, 2, 3, 4, 5]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();
    game.handle_timer(game.timer_id(Timer::Submit), &mut GameCtx::default());
    pull(&mut game, 1).unwrap();

    let mut ctx = GameCtx::default();
    game.player_left(2, &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_skipped" }));
    assert_eq!(game.round, 2);
    assert_eq!(game.pulled, 0);
}

#[test]
fn a_leavers_answer_cant_be_picked() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let leaver = game.reading_order[0];
    let their_card = game.submissions[&leaver][0].id;

    game.player_left(leaver, &mut GameCtx::default());
    let result = pick(&mut game, 1, their_card);
    assert_eq!(result.err().map(|error| error.code), Some("not_a_choice"));
    assert_eq!(game.winner, None);
}

#[test]
fn the_game_ending_mid_reading_stops_the_auto_pull() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());

    game.player_left(3, &mut GameCtx::default());
    assert!(game.is_finished());

    let mut ctx = GameCtx::default();
    game.handle_timer(game.pull_timer_id(), &mut ctx);
    assert!(ctx.outgoing.is_empty());
    assert!(game.is_finished());
}

#[test]
fn the_player_after_a_leaving_judge_judges_next() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);
    assert_eq!(game.judge, Some(2));

    game.player_left(2, &mut GameCtx::default());
    let mut judges = vec![game.judge];
    for _ in 0..3 {
        all_answers_out(&mut game, &mut GameCtx::default());
        finish_round(&mut game);
        judges.push(game.judge);
    }
    assert_eq!(judges, [Some(3), Some(4), Some(1), Some(3)]);
}

#[test]
fn a_judge_leaving_during_the_reveal_keeps_the_point() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    let mut ctx = GameCtx::default();
    game.player_left(1, &mut ctx);
    assert!(ctx.outgoing.is_empty(), "the round already ended");
    assert_eq!(game.scores[&2], 1);

    game.handle_timer(game.timer_id(Timer::Reveal), &mut GameCtx::default());
    assert_eq!(game.judge, Some(2));
}

#[test]
fn a_leavers_cards_are_discarded() {
    let mut game = started_game(&[1, 2, 3, 4]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();

    game.player_left(2, &mut GameCtx::default());
    assert_eq!(
        game.discard.len(),
        HAND_SIZE,
        "the played card and the rest of the hand"
    );
    assert_eq!(cards_in_play(&game), 140);
}
