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

use super::{ToTemplate, ValidatedForm};

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordForm {
    #[validate(length(min = 4, message = "Password must contain at least 4 characters"))]
    password: String,
    #[validate(must_match(other = "password", message = "The passwords must match"))]
    password_repeat: String,
}

impl ToTemplate for ChangePasswordForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        Box::new(ChangePasswordTemplate {
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
#[template(path = "change-password.html")]
pub struct ChangePasswordTemplate {
    password_error: Vec<String>,
    password_repeat_error: Vec<String>,
}

pub async fn get_change_password(session: Session) -> Result<Response, ServerError> {
    session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    Ok(ChangePasswordTemplate {
        ..ChangePasswordTemplate::default()
    }
    .into_response())
}

pub async fn post_change_password(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    ValidatedForm(change_password): ValidatedForm<ChangePasswordForm>,
) -> Result<Response, ServerError> {
    let password = change_password.password.clone();
    let hashed = tokio::task::spawn_blocking(move || hash(&password, 4).unwrap())
        .await
        .unwrap();

    sqlx::query(
        r#"
            UPDATE users
            SET password = $1,
            guest = 0
            WHERE user_id = $2
        "#,
    )
    .bind(&hashed)
    .bind(
        session
            .get::<i64>(crate::USER_ID_KEY)
            .await?
            .ok_or(ServerError::InvalidSession)?,
    )
    .execute(&pool)
    .await?;

    game_state.new_server_connection().await.updated_user_data();

    Ok(Redirect::to("/account").into_response())
}
