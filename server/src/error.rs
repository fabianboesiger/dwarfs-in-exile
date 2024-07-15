use askama_axum::IntoResponse;
use axum::{http::StatusCode, response::{Redirect, Response}};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),
    //#[error(transparent)]
    //AxumFormRejection(#[from] axum::extract::rejection::FormRejection),
    #[error(transparent)]
    SqliteError(#[from] sqlx::Error),
    #[error("stripe error, missing data")]
    StripeErrorMissingData,
    #[error(transparent)]
    ParseError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    SessionError(#[from] tower_sessions::session::Error),
    #[error("session user missing")]
    SessionUserMissing,
    #[error("user deleted")]
    UserDeleted
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::ValidationError(err) => {
                (StatusCode::BAD_REQUEST, format!("{}", err)).into_response()
            }
            ServerError::SqliteError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
            ServerError::StripeErrorMissingData => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self)).into_response()
            }
            ServerError::ParseError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
            ServerError::SessionError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", err)).into_response()
            }
            ServerError::SessionUserMissing | ServerError::UserDeleted => {
                Redirect::to("/login").into_response()
            }
        }
    }
}
