use std::str::FromStr;

use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Redirect,
    Extension,
};
use engine_server::BackendStore;
use engine_shared::{utils::custom_map::CustomMap, GameId};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use shared::{ClientEvent, GameMode, UserData, UserId};
use sqlx::SqlitePool;
use tower_sessions::Session;
use engine_shared::{State, Settings};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartialEventData {
    pub event: ClientEvent,
    pub user_id: Option<UserId>,
}

use crate::ServerError;

pub type GameState = engine_server::ServerState<shared::State, GameStore>;

#[derive(Clone)]
pub struct GameStore {
    db: SqlitePool,
}

impl GameStore {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn load_all(self) -> Result<GameState, ServerError> {
        let open_worlds: Vec<(GameId, String)> = sqlx::query_as(
            r#"
                    SELECT id, game_mode
                    FROM games
                    WHERE closed = 0
                "#,
        )
        .fetch_all(&self.db)
        .await?;

        let pool = self.db.clone();
        let game_state = GameState::new(self);

        for (id, game_mode) in open_worlds {
            let game_finished = game_state.load(id).await?;
            let game_state_clone = game_state.clone();
            let pool_clone = pool.clone();

            tokio::task::spawn(async move {
                game_finished.notified().await;

                let result: Result<(i64,), _> = sqlx::query_as(
                    r#"
                        SELECT auto_start_world
                        FROM settings
                        LIMIT 1
                    "#,
                )
                .fetch_one(&pool_clone)
                .await;
            
                if result.map(|result| result.0 != 0).unwrap_or(false) {
                    game_state_clone.create(GameMode::from_str(&game_mode).unwrap_or(GameMode::Ranked)).await.unwrap();
                }
            });
        }

        Ok(game_state)
    }
}

#[async_trait::async_trait]
impl engine_server::BackendStore<shared::State> for GameStore {
    type Error = ServerError;

    async fn create_game(&self, gamemode: GameMode) -> Result<GameId, Self::Error> {
        let data: Vec<u8> = rmp_serde::to_vec(&shared::State::new(gamemode)).unwrap();

        let (id,): (i64,) = sqlx::query_as(
            r#"
                INSERT INTO games (data, winner, game_mode)
                VALUES ($1, NULL, $2)
                RETURNING id
            "#,
        )
        .bind(data)
        .bind(gamemode.to_string())
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
        .bind(game_id)
        .fetch_optional(&self.db)
        .await?;

        let state: shared::State = result
            .map(|(data,)| {
                data.map(|data| rmp_serde::from_slice(&data[..]).unwrap())
                    .unwrap()
            })
            .unwrap();

        Ok(state)
    }

    async fn load_user_data(&self) -> Result<CustomMap<UserId, UserData>, Self::Error> {
        let users: Vec<(i64, String, i64, i64, i64, i64, time::PrimitiveDateTime, Option<i64>, String)> =
            sqlx::query_as(
                r#"
                        SELECT user_id, username, premium, admin, COUNT(winner), guest, joined, referrer, dwarf_skins
                        FROM users
                        LEFT JOIN games ON winner = user_id
                        GROUP BY user_id, username, premium, admin, guest, joined, referrer
                    "#,
            )
            .fetch_all(&self.db)
            .await
            .unwrap();

        let users = users
            .into_iter()
            .map(|(id, username, premium, admin, games_won, guest, joined, referrer, dwarf_skins)| {
                (
                    id.into(),
                    UserData {
                        username,
                        premium: premium as u64,
                        admin: admin != 0,
                        games_won,
                        guest: guest != 0,
                        joined,
                        referrer: referrer.map(|id| UserId(id)),
                        dwarf_skins: dwarf_skins
                            .split(',')
                            .filter_map(|s| shared::SpecialDwarf::from_str(s).ok())
                            .collect(),
                    },
                )
            })
            .collect::<CustomMap<shared::UserId, shared::UserData>>();

        Ok(users)
    }

