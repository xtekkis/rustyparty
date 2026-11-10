// The game vote
use super::*;

#[tokio::test(start_paused = true)]
async fn vote_state_says_when_voting_ends() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    start_vote(&mut room, &me);

    let mut ends_in = None;
    while let Ok(message) = phones[0].try_recv() {
        if let ServerMessage::VoteState {
            ends_in: seconds, ..
        } = message
        {
            ends_in = Some(seconds);
        }
    }
    assert_eq!(ends_in, Some(30));
}

#[tokio::test]
async fn vote_ends_when_everyone_voted() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    start_vote(&mut room, &me);

    room.vote(1, "dummy");
    room.vote(2, "dummy");
    room.end_vote_if_everyone_voted(&me);
    assert!(
        matches!(room.state, RoomState::Voting { .. }),
        "Kim hasn't voted"
    );

    room.vote(3, "dummy");
    room.end_vote_if_everyone_voted(&me);
    assert!(matches!(room.state, RoomState::InGame { .. }));

    let mut result = None;
    while let Ok(message) = phones[1].try_recv() {
        if let ServerMessage::VoteResult { game, tie_break } = message {
            result = Some((game, tie_break));
        }
    }
    assert_eq!(result, Some(("dummy".to_string(), false)));
}

#[tokio::test(start_paused = true)]
async fn vote_ends_after_30_seconds() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, mut reminders) = mpsc::channel(8);
    let me = commands.downgrade();
    start_vote(&mut room, &me);
    room.vote(1, "dummy");

    tokio::time::sleep(Duration::from_secs(29)).await;
    assert!(reminders.try_recv().is_err(), "still voting");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let timeout = reminders.try_recv().expect("the time limit");
    room.handle(timeout, &me);

    // Alex's single vote is enough to win
    assert!(matches!(room.state, RoomState::InGame { .. }));
}

#[tokio::test]
async fn nobody_voting_goes_back_to_the_lobby() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    start_vote(&mut room, &me);

    room.handle(RoomCommand::VoteTimeout { number: 1 }, &me);
    assert!(matches!(room.state, RoomState::Lobby));
    assert!(room.players.iter().all(|player| !player.ready));
}

#[tokio::test]
async fn old_vote_time_limit_is_ignored() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    start_vote(&mut room, &me);

    room.handle(RoomCommand::VoteTimeout { number: 99 }, &me);
    assert!(matches!(room.state, RoomState::Voting { .. }));
}

#[tokio::test]
async fn starting_the_vote_says_the_room_is_voting() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    for player_id in 1..=3 {
        room.set_ready(player_id, true);
    }
    assert_eq!(last_phase(&mut phones[0]), Some(RoomPhase::Lobby));

    room.start_vote_if_ready(&me);
    assert_eq!(last_phase(&mut phones[0]), Some(RoomPhase::Voting));
}

#[tokio::test]
async fn a_vote_nobody_answers_says_the_room_is_back_in_the_lobby() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    for player_id in 1..=3 {
        room.set_ready(player_id, true);
    }
    room.start_vote_if_ready(&me);
    drain(&mut phones[0]);

    room.handle(RoomCommand::VoteTimeout { number: 1 }, &me);
    assert_eq!(last_phase(&mut phones[0]), Some(RoomPhase::Lobby));
}

#[tokio::test(start_paused = true)]
async fn unanswered_vote_returns_the_room_to_the_lobby() {
    let (room, _rooms) = spawn_room();
    let mut phones = Vec::new();
    for nickname in ["Alex", "Sam", "Kim"] {
        let (player_id, messages) = join(&room, nickname, None).await;
        phones.push(messages);
        room.send(RoomCommand::Ready {
            player_id: player_id.unwrap(),
            ready: true,
        })
        .await;
    }

    tokio::time::sleep(VOTE_DURATION + Duration::from_secs(1)).await;
    player_count(&room).await;

    // The last thing Alex hears is the lobby, with nobody ready
    let mut last = None;
    while let Ok(message) = phones[0].try_recv() {
        last = Some(message);
    }
    let Some(ServerMessage::Lobby { players, .. }) = last else {
        panic!("expected the lobby");
    };
    assert!(players.iter().all(|player| !player.ready));
}

