use seed::{prelude::*, *};
use shared::{Event, EventData, SyncData};
use std::rc::Rc;

const WS_URL: &str = "ws://127.0.0.1:3000/game/ws";

// ------ ------
//     Model
// ------ ------

pub struct Model {
    web_socket: WebSocket,
    web_socket_reconnector: Option<StreamHandle>,
    state: Option<SyncData>,
}

// ------ ------
//     Init
// ------ ------

fn init(_: Url, orders: &mut impl Orders<Msg>) -> Model {
    Model {
        web_socket: create_websocket(orders),
        web_socket_reconnector: None,
        state: None,
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
    ReceiveGameEvent(EventData),
    InitGameState(SyncData),
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
                .close(None, Some("user clicked close button"))
                .unwrap();
        }
        Msg::WebSocketClosed(close_event) => {
            log!("WebSocket connection was closed, reason:", close_event.reason());

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
            let serialized = rmp_serde::to_vec(&shared::Req::Event(event)).unwrap();
            model.web_socket.send_bytes(&serialized).unwrap();
        }
        Msg::ReceiveGameEvent(event) => {
            if let Some(SyncData { state, .. }) = &mut model.state {
                state.update(event);
            }
        }
        Msg::InitGameState(sync_data) => {
            model.state = Some(sync_data);
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
        unreachable!()
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
                }
                shared::Res::Sync(sync) => {
                    msg_sender(Some(Msg::InitGameState(sync)));
                }
            }
        });
    }
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> Vec<Node<Msg>> {
    if let Some(SyncData { user_id, state }) = &model.state {
        vec![
            h1!["WebSocket example"],
            button![
                ev(Ev::Click, move |_| Msg::SendGameEvent(Event::Increment)),
                "Increment Counter"
            ],
            p![state.cnt],
            button![
                ev(Ev::Click, move |_| Msg::SendGameEvent(
                    Event::IncrementPrivate
                )),
                "Increment Private Counter"
            ],
            p![state.cnt_private.get(user_id)],
        ]
    } else {
        vec![p!["Loading ..."]]
    }
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
