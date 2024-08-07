use crate::ServerError;
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
pub struct LoginForm {
    //#[validate(length(min = 1, message = "The username must not be empty"))]
    username: String,
    //#[validate(length(min = 8, message = "Password must contain at least 8 characters"))]
    password: String,
}

impl ToTemplate for LoginForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        Box::new(LoginTemplate {
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
        })
    }
}

#[derive(Template, Default)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    username: String,
    username_error: Vec<String>,
    password_error: Vec<String>,
}

pub async fn get_login() -> LoginTemplate {
    LoginTemplate::default()
}

pub async fn post_login(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
    ValidatedForm(login): ValidatedForm<LoginForm>,
) -> Result<Response, ServerError> {
    let result: Result<(String, i64), _> = sqlx::query_as(
        r#"
            SELECT password, user_id
            FROM users
            WHERE username = $1
        "#,
    )
    .bind(&login.username)
    .fetch_one(&pool)
    .await;

    match result {
        Ok((hash, user_id)) => {
            let password = login.password.clone();
            let verify = tokio::task::spawn_blocking(move || verify(&password, &hash).unwrap())
                .await
                .unwrap();

            if verify {
                session.insert(crate::USER_ID_KEY, user_id).await?;

                Ok(Redirect::to("/game").into_response())
            } else {
                Ok(form_error(
                    login,
                    "verify",
                    "password",
                    "The password is incorrect",
                ))
            }
        }
        Err(_err) => Ok(form_error(
            login,
            "inexistent",
            "username",
            "This username does not exist",
        )),
    }
}
