// Picking the winner and scoring the point
use super::*;

#[test]
fn the_judge_picks_a_winner() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let kims_card = game.submissions[&3][0].id;
    pick(&mut game, 1, kims_card).unwrap();
    assert_eq!(game.winner, Some(3));
    assert_eq!(game.phase, Phase::RoundOver);

    // One pick per round
    let sams_card = game.submissions[&2][0].id;
    let again = pick(&mut game, 1, sams_card);
    assert_eq!(again.err().map(|error| error.code), Some("not_judging"));
    assert_eq!(game.winner, Some(3));
}

#[test]
fn the_winner_gets_a_point_and_everyone_is_told() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());
    assert_eq!(
        game.view_for(2)["winner"],
        json!(null),
        "not before the pick"
    );

    let kims_cards = game.submissions[&3].clone();
    let ctx = pick(&mut game, 1, kims_cards[0].id).unwrap();

    assert_eq!(ctx.outgoing.len(), 1);
    assert_eq!(
        ctx.outgoing[0],
        json!({
            "event": "winner_revealed",
            "winner": 3,
            "cards": kims_cards,
            "points": 1,
            "last_round": false,
            "judge_timed_out": false,
        })
    );
    let view = game.view_for(2);
    assert_eq!(view["scores"], json!({ "1": 0, "2": 0, "3": 1, "4": 0 }));
    assert_eq!(view["winner"], 3);
}

#[test]
fn points_add_up_across_rounds() {
    let mut game = started_game(&[1, 2, 3]);
    game.scores.insert(2, 4);
    all_answers_out(&mut game, &mut GameCtx::default());

    let sams_card = game.submissions[&2][0].id;
    let ctx = pick(&mut game, 1, sams_card).unwrap();
    assert_eq!(game.scores[&2], 5);
    assert_eq!(ctx.outgoing[0]["points"], 5);
}

#[test]
fn views_show_the_winning_cards_once_they_are_picked() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_cards = game.submissions[&2].clone();
    assert_eq!(game.view_for(3)["winning_cards"], json!(null));

    pick(&mut game, 1, sams_cards[0].id).unwrap();
    assert_eq!(game.view_for(3)["winning_cards"], json!(sams_cards));

    // Gone again when the next round starts
    game.handle_timer(game.timer_id(Timer::Reveal), &mut GameCtx::default());
    assert_eq!(game.view_for(3)["winning_cards"], json!(null));
}

#[test]
fn the_judge_has_45_seconds() {
    let mut game = started_game(&[1, 2, 3]);
    let mut ctx = GameCtx::default();
    all_answers_out(&mut game, &mut ctx);
    assert_eq!(
        ctx.timers.last(),
        Some(&(game.timer_id(Timer::Judge), Duration::from_secs(45)))
    );
}

#[test]
fn a_slow_judge_gives_the_point_to_a_random_answer() {
    let mut game = started_game(&[1, 2, 3, 4]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Judge), &mut ctx);
    let winner = game.winner.expect("someone won");
    assert!([2, 3, 4].contains(&winner));
    assert_eq!(game.scores[&winner], 1);
    assert_eq!(game.phase, Phase::RoundOver);
    assert_eq!(ctx.outgoing[0]["judge_timed_out"], true);
    assert_eq!(
        ctx.timers,
        [(game.timer_id(Timer::Reveal), REVEAL_DURATION)]
    );
}

#[test]
fn a_pick_in_time_beats_the_judge_timer() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Judge), &mut ctx);
    assert_eq!(
        game.scores.values().sum::<u8>(),
        1,
        "only one point this round"
    );
    assert!(ctx.outgoing.is_empty());
}

#[test]
fn any_card_of_a_pick_2_answer_picks_it() {
    let mut game = started_game(&[1, 2, 3]);
    game.prompt = Some(starter_prompt(5));
    for player in [2, 3] {
        let cards = [game.hands[&player][0].id, game.hands[&player][1].id];
        submit(&mut game, player, &cards).unwrap();
    }

    pull_and_show_all(&mut game, &mut GameCtx::default());
    let sams_second_card = game.submissions[&2][1].id;
    pick(&mut game, 1, sams_second_card).unwrap();
    assert_eq!(game.winner, Some(2));
}

#[test]
fn only_the_judge_can_pick() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let own_card = game.submissions[&2][0].id;
    let result = pick(&mut game, 2, own_card);
    assert_eq!(result.err().map(|error| error.code), Some("not_the_judge"));
    assert_eq!(game.winner, None);
    assert_eq!(game.phase, Phase::Judging);
}

#[test]
fn no_picking_before_everyone_has_played() {
    let mut game = started_game(&[1, 2, 3]);
    let card = game.hands[&2][0].id;
    submit(&mut game, 2, &[card]).unwrap();

    let result = pick(&mut game, 1, card);
    assert_eq!(result.err().map(|error| error.code), Some("not_judging"));
    assert_eq!(game.winner, None);
}

#[test]
fn the_judge_can_only_pick_a_played_card() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());

    // A card still in Sam's hand, and one from the judge's own hand
    for card in [game.hands[&2][0].id, game.hands[&1][0].id] {
        let result = pick(&mut game, 1, card);
        assert_eq!(result.err().map(|error| error.code), Some("not_a_choice"));
    }
    assert_eq!(game.winner, None);
}
