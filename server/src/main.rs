mod game;
mod auth;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, get_service},
    Router, Extension
};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, ConnectOptions, Pool};
use std::{net::SocketAddr, path::PathBuf, str::FromStr};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum_sessions::{
    async_session::{MemoryStore, Session},
    SessionLayer,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),
    #[error(transparent)]
    AxumFormRejection(#[from] axum::extract::rejection::FormRejection),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::ValidationError(_) => {
                let message = format!("Input validation error: [{}]", self).replace('\n', ", ");
                (StatusCode::BAD_REQUEST, message)
            }
            ServerError::AxumFormRejection(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        }
        .into_response()
    }
}

async fn db_setup() -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let options = SqliteConnectOptions::from_str(&std::env::var("DATABASE_URL")?)?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    let mut transaction = pool.begin().await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            username TEXT PRIMARY KEY,
            password TEXT NOT NULL
        )
    "#)
    .execute(&mut transaction)
    .await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            username TEXT REFERENCES users(usernames),
            expires INTEGER NOT NULL
        )
    "#)
    .execute(&mut transaction)
    .await?;

    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS worlds (
            name TEXT PRIMARY KEY,
            data BLOB
        )
    "#)
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    db_setup().await?;

    /*
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    */

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("public");

    let store = MemoryStore::new();
    let secret = b"7w!z%C*F-JaNdRgUjXn2r5u8x/A?D(G+KbPeShVmYp3s6v9y$B&E)H@McQfTjWnZ";
    let session_layer = SessionLayer::new(store, secret);

    let game_state = game::GameState::new().await;
    

    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
        .route("/game", get(game::get_game))
        .route("/game/ws", get(game::ws_handler))
        .route("/register", get(auth::register::get_register).post(auth::register::post_register))
        .layer(Extension(game_state))
        .layer(session_layer);
        /*
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );
        */

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}