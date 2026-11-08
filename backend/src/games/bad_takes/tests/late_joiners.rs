// Players who join mid-game wait for a seat
use super::*;

#[test]
fn a_waiting_player_keeps_the_game_going() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_joined(4, &mut GameCtx::default());
    game.player_left(3, &mut GameCtx::default());
    assert!(!game.is_finished());

    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);
    assert_eq!(game.players, [1, 2, 4]);
}

#[test]
fn a_late_joiner_waits_for_the_next_round() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_joined(4, &mut GameCtx::default());

    let view = game.view_for(4);
    assert_eq!(view["hand"], json!(null));
    assert_eq!(view["waiting"], json!([4]));
    let result = submit(&mut game, 4, &[1]);
    assert_eq!(result.err().map(|error| error.code), Some("not_playing"));

    // The round doesn't wait for them either
    all_answers_out(&mut game, &mut GameCtx::default());
    assert_eq!(game.phase, Phase::Judging);
}

#[test]
fn a_late_joiner_is_dealt_in_at_the_next_round() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_joined(4, &mut GameCtx::default());
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    assert_eq!(game.hands[&4].len(), HAND_SIZE);
    assert_eq!(game.scores[&4], 0);
    assert!(game.waiting.is_empty());
    assert_eq!(cards_in_play(&game), 140);
}

#[test]
fn a_late_joiner_judges_last() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_joined(4, &mut GameCtx::default());
    let mut judges = vec![game.judge];
    for _ in 0..4 {
        all_answers_out(&mut game, &mut GameCtx::default());
        finish_round(&mut game);
        judges.push(game.judge);
    }
    assert_eq!(judges, [Some(1), Some(2), Some(3), Some(4), Some(1)]);
}

#[test]
fn a_full_table_keeps_late_joiners_waiting() {
    let mut game = started_game(&[1, 2, 3, 4]);
    game.max_players = 4;
    game.player_joined(5, &mut GameCtx::default());
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);
    assert_eq!(game.waiting, [5], "no free seat yet");

    // A seat frees up, and they take it at the next round
    game.player_left(3, &mut GameCtx::default());
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);
    assert_eq!(game.players, [1, 2, 4, 5]);
    assert_eq!(game.hands[&5].len(), HAND_SIZE);
}

#[test]
fn a_waiting_player_who_leaves_is_forgotten() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_joined(4, &mut GameCtx::default());
    game.player_left(4, &mut GameCtx::default());
    all_answers_out(&mut game, &mut GameCtx::default());
    finish_round(&mut game);

    assert_eq!(game.players, [1, 2, 3]);
    assert!(!game.hands.contains_key(&4));
}
