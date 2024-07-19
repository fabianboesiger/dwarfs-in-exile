use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path}, response::Redirect, Extension
};
use engine_shared::{utils::custom_map::CustomMap, GameId, State};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use shared::{ClientEvent, UserData, UserId, WINNER_NUM_PREMIUM_DAYS};
use sqlx::SqlitePool;
use tower_sessions::Session;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartialEventData {
    pub event: ClientEvent,
    pub user_id: Option<UserId>,
}

use crate::ServerError;

pub type GameState = engine_server::ServerState<shared::State>;

#[derive(Clone)]
pub struct GameStore {
    db: SqlitePool,
}

impl GameStore {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn load_all(&self) -> Result<GameState, sqlx::Error> {
        let mut game_state = GameState::new();
        
        let open_worlds: Vec<(GameId,)> = sqlx::query_as(
            r#"
                    SELECT id
                    FROM games
                    WHERE winner IS NULL
                "#,
        )
        .fetch_all(&self.db)
        .await?;

        for (id,) in open_worlds {
            game_state.load(self.clone(), id).await?;
        }

        Ok(game_state)
    }
}

#[async_trait::async_trait]
impl engine_server::BackendStore<shared::State> for GameStore {
    type Error = sqlx::Error;

    async fn create_game(&self) -> Result<GameId, Self::Error> {
        let (id,): (i64,) = sqlx::query_as(
            r#"
                INSERT INTO games (data, winner)
                VALUES (NULL, NULL)
                RETURNING id
            "#,
        )
        .fetch_one(&self.db)
        .await?;

        Ok(id)
    }

    async fn load_game(&self, game_id: GameId) -> Result<shared::State, Self::Error> {
        let result: Option<(Option<Vec<u8>>,)> = sqlx::query_as(
            r#"
                    SELECT data
                    FROM games
                    WHERE id = $1
                "#,
        )
        .bind(&game_id)
        .fetch_optional(&self.db)
        .await?;

        let state: shared::State = result
            .map(|(data,)| data.map(|data| rmp_serde::from_slice(&data[..]).unwrap()).unwrap_or_default())
            .unwrap();

        Ok(state)
    }

    async fn load_user_data(&self) -> Result<CustomMap<UserId, UserData>, Self::Error> {
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

        Ok(users)
    }

    async fn save_game(&self, game_id: GameId, state: &shared::State) -> Result<(), Self::Error> {
        if let Some(winner) = state.has_winner() {
            sqlx::query(
                r#"
                        UPDATE games
                        SET data = $2,
                        winner = $3
                        WHERE id = $1
                    "#,
            )
            .bind(&game_id)
            .bind(rmp_serde::to_vec(&state).unwrap())
            .bind(&winner.0)
            .execute(&self.db)
            .await
            .unwrap();

            sqlx::query(
                r#"
                        UPDATE users
                        SET premium = premium + $2,
                        WHERE id = $1
                    "#,
            )
            .bind(&winner.0)
            .bind(WINNER_NUM_PREMIUM_DAYS * 24)
            .execute(&self.db)
            .await?;
        } else {
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
            .await?;
        }

        Ok(())
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
    current_worlds: Vec<GameId>,
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
                WHERE winner IS NULL
            "#,
    )
    .fetch_all(&pool)
    .await?;

    let current_worlds: Vec<GameId> = result.into_iter().map(|(id, )| id).collect();

    if current_worlds.len() == 1 {
        Ok(Redirect::temporary(&format!("/game/{}", current_worlds[0])).into_response())
    } else {
        Ok(GameSelectTemplate { current_worlds }.into_response())
    }

}

/* 
#[derive(Template, Default)]
#[template(path = "game-select.html")]
pub struct GameSelectTemplate {
    current_worlds: Vec<GameId>,
}

pub async fn get_game_select(
    Extension(pool): Extension<SqlitePool>,
    session: Session,
) -> Result<Response, ServerError> {
    session.get::<i64>(crate::USER_ID_KEY).await?.ok_or(ServerError::InvalidSession)?;

    let result: Vec<(GameId, Option<String>)> = sqlx::query_as(
        r#"
                SELECT id, username
                FROM games
                LEFT JOIN users
                ON winner = user_id
            "#,
    )
    .fetch_all(&pool)
    .await?;

    let (current_worlds, _closed_worlds): (Vec<(GameId, Option<String>)>, Vec<(GameId, Option<String>)>) = result.into_iter().partition(|(_, winner)| {
        winner.is_none()
    });

    let current_worlds: Vec<GameId> = current_worlds.into_iter().map(|(id, _)| id).collect();
    //let closed_worlds: Vec<(GameId, String)> = closed_worlds.into_iter().map(|(id, winner)| (id, winner.unwrap())).collect();

    if current_worlds.len() == 1 {
        Ok(Redirect::temporary(&format!("/game/{}", current_worlds[0])).into_response())
    } else {
        Ok(GameSelectTemplate { current_worlds }.into_response())
    }

}
*/