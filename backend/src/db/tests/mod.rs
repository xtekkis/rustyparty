// Database tests. Each gets a fresh, empty database with every migration run. It's dropped
// when the test passes, and kept for a look when it fails.
// They need PostgreSQL running and DATABASE_URL set (copy .env.example to .env), so plain
// `cargo test` skips them. Run them with: cargo test -- --ignored
use sqlx::PgPool;

use crate::games::bad_takes::deck::{DeckFile, parse_deck};

mod seed;

// A small deck with these prompts and answers, all with one gap
pub fn deck(prompts: &[&str], answers: &[&str]) -> DeckFile {
    let prompts: Vec<_> = prompts
        .iter()
        .enumerate()
        .map(|(i, text)| serde_json::json!({ "id": i + 1, "text": text, "pick": 1 }))
        .collect();
    let answers: Vec<_> = answers
        .iter()
        .enumerate()
        .map(|(i, text)| serde_json::json!({ "id": i + 1, "text": text }))
        .collect();
    let json = serde_json::json!({ "deck": "Test", "prompts": prompts, "answers": answers });
    parse_deck(&json.to_string()).unwrap()
}

// One card row as stored, prompts and answers alike
#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct Card {
    pub kind: String,
    pub id: i64,
    pub deck_id: i64,
    pub text: String,
    // None for answers
    pub pick: Option<i16>,
    pub is_active: bool,
}

// Every card in the database, every column
pub async fn cards(pool: &PgPool) -> Vec<Card> {
    sqlx::query_as(
        "SELECT 'prompt' AS kind, id, deck_id, text, pick, is_active FROM prompts
         UNION ALL SELECT 'answer', id, deck_id, text, NULL, is_active FROM answers
         ORDER BY kind, id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

// The card with this text
pub async fn card(pool: &PgPool, text: &str) -> Card {
    cards(pool)
        .await
        .into_iter()
        .find(|card| card.text == text)
        .unwrap_or_else(|| panic!("no card says {text:?}"))
}
