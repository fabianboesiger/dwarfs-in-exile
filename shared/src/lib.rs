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
use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
    ops::Deref,
};
use strum::Display;

#[cfg(not(debug_assertions))]
pub const SPEED: u64 = 2;
#[cfg(debug_assertions)]
pub const SPEED: u64 = 20;
pub const ONE_MINUTE: u64 = 60;
pub const ONE_HOUR: u64 = ONE_MINUTE * 60;
pub const ONE_DAY: u64 = ONE_HOUR * 24;
pub const MAX_HEALTH: Health = ONE_DAY * 3;
pub const LOOT_CRATE_COST: Money = 1000;
pub const FREE_LOOT_CRATE: u64 = ONE_DAY;
pub const WINNER_NUM_PREMIUM_DAYS: i64 = 30;

pub type Money = u64;
pub type Food = u64;
pub type Health = u64;

pub type Time = u64;

pub type QuestId = u64;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Sequence, Hash)]
pub enum TutorialStep {
    Welcome,
    Mining,
    Logging,
    SettlementExpansion2,
    Hunting,
    FoodPreparation,
    Idling,
    SettlementExpansion3,
    Quests,
    SettlementExpansion5,
    Presitge,
}

pub enum TutorialReward {
    Money(Money),
    Items(Bundle<Item>),
    Dwarfs(usize),
}

pub enum TutorialRequirement {
    Nothing,
    PrestigeLevel(u64),
    Items(Bundle<Item>),
    BaseLevel(u64),
    Food(Food),
    AnyDwarfOccupation(Occupation),
    NumberOfDwarfs(usize)
}

