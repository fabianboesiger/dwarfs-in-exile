mod about;
mod auth;
mod db;
mod error;
mod game;
mod index;
mod stripe;

use error::*;

use axum::{
    routing::{get, get_service, post},
    Extension, Router,
};
use game::{GameState, GameStore};
use std::{net::SocketAddr, path::PathBuf};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, SessionManagerLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let pool = db::setup().await?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");

    let store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(store)
        .with_secure(false)
        .with_http_only(false)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(10)));

    let store = GameStore::new(pool.clone());
    let game_state = GameState::new(store).await;

    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true)), /*
                                                                                           .handle_error(|error: std::io::Error| async move {
                                                                                               (
                                                                                                   StatusCode::INTERNAL_SERVER_ERROR,
                                                                                                   format!("Unhandled internal error: {}", error),
                                                                                               )
                                                                                           }),
                                                                                           */
        )
        .route("/", get(index::get_index))
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
    tracing::debug!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
