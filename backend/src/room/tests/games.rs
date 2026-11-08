// Running any game in a room, then the results and back to the lobby
use super::*;

#[tokio::test(start_paused = true)]
async fn finished_game_shows_results_then_the_lobby() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, mut reminders) = mpsc::channel(8);
    let me = commands.downgrade();
    room.start_game(&DUMMY_INFO, &me);

    // Three presses win the dummy game
    for _ in 0..3 {
        room.game_action(1, json!({ "kind": "press" }), &me);
    }
    room.finish_game_if_over(&me);
    assert!(matches!(room.state, RoomState::Results { number: 1 }));
    let mut got_results = false;
    while let Ok(message) = phones[1].try_recv() {
        got_results |= matches!(message, ServerMessage::Results { ends_in: 10 });
    }
    assert!(got_results);

    // After 21 seconds only the results reminder is due, the game's 30 second limit isn't
    tokio::time::sleep(RESULTS_DURATION + Duration::from_secs(1)).await;
    while let Ok(reminder) = reminders.try_recv() {
        room.handle(reminder, &me);
    }
    assert!(matches!(room.state, RoomState::Lobby));
    assert!(matches!(
        phones[1].try_recv(),
        Ok(ServerMessage::Lobby { .. })
    ));
}

#[tokio::test]
async fn after_a_game_nobody_is_ready_and_no_vote_starts() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();

    // A whole round: everyone ready, vote, game, results
    start_vote(&mut room, &me);
    for player_id in 1..=3 {
        room.vote(player_id, "dummy");
    }
    room.end_vote_if_everyone_voted(&me);
    for _ in 0..3 {
        room.game_action(1, json!({ "kind": "press" }), &me);
    }
    room.finish_game_if_over(&me);
    room.handle(RoomCommand::ResultsOver { number: 1 }, &me);

    assert!(matches!(room.state, RoomState::Lobby));
    assert!(room.players.iter().all(|player| !player.ready));
    room.start_vote_if_ready(&me);
    assert!(
        matches!(room.state, RoomState::Lobby),
        "no vote until they're ready"
    );

    let mut last_lobby = None;
    while let Ok(message) = phones[2].try_recv() {
        if let ServerMessage::Lobby { players, .. } = message {
            last_lobby = Some(players);
        }
    }
    let players = last_lobby.expect("a lobby update");
    assert!(players.iter().all(|player| !player.ready));
}

#[tokio::test]
async fn unfinished_game_keeps_going() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    room.game_action(1, json!({ "kind": "press" }), &me);

    room.finish_game_if_over(&me);
    assert!(matches!(room.state, RoomState::InGame { .. }));
}

#[tokio::test]
async fn old_results_timer_is_ignored() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.state = RoomState::Results { number: 2 };

    room.handle(RoomCommand::ResultsOver { number: 1 }, &me);
    assert!(matches!(room.state, RoomState::Results { number: 2 }));
}

#[tokio::test]
async fn everyone_gets_their_own_view_after_each_game_event() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();

    room.start_game(&DUMMY_INFO, &me);
    for phone in &mut phones {
        assert_eq!(last_game_view(phone), Some(json!({ "presses": 0 })));
    }

    room.game_action(1, json!({ "kind": "press" }), &me);
    assert_eq!(
        last_game_view(&mut phones[0]),
        Some(json!({ "presses": 1 }))
    );
    // Sam and Kim get a fresh view too, still showing only their own presses
    assert_eq!(
        last_game_view(&mut phones[1]),
        Some(json!({ "presses": 0 }))
    );
    assert_eq!(
        last_game_view(&mut phones[2]),
        Some(json!({ "presses": 0 }))
    );
}

#[tokio::test]
async fn refused_action_sends_no_views() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    last_game_view(&mut phones[1]);

    room.game_action(1, json!({ "kind": "dance" }), &me);
    assert_eq!(last_game_view(&mut phones[1]), None);
}

#[tokio::test]
async fn refused_game_action_is_explained() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    while phones[0].try_recv().is_ok() {}

    room.game_action(1, json!({ "kind": "dance" }), &me);
    assert_eq!(error_code(phones[0].try_recv().ok()), "unknown_action");
}

#[test]
fn game_action_outside_a_game_is_refused() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    while phones[0].try_recv().is_ok() {}

    room.game_action(1, json!({ "kind": "press" }), &me);
    assert_eq!(error_code(phones[0].try_recv().ok()), "not_in_game");
}

