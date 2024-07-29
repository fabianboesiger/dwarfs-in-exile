use crate::ServerError;
use axum::response::Redirect;
use tower_sessions::Session;

pub async fn get_logout(session: Session) -> Result<Redirect, ServerError> {
    session.remove::<i64>(crate::USER_ID_KEY).await?;

    Ok(Redirect::to("/"))
}
