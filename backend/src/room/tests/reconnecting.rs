// Phones that drop: the grace period, coming back and empty rooms
use super::*;

#[tokio::test]
async fn reconnecting_mid_game_shows_the_game_at_once() {
    let (mut room, _phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    room.start_game(&DUMMY_INFO, &me);
    room.game_action(1, json!({ "kind": "press" }), &me);
    let token = room.players[0].token.clone();

    let (connection, mut messages) = mpsc::unbounded_channel();
    room.join("Alex".to_string(), None, Some(token), connection, &me);
    assert_eq!(last_game_view(&mut messages), Some(json!({ "presses": 1 })));
}

#[tokio::test(start_paused = true)]
async fn a_disconnect_tells_everyone_the_time_left() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (_commands, me) = test_me();
    let sams_connection = room.players[1].sender.clone();

    room.disconnect(2, &sams_connection, &me);
    let players = drain(&mut phones[0])
        .into_iter()
        .rev()
        .find_map(|message| match message {
            ServerMessage::Lobby { players, .. } => Some(players),
            _ => None,
        })
        .expect("a lobby message");
    assert_eq!(players[1].reconnect_in, Some(90));
    assert_eq!(players[0].reconnect_in, None);
}

#[tokio::test(start_paused = true)]
async fn disconnected_player_is_removed_after_grace() {
    let (room, _rooms) = spawn_room();
    let (connection, _messages) = mpsc::unbounded_channel();
    let (reply, player_id) = oneshot::channel();
    room.send(RoomCommand::Join {
        nickname: "Sam".to_string(),
        avatar: None,
        token: None,
        connection: connection.clone(),
        reply,
    })
    .await;
    let sam = player_id.await.unwrap().unwrap();

    room.send(RoomCommand::Disconnect {
        player_id: sam,
        connection,
    })
    .await;
    tokio::time::sleep(DISCONNECT_GRACE - Duration::from_secs(1)).await;
    assert_eq!(
        player_count(&room).await,
        1,
        "still within the grace period"
    );

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(player_count(&room).await, 0);
}

#[tokio::test(start_paused = true)]
async fn empty_room_deletes_itself_after_5_minutes() {
    let (room, rooms) = spawn_room();

    tokio::time::sleep(EMPTY_ROOM_LIFETIME - Duration::from_secs(1)).await;
    assert!(rooms.lock().unwrap().contains_key("KXTP"));

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!rooms.lock().unwrap().contains_key("KXTP"));
    assert!(
        !room.send(RoomCommand::Leave { player_id: 1 }).await,
        "the task has ended"
    );
}

#[tokio::test(start_paused = true)]
async fn room_with_a_connected_player_is_kept() {
    let (room, rooms) = spawn_room();
    let (_alex, _messages) = join(&room, "Alex", None).await;

    tokio::time::sleep(EMPTY_ROOM_LIFETIME * 2).await;
    assert!(rooms.lock().unwrap().contains_key("KXTP"));
}

#[tokio::test(start_paused = true)]
async fn someone_joining_stops_the_countdown() {
    let (room, rooms) = spawn_room();
    tokio::time::sleep(Duration::from_secs(200)).await;

    // Alex is there from 200s to 300s, then the room is empty again
    let (alex, _messages) = join(&room, "Alex", None).await;
    tokio::time::sleep(Duration::from_secs(100)).await;
    room.send(RoomCommand::Leave {
        player_id: alex.unwrap(),
    })
    .await;

    // The first countdown would have ended at 300s, the new one ends at 600s
    tokio::time::sleep(Duration::from_secs(250)).await;
    assert!(rooms.lock().unwrap().contains_key("KXTP"));
    tokio::time::sleep(Duration::from_secs(60)).await;
    assert!(!rooms.lock().unwrap().contains_key("KXTP"));
}

#[test]
fn lobby_shows_disconnected_players() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    room.add_player("Sam".to_string(), None, test_sender());
    room.player_mut(alex)
        .expect("Alex is in the room")
        .disconnected_since = Some(Instant::now());

    let ServerMessage::Lobby { players, .. } = room.lobby_message() else {
        panic!("expected a lobby message");
    };
    let connected: Vec<bool> = players.iter().map(|player| player.connected).collect();
    assert_eq!(connected, [false, true]);
}

// What the lobby says about one player
fn lobby_player(room: &Room, player_id: u32) -> LobbyPlayer {
    let ServerMessage::Lobby { players, .. } = room.lobby_message() else {
        panic!("a lobby message");
    };
    players
        .into_iter()
        .find(|player| player.id == player_id)
        .expect("the player is in the lobby")
}

