use sqlx::PgPool;

use super::*;
use crate::db::seed::{CardChanges, seed_deck};
use crate::games::bad_takes::deck::starter_deck;

fn added(added: usize) -> CardChanges {
    CardChanges {
        added: added as u64,
        ..CardChanges::default()
    }
}

#[sqlx::test]
#[ignore = "needs PostgreSQL and DATABASE_URL, see db/tests/mod.rs"]
async fn seeding_twice_changes_nothing(pool: PgPool) {
    let starter = starter_deck();
    let first = seed_deck(&pool, &["bad_takes"], "bad_takes", starter)
        .await
        .unwrap();
    assert_eq!(first.prompts, added(starter.prompts.len()));
    assert_eq!(first.answers, added(starter.answers.len()));
    let before = cards(&pool).await;

    let second = seed_deck(&pool, &["bad_takes"], "bad_takes", starter)
        .await
        .unwrap();
    assert!(second.prompts.is_empty() && second.answers.is_empty());
    // Every column of every card is the same: not a single row changed
    assert_eq!(cards(&pool).await, before);
}

#[sqlx::test]
#[ignore = "needs PostgreSQL and DATABASE_URL, see db/tests/mod.rs"]
async fn removed_cards_are_retired_and_come_back(pool: PgPool) {
    let full = deck(
        &["Love is ____.", "My boss loves ____."],
        &["A goose.", "A wet sock."],
    );
    seed_deck(&pool, &["bad_takes"], "bad_takes", &full)
        .await
        .unwrap();
    let boss = card(&pool, "My boss loves ____.").await;
    let goose = card(&pool, "A goose.").await;

    // The goose and the boss prompt are taken out of the file
    let smaller = deck(&["Love is ____."], &["A wet sock."]);
    let synced = seed_deck(&pool, &["bad_takes"], "bad_takes", &smaller)
        .await
        .unwrap();
    assert_eq!(synced.prompts.removed, 1);
    assert_eq!(synced.answers.removed, 1);
    // Kept, but no longer dealt
    assert!(!card(&pool, "My boss loves ____.").await.is_active);
    assert!(!card(&pool, "A goose.").await.is_active);

    // Back in the file: the same rows are dealt again, not new copies
    let synced = seed_deck(&pool, &["bad_takes"], "bad_takes", &full)
        .await
        .unwrap();
    assert_eq!(synced.prompts.restored, 1);
    assert_eq!(synced.answers.restored, 1);
    assert_eq!(synced.prompts.added, 0);
    assert_eq!(synced.answers.added, 0);
    assert_eq!(card(&pool, "My boss loves ____.").await, boss);
    assert_eq!(card(&pool, "A goose.").await, goose);
}

#[sqlx::test]
#[ignore = "needs PostgreSQL and DATABASE_URL, see db/tests/mod.rs"]
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
#[ignore = "needs PostgreSQL and DATABASE_URL, see db/tests/mod.rs"]
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
