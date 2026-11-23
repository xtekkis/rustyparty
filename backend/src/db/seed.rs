use sqlx::{PgConnection, PgPool};

use crate::games::bad_takes::deck::DeckFile;

// What seeding changed for one kind of card
#[derive(Debug, Default, PartialEq)]
pub struct CardChanges {
    // New in the file
    pub added: u64,
    // Gone from the file, so no longer dealt
    pub removed: u64,
    // Back in the file after being removed
    pub restored: u64,
}

impl CardChanges {
    pub fn is_empty(&self) -> bool {
        *self == CardChanges::default()
    }
}

// What seeding changed in one deck
#[derive(Debug)]
pub struct Synced {
    pub prompts: CardChanges,
    pub answers: CardChanges,
}

// Cards gone from the file stop being dealt, and cards back in it are dealt again.
// They're never deleted, because finished rounds may point to them
const RETIRE_PROMPTS: &str = "UPDATE prompts SET is_active = FALSE
    WHERE deck_id = $1 AND is_active AND NOT (text = ANY($2::text[]))";
const RESTORE_PROMPTS: &str = "UPDATE prompts SET is_active = TRUE
    WHERE deck_id = $1 AND NOT is_active AND text = ANY($2::text[])";
const RETIRE_ANSWERS: &str = "UPDATE answers SET is_active = FALSE
    WHERE deck_id = $1 AND is_active AND NOT (text = ANY($2::text[]))";
const RESTORE_ANSWERS: &str = "UPDATE answers SET is_active = TRUE
    WHERE deck_id = $1 AND NOT is_active AND text = ANY($2::text[])";

// Make the database match a deck file, and add every game.
// Cards are matched by their text: the ids in the deck file are only for the file.
// All of it is saved together, or none of it
pub async fn seed_deck(
    pool: &PgPool,
    game_ids: &[&str],
    game_id: &str,
    deck: &DeckFile,
) -> Result<Synced, sqlx::Error> {
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

    // One query per change and kind of card, with the texts passed as a list
    let texts: Vec<&str> = deck.prompts.iter().map(|card| card.text.as_str()).collect();
    let picks: Vec<i16> = deck
        .prompts
        .iter()
        .map(|card| i16::from(card.pick))
        .collect();
    let added = sqlx::query(
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
    let prompts = CardChanges {
        added,
        removed: update_cards(&mut tx, RETIRE_PROMPTS, deck_id, &texts).await?,
        restored: update_cards(&mut tx, RESTORE_PROMPTS, deck_id, &texts).await?,
    };

    let texts: Vec<&str> = deck.answers.iter().map(|card| card.text.as_str()).collect();
    let added = sqlx::query(
        "INSERT INTO answers (deck_id, text)
         SELECT $1, text FROM UNNEST($2::text[]) AS card (text)
         ON CONFLICT (deck_id, text) DO NOTHING",
    )
    .bind(deck_id)
    .bind(&texts)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let answers = CardChanges {
        added,
        removed: update_cards(&mut tx, RETIRE_ANSWERS, deck_id, &texts).await?,
        restored: update_cards(&mut tx, RESTORE_ANSWERS, deck_id, &texts).await?,
    };

    tx.commit().await?;
    Ok(Synced { prompts, answers })
}

// Run one of the update queries above, and say how many cards it changed
async fn update_cards(
    conn: &mut PgConnection,
    sql: &'static str,
    deck_id: i64,
    texts: &[&str],
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(sql)
        .bind(deck_id)
        .bind(texts)
        .execute(conn)
        .await?;
    Ok(result.rows_affected())
}
