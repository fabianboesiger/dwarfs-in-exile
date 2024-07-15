use crate::{game::GameState, ServerError};
use askama::{DynTemplate, Template};
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;
use validator::{Validate, ValidationErrors};

use super::{form_error, ToTemplate, ValidatedForm};

#[derive(Debug, Deserialize, Validate)]
pub struct ChangeUsernameForm {
    #[validate(length(min = 1, message = "The username must not be empty"))]
    username: String,
}

impl ToTemplate for ChangeUsernameForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        Box::new(ChangeUsernameTemplate {
            username: self.username,
            username_error: errors
                .field_errors()
                .get("username")
                .unwrap_or(&&Vec::new())
                .iter()
                .filter_map(|error| error.message.as_ref().map(|msg| msg.to_string()))
                .collect(),
        })
    }
}

#[derive(Template, Default)]
#[template(path = "change-username.html")]
pub struct ChangeUsernameTemplate {
    username: String,
    username_error: Vec<String>,
}

pub async fn get_change_username(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    let result: Option<(String,)> = sqlx::query_as(
        r#"
            SELECT username
            FROM users
            NATURAL JOIN sessions
            WHERE session_id = $1
        "#,
    )
    .bind(session.id().ok_or(ServerError::SessionIdMissing)?.0 as i64)
    .fetch_optional(&pool)
    .await?;

    if let Some((username,)) = result {
        Ok(ChangeUsernameTemplate {
            username,
            ..ChangeUsernameTemplate::default()
        }
        .into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

pub async fn post_change_username(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    ValidatedForm(change_username): ValidatedForm<ChangeUsernameForm>,
) -> Result<Response, ServerError> {
    let result: Result<(i64,), _> = sqlx::query_as(
        r#"
            SELECT user_id
                FROM sessions
                WHERE session_id = $1
        "#,
    )
    .bind(session.id().ok_or(ServerError::SessionIdMissing)?.0 as i64)
    .fetch_one(&pool)
    .await;

    let user_id = match result {
        Err(err) => {
            return Err(ServerError::SqliteError(err));
        }
        Ok((user_id,)) => user_id,
    };

    let result = sqlx::query(
        r#"
            UPDATE users
            SET username = $1
            WHERE user_id = $2
        "#,
    )
    .bind(&change_username.username)
    .bind(&user_id)
    .execute(&pool)
    .await;

    match result {
        Err(_) => Ok(form_error(
            change_username,
            "unique",
            "username",
            "This username is already taken",
        )),
        Ok(_) => {
            game_state.new_server_connection().await.updated_user_data();
            Ok(Redirect::to("/account").into_response())
        }
    }
}
