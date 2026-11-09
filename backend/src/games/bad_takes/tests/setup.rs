// Starting a game: dealing, the first round and the game list entry
use super::*;

#[test]
fn everyone_is_dealt_seven_cards() {
    let game = started_game(&[1, 2, 3]);
    for player in [1, 2, 3] {
        assert_eq!(game.hands[&player].len(), HAND_SIZE);
    }
    assert_eq!(game.answers.len(), 140 - 3 * HAND_SIZE);
}

#[test]
fn no_card_is_dealt_twice() {
    let game = started_game(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let ids: HashSet<_> = game.hands.values().flatten().map(|card| card.id).collect();
    assert_eq!(ids.len(), 10 * HAND_SIZE);
}

#[test]
fn first_player_judges_the_first_round() {
    let cards = deck::starter_deck();
    let mut game = BadTakes::new(
        7,
        cards.prompts.clone(),
        cards.answers.clone(),
        StdRng::seed_from_u64(1),
    );
    let mut ctx = GameCtx::default();
    game.start(&[4, 2, 9], &mut ctx);

    assert_eq!(game.round, 1);
    assert_eq!(game.judge, Some(4));
    let prompt = game.prompt.clone().expect("a prompt is revealed");
    assert_eq!(game.prompts.len(), 45 - 1);

    // Everyone hears the round start
    assert_eq!(ctx.outgoing.len(), 1);
    assert_eq!(
        ctx.outgoing[0],
        json!({
            "event": "round_started",
            "round": 1,
            "judge": 4,
            "prompt": prompt,
        })
    );
    assert_eq!(
        ctx.timers,
        [(game.timer_id(Timer::Submit), Duration::from_secs(60))]
    );
}

#[test]
fn views_show_the_round_judge_and_prompt() {
    let game = started_game(&[1, 2, 3]);
    let view = game.view_for(2);
    assert_eq!(view["round"], 1);
    assert_eq!(view["judge"], 1);
    assert_eq!(view["prompt"], json!(game.prompt));
}

#[test]
fn everyone_starts_on_zero_points() {
    let game = started_game(&[1, 2, 3]);
    assert_eq!(
        game.view_for(2)["scores"],
        json!({ "1": 0, "2": 0, "3": 0 })
    );
}

#[test]
fn unknown_actions_are_refused() {
    let mut game = started_game(&[1, 2, 3]);
    let mut ctx = GameCtx::default();
    let result = game.handle_action(2, json!({ "kind": "dance" }), &mut ctx);
    assert_eq!(result.err().map(|error| error.code), Some("unknown_action"));
}

#[test]
fn max_players_keeps_half_the_answers_in_reserve() {
    assert_eq!(max_players(70), 5);
    assert_eq!(max_players(83), 5);
    assert_eq!(max_players(84), 6);
    assert_eq!(max_players(140), 10);
    // Never more than 10, however big the deck
    assert_eq!(max_players(500), 10);
}

#[test]
fn bad_takes_is_in_the_registry() {
    let info = find_game(&GAMES, "bad_takes").expect("Bad Takes is registered");
    assert_eq!(info.name, "Bad Takes");
    // The 140 answer starter deck allows 3 to 10 players
    assert!(!info.fits(2));
    assert!(info.fits(3));
    assert!(info.fits(10));
    assert!(!info.fits(11));
}

#[test]
fn registry_makes_a_game_with_the_room_settings() {
    let mut game = (info().create)(&RoomSettings { points_to_win: 5 });
    game.start(&[1, 2, 3], &mut GameCtx::default());
    assert_eq!(game.view_for(1)["points_to_win"], 5);
    assert!(!game.is_finished());
}
