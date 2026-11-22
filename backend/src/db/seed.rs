use sqlx::PgPool;

use crate::games::bad_takes::deck::DeckFile;

// How many cards seeding added
pub struct Added {
    pub prompts: u64,
    pub answers: u64,
}

// Put every game and a deck's cards in the database, adding only what isn't there yet.
// Cards are matched by their text: the ids in the deck file are only for the file.
// All of it is saved together, or none of it
pub async fn seed_deck(
    pool: &PgPool,
    game_ids: &[&str],
    game_id: &str,
    deck: &DeckFile,
) -> Result<Added, sqlx::Error> {
    let mut tx = pool.begin().await?;

    for id in game_ids {
        sqlx::query("INSERT INTO game_types (id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        "INSERT INTO decks (game_type_id, name) VALUES ($1, $2)
         ON CONFLICT (game_type_id, name) DO NOTHING",
    )
    .bind(game_id)
    .bind(&deck.name)
    .execute(&mut *tx)
    .await?;
    let deck_id: i64 =
        sqlx::query_scalar("SELECT id FROM decks WHERE game_type_id = $1 AND name = $2")
            .bind(game_id)
            .bind(&deck.name)
            .fetch_one(&mut *tx)
            .await?;

    // One query per kind of card, with the texts passed as a list
    let texts: Vec<&str> = deck.prompts.iter().map(|card| card.text.as_str()).collect();
    let picks: Vec<i16> = deck
        .prompts
        .iter()
        .map(|card| i16::from(card.pick))
        .collect();
    let prompts = sqlx::query(
        "INSERT INTO prompts (deck_id, text, pick)
         SELECT $1, text, pick FROM UNNEST($2::text[], $3::smallint[]) AS card (text, pick)
         ON CONFLICT (deck_id, text) DO NOTHING",
    )
    .bind(deck_id)
    .bind(&texts)
    .bind(&picks)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let texts: Vec<&str> = deck.answers.iter().map(|card| card.text.as_str()).collect();
    let answers = sqlx::query(
        "INSERT INTO answers (deck_id, text)
         SELECT $1, text FROM UNNEST($2::text[]) AS card (text)
         ON CONFLICT (deck_id, text) DO NOTHING",
    )
    .bind(deck_id)
    .bind(&texts)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;
    Ok(Added { prompts, answers })
}
