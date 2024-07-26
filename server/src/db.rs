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
            admin INTEGER NOT NULL DEFAULT 0
        )
    "#,
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data BLOB,
            winner INTEGER,
            FOREIGN KEY(winner) REFERENCES users(user_id)
        )
    "#,
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            free_premium INTEGER NOT NULL
        )
    "#,
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO settings (free_premium)
        VALUES (0)
    "#,
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    Ok(pool)
}
