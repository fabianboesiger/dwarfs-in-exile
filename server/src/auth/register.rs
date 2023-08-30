use std::time::Duration;

use crate::{game::GameState, ServerError};
use askama::{DynTemplate, Template};
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension,
};
use axum_sessions::async_session::Session;
use bcrypt::hash;
use serde::Deserialize;
use shared::UserId;
use sqlx::SqlitePool;
use validator::{Validate, ValidationErrors};

use super::{form_error, ToTemplate, ValidatedForm};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterForm {
    #[validate(length(min = 1, message = "The username must not be empty"))]
    username: String,
    #[validate(length(min = 4, message = "Password must contain at least 4 characters"))]
    password: String,
    #[validate(must_match(other = "password", message = "The passwords must match"))]
    password_repeat: String,
}

impl ToTemplate for RegisterForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        Box::new(RegisterTemplate {
            username: self.username,
            username_error: errors
                .field_errors()
                .get("username")
                .unwrap_or(&&Vec::new())
                .iter()
                .filter_map(|error| error.message.as_ref().map(|msg| msg.to_string()))
                .collect(),
            password_error: errors
                .field_errors()
                .get("password")
                .unwrap_or(&&Vec::new())
                .iter()
                .filter_map(|error| error.message.as_ref().map(|msg| msg.to_string()))
                .collect(),
            password_repeat_error: errors
                .field_errors()
                .get("password_repeat")
                .unwrap_or(&&Vec::new())
                .iter()
                .filter_map(|error| error.message.as_ref().map(|msg| msg.to_string()))
                .collect(),
        })
    }
}

#[derive(Template, Default)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    username: String,
    username_error: Vec<String>,
    password_error: Vec<String>,
    password_repeat_error: Vec<String>,
}

pub async fn get_register() -> RegisterTemplate {
    RegisterTemplate::default()
}

pub async fn post_register(
    ValidatedForm(register): ValidatedForm<RegisterForm>,
    Extension(mut session): Extension<Session>,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
) -> Result<(Extension<Session>, Response), ServerError> {
    let password = register.password.clone();
    let hashed = tokio::task::spawn_blocking(move || hash(&password, 4).unwrap())
        .await
        .unwrap();

    let result: Result<(UserId,), _> = sqlx::query_as(
        r#"
            INSERT INTO users (username, password)
            VALUES ($1, $2)
            RETURNING user_id
        "#,
    )
    .bind(&register.username)
    .bind(&hashed)
    .fetch_one(&pool)
    .await;

    match result {
        Ok((user_id,)) => {
            // Add a player to the game state.
            game_state.add_player(user_id, register.username);

            session.expire_in(Duration::from_secs(60 * 60 * 24 * 7));

            sqlx::query(
                r#"
                    INSERT OR REPLACE INTO sessions (session_id, user_id, expires)
                    VALUES ($1, $2, $3)
                "#,
            )
            .bind(&session.id())
            .bind(user_id)
            .bind(&session.expiry().map(|expiry| expiry.timestamp()))
            .execute(&pool)
            .await?;

            Ok((Extension(session), Redirect::to("/game").into_response()))
        }
        Err(_err) => Ok((
            Extension(session),
            form_error(register, "unique", "This username is already taken"),
        )),
    }
}
