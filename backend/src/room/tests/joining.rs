// Joining a room: nicknames, ids, the host and full rooms
use super::*;

#[tokio::test]
async fn a_full_room_turns_new_players_away() {
    let (mut room, _phones) = full_room();
    let (_commands, me) = test_me();
    let (connection, mut messages) = mpsc::unbounded_channel();

    let joined = room.join("Late".to_string(), None, None, connection, &me);
    assert_eq!(joined, None);
    assert_eq!(error_code(messages.try_recv().ok()), "room_full");
    assert_eq!(room.players.len(), MAX_ROOM_PLAYERS);
}

#[tokio::test]
async fn a_full_room_still_lets_players_back_in() {
    let (mut room, _phones) = full_room();
    let (_commands, me) = test_me();
    let token = room.players[2].token.clone();
    let (connection, _messages) = mpsc::unbounded_channel();

    let joined = room.join("Player 3".to_string(), None, Some(token), connection, &me);
    assert_eq!(joined, Some(3));
}

#[tokio::test]
async fn a_seat_frees_up_when_someone_leaves() {
    let (mut room, _phones) = full_room();
    let (_commands, me) = test_me();
    room.leave(4, &me);
    let (connection, _messages) = mpsc::unbounded_channel();

    let joined = room.join("Late".to_string(), None, None, connection, &me);
    assert!(joined.is_some());
}

#[tokio::test]
async fn join_gets_welcome_then_lobby() {
    let (room, _rooms) = spawn_room();
    let (alex, mut messages) = join(&room, "Alex", None).await;

    assert_eq!(alex, Some(1));
    assert!(matches!(
        messages.recv().await,
        Some(ServerMessage::Welcome { player_id: 1, .. })
    ));
    assert!(matches!(
        messages.recv().await,
        Some(ServerMessage::Lobby { .. })
    ));
}

#[tokio::test]
async fn taken_nickname_is_refused() {
    let (room, _rooms) = spawn_room();
    join(&room, "Alex", None).await;
    let (player_id, mut messages) = join(&room, "alex", None).await;

    assert_eq!(player_id, None);
    assert_eq!(error_code(messages.recv().await), "nickname_taken");
}

#[test]
fn every_game_fits_in_a_room() {
    for game in GAMES.iter() {
        assert!(
            game.max_players <= MAX_ROOM_PLAYERS,
            "{} needs more seats",
            game.id
        );
    }
}

#[test]
fn new_room_has_code_and_no_players() {
    let room = Room::new("KXTP".to_string());
    assert_eq!(room.code, "KXTP");
    assert!(matches!(room.state, RoomState::Lobby));
    assert!(room.players.is_empty());
}

#[test]
fn players_get_increasing_ids() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
    assert_eq!((alex, sam), (1, 2));
    assert_eq!(room.players.len(), 2);
    assert_eq!(room.players[1].nickname, "Sam");
}

#[test]
fn nickname_is_trimmed() {
    assert_eq!(clean_nickname("  Alex  "), Some("Alex".to_string()));
}

#[test]
fn empty_nickname_is_rejected() {
    assert_eq!(clean_nickname(""), None);
    assert_eq!(clean_nickname("    "), None);
}

#[test]
fn nickname_length_limits() {
    assert_eq!(clean_nickname("A"), Some("A".to_string()));
    assert!(clean_nickname(&"a".repeat(16)).is_some());
    assert_eq!(clean_nickname(&"a".repeat(17)), None);
}

#[test]
fn nickname_length_counts_letters_not_bytes() {
    // 16 Greek letters take 32 bytes but are still allowed
    assert!(clean_nickname(&"λ".repeat(16)).is_some());
    assert_eq!(clean_nickname(&"λ".repeat(17)), None);
}

#[test]
fn duplicate_nickname_ignores_case() {
    let mut room = Room::new("KXTP".to_string());
    room.add_player("Alex".to_string(), None, test_sender());
    assert!(room.has_nickname("Alex"));
    assert!(room.has_nickname("aLEX"));
    assert!(!room.has_nickname("Sam"));
}

#[test]
fn first_player_is_host() {
    let mut room = Room::new("KXTP".to_string());
    assert_eq!(room.host, None);
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    room.add_player("Sam".to_string(), None, test_sender());
    assert_eq!(room.host, Some(alex));
}

#[test]
fn find_player_by_id() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    assert_eq!(room.player_mut(alex).map(|player| player.id), Some(alex));
    assert!(room.player_mut(99).is_none());
}

#[test]
fn new_player_is_not_ready() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender());
    assert!(!alex.ready);
}

#[test]
fn broadcast_reaches_every_player() {
    let mut room = Room::new("KXTP".to_string());
    let (alex_sender, mut alex_receiver) = mpsc::unbounded_channel();
    let (sam_sender, mut sam_receiver) = mpsc::unbounded_channel();
    room.add_player("Alex".to_string(), Some("fox".to_string()), alex_sender);
    room.add_player("Sam".to_string(), None, sam_sender);

    room.broadcast(&room.lobby_message());

    for receiver in [&mut alex_receiver, &mut sam_receiver] {
        match receiver.try_recv() {
            Ok(ServerMessage::Lobby { players, .. }) => {
                let names: Vec<&str> = players.iter().map(|p| p.nickname.as_str()).collect();
                assert_eq!(names, ["Alex", "Sam"]);
                assert_eq!(players[0].avatar.as_deref(), Some("fox"));
                assert_eq!(players[1].avatar, None);
            }
            _ => panic!("expected a lobby message"),
        }
    }
}
