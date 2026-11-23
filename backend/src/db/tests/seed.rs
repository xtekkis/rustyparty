use sqlx::PgPool;

use super::*;
use crate::db::seed::{CardChanges, seed_deck};
use crate::games::bad_takes::deck::starter_deck;

fn added(added: u64) -> CardChanges {
    CardChanges {
        added,
        ..CardChanges::default()
    }
}

#[sqlx::test]
#[ignore = "needs the database: docker compose up -d"]
async fn seeding_twice_changes_nothing(pool: PgPool) {
    let first = seed_deck(&pool, &["bad_takes"], "bad_takes", starter_deck())
        .await
        .unwrap();
    assert_eq!(first.prompts, added(45));
    assert_eq!(first.answers, added(140));
    let before = cards(&pool).await;

    let second = seed_deck(&pool, &["bad_takes"], "bad_takes", starter_deck())
        .await
        .unwrap();
    assert!(second.prompts.is_empty() && second.answers.is_empty());
    // Same ids, texts and active cards: not a single row changed
    assert_eq!(cards(&pool).await, before);
}

#[sqlx::test]
#[ignore = "needs the database: docker compose up -d"]
async fn removed_cards_are_retired_and_come_back(pool: PgPool) {
    let full = deck(
        &["Love is ____.", "My boss loves ____."],
        &["A goose.", "A wet sock."],
    );
    seed_deck(&pool, &["bad_takes"], "bad_takes", &full)
        .await
        .unwrap();
    let goose_id = cards(&pool)
        .await
        .into_iter()
        .find(|card| card.2 == "A goose.")
        .unwrap()
        .1;

    // The goose and the second prompt are taken out of the file
    let smaller = deck(&["Love is ____."], &["A wet sock."]);
    let synced = seed_deck(&pool, &["bad_takes"], "bad_takes", &smaller)
        .await
        .unwrap();
    assert_eq!(synced.prompts.removed, 1);
    assert_eq!(synced.answers.removed, 1);
    let goose = cards(&pool)
        .await
        .into_iter()
        .find(|card| card.2 == "A goose.");
    assert_eq!(
        goose.map(|card| card.3),
        Some(false),
        "kept, but no longer dealt"
    );

    // Back in the file: the same row is dealt again, not a new copy
    let synced = seed_deck(&pool, &["bad_takes"], "bad_takes", &full)
        .await
        .unwrap();
    assert_eq!(synced.prompts.restored, 1);
    assert_eq!(synced.answers.restored, 1);
    assert_eq!(synced.answers.added, 0);
    let goose = cards(&pool)
        .await
        .into_iter()
        .find(|card| card.2 == "A goose.");
    assert_eq!(goose.map(|card| (card.1, card.3)), Some((goose_id, true)));
}

#[sqlx::test]
#[ignore = "needs the database: docker compose up -d"]
async fn every_game_gets_a_row(pool: PgPool) {
    let cards = deck(&["Love is ____."], &["A goose."]);
    seed_deck(&pool, &["bad_takes", "trivia"], "bad_takes", &cards)
        .await
        .unwrap();
    let games: Vec<String> = sqlx::query_scalar("SELECT id FROM game_types ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(games, ["bad_takes", "trivia"]);
}

#[sqlx::test]
#[ignore = "needs the database: docker compose up -d"]
async fn a_failed_seed_saves_nothing(pool: PgPool) {
    // A deck for a game that isn't in the list can't be saved
    let cards = deck(&["Love is ____."], &["A goose."]);
    let result = seed_deck(&pool, &["bad_takes"], "chess", &cards).await;
    assert!(result.is_err());

    // The bad_takes row written just before is rolled back with it
    let games: i64 = sqlx::query_scalar("SELECT count(*) FROM game_types")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(games, 0);
}
