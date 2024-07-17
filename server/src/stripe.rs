use axum::{
    async_trait,
    body::Body,
    extract::FromRequest,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Error, Extension,
};
use sqlx::SqlitePool;
use stripe::{CheckoutSession, Client, Event, EventObject, EventType};

use crate::{game::GameState, ServerError};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct StripeEvent(Event);

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

        let payload = String::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(Self(
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), &dotenv::var("STRIPE_WEBHOOK_SECRET").unwrap())
                .map_err(|_| StatusCode::BAD_REQUEST.into_response())?,
        ))
    }
}

enum Product {
    Premium(i64),
}

#[axum::debug_handler]
pub async fn handle_webhook(
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    StripeEvent(event): StripeEvent,
) -> Result<Response, ServerError> {
    tracing::info!("handling webhook");

    match event.type_ {
        EventType::CheckoutSessionCompleted => {
            if let EventObject::CheckoutSession(session) = event.data.object {
                tracing::info!("Received checkout session completed webhook with id: {:?}", &session.id);

                let user_id = session
                    .client_reference_id
                    .as_ref()
                    .ok_or(ServerError::StripeErrorMissingData(format!("missing client_reference_id, {session:?}")))?
                    .parse::<i64>()?;

                let client = Client::new(dotenv::var("STRIPE_CLIENT_SECRET").unwrap());
                let session = CheckoutSession::retrieve(&client, &session.id, &["line_items"]).await?;
                
                for line_item in &session
                    .line_items
                    .as_ref()
                    .ok_or(ServerError::StripeErrorMissingData(format!("missing line_items, {session:?}")))?
                    .data
                {
                    let product = match line_item
                        .price
                        .as_ref()
                        .ok_or(ServerError::StripeErrorMissingData(format!("missing price, {session:?}")))?
                        .product
                        .as_ref()
                        .ok_or(ServerError::StripeErrorMissingData(format!("missing product, {session:?}")))?
                        .id()
                        .as_str()
                    {
                        "prod_QTnZFHdzJE4dQ5" => Product::Premium(365),
                        "prod_QTnXaJhARJBCKk" => Product::Premium(30),
                        "prod_QTnWStL89MpI6m" => Product::Premium(7),
                        _ => {
                            return Err(ServerError::StripeErrorMissingData(format!("invalid product id, {session:?}")))?;
                        }
                    };

                    match product {
                        Product::Premium(days) => {
                            let hours = days * line_item.quantity.ok_or(ServerError::StripeErrorMissingData(format!("missing quantity, {session:?}")))? as i64 * 24;

                            sqlx::query(
                                    r#" 
                                        UPDATE users
                                        SET premium = premium + $1
                                        WHERE user_id = $2
                                    "#,
                                )
                                .bind(hours)
                                .bind(user_id)
                                .execute(&pool)
                                .await
                                .unwrap();
            
                            game_state.new_server_connection().await.updated_user_data();

                            tracing::info!("updated premium usage hours for user with id: {}", user_id);
                        }
                    }
                }

               

               
            }
        }
        _ => {}
    }

    Ok(Response::new(Body::empty()))
}
