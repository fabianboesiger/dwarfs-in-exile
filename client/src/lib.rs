mod images;

use engine_client::{ClientState, EventWrapper, Msg as EngineMsg};
use images::Image;
use itertools::Itertools;
use seed::{prelude::*, *};
use shared::{
    Bundle, ClientEvent, Craftable, DwarfId, Health, Item, ItemRarity, ItemType, LogMsg, Occupation, Player, QuestId, QuestType, RewardMode, Stats, Time, LOOT_CRATE_COST, MAX_HEALTH, SPEED
};
use std::str::FromStr;
use web_sys::js_sys::Date;

#[cfg(not(debug_assertions))]
const WS_URL: &str = "wss://dwarfs-in-exile.com/game/ws";
#[cfg(debug_assertions)]
const WS_URL: &str = "ws://localhost:3000/game/ws";

// ------ ------
//     Model
// ------ ------

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Page {
    Dwarfs(DwarfsMode),
    Dwarf(DwarfId),
    Base,
    Inventory(InventoryMode),
    Quests,
    Quest(QuestId),
    Ranking,
}

impl Page {
    fn from_url(mut url: Url) -> Self {
        url.next_path_part().unwrap();
        match url.next_path_part() {
            Some("dwarfs") => match url.next_path_part() {
                None => Page::Dwarfs(DwarfsMode::Overview),
                Some(id) => Page::Dwarf(id.parse().unwrap()),
            },
            Some("inventory") => Page::Inventory(InventoryMode::Overview),
            Some("quests") => match url.next_path_part() {
                None => Page::Quests,
                Some(id) => Page::Quest(id.parse().unwrap()),
            },
            Some("ranking") => Page::Ranking,
            _ => Page::Base,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InventoryMode {
    Overview,
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
    Quest(QuestId, usize),
}

pub struct InventoryFilter {
    item_name: String,
    craftable: bool,
    food: bool,
    owned: bool,
    pets: bool,
    clothing: bool,
    tools: bool,
}

impl Default for InventoryFilter {
    fn default() -> Self {
        Self {
            item_name: String::new(),
            craftable: true,
            food: false,
            owned: true,
            pets: false,
            clothing: false,
            tools: false,
        }
    }
}

pub struct Model {
    state: ClientState<shared::State>,
    page: Page,
    message: String,
    chat_visible: bool,
    history_visible: bool,
    inventory_filter: InventoryFilter,
    map_time: Option<(Time, f64)>,
}

impl Model {
    fn sync_timestamp_millis_now(&mut self, time: Time) {
        self.map_time = Some((time, Date::now()));
    }

    fn get_timestamp_millis_of(&self, time: Time) -> Option<f64> {
        Some(self.map_time?.1 + (time as f64 - self.map_time?.0 as f64) * 1000.0 / SPEED as f64)
    }

    fn get_timestamp_millis_diff_now(&self, time: Time) -> Option<f64> {
        Some(Date::now() - self.get_timestamp_millis_of(time)?)
    }
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
    orders.subscribe(|subs::UrlChanged(url)| Msg::ChangePage(Page::from_url(url)));

    Model {
        state: ClientState::init(orders, WS_URL.to_owned()),
        page: Page::from_url(url),
        message: String::new(),
        chat_visible: false,
        history_visible: false,
        inventory_filter: InventoryFilter::default(),
        map_time: None,
    }
}

// ------ ------
//    Update
// ------ ------
/*
#[derive(Debug)]
pub enum Msg {
    WebSocketOpened,
    CloseWebSocket,
    WebSocketClosed(CloseEvent),
    WebSocketFailed,
    ReconnectWebSocket(usize),
    SendGameEvent(ClientEvent),
    ReceiveGameEvent(EventData),
    InitGameState(SyncData),

}

impl EngineMsg<shared::State> for Msg {}
*/

#[derive(Debug)]
pub enum Msg {
    GameStateEvent(EventWrapper<shared::State>),
    ChangePage(Page),
    ChangeMessage(String),
    SubmitMessage,
    ToggleChat,
    ToggleHistory,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    AssignToQuest(QuestId, usize, Option<DwarfId>),
    InventoryFilterFood,
    InventoryFilterOwned,
    InventoryFilterCraftable,
    InventoryFilterTools,
    InventoryFilterPets,
    InventoryFilterClothing,
    InventoryFilterName(String),
    InventoryFilterReset,
    GoToItem(Item),
}

impl EngineMsg<shared::State> for Msg {}

impl From<EventWrapper<shared::State>> for Msg {
    fn from(event: EventWrapper<shared::State>) -> Self {
        Self::GameStateEvent(event)
    }
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::GameStateEvent(ev) => {
            if let EventWrapper::InitGameState(sync_data) = &ev {
                model.sync_timestamp_millis_now(sync_data.state.state.time);
            }
            model.state.update(ev, orders);
        }
        Msg::ChangePage(page) => {
            model.page = page;
        }
        Msg::ChangeMessage(message) => {
            model.message = message;
        }
        Msg::SubmitMessage => {
            //send(ClientEvent::Message(model.message.clone()));
            orders.send_msg(Msg::send_event(ClientEvent::Message(model.message.clone())));
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
        Msg::AssignToQuest(quest_id, dwarf_idx, dwarf_id) => {
            if dwarf_id.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!("/game/quests/{}", quest_id)).unwrap(),
                ));
            }
            orders.send_msg(Msg::send_event(ClientEvent::AssignToQuest(
                quest_id, dwarf_idx, dwarf_id,
            )));
        }
        Msg::ChangeEquipment(dwarf_id, item_type, item) => {
            if item.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!("/game/dwarfs/{}", dwarf_id)).unwrap(),
                ));
            }
            orders.send_msg(Msg::send_event(ClientEvent::ChangeEquipment(
                dwarf_id, item_type, item,
            )));
        }
        Msg::GoToItem(item) => {
            model.inventory_filter = InventoryFilter::default();
            model.inventory_filter.item_name = item.to_string();
            orders.notify(subs::UrlRequested::new(
                Url::from_str("/game/inventory").unwrap(),
            ));
        }
    }
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> Vec<Node<Msg>> {
    if let (Some(state), Some(user_id), client_state) = (
        model.state.get_state(),
        model.state.get_user_id(),
        &model.state,
    ) {
        vec![
            div![id!["background"]],
            header![h1![a![attrs! { At::Href => "/" }, "Dwarfs in Exile"]]],
            nav(model),
            #[cfg(debug_assertions)]
            div![
                id!["server-info"],
                /*span![format!(
                    "status: {}, tick number: {}, tick rate: 1/{} s",
                    if model.web_socket_reconnector.is_none() {
                        "connected"
                    } else {
                        "disconnected"
                    },
                    data.state.time,
                    shared::SPEED,
                )]*/
            ],
            main![match model.page {
                Page::Dwarfs(mode) => dwarfs(state, user_id, mode),
                Page::Dwarf(dwarf_id) => dwarf(model, state, user_id, dwarf_id),
                Page::Base => base(model, state, user_id),
                Page::Inventory(mode) => inventory(model, state, user_id, mode),
                Page::Ranking => ranking(state, client_state),
                Page::Quests => quests(state, user_id),
                Page::Quest(quest_id) => quest(state, user_id, quest_id),
            }],
            chat(model, state, client_state),
            history(model, state, user_id, client_state),
            last_received_items(model, state, user_id),
        ]
    } else {
        vec![
            div![id!["background"]],
            header![h1![a![attrs! { At::Href => "/" }, "Dwarfs in Exile"]]],
            div![C!["loading"], "Loading ..."],
        ]
    }
}

