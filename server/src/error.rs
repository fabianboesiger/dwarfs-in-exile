use askama_axum::IntoResponse;
use axum::{
    http::StatusCode,
    response::{Redirect, Response},
};
use stripe::StripeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),
    //#[error(transparent)]
    //AxumFormRejection(#[from] axum::extract::rejection::FormRejection),
    #[error(transparent)]
    SqliteError(#[from] sqlx::Error),
    #[error("stripe error, missing data: {0}")]
    StripeErrorMissingData(String),
    #[error(transparent)]
    StripeError(#[from] StripeError),
    #[error(transparent)]
    ParseError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    #[error("invalid session")]
    InvalidSession,
    #[error("user deleted")]
    UserDeleted,
    #[error("no admin permissions")]
    NoAdminPermissions,
    #[error("engine error: {0}")]
    EngineError(#[from] engine_server::Error),
    #[error("guest account error")]
    GuestAccountError,
    #[error("encoding error: {0}")]
    EncodingError(#[from] rmp_serde::encode::Error),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match &self {
            ServerError::InvalidSession | ServerError::UserDeleted => {
                Redirect::to("/login").into_response()
            }
            ServerError::NoAdminPermissions => {
                (StatusCode::UNAUTHORIZED, format!("{self}")).into_response()
            }
            ServerError::ValidationError(_) => {
                (StatusCode::BAD_REQUEST, format!("{self}")).into_response()
            }
            _ => {
                tracing::error!("an internal server error occurred: {self}");
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{self}")).into_response()
            }
        }
    }
}
