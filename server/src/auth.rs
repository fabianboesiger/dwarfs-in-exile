use axum::{
    extract::{
        FromRequest, RequestParts,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Form, BoxError,
};
use serde::{Deserialize, de::DeserializeOwned};
use validator::Validate;
use axum_sessions::async_session::Session;
use async_trait::async_trait;
use thiserror::Error;

use crate::ServerError;

pub struct UserSession {
    session_id: String,
    username: String
}

pub struct User {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct Register {
    #[validate(length(min = 1, message = "Can not be empty"))]
    username: String,
    #[validate(length(min = 8, message = "Password must contain at least 8 characters"))]
    password: String,
    #[validate(must_match = "password")]
    password_repeat: String,
    #[validate(email)]
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct Login {
    username: String,
    password: String,
}

// https://github.com/tokio-rs/axum/blob/main/examples/validator/src/main.rs
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedForm<T>(pub T);

#[async_trait]
impl<T, B> FromRequest<B> for ValidatedForm<T>
where
    T: DeserializeOwned + Validate,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = ServerError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Form(value) = Form::<T>::from_request(req).await?;
        value.validate()?;
        Ok(ValidatedForm(value))
    }
}

pub async fn post_login(
    Form(login): Form<Login>,
    Extension(session): Extension<Session>,
) -> impl IntoResponse {
    
}

pub async fn post_register(
    ValidatedForm(register): ValidatedForm<Register>,
    Extension(session): Extension<Session>,
) -> impl IntoResponse {
    
}

pub async fn post_logout(
    Extension(session): Extension<Session>,
) -> impl IntoResponse {
    
}