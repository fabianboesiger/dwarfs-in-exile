use seed::{prelude::*, *};
use shared::{
    Bundle, Craftable, Event, EventData, Item, ItemRarity, ItemType, LogMsg, Occupation, QuestType,
    Req, Res, RewardMode, Stats, SyncData, LOOT_CRATE_COST, Health,
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

#[derive(Debug, PartialEq, Eq)]
pub enum InventoryMode {
    Crafting,
    Stats,
}

#[derive(Default)]
pub struct InventoryFilter {
    item_rarity: Option<ItemRarity>,
    item_type: Option<ItemType>,
    item_name: Option<String>,
}

pub struct Model {
    web_socket: WebSocket,
    web_socket_reconnector: Option<StreamHandle>,
    state: Option<SyncData>,
    page: Page,
    message: String,
    chat_visible: bool,
    inventory_filter: InventoryFilter,
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
        inventory_filter: InventoryFilter::default(),
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
                Page::Inventory => inventory(model, data),
                Page::Ranking => ranking(data),
                Page::Quests => quests(data),
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
            .prestige
            .cmp(&p2.base.prestige)
            .then(p1.dwarfs.len().cmp(&p2.dwarfs.len()))
    });

    div![
        C!["content"],
        h2!["Ranking"],
        table![
            tr![
                th!["Rank"],
                th!["Username"],
                th!["Settlement"],
                th!["Population"]
            ],
            players.iter().enumerate().map(|(i, player)| {
                let rank = i + 1;
                tr![
                    td![rank],
                    td![&player.username],
                    td![format!("{}", player.base.village_type())],
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

fn select<F, I, T, N>(action: F, selected: Option<T>, options: I, names: N) -> Node<Msg>
where
    F: (Fn(Option<T>) -> Msg) + Copy + 'static,
    N: Fn(T) -> String,
    I: Iterator<Item = T>,
    T: Copy + Eq + 'static,
{
    details![
        summary![if let Some(t) = selected {
            format!("{}", names(t))
        } else {
            format!("None")
        }],
        div![
            if selected.is_some() {
                button![ev(Ev::Click, move |_| action(None)), format!("None"),]
            } else {
                Node::Empty
            },
            options.filter(|t| Some(*t) != selected).map(|t| {
                button![
                    ev(Ev::Click, move |_| action(Some(t))),
                    format!("{}", names(t)),
                ]
            })
        ]
    ]
}

fn health_bar(curr: Health, max: Health) -> Node<Msg> {
    div![C!["health-bar-wrapper"],
        div![C!["health-bar-max"]

        ],
        div![C!["health-bar-curr"]
        ]
    ]
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
                    span![if let Some(quest) = dwarf.participates_in_quest {
                        format!(
                            "Participating in quest {} since {}.",
                            quest,
                            fmt_time(dwarf.occupation_duration)
                        )
                    } else {
                        format!(
                            "{} since {}.",
                            dwarf.occupation,
                            fmt_time(dwarf.occupation_duration)
                        )
                    }]
                ],
                div![h4!["Stats"], stats(&dwarf.stats),],
                div![
                    h4!["Equipment"],
                    table![enum_iterator::all::<ItemType>().map(|item_type| {
                        let equipment = dwarf.equipment.get(&item_type).unwrap();

                        tr![
                            td![label![format!("{item_type}")]],
                            td![
                                select(
                                    move |item| Msg::SendGameEvent(Event::ChangeEquipment(
                                        id, item_type, item
                                    )),
                                    equipment.as_ref().copied(),
                                    player.inventory.by_type(Some(item_type)).into_iter(),
                                    |item| format!("{}", item)
                                ) /*details![
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
                                  ],*/
                            ],
                        ]
                    })]
                ],
                div![
                    C!["occupation"],
                    h4!["Work"],
                    Occupation::all().map(|occupation| {
                        button![
                            if occupation == dwarf.occupation
                                || dwarf.participates_in_quest.is_some()
                            {
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

fn quests(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![
        C!["quests"],
        state.quests.iter().enumerate().map(|(quest_idx, quest)| {

            div![
            C!["quest"],
            h3![format!("{}", quest.quest_type)],
            match quest.quest_type {
                QuestType::KillTheDragon => p!["A dragon was found high up in the mountains in the forbidden lands. Send your best warriors to defeat it."],
                QuestType::ArenaFight => p!["The King of the Dwarfs has invited the exilants to compete in an arena fight against monsters and creatures from the forbidden lands. The toughest warrior will be rewarded with a gift from the king personally."],
                QuestType::ExploreNewLands => p!["Send up to three dwarfs to explore new lands and find a place for a new settlement. The new settlement will be a better version of your previous settlement that allows a larger maximal population. Keep in mind that if this quest is sucessful, you will loose all of your dwarfs that you left back home."],
                QuestType::FeastForAGuest => p!["Your village is visted by an ominous guest. Go hunting and organize a feast for the guest, and he may stay."],
                QuestType::FreeTheVillage => p!["The Elven Village was raided by the Orks. Free the Elves to earn a reward!"],
                QuestType::SearchForNewDwarfs => p!["Search for a dwarf that got lost in the wilderness. If you find him first, he may stay in your settlement!"],
                QuestType::AFishingFriend => p!["Go fishing and make friends!"],
                QuestType::ADwarfInDanger => p!["Free a dwarf that gets robbed by Orks. If you free him first, he may stay in your settlement!"],
            },
            h4!["Rewards"],
            match quest.quest_type.reward_mode() {
                RewardMode::BestGetsAll(money) => div![p![format!("The best player gets ${money}, the rest gets nothing.")]],
                RewardMode::SplitFairly(money) => div![p![format!("A total of ${money} are split fairly between the players.")]],
                RewardMode::Prestige => div![p![format!("The participating players will have the chance to start over with a better settlement.")]],
                RewardMode::BestGetsItems(items) => div![
                    p![format!("The best player will get the following items:")],
                    p![bundle(&items)]
                ],
                RewardMode::NewDwarf(num) => div![p![format!("The best participant gets {num} new dwarf for their settlement.")]],
            },
            h4!["Participate"],
            p![format!("{} remaining.", fmt_time(quest.time_left))],
            p![format!("This quest requires {}.", quest.quest_type.occupation().to_string().to_lowercase())],
            p![format!("A total of {} people participate in this quest.", quest.contestants.len())],
            /*
            if let Some(_contestant) = quest.contestants.get(user_id) {
                p![format!("You participate in this quest!")]
            } else {
                Node::Empty
            },
            */
            
            (0..quest
                .quest_type
                .max_dwarfs())
                .map(|dwarf_idx| {
                    (dwarf_idx, quest
                        .contestants
                        .get(user_id)
                        .and_then(|contestant| contestant.dwarfs.get(&dwarf_idx).copied()))
                })
                .map(|(dwarf_idx, old_dwarf_id)| {
                    select(move |dwarf_id| Msg::SendGameEvent(
                            Event::AssignToQuest(quest_idx, dwarf_idx, dwarf_id)
                        ),
                        old_dwarf_id,
                        player.dwarfs.iter().filter(|(_, dwarf)| dwarf.participates_in_quest.is_none()).map(|(id, _)| *id),
                        |dwarf_id| player.dwarfs.get(&dwarf_id).unwrap().name.clone()
                    )
                })
        ]})
    ]
}

fn base(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![C!["content"],
        h2!["Your Settlement"],
        table![
            tr![th!["Settlement Type"], td![format!("{}", player.base.village_type())]],
            //tr![th!["Settlement Level"], td![format!("{} / {}", player.base.curr_level, player.base.max_level())]],
            tr![th!["Population"], td![format!("{}/{}", player.dwarfs.len(), player.base.num_dwarfs())]],
            tr![th!["Money"], td![format!("${}", player.money)]],
            tr![th!["Food"], td![format!("{}", player.base.food)]],
        ],
        if let Some(requires) = player.base.upgrade_cost() {
            div![
                h3!["Upgrade Settlement"],
                p!["Upgrade your settlement to increase the maximum population."],
                bundle(&requires),
                button![
                    if player.inventory.items.check_remove(&requires) {
                        attrs! {}
                    } else {
                        attrs! {At::Disabled => "true"}
                    },
                    ev(Ev::Click, move |_| Msg::SendGameEvent(Event::UpgradeBase)),
                    "Upgrade",
                ]
            ]
        } else {
            Node::Empty
        },
        div![
            h3!["Open Loot Crate"],
            p!["A loot crate contains a random rare or legendary item. You can earn loot crates by completing quests."],
            //p![format!("You own {} loot crates.", player.inventory.loot_crates)],
            if let Some(item) = player.inventory.got_from_loot_crate {
                p![format!("You received the item {} from your last opened loot crate.", item)]
            } else {
                Node::Empty
            },
            button![
                if player.money >= LOOT_CRATE_COST {
                    attrs! {}
                } else {
                    attrs! {At::Disabled => "true"}
                },
                ev(Ev::Click, move |_| Msg::SendGameEvent(Event::OpenLootCrate)),
                "Buy and Open ($100)",
            ]
        ],
        div![
            h3!["History"], 
            div![C![".history"],
            player
                .log
                .msgs
                .iter()
                .map(|(time, msg)| {
                    span![
                        span![format!("{} ago: ", fmt_time(state.time - time))],
                        match msg {
                            LogMsg::NewPlayer(user_id) => {
                                span![format!("A new player has joined the game, say hi to {}!", state.players.get(user_id).unwrap().username)]
                            },
                            LogMsg::NewDwarf(dwarf_id) => {
                                span![format!("Your settlement got a new dwarf {}.", player.dwarfs.get(dwarf_id).unwrap().name)]
                            },
                            LogMsg::DwarfDied(name) => {
                                span![format!("Your dwarf {} has died.", name)]
                            },
                            LogMsg::QuestCompleted(_dwarfs, quest) => {
                                span![format!("You completed the quest {}.", quest)]
                            },
                        }
                    ]
                })
            ]
        ],
    ]
}

fn inventory(model: &Model, SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    let items: Bundle<Item> = enum_iterator::all::<Item>()
        .map(|t| (t, 0))
        .chain(player.inventory.items.iter().map(|(item, n)| (*item, *n)))
        .collect();

    div![
        C!["items"],
        div![
            C!["inventory-filter"],
            
        ],
        items
            .sorted()
            .into_iter()
            .filter(|(item, _)| {
                (if let Some(item_rarity) = &model.inventory_filter.item_rarity {
                    item.item_rarity() == *item_rarity
                } else {
                    true
                }) &&
                (if let Some(item_type) = &model.inventory_filter.item_type {
                    item.item_type() == Some(*item_type)
                } else {
                    true
                }) &&
                (if let Some(item_name) = &model.inventory_filter.item_name {
                    item.to_string().contains(item_name)
                } else {
                    true
                })
            })
            .map(|(item, n)| div![
                C!["item"],
                match item.item_rarity() {
                    ItemRarity::Common => C!["item-common"],
                    ItemRarity::Uncommon => C!["item-uncommon"],
                    ItemRarity::Rare => C!["item-rare"],
                    ItemRarity::Epic => C!["item-epic"],
                    ItemRarity::Legendary => C!["item-legendary"],
                },
                div![
                    C!["item-contents"],
                    div![
                        span![if let Some(item_type) = item.item_type() {
                            format!("{item_type} | {}", item.item_rarity())
                        } else {
                            format!("Item | {}", item.item_rarity())
                        }],
                        h3![format!("{item}")],
                        span![format!("{n}x")],
                        //span![format!(" rarity: {}", item.item_rarity())],
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
                            ]
                        ]
                    } else {
                        Node::Empty
                    },
                    if let Some(food) = item.item_food() {
                        button![
                            if player
                                .inventory
                                .items
                                .check_remove(&Bundle::new().add(item, 1))
                            {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::SendGameEvent(Event::Craft(item))),
                            format!("Store as Food ({})", food),
                        ]
                    } else {
                        Node::Empty
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
            "Settlement",
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
