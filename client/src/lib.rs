use seed::{prelude::*, *};
use shared::{
    Bundle, Craftable, DwarfId, Event, EventData, Health, Item, ItemRarity, ItemType, LogMsg,
    Occupation, QuestType, Req, Res, RewardMode, Stats, SyncData, LOOT_CRATE_COST, MAX_HEALTH,
    SPEED, Player,
};
use std::{rc::Rc, str::FromStr};
use itertools::Itertools;

#[cfg(not(debug_assertions))]
const WS_URL: &str = "ws://boesiger.internet-box.ch/game/ws";
#[cfg(debug_assertions)]
const WS_URL: &str = "ws://127.0.0.1:3000/game/ws";

// ------ ------
//     Model
// ------ ------

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Page {
    Dwarfs(DwarfsMode),
    Base,
    Inventory(InventoryMode),
    Quests,
    Ranking,
}

impl Page {
    fn from_url(url: Url) -> Self {
        match url.path().get(1).map(|s| s.as_str()) {
            Some("dwarfs") => Page::Dwarfs(DwarfsMode::Overview),
            Some("inventory") => Page::Inventory(match url.hash().map(|s| s.as_str()) {
                Some("stats") => InventoryMode::Stats,
                _ => InventoryMode::Crafting
            }),
            Some("quests") => Page::Quests,
            Some("ranking") => Page::Ranking,
            _ => Page::Base,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InventoryMode {
    Crafting,
    Stats,
    Select(InventorySelect),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InventorySelect {
    Equipment(DwarfId, ItemType),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DwarfsMode {
    Overview,
    Select(DwarfsSelect),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DwarfsSelect {
    Quest(usize, usize),
}

#[derive(Default)]
pub struct InventoryFilter {
    item_name: String,
    craftable: bool,
    food: bool,
    owned: bool,
    pets: bool,
    clothing: bool,
    tools: bool,
}

pub struct Model {
    web_socket: WebSocket,
    web_socket_reconnector: Option<StreamHandle>,
    state: Option<SyncData>,
    page: Page,
    message: String,
    chat_visible: bool,
    history_visible: bool,
    inventory_filter: InventoryFilter,
}

// ------ ------
//     Init
// ------ ------

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    //orders.subscribe(|subs::UrlRequested(_, url_request)| url_request.handled());
    orders.subscribe(|subs::UrlRequested(url, url_request)| {
        if url.path().get(0).map(|s| s.as_str()) == Some("game") {
            url_request.unhandled()
        } else {
            url_request.handled()
        }
        //url_request.handled()
    });
    orders.subscribe(|subs::UrlChanged(url)| {
        Msg::ChangePage(Page::from_url(url))
    });

    Model {
        web_socket: create_websocket(orders),
        web_socket_reconnector: None,
        state: None,
        page: Page::from_url(url),
        message: String::new(),
        chat_visible: false,
        history_visible: false,
        inventory_filter: InventoryFilter::default(),
    }
}

// ------ ------
//    Update
// ------ ------

#[derive(Debug)]
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
    ToggleHistory,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    AssignToQuest(usize, usize, Option<DwarfId>),
    InventoryFilterFood,
    InventoryFilterOwned,
    InventoryFilterCraftable,
    InventoryFilterTools,
    InventoryFilterPets,
    InventoryFilterClothing,
    InventoryFilterName(String),
    InventoryFilterReset,
    GoToItem(Item),
    InventoryToggleStats,
}

fn update(msg: Msg, mut model: &mut Model, orders: &mut impl Orders<Msg>) {
    let web_socket = &model.web_socket;
    let send = |event| {
        let serialized = rmp_serde::to_vec(&Req::Event(event)).unwrap();
        web_socket.send_bytes(&serialized).unwrap();
    };

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
        Msg::SendGameEvent(event) => send(event),
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
            send(Event::Message(model.message.clone()));
            model.message.clear();
        }
        Msg::ToggleChat => {
            model.chat_visible = !model.chat_visible;
            //model.history_visible = !model.history_visible;
        }
        Msg::ToggleHistory => {
            model.history_visible = !model.history_visible;
            //model.chat_visible = !model.history_visible;
        }
        Msg::InventoryFilterFood => {
            model.inventory_filter.food = !model.inventory_filter.food;
        }
        Msg::InventoryFilterOwned => {
            model.inventory_filter.owned = !model.inventory_filter.owned;
        }
        Msg::InventoryFilterCraftable => {
            model.inventory_filter.craftable = !model.inventory_filter.craftable;
        }
        Msg::InventoryFilterPets => {
            model.inventory_filter.pets = !model.inventory_filter.pets;
        }
        Msg::InventoryFilterTools => {
            model.inventory_filter.tools = !model.inventory_filter.tools;
        }
        Msg::InventoryFilterClothing => {
            model.inventory_filter.clothing = !model.inventory_filter.clothing;
        }
        Msg::InventoryFilterName(item_name) => {
            model.inventory_filter.item_name = item_name;
        }
        Msg::InventoryFilterReset => {
            model.inventory_filter = InventoryFilter::default();
        }
        Msg::AssignToQuest(quest_idx, dwarf_idx, dwarf_id) => {
            if dwarf_id.is_some() {
                model.page = Page::Quests;
            }
            send(Event::AssignToQuest(quest_idx, dwarf_idx, dwarf_id));
        }
        Msg::ChangeEquipment(dwarf_id, item_type, item) => {
            if item.is_some() {
                model.page = Page::Dwarfs(DwarfsMode::Overview);
            }
            send(Event::ChangeEquipment(dwarf_id, item_type, item))
        }
        Msg::GoToItem(item) => {
            model.inventory_filter = InventoryFilter::default();
            model.inventory_filter.item_name = item.to_string();
            //model.page = Page::Inventory(InventoryMode::Crafting);
            orders.notify(subs::UrlRequested::new(Url::from_str("/game/inventory").unwrap())); 
        }
        Msg::InventoryToggleStats => {
            match model.page {
                Page::Inventory(InventoryMode::Stats) => {
                    //model.page = Page::Inventory(InventoryMode::Crafting);
                    orders.notify(subs::UrlRequested::new(Url::from_str("/game/inventory#crafting").unwrap())); 
                }, 
                Page::Inventory(InventoryMode::Crafting) => {
                    //model.page = Page::Inventory(InventoryMode::Stats);
                    orders.notify(subs::UrlRequested::new(Url::from_str("/game/inventory#stats").unwrap())); 

                }
                _ => {}
            }
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
            div![id!["background"]],
            header![
                h1![a![attrs!{ At::Href => "/" }, "Dwarfs in Exile"]]
            ],
            nav(model),
            #[cfg(debug_assertions)]
            div![
                id!["server-info"],
                span![format!(
                    "status: {}, tick number: {}, tick rate: 1/{} s",
                    if model.web_socket_reconnector.is_none() {
                        "connected"
                    } else {
                        "disconnected"
                    },
                    data.state.time,
                    shared::SPEED,
                )]
            ],
            main![match model.page {
                Page::Dwarfs(mode) => dwarfs(data, mode),
                Page::Base => base(data),
                Page::Inventory(mode) => inventory(model, data, mode),
                Page::Ranking => ranking(data),
                Page::Quests => quests(data),
            }],
            chat(model, data),
            history(model, data),
        ]
    } else {
        vec![
            div![id!["background"]],
            header![
                h1![a![attrs!{ At::Href => "/" }, "Dwarfs in Exile"]]
            ],
            div![C!["loading"],
                "Loading ..."
            ]
        ]
    }
}

fn ranking(SyncData { state, user_id: _ }: &SyncData) -> Node<Msg> {
    let mut players: Vec<_> = state.players.values().collect();
    players.sort_by_key(|p| (-(p.base.prestige as i64), -(p.dwarfs.len() as i64)));

    div![
        C!["content"],
        h2!["Ranking"],
        if let Some(king) = state.king {
            let king = state.players.get(&king).unwrap();
            p![format!("All hail our King {}!", king.username), tip("The king gets one tenth of all money that was earned. Make sure you become the king as soon as the quest becomes available.")]
        } else {
            Node::Empty
        },
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
                    td![format!("{} ", player.username), span![C!["symbols", if player.is_online(state.time) { "online" } else { "offline" }], "●"]],
                    td![format!("{}", player.base.village_type())],
                    td![player.dwarfs.len()]
                ]
            })
        ]
    ]
}