#[tokio::test(start_paused = true)]
async fn the_lobby_says_how_long_a_disconnected_player_has_left() {
    let mut room = Room::new("KXTP".to_string());
    let alex = room.add_player("Alex".to_string(), None, test_sender()).id;
    let sam = room.add_player("Sam".to_string(), None, test_sender());
    let (sam, sams_token) = (sam.id, sam.token.clone());
    room.player_mut(sam)
        .expect("Sam is in the room")
        .disconnected_since = Some(Instant::now());
    assert_eq!(lobby_player(&room, sam).reconnect_in, Some(90));
    assert_eq!(lobby_player(&room, alex).reconnect_in, None);

    tokio::time::sleep(Duration::from_secs(30)).await;
    assert_eq!(lobby_player(&room, sam).reconnect_in, Some(60));

    room.reconnect(&sams_token, test_sender());
    assert_eq!(lobby_player(&room, sam).reconnect_in, None);
}

#[test]
fn reconnect_restores_the_seat() {
    let mut room = Room::new("KXTP".to_string());
    let sam = room.add_player("Sam".to_string(), Some("owl".to_string()), test_sender());
    let (sam_id, sam_token) = (sam.id, sam.token.clone());
    room.player_mut(sam_id)
        .expect("Sam is in the room")
        .disconnected_since = Some(Instant::now());

    let (new_sender, mut new_receiver) = mpsc::unbounded_channel();
    assert_eq!(room.reconnect(&sam_token, new_sender), Some(sam_id));

    let sam = &room.players[0];
    assert!(sam.disconnected_since.is_none());
    assert_eq!(sam.avatar.as_deref(), Some("owl"));
    room.broadcast(&room.lobby_message());
    assert!(
        new_receiver.try_recv().is_ok(),
        "messages reach the new connection"
    );
}

#[test]
fn newest_connection_replaces_the_old_one() {
    let mut room = Room::new("KXTP".to_string());
    let (old_sender, mut old_receiver) = mpsc::unbounded_channel();
    let sam = room.add_player("Sam".to_string(), None, old_sender);
    let (sam_id, sam_token) = (sam.id, sam.token.clone());

    let (new_sender, mut new_receiver) = mpsc::unbounded_channel();
    assert_eq!(room.reconnect(&sam_token, new_sender), Some(sam_id));
    assert!(matches!(
        old_receiver.try_recv(),
        Ok(ServerMessage::Replaced)
    ));

    room.broadcast(&room.lobby_message());
    assert!(new_receiver.try_recv().is_ok());
    assert!(
        old_receiver.try_recv().is_err(),
        "the old connection gets nothing more"
    );
}

#[test]
fn unknown_token_does_not_reconnect() {
    let mut room = Room::new("KXTP".to_string());
    room.add_player("Sam".to_string(), None, test_sender());
    assert_eq!(room.reconnect("not-a-real-token", test_sender()), None);
}

// Disconnect a player now and return when
fn disconnect(room: &mut Room, player_id: u32) -> Instant {
    let now = Instant::now();
    room.player_mut(player_id)
        .expect("player is in the room")
        .disconnected_since = Some(now);
    now
}

#[test]
fn player_still_gone_is_removed() {
    let mut room = Room::new("KXTP".to_string());
    let sam = room.add_player("Sam".to_string(), None, test_sender()).id;
    let since = disconnect(&mut room, sam);

    assert!(room.remove_if_gone_since(sam, since));
    assert!(room.players.is_empty());
}

#[test]
fn player_who_came_back_is_kept() {
    let mut room = Room::new("KXTP".to_string());
    let sam = room.add_player("Sam".to_string(), None, test_sender());
    let (sam_id, sam_token) = (sam.id, sam.token.clone());
    let since = disconnect(&mut room, sam_id);
    room.reconnect(&sam_token, test_sender());

    assert!(!room.remove_if_gone_since(sam_id, since));
    assert_eq!(room.players.len(), 1);
}

#[test]
fn old_timer_ignores_a_newer_disconnect() {
    let mut room = Room::new("KXTP".to_string());
    let sam = room.add_player("Sam".to_string(), None, test_sender());
    let (sam_id, sam_token) = (sam.id, sam.token.clone());
    let first = disconnect(&mut room, sam_id);
    room.reconnect(&sam_token, test_sender());
    // Set a clearly later time, two calls to now() can be equal
    let second = first + Duration::from_secs(1);
    room.player_mut(sam_id)
        .expect("Sam is in the room")
        .disconnected_since = Some(second);

    assert!(!room.remove_if_gone_since(sam_id, first));
    assert!(room.remove_if_gone_since(sam_id, second));
}
