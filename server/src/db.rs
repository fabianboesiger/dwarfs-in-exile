use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn setup() -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", dotenv::var("DATABASE_FILE").unwrap()))?.create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    let mut transaction = pool.begin().await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL,
            premium INTEGER NOT NULL
        )
    "#,
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY,
            data BLOB
        )
    "#,
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    Ok(pool)
}
