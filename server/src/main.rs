mod about;
mod auth;
mod db;
mod error;
mod game;
mod index;
mod stripe;
mod admin;

use auth::store;
use error::*;

use axum::{
    routing::{get, get_service, post},
    Extension, Router,
};
use game::GameStore;
use tokio::{task, time};
use std::{net::{SocketAddr, SocketAddrV4}, path::PathBuf, str::FromStr, time::Duration};
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
            dotenv::var("RUST_LOG")
                .unwrap_or_else(|_| "sqlx=warn,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("starting server...");
    
    let pool = db::setup().await?;

    let store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(store)
        .with_secure(false)
        .with_http_only(false)
        .with_expiry(Expiry::OnInactivity(tower_sessions::cookie::time::Duration::days(30)));

    let game_state = GameStore::new(pool.clone()).load_all().await?;
    
    // Manage the number of hours for premium accounts.
    let pool_clone = pool.clone();
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60));
        // The first tick fires immediately, we don't want that so we await it directly.
        interval.tick().await;

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
            get_service(ServeDir::new(dotenv::var("PUBLIC_DIR").unwrap()).append_index_html_on_directories(true))                                                                              
        )
        .route("/", get(index::get_index))
        .route("/store", get(store::get_store))
        .route("/about", get(about::get_about))
        .route("/game", get(game::get_game_select))
        .route("/game/:game_id/ws", get(game::ws_handler))
        .route("/game/:game_id", get(game::get_game))
        .route("/game/:game_id/*subpath", get(game::get_game))
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
        .route("/admin", get(admin::get_admin))
        .route("/stripe-webhooks", post(stripe::handle_webhook))
        .layer(Extension(game_state))
        .layer(Extension(pool.clone()))
        .layer(session_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );
    
    let addr = SocketAddrV4::from_str(&dotenv::var("SERVER_ADDRESS").unwrap()).unwrap();
    let addr = SocketAddr::from(addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    tracing::info!("listening on {}", addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}