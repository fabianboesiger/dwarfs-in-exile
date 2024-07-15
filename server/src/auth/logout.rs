use crate::ServerError;
use axum::{response::Redirect, Extension};
use sqlx::SqlitePool;
use tower_sessions::Session;

pub async fn get_logout(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Redirect, ServerError> {
    sqlx::query(
        r#"
            DELETE FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(session.id().unwrap().0 as i64)
    .fetch_optional(&pool)
    .await?;

    Ok(Redirect::to("/"))
}
