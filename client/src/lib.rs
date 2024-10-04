mod images;

use engine_client::{ClientState, EventWrapper, Msg as EngineMsg};
use engine_shared::{utils::custom_map::CustomMap, GameId};
use images::Image;
use itertools::Itertools;
use rand::RngCore;
use rustrict::CensorStr;
use seed::{prelude::*, *};
use shared::{
    Bundle, ClientEvent, Craftable, Dwarf, DwarfId, Health, Item, ItemRarity, ItemType, LogMsg, Occupation, Player, Popup, QuestId, QuestType, RewardMode, RewardType, Stats, Territory, Time, TradeType, TribeId, TutorialRequirement, TutorialReward, TutorialStep, UserId, WorldEvent, DISMANTLING_DIVIDER, JOIN_TRIBE_LEVEL, MAX_EFFECTIVENESS, MAX_HEALTH, SPEED, TRADE_MONEY_MULTIPLIER, WINNER_NUM_PREMIUM_DAYS, WINNER_TRIBE_NUM_PREMIUM_DAYS
};
use std::str::FromStr;
use strum::Display;
use time::{macros::datetime, Duration};
use web_sys::js_sys::Date;

//const ENTER_KEY: u32 = 13;
//const ESC_KEY: u32 = 27;

#[derive(Clone, Copy, Display)]
#[allow(unused)]
enum Icon {
    Coins,
    Food,
    StarFull,
    StarHalf,
    StarEmpty,
    Person,
    PersonRemove,
    PersonAdd,
    PersonAddDisabled,
    WavingHand,
    Task,
    Inventory,
    Info,
    Trade,
    Settlement,
    Dwarfs,
    Ranking,
    Account,
    History,
    HistoryUnread,
    Chat,
    ChatUnread,
    Manager,
    Tribe,
}

impl Icon {
    fn identifier(&self) -> &str {
        match self {
            Icon::Coins => "paid",
            Icon::Food => "restaurant",
            Icon::StarFull => "star_rate",
            Icon::StarHalf => "star_rate_half",
            Icon::StarEmpty => "star_rate",
            Icon::Person => "person",
            Icon::PersonRemove => "person_remove",
            Icon::WavingHand => "waving_hand",
            Icon::PersonAdd => "person_add",
            Icon::Task => "task_alt",
            Icon::Inventory => "inventory_2",
            Icon::PersonAddDisabled => "person_add_disabled",
            Icon::Info => "info",
            Icon::Trade => "storefront",
            Icon::Settlement => "holiday_village",
            Icon::Dwarfs => "groups",
            Icon::Ranking => "social_leaderboard",
            Icon::Account => "account_circle",
            Icon::History => "notifications",
            Icon::HistoryUnread => "notifications_unread",
            Icon::Chat => "chat_bubble",
            Icon::ChatUnread => "mark_chat_unread",
            Icon::Manager => "history_edu",
            Icon::Tribe => "handshake",
        }
    }

    fn filled(&self) -> bool {
        match self {
            Icon::StarFull => true,
            _ => false,
        }
    }

    fn draw(&self) -> Node<Msg> {
        span![
            attrs! {At::Alt => format!("{self}")},
            C![
                "material-symbols-outlined",
                if self.filled() { "filled" } else { "outlined" }
            ],
            self.identifier()
        ]
    }
}

#[cfg(not(debug_assertions))]
const HOST: &str = "dwarfs-in-exile.com";
#[cfg(debug_assertions)]
const HOST: &str = "localhost:3000";

#[cfg(not(debug_assertions))]
const WS_PROTOCOL: &str = "wss";
#[cfg(debug_assertions)]
const WS_PROTOCOL: &str = "ws";

//const REQUIRES_PREMIUM: &str = "This feature requires a premium account.";

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
    Trading,
    Manager,
    Tribe,
    Visit(Option<UserId>)
}

