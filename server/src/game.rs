use askama::Template;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        TypedHeader
    },
    response::IntoResponse,
    Extension
};
use tokio::sync::{broadcast, mpsc, RwLock};
use futures_util::{stream::StreamExt, sink::SinkExt};
use axum_sessions::{
    async_session::Session,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct GameState(Arc<GameStateImpl>);

struct GameStateImpl {
    game: RwLock<shared::Game>,
    res_sender: broadcast::Sender<shared::Event>,
    req_sender: mpsc::Sender<shared::Event>,
}

impl GameState {
    async fn load_game() -> Option<shared::Game> {
        None
    }

    pub async fn new() -> GameState {
        let (req_sender, mut req_receiver) = mpsc::channel::<shared::Event>(128);
        let (res_sender, res_receiver) = broadcast::channel::<shared::Event>(128);
        
        let game = RwLock::new(GameState::load_game().await.unwrap_or_default());
        let game_state = Arc::new(GameStateImpl { game, res_sender, req_sender });
        let game_state_clone = game_state.clone();

        tokio::spawn(async move {
            let GameStateImpl { game, res_sender, req_sender } = &*game_state_clone;

            while let Some(event) = req_receiver.recv().await {
                res_sender.send(event.clone()).ok();
                game.write().await.update(event);
            }
        });

        GameState(game_state)
    }

    pub async fn new_connection(&self) -> (shared::Game, mpsc::Sender<shared::Event>, broadcast::Receiver<shared::Event>) {
        (self.0.game.read().await.clone(), self.0.req_sender.clone(), self.0.res_sender.subscribe())
    }
}


pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(session): Extension<Session>,
    Extension(game_state): Extension<GameState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket: WebSocket| async move {
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
                                println!("client sent binary data");
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

    })
}

#[derive(Template, Default)]
#[template(path = "game.html")]
pub struct GameTemplate {}

pub async fn get_game() -> GameTemplate {
    GameTemplate::default()
}