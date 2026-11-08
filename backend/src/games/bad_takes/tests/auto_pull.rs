// A judge who stops tapping: cards come out by themselves, faster once they seem away
use super::*;

#[test]
fn answers_come_out_by_themselves_when_the_judge_stops_tapping() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());

    // Both cards come out, each with 6 more seconds before the next step
    for pulled in 1..=2 {
        let answer = game.submissions[&game.reading_order[pulled - 1]].clone();
        let mut ctx = GameCtx::default();
        game.handle_timer(game.pull_timer_id(), &mut ctx);
        assert_eq!(
            ctx.outgoing[0],
            json!({ "event": "answer_pulled", "cards": answer })
        );
        assert_eq!(ctx.timers, [(game.pull_timer_id(), IDLE_PULL_DURATION)]);
        assert_eq!(game.pulled, pulled);
        assert_eq!(game.phase, Phase::Reading);
    }

    // Then they're shown together, and a judge who never tapped gets less time to pick
    let mut ctx = GameCtx::default();
    game.handle_timer(game.pull_timer_id(), &mut ctx);
    assert_eq!(game.phase, Phase::Judging);
    assert_eq!(
        ctx.outgoing[0],
        json!({ "event": "judging_started", "judge_for": 20 })
    );
    assert_eq!(
        ctx.timers,
        [(game.timer_id(Timer::Judge), IDLE_JUDGE_DURATION)]
    );
}

// How long a judge who never taps holds up the round, from the bag to the random winner
fn seconds_held_up_by_an_idle_judge(players: &[PlayerId]) -> u64 {
    let mut game = started_game(players);
    let mut ctx = GameCtx::default();
    everyone_submits(&mut game, &mut ctx);
    let mut seconds = 0;
    while game.phase != Phase::RoundOver {
        let (timer, wait) = *ctx.timers.last().expect("a timer is running");
        seconds += wait.as_secs();
        ctx = GameCtx::default();
        game.handle_timer(timer, &mut ctx);
    }
    seconds
}

#[test]
fn an_idle_judge_holds_up_the_round_for_about_a_minute() {
    assert_eq!(seconds_held_up_by_an_idle_judge(&[1, 2, 3, 4]), 53);
    let full_table: Vec<PlayerId> = (1..=10).collect();
    assert_eq!(seconds_held_up_by_an_idle_judge(&full_table), 89);
}

#[test]
fn a_tap_after_a_missed_one_goes_back_to_15_seconds() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());
    game.handle_timer(game.pull_timer_id(), &mut GameCtx::default());

    let ctx = pull(&mut game, 1).unwrap();
    assert_eq!(ctx.timers, [(game.pull_timer_id(), PULL_DURATION)]);
}

#[test]
fn a_judge_who_tapped_once_keeps_the_full_time_to_pick() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    pull(&mut game, 1).unwrap();

    // The other card comes out by itself, and so does the show all
    game.handle_timer(game.pull_timer_id(), &mut GameCtx::default());
    let mut ctx = GameCtx::default();
    game.handle_timer(game.pull_timer_id(), &mut ctx);
    assert_eq!(ctx.outgoing[0]["judge_for"], 45);
    assert_eq!(ctx.timers, [(game.timer_id(Timer::Judge), JUDGE_DURATION)]);
}

#[test]
fn showing_the_answers_counts_as_a_tap() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    for _ in 0..2 {
        game.handle_timer(game.pull_timer_id(), &mut GameCtx::default());
    }

    let ctx = show_all(&mut game, 1).unwrap();
    assert_eq!(ctx.outgoing[0]["judge_for"], 45);
}

#[test]
fn a_refused_tap_doesnt_count() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());

    // Sam can't pull for the judge, and the judge can't show cards still in the bag
    assert!(pull(&mut game, 2).is_err());
    assert!(show_all(&mut game, 1).is_err());
    assert!(!game.judge_tapped);
}

#[test]
fn an_idle_judge_doesnt_hurry_the_next_one() {
    let mut game = started_game(&[1, 2, 3]);
    let mut ctx = GameCtx::default();
    everyone_submits(&mut game, &mut ctx);

    // Every timer runs out until the next round starts
    while game.round == 1 {
        let (timer, _) = *ctx.timers.last().expect("a timer is running");
        ctx = GameCtx::default();
        game.handle_timer(timer, &mut ctx);
    }

    let mut ctx = GameCtx::default();
    everyone_submits(&mut game, &mut ctx);
    assert_eq!(ctx.timers, [(game.pull_timer_id(), PULL_DURATION)]);
}

#[test]
fn every_round_starts_with_the_full_time() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    game.handle_timer(game.pull_timer_id(), &mut GameCtx::default());
    pull(&mut game, 1).unwrap();
    finish_round(&mut game);

    // The next judge hasn't missed anything, even after the last judge did
    let mut ctx = GameCtx::default();
    everyone_submits(&mut game, &mut ctx);
    assert_eq!(ctx.timers, [(game.pull_timer_id(), PULL_DURATION)]);
    assert!(!game.judge_tapped);
}

#[test]
fn a_tap_gives_the_judge_15_more_seconds() {
    let mut game = started_game(&[1, 2, 3, 4]);
    everyone_submits(&mut game, &mut GameCtx::default());
    let old_timer = game.pull_timer_id();
    pull(&mut game, 1).unwrap();

    let mut ctx = GameCtx::default();
    game.handle_timer(old_timer, &mut ctx);
    assert!(ctx.outgoing.is_empty());
    assert!(ctx.timers.is_empty());
    assert_eq!(game.pulled, 1);
}

#[test]
fn showing_the_answers_stops_the_auto_pull() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.handle_timer(game.pull_timer_id(), &mut ctx);
    assert!(ctx.outgoing.is_empty());
    assert_eq!(game.phase, Phase::Judging);
}

#[test]
fn an_auto_pull_from_an_earlier_round_is_ignored() {
    let mut game = started_game(&[1, 2, 3]);
    everyone_submits(&mut game, &mut GameCtx::default());
    let round_one_timer = game.pull_timer_id();
    finish_round(&mut game);
    everyone_submits(&mut game, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.handle_timer(round_one_timer, &mut ctx);
    assert!(ctx.outgoing.is_empty());
    assert_eq!(game.pulled, 0);
}

#[test]
fn a_full_table_stays_inside_its_rounds_timer_ids() {
    let players: Vec<PlayerId> = (1..=10).collect();
    let mut game = started_game(&players);
    all_answers_out(&mut game, &mut GameCtx::default());

    assert_eq!(
        game.pull_timers, 10,
        "one for the bag, one for each of 9 cards"
    );
    assert!(game.pull_timer_id() < (game.round + 1) * 100);
}