impl Page {
    fn from_url(mut url: Url) -> (GameId, Self) {
        url.next_path_part().unwrap();
        let game_id = url.next_path_part().unwrap().parse().unwrap();
        let page = match url.next_path_part() {
            Some("visit") => Page::Visit(url.next_path_part().map(|id| UserId(id.parse().unwrap()))),
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
            Some("trading") => Page::Trading,
            Some("manager") => Page::Manager,
            Some("tribe") => Page::Tribe,
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
    Mentor(DwarfId),
    Apprentice(DwarfId),
}

pub struct InventoryFilter {
    item_name: String,
    craftable: bool,
    owned: bool,
    sort: InventorySort,
    auto: bool,
    by_type: CustomMap<ItemType, bool>,
}

pub struct TradeFilter {
    can_afford: bool,
    craftable: bool,
    my_bids: bool,
    by_type: CustomMap<ItemType, bool>,
}

impl Default for TradeFilter {
    fn default() -> Self {
        Self {
            can_afford: true,
            my_bids: true,
            craftable: false,
            by_type: CustomMap::new(),
        }
    }
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
            owned: true,
            sort: InventorySort::Rarity,
            auto: false,
            by_type: CustomMap::new(),
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

#[derive(Default)]
pub struct QuestsFilter {
    participating: bool,
    none_participating: bool,
}


#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum SliderType {
    Craft,
    Sell,
    Store,
    Dismantle,
}

pub struct Model {
    state: ClientState<shared::State>,
    page: Page,
    message: String,
    chat_visible: bool,
    history_visible: bool,
    inventory_filter: InventoryFilter,
    trade_filter: TradeFilter,
    dwarfs_filter: DwarfsFilter,
    quests_filter: QuestsFilter,
    map_time: (Time, u64),
    game_id: GameId,
    show_tutorial: bool,
    custom_name: Option<String>,
    ad_loaded: bool,
    confirm: Option<ClientEvent>,
    slider: CustomMap<(Item, SliderType), u64>,
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
    orders.subscribe(|subs::UrlRequested(url, url_request)| {
        println!("url path {:?}", url.path());

        if url.path().first().map(|s| s.as_str()) == Some("game") {
            url_request.unhandled()
        } else {
            url_request.handled()
        }
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
        trade_filter: TradeFilter::default(),
        map_time: (0, 0),
        game_id,
        show_tutorial: false,
        custom_name: None,
        ad_loaded: false,
        confirm: None,
        slider: CustomMap::new(),
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    GameStateEvent(EventWrapper<shared::State>),
    ChangePage(Page),
    ChangeMessage(String),
    SubmitMessage,
    ToggleChat,
    ToggleHistory,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    AssignToQuest(QuestId, usize, Option<DwarfId>),
    AssignMentor(DwarfId, Option<DwarfId>),
    AssignApprentice(DwarfId, Option<DwarfId>),
    InventoryFilterOwned,
    InventoryFilterCraftable,
    InventoryFilterByType(ItemType),
    InventoryFilterAuto,
    InventoryFilterName(String),
    InventoryFilterReset,
    TradeFilterCanAfford,
    TradeFilterMyBids,
    TradeFilterCraftable,
    TradeFilterReset,
    TradeFilterByType(ItemType),
    DwarfsFilterReset,
    DwarfsFilterOccupation(Option<Occupation>),
    DwarfsFilterSort(DwarfsSort),
    QuestsFilterReset,
    QuestsFilterParticipating,
    QuestsFilterNoneParticipating,
    GoToItem(Item),
    ToggleTutorial,
    UpdateName(Option<String>),
    SetName(DwarfId, Option<String>),
    AdLoaded,
    Confirm(ClientEvent),
    ConfirmYes,
    ConfirmNo,
    SetSlider(Item, SliderType, u64),
}

impl EngineMsg<shared::State> for Msg {}

impl From<EventWrapper<shared::State>> for Msg {
    fn from(event: EventWrapper<shared::State>) -> Self {
        Self::GameStateEvent(event)
    }
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::SetSlider(item, ty, value) => {
            model.slider.insert((item, ty), value);
        }
        Msg::Confirm(ev) => {
            model.confirm = Some(ev);
        }
        Msg::ConfirmYes => {
            if let Some(ev) = model.confirm.take() {
                orders.send_msg(Msg::send_event(ev));
            }
        }
        Msg::ConfirmNo => {
            model.confirm = None;
        }
        Msg::AdLoaded => {
            model.ad_loaded = true;
        }
        Msg::UpdateName(name) => {
            model.custom_name = name;
        }
        Msg::SetName(dwarf_id, name) => {
            orders.send_msg(Msg::send_event(ClientEvent::SetDwarfName(
                dwarf_id,
                name.unwrap_or_default(),
            )));
            model.custom_name = None;
        }
        Msg::GameStateEvent(ev) => {
            model.state.update(ev.clone(), orders);

            if let Some(state) = model.state.get_state() {
                if !model.ad_loaded {
                    /*
                    let is_premium = model
                        .state
                        .get_user_data(model.state.get_user_id().unwrap())
                        .map(|user_data| user_data.premium > 0)
                        .unwrap_or(false);

                    if !is_premium {
                        js_sys::eval(r#"
                            if (window.isMobile()) {
                                (function(d,z,s){s.src='https://'+d+'/401/'+z;try{(document.body||document.documentElement).appendChild(s)}catch(e){}})('aistekso.net',7962474,document.createElement('script'));
                            } else {
                                (function(d,z,s){s.src='https://'+d+'/400/'+z;try{(document.body||document.documentElement).appendChild(s)}catch(e){}})('dicouksa.com',7962656,document.createElement('script'));
                            }
                        "#).ok();
                    }
                    */

                    orders.send_msg(Msg::AdLoaded);
                }

                if engine_shared::State::closed(state) {
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
            model.custom_name = None;
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
            if model.chat_visible {
                model.history_visible = false;
            }
            orders.send_msg(Msg::send_event(ClientEvent::ReadChat));
        }
        Msg::ToggleHistory => {
            model.history_visible = !model.history_visible;
            if model.history_visible {
                model.chat_visible = false;
            }
            orders.send_msg(Msg::send_event(ClientEvent::ReadLog));
        }
        Msg::InventoryFilterByType(item_type) => {
            let old_value = model
                .inventory_filter
                .by_type
                .get(&item_type)
                .copied()
                .unwrap_or(false);
            model.inventory_filter.by_type.insert(item_type, !old_value);
        }
        Msg::InventoryFilterAuto => {
            model.inventory_filter.auto = !model.inventory_filter.auto;
            model.inventory_filter.owned = false;
            model.inventory_filter.craftable = false;
        }
        Msg::InventoryFilterOwned => {
            model.inventory_filter.owned = !model.inventory_filter.owned;
        }
        Msg::InventoryFilterCraftable => {
            model.inventory_filter.craftable = !model.inventory_filter.craftable;
        }
        Msg::InventoryFilterName(item_name) => {
            model.inventory_filter.item_name = item_name;
        }
        Msg::InventoryFilterReset => {
            model.inventory_filter = InventoryFilter::default();
        }
        Msg::TradeFilterByType(item_type) => {
            let old_value = model
                .trade_filter
                .by_type
                .get(&item_type)
                .copied()
                .unwrap_or(false);
            model.trade_filter.by_type.insert(item_type, !old_value);
        }
        Msg::TradeFilterCanAfford => {
            model.trade_filter.can_afford = !model.trade_filter.can_afford;
        }
        Msg::TradeFilterMyBids => {
            model.trade_filter.my_bids = !model.trade_filter.my_bids;
        }
        Msg::TradeFilterCraftable => {
            model.trade_filter.craftable = !model.trade_filter.craftable;
        }
        Msg::TradeFilterReset => {
            model.trade_filter = TradeFilter::default();
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
        Msg::AssignMentor(dwarf_id, mentor_id) => {
            if mentor_id.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!("{}/dwarfs/{}", model.base_path(), dwarf_id)).unwrap(),
                ));
            }
            orders.send_msg(Msg::send_event(ClientEvent::SetMentor(dwarf_id, mentor_id)));
        }
        Msg::AssignApprentice(dwarf_id, mentor_id) => {
            if mentor_id.is_some() {
                orders.notify(subs::UrlRequested::new(
                    Url::from_str(&format!(
                        "{}/dwarfs/{}",
                        model.base_path(),
                        mentor_id.unwrap()
                    ))
                    .unwrap(),
                ));
            }
            orders.send_msg(Msg::send_event(ClientEvent::SetMentor(dwarf_id, mentor_id)));
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

fn view(model: &Model) -> Node<Msg> {
    if let (Some(state), Some(user_id), client_state) = (
        model.state.get_state(),
        model.state.get_user_id(),
        &model.state,
    ) {
        let inert = state
            .players
            .get(user_id)
            .map(|player| !player.popups.is_empty())
            .unwrap_or(false)
            || model.show_tutorial;
        div![
            confirm(model, state, user_id),
            popup(model, state, user_id),
            tutorial(model, state, user_id),
            div![
                if inert {
                    attrs! { "inert" => "true" }
                } else {
                    attrs! {}
                },
                /*div![id!["background"]],
                header![
                    h1![a![attrs! { At::Href => "/" }, "Dwarfs in Exile"]],
                    //a![C!["button"], id!["home-button"], attrs! {At::Href => "/account"}, icon_outlined("account_circle")],
                ],*/
                nav(model),
                main![match model.page {
                    Page::Visit(visit_id) => dwarfs(model, state, user_id, DwarfsMode::Overview, visit_id),
                    Page::Dwarfs(mode) => dwarfs(model, state, user_id, mode, None),
                    Page::Dwarf(dwarf_id) => dwarf(model, state, user_id, dwarf_id),
                    Page::Base => base(model, state, user_id),
                    Page::Inventory(mode) => inventory(model, state, user_id, mode),
                    Page::Ranking => ranking(model, state, client_state, user_id),
                    Page::Quests => quests(model, state, user_id),
                    Page::Quest(quest_id) => quest(model, state, user_id, quest_id),
                    Page::Trading => trades(model, state, user_id),
                    Page::Manager => manager(model, state, user_id),
                    Page::Tribe => tribe(model, client_state, state, user_id),

                }],
                chat(model, state, user_id, client_state),
                history(model, state, user_id, client_state),
                last_received_items(model, state, user_id),
            ]
        ]
    } else {
        div![
            /*div![id!["background"]],
            header![h1![a![attrs! { At::Href => "/" }, "Dfnwarfs in Exile"]]],*/
            div![C!["loading"], "Loading ..."],
        ]
    }
}

fn popup(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if let Some(popup) = player.popups.front() {
            if model.confirm.is_none() {
                div![
                    attrs! { At::Role => "dialog", At::AriaLabelledBy => "popup-title", "aria-modal" => "true" },
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
                                    h3![id!["popup-title"], "A New Dwarf has Arrived"],
                                    h4![C!["title"], dwarf.actual_name()],
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
                                            tr![
                                                th!["Agility"],
                                                td![stars(dwarf.stats.agility, true)],
                                            ],
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
                                                            let usefulness = item
                                                                .usefulness_for(occupation)
                                                                as i8;
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
    } else {
        Node::Empty
    }
}

fn tutorial(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if let Some(step) = player.tutorial_step {
            if model.show_tutorial && player.popups.is_empty() && model.confirm.is_none() {
                div![
                    C!["panel-wrapper"],
                    attrs!{ At::Role => "dialog", At::AriaLabelledBy => "popup-title", "aria-modal" => "true" },
                    div![
                        id!["tutorial-panel"],
                        C!["panel"],
                        img![C!["panel-image"], attrs! { At::Src => "/logo.jpg" } ],
                        div![C!["panel-content"],
                            div![
                                C!["panel-scrollable"],
                                h3![id!["popup-title"], format!("{}", step)],
                                match step {
                                    TutorialStep::Welcome => div![
                                        p!["Hey, you, welcome to the forbidden lands! You just arrived here? Oh well, seems like I'm your new best friend now! No worries, I'll show you around!"]
                                    ],
                                    TutorialStep::Logging => div![
                                        p!["The first thing that we should do is go logging. With logging we get wood that we can use to upgrade the settlement."],
                                        p!["Go to the dwarf overview, click on a dwarf and send him logging."]
                                    ],
                                    TutorialStep::SettlementExpansion2 => div![
                                        p!["To expand your settlement, you need to have enough resources. You can see the requirements in the settlement overview."],
                                        p!["Go to the settlement overview and upgrade your settlement."]
                                    ],
                                    TutorialStep::Axe => div![
                                        p!["Craft an axe to impove your dwarfs logging effectiveness."],
                                        p!["Go to the inventory, find the axe and craft one."]
                                    ],
                                    TutorialStep::SettlementExpansion3 => div![
                                        p!["Further expand your settlement to make space for mor more dwarfs."],
                                        p!["Make sure that you equip your axe to your dwarf to make him log faster."]
                                    ],
                                    TutorialStep::Hunting => div![
                                        p!["Hunting is a great way to get food. Food is important to keep your dwarfs healthy."],
                                        p!["Go to the dwarf overview, click on a dwarf and send him hunting."]
                                    ],
                                    TutorialStep::FoodPreparation => div![
                                        p!["Food is important to keep your dwarfs alive."],
                                        p!["In the inventory, craft cooked meat and store it as food in your settlement."],
                                    ],
                                    TutorialStep::SettlementExpansion4 => div![
                                        p!["Further expand your settlement to make space for mor more dwarfs."],
                                    ],
                                    TutorialStep::Idling => div![
                                        p!["Your dwarfs need to rest from time to time. If they are idling and there is enough food, they will recover their health."],
                                        p!["If a dwarfs health reaches zero, he will die."]
                                    ],
                                    TutorialStep::SettlementExpansion5 => div![
                                        p!["Further expand your settlement to make space for mor more dwarfs."],
                                    ],         
                                    TutorialStep::SettlementExpansion7 => div![
                                        p!["Further expand your settlement to make space for mor more dwarfs."],
                                    ],
                                    TutorialStep::SettlementExpansion9 => div![
                                        p!["Further expand your settlement to make space for mor more dwarfs."],
                                    ],
                                    TutorialStep::Quests => div![
                                        p!["Quests are a great way to earn money, or get new dwarfs and items. Go to the quest overview and do quests until you get a new dwarf. Make sure that a new dwarf has enough space in your settlement."]
                                    ],
                                    TutorialStep::MakeLove => div![
                                        p!["If you have both a male and a female adult dwarf, you can let them idle for a while. There is a good chance that they get a child!"]
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
                                }

                            ],
                            if step.requires().complete(player) {
                                button![
                                    attrs! { At::Disabled => (!step.requires().complete(player)).as_at_value() },
                                    ev(Ev::Click, move |_| Msg::send_event(ClientEvent::NextTutorialStep)),
                                    "Complete Quest"
                                ]
                            } else {
                                button![
                                    ev(Ev::Click, move |_| Msg::ToggleTutorial),
                                    "Close"
                                ]
                            }
                        ]

                    ]
                ]
            } else {
                button![
                    attrs! { At::TabIndex => "0", At::AriaLabel => if step.requires().complete(player) {
                        "Tutorial Quest (complete)"
                    } else {
                        "Tutorial Quest (incomplete)"
                    } },
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

fn confirm(model: &Model, _state: &shared::State, _user_id: &shared::UserId) -> Node<Msg> {
    if let Some(client_event) = &model.confirm {
        div![
            C!["panel-wrapper"],
            attrs! { At::Role => "dialog", At::AriaLabelledBy => "popup-title", "aria-modal" => "true" },
            div![
                id!["tutorial-panel"],
                C!["panel"],
                img![C!["panel-image"], attrs! { At::Src => "/logo.jpg" }],
                div![
                    C!["panel-content"],
                    h3![id!["popup-title"], "Confirm"],
                    match client_event {
                        ClientEvent::ReleaseDwarf(..) =>
                            p!["Do you really want to release this dwarf?"],
                        ClientEvent::Sell(..) =>
                            p!["Do you really want to sell this item on the market?"],
                        _ => p![],
                    },
                    button![ev(Ev::Click, move |_| Msg::ConfirmYes), "Yes"],
                    button![ev(Ev::Click, move |_| Msg::ConfirmNo), "No"],
                ]
            ]
        ]
    } else {
        Node::Empty
    }
}

fn name(model: &Model, user_id: &shared::UserId, include_online_status: bool) -> Vec<Node<Msg>> {
    let client_state = &model.state;
    let state: &&shared::State = &client_state.get_state().unwrap();

    if let Some(player) = state.players.get(user_id) {
        let (is_premium, is_dev, games_won, guest, joined) = model
            .state
            .get_user_data(user_id)
            .map(|user_data| {
                (
                    user_data.premium > 0,
                    user_data.admin,
                    user_data.games_won,
                    user_data.guest,
                    user_data.joined.assume_utc(),
                )
            })
            .unwrap_or((false, false, 0, false, datetime!(2100-01-01 0:00 UTC)));

        vec![
            span![
                C!["username"],
                format!(
                    "{}",
                    client_state
                        .get_user_data(user_id)
                        .map(|data| data.username.clone().censor())
                        .unwrap_or_default()
                )
            ],
            if is_dev {
                span![C!["nametag", "developer"], "Developer"]
            } else {
                Node::Empty
            },
            if joined < datetime!(2024-08-27 0:00 UTC) {
                span![C!["nametag", "veteran"], "Veteran"]
            } else {
                Node::Empty
            },
            if guest {
                span![C!["nametag", "guest"], "Guest"]
            } else {
                Node::Empty
            },
            if is_premium {
                span![C!["nametag", "premium"], "Premium"]
            } else {
                Node::Empty
            },
            if games_won == 1 {
                span![C!["nametag", "winner"], "Winner"]
            } else if games_won > 1 {
                span![C!["nametag", "winner"], format!("Winner ({})", games_won)]
            } else {
                Node::Empty
            },
            if include_online_status {
                if player.is_online(state.time) {
                    span![C!["online"], "●"]
                } else {
                    Node::Empty
                }
                /*span![
                    C![
                        "symbols",
                        if player.is_online(state.time) {
                            "online"
                        } else {
                            "offline"
                        }
                    ],
                    "●"
                ]*/
            } else {
                Node::Empty
            },
        ]
    } else {
        Vec::new()
    }
}

fn tribe_name(tribe: TribeId, game_id: GameId) -> Node<Msg> {
    let mut rng = Image::rng_from_str(&format!("{}-{}", tribe, game_id));
    let tribe_name = format!("{} Tribe", Dwarf::name(&mut rng));
    let tribe_color = ((rng.next_u32() % 192) as u8, (rng.next_u32() % 192) as u8, (rng.next_u32() % 192) as u8);
    span![style![ St::Color => format!("rgb({}, {}, {})", tribe_color.0, tribe_color.1, tribe_color.2) ], tribe_name]
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
                if let Some(king) = state.king {
                    div![
                        p![format!("All hail our King {}!", client_state.get_user_data(&king).map(|data| data.username.clone().censor()).unwrap_or_default())],
                        p![format!("Become the new king by being better in the quest {} than the current king.", QuestType::ForTheKing)]
                    ]
                } else {
                    div![
                        p![format!("At the moment, there is no King in this world. Be the first to become the new King by completing the quest {}", QuestType::ForTheKing)]
                    ]
                },
            ]
        ],
    

        h2!["Ranking"],
        p![format!("To win this game, you need to meet two conditions. First, expand your settlement until you reach level 100. Second, become the king of this world. If both conditions are met, the game will be over and you will be the winner. As a reward, you get gifted a free premium account for {} days.", WINNER_NUM_PREMIUM_DAYS)],

        table![
            C!["ranking"],
            tr![
                th!["Rank"],
                th!["Username"],
                th!["Tribe"],
                th!["Level"],
                th![]
            ],
            players.iter().enumerate().map(|(i, (user_id, player))| {
                let rank = i + 1;
                let current_user = *current_user_id == **user_id;

                tr![C![if current_user { "current-user" } else { "" }],
                    td![rank],
                    td![
                        name(model, user_id, true)
                    ],
                    if let Some(tribe) = player.tribe.as_ref() {
                        td![tribe_name(
                            *tribe,
                            model.game_id
                        )]
                    } else {
                        td![]
                    },
                    td![player.base.curr_level],
                    td![
                        if !current_user {
                            a![
                                C!["button", "inline"],
                                attrs! { At::Href => format!("{}/visit/{}", model.base_path(), user_id.0) },
                                format!(
                                    "Visit",
                                ),
                            ]
                        } else {
                            Node::Empty
                        }

                        
                    ]
                ]
            })
        ]
    ]
}

fn fmt_time(mut time: u64, precise: bool) -> String {
    time /= SPEED;
    
    /*if time >= 60 {
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
        format!("{} second", time)
    } else {
        format!("{} seconds", time)
    }
    */

    let mut s = String::new();
    if time / 60 / 60 >= 1 {
        s.push_str(&format!("{}h ", time / 60 / 60));
        if !precise {
            return s;
        }
    }
    if time / 60 >= 1 {
        s.push_str(&format!("{}m ", (time / 60) % 60));
        if !precise {
            return s;
        }
    }
    s.push_str(&format!("{}s", time % 60));
    s
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
                .filter_map(|(item, qty, time)| {
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
        attrs! {At::Role => "progressbar", At::AriaValueMin => 0, At::AriaValueMax => max, At::AriaValueNow => curr, At::AriaLabel => "Health"},
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
        attrs! {At::Role => "progressbar", At::AriaValueMin => 0, At::AriaValueMax => max, At::AriaValueNow => curr, At::AriaLabel => "Score"},
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
                    "{} / {} XP ({} participating)",
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

fn tribe_bar(curr: u64, max: u64, rank: usize, max_rank: usize, markers: Vec<u64>) -> Node<Msg> {
    div![
        attrs! {At::Role => "progressbar", At::AriaValueMin => 0, At::AriaValueMax => max, At::AriaValueNow => curr, At::AriaLabel => "Score"},
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
            if curr == max {
                format!(
                    "{} FP ({} place of {})",
                    big_number(curr),
                    enumerate(rank),
                    max_rank
                )
            } else {
                format!(
                    "{} / {} FP ({} place of {})",
                    big_number(curr),
                    big_number(max),
                    enumerate(rank),
                    max_rank
                )
            }
        ],
    ]
}

fn dwarf_occupation(dwarf: &Dwarf, player: &Player) -> Node<Msg> {
    if dwarf.is_adult() {
        if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
            div![
                if dwarf.auto_idle {
                    span![format!(
                        "Auto-idling, resuming quest {} shortly.",
                        quest_type,
                    )]
                } else {
                    span![format!("Participating in quest {}.", quest_type,)]
                },
                br![],
                if quest_type.occupation() != Occupation::Idling {
                    stars_occupation(dwarf, quest_type.occupation())
                } else {
                    Node::Empty
                }
            ]
        } else {
            div![
                if dwarf.auto_idle {
                    span![format!(
                        "Auto-idling, resuming occupation {} shortly.",
                        dwarf.occupation,
                    )]
                } else {
                    span![format!("Currently {}.", dwarf.occupation)]
                },
                br![],
                if dwarf.occupation != Occupation::Idling {
                    stars_occupation(dwarf, dwarf.occupation)
                } else {
                    Node::Empty
                }
            ]
        }
    } else {
        div![if let Some(mentor) = dwarf.mentor {
            let mentor = player.dwarfs.get(&mentor).unwrap();

            vec![
                span![format!(
                    "Doing an apprenticeship with {} in {}.",
                    mentor.custom_name.as_ref().unwrap_or(&mentor.name),
                    mentor.actual_occupation(),
                )],
                br![],
                if dwarf.occupation != Occupation::Idling {
                    stars_occupation(dwarf, mentor.occupation)
                } else {
                    Node::Empty
                },
            ]
        } else {
            vec![span![format!("Currently {}.", dwarf.occupation)]]
        },]
    }
}

fn dwarf_image(dwarf: Option<&Dwarf>, player: &Player) -> Vec<Node<Msg>> {
    if let Some(dwarf) = dwarf {
        vec![
            td![
                img![
                    C!["list-item-image"],
                    attrs! {At::Src => Image::from_dwarf(dwarf).as_at_value()}
                ],
                if let Some(apprentice) = dwarf.apprentice.and_then(|id| player.dwarfs.get(&id)) {
                    img![
                        C!["list-item-image-corner"],
                        attrs! {At::Src => Image::from_dwarf(apprentice).as_at_value()},
                    ]
                } else {
                    Node::Empty
                },
                if let Some(mentor) = dwarf.mentor.and_then(|id| player.dwarfs.get(&id)) {
                    img![
                        C!["list-item-image-corner"],
                        attrs! {At::Src => Image::from_dwarf(mentor).as_at_value()},
                    ]
                } else {
                    Node::Empty
                }
            ],
            td![div![
                C!["list-item-image-col"],
                enum_iterator::all::<ItemType>()
                    .filter(ItemType::equippable)
                    .map(|item_type| {
                        let equipment = dwarf.equipment.get(&item_type);
                        if let Some(equipment) = equipment {
                            img![attrs! { At::Src => Image::from(*equipment).as_at_value() }]
                        } else {
                            div![C!["placeholder"]]
                        }
                    })
            ]],
        ]
    } else {
        vec![
            td![div![C!["list-item-image", "placeholder"]]],
            td![div![
                C!["list-item-image-col"],
                enum_iterator::all::<ItemType>()
                    .filter(ItemType::equippable)
                    .map(|_| { div![C!["placeholder"]] })
            ]],
        ]
    }
}

fn dwarf_details(dwarf: Option<&Dwarf>, player: &Player, visit_mode: bool) -> Vec<Node<Msg>> {
    if let Some(dwarf) = dwarf {
        vec![
            h3![C!["title"], dwarf.actual_name()],
            p![
                C!["subtitle"],
                format!(
                    "{}, {} Years old.",
                    if dwarf.is_female { "Female" } else { "Male" },
                    dwarf.age_years()
                ),
                if let Some(apprentice) = dwarf.apprentice.and_then(|id| player.dwarfs.get(&id)) {
                    format!(" Mentor of {}.", apprentice.actual_name())
                } else {
                    String::new()
                },
                if let Some(mentor) = dwarf.mentor.and_then(|id| player.dwarfs.get(&id)) {
                    format!(" Apprentice of {}.", mentor.actual_name())
                } else {
                    String::new()
                },
                if visit_mode {
                    Vec::new()
                } else {
                    vec![
                        br![],
                        dwarf_occupation(dwarf, player),
                        health_bar(dwarf.health, MAX_HEALTH),
                    ]
                }
            ],
        ]
    } else {
        vec![h3![C!["title"], "None"]]
    }
}

fn item_details(item: Item, n: u64) -> Vec<Node<Msg>> {
    vec![
        td![img![
            C!["list-item-image"],
            attrs! {At::Src => Image::from(item).as_at_value()}
        ]],
        td![
            C!["list-item-content"],
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
                /*if item.money_value() > 0 {
                    span![C!["short-info"], format!("{} Coins", item.money_value())]
                } else {
                    Node::Empty
                },*/
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
                                Some(span![format!("{} ", occupation), stars(usefulness, true)])
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
    ]
}

fn dwarfs(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    mode: DwarfsMode,
    visit_id: Option<shared::UserId>,
) -> Node<Msg> {
    if let Some(player) = visit_id.and_then(|visit_id| state.players.get(&visit_id)).or_else(|| state.players.get(user_id)) {
        if player.dwarfs.len() > 0 {
            let mut dwarfs = player
                .dwarfs
                .iter()
                .filter(|(_, dwarf)| match mode {
                    DwarfsMode::Select(DwarfsSelect::Mentor(_)) => dwarf.is_adult(),
                    DwarfsMode::Select(DwarfsSelect::Apprentice(_)) => !dwarf.is_adult(),
                    _ => true,
                })
                .collect::<Vec<_>>();

            dwarfs.sort_by_key(|(_, dwarf)| {
                let mut sort = model.dwarfs_filter.sort;
                if let DwarfsMode::Select(DwarfsSelect::Quest(quest_id, _dwarf_idx)) = mode {
                    let occupation = state.quests.get(&quest_id).unwrap().quest_type.occupation();
                    sort = DwarfsSort::BestIn(occupation);
                }
                match sort {
                    DwarfsSort::LeastHealth => dwarf.health,
                    DwarfsSort::BestIn(occupation) => {
                        u64::MAX - dwarf.effectiveness_not_normalized(occupation)
                    }
                    DwarfsSort::WorstAssigned => {
                        if let Some((quest_type, _, _)) = dwarf.participates_in_quest {
                            dwarf.effectiveness_not_normalized(quest_type.occupation()) + 1
                        } else if dwarf.occupation == Occupation::Idling {
                            0
                        } else {
                            dwarf.effectiveness_not_normalized(dwarf.occupation) + 1
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
                        dwarf_image(Some(dwarf), player),
                        td![
                            C!["list-item-content", "grow"],
                            dwarf_details(Some(dwarf), player, visit_id.is_some()),
                            p![match mode {
                                DwarfsMode::Overview if visit_id.is_some() => {
                                    Node::Empty
                                }
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
                                        stars_occupation(
                                            dwarf,
                                            state
                                                .quests
                                                .get(&quest_id)
                                                .unwrap()
                                                .quest_type
                                                .occupation()
                                        )
                                    ]]
                                }
                                DwarfsMode::Select(DwarfsSelect::Mentor(apprentice_id)) => {
                                    div![button![
                                        if !dwarf.is_adult() {
                                            attrs! { At::Disabled => "true" }
                                        } else {
                                            attrs! {}
                                        },
                                        ev(Ev::Click, move |_| Msg::AssignMentor(
                                            apprentice_id,
                                            Some(id)
                                        )),
                                        format!(
                                            "Assign as Mentor",
                                        ),
                                    ]]
                                }
                                DwarfsMode::Select(DwarfsSelect::Apprentice(mentor_id)) => {
                                    div![button![
                                        if dwarf.is_adult() {
                                            attrs! { At::Disabled => "true" }
                                        } else {
                                            attrs! {}
                                        },
                                        ev(Ev::Click, move |_| Msg::AssignApprentice(
                                            id,
                                            Some(mentor_id)
                                        )),
                                        format!(
                                            "Assign as Apprentice",
                                        ),
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
                h2![C!["title"], dwarf.actual_name()],
                div![C!["image-aside"],
                    img![attrs! {At::Src => Image::from_dwarf(dwarf).as_at_value()}],
                    div![
                        p![C!["subtitle"],
                        format!("{}, {} Years old.", if dwarf.is_female {
                            "Female"
                        } else {
                            "Male"
                        }, dwarf.age_years()),
                        br![],
                        dwarf_occupation(dwarf, player),
                        ],
                        health_bar(dwarf.health, MAX_HEALTH),

                        p![
                            if let Some(custom_name) = model.custom_name.as_ref().cloned() {
                                div![
                                    label!["Name"],
                                    input![
                                        attrs! {At::Value => custom_name},
                                        input_ev(Ev::Input, move |name| Msg::UpdateName(Some(name))),
                                    ],
                                    button![
                                        ev(Ev::Click, move |_| Msg::SetName(dwarf_id, Some(custom_name))),
                                        "Save Name"
                                    ],
                                    button![
                                        ev(Ev::Click, move |_| Msg::SetName(dwarf_id, None)),
                                        "Reset Name"
                                    ]
                                ]
                            } else {
                                let dwarf_name = dwarf.actual_name().to_owned();

                                button![
                                    ev(Ev::Click, move |_| Msg::UpdateName(Some(dwarf_name))),
                                    "Edit Name"
                                ]
                            },
                            if is_premium {
                                button![
                                    ev(Ev::Click, move |_| Msg::send_event(ClientEvent::Optimize(Some(dwarf_id)))),
                                    format!("Optimize Equipment for Current Occupation"),
                                ]
                            } else {
                                a![
                                    C!["premium-feature", "button"],
                                    format!("Optimize Equipment for Current Occupation"),
                                    attrs! { At::Href => format!("/store") },

                                ]
                            },
                            button![
                                ev(Ev::Click, move |_| Msg::Confirm(ClientEvent::ReleaseDwarf(dwarf_id))),
                                "Release Dwarf"
                            ],
                            input![
                                id!["manual-management"],
                                attrs! {At::Type => "checkbox", At::Checked => dwarf.manual_management.as_at_value()},
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::ToggleManualManagement(dwarf_id))),
                            ],
                            label![attrs! {At::For => "manual-management"}, "Manual Management Only (Disables Dwarfen Manager)"]
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
                            let equipment = dwarf.equipment.get(&item_type);

                            tr![C!["list-item-row"],
                                if let Some(equipment) = equipment {
                                    td![img![C!["list-item-image"], attrs! { At::Src => Image::from(*equipment).as_at_value() } ]]
                                } else {
                                    td![div![C!["list-item-image", "placeholder"]]]
                                },
                                td![C!["list-item-content"],
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
                                td![C!["list-item-content"],
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
                                        td![C!["list-item-content"],
                                            h3![C!["title"],
                                                format!("{}", occupation),
                                            ],
                                            p![
                                                stars_occupation(dwarf, occupation)
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
                                        td![C!["list-item-content"],
                                            h4![C!["title"], "Requires"],
                                            p![C!["subtitle"],stats_simple(&occupation.requires_stats())],
                                            h4![C!["title"], "Provides"],
                                            p![C!["subtitle"], if all_items.is_empty() {
                                                "Lets your dwarf eat and restore health. Adds a possibility for child dwarfs for each male and female dwarf that is idling.".to_owned()
                                            } else {
                                                all_items.into_iter().join(", ")
                                            }]
                                        ]
                                    ]
                                })
                            ]
                        }
                    } else {
                        p!["This dwarf is still a child and can't work."]
                    },
                    div![
                        h3!["Apprenticeship"],
                        if dwarf.is_adult() {
                            div![
                                p!["Assign an apprentice such that they can learn from this dwarfs occupation."],
                                table![C!["list"],
                                    tr![
                                        C!["list-item-row"],
                                        dwarf_image(dwarf.apprentice.and_then(|apprentice| player.dwarfs.get(&apprentice)), player),
                                        td![
                                            C!["list-item-content", "grow"],
                                            dwarf_details(dwarf.apprentice.and_then(|apprentice| player.dwarfs.get(&apprentice)), player, false),
                                            button![
                                                ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(DwarfsMode::Select(DwarfsSelect::Apprentice(dwarf_id))))),
                                                if dwarf.apprentice.and_then(|apprentice| player.dwarfs.get(&apprentice)).is_some() {
                                                    "Change Apprentice"
                                                } else {
                                                    "Select Apprentice"
                                                }
                                            ],
                                            if dwarf.apprentice.and_then(|apprentice| player.dwarfs.get(&apprentice)).is_some() {
                                                let apprentice_id = dwarf.apprentice.unwrap();
    
                                                vec![
                                                    button![
                                                        ev(Ev::Click, move |_| Msg::AssignApprentice(apprentice_id, None)),
                                                        "Remove Apprentice"
                                                    ],
                                                    a![
                                                        C!["button"],
                                                        attrs! { At::Href => format!("{}/dwarfs/{}", model.base_path(), apprentice_id) },
                                                        "Dwarf Details"
                                                    ]
                                                ]
                                            } else {
                                                Vec::new()
                                            }
                                        ]
                                    ]
                                ]
                            ]
                        } else {
                            div![
                                p!["You can assign a mentor such that this dwarf can learn from the mentors occupation."],
                                table![C!["list"],
                                    tr![
                                        C!["list-item-row"],
                                        dwarf_image(dwarf.mentor.and_then(|mentor| player.dwarfs.get(&mentor)), player),
                                        td![
                                            C!["list-item-content", "grow"],
                                            dwarf_details(dwarf.mentor.and_then(|mentor| player.dwarfs.get(&mentor)), player, false),
                                            button![
                                                ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(DwarfsMode::Select(DwarfsSelect::Mentor(dwarf_id))))),
                                                if dwarf.mentor.and_then(|mentor| player.dwarfs.get(&mentor)).is_some() {
                                                    "Change Mentor"
                                                } else {
                                                    "Select Mentor"
                                                }
                                            ],
                                            if dwarf.mentor.and_then(|mentor| player.dwarfs.get(&mentor)).is_some() {
                                                vec![
                                                    button![
                                                        ev(Ev::Click, move |_| Msg::AssignMentor(dwarf_id, None)),
                                                        "Remove Mentor"
                                                    ],
                                                    a![
                                                        C!["button"],
                                                        attrs! { At::Href => format!("{}/dwarfs/{}", model.base_path(), dwarf.mentor.unwrap()) },
                                                        "Dwarf Details"
                                                    ]
                                                ]
                                            } else {
                                                Vec::new()
                                            }
                                        ]
                                    ]
                                ]
                            ]
                            
                        }
                    ]
                    

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
    let player = state.players.get(user_id).unwrap();

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
                }) && ((player.base.curr_level <= quest.max_level && player.base.curr_level >= quest.min_level) || quest.contestants.contains_key(user_id))
            }).map(|(quest_id, quest)| {
                tr![
                    C!["list-item-row", match quest.quest_type.reward_mode().reward_type() {
                        RewardType::Fair => "reward-mode-fair",
                        RewardType::Best => "reward-mode-best",
                        RewardType::Chance => "reward-mode-chance",
                    }],
                    td![img![
                        C!["list-item-image"],
                        attrs! {At::Src => Image::from(quest.quest_type).as_at_value()}
                    ]],
                    td![
                        C!["list-item-content"],
                        h3![C!["title"], format!("{}", quest.quest_type)],
                        p![
                            C!["subtitle"],
                            format!("{} remaining |  Requires {} | Level {} - {}", fmt_time(quest.time_left, true), quest.quest_type.occupation(), quest.min_level, quest.max_level)
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
                        p![C!["subtitle"], format!("{} remaining.", fmt_time(quest.time_left, true))],
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
                                QuestType::CollapsedCave => p!["A cave has collapsed and some dwarfs are trapped inside. Be the first to save their life and they will move into your settlement."],
                                QuestType::TheHiddenTreasure => p!["The first who finds the hidden treasure can keep it."],
                                QuestType::CatStuckOnATree => p!["A cat is stuck on a tree. Help her get on the ground an she will gladly follow you home."],
                                QuestType::AttackTheOrks => p!["The orc camp was sptted near the elven village in preparation for an attak. Attack them first and get a reward from the elves!"],
                                QuestType::FreeTheDwarf => p!["Some dwarfs captured by the orks. It seems like they don't want to kill them, but instead persuade them to attack the elves instead. Of course, the dwarfs would never do such a thing! Free them and they will join your settlement."],
                                QuestType::FarmersContest => p!["Participate in the farmers contest. The best farmer gets a reward."],
                                QuestType::CrystalsForTheElves => p!["The Elves need special crystals to cast their magic. Although they don't want to tell you what they will use the crystals for, you accept the offer. Bring them some and they will reward you."],
                                QuestType::ElvenVictory => p!["The elves are winning the war against the orks. They need wood to build large fenced areas where the surviving orks will be captured."],
                                QuestType::ADarkSecret => p!["While exploring the elven regions, you find a dark secret. The elves are not what they seem to be. They have used their magic on the dwarfs to turn them into orks. It seems like the orks were never the barbaric enemies that they seemed like, they are just unfortunate dwarfen souls like you and me. A devious plan by the elves to divide and weaken the dwarfen kingdom!"],
                                QuestType::TheMassacre => p!["The elves have unleased their dark magic in a final attempt to eliminate all orks. Realizing that you have helped in this terrible act by providing the elves with the crystals needed for their dark magic, you attempt to fight the elven magicians to stop the massacre."],
                                QuestType::TheElvenWar => p!["All of the dwarfen settlements have realized their mistake and have united to fight the elves. The united dwarfen armies have to fight the elven magicians in order to restore peace in the forbidden lands. Send the best fighters that you have, or the forbidden lands will be lost forever to the elven dark magic."],
                                QuestType::Concert => p!["The dwarfen bards have organized a concert in the tavern. Make sure to participate!"],
                                QuestType::MagicalBerries => p!["The magical berries are ripe and ready to be picked. Everyone that helps picking them gets a reward."],
                                QuestType::EatingContest => p!["Participate in the eating contest and earn a reward."],
                                QuestType::Socializing => p!["Socialize with the other dwarfs in the tavern. You may find a new friend."],
                                QuestType::TheElvenMagician => p!["The elven magician is working tirelessly on a big projects. Get him some of his favorite berries so that he can focus better."],
                                QuestType::ExploreNewLands => p!["Help exploring new lands and earn a reward."],
                                QuestType::DeepInTheCaves => p!["Deep in the caves of the forbidden lands, mythical creatures are said to live. Explore the caves and find out what is hidden there."],
                                QuestType::MinersLuck => p!["You are feeling lucky today. Go mining and see what you can find."],
                                QuestType::AbandonedOrkCamp => p!["The orks have abandoned their camp. Explore it and see if you can find anything of use."],
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
                            dwarf_image(dwarf, player),
                            td![
                                C!["list-item-content", "grow"],
                                dwarf_details(dwarf, player, false),
                                button![
                                    ev(Ev::Click, move |_| Msg::ChangePage(Page::Dwarfs(DwarfsMode::Select(DwarfsSelect::Quest(quest_id, dwarf_idx))))),
                                    if dwarf_id.is_some() {
                                        "Change Dwarf"
                                    } else {
                                        "Select Dwarf"
                                    }
                                ],
                                if dwarf_id.is_some() {

                                    vec![
                                        button![
                                            ev(Ev::Click, move |_| Msg::AssignToQuest(quest_id, dwarf_idx, None)),
                                            "Remove Dwarf"
                                        ],
                                        a![
                                            C!["button"],
                                            attrs! { At::Href => format!("{}/dwarfs/{}", model.base_path(), dwarf_id.unwrap()) },
                                            "Dwarf Details"
                                        ]
                                    ]
                                } else {
                                    Vec::new()
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

fn big_number(num: u64) -> String {
    /*
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
    */
    format!("{}", num)
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
        /*
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
        */

        let guest = model
            .state
            .get_user_data(user_id)
            .map(|user_data| user_data.guest)
            .unwrap_or(false);

        let mut unlocks = (1..100)
            .filter_map(|curr_level| {
                let prev_level = curr_level - 1;
                if player.base.max_dwarfs_at(curr_level) > player.base.max_dwarfs_at(prev_level) {
                    Some(Unlock::MaxPopulation(curr_level))
                } else {
                    None
                }
            })
            .chain(enum_iterator::all::<Occupation>().map(Unlock::Occupation))
            .chain(enum_iterator::all::<Item>().map(Unlock::Item))
            .collect::<Vec<_>>();

        unlocks.sort_by_key(|item| item.unlocked_at_level());
        unlocks.retain(|item| item.unlocked_at_level() > player.base.curr_level);

        div![
            C!["content"],
            if guest {
                let joined = model
                    .state
                    .get_user_data(user_id)
                    .map(|user_data| user_data.joined)
                    .unwrap();

                div![
                    C!["important"],
                    strong![format!("Guest Account")],
                    div![
                        C!["image-aside", "small"],
                        img![attrs! {At::Src => "/guest.jpg"}],
                        div![
                            p![format!(
                                "You are currently using a guest account that expires in {}. Set your username and password to keep access to your account and play from multiple devices.",
                                fmt_time((joined.saturating_add(Duration::days(30)).assume_utc().unix_timestamp() - (Date::now() / 1000.0) as i64).max(0) as u64 * SPEED, true)
                            )],
                            a![
                                C!["button"],
                                attrs! { At::Href => format!("/change-username") },
                                "Set Username"
                            ],
                            a![
                                C!["button"],
                                attrs! { At::Href => format!("/change-password") },
                                "Set Password"
                            ]
                        ]
                    ]
                ]
            } else {
                Node::Empty
            },
            /*
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
            */
            if player.remaining_time_until_starvation(state) <= 60 * 60 * 12 * SPEED {
                div![
                    C!["important"],
                    strong![format!("Your Dwarfs will Starve Soon")],
                    div![
                        C!["image-aside", "small"],
                        img![attrs! {At::Src => Image::Starvation.as_at_value()}],
                        div![
                            p![format!("Your dwarfs will start to die of starvation in {}. Make sure that you have enough food to feed your dwarfs.", fmt_time(player.remaining_time_until_starvation(state), true))],
                        ]
                    ]
                ]
            } else {
                Node::Empty
            },
            if let Some(event) = state.event {
                div![
                    C!["important"],
                    strong![format!("{}", event)],
                    div![
                        C!["image-aside", "small"],
                        img![attrs! {At::Src => Image::from(event).as_at_value()}],
                        div![
                            match event {
                                WorldEvent::Drought => p!["There is a drought happening. The drought makes it harder to farm, gather and hunt. Make sure that your dwarfs don't starve!"],
                                WorldEvent::Flood => p!["A flood has occurred. The flood makes it harder to farm, gather and fish. Make sure that your dwarfs don't starve!"],
                                WorldEvent::Plague => p!["A plague is happening. During a plague, the health of your dwarfs decreases much faster. Settlements with more dwarfs are affected more than settlements with fewer dwarfs. Make sure that your dwarfs don't die!"],
                                WorldEvent::Earthquake => p!["An earthquake has occurred. The earthquake makes it harder to mine, rockhound and log. Be aware of your resource production! There is also a higher chance that new dwarfs arrive in your settlement during this event."],
                                WorldEvent::Tornado => p!["Tornadoes sweep accross the forbidden lands. The tornadoes makes it harder to log, gather and farm. Be aware of your resource production! There is also a higher chance that new dwarfs arrive in your settlement during this event."],
                                WorldEvent::Carnival => p!["The carnival is in town! During the carnival, the dwarfs are in a festive mood and have less time to work. The carnival also increases the chance of new dwarfs arriving in your settlement."],
                                WorldEvent::FullMoon => p!["The full moon is shining bright. During the full moon, the dwarfs can't sleep and thus work less efficiently. As they won't sleep well anyway, there is a chance for more children being born."],
                                WorldEvent::Revolution => p!["The dwarfs are revolting! During a revolution, the dwarfs work less efficiently and any king is overthown immediately."],
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
                tr![
                    th!["Population"],
                    td![format!(
                        "{}/{}",
                        player.dwarfs.len(),
                        player.base.max_dwarfs()
                    )]
                ],
                tr![th!["Money"], td![format!("{} coins", player.money)]],
                tr![th!["Food"], td![format!("{} food", player.base.food)]],
            ],
            h3!["Upgrade Settlement"],
            div![
                C!["image-aside"],
                img![attrs! {At::Src => Image::from(player.base.village_type()).as_at_value()}],
                if let Some(requires) = player.base.upgrade_cost() {
                    div![
                        p!["Upgrade your settlement to increase the maximum population and unlock new occupations for your dwarfs. New dwarfs can be collected by doing quests, or they can simply wander into to your settlement from time to time."],
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
                                        Unlock::MaxPopulation(_) => "+1 Maximum Population".to_string(),
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
                                format!("Upgrading ({} remaining)", fmt_time(player.base.build_time, true)),
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
            /*
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
            */
        ]
    } else {
        Node::Empty
    }
}

fn manager(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
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
            .filter_map(|curr_level| {
                let prev_level = curr_level - 1;
                if player.base.max_dwarfs_at(curr_level) > player.base.max_dwarfs_at(prev_level) {
                    Some(Unlock::MaxPopulation(curr_level))
                } else {
                    None
                }
            })
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

            div![
                h2!["Dwarfen Manager"],
                div![C!["image-aside"],
                    img![attrs! {At::Src => Image::Manager.as_at_value()}],
                    div![
                        p!["The dwarfen manager can optimally assign dwarfs to carry out the occupations that are best suited for them. Furthermore, the manager can also assign the optimal equipment to each dwarf to further increase their effectiveness in their occupation."],
                        p!["The dwarfen manager ignores children and dwarfs that are on quests, as well as dwarfs that have manual magement enabled."],
                        p![
                            strong![if player.auto_functions.auto_idle {
                                "Auto-Idling is enabled."
                            } else {
                                "Auto-Idling is disabled."
                            }]
                        ],
                        p![
                            button![
                                ev(Ev::Click, move |_| Msg::send_event(
                                    ClientEvent::ToggleAutoIdle
                                )),
                                if player.auto_functions.auto_idle && is_premium { "Disable Auto Idling for all Dwarfs" } else { "Enable Auto Idling for all Dwarfs" },
                            ]
                        ],
                        p![
                            strong![format!("Average Efficiency: {}%", player.average_efficiency().unwrap_or(0))]
                        ],
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
                        if is_premium {
                            button![
                                ev(Ev::Click, move |_| Msg::send_event(ClientEvent::Optimize(None))),
                                format!("Reassign Occupations and Equipment"),
                                ]
                        } else {
                            a![
                                C!["premium-feature", "button"],
                                format!("Reassign Occupations and Equipment"),
                                attrs! { At::Href => format!("/store") },
                            ]
                        },
                    ]
                ],
            ],
        ]
    } else {
        Node::Empty
    }
}

fn inventory_options(
    model: &Model,
    player: &Player,
    item: Item,
    n: u64,
    is_premium: bool,
) -> Vec<Node<Msg>> {
    vec![
        if let Some((level, requires)) = item.requires() {
            vec![
                h4!["Craft Item"],
                bundle(
                    &requires.clone().mul(
                        model
                            .slider
                            .get(&(item, SliderType::Craft))
                            .copied()
                            .unwrap_or_default()
                            .max(1),
                    ),
                    player,
                    true,
                ),
                if player.base.curr_level >= level {
                    if player.auto_functions.auto_craft.contains(&item) && is_premium {
                        button![
                            ev(Ev::Click, move |_| Msg::send_event(
                                ClientEvent::ToggleAutoCraft(item)
                            )),
                            "Disable Auto",
                        ]
                    } else {
                        let max = player
                            .inventory
                            .items
                            .can_remove_x_times(&requires)
                            .unwrap_or(0);

                        if max == 0 {
                            Node::Empty
                        } else {
                            slider(
                                model,
                                item,
                                SliderType::Craft,
                                |_| "Craft".to_owned(),
                                max.min(1),
                                max,
                                ClientEvent::Craft,
                                |n| n == 0,
                                Some(if is_premium {
                                    button![
                                        ev(Ev::Click, move |_| Msg::send_event(
                                            ClientEvent::ToggleAutoCraft(item)
                                        )),
                                        "Auto",
                                    ]
                                } else {
                                    a![
                                        C!["premium-feature", "button"],
                                        "Auto",
                                        attrs! { At::Href => format!("/store") },
                                    ]
                                }),
                            )
                        }
                    }
                } else {
                    p!["Unlocked at level ", level]
                },
            ]
        } else {
            Vec::new()
        },
        if let Some((_level, requires)) = item.requires() {
            let max = player
                .inventory
                .items
                .get(&item)
                .copied()
                .unwrap_or_default();

            if matches!(
                item.item_type(),
                Some(ItemType::Tool | ItemType::Jewelry | ItemType::Clothing)
            ) && (max > 0 || player.auto_functions.auto_dismantle.contains(&item))
            {
                vec![
                    h4!["Dismantle Item"],
                    bundle(
                        &requires
                            .clone()
                            .mul(
                                model
                                    .slider
                                    .get(&(item, SliderType::Dismantle))
                                    .copied()
                                    .unwrap_or_default()
                                    .max(1),
                            )
                            .div(DISMANTLING_DIVIDER),
                        player,
                        false,
                    ),
                    if player.auto_functions.auto_dismantle.contains(&item) && is_premium {
                        button![
                            ev(Ev::Click, move |_| Msg::send_event(
                                ClientEvent::ToggleAutoDismantle(item)
                            )),
                            "Disable Auto",
                        ]
                    } else {
                        slider(
                            model,
                            item,
                            SliderType::Dismantle,
                            |_| "Dismantle".to_owned(),
                            max.min(1),
                            max,
                            ClientEvent::Dismantle,
                            |n| n == 0,
                            Some(if is_premium {
                                button![
                                    ev(Ev::Click, move |_| Msg::send_event(
                                        ClientEvent::ToggleAutoDismantle(item)
                                    )),
                                    "Auto",
                                ]
                            } else {
                                a![
                                    C!["premium-feature", "button"],
                                    "Auto",
                                    attrs! { At::Href => format!("/store") },
                                ]
                            }),
                        )
                    },
                ]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        },
        if item.nutritional_value().is_some() {
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
                    slider(
                        model,
                        item,
                        SliderType::Store,
                        |_| "Store".to_owned(),
                        n.min(1),
                        n,
                        ClientEvent::AddToFoodStorage,
                        |n| n == 0,
                        Some(if is_premium {
                            button![
                                ev(Ev::Click, move |_| Msg::send_event(
                                    ClientEvent::ToggleAutoStore(item)
                                )),
                                "Auto",
                            ]
                        } else {
                            a![
                                C!["premium-feature", "button"],
                                "Auto",
                                attrs! { At::Href => format!("/store") },
                            ]
                        }),
                    )
                },
            ]
        } else {
            Vec::new()
        },
        vec![
            h4!["Sell Item"],
            slider(
                model,
                item,
                SliderType::Sell,
                move |n| {
                    format!(
                        "Sell ({} coins)",
                        item.money_value(n) * TRADE_MONEY_MULTIPLIER
                    )
                },
                n.min(1),
                n,
                ClientEvent::Sell,
                move |n| n > 0 && item.money_value(n) * TRADE_MONEY_MULTIPLIER == 0,
                None,
            ),
        ],
    ]
    .into_iter()
    .flatten()
    .collect()
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
            .map(|t| (t, player.inventory.items.get(&t).copied().unwrap_or(0)))
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
                    ],
                    div![
                        input![
                            id!["auto"],
                            attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.auto.as_at_value()},
                            ev(Ev::Click, |_| Msg::InventoryFilterAuto),
                        ],
                        label![attrs! {At::For => "auto"}, "Auto Enabled"]
                    ],
                ],
                div![
                    C!["no-shrink"],
                    enum_iterator::all::<ItemType>()
                        .map(|item_type| {
                            div![
                                input![
                                    id![format!("{}", item_type)],
                                    attrs! {At::Type => "checkbox", At::Checked => model.inventory_filter.by_type.get(&item_type).copied().unwrap_or(false).as_at_value()},
                                    ev(Ev::Click, move |_| Msg::InventoryFilterByType(item_type)),
                                ],
                                label![attrs! {At::For => format!("{}", item_type)}, format!("{}", item_type)]
                            ]
                        })
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
                        .contains(model.inventory_filter.item_name.to_lowercase().trim())
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
                        && (
                            if model.inventory_filter.auto {
                                player.auto_functions.auto_craft.contains(item)
                                || player.auto_functions.auto_sell.contains(item)
                                || player.auto_functions.auto_store.contains(item)
                                || player.auto_functions.auto_dismantle.contains(item)
                            } else {
                                true
                            }
                        )
                        && (

                            model.inventory_filter.by_type.values().all(|v| !v)
                            || if let Some(item_type) = item.item_type() {
                                model.inventory_filter.by_type.get(&item_type).copied().unwrap_or(false)
                            } else {
                                false
                            }
                            
                        )
                        && if let InventoryMode::Select(InventorySelect::Equipment(_, item_type)) =
                            mode
                        {
                            item.item_type() == Some(item_type) && *n > 0
                        } else {
                            true
                        }
                })
                .map(|(item, n)| {
                    tr![
                    C!["item"],
                    C!["list-item-row"],
                    match item.item_rarity() {
                        ItemRarity::Common => C!["item-common"],
                        ItemRarity::Uncommon => C!["item-uncommon"],
                        ItemRarity::Rare => C!["item-rare"],
                        ItemRarity::Epic => C!["item-epic"],
                        ItemRarity::Legendary => C!["item-legendary"],
                    },
                    item_details(item, n),
                    if let InventoryMode::Select(InventorySelect::Equipment(dwarf_id, item_type)) =
                        mode
                    {
                        td![
                            C!["list-item-content"],
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
                            C!["list-item-content"],
                            inventory_options(model, player, item, n, is_premium)
                        ]
                    }
                ]})
            ]
        ]
    } else {
        Node::Empty
    }
}

fn trades(model: &Model, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        let mut trades = state.trade_deals.iter().map(|(trade_id, trade)| (*trade_id, trade)).collect::<Vec<_>>();

        trades.sort_by_key(|(_, trade_deal)| trade_deal.time_left);

        div![
            div![
                C!["filter"],
                div![
                    C!["no-shrink"],
                    div![
                        input![
                            id!["can-afford"],
                            attrs! {At::Type => "checkbox", At::Checked => model.trade_filter.can_afford.as_at_value()},
                            ev(Ev::Click, |_| Msg::TradeFilterCanAfford),
                        ],
                        label![attrs! {At::For => "can-afford"}, "Can Afford"]
                    ],
                    div![
                        input![
                            id!["my-trades"],
                            attrs! {At::Type => "checkbox", At::Checked => model.trade_filter.my_bids.as_at_value()},
                            ev(Ev::Click, |_| Msg::TradeFilterMyBids),
                        ],
                        label![attrs! {At::For => "my-trades"}, "My Trades"]
                    ],
                ],
                div![
                    C!["no-shrink"],
                    enum_iterator::all::<ItemType>()
                        .map(|item_type| {
                            div![
                                input![
                                    id![format!("{}", item_type)],
                                    attrs! {At::Type => "checkbox", At::Checked => model.trade_filter.by_type.get(&item_type).copied().unwrap_or(false).as_at_value()},
                                    ev(Ev::Click, move |_| Msg::TradeFilterByType(item_type)),
                                ],
                                label![attrs! {At::For => format!("{}", item_type)}, format!("{}", item_type)]
                            ]
                        })
                ],
                div![
                    button![
                        ev(Ev::Click, move |_| Msg::TradeFilterReset),
                        "Reset Filter",
                    ],
                ]
            ],
            table![
                C!["items", "list"],
                trades 
                .into_iter()
                .filter(|(_, trade_deal)| {
                        trade_deal.user_trade_type == TradeType::Buy &&
                        ((if model.trade_filter.can_afford {
                            player.money >= trade_deal.next_bid
                        } else {
                            false
                        }) || (if model.trade_filter.my_bids {
                            if let Some((highest_bidder_user_id, _)) = trade_deal.highest_bidder {
                                highest_bidder_user_id == *user_id
                            } else {
                                false
                            }
                        } else {
                            false
                        }) || (!model.trade_filter.can_afford && !model.trade_filter.my_bids))
                        && (
                            model.trade_filter.by_type.values().all(|v| !v)
                            || if let Some(item_type) = trade_deal.items.iter().next().and_then(|(item, _)| item.item_type()) {
                                model.trade_filter.by_type.get(&item_type).copied().unwrap_or(false)
                            } else {
                                false
                            }
                            
                        )
                })
                .map(|(trade_id, trade_deal)| {
                    let item = *trade_deal.items.iter().next().unwrap().0;
                    let n = *trade_deal.items.iter().next().unwrap().1;
                    let highest_bidder_is_you = if let Some((highest_bidder_user_id, _)) = trade_deal.highest_bidder {   
                        highest_bidder_user_id == *user_id
                    } else {
                        false
                    };
                    let can_afford = player.money >= trade_deal.next_bid;

                    tr![

                    C!["item"],
                    C!["list-item-row"],
                    match item.item_rarity() {
                        ItemRarity::Common => C!["item-common"],
                        ItemRarity::Uncommon => C!["item-uncommon"],
                        ItemRarity::Rare => C!["item-rare"],
                        ItemRarity::Epic => C!["item-epic"],
                        ItemRarity::Legendary => C!["item-legendary"],
                    },
                    item_details(item, n),
                    td![
                        C!["list-item-content"],
                        h4![C!["title"], "Cost" ],
                        p![C!["subtitle"], format!("{} coins", trade_deal.next_bid)],
                        p![format!("Deal ends in {}.", fmt_time(trade_deal.time_left, true))],
                        if trade_deal.creator == Some(*user_id) {
                            vec![p![format!("You created this deal.")]]
                        } else {
                            vec![
                                if !can_afford && !highest_bidder_is_you {
                                    p![format!("You can't afford this deal.")]
                                } else {
                                    Node::Empty
                                },
                                if let Some((highest_bidder_user_id, highest_bidder_money)) = trade_deal.highest_bidder {
                                    if highest_bidder_user_id == *user_id {
                                        p![format!("You are the highest bidder with {} coins.", highest_bidder_money)]
                                    } else {
                                        p![format!("Highest bidder has offered {} coins.", highest_bidder_money)]
                                    }
                                } else {
                                    Node::Empty
                                },                                                 
                                button![
                                    attrs! { At::Disabled => (highest_bidder_is_you || !can_afford).as_at_value() },
                                    ev(Ev::Click, move |_| Msg::send_event(ClientEvent::Bid(trade_id))),
                                    format!("Bid {} coins", trade_deal.next_bid)
                                ]
                            ]
                            
                        },
 
                    ]
                ]
            }),
            ]
        ]
    } else {
        Node::Empty
    }
}

fn tribe(model: &Model, client_state: &ClientState<shared::State>, state: &shared::State, user_id: &shared::UserId) -> Node<Msg> {
    if let Some(player) = state.players.get(user_id) {
        if let Some(tribe_id) = player.tribe {
            //let tribe = state.tribes.get(&tribe_id).unwrap();
            let username = &client_state
                    .get_user_data(user_id)
                    .map(|data| data.username.clone().censor())
                    .unwrap_or_default();

            div![C!["content"],
                div![C!["important"],
                    strong!["Invite Players"],
                    div![C!["image-aside", "small"],
                        img![attrs! {At::Src => "/social.jpg"}],
                        div![
                            p![
                                "Use this link to invite players to the game. If they register with this link they will be assigned to your tribe once they reach the required level."
                            ],
                            p![
                                span![C!["invitation-link"], format!("https://dwarfs-in-exile.com/register?referrer={}", user_id.0)],
                            ],
                            button![
                                id!["copy-invitation-link"],
                                "Copy Link",
                            ],
                            Script![format!(
                                r#"
                                let button = document.getElementById("copy-invitation-link");
    
                                button.addEventListener("click", function() {{
                                    navigator.clipboard.writeText("https://dwarfs-in-exile.com/register?referrer={}");
    
                                    button.textContent = "Copied!";
                                }});
                                "#, user_id.0)
                            ],
                            button![
                                id!["share-invitation-link"],
                                style! { "display" => "none" },
                                "Share",
                            ],
                            Script![format!(
                                r#"
                                if (navigator.share) {{
                                    let button = document.getElementById("share-invitation-link");
    
                                    button.style.display = "block";
    
                                    button.addEventListener("click", function() {{
                                        navigator.share({{
                                            title: "Play Dwarfs in Exile!",
                                            text: "Join {} in their adventures and play Dwarfs in Exile now for free!",
                                            url: "https://dwarfs-in-exile.com/register?referrer={}",
                                        }});
                                    }});
                                }}
                                "#, username, user_id.0)
                            ],
                        ]
                        
                    ]

                ],
            
                h2!["Your Tribe"],
                p!["Spend your fame points for your tribe to conquer territories. Controlling territories rewards you with powerful dwarfs that join your settlement more frequently. You can earn tribe points by winning quests that have a single winner with the most XP collected (red quests)."],
                p![format!("If the winner of this world is from your tribe, you will earn a free premium account for {} days, so make sure to support your tribe members.", WINNER_TRIBE_NUM_PREMIUM_DAYS)],
                p![strong!["You are member of the ", tribe_name(
                    tribe_id,
                    model.game_id
                ), "."]],
                p![strong![format!("Your Fame Points: {} FP", player.tribe_points)]],
                table![C!["list"],
                enum_iterator::all::<Territory>()
                    .map(|territory| {
                        let scores =state.tribes
                            .values()
                            .map(|tribe| {
                                tribe.territories.get(&territory).copied().unwrap_or_default()
                            })
                            .collect::<Vec<_>>();

                        let max = scores.iter().max().copied().unwrap_or_default(); 

                        let curr = state.tribes.get(&tribe_id).unwrap().territories.get(&territory).copied().unwrap_or_default();

                        let rank = scores
                            .iter()
                            .filter(|s| **s >= curr)
                            .count();

                        /*div![
                            h4![format!("{}", territory)],
                            
                        ]*/
                        tr![C!["list-item-row"],
                            td![img![C!["list-item-image"], attrs! { At::Src => Image::from(territory).as_at_value() } ]],
                            td![C!["list-item-content"],
                                h3![C!["title"],
                                    format!("{}", territory)
                                ],
                                p![C!["subtitle"],
                                    format!("Provides additional dwarfs with maxed out {}.", stats_simple(&territory.provides_stats()))
                                ],
                                if rank == 1 {
                                    p!["Your tribe controls this territory."]
                                } else {
                                    p!["Your tribe does not control this territory."]
                                },
                                tribe_bar(curr, max, rank, scores.len(), scores),
                                button![
                                    if player.tribe_points > 0 {
                                        attrs! {}
                                    } else {
                                        attrs! {At::Disabled => "true"}
                                    },
                                    ev(Ev::Click, move |_| Msg::send_event(
                                        ClientEvent::SpendTribePoint(territory)
                                    )),
                                    "Spend One Fame Point"
                                ],
                            ],
                        ]
                    })
                ]
            ]
        } else {
            div![C!["content"],
                h2!["Your Tribe"],
                p![format!("You are not a member of a tribe. You will be assigned a tribe at level {}.", JOIN_TRIBE_LEVEL)],
            ]
        }
    } else {
        Node::Empty
    }
}

fn chat(
    model: &Model,
    state: &shared::State,
    user_id: &shared::UserId,
    client_state: &ClientState<shared::State>,
) -> Node<Msg> {
    let message = model.message.clone();

    if let Some(player) = state.players.get(user_id) {
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
                                .get_user_data(user_id)
                                .map(|data| data.username.clone().censor())
                                .unwrap_or_default();
                            p![
                                C!["message"],
                                span![C!["time"], format!("{} ago, ", fmt_time(state.time - time, false))],
                                span![C!["username"], format!("{username}:")],
                                span![C!["message"], format!("{}", message.censor())]
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
            if model.chat_visible {
                button![ev(Ev::Click, move |_| Msg::ToggleChat), span!["Close"]]
            } else {
                button![
                    ev(Ev::Click, move |_| Msg::ToggleChat),
                    span![
                        attrs! {At::AriaHidden => "true"},
                        if player.chat_unread {
                            Icon::ChatUnread.draw()
                        } else {
                            Icon::Chat.draw()
                        },
                        span![" Show Chat"]
                    ]
                ]
            }
        ]
    } else {
        Node::Empty
    }
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
                        p![
                            C!["message"],
                            span![C!["icon"], match msg {
                                LogMsg::DwarfUpgrade(_,_) => Icon::Person,
                                LogMsg::DwarfIsAdult(_) => Icon::Person,
                                LogMsg::DwarfDied(_) => Icon::PersonRemove,
                                LogMsg::NewPlayer(_) => Icon::WavingHand,
                                LogMsg::NewDwarf(_) => Icon::PersonAdd,
                                LogMsg::QuestCompletedMoney(_, _) => Icon::Task,
                                LogMsg::QuestCompletedPrestige(_, _) => Icon::Task,
                                LogMsg::QuestCompletedKing(_, _) => Icon::Task,
                                LogMsg::QuestCompletedItems(_, _) => Icon::Task,
                                LogMsg::QuestCompletedDwarfs(_, _) => Icon::Task,
                                LogMsg::OpenedLootCrate(_) => Icon::Inventory,
                                LogMsg::MoneyForKing(_) => Icon::Coins,
                                LogMsg::NotEnoughSpaceForDwarf => Icon::PersonAddDisabled,
                                LogMsg::Overbid(..) => Icon::Trade,
                                LogMsg::BidWon(..) => Icon::Trade,
                                LogMsg::ItemSold(..) => Icon::Trade,
                                LogMsg::ItemNotSold(..) => Icon::Trade,
                            }.draw()],
                            span![" "],
                            span![C!["time"], format!("{} ago: ", fmt_time(state.time - time, false))],
                            match msg {
                                LogMsg::Overbid(items, money, _) => {
                                    span![format!(
                                        "You have been overbid on {} for {} coins.",
                                        items
                                            .clone()
                                            .sorted_by_rarity()
                                            .into_iter()
                                            .map(|(item, n)| format!("{n}x {item}"))
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                        money
                                    )]
                                }
                                LogMsg::BidWon(items, money, _) => {
                                    span![format!(
                                        "You have successfully bought {} for {} coins.",
                                        items
                                            .clone()
                                            .sorted_by_rarity()
                                            .into_iter()
                                            .map(|(item, n)| format!("{n}x {item}"))
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                        money
                                    )]
                                }
                                LogMsg::ItemSold(items, money) => {
                                    span![format!(
                                        "You sold {} for {} coins.",
                                        items
                                            .clone()
                                            .sorted_by_rarity()
                                            .into_iter()
                                            .map(|(item, n)| format!("{n}x {item}"))
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                        money
                                    )]
                                }
                                LogMsg::ItemNotSold(items, money) => {
                                    span![format!(
                                        "You weren't able to sell {} for {} coins.",
                                        items
                                            .clone()
                                            .sorted_by_rarity()
                                            .into_iter()
                                            .map(|(item, n)| format!("{n}x {item}"))
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                        money
                                    )]
                                }
                                LogMsg::DwarfUpgrade(name, stat) => {
                                    span![format!(
                                        "Your dwarf {} has improved his {} stat while working.",
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
                                            .get_user_data(user_id)
                                            .map(|data| data.username.clone().censor())
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
            if model.history_visible {
                button![ev(Ev::Click, move |_| Msg::ToggleHistory), span!["Close"]]
            } else {
                button![
                    ev(Ev::Click, move |_| Msg::ToggleHistory),
                    span![
                        attrs! {At::AriaHidden => "true"},
                        if player.log.unread {
                            Icon::HistoryUnread.draw()
                        } else {
                            Icon::History.draw()
                        },
                        span![" Show History"]
                    ]
                ]
            }
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
        return "Nothing".to_owned();
    }

    v.into_iter().join(", ")
}

fn stars_occupation(dwarf: &Dwarf, occupation: Occupation) -> Node<Msg> {
    let s = dwarf.effectiveness_not_normalized(occupation) * 10 / MAX_EFFECTIVENESS;

    stars(s as i8, true)
}

fn stars(stars: i8, padded: bool) -> Node<Msg> {
    let mut s = Vec::new();
    for _ in 0..(stars / 2) {
        s.push(Icon::StarFull.draw());
    }
    if stars % 2 == 1 {
        s.push(if padded {
            Icon::StarHalf.draw()
        } else {
            Icon::StarHalf.draw()
        });
    }
    if padded {
        for _ in 0..((10 - stars) / 2) {
            s.push(Icon::StarEmpty.draw());
        }
    }
    span![
        C!["stars"],
        attrs! { At::Role => "meter", At::AriaValueNow => (stars as f64 / 2.0), At::AriaValueMin => 0.0, At::AriaValueMax => 5.0, At::AriaLabel => format!("{}/{}", stars as f64 / 2.0, 5.0)},
        span![attrs! { At::AriaHidden => "true" }, s]
    ]
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
        C!["ingame"],
        /*div![
            C!["nav-section"],
            a![C!["button"], attrs! {At::Href => "/"}, "Home"],
            a![
                C!["button", "disabled"],
                attrs! {At::Href => "/game"},
                "Play"
            ],
            a![C!["button"], attrs! {At::Href => "/wiki"}, "Wiki"],
            a![C!["button"], attrs! {At::Href => "/valhalla"}, "Valhalla"],
            a![C!["button"], attrs! {At::Href => "/account"}, "Account"],
            a![C!["button"], attrs! {At::Href => "/store"}, "Store"],
            a![C!["button"], attrs! {At::Href => "/about"}, "About"],
        ],*/
        div![
            C!["nav-section", "ingame"],
            a![
                C![
                    "button",
                    if let Page::Base = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => model.base_path(), At::AriaLabel => "Settlement"},
                span![C!["nav-image"], Icon::Settlement.draw()],
                span![C!["nav-description"], " Settlement"]
            ],
            a![
                C![
                    "button",
                    if let Page::Manager = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/manager", model.base_path()), At::AriaLabel => "Manager"},
                span![C!["nav-image"], Icon::Manager.draw()],
                span![C!["nav-description"], " Manager"]
            ],
            a![
                C![
                    "button",
                    if let Page::Dwarfs(DwarfsMode::Overview) = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/dwarfs", model.base_path()), At::AriaLabel => "Dwarfs"},
                span![C!["nav-image"], Icon::Dwarfs.draw()],
                span![C!["nav-description"], " Dwarfs"]
            ],
            a![
                C![
                    "button",
                    if let Page::Inventory(InventoryMode::Overview) = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/inventory", model.base_path()), At::AriaLabel => "Inventory"},
                span![C!["nav-image"], Icon::Inventory.draw()],
                span![C!["nav-description"], " Inventory"]
            ],
            a![
                C![
                    "button",
                    if let Page::Trading = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/trading", model.base_path()), At::AriaLabel => "Market"},
                span![C!["nav-image"], Icon::Trade.draw()],
                span![C!["nav-description"], " Market"]
            ],
            a![
                C![
                    "button",
                    if let Page::Quests = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/quests", model.base_path()), At::AriaLabel => "Quests"},
                span![C!["nav-image"], Icon::Task.draw()],
                span![C!["nav-description"], " Quests"]
            ],
            a![
                C![
                    "button",
                    if let Page::Tribe = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/tribe", model.base_path()), At::AriaLabel => "Tribe"},
                span![C!["nav-image"], Icon::Tribe.draw()],
                span![C!["nav-description"], " Tribe"]
            ],
            a![
                C![
                    "button",
                    if let Page::Ranking = model.page {
                        "active disabled"
                    } else {
                        ""
                    }
                ],
                attrs! {At::Href => format!("{}/ranking", model.base_path()), At::AriaLabel => "Ranking"},
                span![C!["nav-image"], Icon::Ranking.draw()],
                span![C!["nav-description"], " Ranking"]
            ],
            /*
            a![
                C![
                    "button",
                ],
                attrs! {At::Href => format!("/account"), At::AriaLabel => "Account"},
                span![C!["nav-image"], Icon::Account.draw()],
                span![C!["nav-description"], " Account"]
            ]
            */
        ] //a![C!["button"], attrs! { At::Href => "/account"}, "Account"]
    ]
}

/*
fn tip<T: std::fmt::Display>(text: T) -> Node<Msg> {
    div![
        C!["tooltip"],
        Icon::Info.draw(),
        span![C!["tooltiptext"], format!("{}", text)]
    ]
}
*/

fn slider<F, S, D>(
    model: &Model,
    item: Item,
    slider_type: SliderType,
    name: S,
    min: u64,
    max: u64,
    f: F,
    disabled: D,
    added: Option<Node<Msg>>,
) -> Node<Msg>
where
    F: Fn(Item, u64) -> ClientEvent + Copy + 'static,
    S: Fn(u64) -> String + Copy + 'static,
    D: Fn(u64) -> bool + Copy + 'static,
{
    let max = max.max(min);

    let value = model
        .slider
        .get(&(item, slider_type))
        .copied()
        .unwrap_or(min);

    div![
        C!["slider"],
        if min == max {
            Node::Empty
        } else {
            input![
                C!["slider-range"],
                attrs! {
                    At::Type => "range",
                    At::Min => min.to_string(),
                    At::Max => max.to_string(),
                    At::Value => value,
                },
                input_ev(Ev::Input, move |v| {
                    let v = v.parse().unwrap_or(min).min(max).max(min);
                    Msg::SetSlider(item, slider_type, v)
                }),
            ]
        },
        if min == max {
            Node::Empty
        } else {
            input![
                C!["slider-number"],
                attrs! {
                    At::Type => "number",
                    At::Min => min.to_string(),
                    At::Max => max.to_string(),
                    At::Value => value,
                },
                input_ev(Ev::Input, move |v| {
                    let v = v.parse().unwrap_or(min).min(max).max(min);
                    Msg::SetSlider(item, slider_type, v)
                }),
            ]
        },
        button![
            attrs! { At::Disabled => disabled(value).as_at_value() },
            C!["slider-confirm"],
            name(value),
            ev(Ev::Click, move |_| Msg::send_event(f(item, value))),
        ],
        if let Some(added) = added {
            added
        } else {
            Node::Empty
        }
    ]
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
