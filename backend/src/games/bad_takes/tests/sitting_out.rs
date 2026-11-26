// Players who keep missing rounds sit out until they play again
use super::*;

// The playing time runs out
fn time_up(game: &mut BadTakes) {
    let timer = game.timer_id(Timer::Submit);
    game.handle_timer(timer, &mut GameCtx::default());
}

// The next round, with a known pick 1 prompt
fn next_round(game: &mut BadTakes) {
    game.start_round(&mut GameCtx::default());
    game.prompt = Some(starter_prompt(2));
}

// Player 4 misses rounds 1 and 2, judged by players 1 and 2
fn player_4_misses_two_rounds() -> BadTakes {
    let mut game = started_game(&[1, 2, 3, 4]);
    play(&mut game, 2);
    play(&mut game, 3);
    time_up(&mut game);
    next_round(&mut game);
    play(&mut game, 1);
    play(&mut game, 3);
    time_up(&mut game);
    game
}

#[test]
fn missing_one_round_isnt_enough() {
    let mut game = started_game(&[1, 2, 3, 4]);
    play(&mut game, 2);
    play(&mut game, 3);
    time_up(&mut game);
    assert!(!game.sitting_out.contains(&4));

    // Playing in the next round starts the count again
    next_round(&mut game);
    play(&mut game, 4);
    assert!(!game.missed.contains_key(&4));
}

#[test]
fn two_missed_rounds_in_a_row_sit_a_player_out() {
    let mut game = player_4_misses_two_rounds();
    assert!(game.sitting_out.contains(&4));

    // Round 3: nobody waits for player 4
    next_round(&mut game);
    assert_eq!(game.judge, Some(3));
    play(&mut game, 1);
    play(&mut game, 2);
    assert_eq!(game.phase, Phase::Reading);
    assert_eq!(
        game.hands[&4].len(),
        HAND_SIZE,
        "still seated with their cards"
    );
}

#[test]
fn a_player_sitting_out_isnt_judge() {
    let mut game = player_4_misses_two_rounds();
    next_round(&mut game);
    // After player 3 it would be player 4's turn
    next_round(&mut game);
    assert_eq!(game.judge, Some(1));
}

#[test]
fn playing_a_card_brings_them_straight_back() {
    let mut game = player_4_misses_two_rounds();
    next_round(&mut game);
    play(&mut game, 4);
    assert!(!game.sitting_out.contains(&4));

    // Back in the judge rotation too: after player 3 it's player 4's turn again
    next_round(&mut game);
    assert_eq!(game.judge, Some(4));
}

#[test]
fn coming_back_online_brings_them_back() {
    let mut game = player_4_misses_two_rounds();
    game.player_back(4, &mut GameCtx::default());
    assert!(!game.sitting_out.contains(&4));
    assert!(!game.missed.contains_key(&4));
}
