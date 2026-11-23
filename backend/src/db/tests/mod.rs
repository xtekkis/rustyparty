// Database tests. Each gets a fresh, empty database with every migration run, dropped afterwards.
// They need PostgreSQL, so plain `cargo test` skips them. With the database up, run them with:
// cargo test -- --ignored
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

// Every card in the database: kind, id, text, and whether it's dealt
pub async fn cards(pool: &PgPool) -> Vec<(String, i64, String, bool)> {
    sqlx::query_as(
        "SELECT 'prompt', id, text, is_active FROM prompts
         UNION ALL SELECT 'answer', id, text, is_active FROM answers
         ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}
