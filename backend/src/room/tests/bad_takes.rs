// Bad Takes played through a real room
use super::*;

#[tokio::test(start_paused = true)]
async fn a_full_round_of_bad_takes() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, mut reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);

    // Everyone gets 7 cards, Alex judges, and they all see the same prompt
    let views: Vec<Value> = phones
        .iter_mut()
        .map(|phone| game_messages(phone).1)
        .collect();
    for view in &views {
        assert_eq!(view["hand"].as_array().map(Vec::len), Some(7));
        assert_eq!(view["judge"], 1);
        assert_eq!(view["prompt"], views[0]["prompt"]);
    }

    // Sam and Kim play their first cards, one for each gap
    let gaps = views[0]["prompt"]["pick"].as_u64().expect("pick") as usize;
    for (player_id, view) in [(2, &views[1]), (3, &views[2])] {
        let hand = view["hand"].as_array().expect("a hand");
        let cards: Vec<&Value> = hand[..gaps].iter().map(|card| &card["id"]).collect();
        room.game_action(player_id, json!({ "kind": "submit", "cards": cards }), &me);
    }

    // The answers go into the bag, and Alex pulls them out one at a time
    let (_, alex_sees) = game_messages(&mut phones[0]);
    assert_eq!(alex_sees["phase"], "reading");
    assert_eq!(alex_sees["in_bag"], 2);
    for _ in 0..2 {
        room.game_action(1, json!({ "kind": "pull" }), &me);
    }
    room.game_action(1, json!({ "kind": "show_all" }), &me);

    // Alex has read both answers, with no names, and picks the first
    let (_, alex_sees) = game_messages(&mut phones[0]);
    assert_eq!(alex_sees["phase"], "judging");
    let answers = alex_sees["cards_to_judge"].as_array().expect("answers");
    assert_eq!(answers.len(), 2);
    let picked = answers[0][0]["id"].clone();
    room.game_action(1, json!({ "kind": "pick", "card": picked }), &me);

    // Everyone hears who won, and the winner has a point
    let (events, kim_sees) = game_messages(&mut phones[2]);
    let reveal = events.last().expect("the reveal");
    assert_eq!(reveal["event"], "winner_revealed");
    assert_eq!(reveal["cards"][0]["id"], picked);
    let winner = reveal["winner"].as_u64().expect("a winner").to_string();
    assert!(winner == "2" || winner == "3");
    assert_eq!(kim_sees["scores"][&winner], 1);

    // After the reveal the next round starts: Sam judges and every hand is full again
    tokio::time::sleep(REVEAL_DURATION + Duration::from_secs(1)).await;
    while let Ok(reminder) = reminders.try_recv() {
        room.handle(reminder, &me);
    }
    for phone in &mut phones {
        let (_, view) = game_messages(phone);
        assert_eq!(view["round"], 2);
        assert_eq!(view["judge"], 2);
        assert_eq!(view["phase"], "submitting");
        assert_eq!(view["hand"].as_array().map(Vec::len), Some(7));
    }
}

#[tokio::test(start_paused = true)]
async fn a_slow_judge_gives_the_point_to_a_random_answer() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim"]);
    let (commands, mut reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);
    for player_id in [2, 3] {
        let (_, view) = game_messages(&mut phones[player_id as usize - 1]);
        play_first_cards(&mut room, player_id, &view, &me);
    }

    // Alex never taps: both cards come out, they're shown, then a random answer wins
    for wait in [
        PULL_DURATION,
        IDLE_PULL_DURATION,
        IDLE_PULL_DURATION,
        IDLE_JUDGE_DURATION,
    ] {
        tokio::time::sleep(wait + Duration::from_secs(1)).await;
        while let Ok(reminder) = reminders.try_recv() {
            room.handle(reminder, &me);
        }
    }
    let (events, view) = game_messages(&mut phones[0]);
    let reveal = events.last().expect("the reveal");
    assert_eq!(reveal["event"], "winner_revealed");
    assert_eq!(reveal["judge_timed_out"], true);
    let winner = reveal["winner"].as_u64().expect("a winner").to_string();
    assert!(winner == "2" || winner == "3");
    assert_eq!(view["scores"][&winner], 1);
}

#[tokio::test(start_paused = true)]
async fn the_judge_leaving_cancels_the_round() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
    let (commands, _reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    room.start_game(&bad_takes(), &me);
    let (_, sam_sees) = game_messages(&mut phones[1]);
    let sams_cards = play_first_cards(&mut room, 2, &sam_sees, &me);

    // Alex, the judge, leaves before anyone wins
    room.leave(1, &me);
    let (events, view) = game_messages(&mut phones[1]);
    assert!(events.contains(&json!({ "event": "round_cancelled" })));
    assert_eq!(view["round"], 2);
    assert_eq!(view["judge"], 2, "the next player judges");
    let hand: Vec<&Value> = view["hand"]
        .as_array()
        .expect("a hand")
        .iter()
        .map(|card| &card["id"])
        .collect();
    assert_eq!(hand.len(), 7);
    for card in &sams_cards {
        assert!(hand.contains(&card), "Sam has his cards back");
    }
    assert_eq!(view["scores"], json!({ "2": 0, "3": 0, "4": 0 }));
}

// Note the answer cards in everything each phone got, and keep its last view
fn read_all(
    phones: &mut [UnboundedReceiver<ServerMessage>],
    seen: &mut [HashSet<u64>],
    views: &mut [Value],
) {
    for (index, phone) in phones.iter_mut().enumerate() {
        for message in drain(phone) {
            let json = serde_json::to_value(&message).expect("messages are JSON");
            answer_ids(&json, &mut seen[index]);
            if let ServerMessage::GameView { view, .. } = message {
                views[index] = view;
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn hands_are_never_sent_to_other_players() {
    let (mut room, mut phones) = room_with(&["Alex", "Sam", "Kim", "Lee"]);
    let (commands, mut reminders) = mpsc::channel(16);
    let me = commands.downgrade();
    // Every answer card each phone was ever sent, and the last view it got
    let mut seen = vec![HashSet::new(); phones.len()];
    let mut views = vec![Value::Null; phones.len()];

    // A whole round and the start of the next: deal, play, pull, judge, reveal, refill
    room.start_game(&bad_takes(), &me);
    read_all(&mut phones, &mut seen, &mut views);
    for player_id in [2, 3, 4] {
        play_first_cards(&mut room, player_id, &views[player_id as usize - 1], &me);
    }
    for _ in 0..3 {
        room.game_action(1, json!({ "kind": "pull" }), &me);
    }
    room.game_action(1, json!({ "kind": "show_all" }), &me);
    read_all(&mut phones, &mut seen, &mut views);
    let picked = views[0]["cards_to_judge"][0][0]["id"].clone();
    room.game_action(1, json!({ "kind": "pick", "card": picked }), &me);
    tokio::time::sleep(REVEAL_DURATION + Duration::from_secs(1)).await;
    while let Ok(reminder) = reminders.try_recv() {
        room.handle(reminder, &me);
    }
    read_all(&mut phones, &mut seen, &mut views);
    assert_eq!(views[0]["round"], 2);

    // No card still in someone's hand ever reached another phone
    for (owner, view) in views.iter().enumerate() {
        for card in view["hand"].as_array().expect("a hand") {
            let id = card["id"].as_u64().expect("a card id");
            for (reader, cards) in seen.iter().enumerate() {
                if reader != owner {
                    assert!(!cards.contains(&id), "phone {reader} got {}", card["text"]);
                }
            }
        }
    }
}
