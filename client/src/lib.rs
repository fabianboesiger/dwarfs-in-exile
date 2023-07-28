use seed::{prelude::*, *};
use shared::{
    Building, Bundle, Craftable, Event, EventData, Item, ItemType, Occupation, Req, Res, Stats,
    SyncData,
};
use std::rc::Rc;

const WS_URL: &str = "ws://127.0.0.1:3000/game/ws";

// ------ ------
//     Model
// ------ ------

#[derive(Debug, PartialEq, Eq)]
pub enum Page {
    Dwarfs,
    Base,
    Inventory,
    Quests,
    Ranking,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct InventoryFilter {
    item_type: Option<ItemType>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InventoryMode {
    Crafting,
    Stats,
}

pub struct Model {
    web_socket: WebSocket,
    web_socket_reconnector: Option<StreamHandle>,
    state: Option<SyncData>,
    page: Page,
    message: String,
    chat_visible: bool,
}

// ------ ------
//     Init
// ------ ------

fn init(_url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders.subscribe(|subs::UrlRequested(_, url_request)| url_request.handled());

    Model {
        web_socket: create_websocket(orders),
        web_socket_reconnector: None,
        state: None,
        page: Page::Base,
        message: String::new(),
        chat_visible: false,
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
    ChangePage(Page),
    ChangeMessage(String),
    SubmitMessage,
    ToggleChat,
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
            log!(
                "WebSocket connection was closed, reason:",
                close_event.reason()
            );

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
            let serialized = rmp_serde::to_vec(&Req::Event(event)).unwrap();
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
        Msg::ChangePage(page) => {
            model.page = page;
        }
        Msg::ChangeMessage(message) => {
            model.message = message;
        }
        Msg::SubmitMessage => {
            let serialized =
                rmp_serde::to_vec(&Req::Event(Event::Message(model.message.clone()))).unwrap();
            model.web_socket.send_bytes(&serialized).unwrap();
            model.message.clear();
        }
        Msg::ToggleChat => {
            model.chat_visible = !model.chat_visible;
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

            let msg: Res = rmp_serde::from_slice(&bytes).unwrap();
            match msg {
                Res::Event(event) => {
                    msg_sender(Some(Msg::ReceiveGameEvent(event)));
                }
                Res::Sync(sync) => {
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
    if let Some(data) = &model.state {
        vec![
            nav(model),
            main![match model.page {
                Page::Dwarfs => dwarfs(data),
                Page::Base => base(data),
                Page::Inventory => inventory(data),
                Page::Ranking => ranking(data),
                _ => span!["Coming Soon!"],
            }],
            chat(model, data),
        ]
    } else {
        vec![p!["Loading ..."]]
    }
}

fn ranking(SyncData { state, user_id: _ }: &SyncData) -> Node<Msg> {
    let mut players: Vec<_> = state.players.values().collect();
    players.sort_by(|p1, p2| {
        p1.base
            .max
            .cmp(&p2.base.max)
            .then(p1.dwarfs.len().cmp(&p2.dwarfs.len()))
    });

    div![
        C!["content"],
        table![
            tr![
                th!["Rank"],
                th!["Username"],
                th!["Base Level"],
                th!["Population"]
            ],
            players.iter().enumerate().map(|(i, player)| {
                let rank = i + 1;
                tr![
                    td![rank],
                    td![&player.username],
                    td![player.base.max],
                    td![player.dwarfs.len()]
                ]
            })
        ]
    ]
}

fn fmt_time(mut time: u64) -> String {
    if time >= 60 {
        time /= 60;
        if time >= 60 {
            time /= 60;
            if time >= 24 {
                time /= 24;
                if time == 1 {
                    return format!("{} day", time);
                } else {
                    return format!("{} days", time);
                }
            }
            if time == 1 {
                return format!("{} hour", time);
            } else {
                return format!("{} hours", time);
            }
        }
        if time == 1 {
            return format!("{} minute", time);
        } else {
            return format!("{} minutes", time);
        }
    }
    if time == 1 {
        return format!("{} second", time);
    } else {
        return format!("{} seconds", time);
    }
}

fn dwarfs(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![
        C!["dwarfs"],
        player.dwarfs.iter().map(|(&id, dwarf)| div![
            C!["dwarf", format!("dwarf-{}", id)],
            div![
                C!["dwarf-contents"],
                div![
                    h3![&dwarf.name],
                    span![format!(
                        "{} since {}",
                        dwarf.occupation,
                        fmt_time(dwarf.occupation_duration)
                    )],
                ],
                div![h4!["Stats"], stats(&dwarf.stats),],
                div![
                    h4!["Equipment"],
                    table![enum_iterator::all::<ItemType>().map(|item_type| {
                        let equipment = dwarf.equipment.get(&item_type).unwrap();

                        tr![
                            td![label![format!("{item_type}")]],
                            td![details![
                                summary![if let Some(item) = equipment {
                                    format!("{}", item)
                                } else {
                                    format!("None")
                                }],
                                div![
                                    if equipment.is_some() {
                                        button![
                                            ev(Ev::Click, move |_| Msg::SendGameEvent(
                                                Event::ChangeEquipment(id, item_type, None)
                                            )),
                                            format!("None"),
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                    player
                                        .inventory
                                        .by_type(Some(item_type))
                                        .into_iter()
                                        .filter(|item| Some(*item) != *equipment)
                                        .map(|item| {
                                            button![
                                                ev(Ev::Click, move |_| Msg::SendGameEvent(
                                                    Event::ChangeEquipment(
                                                        id,
                                                        item_type,
                                                        Some(item)
                                                    )
                                                )),
                                                format!("{}", item),
                                            ]
                                        })
                                ]
                            ],],
                        ]
                    })]
                ],
                div![
                    C!["occupation"],
                    h4!["Work"],
                    Occupation::all().map(|occupation| {
                        button![
                            if occupation == dwarf.occupation {
                                attrs! {At::Disabled => "true"}
                            } else {
                                attrs! {}
                            },
                            ev(Ev::Click, move |_| Msg::SendGameEvent(
                                Event::ChangeOccupation(id, occupation)
                            )),
                            format!("{}", occupation),
                        ]
                    })
                ]
            ] /*
              button![
                  if let Occupation::None = dwarf.occupation {
                      attrs! {At::Disabled => "true"}
                  } else {
                      attrs! {}
                  },
                  ev(Ev::Click, move |_| Msg::SendGameEvent(
                      Event::ChangeOccupation(id, Occupation::None)
                  )),
                  "None",
              ],
              button![
                  if let Occupation::Mining = dwarf.occupation {
                      attrs! {At::Disabled => "true"}
                  } else {
                      attrs! {}
                  },
                  ev(Ev::Click, move |_| Msg::SendGameEvent(
                      Event::ChangeOccupation(id, Occupation::Mining)
                  )),
                  "Mining",
              ],
              */
        ])
    ]
}

fn base(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    let buildings: Bundle<Building> = Building::all()
        .iter()
        .chain(player.base.buildings.iter())
        .map(|(building, n)| (*building, *n))
        .collect();

    div![
        C!["buildings"],
        buildings.sorted().into_iter().map(|(building, n)| div![
            C!["building"],
            div![
                C!["building-contents"],
                div![h3![format!("{building}")], span![format!("Level {n}")],],
                div![
                    h4!["Build Options"],
                    if let Some(requires) = building.requires() {
                        div![
                            bundle(&requires),
                            button![
                                if player.inventory.items.check_remove(&requires) {
                                    attrs! {}
                                } else {
                                    attrs! {At::Disabled => "true"}
                                },
                                ev(Ev::Click, move |_| Msg::SendGameEvent(Event::Build(
                                    building
                                ))),
                                if n == 0 { "Build" } else { "Upgrade" }
                            ],
                        ]
                    } else {
                        div![]
                    }
                ]
            ]
        ])
    ]
}

fn inventory(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    let items: Bundle<Item> = Item::all()
        .iter()
        .chain(player.inventory.items.iter())
        .map(|(item, n)| (*item, *n))
        .collect();

    div![
        C!["items"],
        items.sorted().into_iter().map(|(item, n)| div![
            C!["item"],
            div![
                C!["item-contents"],
                div![
                    span![if let Some(item_type) = item.item_type() {
                        format!("{item_type}")
                    } else {
                        format!("Item")
                    }],
                    h3![format!("{item}")],
                    span![format!("{n}x")],
                ],
                /*
                match item.item_type() {
                    ItemType::Clothing(s) => {
                        div![stats(&s)]
                    }
                    ItemType::Tool(s) => {
                        div![stats(&s)]
                    }
                    ItemType::Misc => {
                        div![]
                    }
                },
                */
                if let Some(requires) = item.requires() {
                    div![
                        bundle(&requires),
                        button![
                            if player.inventory.items.check_remove(&requires) {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::SendGameEvent(Event::Craft(item))),
                            "Craft",
                        ],
                    ]
                } else {
                    div![]
                },
            ]
        ])
    ]
}

fn chat(model: &Model, SyncData { state, user_id: _ }: &SyncData) -> Node<Msg> {
    let message = model.message.clone();

    div![
        id!["chat"],
        button![ev(Ev::Click, move |_| Msg::ToggleChat), "Toggle Chat",],
        if model.chat_visible {
            div![
                div![
                    C!["messages"],
                    state.chat.messages.iter().map(|(user_id, message)| {
                        let username = &state.players.get(user_id).unwrap().username;

                        div![
                            span![C!["username"], format!("{username}")],
                            span![": "],
                            span![C!["message"], format!("{message}")]
                        ]
                    })
                ],
                div![
                    input![
                        attrs! {At::Type => "text", At::Value => model.message, At::Placeholder => "Type your message here ..."},
                        input_ev(Ev::Input, Msg::ChangeMessage)
                    ],
                    button![
                        if message.is_empty() {
                            attrs! {At::Disabled => "true"}
                        } else {
                            attrs! {}
                        },
                        ev(Ev::Click, move |_| Msg::SubmitMessage),
                        "Send",
                    ],
                ]
            ]
        } else {
            div![]
        }
    ]
}

fn bundle(requires: &Bundle<Item>) -> Node<Msg> {
    ul![requires
        .clone()
        .sorted()
        .iter()
        .map(|(item, n)| { li![format!("{n}x {item}")] })]
}

fn stats(stats: &Stats) -> Node<Msg> {
    table![tbody![
        if stats.strength != 0 {
            tr![th!["Strength"], td![format!("{:+}", stats.strength)]]
        } else {
            tr![]
        },
        if stats.endurance != 0 {
            tr![th!["Endurance"], td![format!("{:+}", stats.endurance)]]
        } else {
            tr![]
        },
        if stats.agility != 0 {
            tr![th!["Agility"], td![format!("{:+}", stats.agility)]]
        } else {
            tr![]
        },
        if stats.intelligence != 0 {
            tr![
                th!["Intelligence"],
                td![format!("{:+}", stats.intelligence)]
            ]
        } else {
            tr![]
        },
        if stats.charisma != 0 {
            tr![th!["Charisma"], td![format!("{:+}", stats.charisma)]]
        } else {
            tr![]
        },
    ]]
}

fn nav(model: &Model) -> Node<Msg> {
    nav![div![
        button![
            if let Page::Base = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Base)),
            "Base",
        ],
        button![
            if let Page::Dwarfs = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs)),
            "Dwarfs",
        ],
        button![
            if let Page::Inventory = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Inventory)),
            "Inventory",
        ],
        button![
            if let Page::Quests = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Quests)),
            "Quests",
        ],
        button![
            if let Page::Ranking = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Ranking)),
            "Ranking",
        ],
        //a![C!["button"], attrs! { At::Href => "/account"}, "Account"]
    ]]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