fn ranking(state: &shared::State, client_state: &ClientState<shared::State>) -> Node<Msg> {
    let mut players: Vec<_> = state.players.iter().collect();
    players.sort_by_key(|(_, p)| (-(p.base.prestige as i64), -(p.dwarfs.len() as i64)));

    div![
        C!["content"],
        h2!["Ranking"],
        if let Some(king) = state.king {
            p![format!("All hail our King {}!", client_state.get_user_data(&king).map(|data| data.username.clone()).unwrap_or_default()), tip("The king gets one tenth of all money that was earned. Make sure you become the king as soon as the quest becomes available.")]
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
            players.iter().enumerate().map(|(i, (user_id, player))| {
                let rank = i + 1;
                tr![
                    td![rank],
                    td![
                        format!(
                            "{} ",
                            client_state
                                .get_user_data(&user_id)
                                .map(|data| data.username.clone())
                                .unwrap_or_default()
                        ),
                        span![
                            C![
                                "symbols",
                                if player.is_online(state.time) {
                                    "online"
                                } else {
                                    "offline"
                                }
                            ],
                            "●"
                        ]
                    ],
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

fn last_received_items(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    div![
        C!["received-item-popup"],
        player
            .inventory
            .last_received
            .iter()
            .enumerate()
            .filter_map(|(idx, (item, qty, time))| {
                let time_diff_millis = model.get_timestamp_millis_diff_now(*time)?;

                if time_diff_millis > 3000.0 {
                    None
                } else {
                    Some(div![
                        C!["received-item"],
                        style![St::Opacity => format!("{}", 1.0 - time_diff_millis / 3000.0)],
                        match item.item_rarity() {
                            ItemRarity::Common => C!["item-common"],
                            ItemRarity::Uncommon => C!["item-uncommon"],
                            ItemRarity::Rare => C!["item-rare"],
                            ItemRarity::Epic => C!["item-epic"],
                            ItemRarity::Legendary => C!["item-legendary"],
                        },
                        img![
                            C!["received-item-image"],
                            attrs! {At::Src => Image::from(*item).as_at_value()},
                        ],
                        div![C!["received-item-content"], format!("+{}", qty)]
                    ])
                }
            })
    ]
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

fn score_bar(curr: u64, max: u64, rank: usize, max_rank: usize) -> Node<Msg> {
    div![
        C!["score-bar-wrapper"],
        div![
            C!["score-bar-curr"],
            attrs! {
                At::Style => format!("width: calc(100% / {max} * {curr});")
            }
        ],
        div![
            C!["score-bar-overlay"],
            if curr == max {
                format!(
                    "{} XP ({} place of {})",
                    big_number(curr),
                    enumerate(rank),
                    max_rank
                )
            } else {
                format!(
                    "{} / {}XP ({} place of {})",
                    big_number(curr),
                    big_number(max),
                    enumerate(rank),
                    max_rank
                )
            }
        ],
    ]
}

fn dwarfs(state: &shared::State, user_id: &shared::UserId, mode: DwarfsMode) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();

    if player.dwarfs.len() > 0 {
        table![
            C!["dwarfs", "list"],
            player.dwarfs.iter().map(|(&id, dwarf)| tr![
                C!["dwarf", format!("dwarf-{}", id)],
                C!["list-item-row"],
                td![img![
                    C!["list-item-image"],
                    attrs! {At::Src => Image::dwarf_from_name(&dwarf.name).as_at_value()}
                ]],
                td![
                    C!["list-item-content"],
                    h3![C!["title"], &dwarf.name],
                    p![
                        C!["subtitle"],
                        if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
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
                        }
                    ],
                    health_bar(dwarf.health, MAX_HEALTH),
                    p![match mode {
                        DwarfsMode::Overview => {
                            a![
                                C!["button"],
                                attrs! { At::Href => format!("/game/dwarfs/{}", id) },
                                "Details"
                            ]
                        }
                        DwarfsMode::Select(DwarfsSelect::Quest(quest_id, dwarf_idx)) => {
                            div![button![
                                ev(Ev::Click, move |_| Msg::AssignToQuest(
                                    quest_id,
                                    dwarf_idx,
                                    Some(id)
                                )),
                                format!(
                                    "Assign to Quest {}",
                                    state.quests.get(&quest_id).unwrap().quest_type
                                ),
                                br![],
                                stars(
                                    dwarf.effectiveness(
                                        state
                                            .quests
                                            .get(&quest_id)
                                            .unwrap()
                                            .quest_type
                                            .occupation()
                                    ) as i8,
                                    true
                                )
                            ]]
                        }
                    }]
                ],
            ])
        ]
    } else {
        div![
            C!["content"],
            h2!["There's Noone Here!"],
            p!["All your dwarfs have died! You can wait until a new dwarf finds your settlement or start over with a new settlement."],
            button![
                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::Restart)),
                "Restart your Settlement",
            ],
        ]
    }
}

fn dwarf(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    dwarf_id: DwarfId,
) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();
    let dwarf = player.dwarfs.get(&dwarf_id);
    let is_premium = model
        .state
        .get_user_data(user_id)
        .map(|user_data| user_data.premium > 0)
        .unwrap_or(false);

    if let Some(dwarf) = dwarf {
        div![
            C!["content"],
            C!["dwarf", format!("dwarf-{}", dwarf_id)],
            img![attrs! {At::Src => Image::dwarf_from_name(&dwarf.name).as_at_value()}],
                    
            h3![C!["title"], &dwarf.name],
            p![C!["subtitle"],
            if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
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
            p![
                button![
                    if is_premium {
                        attrs! {}
                    } else {
                        attrs! {At::Disabled => "true"}
                    },
                    ev(Ev::Click, move |_| Msg::send_event(
                        ClientEvent::ToggleAutoIdle
                    )),
                    if player.auto_functions.auto_idle && is_premium { "Disable Auto Idling" } else { "Enable Auto Idling" },
                    if !is_premium {
                        tip("This functionality requires a premium account.")
                    } else {
                        Node::Empty
                    }
                ]
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
                table![C!["list"],
                    enum_iterator::all::<ItemType>().map(|item_type| {
                        let equipment = dwarf.equipment.get(&item_type).unwrap();

                        tr![C!["list-item-row"],
                            if let Some(equipment) = equipment {
                                td![img![C!["list-item-image"], attrs! { At::Src => Image::from(*equipment).as_at_value() } ]]
                            } else {
                                td![div![C!["list-item-image-placeholder"]]]
                            },
                            td![C!["list-item-content", "grow"],
                                h3![C!["title"],
                                    equipment.map(|equipment| format!("{equipment}")).unwrap_or("None".to_owned())
                                ],
                                p![C!["subtitle"],
                                    format!("{item_type}")
                                ],

                                if let Some(item) = equipment {
                                    vec![
                                        // Show stats
                                        if !item.provides_stats().is_zero() {
                                            div![
                                                h4!["Provides"],
                                                stats(&item.provides_stats()),
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
                                    ]
                                } else {
                                    Vec::new()
                                }
                                
                            ],
                            td![C!["list-item-content", "shrink"],
                                button![
                                    ev(Ev::Click, move |_| Msg::ChangePage(Page::Inventory(
                                        InventoryMode::Select(InventorySelect::Equipment(
                                            dwarf_id, item_type
                                        ))
                                    ))),
                                    "Change"
                                ],
                                if equipment.is_some() {
                                    button![
                                        ev(Ev::Click, move |_| Msg::ChangeEquipment(
                                            dwarf_id, item_type, None
                                        )),
                                        "Unequip"
                                    ]
                                } else {
                                    Node::Empty
                                }
                            ]
                        ]
                    })
                ]
                /* 
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
                                        dwarf_id, item_type
                                    ))
                                ))),
                                "Change"
                            ],
                            if equipment.is_some() {
                                button![
                                    ev(Ev::Click, move |_| Msg::ChangeEquipment(
                                        dwarf_id, item_type, None
                                    )),
                                    "Unequip"
                                ]
                            } else {
                                Node::Empty
                            }
                        ],
                    ]
                })]
                */
            ],
            
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
                    table![C!["list"],
                        enum_iterator::all::<Occupation>().filter(|occupation| player.base.curr_level >= occupation.unlocked_at_level()).map(|occupation| {
                            let all_items = enum_iterator::all::<Item>().filter_map(|item| item.item_probability(occupation).map(|_| item)).collect::<Vec<_>>();
                            tr![C!["list-item-row"],
                                td![img![C!["list-item-image"], attrs! { At::Src => Image::from(occupation).as_at_value() } ]],
                                td![C!["list-item-content", "grow"],
                                    h3![C!["title"],
                                        format!("{}", occupation),
                                        /*if all_items.len() == 0 {
                                            tip(format!("From this occupation, you can get no items. This occupation requires {}.", stats_simple(&occupation.requires_stats())))
                                        } else {
                                            tip(format!("From this occupation, you can get the items {}. This occupation requires {}.", all_items.into_iter().join(", "), stats_simple(&occupation.requires_stats())))
                                        },*/
                                    ],
                                    p![
                                        stars(dwarf.effectiveness(occupation) as i8, true),
                                        tip("The stars indicate the effectivenes of this dwarf in this occupation.")
                                    ],
                                    p![
                                        button![
                                            if occupation == dwarf.occupation
                                                || dwarf.participates_in_quest.is_some()
                                            {
                                                attrs! {At::Disabled => "true"}
                                            } else {
                                                attrs! {}
                                            },
                                            ev(Ev::Click, move |_| Msg::send_event(
                                                ClientEvent::ChangeOccupation(dwarf_id, occupation)
                                            )),
                                            "Select"
                                        ],
                                    ]
                                    
                                    /*if !occupation.requires_stats().is_zero() {
                                        p![
                                            h4!["Requires"],
                                            stats(&occupation.requires_stats()),
                                        ]
                                    } else {
                                        Node::Empty
                                    },*/
                                ],
                                td![C!["list-item-content", "shrink"],
                                    h4![C!["title"], "Requires"],
                                    p![C!["subtitle"],stats_simple(&occupation.requires_stats())],
                                    h4![C!["title"], "Provides"],
                                    p![C!["subtitle"], if all_items.len() == 0 {
                                        "None".to_owned()
                                    } else {
                                        all_items.into_iter().join(", ")
                                    }]
                                ]
                            ]
                            /* 
                            button![
                                if occupation == dwarf.occupation
                                    || dwarf.participates_in_quest.is_some()
                                {
                                    attrs! {At::Disabled => "true"}
                                } else {
                                    attrs! {}
                                },
                                ev(Ev::Click, move |_| Msg::send_event(
                                    ClientEvent::ChangeOccupation(dwarf_id, occupation)
                                )),
                                h3![
                                    format!("{}", occupation),
                                    if all_items.len() == 0 {
                                        tip("From this occupation, you can get nothing.")
                                    } else {
                                        tip(format!("From this occupation, you can get the items {}.", all_items.into_iter().join(", ")))
                                    },
                                ],
                                br![],
                                stars(dwarf.effectiveness(occupation) as i8, true),
                                
                            ]
                            */
                        })
                    ]
                }                     
            ]
        ]
    } else {
        div![
            C!["content"],
            h2!["There's Noone Here!"],
            p!["This dwarf has died!"],
            a![attrs! { At::Href => "/game/dwarfs" }, "Go back"],
        ]
    }
}

fn quests(state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    let _player = state.players.get(user_id).unwrap();

    table![
        C!["quests", "list"],
        state.quests.iter().map(|(quest_id, quest)| {
            tr![
                C!["list-item-row"],
                td![img![
                    C!["list-item-image"],
                    attrs! {At::Src => Image::from(quest.quest_type).as_at_value()}
                ]],
                td![
                    C!["list-item-content"],
                    h3![C!["title"], format!("{}", quest.quest_type)],
                    p![
                        C!["subtitle"],
                        format!("{} remaining.", fmt_time(quest.time_left))
                    ],
                    if let Some(contestant) = quest.contestants.get(user_id) {
                        let rank = quest
                            .contestants
                            .values()
                            .filter(|c| c.achieved_score >= contestant.achieved_score)
                            .count();
                        let mut contestants = quest.contestants.values().collect::<Vec<_>>();
                        contestants.sort_by_key(|c| c.achieved_score);
                        let best_score = contestants.last().unwrap().achieved_score;
                        p![score_bar(
                            contestant.achieved_score,
                            best_score,
                            rank,
                            quest.contestants.len()
                        )]
                    } else {
                        Node::Empty
                    },
                    a![
                        C!["button"],
                        attrs! { At::Href => format!("/game/quests/{}", quest_id) },
                        "Details"
                    ]
                ]
            ]
        })
    ]
}

fn quest(state: &shared::State, user_id: &shared::UserId, quest_id: QuestId) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();
    let quest = state.quests.get(&quest_id);

    if let Some(quest) = quest {
        div![
            C!["content"],
            div![
                C!["list-item-row"],
                img![C!["list-item-image"], attrs! {At::Src => Image::from(quest.quest_type).as_at_value()}],
                
                div![
                    C!["list-item-content"],
                    h3![C!["title"], format!("{}", quest.quest_type)],
                    p![C!["subtitle"], format!("{} remaining.", fmt_time(quest.time_left))],
                   
                ]
            ],
    
            p![  
                match quest.quest_type {
                    QuestType::KillTheDragon => p!["A dragon was found high up in the mountains in the forbidden lands. Send your best warriors to defeat it."],
                    QuestType::ArenaFight => p!["The King of the Dwarfs has invited the exilants to compete in an arena fight against monsters and creatures from the forbidden lands. The toughest warrior will be rewarded with a gift from the king personally."],
                    QuestType::ExploreNewLands => p!["Send dwarfs to explore new lands and find a place for a new settlement. The new settlement will be a better version of your previous settlement that allows a larger maximal population. Additionally, the new settlement will attract more and better dwarfs. Keep in mind that if this quest is sucessful, you will loose all of your dwarfs that you left back home."],
                    QuestType::FeastForAGuest => p!["Your village is visted by an ominous guest. Go hunting and organize a feast for the guest, and he may stay."],
                    QuestType::FreeTheVillage => p!["The Elven Village was raided by the Orks. Free the Elves to earn a reward!"],
                    QuestType::ADwarfGotLost => p!["Search for a dwarf that got lost in the wilderness. If you find him first, he may stay in your settlement!"],
                    QuestType::AFishingFriend => p!["Go fishing and make friends!"],
                    QuestType::ADwarfInDanger => p!["Free a dwarf that gets robbed by Orks. If you free him first, he may stay in your settlement!"],
                    QuestType::ForTheKing => p!["Fight a ruthless battle to become the king over all of Exile Island!"],
                    QuestType::DrunkFishing => p!["Participate in the drunk fishing contest! The dwarf that is the most successful drunk fisher gets a reward."],
                    QuestType::CollapsedCave => p!["A cave has collapsed and a dwarf is trapped inside. Be the first to save is life and he will move into your settlement."]
                },
            ], 
            
            p![format!("This quest requires {}.", quest.quest_type.occupation().to_string().to_lowercase())],
            h4!["Rewards"],
            match quest.quest_type.reward_mode() {
                RewardMode::BestGetsAll(money) => div![p![format!("The best player gets {money} coins, the rest gets nothing.")]],
                RewardMode::SplitFairly(money) => div![p![format!("A total of {money} coins are split fairly between the players.")]],
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
                    p![bundle(&items, player, false)]
                ],
                RewardMode::NewDwarf(num) => div![p![format!("The best participant gets {num} new dwarf for their settlement.")]],
            },
            h4!["Participate"],
            
            if let Some(contestant) = quest.contestants.get(user_id) {
                let rank = quest.contestants.values().filter(|c| c.achieved_score >= contestant.achieved_score).count();
                let mut contestants = quest.contestants.values().collect::<Vec<_>>();
                contestants.sort_by_key(|c| c.achieved_score);
                let best_score = contestants.last().unwrap().achieved_score;
                p![
                    score_bar(contestant.achieved_score, best_score, rank, quest.contestants.len())
                ]
            } else {
                Node::Empty
            },
    
            table![C!["list"],
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
                    let dwarf = dwarf_id.map(|dwarf_id| player.dwarfs.get(&dwarf_id).unwrap());
    
                    tr![
                        C!["list-item-row"],
                        if let Some(dwarf) = dwarf {
                            td![img![C!["list-item-image"], attrs! {At::Src => Image::dwarf_from_name(&dwarf.name).as_at_value()}]]
                        } else {
                            td![div![C!["list-item-image-placeholder"]]]
                        },
                        td![
                            C!["list-item-content"],
                            if let Some(dwarf) = dwarf {
                                vec![
                                    h3![C!["title"], &dwarf.name],
                                    stars(dwarf.effectiveness(state.quests.get(&quest_id).unwrap().quest_type.occupation()) as i8, true),
                                ]
                            } else {
                                vec![
                                    h3![C!["title"], "None"]
                                ]
                            },
                            /*span![
                                C!["subtitle"], 
                            ],*/
                            button![
                                ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(DwarfsMode::Select(DwarfsSelect::Quest(quest_id, dwarf_idx))))),
                                "Change"
                            ],
                            if dwarf_id.is_some() {
                                button![
                                    ev(Ev::Click, move |_| Msg::AssignToQuest(quest_id, dwarf_idx, None)),
                                    "Remove"
                                ]
                            } else {
                                Node::Empty
                            }
                        ]
                    ]
                    /*tr![
                        td![
                            .unwrap_or(String::from("None"))
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
                    ]*/
                })
            ]
        ]
    } else {
        div![
            C!["content"],
            h2!["There's Nothing Here!"],
            p!["This quest was completed!"],
            a![attrs! { At::Href => "/game/quests" }, "Go back"],
        ]
    }
    
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

