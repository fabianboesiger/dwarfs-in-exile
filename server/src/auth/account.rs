use crate::ServerError;
use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{response::Redirect, Extension};
use sqlx::SqlitePool;
use tower_sessions::Session;

#[derive(Template, Default)]
#[template(path = "account.html")]
pub struct AccountTemplate {
    username: String,
    user_id: i64,
    premium: i64,
}

pub async fn get_account(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    let result: Option<(String, i64, i64)> = sqlx::query_as(
        r#"
            SELECT username, user_id, premium
            FROM users
            NATURAL JOIN sessions
            WHERE session_id = $1
        "#,
    )
    .bind(session.id().ok_or(ServerError::SessionIdMissing)?.0 as i64)
    .fetch_optional(&pool)
    .await?;

    if let Some((username, user_id, premium)) = result {
        Ok(AccountTemplate {
            username,
            user_id,
            premium,
            ..AccountTemplate::default()
        }
        .into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}
