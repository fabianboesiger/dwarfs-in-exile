use seed::{prelude::*, *};
use shared::{Event, Game};
use std::rc::Rc;

const WS_URL: &str = "ws://127.0.0.1:3000/ws";

// ------ ------
//     Model
// ------ ------

pub struct Model {
    web_socket: WebSocket,
    web_socket_reconnector: Option<StreamHandle>,
    game: Game,
}

// ------ ------
//     Init
// ------ ------

fn init(_: Url, orders: &mut impl Orders<Msg>) -> Model {
    Model {
        web_socket: create_websocket(orders),
        web_socket_reconnector: None,
        game: Game::default()
    }
}

// ------ ------
//    Update
// ------ ------

pub enum Msg {
    WebSocketOpened,
    CloseWebSocket,
    WebSocketClosed(CloseEvent),
    WebSocketFailed,
    ReconnectWebSocket(usize),
    SendGameEvent(Event),
    ReceiveGameEvent(Event),
    GameState(Game),
}

fn update(msg: Msg, mut model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::WebSocketOpened => {
            model.web_socket_reconnector = None;
            log!("WebSocket connection is open now");
        }
        Msg::CloseWebSocket => {
            model.web_socket_reconnector = None;
            model
                .web_socket
                .close(None, Some("user clicked Close button"))
                .unwrap();
        }
        Msg::WebSocketClosed(close_event) => {
            log!("==================");
            log!("WebSocket connection was closed:");
            log!("Clean:", close_event.was_clean());
            log!("Code:", close_event.code());
            log!("Reason:", close_event.reason());
            log!("==================");

            // Chrome doesn't invoke `on_error` when the connection is lost.
            if !close_event.was_clean() && model.web_socket_reconnector.is_none() {
                model.web_socket_reconnector = Some(
                    orders.stream_with_handle(streams::backoff(None, Msg::ReconnectWebSocket)),
                );
            }
        }
        Msg::WebSocketFailed => {
            log!("WebSocket failed");
            if model.web_socket_reconnector.is_none() {
                model.web_socket_reconnector = Some(
                    orders.stream_with_handle(streams::backoff(None, Msg::ReconnectWebSocket)),
                );
            }
        }
        Msg::ReconnectWebSocket(retries) => {
            log!("Reconnect attempt:", retries);
            model.web_socket = create_websocket(orders);
        }
        Msg::SendGameEvent(event) => {
            println!("sent data {:?}", event);

            let serialized = rmp_serde::to_vec(&shared::Req::Event(event)).unwrap();
            model.web_socket.send_bytes(&serialized).unwrap();
        }
        Msg::ReceiveGameEvent(event) => {
            model.game.update(event);
        }
        Msg::GameState(game) => {
            model.game = game;
        }
    }
}

fn create_websocket(orders: &impl Orders<Msg>) -> WebSocket {
    let msg_sender = orders.msg_sender();

    WebSocket::builder(WS_URL, orders)
        .on_open(|| Msg::WebSocketOpened)
        .on_message(move |msg| decode_message(msg, msg_sender))
        .on_close(Msg::WebSocketClosed)
        .on_error(|| Msg::WebSocketFailed)
        .build_and_open()
        .unwrap()
}

fn decode_message(message: WebSocketMessage, msg_sender: Rc<dyn Fn(Option<Msg>)>) {
    if message.contains_text() {
        /*
        let msg = message
            .json::<shared::Event>()
            .expect("Failed to decode WebSocket text message");

        msg_sender(Some(Msg::ReceiveGameEvent(msg)));
        */
    } else {
        spawn_local(async move {
            let bytes = message
                .bytes()
                .await
                .expect("WebsocketError on binary data");

            let msg: shared::Res = rmp_serde::from_slice(&bytes).unwrap();
            match msg {
                shared::Res::Event(event) => {
                    msg_sender(Some(Msg::ReceiveGameEvent(event)));
                },
                shared::Res::Sync(game) => {
                    msg_sender(Some(Msg::GameState(game)));
                }
            }
        });
    }
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> Vec<Node<Msg>> {
    vec![
        h1!["WebSocket example"],
        button![
            ev(Ev::Click, {
                move |_| Msg::SendGameEvent(Event::Increment)
            }),
            "Send Game Event"
        ],
        p![model.game.cnt],
        hr![],
    ]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
