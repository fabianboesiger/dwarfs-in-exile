mod items;

pub use items::*;

use engine_shared::{
    utils::custom_map::{CustomMap, CustomSet},
    Event,
};
use enum_iterator::Sequence;
use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, hash::Hash, ops::Deref};
use strum::Display;

#[cfg(not(debug_assertions))]
pub const SPEED: u64 = 1;
#[cfg(debug_assertions)]
pub const SPEED: u64 = 20;
pub const ONE_MINUTE: u64 = 60;
pub const ONE_HOUR: u64 = ONE_MINUTE * 60;
pub const ONE_DAY: u64 = ONE_HOUR * 24;
pub const MAX_HEALTH: Health = ONE_DAY * 3;
pub const LOOT_CRATE_COST: Money = 1000;
pub const FREE_LOOT_CRATE: u64 = ONE_DAY;
pub const WINNER_NUM_PREMIUM_DAYS: i64 = 30;
pub const FEMALE_PROBABILITY: f64 = 1.0 / 3.0;
pub const MAX_LEVEL: u64 = 100;
pub const AGE_SECONDS_PER_TICK: u64 = 365 * 24;
pub const ADULT_AGE: u64 = 20;
pub const DEATH_AGE: u64 = 200;
pub const IMPROVEMENT_DURATION: u32 = ONE_DAY as u32 * 5;
pub const APPRENTICE_EFFECTIVENESS_DIVIDER: u64 = 10;
pub const MAX_EFFECTIVENESS: u64 = 6000;
pub const MIN_MAX_DWARF_DIFFERENCE: u64 = 3;
pub const TRADE_MONEY_MULTIPLIER: u64 = 10;
pub const DISMANTLING_DIVIDER: u64 = 2;
pub const NEW_PLAYER_DIVIDER: u64 = 8;
pub const JOIN_TRIBE_LEVEL: u64 = 2;

pub type Money = u64;
pub type Food = u64;
pub type Health = u64;

pub type Time = u64;

pub type QuestId = u64;
pub type TribeId = u64;
pub type TradeId = u64;

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
pub enum Popup {
    NewDwarf(Dwarf),
    NewItems(Bundle<Item>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq, Sequence, Copy)]
pub enum WorldEvent {
    Drought,
    Flood,
    Earthquake,
    Plague,
    Tornado,
    Carnival,
    FullMoon,
    Revolution,
}

impl WorldEvent {
    fn occupation_divider(&self, occupation: Occupation) -> u64 {
        match (self, occupation) {
            (
                WorldEvent::Drought,
                Occupation::Farming | Occupation::Gathering | Occupation::Hunting,
            ) => 3,
            (
                WorldEvent::Flood,
                Occupation::Farming | Occupation::Gathering | Occupation::Fishing,
            ) => 3,
            (
                WorldEvent::Earthquake,
                Occupation::Mining | Occupation::Logging | Occupation::Rockhounding,
            ) => 3,
            (
                WorldEvent::Tornado,
                Occupation::Logging | Occupation::Farming | Occupation::Gathering,
            ) => 3,
            (WorldEvent::Carnival, _) => 2,
            (WorldEvent::FullMoon, _) => 2,
            (WorldEvent::Revolution, _) => 2,
            _ => 1,
        }
    }

    fn new_dwarfs_multiplier(&self) -> u32 {
        match self {
            WorldEvent::Earthquake => 5,
            WorldEvent::Tornado => 5,
            WorldEvent::Carnival => 7,
            _ => 1,
        }
    }
}

impl std::fmt::Display for WorldEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldEvent::Drought => write!(f, "Drought"),
            WorldEvent::Flood => write!(f, "Flood"),
            WorldEvent::Earthquake => write!(f, "Earthquake"),
            WorldEvent::Plague => write!(f, "Plague"),
            WorldEvent::Tornado => write!(f, "Tornado"),
            WorldEvent::Carnival => write!(f, "Carnival"),
            WorldEvent::FullMoon => write!(f, "Full Moon"),
            WorldEvent::Revolution => write!(f, "Revolution"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Sequence, Hash)]
pub enum TutorialStep {
    Welcome,
    Logging,
    SettlementExpansion2,
    Axe,
    SettlementExpansion3,
    Hunting,
    FoodPreparation,
    SettlementExpansion4,
    Idling,
    SettlementExpansion5,
    SettlementExpansion7,
    SettlementExpansion9,
    Quests,
    MakeLove,
}

pub enum TutorialReward {
    Money(Money),
    Items(Bundle<Item>),
    Dwarfs(usize),
}

pub enum TutorialRequirement {
    Nothing,
    Items(Bundle<Item>),
    BaseLevel(u64),
    Food(Food),
    AnyDwarfOccupation(Occupation),
    NumberOfDwarfs(usize),
}

impl TutorialRequirement {
    pub fn complete(&self, player: &Player) -> bool {
        match self {
            TutorialRequirement::Nothing => true,
            TutorialRequirement::Items(bundle) => player.inventory.items.check_remove(bundle),
            TutorialRequirement::BaseLevel(level) => player.base.curr_level >= *level,
            TutorialRequirement::Food(food) => player.base.food >= *food,
            TutorialRequirement::AnyDwarfOccupation(occupation) => player
                .dwarfs
                .values()
                .any(|dwarf| dwarf.actual_occupation() == *occupation),
            TutorialRequirement::NumberOfDwarfs(dwarfs) => player.dwarfs.len() >= *dwarfs,
        }
    }
}

impl TutorialStep {
    pub fn requires(&self) -> TutorialRequirement {
        match self {
            TutorialStep::Welcome => TutorialRequirement::Nothing,
            TutorialStep::Logging => TutorialRequirement::Items(Bundle::new().add(Item::Wood, 1)),
            TutorialStep::SettlementExpansion2 => TutorialRequirement::BaseLevel(2),
            TutorialStep::Axe => TutorialRequirement::Items(Bundle::new().add(Item::Axe, 1)),
            TutorialStep::SettlementExpansion3 => TutorialRequirement::BaseLevel(3),
            TutorialStep::Hunting => {
                TutorialRequirement::Items(Bundle::new().add(Item::RawMeat, 1))
            }
            TutorialStep::FoodPreparation => TutorialRequirement::Food(1),
            TutorialStep::SettlementExpansion4 => TutorialRequirement::BaseLevel(4),
            TutorialStep::Idling => TutorialRequirement::AnyDwarfOccupation(Occupation::Idling),
            TutorialStep::SettlementExpansion5 => TutorialRequirement::BaseLevel(5),
            TutorialStep::SettlementExpansion7 => TutorialRequirement::BaseLevel(7),
            TutorialStep::SettlementExpansion9 => TutorialRequirement::BaseLevel(9),
            TutorialStep::Quests => TutorialRequirement::NumberOfDwarfs(6),
            TutorialStep::MakeLove => TutorialRequirement::NumberOfDwarfs(7),
        }
    }

    pub fn reward(&self) -> TutorialReward {
        match self {
            TutorialStep::Welcome => TutorialReward::Money(1000),
            TutorialStep::Logging => TutorialReward::Items(Bundle::new().add(Item::Wood, 100)),
            TutorialStep::SettlementExpansion2 => {
                TutorialReward::Items(Bundle::new().add(Item::Iron, 10).add(Item::Wood, 10))
            }
            TutorialStep::Axe => TutorialReward::Items(Bundle::new().add(Item::Wood, 100)),
            TutorialStep::SettlementExpansion3 => TutorialReward::Dwarfs(1),
            TutorialStep::Hunting => TutorialReward::Items(Bundle::new().add(Item::Coal, 50)),
            TutorialStep::FoodPreparation => {
                TutorialReward::Items(Bundle::new().add(Item::CookedMeat, 50))
            }
            TutorialStep::SettlementExpansion4 => {
                TutorialReward::Items(Bundle::new().add(Item::Wood, 100))
            }
            TutorialStep::Idling => TutorialReward::Items(Bundle::new().add(Item::Wood, 100)),
            TutorialStep::SettlementExpansion5 => TutorialReward::Dwarfs(1),
            TutorialStep::SettlementExpansion7 => TutorialReward::Dwarfs(1),
            TutorialStep::SettlementExpansion9 => TutorialReward::Dwarfs(1),
            TutorialStep::Quests => TutorialReward::Money(1000),
            TutorialStep::MakeLove => TutorialReward::Money(1000),
        }
    }
}

