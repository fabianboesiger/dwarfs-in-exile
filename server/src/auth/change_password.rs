use crate::ServerError;
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

pub async fn get_change_password(
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

    if let Some(_) = result {
        Ok(ChangePasswordTemplate {
            ..ChangePasswordTemplate::default()
        }
        .into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

pub async fn post_change_password(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    ValidatedForm(change_password): ValidatedForm<ChangePasswordForm>,
) -> Result<Response, ServerError> {
    let password = change_password.password.clone();
    let hashed = tokio::task::spawn_blocking(move || hash(&password, 4).unwrap())
        .await
        .unwrap();

    sqlx::query(
        r#"
            UPDATE users
            SET password = $1
            WHERE user_id = (SELECT user_id
                FROM sessions
                WHERE session_id = $2)
        "#,
    )
    .bind(&hashed)
    .bind(session.id().ok_or(ServerError::SessionIdMissing)?.0 as i64)
    .execute(&pool)
    .await?;

    Ok(Redirect::to("/account").into_response())
}
