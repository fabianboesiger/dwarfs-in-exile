use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Redirect,
    Extension,
};
use axum_sessions::extractors::ReadableSession;
use futures_util::{sink::SinkExt, stream::StreamExt};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use shared::{Event, EventData, SyncData, UserId, SPEED};
use sqlx::SqlitePool;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, mpsc, RwLock},
    time,
};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartialEventData {
    pub event: Event,
    pub user_id: Option<UserId>,
}

use crate::ServerError;

#[derive(Clone)]
pub struct GameState(Arc<GameStateImpl>);

struct GameStateImpl {
    state: RwLock<shared::State>,
    res_sender: broadcast::Sender<EventData>,
    req_sender: mpsc::UnboundedSender<PartialEventData>,
}

impl GameState {
    async fn load_game(pool: &SqlitePool) -> Option<shared::State> {
        let result: Result<Option<(Vec<u8>,)>, _> = sqlx::query_as(
            r#"
                SELECT data
                FROM worlds
                WHERE name = 'world'
            "#,
        )
        .fetch_optional(pool)
        .await;

        result
            .unwrap()
            .map(|(data,)| rmp_serde::from_slice(&data[..]).unwrap())
    }

    async fn store_game(pool: &SqlitePool, state: &shared::State) {
        sqlx::query(
            r#"
                INSERT OR REPLACE INTO worlds (name, data)
                VALUES ('world', $1)
            "#,
        )
        .bind(rmp_serde::to_vec(state).unwrap())
        .execute(pool)
        .await
        .unwrap();
    }

    pub async fn new(pool: SqlitePool) -> GameState {
        let (req_sender, mut req_receiver) = mpsc::unbounded_channel::<PartialEventData>();
        let (res_sender, _res_receiver) = broadcast::channel::<EventData>(64);

        let req_sender_clone = req_sender.clone();

        let game = RwLock::new(GameState::load_game(&pool).await.unwrap_or_default());
        //let game = RwLock::new(State::default());
        let game_state = Arc::new(GameStateImpl {
            state: game,
            res_sender,
            req_sender,
        });
        let game_state_clone = game_state.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(1000 / SPEED));

            loop {
                interval.tick().await;

                req_sender_clone
                    .send(PartialEventData {
                        event: Event::Tick,
                        user_id: None,
                    })
                    .unwrap();
            }
        });

        tokio::spawn(async move {
            let GameStateImpl {
                state: game,
                res_sender,
                ..
            } = &*game_state_clone;

            let mut rng = SmallRng::from_entropy();

            while let Some(PartialEventData {
                event,
                user_id,
            }) = req_receiver.recv().await
            {
                let event = match event {
                    // Valid only as server-sent events.
                    Event::Tick
                    | Event::AddPlayer(_, _)
                    | Event::EditPlayer(_, _)
                    | Event::RemovePlayer(_)
                        if user_id.is_some() =>
                    {
                        None
                    }
                    event => Some(event),
                };

                if let Some(event) = event {
                    let state = game.read().await;

                    let event = EventData {
                        user_id,
                        event,
                        state_hash: state.hash_value(),
                        event_idx: state.next_event_idx,
                        seed: rng.gen()
                    };

                    drop(state);

                    res_sender.send(event.clone()).ok();
                    if game.write().await.update(event).is_none() {
                        //tracing::event!(tracing::Level::WARN, "invalid message received, ignoring it.");
                    }
                    let state = &*game.read().await;
                    if state.time % SPEED == 0 {
                        GameState::store_game(&pool, state).await;
                    }
                }
            }
        });

        GameState(game_state)
    }

    pub async fn new_connection(
        &self,
        user_id: UserId,
    ) -> (
        shared::State,
        mpsc::UnboundedSender<PartialEventData>,
        broadcast::Receiver<EventData>,
    ) {
        (
            self.0.state.read().await.view(user_id),
            self.0.req_sender.clone(),
            self.0.res_sender.subscribe(),
        )
    }

    pub fn add_player(&self, user_id: UserId, username: String) {
        self.0
            .req_sender
            .send(PartialEventData {
                event: Event::AddPlayer(user_id, username),
                user_id: None,
            })
            .unwrap();
    }

    pub fn edit_player(&self, user_id: UserId, username: String) {
        self.0
            .req_sender
            .send(PartialEventData {
                event: Event::AddPlayer(user_id, username),
                user_id: None,
            })
            .unwrap();
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    session: ReadableSession,
    Extension(pool): Extension<SqlitePool>,
    Extension(game_state): Extension<GameState>,
) -> Result<Response, ServerError> {
    let result: Option<(UserId,)> = sqlx::query_as(
        r#"
            SELECT user_id
            FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(&session.id())
    .fetch_optional(&pool)
    .await?;

    if let Some((user_id,)) = result {
        Ok(ws.on_upgrade(move |socket: WebSocket| async move {
            let (state, sender, mut receiver) = game_state.new_connection(user_id).await;
            let (mut sink, mut stream) = socket.split();

            let msg = rmp_serde::to_vec(&shared::Res::Sync(SyncData {
                user_id,
                state
            })).unwrap();

            if sink.send(Message::Binary(msg)).await.is_err() {
                return;
            }

            tokio::select!(
                _ = async {
                    while let Some(msg) = stream.next().await {
                        if let Ok(msg) = msg {
                            if let Message::Binary(msg) = msg {
                                let req: shared::Req = rmp_serde::from_slice(&msg).unwrap();
                                match req {
                                    shared::Req::Event(event) => {
                                        if sender.send(PartialEventData {event, user_id: Some(user_id) }).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                } => {},
                _ = async {
                    loop {
                        match receiver.recv().await {
                            Ok(event) => {
                                if event.filter(user_id) {
                                    let msg = rmp_serde::to_vec(&shared::Res::Event(event)).unwrap();
                                    if sink.send(Message::Binary(msg)).await.is_err() {
                                        break;
                                    }
                                }
                            },
                            // If a broadcast message is discarded that wasn't seen yet by this receiver,
                            // request a full game state update.
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                let (state, _, new_receiver) = game_state.new_connection(user_id).await;
                                receiver = new_receiver;
                                let msg = rmp_serde::to_vec(&shared::Res::Sync(SyncData {
                                    user_id,
                                    state
                                })).unwrap();
                                if sink.send(Message::Binary(msg)).await.is_err() {
                                    break;
                                }
                            },
                            _ => {
                                break;
                            }
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
    let result: Option<(UserId,)> = sqlx::query_as(
        r#"
            SELECT user_id
            FROM sessions
            WHERE session_id = $1
        "#,
    )
    .bind(&session.id())
    .fetch_optional(&pool)
    .await?;

    if let Some((_user_id,)) = result {
        Ok(GameTemplate::default().into_response())
    } else {
        Ok(Redirect::to("/login").into_response())
    }
}
