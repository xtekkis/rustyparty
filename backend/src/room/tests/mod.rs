// Room tests, one file per part of a room's life. The helpers they share are here
use super::actor::*;
use super::in_game::RESULTS_DURATION;
use super::vote::VOTE_DURATION;
use super::*;
use crate::games::bad_takes::{
    BadTakes, IDLE_JUDGE_DURATION, IDLE_PULL_DURATION, PULL_DURATION, REVEAL_DURATION, answer_ids,
    deck,
};
use crate::games::dummy::{DUMMY_INFO, DummyGame};
use crate::games::{Game, GameCtx, GameInfo};
use crate::protocol::{RoomPhase, RoomSettings};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use tokio::sync::mpsc::{self, UnboundedReceiver, WeakSender};
use tokio::sync::oneshot;

// The phase in the last lobby message this phone got
fn last_phase(messages: &mut UnboundedReceiver<ServerMessage>) -> Option<RoomPhase> {
    drain(messages)
        .into_iter()
        .rev()
        .find_map(|message| match message {
            ServerMessage::Lobby { phase, .. } => Some(phase),
            _ => None,
        })
}

mod bad_takes;
mod countdown;
mod games;
mod joining;
mod lobby;
mod reconnecting;
mod voting;

// Join a player and return their id and the messages their phone gets
async fn join(
    room: &RoomHandle,
    nickname: &str,
    token: Option<String>,
) -> (Option<u32>, UnboundedReceiver<ServerMessage>) {
    let (connection, messages) = mpsc::unbounded_channel();
    let (reply, player_id) = oneshot::channel();
    room.send(RoomCommand::Join {
        nickname: nickname.to_string(),
        avatar: None,
        token,
        connection,
        reply,
    })
    .await;
    (player_id.await.unwrap(), messages)
}

// A room registered in its own map, like the server does it
fn spawn_room() -> (RoomHandle, RoomMap) {
    let rooms = RoomMap::default();
    let room = RoomHandle::spawn("KXTP".to_string(), rooms.clone());
    rooms
        .lock()
        .unwrap()
        .insert("KXTP".to_string(), room.clone());
    (room, rooms)
}

async fn player_count(room: &RoomHandle) -> usize {
    let (reply, count) = oneshot::channel();
    room.send(RoomCommand::PlayerCount { reply }).await;
    count.await.unwrap()
}

fn error_code(message: Option<ServerMessage>) -> String {
    match message {
        Some(ServerMessage::Error { code, .. }) => code,
        _ => panic!("expected an error"),
    }
}

// Everything this phone has been sent since it was last read
fn drain(messages: &mut UnboundedReceiver<ServerMessage>) -> Vec<ServerMessage> {
    std::iter::from_fn(|| messages.try_recv().ok()).collect()
}

// The last game event this phone got
fn last_game_event(messages: &mut UnboundedReceiver<ServerMessage>) -> Option<Value> {
    game_messages(messages).0.pop()
}

// The last game view this phone got
fn last_game_view(messages: &mut UnboundedReceiver<ServerMessage>) -> Option<Value> {
    Some(game_messages(messages).1).filter(|view| !view.is_null())
}

// The game events this phone got, and the last view
fn game_messages(messages: &mut UnboundedReceiver<ServerMessage>) -> (Vec<Value>, Value) {
    let (mut events, mut view) = (Vec::new(), Value::Null);
    for message in drain(messages) {
        match message {
            ServerMessage::GameEvent { event } => events.push(event),
            ServerMessage::GameView { view: latest, .. } => view = latest,
            _ => {}
        }
    }
    (events, view)
}

// Bad Takes dealt from a seeded deck, so the room tests get the same cards every run
fn bad_takes() -> GameInfo {
    fn create(settings: &RoomSettings) -> Box<dyn Game> {
        let cards = deck::starter_deck();
        Box::new(BadTakes::new(
            settings.points_to_win,
            cards.prompts.clone(),
            cards.answers.clone(),
            StdRng::seed_from_u64(1),
        ))
    }
    GameInfo {
        create,
        ..crate::games::bad_takes::info()
    }
}

fn full_room() -> (Room, Vec<UnboundedReceiver<ServerMessage>>) {
    let names: Vec<String> = (1..=MAX_ROOM_PLAYERS)
        .map(|n| format!("Player {n}"))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    room_with(&names)
}

// A player plays the first cards in their hand, one for each gap, and returns their ids
fn play_first_cards(
    room: &mut Room,
    player_id: u32,
    view: &Value,
    me: &WeakSender<RoomCommand>,
) -> Vec<Value> {
    let gaps = view["prompt"]["pick"].as_u64().expect("pick") as usize;
    let hand = view["hand"].as_array().expect("a hand");
    let cards: Vec<Value> = hand[..gaps].iter().map(|card| card["id"].clone()).collect();
    room.game_action(player_id, json!({ "kind": "submit", "cards": cards }), me);
    cards
}

// A way for code under test to remind the room, kept alive by the returned sender
fn test_me() -> (mpsc::Sender<RoomCommand>, WeakSender<RoomCommand>) {
    let (commands, _receiver) = mpsc::channel(8);
    let me = commands.downgrade();
    (commands, me)
}

// Everyone in the room presses ready and the vote starts
fn start_vote(room: &mut Room, me: &WeakSender<RoomCommand>) {
    let ids: Vec<u32> = room.players.iter().map(|player| player.id).collect();
    for player_id in ids {
        room.set_ready(player_id, true);
    }
    room.start_vote_if_ready(me);
    assert!(matches!(room.state, RoomState::Voting { .. }));
}

// A room with these players in it, and their phones' messages
fn room_with(nicknames: &[&str]) -> (Room, Vec<UnboundedReceiver<ServerMessage>>) {
    let mut room = Room::new("KXTP".to_string());
    room.games = std::slice::from_ref(&DUMMY_INFO);
    let mut phones = Vec::new();
    for nickname in nicknames {
        let (connection, messages) = mpsc::unbounded_channel();
        room.add_player(nickname.to_string(), None, connection);
        phones.push(messages);
    }
    (room, phones)
}

// Messages sent to it are dropped, tests here don't read them
fn test_sender() -> UnboundedSender<ServerMessage> {
    mpsc::unbounded_channel().0
}
