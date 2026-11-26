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