impl TutorialRequirement {
    pub fn complete(&self, player: &Player) -> bool {
        match self {
            TutorialRequirement::Nothing => { true }
            TutorialRequirement::PrestigeLevel(prestige) => {
                player.base.prestige >= *prestige
            }
            TutorialRequirement::Items(bundle) => {
                player
                    .inventory
                    .items
                    .check_remove(bundle)
            }
            TutorialRequirement::BaseLevel(level) => {
                player.base.curr_level >= *level
            }
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
    /*pub fn complete(&self, player: &Player) -> bool {
        match self {
            TutorialStep::Welcome => { true }
            TutorialStep::Mining => {
                player
                    .inventory
                    .items
                    .get(&Item::Stone)
                    .copied()
                    .unwrap_or(0)
                    >= 10
            }
            TutorialStep::Logging => {
                player
                    .inventory
                    .items
                    .get(&Item::Wood)
                    .copied()
                    .unwrap_or(0)
                    >= 10
            }
            TutorialStep::SettlementExpansion2 => player.base.curr_level >= 2,
            TutorialStep::Hunting => {
                player
                    .inventory
                    .items
                    .get(&Item::RawMeat)
                    .copied()
                    .unwrap_or(0)
                    >= 10
            }
            TutorialStep::FoodPreparation => player.base.food > 0,
            TutorialStep::Idling => player
                .dwarfs
                .values()
                .any(|dwarf| dwarf.actual_occupation() == Occupation::Idling),
            TutorialStep::Quests => player.dwarfs.values().any(|dwarf| {
                dwarf
                    .participates_in_quest
                    .map(|(quest_type, _, _)| {
                        matches!(
                            quest_type.reward_mode(),
                            RewardMode::NewDwarf(_) | RewardMode::NewDwarfByChance(_)
                        )
                    })
                    .unwrap_or(false)
            }),
            TutorialStep::SettlementExpansion5 => player.base.curr_level >= 5,
            TutorialStep::Presitge => player.base.prestige > 1,
        }
    }*/

    pub fn requires(&self) -> TutorialRequirement {
        match self {
            TutorialStep::Welcome => TutorialRequirement::Nothing,
            TutorialStep::Mining => TutorialRequirement::Items(Bundle::new().add(Item::Stone, 10)),
            TutorialStep::Logging => TutorialRequirement::Items(Bundle::new().add(Item::Wood, 10)),
            TutorialStep::SettlementExpansion2 => TutorialRequirement::BaseLevel(2),
            TutorialStep::Hunting => TutorialRequirement::Items(Bundle::new().add(Item::RawMeat, 10)),
            TutorialStep::FoodPreparation => TutorialRequirement::Food(1),
            TutorialStep::Idling => TutorialRequirement::AnyDwarfOccupation(Occupation::Idling),
            TutorialStep::SettlementExpansion3 => TutorialRequirement::BaseLevel(3),
            TutorialStep::Quests => TutorialRequirement::NumberOfDwarfs(3),
            TutorialStep::SettlementExpansion5 => TutorialRequirement::BaseLevel(5),
            TutorialStep::Presitge => TutorialRequirement::PrestigeLevel(2)
        }
    }

    pub fn reward(&self) -> TutorialReward {
        match self {
            TutorialStep::Welcome => TutorialReward::Dwarfs(1),
            TutorialStep::Mining => TutorialReward::Items(Bundle::new().add(Item::Stone, 50)),
            TutorialStep::Logging => TutorialReward::Items(Bundle::new().add(Item::Wood, 50)),
            TutorialStep::SettlementExpansion2 => TutorialReward::Dwarfs(1),
            TutorialStep::Hunting => TutorialReward::Items(Bundle::new().add(Item::Coal, 50)),
            TutorialStep::FoodPreparation => {
                TutorialReward::Items(Bundle::new().add(Item::CookedMeat, 50))
            }
            TutorialStep::Idling => TutorialReward::Items(Bundle::new().add(Item::Hemp, 50)),
            TutorialStep::SettlementExpansion3 => TutorialReward::Money(1000),
            TutorialStep::Quests => TutorialReward::Money(1000),
            TutorialStep::SettlementExpansion5 => TutorialReward::Dwarfs(1),
            TutorialStep::Presitge => TutorialReward::Money(1000),
        }
    }
}

impl std::fmt::Display for TutorialStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TutorialStep::Welcome => write!(f, "Welcome to the Exile"),
            TutorialStep::Mining => write!(f, "Into the Mines"),
            TutorialStep::Logging => write!(f, "Into the Woods"),
            TutorialStep::SettlementExpansion2 => write!(f, "Expand Your Settlement"),
            TutorialStep::Hunting => write!(f, "A Well Fed Population"),
            TutorialStep::FoodPreparation => write!(f, "Dinner is Ready"),
            TutorialStep::Idling => write!(f, "Time for a Break"),
            TutorialStep::SettlementExpansion3 => write!(f, "Expand Your Settlement"),
            TutorialStep::Quests => write!(f, "Make new Friends"),
            TutorialStep::SettlementExpansion5 => write!(f, "Expand Your Settlement"),
            TutorialStep::Presitge => write!(f, "Bigger and Better"),
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
}

impl engine_shared::UserData for UserData {}

/*
impl From<String> for UserData {
    fn from(username: String) -> Self {
        UserData { username }
    }
}
*/

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Sequence, Display)]
#[strum(serialize_all = "title_case")]
pub enum HireDwarfType {
    Standard,
    Advanced,
    Expert,
}

impl HireDwarfType {
    pub fn min_stars(&self) -> u64 {
        match self {
            HireDwarfType::Standard => 2,
            HireDwarfType::Advanced => 3,
            HireDwarfType::Expert => 4,
        }
    }

    pub fn cost(&self) -> u64 {
        match self {
            HireDwarfType::Standard => 2000,
            HireDwarfType::Advanced => 5000,
            HireDwarfType::Expert => 10000,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, Hash)]
pub struct State {
    pub players: CustomMap<UserId, Player>,
    pub next_dwarf_id: DwarfId,
    pub chat: Chat,
    pub next_quest_id: QuestId,
    pub quests: CustomMap<QuestId, Quest>,
    pub time: Time,
    pub king: Option<UserId>,
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

    fn sell(player: &mut Player, item: Item, qty: u64) {
        if item.money_value() > 0 {
            if player
                .inventory
                .items
                .remove_checked(Bundle::new().add(item, qty))
            {
                player.money += item.money_value() * qty;
            }
        }
    }

