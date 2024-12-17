use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn setup() -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let options = SqliteConnectOptions::from_str(&format!(
        "sqlite:{}",
        dotenv::var("DATABASE_FILE").unwrap()
    ))?
    .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    let mut transaction = pool.begin().await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL,
            premium INTEGER NOT NULL,
            referrer INTEGER DEFAULT NULL,
            admin INTEGER NOT NULL DEFAULT 0,
            guest INTEGER NOT NULL DEFAULT 0,
            joined TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            skins JSON DEFAULT('[]'))
            FOREIGN KEY(referrer) REFERENCES users(user_id) ON DELETE SET NULL,
        )
    "#,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data BLOB,
            closed INTEGER NOT NULL DEFAULT 0,
            winner INTEGER,
            game_mode TEXT,
            FOREIGN KEY(winner) REFERENCES users(user_id) ON DELETE SET NULL
        )
    "#,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            free_premium INTEGER NOT NULL,
            auto_start_world INTEGER NOT NULL
        )
    "#,
    )
    .execute(&mut *transaction)
    .await?;

    let (settings_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM settings
    "#,
    )
    .fetch_one(&mut *transaction)
    .await?;

    if settings_count == 0 {
        tracing::info!("inserting default settings");

        sqlx::query(
            r#"
            INSERT INTO settings (free_premium, auto_start_world) VALUES (0, 1)
        "#,
        )
        .execute(&mut *transaction)
        .await?;
    } else {
        tracing::info!("settings loaded");
    }
    

    transaction.commit().await?;

    Ok(pool)
}
