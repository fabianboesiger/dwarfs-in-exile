use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Redirect,
    Extension,
};
use axum_sessions::extractors::ReadableSession;
use engine_shared::utils::custom_map::CustomMap;
use futures_util::{sink::SinkExt, stream::StreamExt};
use shared::{ClientEvent, UserData, UserId};
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartialEventData {
    pub event: ClientEvent,
    pub user_id: Option<UserId>,
}

use crate::ServerError;

pub type GameState = engine_server::ServerState<shared::State>;

pub struct GameStore {
    db: SqlitePool,
}

impl GameStore {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl engine_server::BackendStore<shared::State> for GameStore {
    async fn load_game(&self) -> shared::State {
        let result: Option<(Vec<u8>,)> = sqlx::query_as(
            r#"
                    SELECT data
                    FROM games
                    WHERE name = 'game'
                "#,
        )
        .fetch_optional(&self.db)
        .await
        .unwrap();

        let state: shared::State = result
            .map(|(data,)| rmp_serde::from_slice(&data[..]).unwrap())
            .unwrap_or_default();

        state
    }

    async fn load_user_data(&self) -> CustomMap<UserId, UserData> {
        let users: Vec<(i64, String, i64)> = sqlx::query_as(
            r#"
                        SELECT user_id, username, premium
                        FROM users
                    "#,
        )
        .fetch_all(&self.db)
        .await
        .unwrap();

        let users = users
            .into_iter()
            .map(|(id, username, premium)| (id.into(), UserData {
                username,
                premium: premium == 1,
            }))
            .collect::<CustomMap<shared::UserId, shared::UserData>>();

        users
    }

    async fn save_game(&self, state: &shared::State) {
        sqlx::query(
            r#"
                    INSERT OR REPLACE INTO games (name, data)
                    VALUES ('game', $1)
                "#,
        )
        .bind(rmp_serde::to_vec(&state).unwrap())
        .execute(&self.db)
        .await
        .unwrap();
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    session: ReadableSession,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
) -> Result<Response, ServerError> {
    let result: Option<UserId> = sqlx::query_as(
        r#"
            SELECT user_id
            FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(&session.id())
    .fetch_optional(&pool)
    .await?
    .map(|(id,): (i64,)| id.into());

    if let Some(user_id) = result {
        Ok(ws.on_upgrade(move |socket: WebSocket| async move {
            let (conn_req, mut conn_res) = game_state.new_connection(user_id).await;
            let (mut sink, mut stream) = socket.split();

            tokio::select!(
                _ = async {
                    while let Some(msg) = stream.next().await {
                        if let Ok(msg) = msg {
                            if let Message::Binary(msg) = msg {
                                let req: engine_shared::Req<shared::State> = rmp_serde::from_slice(&msg).unwrap();
                                conn_req.request(req);
                            }
                        } else {
                            break;
                        }
                    }
                } => {},
                _ = async {
                    while let Some(res) = conn_res.poll().await {
                        let msg = rmp_serde::to_vec(&res).unwrap();
                        if sink.send(Message::Binary(msg)).await.is_err() {
                            break;
                        }
                    }
                } => {}
            );
        }))
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}

#[derive(Template, Default)]
#[template(path = "game.html")]
pub struct GameTemplate {}

pub async fn get_game(
    session: ReadableSession,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Response, ServerError> {
    let result: Option<(i64,)> = sqlx::query_as(
        r#"
            SELECT user_id
            FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(&session.id())
    .fetch_optional(&pool)
    .await?;

    if let Some(_user_id) = result {
        Ok(GameTemplate::default().into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}
