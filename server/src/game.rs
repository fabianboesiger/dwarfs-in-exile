use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path}, response::Redirect, Extension
};
use engine_shared::{utils::custom_map::CustomMap, GameId};
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
    async fn load_game(&self, game_id: GameId) -> shared::State {
        let result: Option<(Vec<u8>,)> = sqlx::query_as(
            r#"
                    SELECT data
                    FROM games
                    WHERE id = $1
                "#,
        )
        .bind(&game_id)
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
                        premium: premium as u64,
                    },
                )
            })
            .collect::<CustomMap<shared::UserId, shared::UserData>>();

        users
    }

    async fn save_game(&self, game_id: GameId, state: &shared::State) {
        sqlx::query(
            r#"
                    UPDATE games
                    SET data = $2
                    WHERE id = $1
                "#,
        )
        .bind(&game_id)
        .bind(rmp_serde::to_vec(&state).unwrap())
        .execute(&self.db)
        .await
        .unwrap();
    }
}

#[axum::debug_handler]
pub async fn ws_handler(
    Path(game_id): Path<GameId>,
    ws: WebSocketUpgrade,
    session: Session,
    Extension(game_state): Extension<GameState>,
) -> Result<Response, ServerError> {
    tracing::info!("new websocket connection");
    let user_id = UserId(session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?);

    Ok(ws.on_upgrade(move |socket: WebSocket| async move {
        let (conn_req, mut conn_res) = game_state.new_connection(user_id, game_id).await;
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
    Path(_game_id): Path<usize>,
    session: Session,
) -> Result<Response, ServerError> {
    session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?;

    Ok(GameTemplate::default().into_response())
}


#[derive(Template, Default)]
#[template(path = "game-select.html")]
pub struct GameSelectTemplate {
    ids: Vec<GameId>,
}

pub async fn get_game_select(
    Extension(pool): Extension<SqlitePool>,
    session: Session,
) -> Result<Response, ServerError> {
    session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?;

    let result: Vec<(GameId,)> = sqlx::query_as(
        r#"
                SELECT id
                FROM games
            "#,
    )
    .fetch_all(&pool)
    .await?;

    let ids: Vec<GameId> = result.into_iter().map(|(id,)| id).collect();

    if ids.len() == 1 {
        Ok(Redirect::temporary(&format!("/game/{}", ids[0])).into_response())
    } else {
        Ok(GameSelectTemplate { ids }.into_response())
    }

}