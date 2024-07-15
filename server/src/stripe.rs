use axum::{
    async_trait, body::Body, extract::FromRequest, http::{Request, StatusCode}, response::{IntoResponse, Response}, routing::post, Error, Extension, Router
};
use sqlx::SqlitePool;
use stripe::{Event, EventObject, EventType};

use crate::game::GameState;

pub type Result<T, E = Error> = std::result::Result<T, E>;

struct StripeEvent(Event);

#[async_trait]
impl<S> FromRequest<S> for StripeEvent
where
    String: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let signature = if let Some(sig) = req.headers().get("stripe-signature") {
            sig.to_owned()
        } else {
            return Err(StatusCode::BAD_REQUEST.into_response());
        };

        let payload =
            String::from_request(req, state).await.map_err(IntoResponse::into_response)?;

        Ok(Self(
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), "whsec_xxxxx")
                .map_err(|_| StatusCode::BAD_REQUEST.into_response())?,
        ))
    }
}

#[axum::debug_handler]
async fn handle_webhook(
    StripeEvent(event): StripeEvent,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
) {
    match event.type_ {
        EventType::CheckoutSessionCompleted => {
            if let EventObject::CheckoutSession(session) = event.data.object {
                //log::info!("Received checkout session completed webhook with id: {:?}", session.id);


                if let Some(user_id) = session.client_reference_id.and_then(|client_reference_id| client_reference_id.parse::<i64>().ok()) {
                    sqlx::query(
                        r#"
                                UPDATE users
                                SET premium = 1
                                WHERE user_id = $2
                            "#,
                        )
                        .bind(&user_id)
                        .execute(&pool)
                        .await
                        .unwrap();
    
                    game_state.new_server_connection().await.updated_user_data();
                }
            }
        }
        _ => {},
    }
}