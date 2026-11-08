// Between rounds: the reveal, the next judge and skipped rounds
use super::*;

#[test]
fn the_reveal_lasts_four_seconds() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;

    let ctx = pick(&mut game, 1, sams_card).unwrap();
    assert_eq!(
        ctx.timers,
        [(game.timer_id(Timer::Reveal), Duration::from_secs(4))]
    );
}

#[test]
fn the_next_round_starts_after_the_reveal() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut ctx);
    assert_eq!(game.round, 2);
    assert_eq!(game.phase, Phase::Submitting);
    assert_eq!(game.winner, None);
    assert_eq!(game.prompts.len(), 45 - 2);
    assert_eq!(ctx.outgoing[0]["event"], "round_started");
    assert_eq!(ctx.outgoing[0]["round"], 2);
}

#[test]
fn a_late_next_round_timer_does_nothing() {
    let mut game = started_game(&[1, 2, 3]);
    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut ctx);

    assert_eq!(game.round, 1, "the round is still being played");
    assert!(ctx.outgoing.is_empty());
}

#[test]
fn the_judge_rotates_in_join_order() {
    let mut game = started_game(&[4, 2, 9]);
    let mut judges = vec![game.judge];
    for _ in 0..3 {
        all_answers_out(&mut game, &mut GameCtx::default());
        finish_round(&mut game);
        judges.push(game.judge);
    }
    assert_eq!(judges, [Some(4), Some(2), Some(9), Some(4)]);
}

#[test]
fn everyone_hears_who_the_new_judge_is() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut ctx);
    assert_eq!(ctx.outgoing[0]["judge"], 2);
    assert_eq!(game.view_for(3)["judge"], 2);
}

#[test]
fn a_timer_from_an_earlier_round_is_ignored() {
    let mut game = started_game(&[1, 2, 3]);
    let round_one_timer = game.timer_id(Timer::Submit);
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    // Round 1's timer runs out while round 2 is being played
    let card = game.hands[&3][0].id;
    game.prompt = Some(starter_prompt(2));
    submit(&mut game, 3, &[card]).unwrap();
    game.handle_timer(round_one_timer, &mut GameCtx::default());
    assert_eq!(game.round, 2);
    assert_eq!(game.phase, Phase::Submitting);
}

#[test]
fn nobody_playing_skips_the_round() {
    let mut game = started_game(&[1, 2, 3]);
    let hands_before = game.hands.clone();

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Submit), &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_skipped" }));
    assert_eq!(ctx.outgoing[1]["event"], "round_started");
    assert_eq!(game.round, 2);
    assert_eq!(game.judge, Some(2), "the next judge takes over");
    assert_eq!(game.scores.values().sum::<u8>(), 0, "no point");
    assert_eq!(game.hands, hands_before, "everyone keeps their cards");
}
