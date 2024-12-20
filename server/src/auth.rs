pub mod account;
pub mod change_password;
pub mod change_username;
pub mod delete_account;
pub mod login;
pub mod logout;
pub mod register;

use std::borrow::Cow;

use askama::DynTemplate;
use async_trait::async_trait;
use axum::{
    body::Body,
    extract::FromRequest,
    http::{self, Request, StatusCode},
    response::{IntoResponse, Response},
    Form,
};
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationError, ValidationErrors};

// https://github.com/tokio-rs/axum/blob/main/examples/validator/src/main.rs
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedForm<T: ToTemplate>(pub T);

pub trait ToTemplate {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate + 'static>;
}

#[async_trait]
impl<S, F> FromRequest<S> for ValidatedForm<F>
where
    F: DeserializeOwned + Validate + ToTemplate,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let Form(form) = Form::<F>::from_request(req, state)
            .await
            .map_err(|err| err.into_response())?;

        if let Err(errors) = form.validate() {
            Err(form_errors(form, errors))
        } else {
            Ok(ValidatedForm(form))
        }
    }
}

pub fn form_error<F: ToTemplate>(
    form: F,
    code: &'static str,
    field: &'static str,
    message: &'static str,
) -> Response {
    let mut error = ValidationError::new(code);
    error.message = Some(Cow::Borrowed(message));
    let mut errors = ValidationErrors::new();
    errors.add(field, error);

    form_errors(form, errors)
}

pub fn form_errors<F: ToTemplate>(form: F, errors: ValidationErrors) -> Response {
    let template = form.to_template(errors);
    match template.dyn_render() {
        Ok(body) => {
            let headers = [(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static(template.mime_type()),
            )];

            (headers, body).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
