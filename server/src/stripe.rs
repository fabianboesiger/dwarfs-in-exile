use axum::{
    async_trait,
    body::Body,
    extract::FromRequest,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Error, Extension,
};
use sqlx::SqlitePool;
use stripe::{Event, EventObject, EventType};

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
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), "whsec_xxxxx")
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
    match event.type_ {
        EventType::CheckoutSessionCompleted => {
            if let EventObject::CheckoutSession(session) = event.data.object {
                //log::info!("Received checkout session completed webhook with id: {:?}", session.id);

                let user_id = session
                    .client_reference_id
                    .ok_or(ServerError::StripeErrorMissingData)?
                    .parse::<i64>()?;
                
                for line_item in session
                    .line_items
                    .ok_or(ServerError::StripeErrorMissingData)?
                    .data
                {
                    let product = match line_item
                        .price
                        .ok_or(ServerError::StripeErrorMissingData)?
                        .product
                        .ok_or(ServerError::StripeErrorMissingData)?
                        .id()
                        .as_str()
                    {
                        "prod_QTnZFHdzJE4dQ5" => Product::Premium(365),
                        "prod_QTnXaJhARJBCKk" => Product::Premium(30),
                        "prod_QTnWStL89MpI6m" => Product::Premium(7),
                        _ => {
                            return Err(ServerError::StripeErrorMissingData)?;
                        }
                    };

                    match product {
                        Product::Premium(days) => {
                            let hours = days * line_item.quantity.ok_or(ServerError::StripeErrorMissingData)? as i64 * 24;

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

                            tracing::debug!("updated premium usage hours for user with id: {}", user_id);
                        }
                    }
                }

               

               
            }
        }
        _ => {}
    }

    Ok(Response::new(Body::empty()))
}
