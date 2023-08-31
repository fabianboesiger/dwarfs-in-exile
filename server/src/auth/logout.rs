use crate::ServerError;
use axum::{response::Redirect, Extension};
use axum_sessions::extractors::ReadableSession;
use sqlx::SqlitePool;

pub async fn get_logout(
    session: ReadableSession,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Redirect, ServerError> {
    sqlx::query(
        r#"
            DELETE FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(&session.id())
    .fetch_optional(&pool)
    .await?;

    Ok(Redirect::to("/"))
}
