// Players whose phone has dropped keep their seat, but nobody waits for them
use super::*;

// This player plays their first card, enough for the pick 1 prompt in started_game
fn play(game: &mut BadTakes, player: PlayerId) {
    let card = [game.hands[&player][0].id];
    submit(game, player, &card).unwrap();
}

#[test]
fn the_round_moves_on_without_a_player_who_is_away() {
    // Player 1 judges, player 4's phone is down
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_away(4, &mut GameCtx::default());

    play(&mut game, 2);
    assert_eq!(game.phase, Phase::Submitting);
    play(&mut game, 3);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(game.reading_order.len(), 2);
}

#[test]
fn going_away_last_moves_the_round_on() {
    let mut game = started_game(&[1, 2, 3, 4]);
    play(&mut game, 2);
    play(&mut game, 3);
    assert_eq!(game.phase, Phase::Submitting, "still waiting for player 4");

    let mut ctx = GameCtx::default();
    game.player_away(4, &mut ctx);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(ctx.outgoing[0]["event"], "reading_started");
}

#[test]
fn a_player_back_in_time_can_play_and_is_waited_for() {
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_away(4, &mut GameCtx::default());
    game.player_back(4, &mut GameCtx::default());

    play(&mut game, 2);
    play(&mut game, 3);
    assert_eq!(
        game.phase,
        Phase::Submitting,
        "player 4 is back, so waited for"
    );
    play(&mut game, 4);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(game.reading_order.len(), 3);
}

#[test]
fn an_answer_played_before_going_away_still_counts() {
    let mut game = started_game(&[1, 2, 3, 4]);
    play(&mut game, 4);
    game.player_away(4, &mut GameCtx::default());

    play(&mut game, 2);
    play(&mut game, 3);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(game.reading_order.len(), 3);
}

#[test]
fn the_only_connected_player_judges_again() {
    let mut game = started_game(&[1, 2, 3, 4]);
    for player in [2, 3, 4] {
        game.player_away(player, &mut GameCtx::default());
    }
    game.start_round(&mut GameCtx::default());
    assert_eq!(game.judge, Some(1));
}

#[test]
fn the_next_judge_skips_a_player_who_is_away() {
    // Player 1 judges now, player 2 is next but their phone is down
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_away(2, &mut GameCtx::default());
    game.start_round(&mut GameCtx::default());
    assert_eq!(game.judge, Some(3));
}

#[test]
fn someone_judges_even_if_every_phone_is_down() {
    let mut game = started_game(&[1, 2, 3, 4]);
    for player in [1, 2, 3, 4] {
        game.player_away(player, &mut GameCtx::default());
    }
    game.start_round(&mut GameCtx::default());
    assert_eq!(game.judge, Some(2));
}

#[test]
fn a_player_away_at_the_start_isnt_the_first_judge() {
    let cards = deck::starter_deck();
    let mut game = BadTakes::new(
        7,
        cards.prompts.clone(),
        cards.answers.clone(),
        StdRng::seed_from_u64(1),
    );
    // The room says who's away just before the game starts
    game.player_away(1, &mut GameCtx::default());
    game.start(&[1, 2, 3], &mut GameCtx::default());
    assert_eq!(game.judge, Some(2));
}

#[test]
fn a_judge_who_drops_gets_a_moment_then_the_round_moves_on() {
    let mut game = started_game(&[1, 2, 3, 4]);
    play(&mut game, 2);

    let mut ctx = GameCtx::default();
    game.player_away(1, &mut ctx);
    let wait = game.timer_id(Timer::JudgeAway);
    assert_eq!(ctx.background_timers, [(wait, JUDGE_AWAY_DURATION)]);
    assert!(
        ctx.timers.is_empty(),
        "the countdown on phones doesn't change"
    );

    let mut ctx = GameCtx::default();
    game.handle_timer(wait, &mut ctx);
    assert_eq!(ctx.outgoing[0], json!({ "event": "round_cancelled" }));
    assert_eq!(game.round, 2);
    assert_eq!(game.judge, Some(2));
    assert_eq!(game.hands[&2].len(), HAND_SIZE, "player 2's card came back");
}

#[test]
fn a_judge_back_in_time_keeps_the_round() {
    let mut game = started_game(&[1, 2, 3, 4]);
    game.player_away(1, &mut GameCtx::default());
    let wait = game.timer_id(Timer::JudgeAway);
    game.player_back(1, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.handle_timer(wait, &mut ctx);
    assert!(ctx.outgoing.is_empty());
    assert_eq!(game.round, 1);
    assert_eq!(game.judge, Some(1));
}
