mod images;

use engine_client::{ClientState, EventWrapper, Msg as EngineMsg};
use engine_shared::GameId;
use images::Image;
use itertools::Itertools;
use seed::{prelude::*, *};
use shared::{
    Bundle, ClientEvent, Craftable, DwarfId, Health, HireDwarfType, Item, ItemRarity, ItemType,
    LogMsg, Occupation, Player, Popup, QuestId, QuestType, RewardMode, Stats, Time,
    TutorialRequirement, TutorialReward, TutorialStep, WorldEvent, FREE_LOOT_CRATE,
    LOOT_CRATE_COST, MAX_HEALTH, SPEED, WINNER_NUM_PREMIUM_DAYS,
};
use std::str::FromStr;
use web_sys::js_sys::Date;

#[cfg(not(debug_assertions))]
const HOST: &str = "dwarfs-in-exile.com";
#[cfg(debug_assertions)]
const HOST: &str = "localhost:3000";

#[cfg(not(debug_assertions))]
const WS_PROTOCOL: &str = "wss";
#[cfg(debug_assertions)]
const WS_PROTOCOL: &str = "ws";

const REQUIRES_PREMIUM: &str = "This feature requires a premium account.";
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
    fn from_url(mut url: Url) -> (GameId, Self) {
        url.next_path_part().unwrap();
        let game_id = url.next_path_part().unwrap().parse().unwrap();
        let page = match url.next_path_part() {
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
        };

        (game_id, page)
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
    sort: InventorySort,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum InventorySort {
    Rarity,
    Usefulness(Occupation),
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
            sort: InventorySort::Rarity,
        }
    }
}