fn enumerate(num: usize) -> String {
    match num {
        1 => "first".to_owned(),
        2 => "second".to_owned(),
        3 => "third".to_owned(),
        _ => format!("{num}th"),
    }
}

fn base(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();
    let is_premium = model
        .state
        .get_user_data(user_id)
        .map(|user_data| user_data.premium > 0)
        .unwrap_or(false);

    div![C!["content"],
        h2!["Your Settlement"],
        img![attrs! {At::Src => Image::from(player.base.village_type()).as_at_value()}],
        table![
            tr![th![
                "Settlement Type",
                tip("There are ten different types of settlements that get gradually better: Outpost, Dwelling, Hamlet, Village, Small Town, Large Town, Small City, Large City, Metropolis, Megalopolis. To move on to a better settlement type, you need to complete a special quest. Better settlements attract more and better dwarfs.")],
                td![format!("{}", player.base.village_type())]
            ],
            //tr![th!["Settlement Level"], td![format!("{} / {}", player.base.curr_level, player.base.max_level())]],
            tr![th!["Population", tip("Upgrade your settlement to increase the maximum population. You can get new dwarfs from certain quests or at random.")], td![format!("{}/{}", player.dwarfs.len(), player.base.num_dwarfs())]],
            tr![th!["Money", tip("Earn money by doing quests. With money, you can buy loot crates.")], td![format!("{} coins", player.money)]],
            tr![th!["Food", tip("Your settlement can store food for your dwarfs to consume. One quantity of food restores 0.1% of a dwarfs health.")], td![format!("{}", player.base.food)]],
        ],
        if let Some(requires) = player.base.upgrade_cost() {
            div![
                h3!["Upgrade Settlement"],
                p!["Upgrade your settlement to increase the maximum population and unlock new occupations for your dwarfs."],
                if let Some(unlocked_occupation) = enum_iterator::all::<Occupation>().filter(|occupation| occupation.unlocked_at_level() == player.base.curr_level + 1).next() {
                    p![format!("The next upgrade increases your maximal population by one and unlocks the occupation {}.", unlocked_occupation)]
                } else {
                    p!["The next upgrade increases your maximal population by one."]
                },
                bundle(&requires, player, true),
                button![
                    if player.inventory.items.check_remove(&requires) {
                        attrs! {}
                    } else {
                        attrs! {At::Disabled => "true"}
                    },
                    ev(Ev::Click, move |_| Msg::send_event(ClientEvent::UpgradeBase)),
                    "Upgrade",
                ]
            ]
        } else {
            Node::Empty
        },
        div![
            h3!["Open Loot Crate"],
            p!["A loot crate contains a random epic or legendary item. You can earn loot crates by completing quests."],
            button![
                if player.money >= LOOT_CRATE_COST && is_premium {
                    attrs! {}
                } else {
                    attrs! {At::Disabled => "true"}
                },
                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::OpenLootCrate)),
                format!("Buy and Open ({} coins)", LOOT_CRATE_COST),
                if !is_premium {
                    tip("This functionality requires a premium account.")
                } else {
                    Node::Empty
                }
            ]
        ]
    ]
}