fn got_vote_state(messages: &mut UnboundedReceiver<ServerMessage>) -> bool {
    // Skip lobby updates
    while let Ok(message) = messages.try_recv() {
        if matches!(message, ServerMessage::VoteState { .. }) {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn vote_starts_when_everyone_is_ready() {
    let (_commands, me) = test_me();
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    room.set_ready(1, true);
    room.set_ready(2, true);
    room.start_vote_if_ready(&me);
    assert!(
        matches!(room.state, RoomState::Lobby),
        "Kim isn't ready yet"
    );

    room.set_ready(3, true);
    room.start_vote_if_ready(&me);
    assert!(matches!(room.state, RoomState::Voting { .. }));
    for phone in &mut phones {
        assert!(got_vote_state(phone));
    }
}

#[tokio::test]
async fn two_players_are_not_enough() {
    let (_commands, me) = test_me();
    let (mut room, _phones) = room_with(&["Alex", "Sam"]);
    room.set_ready(1, true);
    room.set_ready(2, true);
    room.start_vote_if_ready(&me);
    assert!(matches!(room.state, RoomState::Lobby));
}

#[tokio::test]
async fn disconnected_player_does_not_hold_up_the_vote() {
    let (_commands, me) = test_me();
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
    room.player_mut(4)
        .expect("Lee is in the room")
        .disconnected_since = Some(Instant::now());
    for player_id in 1..=3 {
        room.set_ready(player_id, true);
    }
    room.start_vote_if_ready(&me);
    assert!(matches!(room.state, RoomState::Voting { .. }));
}

// The votes in the newest vote_state this phone got
fn latest_votes(
    messages: &mut UnboundedReceiver<ServerMessage>,
) -> Option<BTreeMap<&'static str, Vec<u32>>> {
    let mut latest = None;
    while let Ok(message) = messages.try_recv() {
        if let ServerMessage::VoteState { votes, .. } = message {
            latest = Some(votes);
        }
    }
    latest
}

fn voting_room(nicknames: &[&str]) -> (Room, Vec<UnboundedReceiver<ServerMessage>>) {
    let (mut room, phones) = room_with(nicknames);
    room.state = RoomState::Voting {
        votes: HashMap::new(),
        number: 1,
        ends_at: Instant::now(),
    };
    (room, phones)
}

#[test]
fn votes_are_shown_to_everyone() {
    let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
    room.vote(3, "dummy");
    room.vote(1, "dummy");

    for phone in &mut phones {
        let votes = latest_votes(phone).expect("a vote_state");
        assert_eq!(votes, BTreeMap::from([("dummy", vec![1, 3])]));
    }
}

#[test]
fn wrong_votes_are_refused() {
    let (mut room, mut phones) = voting_room(&["Alex", "Sam"]);
    room.vote(1, "chess");
    assert_eq!(error_code(phones[0].try_recv().ok()), "unknown_game");
    // The dummy game needs 3 players
    room.vote(1, "dummy");
    assert_eq!(error_code(phones[0].try_recv().ok()), "game_not_available");

    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    room.vote(1, "dummy");
    assert_eq!(error_code(phones[0].try_recv().ok()), "not_voting");
}

#[test]
fn leaving_takes_back_the_vote() {
    let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim", "Lee"]);
    let (_commands, me) = test_me();
    room.vote(4, "dummy");
    room.leave(4, &me);

    let votes = latest_votes(&mut phones[0]).expect("a vote_state");
    assert!(votes.is_empty());
}

#[test]
fn joining_during_the_vote_shows_the_vote() {
    let (mut room, _phones) = voting_room(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.vote(1, "dummy");

    let (connection, mut messages) = mpsc::unbounded_channel();
    let lee = room.join("Lee".to_string(), None, None, connection, &me);
    assert!(lee.is_some());
    let votes = latest_votes(&mut messages).expect("Lee gets the vote");
    assert_eq!(votes, BTreeMap::from([("dummy", vec![1])]));
}

// Two games that both fit 3 to 10 players
static TWO_GAMES: [GameInfo; 2] = [
    GameInfo {
        id: "first",
        name: "First",
        description: "A game for tests",
        min_players: 3,
        max_players: 10,
        create: crate::games::dummy::create,
    },
    GameInfo {
        id: "second",
        name: "Second",
        description: "Another game for tests",
        min_players: 3,
        max_players: 10,
        create: crate::games::dummy::create,
    },
];

#[test]
fn changing_a_vote_moves_it() {
    let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
    room.games = &TWO_GAMES;
    room.vote(1, "first");
    room.vote(2, "first");
    room.vote(1, "second");

    let votes = latest_votes(&mut phones[2]).expect("a vote_state");
    assert_eq!(
        votes,
        BTreeMap::from([("first", vec![2]), ("second", vec![1])])
    );
}

#[test]
fn votes_are_refused_once_voting_ends() {
    let (mut room, mut phones) = voting_room(&["Alex", "Sam", "Kim"]);
    room.vote(1, "dummy");
    latest_votes(&mut phones[0]);

    room.state = RoomState::Lobby;
    room.vote(1, "dummy");
    assert_eq!(error_code(phones[0].try_recv().ok()), "not_voting");
}

#[tokio::test]
async fn ready_players_get_the_vote_through_the_room_task() {
    let (room, _rooms) = spawn_room();
    let mut phones = Vec::new();
    for nickname in ["Alex", "Sam", "Kim"] {
        let (player_id, messages) = join(&room, nickname, None).await;
        phones.push(messages);
        room.send(RoomCommand::Ready {
            player_id: player_id.unwrap(),
            ready: true,
        })
        .await;
    }
    // Ask something so every command above has been handled
    player_count(&room).await;
    for phone in &mut phones {
        assert!(got_vote_state(phone));
    }
}

#[test]
fn vote_marks_games_that_dont_fit() {
    static TEST_GAMES: [GameInfo; 2] = [
        GameInfo {
            id: "small",
            name: "Small",
            description: "For 3 to 4 players",
            min_players: 3,
            max_players: 4,
            create: crate::games::dummy::create,
        },
        GameInfo {
            id: "big",
            name: "Big",
            description: "For 5 to 8 players",
            min_players: 5,
            max_players: 8,
            create: crate::games::dummy::create,
        },
    ];
    let mut room = Room::new("KXTP".to_string());
    room.games = &TEST_GAMES;
    for nickname in ["Alex", "Sam", "Kim"] {
        room.add_player(nickname.to_string(), None, test_sender());
    }

    let ServerMessage::VoteState { options, .. } = room.vote_message() else {
        panic!("expected a vote message");
    };
    let available: Vec<(&str, bool)> = options
        .iter()
        .map(|option| (option.id, option.available))
        .collect();
    assert_eq!(available, [("small", true), ("big", false)]);
}