pub struct DwarfsFilter {
    occupation: Option<Occupation>,
    sort: DwarfsSort,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum DwarfsSort {
    LeastHealth,
    WorstAssigned,
    BestIn(Occupation),
}

impl Default for DwarfsFilter {
    fn default() -> Self {
        Self {
            occupation: None,
            sort: DwarfsSort::LeastHealth,
        }
    }
}

pub struct QuestsFilter {
    participating: bool,
    none_participating: bool,
}

impl Default for QuestsFilter {
    fn default() -> Self {
        Self {
            participating: false,
            none_participating: false,
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
    dwarfs_filter: DwarfsFilter,
    quests_filter: QuestsFilter,
    map_time: (Time, u64),
    game_id: GameId,
    show_tutorial: bool,
}

impl Model {
    fn sync_timestamp_millis_now(&mut self, time: Time) {
        self.map_time.0 = time;
        self.map_time.1 = Date::now() as u64;
    }

    fn get_timestamp_millis_of(&self, time: Time) -> u64 {
        (self.map_time.1 as i64 + (time as i64 - self.map_time.0 as i64) * 1000 / SPEED as i64)
            as u64
    }

    fn get_timestamp_millis_diff_now(&self, time: Time) -> u64 {
        (Date::now() as u64).saturating_sub(self.get_timestamp_millis_of(time))
    }

    fn base_path(&self) -> String {
        format!("/game/{}", self.game_id)
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
    orders.subscribe(|subs::UrlChanged(url)| Msg::ChangePage(Page::from_url(url).1));

    let (game_id, page) = Page::from_url(url);

    Model {
        state: ClientState::init(orders, format!("{WS_PROTOCOL}://{HOST}/game/{game_id}/ws")),
        page,
        message: String::new(),
        chat_visible: false,
        history_visible: false,
        inventory_filter: InventoryFilter::default(),
        dwarfs_filter: DwarfsFilter::default(),
        quests_filter: QuestsFilter::default(),
        map_time: (0, 0),
        game_id,
        show_tutorial: false,
    }
}

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
    DwarfsFilterReset,
    DwarfsFilterOccupation(Option<Occupation>),
    DwarfsFilterSort(DwarfsSort),
    QuestsFilterReset,
    QuestsFilterParticipating,
    QuestsFilterNoneParticipating,
    GoToItem(Item),
    ToggleTutorial,
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
            model.state.update(ev.clone(), orders);

            if let Some(state) = model.state.get_state() {
                if engine_shared::State::has_winner(state).is_some() {
                    orders.notify(subs::UrlRequested::new(Url::from_str("/game").unwrap()));
                }

                if let EventWrapper::ReceiveGameEvent(ev) = &ev {
                    if let engine_shared::Event::ServerEvent(shared::ServerEvent::Tick) = ev.event {
                        model.sync_timestamp_millis_now(state.time);
                    }
                }
            }
        }
        Msg::ChangePage(page) => {
            model.page = page;

            web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0);
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
        Msg::DwarfsFilterReset => {
            model.dwarfs_filter = DwarfsFilter::default();
        }
        Msg::DwarfsFilterSort(sort) => {
            model.dwarfs_filter.sort = sort;
        }
        Msg::DwarfsFilterOccupation(occupation) => {
            model.dwarfs_filter.occupation = occupation;
        }
        Msg::QuestsFilterReset => {
            model.quests_filter = QuestsFilter::default();
        }
        Msg::QuestsFilterParticipating => {
            model.quests_filter.participating = !model.quests_filter.participating;
            if model.quests_filter.participating {
                model.quests_filter.none_participating = false;
            }
        }
        Msg::QuestsFilterNoneParticipating => {
            model.quests_filter.none_participating = !model.quests_filter.none_participating;
            if model.quests_filter.none_participating {
                model.quests_filter.participating = false;
            }
        }
        Msg::ToggleTutorial => {
            model.show_tutorial = !model.show_tutorial;
        }
        Msg::AssignToQuest(quest_id, dwarf_idx, dwarf_id) => {
            if dwarf_id.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!("{}/quests/{}", model.base_path(), quest_id)).unwrap(),
                ));
            }
            orders.send_msg(Msg::send_event(ClientEvent::AssignToQuest(
                quest_id, dwarf_idx, dwarf_id,
            )));
        }
        Msg::ChangeEquipment(dwarf_id, item_type, item) => {
            if item.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!("{}/dwarfs/{}", model.base_path(), dwarf_id)).unwrap(),
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
                Url::from_str(&format!("{}/inventory", model.base_path())).unwrap(),
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
            popup(model, state, user_id),
            tutorial(model, state, user_id),
            main![match model.page {
                Page::Dwarfs(mode) => dwarfs(model, state, user_id, mode),
                Page::Dwarf(dwarf_id) => dwarf(model, state, user_id, dwarf_id),
                Page::Base => base(model, state, user_id),
                Page::Inventory(mode) => inventory(model, state, user_id, mode),
                Page::Ranking => ranking(model, state, client_state, user_id),
                Page::Quests => quests(model, state, user_id),
                Page::Quest(quest_id) => quest(model, state, user_id, quest_id),
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

fn popup(_model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if let Some(popup) = player.popups.front() {
            div![
                C!["panel-wrapper"],
                match popup {
                    Popup::NewDwarf(dwarf) => {
                        div![
                            id!["tutorial-panel"],
                            C!["panel"],
                            img![
                                C!["panel-image"],
                                attrs! { At::Src => Image::from_dwarf(dwarf).as_at_value() }
                            ],
                            div![
                                C!["panel-content"],
                                h3!["A New Dwarf has Arrived"],
                                h4![C!["title"], &dwarf.name],
                                p![
                                    C!["subtitle"],
                                    format!(
                                        "{}, {} Years old.",
                                        if dwarf.is_female { "Female" } else { "Male" },
                                        dwarf.age_years()
                                    ),
                                ],
                                p![
                                    h4!["Stats"],
                                    table![tbody![
                                        tr![th![], th!["Inherent"]],
                                        tr![
                                            th!["Strength"],
                                            td![stars(dwarf.stats.strength, true)],
                                        ],
                                        tr![
                                            th!["Endurance"],
                                            td![stars(dwarf.stats.endurance, true)],
                                        ],
                                        tr![th!["Agility"], td![stars(dwarf.stats.agility, true)],],
                                        tr![
                                            th!["Intelligence"],
                                            td![stars(dwarf.stats.intelligence, true)],
                                        ],
                                        tr![
                                            th!["Perception"],
                                            td![stars(dwarf.stats.perception, true)],
                                        ],
                                    ]]
                                ],
                                button![
                                    ev(Ev::Click, move |_| Msg::send_event(
                                        ClientEvent::ConfirmPopup
                                    )),
                                    "Confirm"
                                ],
                            ]
                        ]
                    }
                    Popup::NewItems(bundle) => {
                        let (item, qty) = bundle.iter().next().unwrap();

                        div![
                            id!["tutorial-panel"],
                            C!["panel"],
                            img![
                                C!["panel-image"],
                                attrs! { At::Src => Image::from(*item).as_at_value() }
                            ],
                            div![
                                C!["panel-content"],
                                h3![C!["title"], format!("You Received {} {}", qty, item)],
                                p![
                                    C!["subtitle"],
                                    if let Some(item_type) = item.item_type() {
                                        span![C!["short-info"], format!("{item_type}")]
                                    } else {
                                        span![C!["short-info"], "Item"]
                                    },
                                    span![C!["short-info"], format!("{}", item.item_rarity())],
                                    if let Some(nutrition) = item.nutritional_value() {
                                        span![C!["short-info"], format!("{} Food", nutrition)]
                                    } else {
                                        Node::Empty
                                    },
                                    if item.money_value() > 0 {
                                        span![
                                            C!["short-info"],
                                            format!("{} Coins", item.money_value())
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                ],
                                p![
                                    if !item.provides_stats().is_zero() {
                                        div![h4!["Provides"], stats(&item.provides_stats()),]
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
                                            itertools::intersperse(
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
                                                ),
                                                br![]
                                            )
                                        ]
                                    } else {
                                        Node::Empty
                                    },
                                ],
                                button![
                                    ev(Ev::Click, move |_| Msg::send_event(
                                        ClientEvent::ConfirmPopup
                                    )),
                                    "Confirm"
                                ],
                            ]
                        ]
                    }
                }
            ]
        } else {
            Node::Empty
        }
    } else {
        Node::Empty
    }
}

fn tutorial(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if let Some(step) = player.tutorial_step {
            if model.show_tutorial && player.popups.is_empty() {
                div![
                    C!["panel-wrapper"],
                    div![
                        id!["tutorial-panel"],
                        C!["panel"],
                        img![C!["panel-image"], attrs! { At::Src => "/logo.jpg" } ],
                        div![C!["panel-content"],
                            h3![format!("{}", step)],
                            match step {
                                TutorialStep::Welcome => div![
                                    p!["Hey, you, welcome to the forbidden lands! You just arrived here? Oh well, seems like I'm your new best friend now! No worries, I'll show you around!"]
                                ],
                                TutorialStep::Mining => div![
                                    p!["The first thing that we should do is go mining. With mining we get stones that we can use to upgrade the settlement."],
                                    p!["Go to the dwarf overview, click on a dwarf and send him mining."]
                                ],
                                TutorialStep::Logging => div![
                                    p!["Wood is very important for expanding your settlement."],
                                    p!["Go to the dwarf overview, click on a dwarf and send him logging."]
                                ],
                                TutorialStep::SettlementExpansion2 => div![
                                    p!["To expand your settlement, you need to have enough resources. You can see the requirements in the settlement overview."],
                                    p!["Go to the settlement overview and upgrade your settlement."]
                                ],
                                TutorialStep::Hunting => div![
                                    p!["Hunting is a great way to get food. Food is important to keep your dwarfs healthy."],
                                    p!["Go to the dwarf overview, click on a dwarf and send him hunting."]
                                ],
                                TutorialStep::FoodPreparation => div![
                                    p!["Food is important to keep your dwarfs alive."],
                                    p!["In the inventory, craft cooked meat and store it as food in your settlement."],
                                ],
                                TutorialStep::Idling => div![
                                    p!["Your dwarfs need to rest from time to time. If they are idling and there is enough food, they will recover their health."],
                                    p!["If a dwarfs health reaches zero, he will die."]
                                ],
                                TutorialStep::SettlementExpansion3 => div![
                                    p!["Further expand your settlement to make space for mor more dwarfs."],
                                ],
                                TutorialStep::Quests => div![
                                    p!["Quests are a great way to earn money, or get new dwarfs and items."],
                                    p!["Go to the quest overview and do quests until you get a new dwarf."]
                                ],
                                TutorialStep::SettlementExpansion5 => div![
                                    p!["Further expand your settlement to make space for more dwarfs."],
                                ],
                            },
                            h4!["Requirements"],
                            match step.requires() {
                                TutorialRequirement::Nothing => p!["No requirements."],
                                TutorialRequirement::Items(items) => div![
                                    p!["The following items need to be in your inventory:"],
                                    bundle(&items, player, true)
                                ],
                                TutorialRequirement::BaseLevel(level) => p![
                                    format!("Expand your settlement until it reaches level {} (your current level is {}).", level, player.base.curr_level)

                                ],
                                TutorialRequirement::Food(food) => p![
                                    format!("Store food until you have {} food in your settlement (you have {} food).", food, player.base.food)
                                ],
                                TutorialRequirement::AnyDwarfOccupation(occupation) => p![
                                    format!("Send any dwarf {}.", occupation)
                                ],
                                TutorialRequirement::NumberOfDwarfs(dwarfs) => p![
                                    format!("Expand your settlement until it reaches a population of {} (your current population is {}).", dwarfs, player.dwarfs.len())
                                ],
                            },
                            h4!["Rewards"],
                            match step.reward() {
                                TutorialReward::Dwarfs(num) if num == 1 => p![format!("A new dwarf")],
                                TutorialReward::Dwarfs(num) => p![format!("{num} dwarfs")],
                                TutorialReward::Items(items) => bundle(&items, player, false),
                                TutorialReward::Money(money) => p![format!("{money} coins")],
                            },
                            button![
                                attrs! { At::Disabled => (!step.requires().complete(player)).as_at_value() },
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::NextTutorialStep)),
                                "Complete Quest"
                            ],
                            button![
                                ev(Ev::Click, move |_| Msg::ToggleTutorial),
                                "Close"
                            ],
                        ]

                    ]
                ]
            } else {
                button![
                    id!["tutorial-button"],
                    C![if step.requires().complete(player) {
                        "complete"
                    } else {
                        "incomplete"
                    }],
                    ev(Ev::Click, move |_| Msg::ToggleTutorial),
                    img![attrs! { At::Src => "/logo.jpg" }],
                ]
            }
        } else {
            Node::Empty
        }
    } else {
        Node::Empty
    }
}

