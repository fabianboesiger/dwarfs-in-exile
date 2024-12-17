use crate::game::GameState;
use crate::ServerError;
use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::Extension;
use axum::{
    async_trait,
    body::Body,
    extract::FromRequest,
    http::{Request, StatusCode},
    Error,
};
use sqlx::SqlitePool;
use stripe::{CheckoutSession, Client, Event, EventObject, EventType};
use tower_sessions::Session;

#[derive(Debug, Clone, Copy)]
pub struct StoreEntry {
    buy_button_id: &'static str,
    publishable_key: &'static str,
    product_id: &'static str,
    name: &'static str,
    product: Product,
}

#[cfg(not(debug_assertions))]
static STORE_ENTRIES: &[StoreEntry] = &[
    /*StoreEntry {
        buy_button_id: "buy_btn_1PfOoiCJSYyq6ul4DXYS01wg",
        publishable_key: "pk_live_51PclDhCJSYyq6ul4z8Wmuf3h9PVDP9vXOyGhZqc4dy3JvkltdKYUt51oeD2x1K23XxEy1qeU6D80GBx3TpEE9VNN00osxE1rXe",
        product_id: "prod_QWRhg5DjHRbafp",
        name: "Premium Account (One Week)",
        product: Product::Premium(7),
    },*/
    StoreEntry {
        buy_button_id: "buy_btn_1PfOogCJSYyq6ul4UGNJGWVk",
        publishable_key: "pk_live_51PclDhCJSYyq6ul4z8Wmuf3h9PVDP9vXOyGhZqc4dy3JvkltdKYUt51oeD2x1K23XxEy1qeU6D80GBx3TpEE9VNN00osxE1rXe",
        product_id: "prod_QWRh0KcP5iPWTr",
        name: "Premium Account (One Month)",
        product: Product::Premium(30),
    },
    StoreEntry {
        buy_button_id: "buy_btn_1PfOodCJSYyq6ul4LYrCeSvJ",
        publishable_key: "pk_live_51PclDhCJSYyq6ul4z8Wmuf3h9PVDP9vXOyGhZqc4dy3JvkltdKYUt51oeD2x1K23XxEy1qeU6D80GBx3TpEE9VNN00osxE1rXe",
        product_id: "prod_QWRhdtALzBi1Os",
        name: "Premium Account (One Year)",
        product: Product::Premium(365),
    },
];

#[cfg(debug_assertions)]
static STORE_ENTRIES: &[StoreEntry] = &[
    /*StoreEntry {
        buy_button_id: "buy_btn_1Pcq8OCJSYyq6ul45QglYe5M",
        publishable_key: "pk_test_51PclDhCJSYyq6ul4shd76Uo28pNWY617Ae8OTV0NXhxZoKCIKEhLkiZRKNnLG635zpSIKJS8eGLPNaKqFtatiZLA00KocaOW8X",
        product_id: "prod_QTnWStL89MpI6m",
        name: "Premium Account (One Week)",
        product: Product::Premium(7),
    },*/
    StoreEntry {
        buy_button_id: "buy_btn_1Pcq8tCJSYyq6ul4f4jhctou",
        publishable_key: "pk_test_51PclDhCJSYyq6ul4shd76Uo28pNWY617Ae8OTV0NXhxZoKCIKEhLkiZRKNnLG635zpSIKJS8eGLPNaKqFtatiZLA00KocaOW8X",
        product_id: "prod_QTnXaJhARJBCKk",
        name: "Premium Account (One Month)",
        product: Product::Premium(30),
    },
    StoreEntry {
        buy_button_id: "buy_btn_1Pcq9GCJSYyq6ul4PQ5OshG9",
        publishable_key: "pk_test_51PclDhCJSYyq6ul4shd76Uo28pNWY617Ae8OTV0NXhxZoKCIKEhLkiZRKNnLG635zpSIKJS8eGLPNaKqFtatiZLA00KocaOW8X",
        product_id: "prod_QTnZFHdzJE4dQ5",
        name: "Premium Account (One Year)",
        product: Product::Premium(365),
    },
];

#[derive(Template, Default)]
#[template(path = "store.html")]
pub struct StoreTemplate {
    user_id: Option<i64>,
    username: Option<String>,
    guest: bool,
    store_entries: &'static [StoreEntry],
}

