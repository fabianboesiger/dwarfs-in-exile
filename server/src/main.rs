mod about;
mod auth;
mod db;
mod error;
mod game;
mod index;
mod stripe;

use auth::store;
use error::*;

use axum::{
    routing::{get, get_service, post},
    Extension, Router,
};
use game::{GameState, GameStore};
use tokio::{task, time};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub const USER_ID_KEY: &str = "user_id";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();


    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "sqlx=warn,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("starting server...");
    
    let pool = db::setup().await?;

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");

    let store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(store)
        .with_secure(false)
        .with_http_only(false)
        .with_expiry(Expiry::OnInactivity(tower_sessions::cookie::time::Duration::days(30)));

    let store = GameStore::new(pool.clone());
    let game_state = GameState::new(store).await;
    
    let pool_clone = pool.clone();
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60));

        loop {
            interval.tick().await;

            sqlx::query(
                    r#" 
                        UPDATE users
                        SET premium = premium - 1
                        WHERE premium > 0
                    "#,
                )
                .execute(&pool_clone)
                .await
                .unwrap();

            tracing::debug!("updated premium usage hours for all users");
        }
    });

    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))                                                                              
        )
        .route("/", get(index::get_index))
        .route("/store", get(store::get_store))
        .route("/about", get(about::get_about))
        .route("/game/ws", get(game::ws_handler))
        .route("/game", get(game::get_game))
        .route("/game/*subpath", get(game::get_game))
        .route(
            "/register",
            get(auth::register::get_register).post(auth::register::post_register),
        )
        .route(
            "/login",
            get(auth::login::get_login).post(auth::login::post_login),
        )
        .route("/logout", get(auth::logout::get_logout))
        .route("/account", get(auth::account::get_account))
        .route(
            "/change-username",
            get(auth::change_username::get_change_username),
        )
        .route(
            "/change-username",
            post(auth::change_username::post_change_username),
        )
        .route(
            "/change-password",
            get(auth::change_password::get_change_password),
        )
        .route(
            "/change-password",
            post(auth::change_password::post_change_password),
        )
        .route("/stripe-webhook", post(stripe::handle_webhook))
        .layer(Extension(game_state))
        .layer(Extension(pool.clone()))
        .layer(session_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    tracing::info!("listening on {}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}