fn ranking(
    model: &Model,
    state: &shared::State,
    client_state: &ClientState<shared::State>,
    current_user_id: &shared::UserId,
) -> Node<Msg> {
    let mut players: Vec<_> = state
        .players
        .iter()
        .filter(|(user_id, player)| {
            player.is_active(state.time) && client_state.get_user_data(user_id).is_some()
        })
        .collect();
    players.sort_by_key(|(_, p)| -(p.base.curr_level as i64));

    div![
        C!["content"],

        div![
            C!["important"],
            strong!["The Dwarfen King"],
            div![C!["image-aside", "small"],
                img![attrs! {At::Src => Image::King.as_at_value()}],
                div![
                    if let Some(king) = state.king {
                        p![format!("All hail our King {}!", client_state.get_user_data(&king).map(|data| data.username.clone()).unwrap_or_default())]
                    } else {
                        p!["At the moment, there is no King in this world. Be the first to become the new King!"]
                    },
                ]
            ],
        ],

        h2!["Ranking"],
        p![format!("To win this game, you need to meet two conditions. First, expand your settlement until you reach level 100. Second, become the king of this world. If both conditions are met, the game will be over and you will be the winner. As a reward, you get gifted a free premium account for {} days.", WINNER_NUM_PREMIUM_DAYS)],

        table![
            tr![
                th!["Rank"],
                th!["Username"],
                th!["Level"],
            ],
            players.iter().enumerate().map(|(i, (user_id, player))| {
                let (is_premium, is_dev, is_winner) = model
                    .state
                    .get_user_data(user_id)
                    .map(|user_data| (user_data.premium > 0, user_data.admin, user_data.games_won > 0))
                    .unwrap_or((false, false, false));

                let rank = i + 1;
                let current_user = *current_user_id == **user_id;

                tr![
                    td![C![if current_user { "current-user" } else { "" }], rank],
                    td![
                        C![if current_user { "current-user" } else { "" }],
                        format!(
                            "{}",
                            client_state
                                .get_user_data(&user_id)
                                .map(|data| data.username.clone())
                                .unwrap_or_default()
                        ),
                        if is_dev {
                            span![
                                C!["nametag"],
                                "Developer"
                            ]
                        } else {
                            Node::Empty
                        },
                        if is_premium {
                            span![
                                C!["nametag"],
                                "Premium"
                            ]
                        } else {
                            Node::Empty
                        },
                        if is_winner {
                            span![
                                C!["nametag"],
                                "Winner"
                            ]
                        } else {
                            Node::Empty
                        },
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
                    td![C![if current_user { "current-user" } else { "" }], player.base.curr_level]
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
    if let Some(player) = state.players.get(user_id) {
        div![
            id!["received-item-popup"],
            player
                .inventory
                .last_received
                .iter()
                .enumerate()
                .filter_map(|(_idx, (item, qty, time))| {
                    let time_diff_millis = model.get_timestamp_millis_diff_now(*time);

                    if time_diff_millis > 3000 {
                        None
                    } else {
                        Some(div![
                            C!["received-item"],
                            style![St::Opacity => format!("{}", 1.0 - time_diff_millis as f64 / 3000.0)],
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
    } else {
        Node::Empty
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

fn score_bar(curr: u64, max: u64, rank: usize, max_rank: usize, markers: Vec<u64>) -> Node<Msg> {
    div![
        C!["score-bar-wrapper"],
        if curr > 0 {
            div![
                C!["score-bar-curr"],
                attrs! {
                    At::Style => format!("width: calc(100% / {max} * {curr});")
                }
            ]
        } else {
            Node::Empty
        },
        markers.iter().map(|marker| {
            div![
                C!["score-bar-marker"],
                attrs! {
                    At::Style => format!("width: calc(100% / {max} * {marker});")
                }
            ]
        }),
        div![
            C!["score-bar-overlay"],
            if curr == 0 {
                format!(
                    "{} / {} XP ({} users participating)",
                    big_number(curr),
                    big_number(max),
                    max_rank
                )
            } else if curr == max {
                format!(
                    "{} XP ({} place of {})",
                    big_number(curr),
                    enumerate(rank),
                    max_rank
                )
            } else {
                format!(
                    "{} / {} XP ({} place of {})",
                    big_number(curr),
                    big_number(max),
                    enumerate(rank),
                    max_rank
                )
            }
        ],
    ]
}

fn dwarfs(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    mode: DwarfsMode,
) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if player.dwarfs.len() > 0 {
            let mut dwarfs = player.dwarfs.iter().collect::<Vec<_>>();
            dwarfs.sort_by_key(|(_, dwarf)| {
                let mut sort = model.dwarfs_filter.sort;
                if let DwarfsMode::Select(DwarfsSelect::Quest(quest_id, _dwarf_idx)) = mode {
                    let occupation = state.quests.get(&quest_id).unwrap().quest_type.occupation();
                    sort = DwarfsSort::BestIn(occupation);
                }
                match sort {
                    DwarfsSort::LeastHealth => dwarf.health,
                    DwarfsSort::BestIn(occupation) => u64::MAX - dwarf.effectiveness(occupation),
                    DwarfsSort::WorstAssigned => {
                        if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
                            dwarf.effectiveness(quest_type.occupation()) + 1
                        } else if dwarf.occupation == Occupation::Idling {
                            0
                        } else {
                            dwarf.effectiveness(dwarf.occupation) + 1
                        }
                    }
                }
            });

            div![
                div![
                    C!["filter"],
                    div![
                        div![C!["button-row"],
                            /*enum_iterator::all::<Occupation>()
                                .filter(|occupation| player.base.curr_level >= occupation.unlocked_at_level())
                                .map(|occupation| {
                                    button![
                                        attrs!{ At::Disabled => (model.dwarfs_filter.occupation == Some(occupation)).as_at_value() },
                                        ev(Ev::Click, move |_| Msg::DwarfsFilterOccupation(Some(occupation))),
                                        format!("{occupation}")
                                    ]
                                }),*/
                            button![
                                attrs!{ At::Disabled => (model.dwarfs_filter.sort == DwarfsSort::LeastHealth).as_at_value() },
                                ev(Ev::Click, move |_| Msg::DwarfsFilterSort(DwarfsSort::LeastHealth)),
                                format!("Least Health")
                            ],
                            button![
                                attrs!{ At::Disabled => (model.dwarfs_filter.sort == DwarfsSort::WorstAssigned).as_at_value() },
                                ev(Ev::Click, move |_| Msg::DwarfsFilterSort(DwarfsSort::WorstAssigned)),
                                format!("Worst Performing")
                            ]
                        ]
                    ],
                    div![
                        button![
                            ev(Ev::Click, move |_| Msg::DwarfsFilterReset),
                            "Reset Filter",
                        ],
                    ]

                ],
                table![
                    C!["dwarfs", "list"],
                    dwarfs.iter().filter(|(_, dwarf)| {
                        if let DwarfsMode::Select(DwarfsSelect::Quest(quest_id, _)) = mode {
                            if let Some((_, dwarf_quest_id, _)) = dwarf.participates_in_quest {
                                if dwarf_quest_id == quest_id {
                                    return false;
                                }
                            }
                        }

                        if let Some(job) = model.dwarfs_filter.occupation {
                            dwarf.occupation == job
                        } else {
                            true
                        }
                    }).map(|(&id, dwarf)| tr![
                        C!["dwarf", format!("dwarf-{}", id)],
                        C!["list-item-row"],
                        td![img![
                            C!["list-item-image"],
                            attrs! {At::Src => Image::from_dwarf(&dwarf).as_at_value()}
                        ]],
                        td![
                            C!["list-item-content"],
                            h3![C!["title"], &dwarf.name],
                            p![
                                C!["subtitle"],
                                format!("{}, {} Years old.", if dwarf.is_female {
                                    "Female"
                                } else {
                                    "Male"
                                }, dwarf.age_years()),
                                br![],
                                if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
                                    div![
                                        if dwarf.auto_idle {
                                            format!(
                                                "Auto-idling, resuming quest {} shortly.",
                                                quest_type,
                                            )
                                        } else {
                                            format!(
                                                "Participating in quest {}.",
                                                quest_type,
                                            )
                                        },
                                        br![],
                                        if dwarf.occupation != Occupation::Idling {
                                            stars(dwarf.effectiveness(quest_type.occupation()) as i8, true)
                                        } else {
                                            Node::Empty
                                        }
                                    ]
                                } else {
                                    div![
                                        if dwarf.auto_idle {
                                            format!(
                                                "Auto-idling, resuming occupation {} shortly.",
                                                dwarf.occupation,
                                            )
                                        } else {
                                            format!(
                                                "{} since {}.",
                                                dwarf.occupation,
                                                fmt_time(dwarf.occupation_duration)
                                            )
                                        },
                                        br![],
                                        if dwarf.occupation != Occupation::Idling {
                                            stars(dwarf.effectiveness(dwarf.occupation) as i8, true)
                                        } else {
                                            Node::Empty
                                        }
                                    ]
                                },
                            ],
                            health_bar(dwarf.health, MAX_HEALTH),
                            p![match mode {
                                DwarfsMode::Overview => {
                                    a![
                                        C!["button"],
                                        attrs! { At::Href => format!("{}/dwarfs/{}", model.base_path(), id) },
                                        "Details"
                                    ]
                                }
                                DwarfsMode::Select(DwarfsSelect::Quest(quest_id, dwarf_idx)) => {
                                    div![button![
                                        if !dwarf.is_adult() {
                                            attrs! { At::Disabled => "true" }
                                        } else {
                                            attrs! {}
                                        },
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
    } else {
        Node::Empty
    }
}

fn dwarf(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    dwarf_id: DwarfId,
) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
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
                h2![C!["title"], &dwarf.name],
                div![C!["image-aside"],
                    img![attrs! {At::Src => Image::from_dwarf(&dwarf).as_at_value()}],
                    div![
                        p![C!["subtitle"],
                        format!("{}, {} Years old.", if dwarf.is_female {
                            "Female"
                        } else {
                            "Male"
                        }, dwarf.age_years()),
                        br![],
                        if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
                            div![
                                if dwarf.auto_idle {
                                    format!(
                                        "Auto-idling, resuming quest {} shortly.",
                                        quest_type,
                                    )
                                } else {
                                    format!(
                                        "Participating in quest {}.",
                                        quest_type,
                                    )
                                },
                                br![],
                                if dwarf.occupation != Occupation::Idling {
                                    stars(dwarf.effectiveness(quest_type.occupation()) as i8, true)
                                } else {
                                    Node::Empty
                                }
                            ]
                        } else {
                            div![
                                if dwarf.auto_idle {
                                    format!(
                                        "Auto-idling, resuming occupation {} shortly.",
                                        dwarf.occupation,
                                    )
                                } else {
                                    format!(
                                        "{} since {}.",
                                        dwarf.occupation,
                                        fmt_time(dwarf.occupation_duration)
                                    )
                                },
                                br![],
                                if dwarf.occupation != Occupation::Idling {
                                    stars(dwarf.effectiveness(dwarf.occupation) as i8, true)
                                } else {
                                    Node::Empty
                                }
                            ]
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
                                if player.auto_functions.auto_idle && is_premium { "Disable Auto Idling for all Dwarfs" } else { "Enable Auto Idling for all Dwarfs" },
                                if !is_premium {
                                    tip(REQUIRES_PREMIUM)
                                } else {
                                    Node::Empty
                                }
                            ]
                        ],
                        div![
                            h3!["Stats"],
                            table![tbody![
                                tr![th![], th!["Inherent"], th!["Effective"]],
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
                    ]
                ],
                div![
                    h3!["Equipment"],
                    table![C!["list"],
                        enum_iterator::all::<ItemType>().filter(ItemType::equippable).map(|item_type| {
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
                                                    itertools::Itertools::intersperse(enum_iterator::all::<Occupation>().filter_map(
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
                                                    ), br![])
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
                ],
                div![
                    C!["occupation"],
                    h3!["Work"],
                    if dwarf.is_adult() {
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
                                    tr![C!["list-item-row", if occupation == dwarf.occupation { "selected" } else { "" }],
                                        td![img![C!["list-item-image"], attrs! { At::Src => Image::from(occupation).as_at_value() } ]],
                                        td![C!["list-item-content", "grow"],
                                            h3![C!["title"],
                                                format!("{}", occupation),
                                            ],
                                            p![
                                                stars(dwarf.effectiveness(occupation) as i8, true)
                                            ],
                                            p![
                                                button![
                                                    if occupation == dwarf.occupation
                                                        || dwarf.participates_in_quest.is_some()
                                                        || !dwarf.is_adult()
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
                                })
                            ]
                        }
                    } else {
                        p!["This dwarf is too young to work."]
                    }

                ]
            ]
        } else {
            div![
                C!["content"],
                h2!["There's Noone Here!"],
                p!["This dwarf has died!"],
                a![
                    attrs! { At::Href => format!("{}/dwarfs", model.base_path()) },
                    "Go back"
                ],
            ]
        }
    } else {
        Node::Empty
    }
}

fn quests(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    //let _player = state.players.get(user_id).unwrap();

    let mut quests = state.quests.iter().collect::<Vec<_>>();
    quests.sort_by_key(|(_, quest)| quest.time_left);

    div![
        div![
            C!["filter"],
            div![
                div![
                    input![
                        id!["participating"],
                        attrs! {At::Type => "checkbox", At::Checked => model.quests_filter.participating.as_at_value()},
                        ev(Ev::Click, |_| Msg::QuestsFilterParticipating),
                    ],
                    label![attrs! {At::For => "participating"}, "Participating"]
                ],
                div![
                    input![
                        id!["none-participating"],
                        attrs! {At::Type => "checkbox", At::Checked => model.quests_filter.none_participating.as_at_value()},
                        ev(Ev::Click, |_| Msg::QuestsFilterNoneParticipating),
                    ],
                    label![attrs! {At::For => "none-participating"}, "No Participants"]
                ],
            ],
            div![
                button![
                    ev(Ev::Click, move |_| Msg::QuestsFilterReset),
                    "Reset Filter",
                ],
            ]

        ],
        table![
            C!["quests", "list"],
            quests.iter().filter(|(_, quest)| {
                (if model.quests_filter.participating {
                    quest.contestants.contains_key(user_id)
                } else {
                    true
                }) && (if model.quests_filter.none_participating {
                    quest.contestants.is_empty()
                } else {
                    true
                })
            }).map(|(quest_id, quest)| {
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
                            let mut contestants = quest.contestants.values().map(|c| c.achieved_score).collect::<Vec<_>>();
                            contestants.sort();
                            let best_score = contestants.last().copied().unwrap();
                            p![score_bar(
                                contestant.achieved_score,
                                best_score,
                                rank,
                                quest.contestants.len(),
                                contestants
                            )]
                        } else {
                            let mut contestants = quest.contestants.values().map(|c| c.achieved_score).collect::<Vec<_>>();
                            contestants.sort();
                            let best_score = contestants.last().copied().unwrap_or_default();
                            p![score_bar(0, best_score, 0, quest.contestants.len(), contestants)]
                        },
                        a![
                            C!["button"],
                            attrs! { At::Href => format!("{}/quests/{}", model.base_path(), quest_id) },
                            "Details"
                        ]
                    ]
                ]
            })
        ]
    ]
}

fn quest(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    quest_id: QuestId,
) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        let quest = state.quests.get(&quest_id);

        if let Some(quest) = quest {
            div![
                C!["content"],
                h2![C!["title"], format!("{}", quest.quest_type)],
                div![C!["image-aside"],
                    img![attrs! {At::Src => Image::from(quest.quest_type).as_at_value()}],
                    div![
                        p![C!["subtitle"], format!("{} remaining.", fmt_time(quest.time_left))],
                        if let Some(contestant) = quest.contestants.get(user_id) {
                            let rank = quest.contestants.values().filter(|c| c.achieved_score >= contestant.achieved_score).count();
                            let mut contestants = quest.contestants.values().map(|c| c.achieved_score).collect::<Vec<_>>();
                            contestants.sort();
                            let best_score = contestants.last().copied().unwrap();
                            p![
                                score_bar(contestant.achieved_score, best_score, rank, quest.contestants.len(), contestants)
                            ]
                        } else {
                            let mut contestants = quest.contestants.values().map(|c| c.achieved_score).collect::<Vec<_>>();
                            contestants.sort();
                            let best_score = contestants.last().copied().unwrap_or_default();
                            p![score_bar(
                                0,
                                best_score,
                                0,
                                quest.contestants.len(),
                                contestants
                            )]
                        },
                        p![format!("This quest requires {}.", quest.quest_type.occupation().to_string().to_lowercase())],
                        p![
                            match quest.quest_type {
                                QuestType::KillTheDragon => p!["A dragon was found high up in the mountains in the forbidden lands. Send your best warriors to defeat it."],
                                QuestType::ArenaFight => p!["The King of the dwarfs has invited the exilants to compete in an arena fight against monsters and creatures from the forbidden lands. The toughest warrior will be rewarded with a gift from the king personally."],
                                QuestType::FeastForAGuest => p!["Your village is visted by an ominous guest that tells you disturbing stories about the elves. Although the stories seem unbelievable, he still seems like wise man. Go hunting and organize a feast for the guest, and he may stay."],
                                QuestType::FreeTheVillage => p!["The elven village was raided by the orks in an attempt to capture the elven magician. Free the elven village and fight the orks to earn a reward!"],
                                QuestType::ADwarfGotLost => p!["Search for a dwarf that got lost. It is unclear why so many dwarfs have disappeared in recent times, but that is a mistery that you may uncover later. If you find the lost dwarf first, he may stay in your settlement!"],
                                QuestType::AFishingFriend => p!["Go fishing and make friends!"],
                                QuestType::ADwarfInDanger => p!["A dwarf was abducted by the orks. They didn't hurt him yet, but the elves tell you that he is in danger and needs to be freed as soon as possible. If you free him first, he may stay in your settlement!"],
                                QuestType::ForTheKing => p!["Fight a ruthless battle to become the king over all the dwarfen settlements!"],
                                QuestType::DrunkFishing => p!["Participate in the drunk fishing contest! The dwarf that is the most successful drunk fisher gets a reward."],
                                QuestType::CollapsedCave => p!["A cave has collapsed and a dwarf is trapped inside. Be the first to save is life and he will move into your settlement."],
                                QuestType::TheHiddenTreasure => p!["The first who finds the hidden treasure can keep it."],
                                QuestType::CatStuckOnATree => p!["A cat is stuck on a tree. Help her get on the ground an she will gladly follow you home."],
                                QuestType::AttackTheOrks => p!["The orc camp was sptted near the elven village in preparation for an attak. Attack them first and get a reward from the elves!"],
                                QuestType::FreeTheDwarf => p!["The dwarfen king was captured by the orks. It seems like they don't want to kill him, but instead persuade him to attack the elves instead. Of course, the dwarfs would never do such a thing! Free him and he will join your settlement."],
                                QuestType::FarmersContest => p!["Participate in the farmers contest. The best farmer gets a reward."],
                                QuestType::CrystalsForTheElves => p!["The Elves need special crystals to cast their magic. Although they don't want to tell you what they will use the crystals for, you accept the offer. Bring them some and they will reward you."],
                                QuestType::ElvenVictory => p!["The elves are winning the war against the orks. They need wood to build large fenced areas where the surviving orks will be captured."],
                                QuestType::ADarkSecret => p!["While exploring the elven regions, you find a dark secret. The elves are not what they seem to be. They have used their magic on the dwarfs to turn them into orks. It seems like the orks were never the barbaric enemies that they seemed like, they are just unfortunate dwarfen souls like you and me. A devious plan by the elves to divide and weaken the dwarfen kingdom!"],
                                QuestType::TheMassacre => p!["The elves have unleased their dark magic in a final attempt to eliminate all orks. Realizing that you have helped in this terrible act by providing the elves with the crystals needed for their dark magic, you attempt to fight the elven magicians to stop the massacre."],
                                QuestType::TheElvenWar => p!["All of the dwarfen settlements have realized their mistake and have united to fight the elves. The united dwarfen armies have to fight the elven magicians in order to restore peace in the forbidden lands. Send the best fighters that you have, or the forbidden lands will be lost forever to the elven dark magic."],
                            },
                        ],
                        h3!["Rewards"],
                        match quest.quest_type.reward_mode() {
                            RewardMode::BestGetsAll(money) => div![p![format!("The best player gets {money} coins, the rest gets nothing.")]],
                            RewardMode::SplitFairly(money) => div![p![format!("A total of {money} coins are split fairly between the players.")]],
                            RewardMode::BecomeKing => div![
                                p![format!("The best player will become the king and get one tenth of all money that is earned during his reign.")],
                            ],
                            RewardMode::BestGetsItems(items) => div![
                                p![format!("The best player will get the following items:")],
                                p![bundle(&items, player, false)]
                            ],
                            RewardMode::ItemsByChance(items) => div![
                                p![format!("The participants will have a chance to get the following items. The better your score, the better are your chances to win:")],
                                p![bundle(&items, player, false)]
                            ],
                            RewardMode::NewDwarf(num) => div![p![format!("The best participant gets {num} new dwarf for their settlement.")]],
                            RewardMode::NewDwarfByChance(num) => div![p![format!("The participants have a chance to win {num} new dwarf for their settlement. The better your score, the better are your chances to win.")]],
                        },
                    ]
                ],
                h3!["Participate"],
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
                                td![img![C!["list-item-image"], attrs! {At::Src => Image::from_dwarf(&dwarf).as_at_value()}]]
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
                                    if dwarf_id.is_some() {
                                        "Change Dwarf"
                                    } else {
                                        "Select Dwarf"
                                    }
                                ],
                                if dwarf_id.is_some() {
                                    /*button![
                                        ev(Ev::Click, move |_| Msg::AssignToQuest(quest_id, dwarf_idx, None)),
                                        "Remove Dwarf"
                                    ]*/
                                    a![
                                        C!["button"],
                                        attrs! { At::Href => format!("{}/dwarfs/{}", model.base_path(), dwarf_id.unwrap()) },
                                        "Dwarf Details"
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
                a![
                    attrs! { At::Href => format!("{}/quests", model.base_path()) },
                    "Go back"
                ],
            ]
        }
    } else {
        Node::Empty
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

enum Unlock {
    Item(Item),
    Occupation(Occupation),
    MaxPopulation(u64),
}

impl Unlock {
    fn unlocked_at_level(&self) -> u64 {
        match self {
            Unlock::Item(item) => item.unlocked_at_level(),
            Unlock::Occupation(occupation) => occupation.unlocked_at_level(),
            Unlock::MaxPopulation(level) => *level,
        }
    }
}

fn base(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        let is_premium = model
            .state
            .get_user_data(user_id)
            .map(|user_data| user_data.premium > 0)
            .unwrap_or(false);

        let premium_hours = model
            .state
            .get_user_data(user_id)
            .map(|user_data| user_data.premium)
            .unwrap_or(0);

        let mut unlocks = (1..100)
            .map(Unlock::MaxPopulation)
            .chain(enum_iterator::all::<Occupation>().map(Unlock::Occupation))
            .chain(enum_iterator::all::<Item>().map(Unlock::Item))
            .collect::<Vec<_>>();

        unlocks.sort_by_key(|item| item.unlocked_at_level());
        unlocks.retain(|item| item.unlocked_at_level() > player.base.curr_level);

        div![C!["content"],
            if premium_hours <= 24 {
                div![
                    C!["important"],
                    if premium_hours == 0 {
                        strong![format!("Premium Account Expired")]
                    } else {
                        strong![format!("Premium Account Expires Soon")]
                    },
                    div![
                        C!["image-aside", "small"],
                        img![attrs! {At::Src => "/premium.jpg"}],
                        div![
                            if premium_hours == 0 {
                                p![format!("Your premium account has expired. If you want to upgrade to a premium account, you can purchase it in the store.")]
                            } else {
                                p![format!("Your premium account will expire in {premium_hours} hours. If you want to keep your premium account, you can extend it by purchasing more premium time in the store.")]
                            },
                            a![
                                C!["button"],
                                attrs! { At::Href => format!("/store") },
                                "Visit Store"
                            ]
                        ]
                    ]
                ]
            } else {
                Node::Empty
            },

            if let Some(event) = state.event {
                div![
                    C!["important"],
                    strong![format!("Current Event: {}", event)],
                    div![
                        C!["image-aside", "small"],
                        img![attrs! {At::Src => Image::from(event).as_at_value()}],
                        div![
                            match event {
                                WorldEvent::Drought => p!["There is a drought happening. The drought makes it harder to farm, gather and hunt. Make sure that your dwarfs don't starve!"],
                                WorldEvent::Flood => p!["A flood has occurred. The flood makes it harder to farm, gather and fish. Make sure that your dwarfs don't starve!"],
                                WorldEvent::Earthquake => p!["An earthquake has occurred. The earthquake makes it harder to mine, rockhound and log. Be aware of your resource production!"],
                                WorldEvent::Plague => p!["A plague is happening. During a plague, the health of your dwarfs decreases much faster. Settlements with more dwarfs are affected more than settlements with fewer dwarfs. Make sure that your dwarfs don't die!"],
                            }
                        ]
                    ]
                ]
            } else {
                Node::Empty
            },

            h2!["Your Settlement"],
            table![
                tr![th!["Level"], td![format!("{}", player.base.curr_level)]],
                tr![th!["Population"], td![format!("{}/{}", player.dwarfs.len(), player.base.max_dwarfs())]],
                tr![th!["Money"], td![format!("{} coins", player.money)]],
                tr![th!["Food"], td![format!("{} food", player.base.food)]],
            ],

            h3!["Upgrade Settlement"],
            div![C!["image-aside"],
                img![attrs! {At::Src => Image::from(player.base.village_type()).as_at_value()}],
                if let Some(requires) = player.base.upgrade_cost() {
                    div![
                        p!["Upgrade your settlement to increase the maximum population and unlock new occupations for your dwarfs. New dwarfs can be collected by doing quests, or they can simply wander into to your settlement from time to time. Dwarfs can also be hired by using coins."],
                        h4!["Next Unlocks"],
                        div![
                            C!["next-unlocks"],
                            unlocks.iter().map(|unlock| {
                                div![C!["next-unlock", if unlock.unlocked_at_level() == player.base.curr_level + 1 { "next" } else { "future" }],
                                    img![attrs! {At::Src => match unlock {
                                        Unlock::Item(item) => Image::from(*item).as_at_value(),
                                        Unlock::Occupation(occupation) => Image::from(*occupation).as_at_value(),
                                        Unlock::MaxPopulation(level) => Image::from_dwarf_str(&format!("{}", level)).as_at_value(),
                                    }}],
                                    p![strong![match unlock {
                                        Unlock::Item(item) => format!("{}", item),
                                        Unlock::Occupation(occupation) => format!("{}", occupation),
                                        Unlock::MaxPopulation(_) => format!("+1 Maximum Population"),
                                    }], br![], format!("Unlocked at level {}", unlock.unlocked_at_level())]
                                ]
                            }),
                        ],
                        h4!["Requires"],
                        bundle(&requires, player, true),
                        if player.base.build_time > 0 {
                            button![
                                attrs! {At::Disabled => "true"},
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::UpgradeBase)),
                                format!("Upgrading ({} remaining)", fmt_time(player.base.build_time))
                            ]
                        } else {
                            button![
                                if player.inventory.items.check_remove(&requires) {
                                    attrs! {}
                                } else {
                                    attrs! {At::Disabled => "true"}
                                },
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::UpgradeBase)),
                                "Upgrade",
                            ]
                        }

                    ]
                } else {
                    Node::Empty
                },
            ],

            div![
                h3!["Open Loot Crate"],
                div![C!["image-aside", "small"],
                    img![attrs! {At::Src => Image::LootCrate.as_at_value()}],
                    div![
                        p![format!("A loot crate contains a random item. You can earn loot crates by completing quests. You can also get a loot crate every {} for free.", fmt_time(FREE_LOOT_CRATE))],
                        button![
                            if player.reward_time <= state.time {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::send_event(ClientEvent::OpenDailyReward)),
                            if player.reward_time <= state.time {
                                format!("Open Free Loot Crate")
                            } else {
                                format!("Open Free Loot Crate (available in {})", fmt_time(player.reward_time - state.time))
                            },
                        ],
                        button![
                            if player.money >= LOOT_CRATE_COST {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::send_event(ClientEvent::OpenLootCrate)),
                            format!("Buy and Open ({} coins)", LOOT_CRATE_COST),
                        ]
                    ]
                ]
            ],
            div![
                h3!["Hire Dwarf"],
                div![C!["image-aside", "small"],
                    img![attrs! {At::Src => Image::HireDwarf.as_at_value()}],
                    div![
                        p!["Hire a dwarf to work for you in exchange for money."],
                        enum_iterator::all::<HireDwarfType>()
                        .map(|dwarf_type| {
                            button![
                                if player.money >= dwarf_type.cost() && player.dwarfs.len() < player.base.max_dwarfs() {
                                    attrs! {}
                                } else {
                                    attrs! {At::Disabled => "true"}
                                },
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::HireDwarf(dwarf_type))),
                                format!("Hire Dwarf ({} coins)", dwarf_type.cost()),
                            ]
                        })
                    ],
                ]
            ],
            div![
                h3!["Dwarfen Manager"],
                div![C!["image-aside", "small"],
                    img![attrs! {At::Src => Image::Manager.as_at_value()}],
                    div![
                        p!["The dwarfen manager can optimally assign dwarfs to carry out the occupations that are best suited for them. Furthermore, the manager can also assign the optimal equipment to each dwarf to further increase their effectiveness in their occupation."],
                        table![
                            tr![
                                th!["Occupation"],
                                th!["Number of Dwarfs to Assign"],
                            ],
                            enum_iterator::all::<Occupation>()
                                .filter(|occupation| player.base.curr_level >= occupation.unlocked_at_level())
                                .map(|occupation| {
                                    tr![
                                        td![format!("{}", occupation)],
                                        td![input![
                                            attrs! {
                                                At::Type => "number",
                                                At::Min => "0",
                                                At::Step => "1",
                                                At::Value => format!("{}", player.manager.get(&occupation).copied().unwrap_or(0)),
                                                At::Disabled => (matches!(occupation, Occupation::Idling) || !is_premium).as_at_value(),
                                            },
                                            input_ev(Ev::Input, move |str| {
                                                Msg::send_event(ClientEvent::SetManagerOccupation(occupation, str.parse().unwrap_or(0)))
                                            })
                                        ]],
                                    ]
                                }),
                        ],
                        button![
                            if is_premium {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::send_event(ClientEvent::OptimizeOccupations)),
                            format!("Reassign Occupations"),
                            if !is_premium {
                                tip(REQUIRES_PREMIUM)
                            } else {
                                Node::Empty
                            }
                        ],
                        button![
                            if is_premium {
                                attrs! {}
                            } else {
                                attrs! {At::Disabled => "true"}
                            },
                            ev(Ev::Click, move |_| Msg::send_event(ClientEvent::OptimizeEquipment)),
                            format!("Reassign Equipment"),
                            if !is_premium {
                                tip(REQUIRES_PREMIUM)
                            } else {
                                Node::Empty
                            }
                        ],
                    ]
                ],
            ],
        ]
    } else {
        Node::Empty
    }
}

fn inventory(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    mode: InventoryMode,
) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
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
                C!["filter"],
                div![
                    C!["no-shrink"],
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
                    C!["no-shrink"],
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
                {
                    let mut sort = model.inventory_filter.sort;
                    if let InventoryMode::Select(InventorySelect::Equipment(dwarf_id, _item_type)) =
                        mode
                    {
                        let occupation = player.dwarfs.get(&dwarf_id).unwrap().actual_occupation();
                        sort = InventorySort::Usefulness(occupation);
                    }
                    match sort {
                        InventorySort::Rarity => items.sorted_by_rarity(),
                        InventorySort::Usefulness(occupation) => {
                            items.sorted_by_usefulness(occupation)
                        }
                    }
                }
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
                            if let Some((level, requires)) = item.requires() {
                                player.inventory.items.check_remove(&requires)
                                    && player.base.curr_level >= level
                            } else {
                                false
                            }
                        } else {
                            false
                        }) || (!model.inventory_filter.owned
                            && !model.inventory_filter.craftable))
                        && ((if model.inventory_filter.food {
                            item.item_type() == Some(ItemType::Food)
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
                    td![img![
                        C!["list-item-image"],
                        attrs! {At::Src => Image::from(item).as_at_value()}
                    ]],
                    td![
                        C!["list-item-content", "grow"],
                        h3![C!["title"], format!("{} {item}", big_number(n))],
                        p![
                            C!["subtitle"],
                            if let Some(item_type) = item.item_type() {
                                span![C!["short-info"], format!("{item_type}")]
                            } else {
                                span![C!["short-info"], "Item"]
                            },
                            span![C!["short-info"], format!("{}", item.item_rarity())],
                            if let Some(nutrition) = item.nutritional_value() {
                                span![C!["short-info"], format!("{} Food", nutrition)]
                            } else {
                                Node::Empty
                            },
                            if item.money_value() > 0 {
                                span![C!["short-info"], format!("{} Coins", item.money_value())]
                            } else {
                                Node::Empty
                            },
                            if cfg!(debug_assertions) {
                                vec![
                                    span![
                                        C!["short-info"],
                                        format!("Rarity: {}", item.item_rarity_num())
                                    ],
                                    span![
                                        C!["short-info"],
                                        format!(
                                            "Loot Crate QTY: {}",
                                            (10000 / item.item_rarity_num()).max(1).min(100)
                                        )
                                    ],
                                ]
                            } else {
                                Vec::new()
                            },
                        ],
                        // Show stats
                        if !item.provides_stats().is_zero() {
                            div![h4!["Provides"], stats(&item.provides_stats()),]
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
                                itertools::intersperse(
                                    enum_iterator::all::<Occupation>().filter_map(|occupation| {
                                        let usefulness = item.usefulness_for(occupation) as i8;
                                        if usefulness > 0 {
                                            Some(span![
                                                format!("{} ", occupation),
                                                stars(usefulness, true)
                                            ])
                                        } else {
                                            None
                                        }
                                    }),
                                    br![]
                                )
                            ]
                        } else {
                            Node::Empty
                        },
                    ],
                    if let InventoryMode::Select(InventorySelect::Equipment(dwarf_id, item_type)) =
                        mode
                    {
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
                            if let Some((level, requires)) = item.requires() {
                                vec![
                                    h4!["Crafting"],
                                    bundle(&requires, player, true),
                                    if player.base.curr_level >= level {
                                        if player.auto_functions.auto_craft.contains(&item)
                                            && is_premium
                                        {
                                            button![
                                                ev(Ev::Click, move |_| Msg::send_event(
                                                    ClientEvent::ToggleAutoCraft(item)
                                                )),
                                                "Disable Auto",
                                            ]
                                        } else {
                                            div![
                                                C!["button-row"],
                                                button![
                                                    if player
                                                        .inventory
                                                        .items
                                                        .check_remove(&requires)
                                                    {
                                                        attrs! {}
                                                    } else {
                                                        attrs! {At::Disabled => "true"}
                                                    },
                                                    ev(Ev::Click, move |_| Msg::send_event(
                                                        ClientEvent::Craft(item, 1)
                                                    )),
                                                    "1x",
                                                ],
                                                button![
                                                    if player
                                                        .inventory
                                                        .items
                                                        .check_remove(&requires.clone().mul(10))
                                                    {
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
                                                    if player
                                                        .inventory
                                                        .items
                                                        .check_remove(&requires.clone().mul(100))
                                                    {
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
                                                        tip(REQUIRES_PREMIUM)
                                                    } else {
                                                        Node::Empty
                                                    }
                                                ]
                                            ]
                                        }
                                    } else {
                                        p!["Unlocked at level ", level]
                                    },
                                ]
                            } else {
                                Vec::new()
                            },
                            if let Some(_) = item.nutritional_value() {
                                vec![
                                    h4!["Food Storage"],
                                    if player.auto_functions.auto_store.contains(&item)
                                        && is_premium
                                    {
                                        button![
                                            ev(Ev::Click, move |_| Msg::send_event(
                                                ClientEvent::ToggleAutoStore(item)
                                            )),
                                            "Disable Auto"
                                        ]
                                    } else {
                                        div![
                                            C!["button-row"],
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
                                                format!("1x"),
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
                                                    tip(REQUIRES_PREMIUM)
                                                } else {
                                                    Node::Empty
                                                }
                                            ]
                                        ]
                                    },
                                ]
                            } else {
                                Vec::new()
                            },
                            if item.money_value() > 0 {
                                vec![
                                    h4!["Sell Item"],
                                    if player.auto_functions.auto_store.contains(&item)
                                        && is_premium
                                    {
                                        button![
                                            ev(Ev::Click, move |_| Msg::send_event(
                                                ClientEvent::ToggleAutoSell(item)
                                            )),
                                            "Disable Auto"
                                        ]
                                    } else {
                                        div![
                                            C!["button-row"],
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
                                                    ClientEvent::Sell(item, 1)
                                                )),
                                                format!("1x"),
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
                                                    ClientEvent::Sell(item, 10)
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
                                                    ClientEvent::Sell(item, 100)
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
                                                    ClientEvent::ToggleAutoSell(item)
                                                )),
                                                "Auto",
                                                if !is_premium {
                                                    tip(REQUIRES_PREMIUM)
                                                } else {
                                                    Node::Empty
                                                }
                                            ]
                                        ]
                                    },
                                ]
                            } else {
                                Vec::new()
                            },
                        ]
                    }
                ]),
            ]
        ]
    } else {
        Node::Empty
    }
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
                    state.chat.messages.iter().map(|(user_id, message, time)| {
                        let username = &client_state
                            .get_user_data(&user_id)
                            .map(|data| data.username.clone())
                            .unwrap_or_default();
                        div![
                            C!["message"],
                            span![C!["time"], format!("{} ago, ", fmt_time(state.time - time))],
                            span![C!["username"], format!("{username}:")],
                            span![C!["message"], format!("{message}")]
                        ]
                    }),
                ],
                div![
                    input![
                        id!["chat-input"],
                        attrs! {At::Type => "text", At::Value => model.message, At::Placeholder => "Type your message here ..."},
                        input_ev(Ev::Input, Msg::ChangeMessage)
                    ],
                    button![
                        id!["chat-submit"],
                        if message.is_empty() {
                            attrs! {At::Disabled => "true"}
                        } else {
                            attrs! {}
                        },
                        ev(Ev::Click, move |_| Msg::SubmitMessage),
                        "Send",
                    ]
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
    if let Some(player) = state.players.get(user_id) {
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
                                LogMsg::DwarfUpgrade(name, stat) => {
                                    span![format!(
                                        "Your dwarf {} has inproved his {} stat while working.",
                                        name, stat
                                    )]
                                }
                                LogMsg::DwarfIsAdult(name) => {
                                    span![format!(
                                        "Your dwarf {} is now an adult.",
                                        name
                                    )]
                                }
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
                                LogMsg::NewDwarf(name) => {
                                    span![format!(
                                        "Your settlement got a new dwarf {}.",
                                        name
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
                                            "You completed the quest {} and can start a new settlement.",
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
                                            .map(|(item, n)| format!("{n} {item}"))
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
    } else {
        Node::Empty
    }
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

    span![itertools::intersperse(
        v.into_iter()
            .map(|(num, abv)| span![format!("{abv} "), stars(num, false)]),
        br![]
    )]
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
    let mut s = Vec::new();
    if stars < 0 {
        s.push(span!["-"]);
    }
    for _ in 0..(stars.abs() / 2) {
        s.push(icon_filled("star_rate"));
    }
    if stars.abs() % 2 == 1 {
        s.push(if padded {
            icon_filled("star_rate_half")
        } else {
            icon_filled("star_rate_half")
        });
    }
    if padded {
        for _ in 0..((10 - stars.abs()) / 2) {
            s.push(icon_outlined("star_rate"));
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

    nav![
        a![C!["button"], attrs! {At::Href => "/"}, "Home"],
        a![
            C![
                "button",
                if let Page::Base = model.page {
                    "active"
                } else {
                    ""
                }
            ],
            attrs! {At::Href => model.base_path()},
            "Settlement"
        ],
        a![
            C![
                "button",
                if let Page::Dwarfs(DwarfsMode::Overview) = model.page {
                    "active"
                } else {
                    ""
                }
            ],
            attrs! {At::Href => format!("{}/dwarfs", model.base_path())},
            "Dwarfs",
        ],
        a![
            C![
                "button",
                if let Page::Inventory(InventoryMode::Overview) = model.page {
                    "active"
                } else {
                    ""
                }
            ],
            attrs! {At::Href => format!("{}/inventory", model.base_path())},
            "Inventory",
        ],
        a![
            C![
                "button",
                if let Page::Quests = model.page {
                    "active"
                } else {
                    ""
                }
            ],
            attrs! {At::Href => format!("{}/quests", model.base_path())},
            "Quests",
        ],
        a![
            C![
                "button",
                if let Page::Ranking = model.page {
                    "active"
                } else {
                    ""
                }
            ],
            attrs! {At::Href => format!("{}/ranking", model.base_path())},
            "Ranking",
        ],
        //a![C!["button"], attrs! { At::Href => "/account"}, "Account"]
    ]
}

fn tip<T: std::fmt::Display>(text: T) -> Node<Msg> {
    div![
        C!["tooltip"],
        icon_outlined("info"),
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

fn icon_outlined(name: &str) -> Node<Msg> {
    span![C!["material-symbols-outlined", "outlined"], name]
}

fn icon_filled(name: &str) -> Node<Msg> {
    span![C!["material-symbols-outlined", "filled"], name]
}