pub async fn get_store(
    session: Session,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    if let Some(user_id) = session.get::<i64>(crate::USER_ID_KEY).await? {
        let (username, user_id, guest): (String, i64, i64) = sqlx::query_as(
            r#"
                SELECT username, user_id, guest
                FROM users
                WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&pool)
        .await?
        .ok_or(ServerError::UserDeleted)?;

        Ok(StoreTemplate {
            username: Some(username),
            user_id: Some(user_id),
            guest: guest != 0,
            store_entries: STORE_ENTRIES,
            ..StoreTemplate::default()
        }
        .into_response())
    } else {
        Ok(StoreTemplate {
            username: None,
            user_id: None,
            guest: false,
            store_entries: STORE_ENTRIES,
        }
        .into_response())
    }
}

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
            tracing::warn!("missing stripe-signature header");
            return Err(StatusCode::BAD_REQUEST.into_response());
        };

        let payload = String::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        Ok(Self(
            stripe::Webhook::construct_event(
                &payload,
                signature.to_str().unwrap(),
                &dotenv::var("STRIPE_WEBHOOK_SECRET").unwrap(),
            )
            .map_err(|_| {
                tracing::warn!("failed to construct stripe event");
                StatusCode::BAD_REQUEST.into_response()
            })?,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
enum Product {
    Premium(i64),
    Skin(i64),
}

#[axum::debug_handler]
pub async fn handle_webhook(
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
    StripeEvent(event): StripeEvent,
) -> Result<Response, ServerError> {
    let span = tracing::span!(tracing::Level::INFO, "handle_webhook");
    let _enter = span.enter();

    tracing::info!("handling webhook");

    if event.type_ == EventType::CheckoutSessionCompleted {
        if let EventObject::CheckoutSession(session) = event.data.object {
            tracing::info!(
                "Received checkout session completed webhook with id: {:?}",
                &session.id
            );

            let user_id = session
                .client_reference_id
                .as_ref()
                .ok_or(ServerError::StripeErrorMissingData(format!(
                    "missing client_reference_id, {session:?}"
                )))?
                .parse::<i64>()?;

            let client = Client::new(dotenv::var("STRIPE_CLIENT_SECRET").unwrap());
            let session =
                CheckoutSession::retrieve(&client, &session.id, &["line_items"]).await?;

            for line_item in &session
                .line_items
                .as_ref()
                .ok_or(ServerError::StripeErrorMissingData(format!(
                    "missing line_items, {session:?}"
                )))?
                .data
            {
                let product_id = line_item
                    .price
                    .as_ref()
                    .ok_or(ServerError::StripeErrorMissingData(format!(
                        "missing price, {session:?}"
                    )))?
                    .product
                    .as_ref()
                    .ok_or(ServerError::StripeErrorMissingData(format!(
                        "missing product, {session:?}"
                    )))?
                    .id();

                let mut store_entry: Option<StoreEntry> = None;

                for e in STORE_ENTRIES {
                    if e.product_id == product_id.as_str() {
                        store_entry = Some(*e);
                        break;
                    }
                }

                let store_entry = store_entry.ok_or(ServerError::StripeErrorMissingData(
                    format!("unknown product id {product_id}"),
                ))?;

                tracing::info!("store entry found {:?}", store_entry.name);

                match store_entry.product {
                    Product::Premium(days) => {
                        let hours = days
                            * line_item
                                .quantity
                                .ok_or(ServerError::StripeErrorMissingData(format!(
                                    "missing quantity, {session:?}"
                                )))? as i64
                            * 24;

                        sqlx::query(
                            r#"
                                    UPDATE users
                                    SET premium = premium + $1
                                    WHERE user_id = $2
                                    OR user_id = (
                                        SELECT referrer
                                        FROM users
                                        WHERE user_id = $2
                                        LIMIT 1
                                    )
                                "#,
                        )
                        .bind(hours)
                        .bind(user_id)
                        .execute(&pool)
                        .await
                        .unwrap();

                        game_state.new_server_connection().await.updated_user_data();

                        tracing::info!(
                            "updated premium usage hours for user with id: {}",
                            user_id
                        );
                    },
                    Product::Skin(skin) => {
                        let quantity = line_item
                                .quantity
                                .ok_or(ServerError::StripeErrorMissingData(format!(
                                    "missing quantity, {session:?}"
                                )))? as i64;

                        for _ in 0..quantity {
                            sqlx::query(
                                r#"
                                    UPDATE users
                                    SET skins = json_insert(skins, '$[#]', $1)
                                    WHERE user_id = $2
                                "#,
                                )
                                .bind(skin)
                                .bind(user_id)
                                .execute(&pool)
                                .await
                                .unwrap();
                        }
   

                        game_state.new_server_connection().await.updated_user_data();

                        tracing::info!(
                            "updated premium usage hours for user with id: {}",
                            user_id
                        );
                    }
                }
            }
        }
    }

    Ok(Response::new(Body::empty()))
}
