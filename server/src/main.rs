mod about;
mod admin;
mod auth;
mod db;
mod error;
mod game;
mod index;
mod store;
mod wiki;

use engine_shared::utils::custom_map::CustomMap;
use error::*;

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, Response},
    middleware::{self, Next},
    routing::{get, get_service, post},
    Extension, Router,
};
use game::GameStore;
use std::{
    net::{SocketAddr, SocketAddrV4},
    str::FromStr,
    time::Duration,
};
use tokio::task;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tower_sessions::{cookie::SameSite, ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub const USER_ID_KEY: &str = "user_id";

async fn set_static_cache_control(request: Request, next: Next) -> Response<Body> {
    let cache_uri = request.uri().to_string();

    if cache_uri.contains(".jpg") || cache_uri.contains(".png") {
        let mut response = next.run(request).await;
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=2592000"),
        );
        response
    } else if cache_uri.contains(".wasm") || cache_uri.contains(".js") || cache_uri.contains(".css")
    {
        let mut response = next.run(request).await;
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=0"),
        );
        response
    } else {
        next.run(request).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            dotenv::var("RUST_LOG").unwrap_or_else(|_| "sqlx=warn,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("starting server ...");

    let pool = db::setup().await?;

    /*
    let store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(store)
        .with_http_only(false)
        .with_expiry(Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::days(30),
        ));
    */

    let session_store = SqliteStore::new(pool.clone());
    session_store.migrate().await?;

    let _deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(60 * 60)),
    );

    let pool_clone = pool.clone();
    let _guest_deletion_task = task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));

        loop {
            interval.tick().await;

            sqlx::query(
                r#"
                        DELETE FROM users
                        WHERE guest
                        AND CURRENT_TIMESTAMP > DATE(joined, '+30 days')
                    "#,
            )
            .execute(&pool_clone)
            .await
            .unwrap();
        }
    });

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(30)));

    let game_state = GameStore::new(pool.clone()).load_all().await?;

    // Manage the number of hours for premium accounts.
    let pool_clone = pool.clone();
    let game_state_clone = game_state.clone();
    task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        // The first tick fires immediately, we don't want that so we await it directly.
        interval.tick().await;

        loop {
            interval.tick().await;

            let mut active_users = CustomMap::new();
            let mut num_active_games = 0;

            game_state_clone
                .read_games(|game| {
                    if engine_shared::State::has_winner(game).is_none() {
                        num_active_games += 1;

                        for (user_id, player) in game.players.iter() {
                            active_users
                                .entry(*user_id)
                                .and_modify(|level| {
                                    if player.base.curr_level > *level {
                                        *level = player.base.curr_level;
                                    }
                                })
                                .or_insert(player.base.curr_level);
                        }
                    }
                })
                .await;

            tracing::debug!("active users: {:?}", active_users);

            for (user_id, _level) in &active_users {
                sqlx::query(
                    r#"
                            UPDATE users
                            SET premium = premium - 1
                            WHERE premium > 0
                            AND user_id = $1
                        "#,
                )
                .bind(user_id.0)
                .execute(&pool_clone)
                .await
                .unwrap();
            }

            /*if num_active_games == 0 {
                game_state_clone.create().await.unwrap();
            }*/
        }
    });

    // build our application with some routes
    let app = Router::new()
        .fallback(
            get_service(
                ServeDir::new(dotenv::var("PUBLIC_DIR").unwrap())
                    .precompressed_br()
                    .precompressed_gzip(),
            )
            .layer(middleware::from_fn(set_static_cache_control)),
        )
        .route("/", get(index::get_index))
        .route("/wiki", get(wiki::get_wiki))
        .route("/store", get(store::get_store))
        .route("/about", get(about::get_about))
        .route("/valhalla", get(game::get_valhalla))
        .nest(
            "/game",
            Router::new()
                .route("/", get(game::get_game_select))
                .route("/:game_id/ws", get(game::ws_handler))
                .nest_service("/:game_id", get(game::get_game)),
        )
        .route(
            "/register",
            get(auth::register::get_register).post(auth::register::post_register),
        )
        .route("/register-guest", get(auth::register::get_register_guest))
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
        .route(
            "/delete-account",
            get(auth::delete_account::get_delete_account),
        )
        .route(
            "/delete-account",
            post(auth::delete_account::post_delete_account),
        )
        .route("/admin", get(admin::get_admin))
        .route("/admin/manage-user", post(admin::post_manage_user))
        .route("/admin/create-world", post(admin::post_create_world))
        .route("/admin/update-settings", post(admin::post_update_settings))
        .route("/admin/add-premium", post(admin::post_add_premium))
        .route("/stripe-webhooks", post(store::handle_webhook))
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
