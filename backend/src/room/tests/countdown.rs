// The seconds left on a game's countdown
use super::*;

// The time left in the last game view this phone got, if it got one
fn view_ends_in(messages: &mut UnboundedReceiver<ServerMessage>) -> Option<Option<u64>> {
    drain(messages)
        .into_iter()
        .rev()
        .find_map(|message| match message {
            ServerMessage::GameView { ends_in, .. } => Some(ends_in),
            _ => None,
        })
}

#[tokio::test(start_paused = true)]
async fn game_views_say_how_long_the_countdown_has_left() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, _reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(60)));

    // Half a second gone still shows 60
    tokio::time::sleep(Duration::from_millis(500)).await;
    room.send_game_views();
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(60)));

    tokio::time::sleep(Duration::from_secs(10)).await;
    room.send_game_views();
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(50)));
}

#[tokio::test(start_paused = true)]
async fn a_phone_coming_back_gets_the_time_left() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, _reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);
    tokio::time::sleep(Duration::from_secs(20)).await;

    let token = room.players[1].token.clone();
    let (connection, mut new_phone) = mpsc::unbounded_channel();
    room.join("Sam".to_string(), None, Some(token), connection, &me);
    assert_eq!(view_ends_in(&mut new_phone), Some(Some(40)));
}

#[tokio::test(start_paused = true)]
async fn the_countdown_follows_the_latest_timer() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, _reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);
    for player_id in [2, 3] {
        let (_, view) = game_messages(&mut phones[player_id as usize - 1]);
        play_first_cards(&mut room, player_id, &view, &me);
    }
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(15)), "the bag");

    for _ in 0..2 {
        room.game_action(1, json!({ "kind": "pull" }), &me);
    }
    room.game_action(1, json!({ "kind": "show_all" }), &me);
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(45)), "the pick");

    let (_, view) = game_messages(&mut phones[1]);
    let card = view["cards_to_judge"][0][0]["id"].clone();
    room.game_action(1, json!({ "kind": "pick", "card": card }), &me);
    assert_eq!(view_ends_in(&mut phones[0]), Some(Some(4)), "the winner");
}

#[tokio::test(start_paused = true)]
async fn a_finished_game_has_no_countdown() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, _reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);

    // Too few players left, with most of the 60 seconds still to go
    room.leave(3, &me);
    assert_eq!(view_ends_in(&mut phones[0]), Some(None));
}

#[tokio::test]
async fn a_game_with_no_timer_has_no_countdown() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    room.state = RoomState::InGame {
        game: Box::new(DummyGame::default()),
        number: 1,
        ends_at: None,
    };

    room.send_game_views();
    assert_eq!(view_ends_in(&mut phones[0]), Some(None));
}
