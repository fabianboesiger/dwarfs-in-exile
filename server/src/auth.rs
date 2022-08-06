pub mod register;

use askama::{Template, DynTemplate};
use axum::{
    extract::{
        FromRequest, RequestParts,
    },
    http::{StatusCode, self},
    response::{IntoResponse, Response, Redirect},
    Extension, Form, BoxError,
};
use headers::HeaderValue;
use serde::{Deserialize, de::DeserializeOwned};
use validator::{Validate, ValidationErrors};
use axum_sessions::async_session::Session;
use async_trait::async_trait;
use thiserror::Error;

use crate::ServerError;
use axum_flash::{IncomingFlashes, Flash, Key};

#[derive(Debug, Deserialize)]
pub struct Login {
    username: String,
    password: String,
}

// https://github.com/tokio-rs/axum/blob/main/examples/validator/src/main.rs
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedForm<T: ToTemplate>(pub T);

pub trait ToTemplate {
    fn to_template(self, errors: ValidationErrors) -> Box<dyn DynTemplate + 'static>;
}

#[async_trait]
impl<T, B> FromRequest<B> for ValidatedForm<T>
where
    T: DeserializeOwned + Validate + ToTemplate,
    B: http_body::Body + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    type Rejection = Response;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Form(value) = Form::<T>::from_request(req).await.unwrap();

        println!("got form request");
        
        if let Err(errors) = value.validate() {
            println!("had validation errors");
            let template = value.to_template(errors);
            Err(match template.dyn_render() {
                Ok(body) => {
                    let headers = [(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static(template.mime_type()),
                    )];
        
                    (headers, body).into_response()
                }
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            })
        } else {
            println!("all ok!");
            Ok(ValidatedForm(value))
        }
    }
}