#[tokio::test(start_paused = true)]
async fn starting_a_game_runs_its_start() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, mut receiver) = mpsc::channel(8);
    room.start_game(&DUMMY_INFO, &commands.downgrade());

    assert!(matches!(room.state, RoomState::InGame { number: 1, .. }));
    for phone in &mut phones {
        let mut got_started = false;
        while let Ok(message) = phone.try_recv() {
            if let ServerMessage::GameEvent { event } = message {
                got_started = event == json!({ "event": "started" });
            }
        }
        assert!(got_started, "every player hears the game start");
    }

    // The dummy game's 30 second time limit was scheduled for game 1
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert!(matches!(
        receiver.try_recv(),
        Ok(RoomCommand::GameTimer {
            game_number: 1,
            timer: 1
        })
    ));
}

#[tokio::test]
async fn each_game_gets_a_new_number() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, _receiver) = mpsc::channel(8);
    room.start_game(&DUMMY_INFO, &commands.downgrade());
    room.start_game(&DUMMY_INFO, &commands.downgrade());
    assert!(matches!(room.state, RoomState::InGame { number: 2, .. }));
}

#[tokio::test(start_paused = true)]
async fn game_timer_fires_after_its_delay() {
    let mut room = Room::new("KXTP".to_string());
    let (commands, mut receiver) = mpsc::channel(8);

    let mut ctx = GameCtx::default();
    ctx.start_timer(7, Duration::from_secs(60));
    room.run_game_ctx(ctx, 1, &commands.downgrade());

    tokio::time::sleep(Duration::from_secs(59)).await;
    assert!(receiver.try_recv().is_err(), "not yet");
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(matches!(
        receiver.try_recv(),
        Ok(RoomCommand::GameTimer {
            game_number: 1,
            timer: 7
        })
    ));
}

#[test]
fn game_timer_only_reaches_the_game_that_asked() {
    let mut room = Room::new("KXTP".to_string());
    let (connection, mut messages) = mpsc::unbounded_channel();
    room.add_player("Alex".to_string(), None, connection);
    room.state = RoomState::InGame {
        game: Box::new(DummyGame::default()),
        number: 2,
        ends_at: None,
    };
    let (commands, _receiver) = mpsc::channel(8);
    let me = commands.downgrade();

    // A timer from an earlier game is ignored
    room.handle(
        RoomCommand::GameTimer {
            game_number: 1,
            timer: 1,
        },
        &me,
    );
    assert!(messages.try_recv().is_err());

    room.handle(
        RoomCommand::GameTimer {
            game_number: 2,
            timer: 1,
        },
        &me,
    );
    assert!(matches!(
        messages.try_recv(),
        Ok(ServerMessage::GameEvent { event }) if event == json!({ "event": "time_up" })
    ));
}

#[tokio::test]
async fn the_game_hears_when_someone_leaves_for_good() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    last_game_event(&mut phones[0]);

    room.leave(4, &me);
    assert_eq!(
        last_game_event(&mut phones[0]),
        Some(json!({ "event": "left", "player": 4 }))
    );

    room.remove_by_host(1, 3, false, &me);
    room.finish_game_if_over(&me);
    // Kim was kicked, two players are too few, so the game ends
    assert!(matches!(room.state, RoomState::Results { .. }));
}

#[tokio::test]
async fn the_game_hears_when_a_grace_period_runs_out() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    let since = Instant::now();
    room.player_mut(4)
        .expect("Lee is in the room")
        .disconnected_since = Some(since);
    last_game_event(&mut phones[0]);

    room.handle(
        RoomCommand::GraceOver {
            player_id: 4,
            since,
        },
        &me,
    );
    assert_eq!(
        last_game_event(&mut phones[0]),
        Some(json!({ "event": "left", "player": 4 }))
    );
}

#[tokio::test]
async fn late_joiner_is_passed_to_the_game() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);

    let (connection, mut messages) = mpsc::unbounded_channel();
    let lee = room
        .join("Lee".to_string(), None, None, connection, &me)
        .expect("Lee joins");
    assert_eq!(last_game_view(&mut messages), Some(json!({ "presses": 0 })));

    // The dummy game lets late joiners play at once
    room.game_action(lee, json!({ "kind": "press" }), &me);
    assert_eq!(last_game_view(&mut messages), Some(json!({ "presses": 1 })));
}

#[test]
fn game_messages_reach_everyone() {
    let (room, mut phones) = room_with(&["Alex", "Sam"]);
    let mut ctx = GameCtx::default();
    ctx.broadcast(json!({ "round": 1 }));
    room.send_game_messages(ctx.outgoing);

    for phone in &mut phones {
        assert!(matches!(
            phone.try_recv(),
            Ok(ServerMessage::GameEvent { event }) if event == json!({ "round": 1 })
        ));
    }
}