    async fn save_game(&self, game_id: GameId, state: &shared::State) -> Result<(), Self::Error> {
        if let Some(winner) = state.winner() {
            sqlx::query(
                r#"
                        UPDATE games
                        SET data = NULL,
                        winner = $2,
                        closed = 1
                        WHERE id = $1
                    "#,
            )
            .bind(game_id)
            .bind(winner.0)
            .execute(&self.db)
            .await?;

            tracing::info!("game {} saved, ingame time {}", game_id, state.time);

            if state.settings().is_ranked() {
                for (user_id, premium_days) in state.rewarded_premium_days() {
                    sqlx::query(
                        r#"
                                UPDATE users
                                SET premium = premium + $2
                                WHERE user_id = $1
                            "#,
                    )
                    .bind(user_id.0)
                    .bind(premium_days * 24)
                    .execute(&self.db)
                    .await?;
                }
            }
            
        } else {
            sqlx::query(
                r#"
                        UPDATE games
                        SET data = $2
                        WHERE id = $1
                    "#,
            )
            .bind(game_id)
            .bind(rmp_serde::to_vec(&state)?)
            .execute(&self.db)
            .await?;

            tracing::info!("game {} saved, ingame time {}", game_id, state.time);
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
    tracing::info!("starting new websocket connection");

    let user_id = UserId(
        session
            .get::<i64>(crate::USER_ID_KEY)
            .await?
            .ok_or(ServerError::InvalidSession)?,
    );

    tracing::info!("user {} connecting to game {}", user_id.0, game_id);

    Ok(ws.on_upgrade(move |socket: WebSocket| async move {
        tracing::info!("websocket connection upgraded");

        if let Ok((conn_req, mut conn_res)) = game_state.new_connection(user_id, game_id).await {
            let (mut sink, mut stream) = socket.split();

            tracing::info!("new websocket connection for game {}", game_id);

            tokio::select!(
                _ = async {
                    while let Some(msg) = stream.next().await {
                        if let Ok(msg) = msg {
                            tracing::debug!("got message");

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
                    while let Ok(Some(res)) = conn_res.poll().await {
                        tracing::debug!("sending response");

                        let msg = rmp_serde::to_vec(&res).unwrap();
                        if sink.send(Message::Binary(msg)).await.is_err() {
                            break;
                        }
                    }
                } => {}
            );
        }
    }))
}

#[derive(Template, Default)]
#[template(path = "game.html")]
pub struct GameTemplate {}

pub async fn get_game(
    Path(_game_id): Path<usize>,
    session: Session,
) -> Result<Response, ServerError> {
    session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    Ok(GameTemplate::default().into_response())
}

#[derive(Template, Default)]
#[template(path = "game-select.html")]
pub struct GameSelectTemplate {
    current_worlds: Vec<(GameId, String)>,
}

pub async fn get_game_select(
    Extension(pool): Extension<SqlitePool>,
    session: Session,
) -> Result<Response, ServerError> {
    session
        .get::<i64>(crate::USER_ID_KEY)
        .await?
        .ok_or(ServerError::InvalidSession)?;

    let result: Vec<(GameId,String,)> = sqlx::query_as(
        r#"
                SELECT id, game_mode
                FROM games
                WHERE closed = 0
            "#,
    )
    .fetch_all(&pool)
    .await?;

    let current_worlds: Vec<(GameId, String)> = result.into_iter().map(|(id, game_mode,)| (id, game_mode)).collect();

    if current_worlds.len() == 1 {
        Ok(Redirect::temporary(&format!("/game/{}", current_worlds[0].0)).into_response())
    } else {
        Ok(GameSelectTemplate { current_worlds }.into_response())
    }
}

#[derive(Template, Default)]
#[template(path = "valhalla.html")]
pub struct ValhallaTemplate {
    users: Vec<UserData>,
}

pub async fn get_valhalla(Extension(pool): Extension<SqlitePool>) -> Result<Response, ServerError> {
    let mut users = GameStore::new(pool).load_user_data().await?
        .into_iter()
        .map(|(_, user_data)| {
            user_data
        })
        .filter(|user_data| user_data.games_won > 0)
        .collect::<Vec<_>>();

    users.sort_by_key(|user_data| {
        (
            -user_data.games_won,
            -(user_data.premium as i64),
            user_data.username.clone(),
        )
    });

    Ok(ValhallaTemplate { users }.into_response())
}
