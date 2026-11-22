pub mod seed;

use std::time::Duration;

use sqlx::PgPool;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::PgPoolOptions;

// How long a query waits for a free connection before giving up
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);
// Few connections: only decks and finished games use the database
const MAX_CONNECTIONS: u32 = 5;

// Every file in backend/migrations, built into the server
static MIGRATOR: Migrator = sqlx::migrate!();

// The database from DATABASE_URL, or None without one.
// Games never wait on it, so the server starts even when it's down
pub async fn connect() -> Option<PgPool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        tracing::warn!("DATABASE_URL is not set, running without a database");
        return None;
    };

    // Lazy: connections are opened when first needed, so a database that
    // starts after the server is still found
    let pool = match PgPoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_lazy(&url)
    {
        Ok(pool) => pool,
        Err(_) => {
            // The address isn't logged, it holds the password
            tracing::error!(
                "DATABASE_URL is not a valid database address, running without a database"
            );
            return None;
        }
    };

    Some(pool)
}

// Bring the tables up to date: each migration not yet run is run once, in order.
// This is also the first time the server reaches the database, so it says whether that works.
// True when the tables are ready to use
pub async fn migrate(pool: &PgPool) -> bool {
    let result = MIGRATOR.run(pool).await;
    match &result {
        Ok(()) => tracing::info!(
            "Connected to the database, tables up to date (latest migration {})",
            MIGRATOR
                .iter()
                .map(|migration| migration.version)
                .max()
                .unwrap_or(0)
        ),
        Err(MigrateError::Execute(sqlx::Error::PoolTimedOut)) => {
            tracing::warn!("Can't reach the database, games still work")
        }
        Err(error) => tracing::error!("Database migration failed, games still work: {error}"),
    }
    result.is_ok()
}
