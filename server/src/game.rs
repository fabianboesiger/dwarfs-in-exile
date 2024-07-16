use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    Extension,
};
use engine_shared::utils::custom_map::CustomMap;
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use shared::{ClientEvent, UserData, UserId};
use sqlx::SqlitePool;
use tower_sessions::Session;

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
            .map(|(id, username, premium)| {
                (
                    id.into(),
                    UserData {
                        username,
                        premium: premium == 1,
                    },
                )
            })
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
    session: Session,
    Extension(game_state): Extension<GameState>,
) -> Result<Response, ServerError> {
    tracing::info!("new websocket connection");
    let user_id = UserId(session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?);

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
}

#[derive(Template, Default)]
#[template(path = "game.html")]
pub struct GameTemplate {}

pub async fn get_game(
    session: Session,
) -> Result<Response, ServerError> {
    session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?;

    Ok(GameTemplate::default().into_response())
}
