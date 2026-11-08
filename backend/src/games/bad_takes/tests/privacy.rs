// Nobody ever sees a card in someone else's hand
use super::*;

// Nobody's view or messages hold a card from someone else's hand,
// and everyone's view holds their own
fn assert_hands_private(game: &BadTakes, ctx: &GameCtx) {
    let everyone: Vec<PlayerId> = game.players.iter().chain(&game.waiting).copied().collect();
    for &reader in &everyone {
        let mut seen = HashSet::new();
        answer_ids(&game.view_for(reader), &mut seen);
        for message in &ctx.outgoing {
            answer_ids(message, &mut seen);
        }
        for (&owner, hand) in &game.hands {
            for card in hand {
                let visible = seen.contains(&u64::from(card.id));
                if owner == reader {
                    assert!(visible, "player {reader} can't see their own {}", card.text);
                } else {
                    assert!(!visible, "player {reader} can see {owner}'s {}", card.text);
                }
            }
        }
    }
}

#[test]
fn hands_stay_private_through_every_kind_of_round() {
    let cards = deck::starter_deck();
    let mut game = BadTakes::new(
        7,
        cards.prompts.clone(),
        cards.answers.clone(),
        StdRng::seed_from_u64(1),
    );

    // The deal
    let mut ctx = GameCtx::default();
    game.start(&[1, 2, 3, 4, 5], &mut ctx);
    assert_hands_private(&game, &ctx);

    // A late joiner waits for a seat
    let mut ctx = GameCtx::default();
    game.player_joined(6, &mut ctx);
    assert_hands_private(&game, &ctx);

    // Sam plays, then the judge leaves: the round is cancelled and Sam gets his cards back
    let gaps = usize::from(game.prompt.as_ref().expect("a prompt").pick);
    let cards: Vec<CardId> = game.hands[&2][..gaps].iter().map(|card| card.id).collect();
    let ctx = submit(&mut game, 2, &cards).unwrap();
    assert_hands_private(&game, &ctx);
    let mut ctx = GameCtx::default();
    game.player_left(1, &mut ctx);
    assert_eq!(game.round, 2);
    assert_hands_private(&game, &ctx);

    // Everyone plays and the judge runs out of time
    let mut ctx = GameCtx::default();
    all_answers_out(&mut game, &mut ctx);
    assert_hands_private(&game, &ctx);
    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Judge), &mut ctx);
    assert_hands_private(&game, &ctx);

    // The next round seats the late joiner and refills every hand
    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Reveal), &mut ctx);
    assert!(game.players.contains(&6));
    assert_hands_private(&game, &ctx);

    // A player leaves mid-round, then nobody plays in time
    let mut ctx = GameCtx::default();
    game.player_left(5, &mut ctx);
    assert_hands_private(&game, &ctx);
    let mut ctx = GameCtx::default();
    game.handle_timer(game.timer_id(Timer::Submit), &mut ctx);
    assert_eq!(game.round, 4, "the round was skipped");
    assert_hands_private(&game, &ctx);

    // A normal round: everyone plays and the judge picks
    let mut ctx = GameCtx::default();
    all_answers_out(&mut game, &mut ctx);
    assert_hands_private(&game, &ctx);
    let judge = game.judge.expect("a judge");
    let card = game.submissions[&game.reading_order[0]][0].id;
    let ctx = pick(&mut game, judge, card).unwrap();
    assert_hands_private(&game, &ctx);
}

#[test]
fn a_view_shows_only_your_own_hand() {
    let game = started_game(&[1, 2, 3]);
    let view = game.view_for(1);
    assert_eq!(view["hand"], json!(game.hands[&1]));

    // None of Sam's cards appear anywhere in Alex's view
    let alex_sees = view.to_string();
    for card in &game.hands[&2] {
        assert!(
            !alex_sees.contains(&card.text),
            "Alex can see {}",
            card.text
        );
    }
}