fn inventory(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    mode: InventoryMode,
) -> Node<Msg> {
    let player = state.players.get(user_id).unwrap();
    let is_premium = model
        .state
        .get_user_data(user_id)
        .map(|user_data| user_data.premium > 0)
        .unwrap_or(false);

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
                    attrs! {At::Type => "text", At::Value => model.inventory_filter.item_name, At::Placeholder => "Item Name"},
                    input_ev(Ev::Input, Msg::InventoryFilterName)
                ],
                button![
                    ev(Ev::Click, move |_| Msg::InventoryFilterReset),
                    "Reset Filter",
                ],
            ]
        ],
        table![
            C!["items", "list"],
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
                .map(|(item, n)| tr![
                    C!["item"],
                    C!["list-item-row"],
                    match item.item_rarity() {
                        ItemRarity::Common => C!["item-common"],
                        ItemRarity::Uncommon => C!["item-uncommon"],
                        ItemRarity::Rare => C!["item-rare"],
                        ItemRarity::Epic => C!["item-epic"],
                        ItemRarity::Legendary => C!["item-legendary"],
                    },
                    td![img![C!["list-item-image"], attrs! {At::Src => Image::from(item).as_at_value()}]],
                    td![
                        C!["list-item-content", "grow"],
                        
                        h3![C!["title"], format!("{}x {item}", big_number(n))],
                        p![
                            C!["subtitle"], if let Some(food) = item.nutritional_value() {
                                format!("Food ({food}) | {}", item.item_rarity())
                            } else if let Some(item_type) = item.item_type() {
                                format!("{item_type} | {}", item.item_rarity())
                            } else {
                                format!("Item | {}", item.item_rarity())
                            }
                        ],

                        // Show stats
                        if !item.provides_stats().is_zero() {
                            div![
                                h4!["Provides"],
                                stats(&item.provides_stats()),
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

                    ],

                    if let InventoryMode::Select(InventorySelect::Equipment(
                        dwarf_id,
                        item_type,
                    )) = mode {
                        td![
                            C!["list-item-content", "shrink"],
                            button![
                                ev(Ev::Click, move |_| Msg::ChangeEquipment(
                                    dwarf_id,
                                    item_type,
                                    Some(item)
                                )),
                                "Equip"
                            ]
                        ]
                    } else {     
                        td![
                            C!["list-item-content", "shrink"],
                            if let Some(requires) = item.requires() {
                                vec![
                                    h4!["Crafting"],
                                    bundle(&requires, player, true),
                                    if player.auto_functions.auto_craft.contains(&item) && is_premium {
                                        button![
                                            ev(Ev::Click, move |_| Msg::send_event(
                                                ClientEvent::ToggleAutoCraft(item)
                                            )),
                                            "Disable Auto",
                                        ]
                                    } else {
                                        div![C!["button-row"],
                                            button![
                                                if player.inventory.items.check_remove(&requires) {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::Craft(item, 1)
                                                )),
                                                "Craft",
                                            ],
                                            button![
                                                if player.inventory.items.check_remove(&requires.clone().mul(10)) {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::Craft(item, 10)
                                                )),
                                                "10x",
                                            ],
                                            button![
                                                if player.inventory.items.check_remove(&requires.clone().mul(100)) {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::Craft(item, 100)
                                                )),
                                                "100x",
                                            ],
                                            button![
                                                if is_premium {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::ToggleAutoCraft(item)
                                                )),
                                                "Auto",
                                                if !is_premium {
                                                    tip("This functionality requires a premium account.")
                                                } else {
                                                    Node::Empty
                                                }
                                            ]
                                        ]
                                    }

                                ]
                            } else {
                                Vec::new()
                            },

                            if let Some(_) = item.nutritional_value() {
                                vec![
                                    h4!["Food Storage"],
                                    if player.auto_functions.auto_store.contains(&item) && is_premium {
                                        button![
                                            ev(Ev::Click, move |_| Msg::send_event(
                                                ClientEvent::ToggleAutoStore(item)
                                            )),
                                            "Disable Auto"
                                        ]
                                    } else {
                                        div![C!["button-row"],
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
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::AddToFoodStorage(item, 1)
                                                )),
                                                format!("Store"),
                                            ],
                                            button![
                                                if player
                                                    .inventory
                                                    .items
                                                    .check_remove(&Bundle::new().add(item, 10))
                                                {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::AddToFoodStorage(item, 10)
                                                )),
                                                format!("10x"),
                                            ],
                                            button![
                                                if player
                                                    .inventory
                                                    .items
                                                    .check_remove(&Bundle::new().add(item, 100))
                                                {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::AddToFoodStorage(item, 100)
                                                )),
                                                format!("100x"),
                                            ],
                                            button![
                                                if is_premium {
                                                    attrs! {}
                                                } else {
                                                    attrs! {At::Disabled => "true"}
                                                },
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::ToggleAutoStore(item)
                                                )),
                                                "Auto",
                                                if !is_premium {
                                                    tip("This functionality requires a premium account.")
                                                } else {
                                                    Node::Empty
                                                }
                                            ]
                                        ]
                                    }
                                ] 
                            } else {
                                Vec::new()
                            },
                        
                        ]
                    }
                ]),
        ]
    ]
}