impl std::fmt::Display for TutorialStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TutorialStep::Welcome => write!(f, "Welcome to the Exile"),
            TutorialStep::Logging => write!(f, "Into the Woods"),
            TutorialStep::SettlementExpansion2 => write!(f, "Expand Your Settlement"),
            TutorialStep::Hunting => write!(f, "A Well Fed Population"),
            TutorialStep::FoodPreparation => write!(f, "Dinner is Ready"),
            TutorialStep::SettlementExpansion3 => write!(f, "Expand Your Settlement"),
            TutorialStep::Idling => write!(f, "Time for a Break"),
            TutorialStep::Quests => write!(f, "Make new Friends"),
            TutorialStep::SettlementExpansion4 => write!(f, "Expand Your Settlement"),
            TutorialStep::SettlementExpansion5 => write!(f, "Expand Your Settlement"),
            TutorialStep::Axe => write!(f, "Craft an Axe"),
            TutorialStep::SettlementExpansion7 => write!(f, "Expand Your Settlement"),
            TutorialStep::SettlementExpansion9 => write!(f, "Expand Your Settlement"),
            TutorialStep::MakeLove => write!(f, "Make Love, Not War"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct UserId(pub i64);

impl engine_shared::UserId for UserId {}

impl From<i64> for UserId {
    fn from(id: i64) -> Self {
        UserId(id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct UserData {
    pub username: String,
    pub premium: u64,
    pub games_won: i64,
    pub admin: bool,
    pub guest: bool,
    pub joined: time::PrimitiveDateTime,
}

impl engine_shared::UserData for UserData {}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Sequence, Display, PartialEq, Eq)]
#[strum(serialize_all = "title_case")]
pub enum HireDwarfType {
    Standard,
}

impl HireDwarfType {
    pub fn cost(&self) -> u64 {
        match self {
            HireDwarfType::Standard => 5000,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct State {
    pub players: CustomMap<UserId, Player>,
    pub next_dwarf_id: DwarfId,
    pub chat: Chat,
    pub next_quest_id: QuestId,
    pub next_trade_id: TradeId,
    pub quests: CustomMap<QuestId, Quest>,
    pub time: Time,
    pub king: Option<UserId>,
    #[serde(default)]
    pub event: Option<WorldEvent>,
    pub trade_deals: CustomMap<TradeId, TradeDeal>,
    pub tribes: CustomMap<TribeId, Tribe>,
}

impl Default for State {
    fn default() -> Self {
        let mut tribes = CustomMap::default();
        tribes.insert(0, Tribe::default());
        tribes.insert(1, Tribe::default());
        tribes.insert(2, Tribe::default());


        Self {
            players: CustomMap::default(),
            next_dwarf_id: 0,
            chat: Chat::default(),
            next_quest_id: 0,
            next_trade_id: 0,
            quests: CustomMap::default(),
            time: 0,
            king: None,
            event: None,
            trade_deals: CustomMap::default(),
            tribes,
        }
    }
}

impl State {
    fn add_to_food_storage(player: &mut Player, item: Item, qty: u64) {
        if let Some(food) = item.nutritional_value() {
            if player
                .inventory
                .items
                .remove_checked(Bundle::new().add(item, qty))
            {
                player.base.food += food * qty;
            }
        }
    }

    /*
    fn sell(player: &mut Player, item: Item, qty: u64) {
        if item.money_value(1) > 0 {
            if player
                .inventory
                .items
                .remove_checked(Bundle::new().add(item, qty))
            {
                player.money += item.money_value(1) * qty;
            }
        }
    }
    */

    fn craft(player: &mut Player, item: Item, qty: u64) {
        if let Some((level, requires)) = item.requires() {
            if player.base.curr_level >= level && player.inventory.items.remove_checked(requires.mul(qty)) {
                player
                    .inventory
                    .items
                    .add_checked(Bundle::new().add(item, qty));
            }
        }
    }

    fn dismantle(player: &mut Player, item: Item, qty: u64) {
        if let Some((_level, requires)) = item.requires() {
            if matches!(
                item.item_type(),
                Some(ItemType::Tool | ItemType::Jewelry | ItemType::Clothing)
            ) && player
                    .inventory
                    .items
                    .remove_checked(Bundle::new().add(item, qty)) {
                player
                    .inventory
                    .items
                    .add_checked(requires.mul(qty).div(DISMANTLING_DIVIDER));
            }
        }
    }
}

impl engine_shared::State for State {
    type ServerEvent = ServerEvent;
    type ClientEvent = ClientEvent;
    type UserId = UserId;
    type UserData = UserData;

    const DURATION_PER_TICK: std::time::Duration = std::time::Duration::from_millis(1000 / SPEED);

    fn has_winner(&self) -> Option<UserId> {
        let mut winner = None;
        for (user_id, player) in &self.players {
            if player.base.curr_level == 100 && self.king == Some(*user_id) {
                winner = Some(*user_id);
            }
        }
        winner
    }

    fn update(
        &mut self,
        rng: &mut impl Rng,
        event: Event<Self>,
        user_data: &CustomMap<UserId, UserData>,
    ) {
        let update_result = move || -> Option<()> {
            match event {
                Event::ClientEvent(event, user_id) => {
                    if !self.players.contains_key(&user_id) {
                        self.players.insert(
                            user_id,
                            Player::new(self.time, rng, &mut self.next_dwarf_id),
                        );
                    }
                    let player = self.players.get_mut(&user_id)?;
                    player.last_online = self.time;

                    let is_premium = user_data
                        .get(&user_id)
                        .map(|user_data| user_data.premium > 0)
                        .unwrap_or(false);

                    match event {
                        ClientEvent::Init => {}
                        ClientEvent::Bid(trade_id) => {
                            if let Some(trade) = self.trade_deals.get_mut(&trade_id) {
                                trade.bid(&mut self.players, user_id, self.time)?;
                            }
                        }
                        ClientEvent::SetMentor(apprentice_id, mentor_id) => {
                            if let Some(mentor_id) = mentor_id {
                                let apprentice = player.dwarfs.get(&apprentice_id)?;
                                if apprentice.is_adult() {
                                    return None;
                                }
                                let mentor = player.dwarfs.get(&mentor_id)?;
                                if !mentor.is_adult() {
                                    return None;
                                }
                            }
                            player.set_mentor(apprentice_id, mentor_id)?;
                        }
                        ClientEvent::ReleaseDwarf(dwarf_id) => {
                            let dwarf = player.dwarfs.get_mut(&dwarf_id)?;
                            dwarf.released = true;
                        }
                        ClientEvent::ToggleManualManagement(dwarf_id) => {
                            let dwarf = player.dwarfs.get_mut(&dwarf_id)?;
                            dwarf.manual_management = !dwarf.manual_management;
                        }
                        ClientEvent::SetDwarfName(dwarf_id, name) => {
                            let dwarf = player.dwarfs.get_mut(&dwarf_id)?;
                            let name = name.trim();
                            if name.is_empty() {
                                dwarf.custom_name = None;
                            } else {
                                dwarf.custom_name = Some(name.to_string());
                            }
                        }
                        ClientEvent::Optimize(to_optimize_dwarf_id) => {
                            // Reassign occupations.

                            if to_optimize_dwarf_id.is_none() {
                                player.set_manager();

                                let mut occupations_to_fill = player.manager.clone();
                                occupations_to_fill.swap_remove(&Occupation::Idling);

                                for dwarf in player.dwarfs.values_mut() {
                                    if dwarf.can_be_managed() {
                                        dwarf.occupation = Occupation::Idling;
                                    }
                                }

                                loop {
                                    occupations_to_fill.retain(|_, num| *num > 0);

                                    if occupations_to_fill.is_empty() {
                                        break;
                                    }

                                    let mut best_dwarf_effectiveness = 0;
                                    let mut best_dwarf_occupation = None;
                                    let mut best_dwarf_id = None;
                                    for (dwarf_id, dwarf) in &player.dwarfs {
                                        if dwarf.occupation == Occupation::Idling
                                            && dwarf.can_be_managed()
                                        {
                                            for (occupation, _num) in &occupations_to_fill {
                                                let effectiveness =
                                                    dwarf.stats.cross(occupation.requires_stats());
                                                if effectiveness >= best_dwarf_effectiveness {
                                                    best_dwarf_effectiveness = effectiveness;
                                                    best_dwarf_id = Some(*dwarf_id);
                                                    best_dwarf_occupation = Some(*occupation);
                                                }
                                            }
                                        }
                                    }

                                    if let Some(best_dwarf_id) = best_dwarf_id {
                                        let best_dwarf = player.dwarfs.get_mut(&best_dwarf_id)?;
                                        let best_dwarf_occupation = best_dwarf_occupation
                                            .expect("occupation known if id is known");
                                        best_dwarf.change_occupation(best_dwarf_occupation);
                                        *occupations_to_fill
                                            .get_mut(&best_dwarf_occupation)
                                            .expect("occupation is always one that is to fill") -=
                                            1;
                                    } else {
                                        break;
                                    }
                                }

                                debug_assert!(occupations_to_fill.is_empty());
                            }

                            // Reassign equipment.

                            let dwarf_ids = if let Some(dwarf_id) = to_optimize_dwarf_id {
                                vec![dwarf_id]
                            } else {
                                player.dwarfs.keys().cloned().collect()
                            };

                            for dwarf_id in &dwarf_ids {
                                let dwarf = player.dwarfs.get_mut(dwarf_id)?;
                                if dwarf.can_be_managed() || to_optimize_dwarf_id.is_some() {
                                    for (_, item) in dwarf.equipment.drain(..) {
                                        player
                                            .inventory
                                            .items
                                            .add_checked(Bundle::new().add(item, 1));
                                    }
                                }
                            }

                            loop {
                                let mut best_dwarf_effectiveness = 0;
                                let mut best_dwarf_id = None;
                                let mut best_dwarf_item = None;
                                for dwarf_id in &dwarf_ids {
                                    let dwarf = player.dwarfs.get(dwarf_id)?;
                                    if dwarf.can_be_managed() || to_optimize_dwarf_id.is_some() {
                                        let occupation_to_optimize =
                                            if to_optimize_dwarf_id.is_some() {
                                                dwarf.actual_occupation()
                                            } else {
                                                dwarf.occupation
                                            };

                                        for (item, _) in
                                            player.inventory.items.iter().filter(|(item, num)| {
                                                **num > 0
                                                    && item
                                                        .item_type()
                                                        .map(|item_type| item_type.equippable())
                                                        .unwrap_or(false)
                                                    && dwarf
                                                        .equipment
                                                        .get(&item.item_type().expect(
                                                            "equippables always have item types",
                                                        ))
                                                        .is_none()
                                            })
                                        {
                                            let mut dwarf_clone = dwarf.clone();

                                            let effectiveness_before = dwarf_clone
                                                .effectiveness_not_normalized(
                                                    occupation_to_optimize,
                                                );

                                            dwarf_clone.equipment.insert(
                                                item.item_type()
                                                    .expect("equippables always have item types"),
                                                *item,
                                            );

                                            let effectiveness_after = dwarf_clone
                                                .effectiveness_not_normalized(
                                                    occupation_to_optimize,
                                                );

                                            let effectiveness_diff = effectiveness_after as i64
                                                - effectiveness_before as i64;

                                            if effectiveness_diff > best_dwarf_effectiveness {
                                                best_dwarf_effectiveness = effectiveness_diff;
                                                best_dwarf_item = Some(*item);
                                                best_dwarf_id = Some(*dwarf_id);
                                            }
                                        }
                                    }
                                }

                                if let Some(best_dwarf_id) = best_dwarf_id {
                                    let best_dwarf = player.dwarfs.get_mut(&best_dwarf_id)?;
                                    let best_dwarf_item =
                                        best_dwarf_item.expect("item known if id is known");

                                    if player
                                        .inventory
                                        .items
                                        .remove_checked(Bundle::new().add(best_dwarf_item, 1))
                                    {
                                        best_dwarf.equipment.insert(
                                            best_dwarf_item
                                                .item_type()
                                                .expect("equippables always have item types"),
                                            best_dwarf_item,
                                        );
                                    } else if cfg!(debug_assertions) {
                                        panic!("something went wrong!");
                                    }
                                } else {
                                    break;
                                }
                            }
                        }
                        ClientEvent::SetManagerOccupation(occupation, num) => {
                            player.set_manager();

                            let curr = player.manager.get(&occupation).copied().unwrap_or_default();
                            if curr < num {
                                let diff = num - curr;
                                if player
                                    .manager
                                    .get(&Occupation::Idling)
                                    .copied()
                                    .unwrap_or_default()
                                    >= diff
                                {
                                    player.manager.insert(occupation, num);
                                    *player.manager.entry(Occupation::Idling).or_default() -= diff;
                                }
                            } else {
                                let diff = curr - num;
                                player.manager.insert(occupation, num);
                                *player.manager.entry(Occupation::Idling).or_default() += diff;
                            }
                        }
                        ClientEvent::ConfirmPopup => {
                            player.popups.pop_front();
                        }
                        ClientEvent::NextTutorialStep => {
                            if let Some(step) = player.tutorial_step {
                                if step.requires().complete(player) {
                                    match step.reward() {
                                        TutorialReward::Money(money) => {
                                            player.money += money;
                                        }
                                        TutorialReward::Items(bundle) => {
                                            player.inventory.add(bundle, self.time);
                                        }
                                        TutorialReward::Dwarfs(num) => {
                                            for _ in 0..num {
                                                player.new_dwarf(
                                                    rng,
                                                    &mut self.next_dwarf_id,
                                                    self.time,
                                                    Some(Stats::default()),
                                                );
                                            }
                                        }
                                    }
                                    player.tutorial_step = step.next();
                                }
                            }
                        }
                        ClientEvent::HireDwarf(_dwarf_type) => {
                            /*if player.money >= dwarf_type.cost()
                                && player.dwarfs.len() < player.base.max_dwarfs()
                            {
                                player.money -= dwarf_type.cost();
                                player.new_dwarf(rng, &mut self.next_dwarf_id, self.time, false);
                            }*/
                        }
                        ClientEvent::ToggleAutoCraft(item) => {
                            if is_premium {
                                if player.auto_functions.auto_craft.contains(&item) {
                                    player.auto_functions.auto_craft.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_craft.insert(item);
                                    player.auto_functions.auto_dismantle.swap_remove(&item);
                                    player.auto_craft(self.time, is_premium);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoDismantle(item) => {
                            if is_premium {
                                if player.auto_functions.auto_dismantle.contains(&item) {
                                    player.auto_functions.auto_dismantle.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_dismantle.insert(item);
                                    player.auto_functions.auto_craft.swap_remove(&item);
                                    player.auto_dismantle(self.time, is_premium);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoStore(item) => {
                            if is_premium {
                                if player.auto_functions.auto_store.contains(&item) {
                                    player.auto_functions.auto_store.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_store.insert(item);
                                    player.auto_store(is_premium);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoSell(_item) => {
                            /*if is_premium {
                                if player.auto_functions.auto_sell.contains(&item) {
                                    player.auto_functions.auto_sell.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_sell.insert(item);
                                }
                            }*/
                        }
                        ClientEvent::ToggleAutoIdle => {
                            //if is_premium {
                            player.auto_functions.auto_idle = !player.auto_functions.auto_idle;
                            //}
                        }
                        ClientEvent::Restart => {
                            if player.dwarfs.len() == 0 {
                                let player = Player::new(self.time, rng, &mut self.next_dwarf_id);
                                self.players.insert(user_id, player);
                            }
                        }
                        ClientEvent::Message(message) => {
                            self.chat
                                .add_message(&mut self.players, user_id, message, self.time);
                        }
                        ClientEvent::ChangeOccupation(dwarf_id, occupation) => {
                            let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                            if dwarf.participates_in_quest.is_none()
                                && player.base.curr_level >= occupation.unlocked_at_level()
                                && dwarf.is_adult()
                            {
                                dwarf.change_occupation(occupation);
                            }
                        }
                        ClientEvent::Craft(item, qty) => {
                            Self::craft(player, item, qty);
                        }
                        ClientEvent::Dismantle(item, qty) => {
                            Self::dismantle(player, item, qty);
                        }
                        ClientEvent::UpgradeBase => {
                            if let Some(requires) = player.base.upgrade_cost() {
                                if player.inventory.items.remove_checked(requires) {
                                    player.base.upgrade();
                                }
                            }
                        }
                        ClientEvent::ChangeEquipment(dwarf_id, item_type, item) => {
                            let equipment = &mut player.dwarfs.get_mut(&dwarf_id)?.equipment;

                            let old_item = if let Some(item) = item {
                                if item
                                    .item_type()
                                    .as_ref()
                                    .map(ItemType::equippable)
                                    .unwrap_or(false)
                                    && item.item_type().unwrap() == item_type
                                    && player
                                        .inventory
                                        .items
                                        .remove_checked(Bundle::new().add(item, 1))
                                {
                                    equipment.insert(item_type, item)
                                } else {
                                    None
                                }
                            } else {
                                equipment.swap_remove(&item_type)
                            };

                            if let Some(old_item) = old_item {
                                player
                                    .inventory
                                    .items
                                    .add_checked(Bundle::new().add(old_item, 1));
                            }
                        }
                        ClientEvent::OpenLootCrate => {
                            /*if player.money >= LOOT_CRATE_COST {
                                player.money -= LOOT_CRATE_COST;
                                player.open_loot_crate(rng, self.time);
                            }*/
                        }
                        ClientEvent::OpenDailyReward => {
                            /*if player.reward_time <= self.time {
                                player.reward_time = self.time + FREE_LOOT_CRATE;
                                player.open_loot_crate(rng, self.time);
                            }*/
                        }
                        ClientEvent::AssignToQuest(quest_id, dwarf_idx, dwarf_id) => {
                            if let Some(dwarf_id) = dwarf_id {
                                let quest = self.quests.get(&quest_id)?;
                                if player.base.curr_level > quest.max_level
                                    || player.base.curr_level < quest.min_level
                                {
                                    return None;
                                }

                                let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                                if dwarf.is_adult() {
                                    if let Some((_, old_quest_id, old_dwarf_idx)) =
                                        dwarf.participates_in_quest
                                    {
                                        let old_quest = self.quests.get_mut(&old_quest_id)?;
                                        let old_contestant =
                                            old_quest.contestants.entry(user_id).or_default();
                                        old_contestant.dwarfs.swap_remove(&old_dwarf_idx);
                                    }

                                    let quest = self.quests.get_mut(&quest_id)?;
                                    let contestant = quest.contestants.entry(user_id).or_default();

                                    dwarf.participates_in_quest =
                                        Some((quest.quest_type, quest_id, dwarf_idx));

                                    if dwarf_idx < quest.quest_type.max_dwarfs() {
                                        let old_dwarf_id =
                                            contestant.dwarfs.insert(dwarf_idx, dwarf_id);
                                        if let Some(old_dwarf_id) = old_dwarf_id {
                                            let dwarf = player.dwarfs.get_mut(&old_dwarf_id)?;
                                            dwarf.participates_in_quest = None;
                                        }
                                    }
                                }
                            } else {
                                let quest = self.quests.get_mut(&quest_id)?;
                                let contestant = quest.contestants.entry(user_id).or_default();

                                let old_dwarf_id = contestant.dwarfs.swap_remove(&dwarf_idx);

                                if let Some(old_dwarf_id) = old_dwarf_id {
                                    let dwarf = player.dwarfs.get_mut(&old_dwarf_id)?;
                                    dwarf.participates_in_quest = None;
                                }
                            }
                        }
                        ClientEvent::AddToFoodStorage(item, qty) => {
                            Self::add_to_food_storage(player, item, qty);
                        }
                        ClientEvent::Sell(item, qty) => {
                            if qty > 0 {
                                /*
                                if let Some(trade_deal) =
                                    TradeDeal::from_player(user_id, player, item, qty)
                                {
                                    self.trade_deals.push(trade_deal);
                                }
                                */
                                let qty = qty.min(player.inventory.items.get(&item).copied().unwrap_or(0));
                                let items = Bundle::new().add(item, qty);
                                let next_bid = item.money_value(qty) * TRADE_MONEY_MULTIPLIER;

                                if qty == 0 {
                                    return None;
                                }

                                if next_bid == 0 {
                                    return None;
                                }

                                if !player.inventory.items.remove_checked(items.clone()) {
                                    return None;
                                }

                                player.money += next_bid;
                            }
                        }
                        ClientEvent::ReadLog => {
                            player.log.unread = false;
                        }
                        ClientEvent::ReadChat => {
                            player.chat_unread = false;
                        }
                    }
                }
                Event::ServerEvent(event) => {
                    match event {
                        ServerEvent::Tick => {
                            self.time += 1;

                            if matches!(self.event, Some(WorldEvent::Revolution)) {
                                self.king = None;
                            };

                            if self.event.is_some() {
                                if rng.gen_ratio(1, ONE_DAY as u32 / 4) {
                                    self.event = None;
                                }
                            } else if rng.gen_ratio(1, ONE_DAY as u32 / 4) {
                                self.event = Some(enum_iterator::all().choose(rng).unwrap());
                            }

                            let fewest_members_tribe_id = *self
                                        .tribes
                                        .keys()
                                        .map(|tribe_id| (tribe_id, self.players.values().filter(|player| player.is_active(self.time) && player.tribe == Some(*tribe_id)).count()))
                                        .min_by_key(|(_, count)| *count)
                                        .unwrap().0;

                            for (user_id, player) in self.players.iter_mut() {
                                let is_premium = user_data
                                    .get(user_id)
                                    .map(|user_data| user_data.premium > 0)
                                    .unwrap_or(false);

                                // Build the base.
                                if let Some(JOIN_TRIBE_LEVEL) = player.base.build() {
                                    player.tribe = Some(fewest_members_tribe_id);
                                }

                                // Chance for a new dwarf!
                                let controlled_territories = enum_iterator::all::<Territory>()
                                    .filter(|territory| {
                                        if let Some(tribe_id) = player.tribe {
                                            let score = self.tribes.get(&tribe_id).unwrap().territories.get(territory).copied().unwrap_or(0);
                                            let scores = self.tribes
                                                .values()
                                                .map(|tribe| {
                                                    tribe.territories.get(territory).copied().unwrap_or_default()
                                                })
                                                .collect::<Vec<_>>();

                                            let under_control = scores
                                                .iter()
                                                .filter(|s| **s > score)
                                                .count() == 0;
                                            
                                            under_control
                                        } else {
                                            false
                                        }
                                    })
                                    .map(|territory| territory.provides_stats())
                                    .collect::<Vec<_>>();

                                if rng.gen_ratio(
                                    self.event
                                        .map(|event| event.new_dwarfs_multiplier())
                                        .unwrap_or(1),
                                    (ONE_DAY as u32 * 5) / (controlled_territories.len() as u32 + 1),
                                ) {
                                    let added_stats = controlled_territories.choose(rng).cloned().unwrap_or(Stats::default());

                                    player.new_dwarf(
                                        rng,
                                        &mut self.next_dwarf_id,
                                        self.time,
                                        Some(added_stats),
                                    );
                                }

                                let male_idle_dwarfs = player
                                    .dwarfs
                                    .values()
                                    .filter(|dwarf| {
                                        dwarf.occupation == Occupation::Idling
                                            && dwarf.is_adult()
                                            && !dwarf.is_female
                                    })
                                    .count();

                                let female_idle_dwarfs = player
                                    .dwarfs
                                    .values()
                                    .filter(|dwarf| {
                                        dwarf.occupation == Occupation::Idling
                                            && dwarf.is_adult()
                                            && dwarf.is_female
                                    })
                                    .count();

                                // Chance for a new baby dwarf!
                                let baby_dwarf_multiplier =
                                    if matches!(self.event, Some(WorldEvent::FullMoon)) {
                                        5
                                    } else {
                                        1
                                    };
                                if rng.gen_ratio(
                                    male_idle_dwarfs.min(female_idle_dwarfs) as u32
                                        * baby_dwarf_multiplier,
                                    ONE_DAY as u32 / 4,
                                ) {
                                    player.new_dwarf(rng, &mut self.next_dwarf_id, self.time, None);
                                }

                                let mut became_adult = CustomSet::new();

                                // Let the dwarfs eat!
                                let health_cost_multiplier = match self.event {
                                    Some(WorldEvent::Plague) => {
                                        (1 + player.dwarfs.len() as u64 / 20).min(5)
                                    }
                                    _ => 1,
                                };
                                let mut sorted_by_health =
                                    player.dwarfs.iter_mut().collect::<Vec<_>>();
                                sorted_by_health.sort_by_key(|(_, dwarf)| dwarf.health);
                                for (dwarf_id, dwarf) in sorted_by_health {
                                    dwarf.decr_health(
                                        dwarf.actual_occupation().health_cost_per_tick()
                                            * health_cost_multiplier,
                                    );
                                    if dwarf.actual_occupation() == Occupation::Idling {
                                        if player.base.food > 0 {
                                            if dwarf.health <= MAX_HEALTH - MAX_HEALTH / 1000 {
                                                player.base.food -= 1;
                                                dwarf.incr_health(MAX_HEALTH / 1000);
                                            } else if player.auto_functions.auto_idle {
                                                dwarf.auto_idle = false;
                                            }
                                        } else if dwarf.auto_idle {
                                            dwarf.auto_idle = false;
                                        }
                                    } else if player.auto_functions.auto_idle
                                        && dwarf.health <= MAX_HEALTH / 5
                                        && dwarf.occupation != Occupation::Idling
                                        && player.base.food > 0
                                    {
                                        dwarf.auto_idle = true;
                                    }

                                    if !dwarf.dead() {
                                        let is_adult_before = dwarf.is_adult();
                                        dwarf.age_seconds += AGE_SECONDS_PER_TICK;

                                        if dwarf.age_years() > 200 && rng.gen_ratio(1, ONE_DAY as u32 * 5) {
                                            dwarf.health = 0;
                                        }
                                        if !is_adult_before && dwarf.is_adult() {
                                            became_adult.insert(*dwarf_id);
                                        }
                                    }
                                }

                                // Let the dwarfs improve and handle deaths.
                                let ids = player.dwarfs.keys().cloned().collect::<Vec<_>>();
                                for dwarf_id in ids {
                                    let dwarf = player.dwarfs.get(&dwarf_id)?;
                                    let (improvement_occupation, improvement_multiplier) = if dwarf
                                        .is_adult()
                                    {
                                        (dwarf.actual_occupation(), 1)
                                    } else if let Some(mentor_id) = dwarf.mentor {
                                        if let Some(mentor) = player.dwarfs.get(&mentor_id) {
                                            if mentor.is_adult() {
                                                (
                                                    mentor.actual_occupation(),
                                                    APPRENTICE_EFFECTIVENESS_DIVIDER,
                                                )
                                            } else {
                                                (
                                                    Occupation::Idling,
                                                    APPRENTICE_EFFECTIVENESS_DIVIDER,
                                                )
                                            }
                                        } else {
                                            (
                                                Occupation::Idling,
                                                APPRENTICE_EFFECTIVENESS_DIVIDER,
                                            )
                                        }
                                    } else {
                                        (Occupation::Idling, APPRENTICE_EFFECTIVENESS_DIVIDER)
                                    };

                                    let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                                    if !dwarf.dead() {
                                        if rng.gen_ratio(
                                            improvement_occupation.requires_stats().agility as u32
                                                * improvement_multiplier as u32,
                                            IMPROVEMENT_DURATION,
                                        ) && dwarf.stats.agility < 10
                                        {
                                            dwarf.stats.agility += 1;
                                            player.log.add(
                                                self.time,
                                                LogMsg::DwarfUpgrade(
                                                    dwarf.actual_name().to_owned(),
                                                    "agility".to_string(),
                                                ),
                                            );
                                        }
                                        if rng.gen_ratio(
                                            improvement_occupation.requires_stats().endurance
                                                as u32
                                                * improvement_multiplier as u32,
                                            IMPROVEMENT_DURATION,
                                        ) && dwarf.stats.endurance < 10
                                        {
                                            dwarf.stats.endurance += 1;
                                            player.log.add(
                                                self.time,
                                                LogMsg::DwarfUpgrade(
                                                    dwarf.actual_name().to_owned(),
                                                    "endurance".to_string(),
                                                ),
                                            );
                                        }
                                        if rng.gen_ratio(
                                            improvement_occupation.requires_stats().strength as u32
                                                * improvement_multiplier as u32,
                                            IMPROVEMENT_DURATION,
                                        ) && dwarf.stats.strength < 10
                                        {
                                            dwarf.stats.strength += 1;
                                            player.log.add(
                                                self.time,
                                                LogMsg::DwarfUpgrade(
                                                    dwarf.actual_name().to_owned(),
                                                    "strength".to_string(),
                                                ),
                                            );
                                        }
                                        if rng.gen_ratio(
                                            improvement_occupation.requires_stats().intelligence
                                                as u32
                                                * improvement_multiplier as u32,
                                            IMPROVEMENT_DURATION,
                                        ) && dwarf.stats.intelligence < 10
                                        {
                                            dwarf.stats.intelligence += 1;
                                            player.log.add(
                                                self.time,
                                                LogMsg::DwarfUpgrade(
                                                    dwarf.actual_name().to_owned(),
                                                    "intelligence".to_string(),
                                                ),
                                            );
                                        }
                                        if rng.gen_ratio(
                                            improvement_occupation.requires_stats().perception
                                                as u32
                                                * improvement_multiplier as u32,
                                            IMPROVEMENT_DURATION,
                                        ) && dwarf.stats.perception < 10
                                        {
                                            dwarf.stats.perception += 1;
                                            player.log.add(
                                                self.time,
                                                LogMsg::DwarfUpgrade(
                                                    dwarf.actual_name().to_owned(),
                                                    "perception".to_string(),
                                                ),
                                            );
                                        }
                                    }
                                }

                                // Let the dwarfs work!
                                let mut added_items = Bundle::new();
                                for dwarf in player.dwarfs.values() {
                                    if !dwarf.dead() && dwarf.is_adult() {
                                        for item in enum_iterator::all::<Item>() {
                                            if let Some(ItemProbability {
                                                expected_ticks_per_drop,
                                            }) =
                                                item.item_probability(dwarf.actual_occupation())
                                            {
                                                if dwarf.gen_ratio_effectiveness(
                                                    &player.dwarfs,
                                                    rng,
                                                    expected_ticks_per_drop
                                                        * self
                                                            .event
                                                            .as_ref()
                                                            .map(|f| {
                                                                f.occupation_divider(
                                                                    dwarf.actual_occupation(),
                                                                )
                                                            })
                                                            .unwrap_or(1),
                                                ) {
                                                    added_items = added_items.add(item, 1);
                                                }
                                            }
                                        }
                                    }
                                }
                                player.add_items(added_items, self.time, is_premium);

                                // Handle dwarfs that became adult
                                for dwarf_id in became_adult {
                                    player.set_mentor(dwarf_id, None);

                                    let dwarf: &Dwarf = player.dwarfs.get(&dwarf_id)?;
                                    player.log.add(
                                        self.time,
                                        LogMsg::DwarfIsAdult(dwarf.actual_name().to_owned()),
                                    );
                                }

                                // Handle removed dwarfs
                                let removed_dwarfs = player
                                    .dwarfs
                                    .iter()
                                    .filter(|(_, dwarf)| dwarf.dead() || dwarf.released)
                                    .map(|(id, _)| id)
                                    .copied()
                                    .collect::<CustomSet<DwarfId>>();
                                for dwarf_id in removed_dwarfs {
                                    let apprentice_id: Option<DwarfId> =
                                        player.dwarfs.get(&dwarf_id)?.apprentice;

                                    if let Some(apprentice_id) = apprentice_id {
                                        player.set_mentor(apprentice_id, None);
                                    }
                                    player.set_mentor(dwarf_id, None);

                                    let dwarf: &Dwarf = player.dwarfs.get(&dwarf_id)?;
                                    // Send log message that dwarf died.
                                    player.log.add(
                                        self.time,
                                        LogMsg::DwarfDied(dwarf.actual_name().to_owned()),
                                    );
                                    // Add the equipment to the inventory.
                                    for (_, item) in &dwarf.equipment {
                                        player
                                            .inventory
                                            .items
                                            .add_checked(Bundle::new().add(*item, 1));
                                    }
                                }
                                // Remove dead dwarfs from quests.
                                for quest in self.quests.values_mut() {
                                    if let Some(contestant) = quest.contestants.get_mut(user_id) {
                                        contestant.dwarfs.retain(|_, dwarf_id| {
                                            !player
                                                .dwarfs
                                                .get(&*dwarf_id)
                                                .map(|d| d.dead() || d.released)
                                                .unwrap_or(true)
                                        });
                                    }
                                }

                                // Remove dead dwarfs from the base.
                                player
                                    .dwarfs
                                    .retain(|_, dwarf| !(dwarf.dead() || dwarf.released));
                            }

                            // Continue the active quests.
                            for quest in self.quests.values_mut() {
                                quest.run(&self.players)?;

                                if quest.done() {
                                    match quest.quest_type.reward_mode() {
                                        RewardMode::BestGetsAll(money) => {
                                            if let Some(user_id) = quest.best() {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    if self.king.is_some() {
                                                        player.money += money * 9 / 10;
                                                    } else {
                                                        player.money += money;
                                                    }
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedMoney(
                                                            quest.quest_type,
                                                            money,
                                                        ),
                                                    );
                                                }
                                                if let Some(king) = self.king {
                                                    if let Some(player) =
                                                        self.players.get_mut(&king)
                                                    {
                                                        player.money += money / 10;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::MoneyForKing(money / 10),
                                                        );
                                                    }
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedMoney(
                                                                quest.quest_type,
                                                                0,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::BecomeKing => {
                                            if let Some(user_id) = quest.best() {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    if !matches!(
                                                        self.event,
                                                        Some(WorldEvent::Revolution)
                                                    ) {
                                                        self.king = Some(user_id);
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedKing(
                                                                quest.quest_type,
                                                                true,
                                                            ),
                                                        );
                                                    }
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if Some(*contestant_id) != self.king {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedKing(
                                                                quest.quest_type,
                                                                false,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::SplitFairly(money) => {
                                            for (user_id, money) in
                                                quest.split_by_score(if self.king.is_some() {
                                                    money * 9 / 10
                                                } else {
                                                    money
                                                })
                                            {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    player.money += money;
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedMoney(
                                                            quest.quest_type,
                                                            money,
                                                        ),
                                                    );
                                                }
                                            }
                                            if let Some(king) = self.king {
                                                if let Some(player) = self.players.get_mut(&king) {
                                                    player.money += money / 10;
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::MoneyForKing(money / 10),
                                                    );
                                                }
                                            }
                                        }
                                        RewardMode::BestGetsItems(items) => {
                                            if let Some(user_id) = quest.best() {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    let is_premium = user_data
                                                        .get(&user_id)
                                                        .map(|user_data| user_data.premium > 0)
                                                        .unwrap_or(false);

                                                    player.add_items(
                                                        items.clone(),
                                                        self.time,
                                                        is_premium,
                                                    );
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedItems(
                                                            quest.quest_type,
                                                            Some(items),
                                                        ),
                                                    );
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedItems(
                                                                quest.quest_type,
                                                                None,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::ItemsByChance(items) => {
                                            if let Some(user_id) = quest.chance_by_score(rng) {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    let is_premium = user_data
                                                        .get(&user_id)
                                                        .map(|user_data| user_data.premium > 0)
                                                        .unwrap_or(false);

                                                    player.add_items(
                                                        items.clone(),
                                                        self.time,
                                                        is_premium,
                                                    );
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedItems(
                                                            quest.quest_type,
                                                            Some(items),
                                                        ),
                                                    );
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedItems(
                                                                quest.quest_type,
                                                                None,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::NewDwarfByChance(num_dwarfs) => {
                                            for _ in 0..num_dwarfs {
                                                if let Some(user_id) = quest.chance_by_score(rng) {
                                                    if let Some(player) =
                                                        self.players.get_mut(&user_id)
                                                    {
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedDwarfs(
                                                                quest.quest_type,
                                                                Some(num_dwarfs),
                                                            ),
                                                        );
                                                        player.new_dwarf(
                                                            rng,
                                                            &mut self.next_dwarf_id,
                                                            self.time,
                                                            Some(Stats::default()),
                                                        );
                                                    }
                                                    for contestant_id in quest.contestants.keys() {
                                                        if *contestant_id != user_id {
                                                            let player = self
                                                                .players
                                                                .get_mut(contestant_id)?;
                                                            player.log.add(
                                                                self.time,
                                                                LogMsg::QuestCompletedDwarfs(
                                                                    quest.quest_type,
                                                                    None,
                                                                ),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::NewDwarf(num_dwarfs) => {
                                            if let Some(user_id) = quest.best() {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedDwarfs(
                                                            quest.quest_type,
                                                            Some(num_dwarfs),
                                                        ),
                                                    );
                                                    for _ in 0..num_dwarfs {
                                                        player.new_dwarf(
                                                            rng,
                                                            &mut self.next_dwarf_id,
                                                            self.time,
                                                            Some(Stats::default()),
                                                        );
                                                    }
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedDwarfs(
                                                                quest.quest_type,
                                                                None,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    for (contestant_id, contestant) in quest.contestants.iter() {
                                        let player = self.players.get_mut(contestant_id)?;
                                        for dwarf_id in contestant.dwarfs.values() {
                                            let dwarf = player.dwarfs.get_mut(dwarf_id)?;
                                            dwarf.participates_in_quest = None;
                                        }
                                    }
                                }
                            }

                            self.quests.retain(|_, quest| !quest.done());

                            // Add quests.
                            let active_players = self
                                .players
                                .iter()
                                .filter(|(_, player)| player.is_active(self.time))
                                .count();

                            let active_not_new_players = self
                                .players
                                .iter()
                                .filter(|(_, player)| {
                                    player.is_active(self.time) && !player.is_new(self.time)
                                })
                                .count();

                            let num_quests = if cfg!(debug_assertions) {
                                30
                            } else {
                                (active_players / (2 * NEW_PLAYER_DIVIDER as usize) + active_not_new_players / 2)
                                    .max(10)
                                    .min(100)
                            };

                            let max_player_level = self
                                .players
                                .iter()
                                .map(|(_, player)| player.base.curr_level)
                                .max()
                                .unwrap_or(1);

                            let mut potential_quests = enum_iterator::all::<QuestType>()
                                .filter(|quest_type| {
                                    (if let Some(level) = quest_type.max_level() {
                                        max_player_level > level - 10 && max_player_level <= level
                                    } else {
                                        true
                                    }) && (!quest_type.one_at_a_time()
                                        || (quest_type.one_at_a_time()
                                            && !self
                                                .quests
                                                .values()
                                                .any(|quest| quest.quest_type == *quest_type)))
                                })
                                .collect::<CustomSet<_>>();

                            while self.quests.len() < num_quests {
                                if potential_quests.is_empty() {
                                    break;
                                }

                                let selected_quest = *potential_quests
                                    .iter()
                                    .copied()
                                    .collect::<Vec<_>>()
                                    .choose(rng)
                                    .expect("potential quests is empty");

                                if selected_quest.one_at_a_time() {
                                    potential_quests.swap_remove(&selected_quest);
                                }

                                let (min_level, max_level) =
                                    if let Some(level) = selected_quest.max_level() {
                                        (1, level)
                                    } else if selected_quest.one_at_a_time() {
                                        (1, 100)
                                    } else {
                                        let selected_level = self
                                            .players
                                            .iter()
                                            .collect::<Vec<_>>()
                                            .choose_weighted(rng, |(_, player)| {
                                                if player.is_active(self.time) {
                                                    if player.is_new(self.time) {
                                                        1
                                                    } else {
                                                        NEW_PLAYER_DIVIDER
                                                    }
                                                } else {
                                                    0
                                                }
                                            })
                                            .map(|(_, player)| player.base.curr_level)
                                            .unwrap_or(1);

                                        let min_level = (selected_level.saturating_sub(rng.gen_range(5..=15))).max(1);
                                        let max_level = (selected_level + rng.gen_range(5..=15)).min(100);
                                        (min_level, max_level)
                                    };

                                let quest = Quest::new(selected_quest, min_level, max_level);

                                self.quests.insert(self.next_quest_id, quest);

                                self.next_quest_id += 1;
                            }

                            // Add trades.
                            for trade in self.trade_deals.values_mut() {
                                trade.update(&mut self.players, self.time)?;
                            }

                            self.trade_deals.retain(|_, trade| !trade.done());

                            let num_trades = if cfg!(debug_assertions) {
                                15
                            } else {
                                (active_players / (5 * NEW_PLAYER_DIVIDER as usize) + active_not_new_players / 5)
                                    .max(3)
                                    .min(15)
                            };

                            while self
                                .trade_deals
                                .values()
                                .filter(|trade_deal| trade_deal.creator.is_none())
                                .count()
                                < num_trades
                            {
                                self.trade_deals.insert(self.next_trade_id, TradeDeal::new(rng, max_player_level));
                                self.next_trade_id += 1;
                            }
                        }
                    }
                }
            }

            Some(())
        }();

        if update_result.is_none() {
            println!("state update failed");
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct Bundle<T: BundleType>(CustomMap<T, u64>);

impl<T: BundleType> Bundle<T> {
    pub fn new() -> Self {
        Bundle(CustomMap::new())
    }

    pub fn add(mut self, t: T, n: u64) -> Self {
        if n > 0 {
            let mut map = CustomMap::new();
            map.insert(t, n);
            self.add_checked(Bundle(map));
        }
        self
    }

    pub fn mul(mut self, n: u64) -> Self {
        for qty in self.0.values_mut() {
            *qty *= n;
        }
        self
    }

    pub fn div(mut self, n: u64) -> Self {
        for qty in self.0.values_mut() {
            *qty /= n;
        }
        self
    }

    pub fn can_remove_x_times(&self, other: &Self) -> Option<u64> {
        let mut bound: Option<u64> = None;

        for t in self.0.keys().chain(other.0.keys()) {
            let n = self.0.get(t).copied().unwrap_or_default();
            if let Some(other_n) = other.0.get(t) {
                if *other_n > 0 {
                    if let Some(bound) = &mut bound {
                        *bound = (*bound).min(n / other_n);
                    } else {
                        bound = Some(n / other_n);
                    }
                }
            }
        }

        bound
    }

    pub fn check_add(&self, to_add: &Self) -> bool {
        for (t, n) in &to_add.0 {
            if let Some(max) = t.max() {
                if self.0.get(t).cloned().unwrap_or_default() + n > max {
                    return false;
                }
            }
        }

        true
    }

    pub fn add_checked(&mut self, to_add: Self) -> bool {
        if !self.check_add(&to_add) {
            return false;
        }

        for (t, n) in to_add.0 {
            *(self.0.entry(t).or_default()) += n;
        }

        true
    }

    pub fn check_remove(&self, to_remove: &Self) -> bool {
        for (t, n) in &to_remove.0 {
            if self
                .0
                .get(t)
                .cloned()
                .unwrap_or_default()
                .checked_sub(*n)
                .is_none()
            {
                return false;
            }
        }
        true
    }

    pub fn remove_checked(&mut self, to_remove: Self) -> bool {
        if !self.check_remove(&to_remove) {
            return false;
        }

        for (t, n) in to_remove.0 {
            *(self.0.entry(t).or_default()) -= n;
        }

        true
    }
}

impl Bundle<Item> {
    pub fn sorted_by_name(self) -> Vec<(Item, u64)> {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort_by_key(|(item, _)| (format!("{}", item), item.item_rarity()));
        vec
    }

    pub fn sorted_by_rarity(self) -> Vec<(Item, u64)> {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort_by_key(|(item, _)| (item.item_rarity(), format!("{}", item)));
        vec
    }

    pub fn sorted_by_usefulness(self, occupation: Occupation) -> Vec<(Item, u64)> {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort_by_key(|(item, _)| {
            (
                u64::MAX - item.usefulness_for(occupation),
                item.item_rarity(),
                format!("{}", item),
            )
        });
        vec
    }
}

impl<T: BundleType> Deref for Bundle<T> {
    type Target = CustomMap<T, u64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: BundleType> FromIterator<(T, u64)> for Bundle<T> {
    fn from_iter<I: IntoIterator<Item = (T, u64)>>(iter: I) -> Self {
        Bundle(iter.into_iter().collect())
    }
}

impl<T: BundleType> Default for Bundle<T> {
    fn default() -> Self {
        Bundle(CustomMap::new())
    }
}

pub trait BundleType: Hash + Eq + PartialEq + Copy + Ord {
    fn max(&self) -> Option<u64> {
        None
    }
}

pub trait Craftable: Sequence + BundleType {
    fn requires(self) -> Option<(u64, Bundle<Item>)>;
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Hash)]
pub struct Log {
    pub msgs: VecDeque<(Time, LogMsg)>,
    #[serde(default)]
    pub unread: bool,
}

impl Log {
    pub fn add(&mut self, time: Time, msg: LogMsg) {
        self.unread = true;
        self.msgs.push_back((time, msg));
        if self.msgs.len() > 100 {
            self.msgs.pop_front();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub enum LogMsg {
    NewPlayer(UserId),
    NewDwarf(String),
    DwarfDied(String),
    QuestCompletedMoney(QuestType, Money),
    QuestCompletedPrestige(QuestType, bool),
    QuestCompletedKing(QuestType, bool),
    QuestCompletedItems(QuestType, Option<Bundle<Item>>),
    QuestCompletedDwarfs(QuestType, Option<usize>),
    OpenedLootCrate(Bundle<Item>),
    MoneyForKing(Money),
    NotEnoughSpaceForDwarf,
    DwarfUpgrade(String, String),
    DwarfIsAdult(String),
    Overbid(Bundle<Item>, Money, TradeType),
    BidWon(Bundle<Item>, Money, TradeType),
    ItemSold(Bundle<Item>, Money),
    ItemNotSold(Bundle<Item>, Money),
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct Player {
    pub base: Base,
    pub dwarfs: CustomMap<DwarfId, Dwarf>,
    pub inventory: Inventory,
    pub log: Log,
    pub money: Money,
    pub last_online: Time,
    pub auto_functions: AutoFunctions,
    pub reward_time: Time,
    #[serde(default = "TutorialStep::first")]
    pub tutorial_step: Option<TutorialStep>,
    #[serde(default)]
    pub start_time: Time,
    pub popups: VecDeque<Popup>,
    pub manager: CustomMap<Occupation, u64>,
    #[serde(default)]
    pub chat_unread: bool,
    pub tribe: Option<TribeId>,
    pub tribe_points: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct AutoFunctions {
    pub auto_idle: bool,
    pub auto_craft: CustomSet<Item>,
    pub auto_store: CustomSet<Item>,
    pub auto_sell: CustomSet<Item>,
    #[serde(default = "CustomSet::new")]
    pub auto_dismantle: CustomSet<Item>,
}

impl Default for AutoFunctions {
    fn default() -> Self {
        Self {
            auto_idle: true,
            auto_craft: CustomSet::new(),
            auto_store: CustomSet::new(),
            auto_sell: CustomSet::new(),
            auto_dismantle: CustomSet::new(),
        }
    }
}

impl Player {
    pub fn new(time: Time, rng: &mut impl Rng, next_dwarf_id: &mut DwarfId) -> Self {
        let mut player = Player {
            dwarfs: CustomMap::new(),
            base: Base::new(),
            inventory: Inventory::new(),
            log: Log::default(),
            money: 0,
            last_online: time,
            auto_functions: AutoFunctions::default(),
            reward_time: time,
            tutorial_step: TutorialStep::first(),
            start_time: time,
            popups: VecDeque::new(),
            manager: CustomMap::new(),
            chat_unread: false,
            tribe: None,
            tribe_points: 0,
        };

        player.new_dwarf(rng, next_dwarf_id, time, Some(Stats::default()));

        if cfg!(debug_assertions) {
            player.base.curr_level = 15;
            player.money = 100000;
            for _ in 0..4 {
                player.new_dwarf(rng, next_dwarf_id, time, Some(Stats::default()));
            }
            player.inventory.add(
                Bundle::new()
                    .add(Item::Wood, 10000)
                    .add(Item::Iron, 10000)
                    .add(Item::Stone, 10000)
                    .add(Item::Coal, 10000),
                time,
            )
        }

        player
    }

    pub fn remaining_time_until_starvation(&self, state: &State) -> Time {
        let mut health_available = self.base.food * (MAX_HEALTH / 1000);
        let mut health_cost_per_tick = 0;

        let health_cost_multiplier = match state.event {
            Some(WorldEvent::Plague) => (1 + self.dwarfs.len() as u64 / 20).min(5),
            _ => 1,
        };

        for dwarf in self.dwarfs.values() {
            health_available += dwarf.health;
            health_cost_per_tick +=
                dwarf.actual_occupation().health_cost_per_tick() * health_cost_multiplier;
        }

        if health_cost_per_tick == 0 {
            return u64::MAX;
        }
        health_available / health_cost_per_tick
    }

    pub fn average_efficiency(&self) -> Option<u64> {
        self.dwarfs
            .values()
            .map(|dwarf| dwarf.effectiveness_percent(dwarf.occupation))
            .sum::<u64>()
            .checked_div(self.dwarfs.len() as u64)
    }

    pub fn set_manager(&mut self) {
        let manager_num = self.manager.values().copied().sum::<u64>();
        let dwarfs_num = self
            .dwarfs
            .values()
            .filter(|dwarf| dwarf.can_be_managed())
            .count() as u64;

        if manager_num > dwarfs_num {
            self.manager.clear();
            for dwarf in self.dwarfs.values() {
                if dwarf.can_be_managed() {
                    *self.manager.entry(dwarf.occupation).or_default() += 1;
                }
            }
        } else if dwarfs_num > manager_num {
            self.manager
                .entry(Occupation::Idling)
                .and_modify(|v| *v += dwarfs_num - manager_num)
                .or_insert(dwarfs_num - manager_num);
        }
    }

    fn set_mentor(&mut self, apprentice_id: DwarfId, mentor_id: Option<DwarfId>) -> Option<()> {
        let old_apprentice = if let Some(mentor_id) = mentor_id {
            let mentor = self.dwarfs.get_mut(&mentor_id)?;
            mentor.apprentice.replace(apprentice_id)
        } else {
            let apprentice = self.dwarfs.get(&apprentice_id)?;
            if let Some(mentor_id) = apprentice.mentor {
                let mentor = self.dwarfs.get_mut(&mentor_id)?;
                mentor.apprentice.take()
            } else {
                None
            }
        };
        if let Some(old_apprentice) = old_apprentice {
            let apprentice = self.dwarfs.get_mut(&old_apprentice)?;
            apprentice.mentor = None;
        }

        let apprentice = self.dwarfs.get_mut(&apprentice_id)?;
        let old_mentor = if let Some(mentor_id) = mentor_id {
            apprentice.mentor.replace(mentor_id)
        } else {
            apprentice.mentor.take()
        };
        if let Some(old_mentor) = old_mentor {
            let mentor = self.dwarfs.get_mut(&old_mentor)?;
            mentor.apprentice = None;
        }

        Some(())
    }

    pub fn add_popup(&mut self, popup: Popup) {
        self.popups.push_back(popup);
    }

    pub fn is_online(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_MINUTE * 5
    }

    pub fn is_active(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_DAY && !self.dwarfs.is_empty()
    }

    pub fn is_new(&self, time: Time) -> bool {
        (time - self.start_time) / SPEED < ONE_DAY || self.base.curr_level == 1
    }

    pub fn new_dwarf(
        &mut self,
        rng: &mut impl Rng,
        next_dwarf_id: &mut DwarfId,
        time: Time,
        adult_with_added_stats: Option<Stats>,
    ) {
        if self.dwarfs.len() < self.base.max_dwarfs() {
            let dwarf = if let Some(stats) = adult_with_added_stats {
                Dwarf::new_with_added_stats(rng, stats)
            } else {
                Dwarf::new_baby(rng)
            };
            self.log
                .add(time, LogMsg::NewDwarf(dwarf.actual_name().to_owned()));
            self.add_popup(Popup::NewDwarf(dwarf.clone()));
            self.dwarfs.insert(*next_dwarf_id, dwarf);
            *next_dwarf_id += 1;
        } else {
            self.log.add(time, LogMsg::NotEnoughSpaceForDwarf);
        }
    }

    pub fn open_loot_crate(&mut self, rng: &mut impl Rng, time: Time) {
        let possible_items: Vec<Item> = enum_iterator::all::<Item>()
            /*.filter(|item| {
                matches!(item.item_rarity(), ItemRarity::Epic | ItemRarity::Legendary)
            })*/
            .collect();
        let item = *possible_items.choose(rng).expect("possible items is empty");
        let bundle = Bundle::new().add(item, (10000 / item.item_rarity_num()).max(1).min(100));
        self.log.add(time, LogMsg::OpenedLootCrate(bundle.clone()));
        self.add_popup(Popup::NewItems(bundle.clone()));
        self.add_items(bundle, time, true);
    }

    pub fn add_items(&mut self, bundle: Bundle<Item>, time: Time, is_premium: bool) {
        self.inventory.add(bundle, time);
        self.auto_dismantle(time, is_premium);
        self.auto_craft(time, is_premium);
        self.auto_store(is_premium);
        self.auto_sell(is_premium);
    }

    pub fn auto_craft(&mut self, time: Time, is_premium: bool) {
        if is_premium {
            let mut items_added = false;
            // Auto-craft!
            for &item in &self.auto_functions.auto_craft {
                if let Some((level, requires)) = item.requires() {
                    if self.base.curr_level >= level {
                        if let Some(qty) = self.inventory.items.can_remove_x_times(&requires) {
                            if qty > 0 && self.inventory.items.remove_checked(requires.mul(qty)) {
                                self.inventory.add(Bundle::new().add(item, qty), time);

                                items_added = true;
                            }
                        }
                    }
                }
            }

            if items_added {
                self.auto_craft(time, is_premium);
            }
        }
    }

    pub fn auto_dismantle(&mut self, time: Time, is_premium: bool) {
        if is_premium {
            let mut items_added = false;
            // Auto-craft!
            for &item in &self.auto_functions.auto_dismantle {
                if let Some((_level, requires)) = item.requires() {
                    let qty = self.inventory.items.get(&item).copied().unwrap_or_default();
                    if qty > 0 && self
                            .inventory
                            .items
                            .remove_checked(Bundle::new().add(item, qty)) {
                        self.inventory
                            .add(requires.mul(qty).div(DISMANTLING_DIVIDER), time);

                        items_added = true;
                    }
                }
            }

            if items_added {
                self.auto_dismantle(time, is_premium);
            }
        }
    }

    pub fn auto_store(&mut self, is_premium: bool) {
        if is_premium {
            // Auto-store!
            for &item in &self.auto_functions.auto_store {
                if let Some(&qty) = self.inventory.items.get(&item) {
                    if let Some(food) = item.nutritional_value() {
                        if self
                            .inventory
                            .items
                            .remove_checked(Bundle::new().add(item, qty))
                        {
                            self.base.food += food * qty;
                        }
                    }
                }
            }
        }
    }

    pub fn auto_sell(&mut self, is_premium: bool) {
        if is_premium {
            // Auto-sell!
            for &item in &self.auto_functions.auto_sell {
                if let Some(&qty) = self.inventory.items.get(&item) {
                    if item.money_value(1) > 0 && self
                            .inventory
                            .items
                            .remove_checked(Bundle::new().add(item, qty)) {
                        self.money += item.money_value(1) * qty;
                    }
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct Inventory {
    pub items: Bundle<Item>,
    pub last_received: VecDeque<(Item, u64, Time)>,
}

impl Inventory {
    fn new() -> Self {
        Inventory {
            items: Bundle::new(),
            last_received: VecDeque::new(),
        }
    }

    pub fn add(&mut self, bundle: Bundle<Item>, time: Time) {
        for (item, qty) in bundle.iter() {
            if let Some((back_item, back_qty, back_time)) = self.last_received.back_mut() {
                if back_item == item {
                    *back_qty += qty;
                    *back_time = time;
                    continue;
                }
            }
            self.last_received.push_back((*item, *qty, time));
            if self.last_received.len() > 8 {
                self.last_received.pop_front();
            }
        }
        self.items.add_checked(bundle);
    }

    pub fn by_type(&self, item_type: Option<ItemType>) -> Vec<Item> {
        self.items
            .iter()
            .filter(|(item, n)| item.item_type() == item_type && **n > 0)
            .map(|(item, _)| item)
            .copied()
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
pub struct Stats {
    pub strength: i8,
    pub endurance: i8,
    pub agility: i8,
    pub intelligence: i8,
    pub perception: i8,
}

impl Stats {
    pub fn random(rng: &mut impl Rng, min: i8, max: i8) -> Self {
        Stats {
            strength: rng.gen_range(min..=max),
            endurance: rng.gen_range(min..=max),
            agility: rng.gen_range(min..=max),
            intelligence: rng.gen_range(min..=max),
            perception: rng.gen_range(min..=max),
        }
    }

    pub fn sum(self, other: Self) -> Self {
        Stats {
            strength: (self.strength + other.strength).min(10).max(1),
            endurance: (self.endurance + other.endurance).min(10).max(1),
            agility: (self.agility + other.agility).min(10).max(1),
            intelligence: (self.intelligence + other.intelligence).min(10).max(1),
            perception: (self.perception + other.perception).min(10).max(1),
        }
    }

    pub fn cross(self, other: Self) -> u64 {
        

        self.strength as u64 * other.strength as u64
            + self.endurance as u64 * other.endurance as u64
            + self.agility as u64 * other.agility as u64
            + self.intelligence as u64 * other.intelligence as u64
            + self.perception as u64 * other.perception as u64
    }

    pub fn is_zero(&self) -> bool {
        *self == Stats::default()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct Dwarf {
    pub name: String,
    pub participates_in_quest: Option<(QuestType, QuestId, usize)>,
    pub occupation: Occupation,
    pub auto_idle: bool,
    pub stats: Stats,
    pub equipment: CustomMap<ItemType, Item>,
    pub health: Health,
    pub is_female: bool,
    pub age_seconds: u64,
    #[serde(default)]
    pub custom_name: Option<String>,
    #[serde(default)]
    pub manual_management: bool,
    #[serde(default)]
    pub mentor: Option<DwarfId>,
    #[serde(default)]
    pub apprentice: Option<DwarfId>,
    #[serde(default)]
    pub released: bool,
}

impl Dwarf {
    pub fn actual_name(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(&self.name)
    }

    pub fn age_years(&self) -> u64 {
        self.age_seconds / (365 * 24 * 60 * 60)
    }

    pub fn is_adult(&self) -> bool {
        self.age_years() >= ADULT_AGE
    }

    pub fn can_be_managed(&self) -> bool {
        self.is_adult() && self.participates_in_quest.is_none() && !self.manual_management
    }

    pub fn name(rng: &mut impl Rng) -> String {
        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let consonants = [
            'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'v',
            'w', 'x', 'y', 'z',
        ];

        let len = (2..8).choose(rng).unwrap();

        let mut name = String::new();

        name.push(
            consonants
                .choose(rng)
                .unwrap()
                .to_uppercase()
                .next()
                .unwrap(),
        );
        name.push(*vowels.choose(rng).unwrap());

        for _ in 0..len {
            let mut rev_chars = name.chars().rev();
            let last_is_consonant = consonants.contains(&rev_chars.next().unwrap());
            let second_last_is_consonant = consonants.contains(&rev_chars.next().unwrap());
            if last_is_consonant {
                if second_last_is_consonant {
                    name.push(*vowels.choose(rng).unwrap());
                } else if rng.gen_bool(0.4) {
                    name.push(*vowels.choose(rng).unwrap());
                } else if rng.gen_bool(0.7) {
                    name.push(*consonants.choose(rng).unwrap());
                } else {
                    let last = name.pop().unwrap();
                    name.push(last);
                    name.push(last);
                }
            } else {
                name.push(*consonants.choose(rng).unwrap());
            }
        }

        name
    }

    /*fn new_adult(rng: &mut impl Rng) -> Self {
        let name = Dwarf::name(rng);

        Dwarf {
            name,
            occupation: Occupation::Idling,
            auto_idle: false,
            stats: Stats::random(rng, 1, 6),
            equipment: CustomMap::new(),
            health: MAX_HEALTH,
            participates_in_quest: None,
            is_female: rng.gen_bool(FEMALE_PROBABILITY),
            age_seconds: rng.gen_range(ADULT_AGE..DEATH_AGE) * 365 * 24 * 60 * 60,
            custom_name: None,
            manual_management: false,
            mentor: None,
            apprentice: None,
            released: false,
        }
    }*/

    fn new_with_added_stats(rng: &mut impl Rng, stats: Stats) -> Self {
        let name = Dwarf::name(rng);

        Dwarf {
            name,
            occupation: Occupation::Idling,
            auto_idle: false,
            stats: Stats::random(rng, 1, 6).sum(stats),
            equipment: CustomMap::new(),
            health: MAX_HEALTH,
            participates_in_quest: None,
            is_female: rng.gen_bool(FEMALE_PROBABILITY),
            age_seconds: rng.gen_range(ADULT_AGE..DEATH_AGE) * 365 * 24 * 60 * 60,
            custom_name: None,
            manual_management: false,
            mentor: None,
            apprentice: None,
            released: false,
        }
    }


    fn new_baby(rng: &mut impl Rng) -> Self {
        let name = Dwarf::name(rng);

        Dwarf {
            name,
            occupation: Occupation::Idling,
            auto_idle: false,
            stats: Stats::random(rng, 1, 6),
            equipment: CustomMap::new(),
            health: MAX_HEALTH,
            participates_in_quest: None,
            is_female: rng.gen_bool(FEMALE_PROBABILITY),
            age_seconds: 0,
            custom_name: None,
            manual_management: false,
            mentor: None,
            apprentice: None,
            released: false,
        }
    }

    pub fn dead(&self) -> bool {
        self.health == 0
    }

    pub fn actual_occupation(&self) -> Occupation {
        if self.auto_idle {
            return Occupation::Idling;
        }

        self.participates_in_quest
            .map(|(quest_type, _, _)| quest_type.occupation())
            .unwrap_or(self.occupation)
    }

    pub fn incr_health(&mut self, incr: u64) {
        if self.health + incr >= MAX_HEALTH {
            self.health = MAX_HEALTH;
        } else {
            self.health += incr;
        }
    }

    pub fn decr_health(&mut self, decr: u64) {
        if let Some(res) = self.health.checked_sub(decr) {
            self.health = res;
        } else {
            self.health = 0;
        }
    }

    pub fn change_occupation(&mut self, occupation: Occupation) {
        self.occupation = occupation;
    }

    pub fn effective_stats(&self) -> Stats {
        let mut stats = self.stats;
        for item in self.equipment.values() {
            stats = stats.sum(item.provides_stats());
        }
        stats
    }

    pub fn numerator_effectiveness(&self, dwarfs: &CustomMap<DwarfId, Dwarf>) -> u64 {
        self.effectiveness_not_normalized_with_apprentice(self.actual_occupation(), dwarfs)
            + (MAX_EFFECTIVENESS / (MIN_MAX_DWARF_DIFFERENCE - 1))
    }

    pub fn gen_ratio_effectiveness(
        &self,
        dwarfs: &CustomMap<DwarfId, Dwarf>,
        rng: &mut impl Rng,
        denominator_mul: u64,
    ) -> bool {
        let denominator = (MAX_EFFECTIVENESS / (MIN_MAX_DWARF_DIFFERENCE - 1)) * denominator_mul;
        rng.gen_ratio(
            (self.numerator_effectiveness(dwarfs) / 100) as u32,
            (denominator / 100) as u32,
        )
    }

    pub fn effectiveness_not_normalized_with_apprentice(
        &self,
        occupation: Occupation,
        dwarfs: &CustomMap<DwarfId, Dwarf>,
    ) -> u64 {
        let mut effectiveness = self.effectiveness_not_normalized(occupation);

        if let Some(apprentice_id) = self.apprentice {
            if let Some(apprentice) = dwarfs.get(&apprentice_id) {
                if !apprentice.is_adult() {
                    effectiveness += apprentice.effectiveness_not_normalized(occupation)
                        / APPRENTICE_EFFECTIVENESS_DIVIDER;
                }
            }
        }

        effectiveness
    }

    // output 0 - 100
    pub fn effectiveness_percent(&self, occupation: Occupation) -> u64 {
        self.effectiveness_not_normalized(occupation) * 100 / MAX_EFFECTIVENESS
    }

    // output 0 - 6000
    pub fn effectiveness_not_normalized(&self, occupation: Occupation) -> u64 {
        let mut usefulness = 0;
        for item in self.equipment.values() {
            usefulness += item.usefulness_for(occupation);
        }

        usefulness = usefulness.min(30);

        let effectiveness = usefulness * self.effective_stats().cross(occupation.requires_stats());

        effectiveness.min(MAX_EFFECTIVENESS)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, Sequence, PartialEq, Eq, Display, Hash)]
#[strum(serialize_all = "title_case")]
pub enum Occupation {
    Idling,
    Mining,
    Logging,
    Hunting,
    Gathering,
    Fishing,
    Fighting,
    Exploring,
    Farming,
    Rockhounding,
}

impl Occupation {
    pub fn health_cost_per_tick(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 3,
            Occupation::Logging => 3,
            Occupation::Hunting => 3,
            Occupation::Gathering => 2,
            Occupation::Fishing => 2,
            Occupation::Fighting => 8,
            Occupation::Exploring => 5,
            Occupation::Farming => 3,
            Occupation::Rockhounding => 5,
        }
    }

    pub fn unlocked_at_level(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 1,
            Occupation::Logging => 1,
            Occupation::Hunting => 1,
            Occupation::Gathering => 1,
            Occupation::Fishing => 10,
            Occupation::Exploring => 20,
            Occupation::Fighting => 30,
            Occupation::Farming => 40,
            Occupation::Rockhounding => 50,
        }
    }

    pub fn requires_stats(self) -> Stats {
        match self {
            Occupation::Idling => Stats {
                ..Default::default()
            },
            Occupation::Mining => Stats {
                strength: 10,
                perception: 10,
                ..Default::default()
            },
            Occupation::Logging => Stats {
                strength: 10,
                endurance: 10,
                ..Default::default()
            },
            Occupation::Hunting => Stats {
                agility: 10,
                perception: 10,
                ..Default::default()
            },
            Occupation::Gathering => Stats {
                intelligence: 10,
                perception: 10,
                ..Default::default()
            },
            Occupation::Fishing => Stats {
                intelligence: 10,
                agility: 10,
                ..Default::default()
            },
            Occupation::Fighting => Stats {
                strength: 10,
                endurance: 10,
                ..Default::default()
            },
            Occupation::Exploring => Stats {
                endurance: 10,
                intelligence: 10,
                ..Default::default()
            },
            Occupation::Farming => Stats {
                endurance: 10,
                agility: 10,
                ..Default::default()
            },
            Occupation::Rockhounding => Stats {
                intelligence: 10,
                strength: 10,
                ..Default::default()
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct Base {
    pub curr_level: u64,
    pub build_time: Time,
    pub food: Food,
}

impl Default for Base {
    fn default() -> Self {
        Self::new()
    }
}

impl Base {
    pub fn new() -> Base {
        Base {
            curr_level: 1,
            build_time: 0,
            food: 0,
        }
    }

    pub fn max_dwarfs(&self) -> usize {
        self.max_dwarfs_at(self.curr_level)
    }

    pub fn max_dwarfs_at(&self, level: u64) -> usize {
        (level as usize + 1) / 2
    }

    pub fn upgrade_cost(&self) -> Option<Bundle<Item>> {
        if self.curr_level < MAX_LEVEL {
            let multiplier = |unlocked_after_level: u64| {
                self.curr_level.saturating_sub(unlocked_after_level)
                    * (self.curr_level.saturating_sub(unlocked_after_level) / 10 + 1)
            };

            Some(
                Bundle::new()
                    .add(Item::Wood, 50 * multiplier(0))
                    .add(Item::Stone, 50 * multiplier(20))
                    .add(Item::Nail, 10 * multiplier(40))
                    .add(Item::Fabric, 10 * multiplier(60))
                    .add(Item::Gold, 10 * multiplier(80)),
            )
        } else {
            None
        }
    }

    pub fn build_time_ticks(&self) -> u64 {
        self.curr_level * (self.curr_level / 10 + 1) * 15
    }

    pub fn build(&mut self) -> Option<u64> {
        if self.build_time > 0 {
            self.build_time -= 1;
            if self.build_time == 0 {
                self.curr_level += 1;
                return Some(self.curr_level);
            }
        }
        None
    }

    pub fn upgrade(&mut self) {
        if self.curr_level < MAX_LEVEL && self.build_time == 0 {
            self.build_time = self.build_time_ticks();
        }
    }

    pub fn village_type(&self) -> VillageType {
        match self.curr_level / 10 {
            0 => VillageType::Outpost,
            1 => VillageType::Dwelling,
            2 => VillageType::Hamlet,
            3 => VillageType::Village,
            4 => VillageType::SmallTown,
            5 => VillageType::LargeTown,
            6 => VillageType::SmallCity,
            7 => VillageType::LargeCity,
            8 => VillageType::Metropolis,
            _ => VillageType::Megalopolis,
        }
    }
}

#[derive(Display, Sequence)]
#[strum(serialize_all = "title_case")]
pub enum VillageType {
    Outpost,
    Dwelling,
    Hamlet,
    Village,
    SmallTown,
    LargeTown,
    SmallCity,
    LargeCity,
    Metropolis,
    Megalopolis,
}

pub type DwarfId = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientEvent {
    Init,
    Message(String),
    ChangeOccupation(DwarfId, Occupation),
    Craft(Item, u64),
    Dismantle(Item, u64),
    UpgradeBase,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    OpenLootCrate,
    OpenDailyReward,
    AssignToQuest(QuestId, usize, Option<DwarfId>),
    AddToFoodStorage(Item, u64),
    Sell(Item, u64),
    Restart,
    ToggleAutoCraft(Item),
    ToggleAutoStore(Item),
    ToggleAutoSell(Item),
    ToggleAutoDismantle(Item),
    ToggleAutoIdle,
    HireDwarf(HireDwarfType),
    NextTutorialStep,
    ConfirmPopup,
    SetManagerOccupation(Occupation, u64),
    Optimize(Option<DwarfId>),
    SetDwarfName(DwarfId, String),
    ToggleManualManagement(DwarfId),
    SetMentor(DwarfId, Option<DwarfId>),
    Bid(TradeId),
    ReleaseDwarf(DwarfId),
    ReadLog,
    ReadChat,
}

impl engine_shared::ClientEvent for ClientEvent {
    fn init() -> Self {
        ClientEvent::Init
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerEvent {
    Tick,
}

impl engine_shared::ServerEvent<State> for ServerEvent {
    fn tick() -> Self {
        ServerEvent::Tick
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Hash)]
pub struct Chat {
    pub messages: VecDeque<(UserId, String, Time)>,
}

impl Chat {
    pub fn add_message(
        &mut self,
        players: &mut CustomMap<UserId, Player>,
        user_id: UserId,
        message: String,
        time: Time,
    ) {
        for player in players.values_mut() {
            player.chat_unread = true;
        }
        self.messages.push_back((user_id, message, time));
        if self.messages.len() > 100 {
            self.messages.pop_front();
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
pub struct Quest {
    pub contestants: CustomMap<UserId, Contestant>,
    pub time_left: u64,
    pub quest_type: QuestType,
    #[serde(default = "min_level")]
    pub min_level: u64,
    #[serde(default = "max_level")]
    pub max_level: u64,
}

const fn max_level() -> u64 {
    100
}

const fn min_level() -> u64 {
    100
}

impl Quest {
    pub fn new(quest_type: QuestType, min_level: u64, max_level: u64) -> Self {
        Quest {
            contestants: CustomMap::new(),
            time_left: quest_type.duration(),
            quest_type,
            min_level,
            max_level,
        }
    }

    pub fn best(&self) -> Option<UserId> {
        let mut best_score = 0;
        let mut best_user_id = None;
        for (user_id, contestant) in &self.contestants {
            if contestant.achieved_score >= best_score {
                best_user_id = Some(*user_id);
                best_score = contestant.achieved_score;
            }
        }
        best_user_id
    }

    pub fn split_by_score(&self, num: u64) -> Vec<(UserId, u64)> {
        let total_score: u64 = self.contestants.values().map(|c| c.achieved_score).sum();
        self.contestants
            .iter()
            .map(|(user_id, c)| (*user_id, num * c.achieved_score / total_score))
            .collect()
    }

    pub fn chance_by_score(&self, rng: &mut impl Rng) -> Option<UserId> {
        let total_score: u64 = self.contestants.values().map(|c| c.achieved_score).sum();
        self.contestants
            .iter()
            .map(|(user_id, c)| (*user_id, c.achieved_score as f64 / total_score as f64))
            .collect::<Vec<_>>()
            .choose_weighted(rng, |elem| elem.1)
            .ok()
            .map(|item| item.0)
    }

    pub fn add_contenstant(&mut self, user_id: UserId) {
        self.contestants.insert(
            user_id,
            Contestant {
                dwarfs: CustomMap::new(),
                achieved_score: 0,
            },
        );
    }

    pub fn run(&mut self, players: &CustomMap<UserId, Player>) -> Option<()> {
        if self.time_left > 0 {
            self.time_left -= 1;
            for (user_id, contestant) in self.contestants.iter_mut() {
                for dwarf_id in contestant.dwarfs.values() {
                    let player = players.get(user_id)?;
                    let dwarf = player.dwarfs.get(dwarf_id)?;

                    if dwarf.actual_occupation() == self.quest_type.occupation() {
                        for _ in 0..10 {}
                        contestant.achieved_score +=
                            dwarf.numerator_effectiveness(&player.dwarfs) / 100;
                    }
                }
            }
        }

        Some(())
    }

    pub fn done(&self) -> bool {
        self.time_left == 0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RewardMode {
    BestGetsAll(Money),
    SplitFairly(Money),
    BestGetsItems(Bundle<Item>),
    ItemsByChance(Bundle<Item>),
    NewDwarf(usize),
    NewDwarfByChance(usize),
    BecomeKing,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardType {
    Fair,
    Chance,
    Best,
}

impl RewardMode {
    pub fn reward_type(&self) -> RewardType {
        match self {
            RewardMode::BestGetsAll(_)
            | RewardMode::BestGetsItems(_)
            | RewardMode::NewDwarf(_)
            | RewardMode::BecomeKing => RewardType::Best,
            RewardMode::SplitFairly(_) => RewardType::Fair,
            RewardMode::ItemsByChance(_) | RewardMode::NewDwarfByChance(_) => RewardType::Chance,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Hash)]
pub struct Contestant {
    pub dwarfs: CustomMap<usize, DwarfId>,
    pub achieved_score: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Sequence, Hash)]
pub enum QuestType {
    KillTheDragon,
    ArenaFight,
    FreeTheVillage,
    FeastForAGuest,
    ADwarfGotLost,
    AFishingFriend,
    ADwarfInDanger,
    ForTheKing,
    DrunkFishing,
    CollapsedCave,
    TheHiddenTreasure,
    CatStuckOnATree,
    AttackTheOrks,
    FreeTheDwarf,
    FarmersContest,
    CrystalsForTheElves,
    ADarkSecret,
    ElvenVictory,
    TheMassacre,
    TheElvenWar,
    Concert,
    MagicalBerries,
    EatingContest,
    Socializing,
    TheElvenMagician,
    ExploreNewLands,
    DeepInTheCaves,
    MinersLuck,
    AbandonedOrkCamp,
}

impl std::fmt::Display for QuestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuestType::ArenaFight => write!(f, "Arena Fight"),
            QuestType::KillTheDragon => write!(f, "Kill the Dragon"),
            QuestType::FreeTheVillage => write!(f, "Free the Elven Village"),
            QuestType::FeastForAGuest => write!(f, "A Feast for a Guest"),
            QuestType::ADwarfGotLost => write!(f, "A Dwarf got Lost"),
            QuestType::AFishingFriend => write!(f, "A Fishing Friend"),
            QuestType::ADwarfInDanger => write!(f, "A Dwarf in Danger"),
            QuestType::ForTheKing => write!(f, "For the King!"),
            QuestType::DrunkFishing => write!(f, "Drunk Fishing Contest"),
            QuestType::CollapsedCave => write!(f, "Trapped in the Collapsed Cave"),
            QuestType::TheHiddenTreasure => write!(f, "The Hidden Treasure"),
            QuestType::CatStuckOnATree => write!(f, "Cat Stuck on a Tree"),
            QuestType::AttackTheOrks => write!(f, "Attack the Orks"),
            QuestType::FreeTheDwarf => write!(f, "Free the Dwarfs"),
            QuestType::FarmersContest => write!(f, "Farmers Contest"),
            QuestType::CrystalsForTheElves => write!(f, "Crystals for the Elves"),
            QuestType::ADarkSecret => write!(f, "A Dark Secret"),
            QuestType::ElvenVictory => write!(f, "The Elven Victory"),
            QuestType::TheMassacre => write!(f, "The Massacre"),
            QuestType::TheElvenWar => write!(f, "The Elven War"),
            QuestType::Concert => write!(f, "Concert in the Tavern"),
            QuestType::MagicalBerries => write!(f, "Magical Berries"),
            QuestType::EatingContest => write!(f, "Eating Contest"),
            QuestType::Socializing => write!(f, "Socializing in the Tavern"),
            QuestType::TheElvenMagician => write!(f, "The Elven Magician"),
            QuestType::ExploreNewLands => write!(f, "Explore new Lands"),
            QuestType::DeepInTheCaves => write!(f, "Deep in the Caves"),
            QuestType::MinersLuck => write!(f, "Miner's Luck"),
            QuestType::AbandonedOrkCamp => write!(f, "Abandonned Ork Camp"),
        }
    }
}

impl QuestType {
    pub fn reward_mode(self) -> RewardMode {
        match self {
            Self::KillTheDragon => {
                RewardMode::BestGetsItems(Bundle::new().add(Item::DragonsEgg, 1))
            }
            Self::ArenaFight => RewardMode::BestGetsAll(2000),
            Self::FreeTheVillage => RewardMode::SplitFairly(2000),
            Self::FeastForAGuest => RewardMode::NewDwarf(1),
            Self::ADwarfGotLost => RewardMode::NewDwarfByChance(1),
            Self::AFishingFriend => RewardMode::NewDwarfByChance(3),
            Self::ADwarfInDanger => RewardMode::NewDwarf(1),
            Self::ForTheKing => RewardMode::BecomeKing,
            Self::DrunkFishing => RewardMode::BestGetsAll(2000),
            Self::CollapsedCave => RewardMode::NewDwarf(3),
            Self::TheHiddenTreasure => RewardMode::BestGetsItems(
                Bundle::new()
                    .add(Item::Diamond, 3)
                    .add(Item::Gold, 30)
                    .add(Item::Iron, 300),
            ),
            Self::CatStuckOnATree => RewardMode::BestGetsItems(Bundle::new().add(Item::Cat, 1)),
            Self::AttackTheOrks => RewardMode::SplitFairly(2000),
            Self::FreeTheDwarf => RewardMode::NewDwarfByChance(1),
            Self::FarmersContest => RewardMode::BestGetsItems(Bundle::new().add(Item::Horse, 1)),
            Self::CrystalsForTheElves => RewardMode::BestGetsItems(
                Bundle::new()
                    .add(Item::CrystalNecklace, 1)
                    .add(Item::Gold, 50),
            ),
            Self::ADarkSecret => RewardMode::NewDwarf(1),
            Self::ElvenVictory => RewardMode::SplitFairly(2000),
            Self::TheElvenWar => RewardMode::SplitFairly(10000),
            Self::TheMassacre => RewardMode::NewDwarfByChance(3),
            Self::Concert => RewardMode::SplitFairly(1000),
            Self::MagicalBerries => RewardMode::SplitFairly(1000),
            Self::EatingContest => RewardMode::SplitFairly(1000),
            Self::Socializing => RewardMode::NewDwarfByChance(3),
            Self::TheElvenMagician => RewardMode::SplitFairly(2000),
            Self::ExploreNewLands => RewardMode::SplitFairly(4000),
            Self::DeepInTheCaves => RewardMode::ItemsByChance(Bundle::new().add(Item::Kobold, 1)),
            Self::MinersLuck => RewardMode::ItemsByChance(Bundle::new().add(Item::Diamond, 3)),
            Self::AbandonedOrkCamp => RewardMode::ItemsByChance(
                Bundle::new()
                    .add(Item::Coal, 1000)
                    .add(Item::Wood, 1000)
                    .add(Item::Iron, 100),
            ),
        }
    }

    pub fn one_at_a_time(self) -> bool {
        matches!(self.reward_mode(), RewardMode::BecomeKing)
    }

    pub fn duration(self) -> u64 {
        match self {
            Self::KillTheDragon => ONE_HOUR * 2,
            Self::ArenaFight => ONE_HOUR * 4,
            Self::FreeTheVillage => ONE_HOUR * 2,
            Self::FeastForAGuest => ONE_HOUR * 2,
            Self::ADwarfGotLost => ONE_HOUR * 2,
            Self::AFishingFriend => ONE_HOUR,
            Self::ADwarfInDanger => ONE_HOUR * 2,
            Self::ForTheKing => ONE_HOUR * 8,
            Self::DrunkFishing => ONE_HOUR * 2,
            Self::CollapsedCave => ONE_HOUR * 4,
            Self::TheHiddenTreasure => ONE_HOUR * 2,
            Self::CatStuckOnATree => ONE_HOUR,
            Self::AttackTheOrks => ONE_HOUR * 2,
            Self::FreeTheDwarf => ONE_HOUR * 4,
            Self::FarmersContest => ONE_HOUR,
            Self::CrystalsForTheElves => ONE_HOUR,
            Self::ADarkSecret => ONE_HOUR * 4,
            Self::ElvenVictory => ONE_HOUR * 2,
            Self::TheMassacre => ONE_HOUR * 8,
            Self::TheElvenWar => ONE_HOUR * 8,
            Self::Concert => ONE_HOUR,
            Self::MagicalBerries => ONE_HOUR * 2,
            Self::EatingContest => ONE_HOUR,
            Self::Socializing => ONE_HOUR * 2,
            Self::TheElvenMagician => ONE_HOUR * 2,
            Self::ExploreNewLands => ONE_HOUR * 4,
            Self::DeepInTheCaves => ONE_HOUR * 4,
            Self::MinersLuck => ONE_HOUR * 4,
            Self::AbandonedOrkCamp => ONE_HOUR * 4,
        }
    }

    pub fn occupation(self) -> Occupation {
        match self {
            Self::KillTheDragon => Occupation::Fighting,
            Self::ArenaFight => Occupation::Fighting,
            Self::FreeTheVillage => Occupation::Fighting,
            Self::FeastForAGuest => Occupation::Hunting,
            Self::ADwarfGotLost => Occupation::Exploring,
            Self::AFishingFriend => Occupation::Fishing,
            Self::ADwarfInDanger => Occupation::Fighting,
            Self::ForTheKing => Occupation::Fighting,
            Self::DrunkFishing => Occupation::Fishing,
            Self::CollapsedCave => Occupation::Mining,
            Self::TheHiddenTreasure => Occupation::Exploring,
            Self::CatStuckOnATree => Occupation::Logging,
            Self::AttackTheOrks => Occupation::Fighting,
            Self::FreeTheDwarf => Occupation::Fighting,
            Self::FarmersContest => Occupation::Farming,
            Self::CrystalsForTheElves => Occupation::Rockhounding,
            Self::ADarkSecret => Occupation::Exploring,
            Self::ElvenVictory => Occupation::Logging,
            Self::TheMassacre => Occupation::Fighting,
            Self::TheElvenWar => Occupation::Fighting,
            Self::Concert => Occupation::Idling,
            Self::MagicalBerries => Occupation::Gathering,
            Self::EatingContest => Occupation::Idling,
            Self::Socializing => Occupation::Idling,
            Self::TheElvenMagician => Occupation::Gathering,
            Self::ExploreNewLands => Occupation::Exploring,
            Self::DeepInTheCaves => Occupation::Mining,
            Self::MinersLuck => Occupation::Rockhounding,
            Self::AbandonedOrkCamp => Occupation::Exploring,
        }
    }

    pub fn max_dwarfs(self) -> usize {
        match self {
            Self::KillTheDragon => 3,
            Self::ArenaFight => 1,
            Self::FreeTheVillage => 3,
            Self::FeastForAGuest => 1,
            Self::ADwarfGotLost => 1,
            Self::AFishingFriend => 1,
            Self::ADwarfInDanger => 3,
            Self::ForTheKing => 3,
            Self::DrunkFishing => 1,
            Self::CollapsedCave => 3,
            Self::TheHiddenTreasure => 3,
            Self::CatStuckOnATree => 1,
            Self::AttackTheOrks => 3,
            Self::FreeTheDwarf => 3,
            Self::FarmersContest => 1,
            Self::CrystalsForTheElves => 3,
            Self::ADarkSecret => 1,
            Self::ElvenVictory => 3,
            Self::TheMassacre => 3,
            Self::TheElvenWar => 5,
            Self::Concert => 1,
            Self::MagicalBerries => 3,
            Self::EatingContest => 1,
            Self::Socializing => 1,
            Self::TheElvenMagician => 1,
            Self::ExploreNewLands => 3,
            Self::DeepInTheCaves => 1,
            Self::MinersLuck => 1,
            Self::AbandonedOrkCamp => 1,
        }
    }

    pub fn max_level(self) -> Option<u64> {
        match self {
            //s if s.reward_mode().reward_type() == RewardType::Fair => 100,
            /*
            QuestType::ForTheKing => 100,

            QuestType::AFishingFriend => 20,
            QuestType::Concert => 20,
            QuestType::CollapsedCave => 40,
            QuestType::DrunkFishing => 60,
            QuestType::CatStuckOnATree => 60,
            QuestType::FarmersContest => 60,
            QuestType::EatingContest => 80,
            QuestType::MagicalBerries => 80,
            QuestType::Socializing => 80,
            QuestType::AbandonedOrkCamp => 80,
            QuestType::DeepInTheCaves => 100,
            QuestType::KillTheDragon => 100,
            QuestType::TheHiddenTreasure => 100,
            QuestType::ArenaFight => 100,
            QuestType::ExploreNewLands => 100,
            QuestType::MinersLuck => 100,
            */
            QuestType::FeastForAGuest => Some(10),
            QuestType::FreeTheVillage => Some(15),
            QuestType::ADwarfGotLost => Some(20),
            QuestType::ADwarfInDanger => Some(25),
            QuestType::AttackTheOrks => Some(30),
            QuestType::FreeTheDwarf => Some(40),
            QuestType::CrystalsForTheElves => Some(50),
            QuestType::TheElvenMagician => Some(60),
            QuestType::ADarkSecret => Some(70),
            QuestType::ElvenVictory => Some(80),
            QuestType::TheMassacre => Some(90),
            QuestType::TheElvenWar => Some(100),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct TradeDeal {
    pub items: Bundle<Item>,
    pub next_bid: Money,
    pub highest_bidder: Option<(UserId, Money)>,
    pub time_left: Time,
    pub user_trade_type: TradeType,
    #[serde(default)]
    pub creator: Option<UserId>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TradeType {
    Buy,
    Sell,
}

impl TradeDeal {
    pub fn new(rng: &mut impl Rng, max_player_level: u64) -> Self {
        let item = enum_iterator::all::<Item>()
            .filter(|item| {
                (if let Some((level, _)) = item.requires() {
                    level <= max_player_level
                } else {
                    true
                }) && item.item_type().is_some()
            })
            .choose(rng)
            .unwrap();
        let time_left = rng.gen_range(ONE_MINUTE * 20..ONE_HOUR * 2);
        let qty = ((time_left * 10) / item.item_rarity_num()).max(1);

        /*
        let user_trade_type = if rng.gen_bool(0.5) {
            TradeType::Buy
        } else {
            TradeType::Sell
        };
        */

        TradeDeal {
            items: Bundle::new().add(item, qty),
            next_bid: item.money_value(qty) as u64 * TRADE_MONEY_MULTIPLIER,
            time_left,
            highest_bidder: None,
            creator: None,
            user_trade_type: TradeType::Buy,
        }
    }

    pub fn from_player(user_id: UserId, player: &mut Player, item: Item, qty: u64) -> Option<Self> {
        let qty = qty.min(player.inventory.items.get(&item).copied().unwrap_or(0));
        let time_left = ((qty * item.item_rarity_num()) / 10)
            .max(ONE_MINUTE * 20)
            .min(ONE_HOUR * 4);
        let items = Bundle::new().add(item, qty);
        let next_bid = item.money_value(qty) * TRADE_MONEY_MULTIPLIER;

        if qty == 0 {
            return None;
        }

        if next_bid == 0 {
            return None;
        }

        if !player.inventory.items.remove_checked(items.clone()) {
            return None;
        }

        Some(TradeDeal {
            items,
            next_bid,
            time_left,
            highest_bidder: None,
            creator: Some(user_id),
            user_trade_type: TradeType::Buy,
        })
    }

    pub fn update(&mut self, players: &mut CustomMap<UserId, Player>, time: Time) -> Option<()> {
        if self.time_left > 0 {
            self.time_left -= 1;
            if self.time_left == 0 || self.next_bid <= 1 {
                if let Some((best_bidder_user_id, best_bidder_money)) = self.highest_bidder {
                    let p = players.get_mut(&best_bidder_user_id)?;
                    p.inventory.add(self.items.clone(), time);
                    p.log.add(
                        time,
                        LogMsg::BidWon(self.items.clone(), best_bidder_money, self.user_trade_type),
                    );

                    if let Some(creator) = self.creator {
                        let c = players.get_mut(&creator)?;
                        c.money += best_bidder_money;
                        c.log.add(
                            time,
                            LogMsg::ItemSold(self.items.clone(), best_bidder_money),
                        );
                    }
                } else if let Some(creator) = self.creator {
                    let c = players.get_mut(&creator)?;
                    c.inventory.add(self.items.clone(), time);
                    c.log
                        .add(time, LogMsg::ItemNotSold(self.items.clone(), self.next_bid));
                }
            }
        }
        Some(())
    }

    pub fn done(&self) -> bool {
        self.time_left == 0
    }

    pub fn bid(
        &mut self,
        players: &mut CustomMap<UserId, Player>,
        user_id: UserId,
        time: Time,
    ) -> Option<()> {
        if self.user_trade_type == TradeType::Buy {
            if self.creator == Some(user_id) {
                return None;
            }
            if players.get_mut(&user_id)?.money >= self.next_bid {
                if let Some((best_bidder_user_id, best_bidder_money)) = self.highest_bidder {
                    let p = players.get_mut(&best_bidder_user_id)?;
                    p.money += best_bidder_money;
                    p.log.add(
                        time,
                        LogMsg::Overbid(self.items.clone(), self.next_bid, self.user_trade_type),
                    );
                }
                players.get_mut(&user_id)?.money -= self.next_bid;
                self.highest_bidder = Some((user_id, self.next_bid));
                self.next_bid += (self.next_bid / 10).max(1);
                if self.time_left < ONE_MINUTE * SPEED {
                    self.time_left += ONE_MINUTE * SPEED;
                }
            }
        }

        Some(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Default)]
pub struct Tribe {
    pub territories: CustomMap<Territory, u64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Sequence, Display)]
pub enum Territory {
    Mountains, // Mining, Rockhounding -> Stength
    Forest, // Logging, Hunting -> Agility
    Plains, // Farming, Fighting -> Perception
    Swamp, // Fishing, Gathering -> Intelligence
    Desert, // Exploring -> Endurance
}

impl Territory {
    pub fn provides_stats(self) -> Stats {
        match self {
            Territory::Mountains => Stats {
                strength: 10,
                perception: 10,
                ..Default::default()
            },
            Territory::Forest => Stats {
                agility: 10,
                strength: 10,
                ..Default::default()
            },
            Territory::Plains => Stats {
                perception: 10,
                endurance: 10,
                ..Default::default()
            },
            Territory::Swamp => Stats {
                intelligence: 10,
                agility: 10,
                ..Default::default()
            },
            Territory::Desert => Stats {
                endurance: 10,
                intelligence: 10,
                ..Default::default()
            },
        }
    }
}