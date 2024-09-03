use crate::{game::GameState, ServerError};
use askama::{DynTemplate, Template};
use askama_axum::Response;
use axum::{
    response::{IntoResponse, Redirect},
    Extension,
};
use bcrypt::verify;
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_sessions::Session;
use validator::{Validate, ValidationErrors};

use super::{form_error, ToTemplate, ValidatedForm};

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteAccountForm {
    #[validate(length(min = 4, message = "Password must contain at least 4 characters"))]
    password: String,
}

impl ToTemplate for DeleteAccountForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        Box::new(DeleteAccountTemplate {
            password_error: errors
                .field_errors()
                .get("password")
                .unwrap_or(&&Vec::new())
                .iter()
                .filter_map(|error| error.message.as_ref().map(|msg| msg.to_string()))
                .collect(),
        })
    }
}

#[derive(Template, Default)]
#[template(path = "delete-account.html")]
pub struct DeleteAccountTemplate {
    password_error: Vec<String>,
}

pub async fn get_delete_account(session: Session) -> Result<Response, ServerError> {
    session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    Ok(DeleteAccountTemplate {
        ..DeleteAccountTemplate::default()
    }
    .into_response())
}

pub async fn post_delete_account(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    ValidatedForm(delete_account): ValidatedForm<DeleteAccountForm>,
) -> Result<Response, ServerError> {
    let user_id = session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    let (hash,): (String,) = sqlx::query_as(
        r#"
            SELECT password
            FROM users
            WHERE user_id = $1
        "#,
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await?;

    let password = delete_account.password.clone();
    let verify = tokio::task::spawn_blocking(move || verify(&password, &hash).unwrap())
        .await
        .unwrap();

    if verify {
        sqlx::query(
            r#"
                DELETE FROM users
                WHERE user_id = $1
            "#,
        )
        .bind(&user_id)
        .execute(&pool)
        .await?;

        tracing::debug!("Account deleted");

        session.remove::<i64>(crate::USER_ID_KEY).await?;

        game_state.new_server_connection().await.updated_user_data();

        Ok(Redirect::to("/").into_response())
    } else {
        Ok(form_error(
            delete_account,
            "verify",
            "password",
            "The password is incorrect",
        ))
    }
}