fn fmt_time(mut time: u64) -> String {
    time = time / SPEED;
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

fn health_bar(curr: Health, max: Health) -> Node<Msg> {
    div![
        C!["health-bar-wrapper"],
        div![
            C!["health-bar-curr"],
            attrs! {
                At::Style => format!("width: calc(100% / {max} * {curr});")
            }
        ],
        div![
            C!["health-bar-overlay"],
            format!("{}%", (100 * curr / max + 1).min(100))
        ],
    ]
}

fn dwarfs(SyncData { state, user_id }: &SyncData, mode: DwarfsMode) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    if player.dwarfs.len() > 0 {
        div![
            C!["dwarfs"],
            player.dwarfs.iter().map(|(&id, dwarf)| div![
                C!["dwarf", format!("dwarf-{}", id)],
                div![
                    C!["dwarf-contents"],
                    div![
                        h3![&dwarf.name],
                        span![if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
                            format!(
                                "Participating in quest {} since {}.",
                                quest_type,
                                fmt_time(dwarf.occupation_duration)
                            )
                        } else {
                            format!(
                                "{} since {}.",
                                dwarf.occupation,
                                fmt_time(dwarf.occupation_duration)
                            )
                        }],
                        health_bar(dwarf.health, MAX_HEALTH),
                        tip("This shows your dwarfs health. If it reaches zero, your dwarf dies. To restore a dwarfs health, let him idle and make sure that there is food in your settlement.")
                    ],
                    div![
                        h4!["Stats"],
                        table![tbody![
                            tr![th![], th!["Inherent", tip("Each dwarf has some inherent stats that he was born with and that cannot be changed.")], th!["Effective", tip("The effective stats include the effects of the dwarfs equipment.")]],
                            tr![th!["Strength"],
                                td![stars(dwarf.stats.strength, true)],
                                td![stars(dwarf.effective_stats().strength, true)],
                            ],
                            tr![th!["Endurance"],
                                td![stars(dwarf.stats.endurance, true)],
                                td![stars(dwarf.effective_stats().endurance, true)],
                            ],
                            tr![th!["Agility"],
                                td![stars(dwarf.stats.agility, true)],
                                td![stars(dwarf.effective_stats().agility, true)],
                            ],
                            tr![th!["Intelligence"],
                                td![stars(dwarf.stats.intelligence, true)],
                                td![stars(dwarf.effective_stats().intelligence, true)],
                            ],
                            tr![th!["Perception"],
                                td![stars(dwarf.stats.perception, true)],
                                td![stars(dwarf.effective_stats().perception, true)],
                            ],
                        ]]
                    ],
                    div![
                        h4!["Equipment"],
                        table![
                            tr![th![], th!["Equipped"], th![format!("Effectiveness for {}", dwarf.occupation), tip("This shows how effective the current tool is for the current job of this dwarf, considering the dwarfs stats.")], th![]],
                            enum_iterator::all::<ItemType>().map(|item_type| {
                            let equipment = dwarf.equipment.get(&item_type).unwrap();
                            tr![
                                th![label![format!("{item_type}")]],
                                td![equipment
                                    .map(|item| format!("{}", item))
                                    .unwrap_or(String::from("None")),],
                                td![equipment
                                    .map(|item| stars(dwarf.equipment_usefulness(dwarf.occupation, item) as i8, true))
                                    .unwrap_or(Node::Empty),
                                ],
                                td![
                                    button![
                                        ev(Ev::Click, move |_| Msg::ChangePage(Page::Inventory(
                                            InventoryMode::Select(InventorySelect::Equipment(
                                                id, item_type
                                            ))
                                        ))),
                                        "Change"
                                    ],
                                    if equipment.is_some() {
                                        button![
                                            ev(Ev::Click, move |_| Msg::ChangeEquipment(
                                                id, item_type, None
                                            )),
                                            "Unequip"
                                        ]
                                    } else {
                                        Node::Empty
                                    }
                                ],
                            ]
                        })]
                    ],
                    match mode {
                        DwarfsMode::Overview => {
                            div![
                                C!["occupation"],
                                h4!["Work"],
                                if let Some((quest_type, quest_idx, dwarf_idx)) = dwarf.participates_in_quest {
                                    div![
                                        div![button![
                                            ev(Ev::Click, move |_| Msg::AssignToQuest(
                                                quest_idx,
                                                dwarf_idx,
                                                None
                                            )),
                                            format!("Remove from Quest {}", quest_type)
                                        ]]
                                    ]
                                } else {
                                    div![
                                        enum_iterator::all::<Occupation>().filter(|occupation| player.base.curr_level >= occupation.unlocked_at_level()).map(|occupation| {
                                            let all_items = enum_iterator::all::<Item>().filter_map(|item| item.item_probability(occupation).map(|_| item)).collect::<Vec<_>>();

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
                                                if all_items.len() == 0 {
                                                    tip("From this occupation, you can get nothing.")
                                                } else {
                                                    tip(format!("From this occupation, you can get the items {}.", all_items.into_iter().join(", ")))
                                                },
                                                br![],
                                                stars(dwarf.effectiveness(occupation) as i8, true)
                                            ]
                                        })
                                    ]
                                }
                                
                            ]
                        }
                        DwarfsMode::Select(DwarfsSelect::Quest(quest_idx, dwarf_idx)) => {
                            div![button![
                                ev(Ev::Click, move |_| Msg::AssignToQuest(
                                    quest_idx,
                                    dwarf_idx,
                                    Some(id)
                                )),
                                format!("Assign to Quest {}", state.quests.get(quest_idx).unwrap().quest_type),
                                br![],
                                stars(dwarf.effectiveness(state.quests.get(quest_idx).unwrap().quest_type.occupation()) as i8, true)
                            ]]
                        }
                    }
                ]
            ])
        ]
    } else {
        div![
            C!["content"],
            h2!["There's Noone Here!"],
            p!["All your dwarfs have died! You can wait until a new dwarf finds your settlement or start over with a new settlement."],
            button![
                ev(Ev::Click, move |_| Msg::SendGameEvent(Event::Restart)),
                "Restart your Settlement",
            ],
        ]
    }
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
                QuestType::ForTheKing => p!["Fight a ruthless battle to become the king over all of Exile Island!"],
                QuestType::DrunkFishing => p!["Participate in the drunk fishing contest! The dwarf that is the most successful drunk fisher gets a reward."],
                QuestType::CollapsedCave => p!["A cave has collapsed and a dwarf is trapped inside. Be the first to save is life and he will move into your settlement."]

            },
            p![format!("{} remaining.", fmt_time(quest.time_left))],
            p![format!("This quest requires {}.", quest.quest_type.occupation().to_string().to_lowercase())],
            h4!["Rewards"],
            match quest.quest_type.reward_mode() {
                RewardMode::BestGetsAll(money) => div![p![format!("The best player gets 🜚{money}, the rest gets nothing.")]],
                RewardMode::SplitFairly(money) => div![p![format!("A total of 🜚{money} are split fairly between the players.")]],
                RewardMode::Prestige => div![
                    p![format!("The participating players will have the chance to start over with a better settlement. For this quest to be successful, your settlement needs to be fully upgraded.")],
                    if player.can_prestige() {
                        p![format!("Your settlement is fully upgraded!")]
                    } else {
                        p![format!("Your settelemnt is not fully upgraded!")]
                    }
                ],
                RewardMode::BecomeKing => div![
                    p![format!("The best player will become the king and get one tenth of all money that is earned during his reign.")],
                ],
                RewardMode::BestGetsItems(items) => div![
                    p![format!("The best player will get the following items:")],
                    p![bundle(&items, player)]
                ],
                RewardMode::NewDwarf(num) => div![p![format!("The best participant gets {num} new dwarf for their settlement.")]],
            },
            h4!["Participate"],
            p![format!("A total of {} people participate in this quest.", quest.contestants.len())],
            
            if let Some(contestant) = quest.contestants.get(user_id) {
                let rank = quest.contestants.values().filter(|c| c.achieved_score >= contestant.achieved_score).count();
                p![format!("You have a score of {} so far in this quest and with this you are on rank {}.", big_number(contestant.achieved_score), rank)]
            } else {
                Node::Empty
            },

            table![
            (0..quest
                .quest_type
                .max_dwarfs())
                .map(|dwarf_idx| {
                    (dwarf_idx, quest
                        .contestants
                        .get(user_id)
                        .and_then(|contestant| contestant.dwarfs.get(&dwarf_idx).copied()))
                })
                .map(|(dwarf_idx, dwarf_id)| {
                    tr![
                        td![
                            dwarf_id.map(|dwarf_id| player.dwarfs.get(&dwarf_id).unwrap().name.clone()).unwrap_or(String::from("None"))
                        ],
                        td![
                            button![
                                ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(DwarfsMode::Select(DwarfsSelect::Quest(quest_idx, dwarf_idx))))),
                                "Change"
                            ],
                            if dwarf_id.is_some() {
                                button![
                                    ev(Ev::Click, move |_| Msg::AssignToQuest(quest_idx, dwarf_idx, None)),
                                    "Remove"
                                ]
                            } else {
                                Node::Empty
                            }
                        ]
                    ]
                })
            ]
        ]})
    ]
}

