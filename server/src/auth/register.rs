use std::borrow::Cow;

use askama::{Template, DynTemplate};
use axum::{
    response::{IntoResponse},
    Extension, Form, BoxError,
};
use serde::{Deserialize, de::DeserializeOwned};
use validator::{Validate, ValidationErrors};
use axum_sessions::async_session::Session;
use async_trait::async_trait;
use thiserror::Error;

use crate::ServerError;
use super::{ValidatedForm, ToTemplate};

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterForm {
    #[validate(length(min = 1, message = "The username must not be empty"))]
    username: String,
    #[validate(email(message = "The email address must be valid"))]
    email: String,
    #[validate(length(min = 8, message = "Password must contain at least 8 characters"))]
    password: String,
    #[validate(must_match(other = "password", message = "The passwords must match"))]
    password_repeat: String,
}

impl ToTemplate for RegisterForm {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate> {
        println!("{:#?}", errors.field_errors());

        Box::new(RegisterTemplate {
            username: self.username,
            username_error: errors.field_errors().get("username").unwrap_or(&&Vec::new()).iter().filter_map(|error| error.message.as_ref().map(|msg| msg.to_string())).collect(),
            email: self.email,
            email_error: errors.field_errors().get("email").unwrap_or(&&Vec::new()).iter().filter_map(|error| error.message.as_ref().map(|msg| msg.to_string())).collect(),
            password_error: errors.field_errors().get("password").unwrap_or(&&Vec::new()).iter().filter_map(|error| error.message.as_ref().map(|msg| msg.to_string())).collect(),
            password_repeat_error: errors.field_errors().get("password_repeat").unwrap_or(&&Vec::new()).iter().filter_map(|error| error.message.as_ref().map(|msg| msg.to_string())).collect(),
        })
    }
}

#[derive(Template, Default)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    username: String,
    username_error: Vec<String>,
    email: String,
    email_error: Vec<String>,
    password_error: Vec<String>,
    password_repeat_error: Vec<String>,
}

pub async fn get_register(

) -> RegisterTemplate {
    RegisterTemplate::default()
}

pub async fn post_register(
    ValidatedForm(register): ValidatedForm<RegisterForm>,
    Extension(session): Extension<Session>,
) -> impl IntoResponse {
    
}