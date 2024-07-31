use crate::{game::GameState, ServerError};
use askama::{DynTemplate, Template};
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension,
};
use bcrypt::hash;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;
use validator::{Validate, ValidationErrors};

use super::{form_error, ToTemplate, ValidatedForm};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterForm {
    #[validate(length(
        min = 1,
        max = 16,
        message = "The username must not be empty and contain at most 16 characters"
    ))]
    username: String,
    #[validate(length(
        min = 4,
        max = 32,
        message = "Password must contain at least 4 and at most 32 characters"
    ))]
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
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    ValidatedForm(register): ValidatedForm<RegisterForm>,
) -> Result<Response, ServerError> {
    let password = register.password.clone();
    let hashed = tokio::task::spawn_blocking(move || hash(&password, 4).unwrap())
        .await
        .unwrap();

    let result: Result<(i64,), _> = sqlx::query_as(
        r#"
            INSERT INTO users (username, password, premium, admin)
            VALUES ($1, $2, (
                SELECT free_premium
                FROM settings
                LIMIT 1
            ), (
                SELECT count(*) 
                FROM users
            ) = 0)
            RETURNING user_id
        "#,
    )
    .bind(&register.username)
    .bind(&hashed)
    .fetch_one(&pool)
    .await;

    match result {
        Ok((user_id,)) => {
            game_state.new_server_connection().await.updated_user_data();

            session.insert(crate::USER_ID_KEY, user_id).await?;

            Ok(Redirect::to("/game").into_response())
        }
        Err(_err) => Ok(form_error(
            register,
            "unique",
            "username",
            "This username is already taken",
        )),
    }
}