fn big_number(mut num: u64) -> String {
    let mut ending = String::new();
    if num >= 1000 {
        ending = String::from("K");
        num /= 1000;
    }
    if num >= 1000 {
        ending = String::from("M");
        num /= 1000;
    }
    format!("{}{}", num, ending)
}

fn base(SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![C!["content"],
        h2!["Your Settlement"],
        table![
            tr![th![
                "Settlement Type",
                tip("There are ten different types of settlements that get gradually better: Outpost, Dwelling, Hamlet, Village, Small Town, Large Town, Small City, Large City, Metropolis, Megalopolis. To move on to a better settlement type, you need to complete a special quest.")],
                td![format!("{}", player.base.village_type())]
            ],
            //tr![th!["Settlement Level"], td![format!("{} / {}", player.base.curr_level, player.base.max_level())]],
            tr![th!["Population", tip("Upgrade your settlement to increase the maximum population. You can get new dwarfs from certain quests or at random.")], td![format!("{}/{}", player.dwarfs.len(), player.base.num_dwarfs())]],
            tr![th!["Money", tip("Earn money by doing quests. With money, you can buy loot crates.")], td![format!("🜚{}", player.money)]],
            tr![th!["Food", tip("Your settlement can store food for your dwarfs to consume. One quantity of food restores 0.1% of a dwarfs health.")], td![format!("{}", player.base.food)]],
        ],
        if let Some(requires) = player.base.upgrade_cost() {
            div![
                h3!["Upgrade Settlement"],
                p!["Upgrade your settlement to increase the maximum population and unlock new occupations for your dwarfs."],
                if let Some(unlocked_occupation) = enum_iterator::all::<Occupation>().filter(|occupation| occupation.unlocked_at_level() == player.base.curr_level + 1).next() {
                    p![format!("The next upgrade increases your maximal population by two and unlocks the occupation {}.", unlocked_occupation)]
                } else {
                    p!["The next upgrade increases your maximal population by two."]
                },
                bundle(&requires, player),
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
            button![
                if player.money >= LOOT_CRATE_COST {
                    attrs! {}
                } else {
                    attrs! {At::Disabled => "true"}
                },
                ev(Ev::Click, move |_| Msg::SendGameEvent(Event::OpenLootCrate)),
                format!("Buy and Open (🜚{})", LOOT_CRATE_COST),
            ]
        ]
    ]
}

fn inventory(
    model: &Model,
    SyncData { state, user_id }: &SyncData,
    mode: InventoryMode,
) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    let items: Bundle<Item> = enum_iterator::all::<Item>()
        .map(|t| (t, 0))
        .chain(player.inventory.items.iter().map(|(item, n)| (*item, *n)))
        .collect();

    div![
        div![
            C!["inventory-filter"],
            div![
                div![
                    input![
                        id!["owned"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.owned.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterOwned),
                    ],
                    label![attrs! {At::For => "owned"}, "Owned"]
                ],
                div![
                    input![
                        id!["craftable"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.craftable.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterCraftable),
                    ],
                    label![attrs! {At::For => "craftable"}, "Craftable"]
                ]
            ],
            div![
                div![
                    input![
                        id!["food"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.food.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterFood),
                    ],
                    label![attrs! {At::For => "food"}, "Food"]
                ],
                div![
                    input![
                        id!["tools"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.tools.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterTools),
                    ],
                    label![attrs! {At::For => "tools"}, "Tools"]
                ],
                div![
                    input![
                        id!["clothing"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.clothing.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterClothing),
                    ],
                    label![attrs! {At::For => "clothing"}, "Clothing"]
                ],
                div![
                    input![
                        id!["pets"],
                        attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.pets.as_at_value()},
                        ev(Ev::Click, |_| Msg::InventoryFilterPets),
                    ],
                    label![attrs! {At::For => "pets"}, "Pets"]
                ],
            ],
            div![
                input![
                    id!["tools"],
                    attrs! {At::Type => "checkbox", At::Checked => matches!(model.page, Page::Inventory(InventoryMode::Stats)).as_at_value()},
                    ev(Ev::Click, |_| Msg::InventoryToggleStats),
                ],
                label![attrs! {At::For => "tools"}, "Show Stats"],
                input![
                    attrs! {At::Type => "text", At::Value => model.inventory_filter.item_name, At::Placeholder => "Item Name"},
                    input_ev(Ev::Input, Msg::InventoryFilterName)
                ],
                button![
                    ev(Ev::Click, move |_| Msg::InventoryFilterReset),
                    "Reset Filter",
                ],
            ]
        ],
        div![
            C!["items"],
            items
                .sorted_by_rarity()
                .into_iter()
                .filter(|(item, n)| {
                    item.to_string()
                        .to_lowercase()
                        .contains(&model.inventory_filter.item_name.to_lowercase().trim())
                        && ((if model.inventory_filter.owned {
                            *n > 0
                        } else {
                            false
                        }) || (if model.inventory_filter.craftable {
                            if let Some(requires) = item.requires() {
                                player.inventory.items.check_remove(&requires)
                            } else {
                                false
                            }
                        } else {
                            false
                        }) || (!model.inventory_filter.owned
                            && !model.inventory_filter.craftable))
                        && ((if model.inventory_filter.food {
                            item.nutritional_value().is_some()
                        } else {
                            false
                        }) || (if model.inventory_filter.tools {
                            item.item_type() == Some(ItemType::Tool)
                        } else {
                            false
                        }) || (if model.inventory_filter.clothing {
                            item.item_type() == Some(ItemType::Clothing)
                        } else {
                            false
                        }) || (if model.inventory_filter.pets {
                            item.item_type() == Some(ItemType::Pet)
                        } else {
                            false
                        }) || (!model.inventory_filter.food
                            && !model.inventory_filter.tools
                            && !model.inventory_filter.clothing
                            && !model.inventory_filter.pets))
                        && if let InventoryMode::Select(InventorySelect::Equipment(_, item_type)) =
                            mode
                        {
                            item.item_type() == Some(item_type) && *n > 0
                        } else {
                            true
                        }
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
                            h3![format!("{n}x {item}")],
                        ],
                        match mode {
                            InventoryMode::Crafting => {
                                vec![div![
                                    if let Some(requires) = item.requires() {
                                        div![
                                            bundle(&requires, player),
                                            button![
                                                if player.inventory.items.check_remove(&requires) {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::SendGameEvent(
                                                    Event::Craft(item)
                                                )),
                                                "Craft",
                                            ]
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                    if let Some(food) = item.nutritional_value() {
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
                                            ev(Ev::Click, move |_| Msg::SendGameEvent(
                                                Event::AddToFoodStorage(item)
                                            )),
                                            format!("Store as {} Food", food),
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                ]]
                            }
                            mode => {
                                vec![
                                    if !item.provides_stats().is_zero() {
                                        div![
                                            h4!["Provides"],
                                            stats(&item.provides_stats()),
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                    if !item.requires_stats().is_zero() {
                                        div![
                                            h4!["Requires"],
                                            stats(&item.requires_stats()),
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                    if enum_iterator::all::<Occupation>()
                                        .filter(|occupation| {
                                            let usefulness = item.usefulness_for(*occupation);
                                            usefulness > 0
                                        })
                                        .count()
                                        > 0
                                    {
                                        div![
                                            h4!["Utility"],
                                            enum_iterator::all::<Occupation>().filter_map(
                                                |occupation| {
                                                    let usefulness =
                                                        item.usefulness_for(occupation) as i8;
                                                    if usefulness > 0 {
                                                        Some(span![
                                                            format!("{} ", occupation),
                                                            stars(usefulness, true)
                                                        ])
                                                    } else {
                                                        None
                                                    }
                                                }
                                            ).intersperse(br![])
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                    if let InventoryMode::Select(InventorySelect::Equipment(
                                        dwarf_id,
                                        item_type,
                                    )) = mode {
                                        button![
                                            ev(Ev::Click, move |_| Msg::ChangeEquipment(
                                                dwarf_id,
                                                item_type,
                                                Some(item)
                                            )),
                                            "Equip"
                                        ]
                                    } else {
                                        Node::Empty
                                    }
                                ]
                            }
                        }
                    ]
                ]),
            div![C!["item", "hidden"]],
            div![C!["item", "hidden"]],
            div![C!["item", "hidden"]],
            div![C!["item", "hidden"]],
            div![C!["item", "hidden"]],
            div![C!["item", "hidden"]]
        ]
    ]
}

fn chat(model: &Model, SyncData { state, user_id: _ }: &SyncData) -> Node<Msg> {
    let message = model.message.clone();

    div![
        id!["chat"],
        if model.chat_visible {
            C!["visible"]
        } else {
            C![]
        },
        if model.chat_visible {
            div![
                C!["togglable"],
                div![
                    C!["messages"],
                    state.chat.messages.iter().map(|(user_id, message)| {
                        let username = &state.players.get(user_id).unwrap().username;
                        div![
                            C!["message"],
                            span![C!["username"], format!("{username}")],
                            span![": "],
                            span![C!["message"], format!("{message}")]
                        ]
                    }),
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
            Node::Empty
        },
        button![ev(Ev::Click, move |_| Msg::ToggleChat), "Toggle Chat",],
    ]
}

fn history(model: &Model, SyncData { state, user_id }: &SyncData) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![
        id!["history"],
        if model.history_visible {
            C!["visible"]
        } else {
            C![]
        },
        if model.history_visible {
            div![
                C!["messages", "togglable"],
                player.log.msgs.iter().map(|(time, msg)| {
                    div![
                        C!["message"],
                        span![C!["time"], format!("{} ago: ", fmt_time(state.time - time))],
                        match msg {
                            LogMsg::NotEnoughSpaceForDwarf => {
                                span![format!("You got a dwarf but don't have enough space for him.")]
                            }
                            LogMsg::NewPlayer(user_id) => {
                                span![format!(
                                    "A new player has joined the game, say hi to {}!",
                                    state.players.get(user_id).unwrap().username
                                )]
                            }
                            LogMsg::MoneyForKing(money) => {
                                span![format!(
                                    "You are the king and earned 🜚{}!",
                                    money
                                )]
                            }
                            LogMsg::NewDwarf(dwarf_id) => {
                                span![format!(
                                    "Your settlement got a new dwarf {}.",
                                    player.dwarfs.get(dwarf_id).unwrap().name
                                )]
                            }
                            LogMsg::DwarfDied(name) => {
                                span![format!("Your dwarf {} has died.", name)]
                            }
                            LogMsg::QuestCompletedItems(quest, items) => {
                                if let Some(items) = items {
                                    span![format!(
                                        "You completed the quest {} and won {}.",
                                        quest,
                                        items
                                            .clone()
                                            .sorted_by_rarity()
                                            .into_iter()
                                            .map(|(item, n)| format!("{n}x {item}"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    )]
                                } else {
                                    span![format!(
                                        "You did not get any items from the quest {}.",
                                        quest
                                    )]
                                }
                            }
                            LogMsg::QuestCompletedMoney(quest, money) => {
                                span![format!(
                                    "You completed the quest {} and earned 🜚{}.",
                                    quest, money
                                )]
                            }
                            LogMsg::QuestCompletedPrestige(quest, success) => {
                                if *success {
                                    span![format!(
                                        "You completed the quest {} and started a new settlement.",
                                        quest
                                    )]
                                } else {
                                    span![format!(
                                        "You did not start a new settlement from the quest {}.",
                                        quest
                                    )]
                                }
                            }
                            LogMsg::QuestCompletedKing(quest, success) => {
                                if *success {
                                    span![format!(
                                        "You completed the quest {} and became the King.",
                                        quest
                                    )]
                                } else {
                                    span![format!(
                                        "You did not become the King from the quest {}.",
                                        quest
                                    )]
                                }
                            }
                            LogMsg::QuestCompletedDwarfs(quest, num_dwarfs) => {
                                if let Some(num_dwarfs) = num_dwarfs {
                                    if *num_dwarfs == 1 {
                                        span![format!(
                                            "You completed the quest {} and got a new dwarf.",
                                            quest
                                        )]
                                    } else {
                                        span![format!(
                                            "You completed the quest {} and got {} new dwarfs.",
                                            quest, num_dwarfs
                                        )]
                                    }
                                } else {
                                    span![format!(
                                        "You did not get any new dwarfs from the quest {}.",
                                        quest
                                    )]
                                }
                            }
                            LogMsg::OpenedLootCrate(items) => {
                                span![format!(
                                    "You opened a loot crate and got {}.",
                                    items
                                        .clone()
                                        .sorted_by_rarity()
                                        .into_iter()
                                        .map(|(item, n)| format!("{n}x {item}"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )]
                            }
                        }
                    ]
                })
            ]
        } else {
            Node::Empty
        },
        button![ev(Ev::Click, move |_| Msg::ToggleHistory), "Toggle History",],
    ]
}

fn bundle(requires: &Bundle<Item>, player: &Player) -> Node<Msg> {
    ul![requires
        .clone()
        .sorted_by_rarity()
        .into_iter()
        .map(|(item, n)| {
            let available = player.inventory.items.check_remove(&Bundle::new().add(item, n));
            li![C!["clickable-item"],
                span![
                    if available { C![] } else { C!["unavailable"]},
                    format!("{n}x {item}"), ev(Ev::Click, move |_| Msg::GoToItem(item))
                ],
                span![format!(" ({})", player.inventory.items.get(&item).copied().unwrap_or_default())]
            ]
        })]
}

fn stats(stats: &Stats) -> Node<Msg> {
    let mut v = Vec::new();

    if stats.strength != 0 {
        v.push((stats.strength, "Strength"));
    }
    if stats.endurance != 0 {
        v.push((stats.endurance, "Endurance"));
    }
    if stats.agility != 0 {
        v.push((stats.agility, "Agility"));
    }
    if stats.intelligence != 0 {
        v.push((stats.intelligence, "Intelligence"));
    }
    if stats.perception != 0 {
        v.push((stats.perception, "Perception"));
    }

    v.sort_by_key(|t| -t.0);

    span![v.into_iter().map(|(num, abv)| span![format!("{abv} "), stars(num, false)]).intersperse(br![])]
}

fn stars(stars: i8, padded: bool) -> Node<Msg> {
    let mut s = String::new();
    if stars < 0 {
        s += "-";
    }
    for _ in 0..(stars.abs() / 2) {
        s += "★";
    }
    if stars.abs() % 2 == 1 {
        s += if padded { "⯪" } else { "⯨" }
    }
    if padded {
        for _ in 0..((10 - stars.abs()) / 2) {
            s += "☆";
        }
    }
    span![C!["symbols"], s]
}

fn nav(model: &Model) -> Node<Msg> {
    /* 
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
            if let Page::Dwarfs(DwarfsMode::Overview) = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(
                DwarfsMode::Overview
            ))),
            "Dwarfs",
        ],
        button![
            if let Page::Inventory(InventoryMode::Crafting) = model.page {
                attrs! {At::Disabled => "true"}
            } else {
                attrs! {}
            },
            ev(Ev::Click, move |_| Msg::ChangePage(Page::Inventory(
                InventoryMode::Crafting
            ))),
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
    */

    nav![div![
        a![C!["button"],
            if let Page::Base = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game"}
            } else {
                attrs! {At::Href => "/game"}
            },
            "Settlement",
        ],
        a![C!["button"],
            if let Page::Dwarfs(DwarfsMode::Overview) = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game/dwarfs"}
            } else {
                attrs! {At::Href => "/game/dwarfs"}
            },
            "Dwarfs",
        ],
        a![C!["button"],
            if let Page::Inventory(InventoryMode::Crafting) = model.page {
                attrs! {At::Disabled => "true",  At::Href => "/game/inventory"}
            } else {
                attrs! {At::Href => "/game/inventory"}
            },
            "Inventory",
        ],
        a![C!["button"],
            if let Page::Quests = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game/quests"}
            } else {
                attrs! {At::Href => "/game/quests"}
            },
            "Quests",
        ],
        a![C!["button"],
            if let Page::Ranking = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game/ranking"}
            } else {
                attrs! {At::Href => "/game/ranking"}
            },
            "Ranking",
        ],
        //a![C!["button"], attrs! { At::Href => "/account"}, "Account"]
    ]]
}

fn tip<T: std::fmt::Display>(text: T) -> Node<Msg> {
    div![
        C!["tooltip"],
        span![C!["symbols"], "ⓘ"],
        span![C!["tooltiptext"], format!("{}", text)]
    ]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