    fn craft(player: &mut Player, item: Item, qty: u64) {
        if let Some((level, requires)) = item.requires() {
            if player.base.curr_level >= level {
                if player.inventory.items.remove_checked(requires.mul(qty)) {
                    player
                        .inventory
                        .items
                        .add_checked(Bundle::new().add(item, qty));
                }
            }
        }
    }

    pub fn prestige(&mut self, user_id: &UserId) {
        let player = self.players.get_mut(user_id).unwrap();
        player.base.prestige += 1;
        player.base.food = 0;
        player.inventory.items = Bundle::new();
        player.prestige_quest_completed = false;
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
        return winner;
    }

    fn update(
        &mut self,
        rng: &mut impl Rng,
        event: Event<Self>,
        user_data: &CustomMap<UserId, UserData>,
    ) {
        move || -> Option<()> {
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
                                                );
                                            }
                                        }
                                    }
                                    player.tutorial_step = step.next();
                                }
                            }
                        }
                        ClientEvent::HireDwarf(dwarf_type) => {
                            if player.money >= dwarf_type.cost()
                                && player.dwarfs.len() < player.base.max_dwarfs()
                            {
                                player.money -= dwarf_type.cost();
                                let dwarf = Dwarf::new(rng, dwarf_type.min_stars() * 2);
                                player
                                    .log
                                    .add(self.time, LogMsg::NewDwarf(dwarf.name.clone()));
                                player.dwarfs.insert(self.next_dwarf_id, dwarf);
                                self.next_dwarf_id += 1;
                            }
                        }
                        ClientEvent::ToggleAutoCraft(item) => {
                            if is_premium {
                                if player.auto_functions.auto_craft.contains(&item) {
                                    player.auto_functions.auto_craft.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_craft.insert(item);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoStore(item) => {
                            if is_premium {
                                if player.auto_functions.auto_store.contains(&item) {
                                    player.auto_functions.auto_store.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_store.insert(item);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoSell(item) => {
                            if is_premium {
                                if player.auto_functions.auto_sell.contains(&item) {
                                    player.auto_functions.auto_sell.swap_remove(&item);
                                } else {
                                    player.auto_functions.auto_sell.insert(item);
                                }
                            }
                        }
                        ClientEvent::ToggleAutoIdle => {
                            if is_premium {
                                player.auto_functions.auto_idle = !player.auto_functions.auto_idle;
                            }
                        }
                        ClientEvent::Restart => {
                            if player.dwarfs.len() == 0 {
                                let player =
                                    Player::new(self.time, rng, &mut self.next_dwarf_id);
                                self.players.insert(user_id, player);
                            }
                        }
                        ClientEvent::Message(message) => {
                            self.chat.add_message(user_id, message, self.time);
                        }
                        ClientEvent::ChangeOccupation(dwarf_id, occupation) => {
                            let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                            if dwarf.participates_in_quest.is_none()
                                && player.base.curr_level >= occupation.unlocked_at_level()
                            {
                                dwarf.change_occupation(occupation);
                            }
                        }
                        ClientEvent::Craft(item, qty) => {
                            Self::craft(player, item, qty);
                        }
                        ClientEvent::UpgradeBase => {
                            if let Some(requires) = player.base.upgrade_cost() {
                                if player.inventory.items.remove_checked(requires) {
                                    player.base.upgrade();
                                }
                            }
                        }
                        ClientEvent::ChangeEquipment(dwarf_id, item_type, item) => {
                            let equipment = player
                                .dwarfs
                                .get_mut(&dwarf_id)?
                                .equipment
                                .get_mut(&item_type)?;

                            let old_item = if let Some(item) = item {
                                if item
                                    .item_type()
                                    .as_ref()
                                    .map(ItemType::equippable)
                                    .unwrap_or(false)
                                    && player
                                        .inventory
                                        .items
                                        .remove_checked(Bundle::new().add(item, 1))
                                {
                                    equipment.replace(item)
                                } else {
                                    None
                                }
                            } else {
                                equipment.take()
                            };

                            if let Some(old_item) = old_item {
                                player
                                    .inventory
                                    .items
                                    .add_checked(Bundle::new().add(old_item, 1));
                            }
                        }
                        ClientEvent::OpenLootCrate => {
                            if player.money >= LOOT_CRATE_COST {
                                player.money -= LOOT_CRATE_COST;
                                player.open_loot_crate(rng, self.time);
                            }
                        }
                        ClientEvent::OpenDailyReward => {
                            if player.reward_time <= self.time {
                                player.reward_time = self.time + FREE_LOOT_CRATE;
                                player.open_loot_crate(rng, self.time);
                            }
                        }
                        ClientEvent::AssignToQuest(quest_id, dwarf_idx, dwarf_id) => {
                            if let Some(dwarf_id) = dwarf_id {
                                let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

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
                            if is_premium {
                                Self::sell(player, item, qty);
                            }
                        }
                        ClientEvent::Prestige => {
                            if player.can_prestige() && player.prestige_quest_completed {
                                self.prestige(&user_id);
                            }
                        }
                    }
                }
                Event::ServerEvent(event) => {
                    match event {
                        ServerEvent::Tick => {
                            self.time += 1;

                            for (user_id, player) in self.players.iter_mut() {
                                let is_premium = user_data
                                    .get(user_id)
                                    .map(|user_data| user_data.premium > 0)
                                    .unwrap_or(false);

                                // Chance for a new dwarf!
                                if rng.gen_ratio(
                                    1,
                                    ONE_DAY as u32 / 20 * (21 - player.base.prestige) as u32,
                                ) {
                                    player.new_dwarf(rng, &mut self.next_dwarf_id, self.time);
                                }

                                // Let the dwarfs eat!
                                let mut sorted_by_health =
                                    player.dwarfs.values_mut().collect::<Vec<_>>();
                                sorted_by_health.sort_by_key(|dwarf| dwarf.health);
                                for dwarf in sorted_by_health {
                                    dwarf.decr_health(
                                        dwarf.actual_occupation().health_cost_per_second(),
                                    );
                                    if dwarf.actual_occupation() == Occupation::Idling {
                                        if player.base.food > 0 {
                                            if dwarf.health <= MAX_HEALTH - MAX_HEALTH / 1000 {
                                                player.base.food -= 1;
                                                dwarf.incr_health(MAX_HEALTH / 1000);
                                            } else {
                                                if player.auto_functions.auto_idle {
                                                    dwarf.auto_idle = false;
                                                }
                                            }
                                        } else if dwarf.auto_idle {
                                            dwarf.auto_idle = false;
                                        }
                                    } else {
                                        if is_premium
                                            && player.auto_functions.auto_idle
                                            && dwarf.health <= MAX_HEALTH / 10
                                            && dwarf.occupation != Occupation::Idling
                                        {
                                            dwarf.auto_idle = true;
                                        }
                                    }
                                    if dwarf.dead() {
                                        player
                                            .log
                                            .add(self.time, LogMsg::DwarfDied(dwarf.name.clone()));
                                    }
                                }

                                // Let the dwarfs work!
                                let mut added_items = Bundle::new();
                                for (_, dwarf) in player.dwarfs.iter_mut() {
                                    if !dwarf.dead() {
                                        for _ in 0..=dwarf.effectiveness(dwarf.actual_occupation())
                                        {
                                            for item in enum_iterator::all::<Item>() {
                                                if let Some(ItemProbability {
                                                    starting_from_tick,
                                                    expected_ticks_per_drop,
                                                }) =
                                                    item.item_probability(dwarf.actual_occupation())
                                                {
                                                    if dwarf.occupation_duration
                                                        >= starting_from_tick
                                                    {
                                                        if rng.gen_ratio(
                                                            1,
                                                            expected_ticks_per_drop as u32,
                                                        ) {
                                                            added_items = added_items.add(item, 1);
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        dwarf.occupation_duration += 1;
                                    }
                                }
                                player.add_items(added_items, self.time, is_premium);

                                // Remove dead dwarfs.
                                for quest in self.quests.values_mut() {
                                    if let Some(contestant) = quest.contestants.get_mut(user_id) {
                                        contestant.dwarfs.retain(|_, dwarf_id| {
                                            !player.dwarfs.get(&*dwarf_id).unwrap().dead()
                                        });
                                    }
                                }
                                player.dwarfs.retain(|_, dwarf| !dwarf.dead());
                            }

                            // Continue the active quests.
                            for quest in self.quests.values_mut() {
                                quest.run(&self.players);

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
                                                    self.king = Some(user_id);
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedKing(
                                                            quest.quest_type,
                                                            true,
                                                        ),
                                                    );
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
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
                                        RewardMode::Prestige => {
                                            if let Some(user_id) = quest.chance_by_score(rng) {
                                                if let Some(player) = self.players.get_mut(&user_id)
                                                {
                                                    player.prestige_quest_completed = true;
                                                    player.log.add(
                                                        self.time,
                                                        LogMsg::QuestCompletedPrestige(
                                                            quest.quest_type,
                                                            true,
                                                        ),
                                                    );
                                                }
                                                for contestant_id in quest.contestants.keys() {
                                                    if *contestant_id != user_id {
                                                        let player =
                                                            self.players.get_mut(contestant_id)?;
                                                        player.log.add(
                                                            self.time,
                                                            LogMsg::QuestCompletedPrestige(
                                                                quest.quest_type,
                                                                false,
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        RewardMode::NewDwarfByChance(num_dwarfs) => {
                                            if let Some(user_id) = quest.chance_by_score(rng) {
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

                            let num_quests = if cfg!(debug_assertions) {
                                30
                            } else {
                                (active_players / 5).max(3).min(30)
                            };

                            let max_prestige = self
                                .players
                                .iter()
                                .map(|(_, player)| player.base.prestige)
                                .max()
                                .unwrap_or(1);

                            let available_quests = enum_iterator::all::<QuestType>()
                                .filter(|quest_type| quest_type.is_available(max_prestige))
                                .collect::<HashSet<_>>();

                            while self.quests.len() < num_quests {
                                let disabled_quests = self
                                    .quests
                                    .values()
                                    .map(|q| q.quest_type)
                                    .filter(|quest_type| quest_type.one_at_a_time())
                                    .collect::<HashSet<_>>();

                                let potential_quests = &available_quests - &disabled_quests;

                                if potential_quests.is_empty() {
                                    break;
                                }

                                let quest = Quest::new(
                                    *potential_quests
                                        .into_iter()
                                        .collect::<Vec<_>>()
                                        .choose(rng)
                                        .unwrap(),
                                );

                                self.quests.insert(self.next_quest_id, quest);

                                self.next_quest_id += 1;
                            }
                        }
                    }
                }
            }

            Some(())
        }()
        .unwrap();
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

    pub fn can_remove_x_times(&self, other: &Self) -> Option<u64> {
        let mut bound: Option<u64> = None;

        for (t, n) in &self.0 {
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
}

impl Log {
    pub fn add(&mut self, time: Time, msg: LogMsg) {
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
    pub prestige_quest_completed: bool,
    pub reward_time: Time,
    #[serde(default = "TutorialStep::first")]
    pub tutorial_step: Option<TutorialStep>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
pub struct AutoFunctions {
    pub auto_idle: bool,
    pub auto_craft: CustomSet<Item>,
    pub auto_store: CustomSet<Item>,
    pub auto_sell: CustomSet<Item>,
}

impl Default for AutoFunctions {
    fn default() -> Self {
        Self {
            auto_idle: false,
            auto_craft: CustomSet::new(),
            auto_store: CustomSet::new(),
            auto_sell: CustomSet::new(),
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
            prestige_quest_completed: false,
            reward_time: time,
            tutorial_step: TutorialStep::first(),
        };

        if cfg!(debug_assertions) {
            player.prestige_quest_completed = true;
            player.base.curr_level = 10;
            player.money = 100000;
            player.base.prestige = 8;
            for _ in 0..4 {
                player.new_dwarf(rng, next_dwarf_id, time);
            }
        }

        player
    }

    pub fn is_online(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_MINUTE * 5
    }

    pub fn is_active(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_DAY && !self.dwarfs.is_empty()
    }

    pub fn new_dwarf(&mut self, rng: &mut impl Rng, next_dwarf_id: &mut DwarfId, time: Time) {
        if self.dwarfs.len() < self.base.max_dwarfs() {
            let dwarf = Dwarf::new(rng, self.base.prestige);
            self.log.add(time, LogMsg::NewDwarf(dwarf.name.clone()));
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
        let item = *possible_items.choose(rng).unwrap();
        let bundle = Bundle::new().add(item, (10000 / item.item_rarity_num()).max(1).min(100));
        self.log.add(time, LogMsg::OpenedLootCrate(bundle.clone()));
        self.add_items(bundle, time, true);
    }

    pub fn can_prestige(&self) -> bool {
        self.base.prestige < 10 && self.base.curr_level == self.base.max_level()
    }

    pub fn add_items(&mut self, bundle: Bundle<Item>, time: Time, is_premium: bool) {
        self.inventory.add(bundle, time);
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
                            if qty > 0 {
                                if self.inventory.items.remove_checked(requires.mul(qty)) {
                                    self.inventory.add(Bundle::new().add(item, qty), time);
    
                                    items_added = true;
                                }
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
            for &item in &self.auto_functions.auto_store {
                if let Some(&qty) = self.inventory.items.get(&item) {
                    if item.money_value() > 0 {
                        if self
                            .inventory
                            .items
                            .remove_checked(Bundle::new().add(item, qty))
                        {
                            self.base.food += item.money_value() * qty;
                        }
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
    pub fn random(rng: &mut impl Rng, prestige: u64) -> Self {
        Stats {
            strength: rng.gen_range(prestige as i8..=10),
            endurance: rng.gen_range(prestige as i8..=10),
            agility: rng.gen_range(prestige as i8..=10),
            intelligence: rng.gen_range(prestige as i8..=10),
            perception: rng.gen_range(prestige as i8..=10),
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
        let out = self.strength as u64 * other.strength as u64
            + self.endurance as u64 * other.endurance as u64
            + self.agility as u64 * other.agility as u64
            + self.intelligence as u64 * other.intelligence as u64
            + self.perception as u64 * other.perception as u64;

        out
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
    pub occupation_duration: u64,
    pub stats: Stats,
    pub equipment: CustomMap<ItemType, Option<Item>>,
    pub health: Health,
}

impl Dwarf {
    fn name(rng: &mut impl Rng) -> String {
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
                } else {
                    if rng.gen_bool(0.4) {
                        name.push(*vowels.choose(rng).unwrap());
                    } else {
                        if rng.gen_bool(0.7) {
                            name.push(*consonants.choose(rng).unwrap());
                        } else {
                            let last = name.pop().unwrap();
                            name.push(last);
                            name.push(last);
                        }
                    }
                }
            } else {
                name.push(*consonants.choose(rng).unwrap());
            }
        }

        name
    }

    fn new(rng: &mut impl Rng, prestige: u64) -> Self {
        let name = Dwarf::name(rng);

        Dwarf {
            name,
            occupation: Occupation::Idling,
            auto_idle: false,
            occupation_duration: 0,
            stats: Stats::random(rng, prestige),
            equipment: enum_iterator::all()
                .filter(ItemType::equippable)
                .map(|item_type| (item_type, None))
                .collect(),
            health: MAX_HEALTH,
            participates_in_quest: None,
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
        self.occupation_duration = 0;
    }

    pub fn effective_stats(&self) -> Stats {
        let mut stats = self.stats.clone();
        for item in self.equipment.values().flatten() {
            stats = stats.sum(item.provides_stats());
        }
        stats
    }

    // output 0 - 10
    pub fn effectiveness(&self, occupation: Occupation) -> u64 {
        let mut usefulness = 0;
        for item in self.equipment.values().flatten() {
            usefulness += item.usefulness_for(occupation).max(1);
        }
        usefulness /= self.equipment.len() as u64;

        debug_assert!(usefulness <= 10);

        let effectiveness =
            usefulness * self.effective_stats().cross(occupation.requires_stats()) / 200;

        debug_assert!(effectiveness <= 10);

        effectiveness
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
    pub fn health_cost_per_second(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 3,
            Occupation::Logging => 3,
            Occupation::Hunting => 3,
            Occupation::Gathering => 2,
            Occupation::Fishing => 2,
            Occupation::Fighting => 5,
            Occupation::Exploring => 3,
            Occupation::Farming => 3,
            Occupation::Rockhounding => 3,
        }
    }

    pub fn unlocked_at_level(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 1,
            Occupation::Logging => 1,
            Occupation::Hunting => 1,
            Occupation::Gathering => 1,
            Occupation::Fishing => 5,
            Occupation::Exploring => 10,
            Occupation::Fighting => 15,
            Occupation::Farming => 20,
            Occupation::Rockhounding => 30,
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
    pub prestige: u64,
    pub curr_level: u64,
    pub food: Food,
}

impl Base {
    pub fn new() -> Base {
        Base {
            prestige: 1,
            curr_level: 1,
            food: 0,
        }
    }

    pub fn max_level(&self) -> u64 {
        self.prestige * 10
    }

    pub fn max_dwarfs(&self) -> usize {
        self.curr_level as usize
    }

    pub fn upgrade_cost(&self) -> Option<Bundle<Item>> {
        if self.curr_level < self.max_level() {
            let multiplier = |unlocked_at_prestige: u64| {
                let prev_prestige = unlocked_at_prestige - 1;
                let starting_level = (prev_prestige * 10).saturating_sub(1);
                self.curr_level.saturating_sub(starting_level) * self.prestige.saturating_sub(prev_prestige)
            };

            Some(
                Bundle::new()
                    .add(Item::Wood, 50 * multiplier(1))
                    .add(Item::Stone, 50 * multiplier(1))
                    .add(
                        Item::Nail,
                        10 * multiplier(3),
                    )
                    .add(
                        Item::Fabric,
                        10 * multiplier(5),
                    ),
            )
        } else {
            None
        }
    }

    pub fn upgrade(&mut self) {
        self.curr_level += 1;
        assert!(self.curr_level <= self.max_level());
    }

    pub fn village_type(&self) -> VillageType {
        match self.prestige {
            1 => VillageType::Outpost,
            2 => VillageType::Dwelling,
            3 => VillageType::Hamlet,
            4 => VillageType::Village,
            5 => VillageType::SmallTown,
            6 => VillageType::LargeTown,
            7 => VillageType::SmallCity,
            8 => VillageType::LargeCity,
            9 => VillageType::Metropolis,
            10 => VillageType::Megalopolis,
            _ => panic!(),
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
    UpgradeBase,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    OpenLootCrate,
    OpenDailyReward,
    AssignToQuest(QuestId, usize, Option<DwarfId>),
    AddToFoodStorage(Item, u64),
    Sell(Item, u64),
    Prestige,
    Restart,
    ToggleAutoCraft(Item),
    ToggleAutoStore(Item),
    ToggleAutoSell(Item),
    ToggleAutoIdle,
    HireDwarf(HireDwarfType),
    NextTutorialStep,
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
    pub fn add_message(&mut self, user_id: UserId, message: String, time: Time) {
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
}

impl Quest {
    pub fn new(quest_type: QuestType) -> Self {
        Quest {
            contestants: CustomMap::new(),
            time_left: quest_type.duration(),
            quest_type,
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

    pub fn run(&mut self, players: &CustomMap<UserId, Player>) {
        if self.time_left > 0 {
            self.time_left -= 1;
            for (user_id, contestant) in self.contestants.iter_mut() {
                for dwarf_id in contestant.dwarfs.values() {
                    let dwarf = players.get(user_id).unwrap().dwarfs.get(dwarf_id).unwrap();

                    if dwarf.actual_occupation() == self.quest_type.occupation() {
                        contestant.achieved_score +=
                            dwarf.effectiveness(self.quest_type.occupation()) + 1;
                    }
                }
            }
        }
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
    Prestige,
    NewDwarf(usize),
    NewDwarfByChance(usize),
    BecomeKing,
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
    ExploreNewLands,
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
}

impl std::fmt::Display for QuestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuestType::ArenaFight => write!(f, "Arena Fight"),
            QuestType::KillTheDragon => write!(f, "Kill the Dragon"),
            QuestType::ExploreNewLands => write!(f, "Explore New Lands"),
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
            QuestType::FreeTheDwarf => write!(f, "Free the Dwarf"),
            QuestType::FarmersContest => write!(f, "Farmers Contest"),
            QuestType::CrystalsForTheElves => write!(f, "Crystals for the Elves"),
            QuestType::ADarkSecret => write!(f, "A Dark Secret"),
            QuestType::ElvenVictory => write!(f, "The Elven Victory"),
            QuestType::TheMassacre => write!(f, "The Massacre"),
            QuestType::TheElvenWar => write!(f, "The Elven War"),
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
            Self::ExploreNewLands => RewardMode::Prestige,
            Self::FreeTheVillage => RewardMode::SplitFairly(2000),
            Self::FeastForAGuest => RewardMode::NewDwarf(1),
            Self::ADwarfGotLost => RewardMode::NewDwarfByChance(1),
            Self::AFishingFriend => RewardMode::NewDwarfByChance(1),
            Self::ADwarfInDanger => RewardMode::NewDwarf(1),
            Self::ForTheKing => RewardMode::BecomeKing,
            Self::DrunkFishing => RewardMode::BestGetsAll(2000),
            Self::CollapsedCave => RewardMode::NewDwarf(1),
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
        }
    }

    pub fn one_at_a_time(self) -> bool {
        matches!(self.reward_mode(), RewardMode::BecomeKing)
    }

    pub fn duration(self) -> u64 {
        match self {
            Self::KillTheDragon => ONE_HOUR * 2,
            Self::ArenaFight => ONE_HOUR * 4,
            Self::ExploreNewLands => ONE_HOUR * 8,
            Self::FreeTheVillage => ONE_HOUR * 2,
            Self::FeastForAGuest => ONE_HOUR * 4,
            Self::ADwarfGotLost => ONE_HOUR * 2,
            Self::AFishingFriend => ONE_HOUR,
            Self::ADwarfInDanger => ONE_HOUR * 4,
            Self::ForTheKing => ONE_HOUR * 8,
            Self::DrunkFishing => ONE_HOUR * 4,
            Self::CollapsedCave => ONE_HOUR * 4,
            Self::TheHiddenTreasure => ONE_HOUR * 2,
            Self::CatStuckOnATree => ONE_HOUR,
            Self::AttackTheOrks => ONE_HOUR * 2,
            Self::FreeTheDwarf => ONE_HOUR * 2,
            Self::FarmersContest => ONE_HOUR,
            Self::CrystalsForTheElves => ONE_HOUR,
            Self::ADarkSecret => ONE_HOUR * 4,
            Self::ElvenVictory => ONE_HOUR * 2,
            Self::TheMassacre => ONE_HOUR * 8,
            Self::TheElvenWar => ONE_HOUR * 8,
        }
    }

    pub fn occupation(self) -> Occupation {
        match self {
            Self::KillTheDragon => Occupation::Fighting,
            Self::ArenaFight => Occupation::Fighting,
            Self::ExploreNewLands => Occupation::Exploring,
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
        }
    }

    pub fn max_dwarfs(self) -> usize {
        match self {
            Self::KillTheDragon => 3,
            Self::ArenaFight => 1,
            Self::ExploreNewLands => 1,
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
        }
    }

    pub fn is_available(self, max_prestige: u64) -> bool {
        match self {
            Self::FreeTheVillage => (1..=3).contains(&max_prestige),
            Self::FeastForAGuest => (1..=3).contains(&max_prestige),
            Self::ADwarfInDanger => (1..=3).contains(&max_prestige),
            Self::AttackTheOrks => (1..=6).contains(&max_prestige),
            Self::FreeTheDwarf => (3..=6).contains(&max_prestige),
            Self::ADwarfGotLost => (1..=6).contains(&max_prestige),
            Self::CrystalsForTheElves => (3..=6).contains(&max_prestige),
            Self::ElvenVictory => (5..=6).contains(&max_prestige),
            Self::ADarkSecret => (7..=8).contains(&max_prestige),
            Self::TheMassacre => (7..=8).contains(&max_prestige),
            Self::TheElvenWar => (9..=10).contains(&max_prestige),
            _ => true,
        }
    }
}
