use askama::Template;
use askama_axum::{IntoResponse, Response};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Redirect,
    Extension,
};
use axum_sessions::async_session::Session;
use futures_util::{sink::SinkExt, stream::StreamExt};
use shared::UserId;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::ServerError;

#[derive(Clone)]
pub struct GameState(Arc<GameStateImpl>);

struct GameStateImpl {
    game: RwLock<shared::State>,
    res_sender: broadcast::Sender<shared::Event>,
    req_sender: mpsc::Sender<shared::Event>,
}

impl GameState {
    async fn load_game() -> Option<shared::State> {
        None
    }

    pub async fn new() -> GameState {
        let (req_sender, mut req_receiver) = mpsc::channel::<shared::Event>(128);
        let (res_sender, res_receiver) = broadcast::channel::<shared::Event>(128);

        let game = RwLock::new(GameState::load_game().await.unwrap_or_default());
        let game_state = Arc::new(GameStateImpl {
            game,
            res_sender,
            req_sender,
        });
        let game_state_clone = game_state.clone();

        tokio::spawn(async move {
            let GameStateImpl {
                game,
                res_sender,
                req_sender,
            } = &*game_state_clone;

            while let Some(event) = req_receiver.recv().await {
                res_sender.send(event.clone()).ok();
                game.write().await.update(event);
            }
        });

        GameState(game_state)
    }

    pub async fn new_connection(
        &self,
    ) -> (
        shared::State,
        mpsc::Sender<shared::Event>,
        broadcast::Receiver<shared::Event>,
    ) {
        (
            self.0.game.read().await.clone(),
            self.0.req_sender.clone(),
            self.0.res_sender.subscribe(),
        )
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(session): Extension<Session>,
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
    .await
    .unwrap();

    if let Some((user_id,)) = result {
        Ok(ws.on_upgrade(move |socket: WebSocket| async move {
            let (mut game, sender, mut receiver) = game_state.new_connection().await;
            let (mut sink, mut stream) = socket.split();
    
            let msg = rmp_serde::to_vec(&shared::Res::Sync(game.clone())).unwrap();
            if sink.send(Message::Binary(msg)).await.is_err() {
                return;
            }
    
            tokio::select!(
                _ = async {
                    while let Some(msg) = stream.next().await {
                        if let Ok(msg) = msg {
                            match msg {
                                Message::Binary(msg) => {
                                    println!("client {} sent data", user_id);
                                    let req: shared::Req = rmp_serde::from_slice(&msg).unwrap();
                                    match req {
                                        shared::Req::Event(event) => {
                                            if sender.send(event).await.is_err() {
                                                break;
                                            }
                                        }
                                    }  
                                }
                                _ => {}
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
                                let msg = rmp_serde::to_vec(&shared::Res::Event(event)).unwrap();
                                if sink.send(Message::Binary(msg)).await.is_err() {
                                    break;
                                }
                            },
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                (game, _, receiver) = game_state.new_connection().await;
                                let msg = rmp_serde::to_vec(&shared::Res::Sync(game.clone())).unwrap();
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

pub async fn get_game() -> GameTemplate {
    GameTemplate::default()
}
