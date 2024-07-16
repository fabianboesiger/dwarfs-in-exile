use crate::ServerError;
use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::Extension;
use sqlx::SqlitePool;
use tower_sessions::Session;

#[derive(Template, Default)]
#[template(path = "store.html")]
pub struct StoreTemplate {
    username: String,
    user_id: i64,
}

pub async fn get_store(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    let (username, user_id): (String, i64) = sqlx::query_as(
        r#"
            SELECT username, user_id
            FROM users
            WHERE user_id = $1
        "#,
    )
    .bind(session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?)
    .fetch_optional(&pool)
    .await?
    .ok_or(ServerError::UserDeleted)?;

    Ok(StoreTemplate {
        username,
        user_id,
        ..StoreTemplate::default()
    }
    .into_response())
   
}