fn chat(
    model: &Model,
    state: &shared::State,
    client_state: &ClientState<shared::State>,
) -> Node<Msg> {
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
                        let username = &client_state
                            .get_user_data(&user_id)
                            .map(|data| data.username.clone())
                            .unwrap_or_default();
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

fn history(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    client_state: &ClientState<shared::State>,
) -> Node<Msg> {
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
                                span![format!(
                                    "You got a dwarf but don't have enough space for him."
                                )]
                            }
                            LogMsg::NewPlayer(user_id) => {
                                span![format!(
                                    "A new player has joined the game, say hi to {}!",
                                    client_state
                                        .get_user_data(&user_id)
                                        .map(|data| data.username.clone())
                                        .unwrap_or_default()
                                )]
                            }
                            LogMsg::MoneyForKing(money) => {
                                span![format!("You are the king and earned {} coins!", money)]
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
                                    "You completed the quest {} and earned {} coins.",
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

fn bundle(requires: &Bundle<Item>, player: &Player, requirement: bool) -> Node<Msg> {
    ul![requires
        .clone()
        .sorted_by_rarity()
        .into_iter()
        .map(|(item, n)| {
            if requirement {
                let available = player
                    .inventory
                    .items
                    .check_remove(&Bundle::new().add(item, n));
                li![
                    C!["clickable-item"],
                    span![
                        if available { C![] } else { C!["unavailable"] },
                        format!("{n}x {item}"),
                        ev(Ev::Click, move |_| Msg::GoToItem(item))
                    ],
                    span![format!(
                        " ({})",
                        player
                            .inventory
                            .items
                            .get(&item)
                            .copied()
                            .unwrap_or_default()
                    )]
                ]
            } else {
                //let available = player.inventory.items.check_remove(&Bundle::new().add(item, n));
                li![
                    C!["clickable-item"],
                    span![
                        format!("{n}x {item}"),
                        ev(Ev::Click, move |_| Msg::GoToItem(item))
                    ],
                ]
            }
        })]
}

fn icon(name: &str) -> Node<Msg> {
    span![C!["material-symbols-rounded"], name]
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

    span![v
        .into_iter()
        .map(|(num, abv)| span![format!("{abv} "), stars(num, false)])
        .intersperse(br![])]
}

fn stats_simple(stats: &Stats) -> String {
    let mut v = Vec::new();

    if stats.strength != 0 {
        v.push("Strength");
    }
    if stats.endurance != 0 {
        v.push("Endurance");
    }
    if stats.agility != 0 {
        v.push("Agility");
    }
    if stats.intelligence != 0 {
        v.push("Intelligence");
    }
    if stats.perception != 0 {
        v.push("Perception");
    }

    if v.is_empty() {
        return "no skills".to_owned();
    }

    v.into_iter().join(", ")
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
        a![
            C!["button"],
            if let Page::Base = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game"}
            } else {
                attrs! {At::Href => "/game"}
            },
            "Settlement"
        ],
        a![
            C!["button"],
            if let Page::Dwarfs(DwarfsMode::Overview) = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game/dwarfs"}
            } else {
                attrs! {At::Href => "/game/dwarfs"}
            },
            "Dwarfs",
        ],
        a![
            C!["button"],
            if let Page::Inventory(InventoryMode::Overview) = model.page {
                attrs! {At::Disabled => "true",  At::Href => "/game/inventory"}
            } else {
                attrs! {At::Href => "/game/inventory"}
            },
            "Inventory",
        ],
        a![
            C!["button"],
            if let Page::Quests = model.page {
                attrs! {At::Disabled => "true", At::Href => "/game/quests"}
            } else {
                attrs! {At::Href => "/game/quests"}
            },
            "Quests",
        ],
        a![
            C!["button"],
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
