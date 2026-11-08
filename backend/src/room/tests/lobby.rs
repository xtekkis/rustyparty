// The lobby: ready, settings, kick and ban
use super::*;

#[test]
fn ready_only_changes_in_the_lobby() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    room.state = RoomState::Voting {
        votes: HashMap::new(),
        number: 1,
        ends_at: Instant::now(),
    };
    room.set_ready(1, true);
    assert_eq!(error_code(phones[0].try_recv().ok()), "not_in_lobby");
}

#[test]
fn points_to_win_only_change_in_the_lobby() {
    let mut room = Room::new("KXTP".to_string());
    let (connection, mut messages) = mpsc::unbounded_channel();
    let alex = room.add_player("Alex".to_string(), None, connection).id;

    room.state = RoomState::Voting {
        votes: HashMap::new(),
        number: 1,
        ends_at: Instant::now(),
    };
    room.set_points_to_win(alex, 10);
    assert_eq!(error_code(messages.try_recv().ok()), "not_in_lobby");
    assert_eq!(room.settings.points_to_win, 7);

    room.state = RoomState::Lobby;
    room.set_points_to_win(alex, 10);
    assert_eq!(room.settings.points_to_win, 10);
}

#[tokio::test]
async fn only_the_host_can_kick() {
    let (room, _rooms) = spawn_room();
    join(&room, "Alex", None).await;
    let (sam, mut sam_messages) = join(&room, "Sam", None).await;
    let sam = sam.unwrap();
    // Skip Sam's welcome and lobby
    sam_messages.recv().await;
    sam_messages.recv().await;

    room.send(RoomCommand::Remove {
        player_id: sam,
        target_id: 1,
        ban: false,
    })
    .await;
    assert_eq!(error_code(sam_messages.recv().await), "not_host");
    assert_eq!(player_count(&room).await, 2);
}

#[test]
fn lobby_shows_ready_state() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    room.add_player("Sam".to_string(), None, test_sender());
    room.player_mut(alex).expect("Alex is in the room").ready = true;

    let ServerMessage::Lobby { players, .. } = room.lobby_message() else {
        panic!("expected a lobby message");
    };
    let ready: Vec<bool> = players.iter().map(|player| player.ready).collect();
    assert_eq!(ready, [true, false]);
}

#[test]
fn points_to_win_starts_at_7() {
    let room = Room::new("KXTP".to_string());
    assert_eq!(room.settings.points_to_win, 7);
}

#[test]
fn points_to_win_limits() {
    assert!(!is_valid_points_to_win(2));
    assert!(is_valid_points_to_win(3));
    assert!(is_valid_points_to_win(15));
    assert!(!is_valid_points_to_win(16));
}

#[test]
fn player_can_be_removed() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    room.add_player("Sam".to_string(), None, test_sender());

    assert!(room.remove_player(alex));
    assert_eq!(room.players.len(), 1);
    assert!(!room.has_nickname("Alex"));
    assert!(!room.remove_player(alex));
}

#[test]
fn host_passes_to_next_earliest_player() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
    let kim = room.add_player("Kim".to_string(), None, test_sender()).id;

    room.remove_player(kim);
    assert_eq!(room.host, Some(alex), "a guest leaving keeps the host");

    room.remove_player(alex);
    assert_eq!(room.host, Some(sam));

    room.remove_player(sam);
    assert_eq!(room.host, None, "an empty room has no host");
}

#[test]
fn kicked_player_is_told_and_can_rejoin() {
    let mut room = Room::new("KXTP".to_string());
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let sam = room.add_player("Sam".to_string(), None, sender);
    let (sam_id, sam_token) = (sam.id, sam.token.clone());

    assert!(room.kick_player(sam_id));
    assert!(matches!(receiver.try_recv(), Ok(ServerMessage::Kicked)));
    assert!(room.players.is_empty());
    assert!(!room.is_banned(&sam_token));
    assert!(!room.kick_player(sam_id));
}

#[test]
fn banned_player_is_told_and_blocked() {
    let mut room = Room::new("KXTP".to_string());
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let sam = room.add_player("Sam".to_string(), None, sender);
    let (sam_id, sam_token) = (sam.id, sam.token.clone());

    assert!(room.ban_player(sam_id));
    assert!(matches!(receiver.try_recv(), Ok(ServerMessage::Banned)));
    assert!(room.players.is_empty());
    assert!(room.is_banned(&sam_token));
    assert!(!room.is_banned("some-other-token"));
    assert!(!room.ban_player(sam_id));
}
