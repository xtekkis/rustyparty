// How a game ends: reaching the points to win, or too few players
use super::*;

#[test]
fn reaching_the_points_to_win_ends_the_game() {
    let mut game = started_game(&[1, 2, 3]);
    game.scores.insert(2, 6);
    all_answers_out(&mut game, &mut GameCtx::default());

    let sams_card = game.submissions[&2][0].id;
    let ctx = pick(&mut game, 1, sams_card).unwrap();
    assert_eq!(ctx.outgoing[0]["last_round"], true);
    assert!(!game.is_finished(), "everyone sees the winning card first");

    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut ctx);
    assert!(game.is_finished());
    assert_eq!(game.view_for(3)["phase"], "game_over");
    assert_eq!(game.round, 1, "no next round");
    assert_eq!(
        ctx.outgoing[0],
        json!({
            "event": "game_over",
            "reason": "points_to_win",
            "winner": 2,
            "cards": game.submissions[&2],
            "scores": { "1": 0, "2": 7, "3": 0 },
        })
    );
}

#[test]
fn the_last_view_of_the_game_shows_the_winning_cards() {
    let mut game = started_game(&[1, 2, 3]);
    game.scores.insert(2, 6);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_cards = game.submissions[&2].clone();
    pick(&mut game, 1, sams_cards[0].id).unwrap();

    game.handle_timer(game.timer_id(Timer::Reveal), &mut GameCtx::default());
    assert!(game.is_finished());
    assert_eq!(game.view_for(3)["winner"], 2);
    assert_eq!(game.view_for(3)["winning_cards"], json!(sams_cards));
}

#[test]
fn too_few_players_during_the_reveal_means_nobody_won_the_game() {
    let mut game = started_game(&[1, 2, 3]);
    all_answers_out(&mut game, &mut GameCtx::default());
    let sams_card = game.submissions[&2][0].id;
    pick(&mut game, 1, sams_card).unwrap();

    let mut ctx = GameCtx::default();
    game.player_left(3, &mut ctx);
    assert!(game.is_finished());
    assert_eq!(ctx.outgoing[0]["winner"], json!(null));
    assert_eq!(ctx.outgoing[0]["cards"], json!(null));
    assert_eq!(game.view_for(1)["winner"], json!(null));
    assert_eq!(game.view_for(1)["winning_cards"], json!(null));
}

#[test]
fn one_point_short_keeps_the_game_going() {
    let mut game = started_game(&[1, 2, 3]);
    game.scores.insert(2, 5);
    all_answers_out(&mut game, &mut GameCtx::default());

    let sams_card = game.submissions[&2][0].id;
    let ctx = pick(&mut game, 1, sams_card).unwrap();
    assert_eq!(ctx.outgoing[0]["last_round"], false);

    game.handle_timer(game.timer_id(Timer::Reveal), &mut GameCtx::default());
    assert!(!game.is_finished());
    assert_eq!(game.round, 2);
}

#[test]
fn a_game_to_three_ends_after_three_wins() {
    let mut game = started_game(&[1, 2, 3]);
    game.points_to_win = 3;
    // Whoever is read first wins each round, until someone has 3
    while !game.is_finished() {
        all_answers_out(&mut game, &mut GameCtx::default());
        finish_round(&mut game);
    }
    assert_eq!(game.scores.values().max(), Some(&3));
    assert_eq!(game.scores.values().sum::<u8>(), game.round as u8);
}

#[test]
fn fewer_than_three_players_ends_the_game() {
    let mut game = started_game(&[1, 2, 3]);
    game.scores.insert(2, 4);

    let mut ctx = GameCtx::default();
    game.player_left(3, &mut ctx);
    assert!(game.is_finished());
    assert_eq!(
        ctx.outgoing[0],
        json!({
            "event": "game_over",
            "reason": "not_enough_players",
            "winner": null,
            "cards": null,
            "scores": { "1": 0, "2": 4 },
        })
    );
    assert_eq!(ctx.outgoing.len(), 1, "no new round starts");
}

#[test]
fn leaving_after_the_game_is_over_changes_nothing() {
    let mut game = started_game(&[1, 2, 3]);
    game.player_left(3, &mut GameCtx::default());

    let mut ctx = GameCtx::default();
    game.player_left(2, &mut ctx);
    assert!(ctx.outgoing.is_empty(), "game over is only said once");
}
