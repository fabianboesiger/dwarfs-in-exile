use enum_iterator::Sequence;
use rand::{
    rngs::SmallRng,
    seq::{IteratorRandom, SliceRandom},
    Rng, SeedableRng,
};
use serde::{Deserialize, Serialize};
use strum::Display;

#[cfg(not(debug_assertions))]
pub const SPEED: u64 = 2;
#[cfg(debug_assertions)]
pub const SPEED: u64 = 10;
pub const ONE_MINUTE: u64 = 60;
pub const ONE_HOUR: u64 = ONE_MINUTE * 60;
pub const ONE_DAY: u64 = ONE_HOUR * 24;
pub const MAX_HEALTH: Health = ONE_DAY * 3;
pub const LOOT_CRATE_COST: Money = 1000;

pub type Money = u64;
pub type Food = u64;
pub type Health = u64;

pub type UserId = i64;
pub type Time = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventData {
    pub event: Event,
    pub seed: Option<Seed>,
    pub user_id: Option<UserId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Req {
    Event(Event),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Res {
    Sync(SyncData),
    Event(EventData),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncData {
    pub user_id: UserId,
    pub state: State,
}

// MODIFY EVENTS AND STATE BELOW

use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    ops::Deref,
};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct State {
    pub players: HashMap<UserId, Player>,
    pub next_dwarf_id: DwarfId,
    pub chat: Chat,
    pub quests: Vec<Quest>,
    pub time: Time,
    pub king: Option<UserId>,
}

impl State {
    pub fn update(
        &mut self,
        EventData {
            event,
            seed,
            user_id,
        }: EventData,
    ) -> Option<()> {
        self.time += 1;

        if let Some(user_id) = user_id {
            let player = self.players.get_mut(&user_id)?;
            player.last_online = self.time;
        }

        match event {
            Event::Tick => {
                let mut rng: SmallRng = SmallRng::seed_from_u64(seed.unwrap());

                for (user_id, player) in &mut self.players {
                    // Chance for a new dwarf!
                    if rng.gen_ratio(1, ONE_DAY as u32) {
                        player.new_dwarf(seed.unwrap(), &mut self.next_dwarf_id, self.time);
                    }

                    // Let the dwarfs eat!
                    let mut sorted_by_health = player.dwarfs.values_mut().collect::<Vec<_>>();
                    sorted_by_health.sort_by_key(|dwarf| dwarf.health);
                    for dwarf in sorted_by_health {
                        dwarf.decr_health(dwarf.occupation.health_cost_per_second());
                        if dwarf.occupation == Occupation::Idling {
                            if player.base.food > 0
                                && dwarf.health <= MAX_HEALTH - MAX_HEALTH / 1000
                            {
                                player.base.food -= 1;
                                dwarf.incr_health(MAX_HEALTH / 1000);
                            }
                        }
                    }

                    // Let the dwarfs work!
                    for (_, dwarf) in &mut player.dwarfs {
                        if !dwarf.dead() {
                            dwarf.work(&mut player.inventory, seed.unwrap());
                        }
                    }

                    // Remove dead dwarfs.
                    for quest in &mut self.quests {
                        if let Some(contestant) = quest.contestants.get_mut(user_id) {
                            contestant.dwarfs.retain(|_, dwarf_id| !player.dwarfs.get(&dwarf_id).unwrap().dead());
                        }
                    }
                    player.dwarfs.retain(|_, dwarf| !dwarf.dead());
                }

                // Continue the active quests.
                for quest in &mut self.quests {
                    quest.run(&self.players);

                    if quest.done() {
                        /*
                        for (user_id, player) in self.players.iter_mut() {
                            if let Some(contestant) = quest.contestants.get(user_id) {
                                player.log.add(
                                    self.time,
                                    LogMsg::QuestCompleted(
                                        contestant.dwarfs.values().copied().collect(),
                                        quest.quest_type,
                                    ),
                                );
                            }
                        }
                        */

                        match quest.quest_type.reward_mode() {
                            RewardMode::BestGetsAll(money) => {
                                if let Some(user_id) = quest.best() {
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        if self.king.is_some() {
                                            player.money += money * 9 / 10;
                                        } else {
                                            player.money += money;
                                        }
                                        player.log.add(
                                            self.time,
                                            LogMsg::QuestCompletedMoney(quest.quest_type, money),
                                        );
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
                                    for contestant_id in quest.contestants.keys() {
                                        if *contestant_id != user_id {
                                            let player = self.players.get_mut(contestant_id)?;
                                            player.log.add(
                                                self.time,
                                                LogMsg::QuestCompletedMoney(quest.quest_type, 0),
                                            );
                                        }
                                    }
                                }
                            }
                            RewardMode::BecomeKing => {
                                if let Some(user_id) = quest.best() {
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        self.king = Some(user_id);
                                        player.log.add(
                                            self.time,
                                            LogMsg::QuestCompletedKing(quest.quest_type, true),
                                        );
                                    }
                                    for contestant_id in quest.contestants.keys() {
                                        if *contestant_id != user_id {
                                            let player = self.players.get_mut(contestant_id)?;
                                            player.log.add(
                                                self.time,
                                                LogMsg::QuestCompletedKing(quest.quest_type, false),
                                            );
                                        }
                                    }
                                }
                            }
                            RewardMode::SplitFairly(money) => {
                                for (user_id, money) in quest.split_by_score(if self.king.is_some() {
                                    money * 9 / 10
                                } else {
                                    money
                                }) {
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        player.money += money;
                                        player.log.add(
                                            self.time,
                                            LogMsg::QuestCompletedMoney(quest.quest_type, money),
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
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        player.inventory.items.add_checked(items.clone());
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
                                            let player = self.players.get_mut(contestant_id)?;
                                            player.log.add(
                                                self.time,
                                                LogMsg::QuestCompletedItems(quest.quest_type, None),
                                            );
                                        }
                                    }
                                }
                            }
                            RewardMode::Prestige => {
                                if let Some(user_id) = quest.chance_by_score(seed.unwrap()) {
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        if player.can_prestige() {
                                            player.prestige();
                                        }
                                        player.log.add(
                                            self.time,
                                            LogMsg::QuestCompletedPrestige(quest.quest_type, true),
                                        );
                                    }
                                    for contestant_id in quest.contestants.keys() {
                                        if *contestant_id != user_id {
                                            let player = self.players.get_mut(contestant_id)?;
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
                            RewardMode::NewDwarf(num_dwarfs) => {
                                if let Some(user_id) = quest.chance_by_score(seed.unwrap()) {
                                    if let Some(player) = self.players.get_mut(&user_id) {
                                        player.log.add(
                                            self.time,
                                            LogMsg::QuestCompletedDwarfs(
                                                quest.quest_type,
                                                Some(num_dwarfs),
                                            ),
                                        );
                                        for _ in 0..num_dwarfs {
                                            player
                                                .new_dwarf(seed.unwrap(), &mut self.next_dwarf_id, self.time);
                                        }
                                    }
                                    for contestant_id in quest.contestants.keys() {
                                        if *contestant_id != user_id {
                                            let player = self.players.get_mut(contestant_id)?;
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
                                let dwarf = player
                                    .dwarfs
                                    .get_mut(dwarf_id)?;
                                dwarf.participates_in_quest = None;
                                dwarf.change_occupation(Occupation::Idling);
                            }
                        }
                    }
                }

                self.quests.retain(|quest| !quest.done());

                // Add quests.
                let active_players = self.players.iter().filter(|(_, player)| player.is_active(self.time)).count();
                while self.quests.len() < 3.max(active_players / 3) {
                    let active_quests = self
                        .quests
                        .iter()
                        .map(|q| q.quest_type)
                        .collect::<HashSet<_>>();
                    let all_quests = enum_iterator::all::<QuestType>().collect::<HashSet<_>>();
                    let potential_quests = &all_quests - &active_quests;

                    if potential_quests.is_empty() {
                        break;
                    }

                    self.quests.push(Quest::new(
                        *potential_quests
                            .into_iter()
                            .collect::<Vec<_>>()
                            .choose(&mut rng)
                            .unwrap(),
                    ))
                }
            }
            Event::AddPlayer(user_id, username) => {
                let mut player = Player::new(username, self.time);
                player.new_dwarf(seed.unwrap(), &mut self.next_dwarf_id, self.time);
                self.players.insert(user_id, player);

                for players in self.players.values_mut() {
                    players.log.add(self.time, LogMsg::NewPlayer(user_id));
                }
            }
            Event::EditPlayer(user_id, username) => {
                self.players.get_mut(&user_id)?.username = username;
            }
            Event::RemovePlayer(user_id) => {
                self.players.remove(&user_id);
            }
            Event::Restart => {
                let player = self.players.get_mut(&user_id.unwrap())?;
                if player.dwarfs.len() == 0 {
                    let player = self.players.remove(&user_id.unwrap()).unwrap();
                    let username = player.username;
                    let mut player = Player::new(username, self.time);
                    player.new_dwarf(seed.unwrap(), &mut self.next_dwarf_id, self.time);
                    self.players.insert(user_id.unwrap(), player);
                }
            }
            Event::Message(message) => {
                self.chat.add_message(user_id.unwrap(), message);
            }
            Event::ChangeOccupation(dwarf_id, occupation) => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                if dwarf.participates_in_quest.is_none() && player.base.curr_level >= occupation.unlocked_at_level() {
                    dwarf.change_occupation(occupation);
                }
            }
            Event::Craft(item) => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if let Some(requires) = item.requires() {
                    if player.inventory.items.remove_checked(requires) {
                        player
                            .inventory
                            .items
                            .add_checked(Bundle::new().add(item, 1));
                    }
                }
            }
            Event::UpgradeBase => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if let Some(requires) = player.base.upgrade_cost() {
                    if player.inventory.items.remove_checked(requires) {
                        player.base.upgrade();
                    }
                }
            }
            Event::ChangeEquipment(dwarf_id, item_type, item) => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if let Some(item) = item {
                    if player
                        .inventory
                        .items
                        .remove_checked(Bundle::new().add(item, 1))
                    {
                        let equipment = player
                            .dwarfs
                            .get_mut(&dwarf_id)?
                            .equipment
                            .get_mut(&item_type)?;
                        let old_item = equipment.replace(item);
                        if let Some(old_item) = old_item {
                            player
                                .inventory
                                .items
                                .add_checked(Bundle::new().add(old_item, 1));
                        }
                    }
                } else {
                    let equipment = player
                        .dwarfs
                        .get_mut(&dwarf_id)?
                        .equipment
                        .get_mut(&item_type)?;

                    let old_item = equipment.take();
                    if let Some(old_item) = old_item {
                        player
                            .inventory
                            .items
                            .add_checked(Bundle::new().add(old_item, 1));
                    }
                }
            }
            Event::OpenLootCrate => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                player.open_loot_crate(seed.unwrap(), self.time);
            }
            Event::AssignToQuest(quest_idx, dwarf_idx, dwarf_id) => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if let Some(dwarf_id) = dwarf_id {
                    let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                    if let Some((_, old_quest_idx, old_dwarf_idx)) = dwarf.participates_in_quest {
                        let old_quest = self.quests.get_mut(old_quest_idx)?;
                        let old_contestant =
                            old_quest.contestants.entry(user_id.unwrap()).or_default();
                        old_contestant.dwarfs.remove(&old_dwarf_idx);
                    }

                    let quest = self.quests.get_mut(quest_idx)?;
                    let contestant = quest.contestants.entry(user_id.unwrap()).or_default();

                    dwarf.change_occupation(quest.quest_type.occupation());
                    dwarf.participates_in_quest = Some((quest.quest_type, quest_idx, dwarf_idx));
                    if dwarf_idx < quest.quest_type.max_dwarfs() {
                        contestant.dwarfs.insert(dwarf_idx, dwarf_id);
                    }
                } else {
                    let quest = self.quests.get_mut(quest_idx)?;
                    let contestant = quest.contestants.entry(user_id.unwrap()).or_default();

                    let old_dwarf_id = contestant.dwarfs.remove(&dwarf_idx);

                    if let Some(old_dwarf_id) = old_dwarf_id {
                        let dwarf = player.dwarfs.get_mut(&old_dwarf_id)?;
                        dwarf.change_occupation(Occupation::Idling);
                        dwarf.participates_in_quest = None;
                    }
                }
            }
            Event::AddToFoodStorage(item) => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if player
                    .inventory
                    .items
                    .remove_checked(Bundle::new().add(item, 1))
                {
                    if let Some(food) = item.nutritional_value() {
                        player.base.food += food;
                    }
                }
            }
            Event::Prestige => {
                let player = self.players.get_mut(&user_id.unwrap())?;

                if player.can_prestige() {
                    player.prestige();
                }
            }
        }

        Some(())
    }

    pub fn view(&self, _receiver: UserId) -> Self {
        State { ..self.clone() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bundle<T: BundleType>(HashMap<T, u64>);

impl<T: BundleType> Bundle<T> {
    pub fn new() -> Self {
        Bundle(HashMap::new())
    }

    pub fn add(mut self, t: T, n: u64) -> Self {
        let mut map = HashMap::new();
        map.insert(t, n);
        self.add_checked(Bundle(map));
        self
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

    pub fn sorted_by_rarity(self) -> Vec<(Item, u64)>
where {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort_by_key(|(item, _)| (item.item_rarity(), format!("{}", item)));
        vec
    }
}

impl<T: BundleType> Deref for Bundle<T> {
    type Target = HashMap<T, u64>;

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
        Bundle(HashMap::new())
    }
}

pub trait BundleType: Hash + Eq + PartialEq + Copy {
    fn max(self) -> Option<u64> {
        None
    }
}

pub trait Craftable: Sequence + BundleType {
    fn requires(self) -> Option<Bundle<Item>>;
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LogMsg {
    NewPlayer(UserId),
    NewDwarf(DwarfId),
    DwarfDied(String),
    QuestCompletedMoney(QuestType, Money),
    QuestCompletedPrestige(QuestType, bool),
    QuestCompletedKing(QuestType, bool),
    QuestCompletedItems(QuestType, Option<Bundle<Item>>),
    QuestCompletedDwarfs(QuestType, Option<usize>),
    OpenedLootCrate(Bundle<Item>),
    MoneyForKing(Money),
    NotEnoughSpaceForDwarf
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Player {
    pub username: String,
    pub base: Base,
    pub dwarfs: HashMap<DwarfId, Dwarf>,
    pub inventory: Inventory,
    pub log: Log,
    pub money: Money,
    pub last_online: Time,
}

impl Player {
    pub fn new(username: String, time: Time) -> Self {
        Player {
            username,
            dwarfs: HashMap::new(),
            base: Base::new(),
            inventory: Inventory::new(),
            log: Log::default(),
            money: 0,
            last_online: time,
        }
    }

    pub fn is_online(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_MINUTE * 3
    }

    pub fn is_active(&self, time: Time) -> bool {
        (time - self.last_online) / SPEED < ONE_DAY
    }

    pub fn new_dwarf(&mut self, seed: Seed, next_dwarf_id: &mut DwarfId, time: Time) {
        if self.dwarfs.len() < self.base.num_dwarfs() {
            self.dwarfs.insert(*next_dwarf_id, Dwarf::new(seed));
            *next_dwarf_id += 1;
        } else {
            self.log.add(
                time,
                LogMsg::NotEnoughSpaceForDwarf,
            );
        }
    }

    pub fn open_loot_crate(&mut self, seed: Seed, time: Time) {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);

        if self.money >= LOOT_CRATE_COST {
            self.money -= LOOT_CRATE_COST;
            let possible_items: Vec<Item> = enum_iterator::all::<Item>()
                .filter(|item| {
                    matches!(item.item_rarity(), ItemRarity::Epic | ItemRarity::Legendary)
                })
                .collect();
            let item = *possible_items.choose(&mut rng).unwrap();
            let bundle = Bundle::new().add(item, 1);
            self.log.add(time, LogMsg::OpenedLootCrate(bundle.clone()));
            self.inventory.items.add_checked(bundle);
        }
    }

    pub fn can_prestige(&self) -> bool {
        self.base.prestige < 10 && self.base.curr_level == self.base.max_level()
    }

    pub fn prestige(&mut self) {
        self.base.prestige += 1;
        self.base.curr_level = 1;
        self.base.food = 0;
        self.inventory.items = Bundle::new();
        self.dwarfs.retain(|_, dwarf| {
            matches!(
                dwarf.participates_in_quest,
                Some((QuestType::ExploreNewLands, _, _))
            )
        });
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub items: Bundle<Item>,
}

impl Inventory {
    fn new() -> Self {
        Inventory {
            items: Bundle::new(),
        }
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

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    Hash,
    PartialEq,
    Eq,
    Sequence,
    PartialOrd,
    Ord,
    Display,
)]
#[strum(serialize_all = "title_case")]
pub enum Item {
    Wood,
    Coal,
    Stone,
    IronOre,
    Iron,
    Nail,
    Chain,
    ChainMail,
    Bow,
    RawMeat,
    CookedMeat,
    Leather,
    Bone,
    Blueberry,
    RawFish,
    CookedFish,
    PufferFish,
    Poison,
    PoisonedBow,
    Parrot,
    String,
    Hemp,
    Wolf,
    LeatherArmor,
    Sword,
    Longsword,
    Spear,
    PoisonedSpear,
    Cat,
    Apple,
    DragonsEgg,
    Dragon,
    Donkey,
    Milk,
    Wheat,
    Egg,
    Bread,
    Flour,
    BlueberryCake,
    Potato,
    BakedPotato,
    Soup,
    Carrot,
    Crossbow,
    Pickaxe,
    Axe,
    Pitchfork,
    ApplePie,
    Bird,
    Sulfur,
    BlackPowder,
    Musket,
    Dynamite,
    Fabric,
    Backpack,
    Helmet,
    Horse,
    Map,
    FishingHat,
    FishingRod,
    Overall,
    Boots,
    Wheel,
    Wheelbarrow,
    Plough,
    Lantern,
    GoldOre,
    Gold,
    GoldenRing,
    Fluorite, // Intelligence
    Agate, // Strength 
    Sodalite, // Perception
    Ruby, // Endurance
    Selenite, // Agility
    RingOfIntelligence, // Intelligence
    RingOfStrength, // Strength 
    RingOfPerception, // Perception
    RingOfEndurance, // Endurance
    RingOfAgility, // Agility
    CrystalNecklace,
    TigerFang,
    Dagger,
    TigerFangDagger,
    RhinoHorn,
    RhinoHornHelmet,
    BearClaw,
    Gloves,
    BearClawGloves,
    BearClawBoots,
    FishingNet,
    Bag,
}

impl Into<usize> for Item {
    fn into(self) -> usize {
        self as usize
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Sequence)]
pub enum ItemType {
    Tool,
    Clothing,
    Pet,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Tool => write!(f, "Tool"),
            ItemType::Clothing => write!(f, "Clothing"),
            ItemType::Pet => write!(f, "Pet"),
        }
    }
}

impl Item {
    pub fn item_type(self) -> Option<ItemType> {
        match self {
            Item::ChainMail => Some(ItemType::Clothing),
            Item::LeatherArmor => Some(ItemType::Clothing),
            Item::Backpack => Some(ItemType::Clothing),
            Item::Helmet => Some(ItemType::Clothing),
            Item::FishingHat => Some(ItemType::Clothing),
            Item::Overall => Some(ItemType::Clothing),
            Item::Boots => Some(ItemType::Clothing),
            Item::RingOfIntelligence => Some(ItemType::Clothing),
            Item::RingOfStrength => Some(ItemType::Clothing),
            Item::RingOfPerception => Some(ItemType::Clothing),
            Item::RingOfEndurance => Some(ItemType::Clothing),
            Item::RingOfAgility => Some(ItemType::Clothing),
            Item::RhinoHornHelmet => Some(ItemType::Clothing),
            Item::Gloves => Some(ItemType::Clothing),
            Item::BearClawGloves => Some(ItemType::Clothing),

            Item::Bow => Some(ItemType::Tool),
            Item::PoisonedBow => Some(ItemType::Tool),
            Item::Sword => Some(ItemType::Tool),
            Item::Longsword => Some(ItemType::Tool),
            Item::Spear => Some(ItemType::Tool),
            Item::PoisonedSpear => Some(ItemType::Tool),
            Item::Crossbow => Some(ItemType::Tool),
            Item::Pickaxe => Some(ItemType::Tool),
            Item::Axe => Some(ItemType::Tool),
            Item::Pitchfork => Some(ItemType::Tool),
            Item::Musket => Some(ItemType::Tool),
            Item::Dynamite => Some(ItemType::Tool),
            Item::FishingRod => Some(ItemType::Tool),
            Item::Map => Some(ItemType::Tool),
            Item::Wheelbarrow => Some(ItemType::Tool),
            Item::Plough => Some(ItemType::Tool),
            Item::Lantern => Some(ItemType::Tool),
            Item::FishingNet => Some(ItemType::Tool),
            Item::TigerFangDagger => Some(ItemType::Tool),
            Item::Bag => Some(ItemType::Tool),

            Item::Parrot => Some(ItemType::Pet),
            Item::Wolf => Some(ItemType::Pet),
            Item::Cat => Some(ItemType::Pet),
            Item::Dragon => Some(ItemType::Pet),
            Item::Donkey => Some(ItemType::Pet),
            Item::Bird => Some(ItemType::Pet),
            Item::Horse => Some(ItemType::Pet),
            _ => None,
        }
    }

    pub fn provides_stats(self) -> Stats {
        match self {
            Item::ChainMail => Stats {
                endurance: -3,
                agility: -1,
                ..Default::default()
            },
            Item::LeatherArmor => Stats {
                agility: -1,
                ..Default::default()
            },
            Item::Backpack => Stats {
                agility: -4,
                ..Default::default()
            },
            Item::Musket => Stats {
                agility: -4,
                ..Default::default()
            },
            Item::Parrot => Stats {
                perception: 2,
                intelligence: 3,
                ..Default::default()
            },
            Item::Bird => Stats {
                perception: 4,
                ..Default::default()
            },
            Item::Horse => Stats {
                strength: 4,
                agility: 4,
                endurance: 4,
                ..Default::default()
            },
            Item::Boots | Item::BearClawBoots => Stats { 
                endurance: 4,
                .. Default::default()
            },
            Item::Gloves | Item::BearClawGloves => Stats { 
                agility: 4,
                .. Default::default()
            },
            Item::Map => Stats { 
                intelligence: 2,
                .. Default::default()
            },
            Item::Lantern => Stats { 
                perception: 4,
                .. Default::default()
            },
            Item::RingOfIntelligence => Stats { 
                intelligence: 8,
                .. Default::default()
            },
            Item::RingOfStrength => Stats { 
                strength: 8,
                .. Default::default()
            },
            Item::RingOfPerception => Stats { 
                perception: 8,
                .. Default::default()
            },
            Item::RingOfEndurance => Stats { 
                endurance: 8,
                .. Default::default()
            },
            Item::RingOfAgility => Stats { 
                agility: 8,
                .. Default::default()
            },
            Item::CrystalNecklace => Stats {
                strength: 6,
                endurance: 6,
                agility: 6,
                intelligence: 6,
                perception: 6,
            },
            _ => Stats::default(),
        }
    }

    pub fn nutritional_value(self) -> Option<Food> {
        if matches!(
            self,
            Item::Apple
                | Item::Blueberry
                | Item::Bread
                | Item::BlueberryCake
                | Item::CookedFish
                | Item::CookedMeat
                | Item::BakedPotato
                | Item::Soup
                | Item::ApplePie
        ) {
            let nutrition = self.item_rarity_num() / 100 + self.crafting_depth() * 5;
            Some(nutrition.max(1))
        } else {
            None
        }
    }

    // sefulness from 0-10
    pub fn usefulness_for(self, occupation: Occupation) -> u64 {
        match (self, occupation) {
            (Item::Crossbow, Occupation::Hunting | Occupation::Fighting) => 8,
            (Item::Bow, Occupation::Hunting | Occupation::Fighting) => 4,
            (Item::PoisonedBow, Occupation::Hunting | Occupation::Fighting) => 5,
            (Item::Spear, Occupation::Hunting | Occupation::Fighting) => 3,
            (Item::PoisonedSpear, Occupation::Hunting | Occupation::Fighting) => 4,
            (Item::Sword, Occupation::Fighting) => 6,
            (Item::Longsword, Occupation::Fighting) => 7,
            (Item::Dagger, Occupation::Fighting) => 5,
            (Item::TigerFangDagger, Occupation::Fighting) => 8,
            (Item::Dragon, Occupation::Hunting) => 2,
            (Item::Dragon, Occupation::Fighting) => 10,
            (Item::Donkey, Occupation::Gathering) => 6,
            (Item::Donkey, Occupation::Farming) => 4,
            (Item::Axe, Occupation::Logging) => 6,
            (Item::Axe, Occupation::Fighting) => 3,
            (Item::Pickaxe, Occupation::Mining) => 6,
            (Item::Pitchfork, Occupation::Farming) => 6,
            (Item::ChainMail, Occupation::Fighting) => 8,
            (Item::LeatherArmor, Occupation::Fighting) => 4,
            (Item::Bird, Occupation::Mining) => 3,
            (Item::Musket, Occupation::Hunting) => 8,
            (Item::Musket, Occupation::Fighting) => 5,
            (Item::Dynamite, Occupation::Fighting) => 4,
            (Item::Dynamite, Occupation::Mining) => 8,
            (Item::Backpack, Occupation::Gathering) => 7,
            (Item::Bag, Occupation::Gathering) => 5,
            (Item::Helmet, Occupation::Mining | Occupation::Logging) => 4,
            (Item::Helmet, Occupation::Fighting) => 3,
            (Item::RhinoHornHelmet, Occupation::Fighting) => 8,
            (Item::Horse, Occupation::Fighting) => 5,
            (Item::Horse, Occupation::Farming | Occupation::Logging) => 7,
            (Item::Map, Occupation::Gathering | Occupation::Exploring) => 8,
            (Item::FishingHat, Occupation::Fishing) => 6,
            (Item::FishingRod, Occupation::Fishing) => 6,
            (Item::FishingNet, Occupation::Fishing) => 10,
            (Item::Overall, Occupation::Farming | Occupation::Logging) => 8,
            (Item::Boots, Occupation::Hunting | Occupation::Gathering | Occupation::Exploring) => 3,
            (Item::BearClawBoots | Item::BearClawGloves, Occupation::Fighting) => 6,
            (Item::Wheelbarrow, Occupation::Gathering) => 8,
            (Item::Plough, Occupation::Farming) => 10,
            (Item::Lantern, Occupation::Mining) => 4,
            _ => 0,
        }
    }

    pub fn requires_stats(self) -> Stats {
        match self {
            Item::Crossbow => Stats {
                agility: 2,
                perception: 8,
                ..Default::default()
            },
            Item::Bow | Item::PoisonedBow => Stats {
                agility: 4,
                perception: 6,
                ..Default::default()
            },
            Item::Spear | Item::PoisonedSpear => Stats {
                strength: 5,
                agility: 5,
                ..Default::default()
            },
            Item::Sword => Stats {
                strength: 7,
                agility: 3,
                ..Default::default()
            },
            Item::Longsword => Stats {
                strength: 8,
                agility: 2,
                ..Default::default()
            },
            Item::Dagger | Item::TigerFangDagger => Stats {
                intelligence: 3,
                agility: 7,
                ..Default::default()
            },
            Item::Dragon => Stats {
                intelligence: 10,
                ..Default::default()
            },
            Item::Donkey => Stats {
                intelligence: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::Axe => Stats {
                strength: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::Pickaxe => Stats {
                strength: 5,
                agility: 5,
                ..Default::default()
            },
            Item::Pitchfork => Stats {
                endurance: 5,
                agility: 5,
                ..Default::default()
            },
            Item::ChainMail => Stats {
                strength: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::RhinoHornHelmet => Stats {
                strength: 5,
                agility: 5,
                ..Default::default()
            },
            Item::Bird => Stats {
                intelligence: 5,
                agility: 5,
                ..Default::default()
            },
            Item::Dynamite => Stats {
                intelligence: 8,
                perception: 2,
                ..Default::default()
            },
            Item::Musket => Stats {
                intelligence: 2,
                perception: 0,
                ..Default::default()
            },
            Item::Backpack => Stats {
                strength: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::Bag => Stats {
                strength: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::Horse => Stats {
                agility: 5,
                intelligence: 5,
                ..Default::default()
            },
            Item::FishingRod | Item::FishingNet => Stats {
                agility: 5,
                intelligence: 5,
                ..Default::default()
            },
            Item::Map => Stats {
                intelligence: 10,
                ..Default::default()
            },
            Item::Plough => Stats {
                strength: 5,
                intelligence: 5,
                ..Default::default()
            },
            Item::Wheelbarrow => Stats {
                strength: 5,
                endurance: 5,
                ..Default::default()
            },
            Item::BearClawBoots | Item::BearClawGloves => Stats {
                agility: 10,
                ..Default::default()
            },
            _ => Stats::default(),
        }
    }

    pub fn item_probability(self, occupation: Occupation) -> Option<ItemProbability> {
        match occupation {
            Occupation::Mining => match self {
                Item::Stone => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE / 2,
                }),
                Item::IronOre => Some(ItemProbability {
                    starting_from_tick: ONE_MINUTE * 3,
                    expected_ticks_per_drop: ONE_MINUTE * 3,
                }),
                Item::Coal => Some(ItemProbability {
                    starting_from_tick: ONE_MINUTE * 2,
                    expected_ticks_per_drop: ONE_MINUTE * 2,
                }),
                Item::Sulfur => Some(ItemProbability {
                    starting_from_tick: ONE_HOUR,
                    expected_ticks_per_drop: ONE_HOUR,
                }),
                Item::GoldOre => Some(ItemProbability {
                    starting_from_tick: ONE_HOUR * 2,
                    expected_ticks_per_drop: ONE_HOUR * 2,
                }),
                _ => None,
            },
            Occupation::Rockhounding => match self {
                Item::Fluorite => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Agate => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Sodalite => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Ruby => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Selenite => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                _ => None,
            },
            Occupation::Logging => match self {
                Item::Wood => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE / 2,
                }),
                Item::Apple => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Parrot => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Bird => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                _ => None,
            },
            Occupation::Hunting => match self {
                Item::RawMeat => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 2,
                }),
                Item::Leather => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 4,
                }),
                Item::Bone => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                _ => None,
            },
            Occupation::Gathering => match self {
                Item::Blueberry => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE,
                }),
                Item::Apple => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 2,
                }),
                Item::Hemp => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 3,
                }),
                _ => None,
            },
            Occupation::Fishing => match self {
                Item::RawFish => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 2,
                }),
                Item::PufferFish => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_HOUR,
                }),
                Item::Boots => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_HOUR * 4,
                }),
                Item::Gloves => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_HOUR * 4,
                }),
                _ => None,
            },
            Occupation::Fighting => match self {
                Item::Wolf => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::TigerFang => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::BearClaw => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::RhinoHorn => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                _ => None
            },
            Occupation::Exploring => match self {
                Item::Cat => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Parrot => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Bird => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Donkey => Some(ItemProbability {
                    starting_from_tick: ONE_DAY / 2,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Horse => Some(ItemProbability {
                    starting_from_tick: ONE_DAY,
                    expected_ticks_per_drop: ONE_DAY,
                }),
                _ => None,
            },
            Occupation::Farming => match self {
                Item::Milk => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Egg => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Wheat => Some(ItemProbability {
                    starting_from_tick: ONE_HOUR,
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Potato => Some(ItemProbability {
                    starting_from_tick: ONE_HOUR * 3,
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Carrot => Some(ItemProbability {
                    starting_from_tick: ONE_HOUR * 3,
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                _ => None,
            },
            Occupation::Idling => None,
        }
    }

    pub fn item_rarity_num(self) -> u64 {
        let mut rarity = None;

        let mut update_rarity = |new_rarity| {
            if let Some(rarity) = &mut rarity {
                if new_rarity < *rarity {
                    *rarity = new_rarity;
                }
            } else {
                rarity = Some(new_rarity);
            }
        };

        for occupation in enum_iterator::all::<Occupation>() {
            if let Some(item_probability) = self.item_probability(occupation) {
                update_rarity(item_probability.expected_ticks_per_drop);
            }
        }

        if let Some(requires) = self.requires() {
            update_rarity(
                requires
                    .iter()
                    .map(|(item, n)| item.item_rarity_num() * *n)
                    .sum(),
            )
        }

        rarity.unwrap_or(25000)
    }

    pub fn crafting_depth(self) -> u64 {
        let mut depth = 0;

        let mut update_depth = |new_depth| {
            depth = depth.max(new_depth);
        };

        if let Some(requires) = self.requires() {
            if let Some(max_depth) = requires.iter().map(|(item, _)| item.crafting_depth()).max() {
                update_depth(max_depth + 1)
            }
        }

        depth
    }

    pub fn item_rarity(self) -> ItemRarity {
        ItemRarity::from(self.item_rarity_num())
    }
}

#[derive(Debug, PartialEq, Eq, Display, PartialOrd, Ord)]
#[strum(serialize_all = "title_case")]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl From<u64> for ItemRarity {
    fn from(value: u64) -> Self {
        if value < 200 {
            ItemRarity::Common
        } else if value < 1000 {
            ItemRarity::Uncommon
        } else if value < 5000 {
            ItemRarity::Rare
        } else if value < 25000 {
            ItemRarity::Epic
        } else {
            ItemRarity::Legendary
        }
    }
}

pub struct ItemProbability {
    starting_from_tick: u64,
    expected_ticks_per_drop: u64,
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
    pub fn random(seed: Seed) -> Self {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);

        Stats {
            strength: rng.gen_range(1..=10),
            endurance: rng.gen_range(1..=10),
            agility: rng.gen_range(1..=10),
            intelligence: rng.gen_range(1..=10),
            perception: rng.gen_range(1..=10),
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

impl Craftable for Item {
    fn requires(self) -> Option<Bundle<Item>> {
        match self {
            Item::Iron => Some(Bundle::new().add(Item::IronOre, 1).add(Item::Coal, 1)),
            Item::Nail => Some(Bundle::new().add(Item::Iron, 1).add(Item::Coal, 1)),
            Item::Chain => Some(Bundle::new().add(Item::Iron, 5).add(Item::Coal, 2)),
            Item::ChainMail => Some(Bundle::new().add(Item::Chain, 5)),
            Item::Coal => Some(Bundle::new().add(Item::Wood, 3)),
            Item::Bow => Some(Bundle::new().add(Item::Wood, 3).add(Item::String, 1)),
            Item::CookedMeat => Some(Bundle::new().add(Item::RawMeat, 1).add(Item::Coal, 1)),
            Item::CookedFish => Some(Bundle::new().add(Item::RawFish, 1).add(Item::Coal, 1)),
            Item::Poison => Some(Bundle::new().add(Item::PufferFish, 1)),
            Item::PoisonedBow => Some(Bundle::new().add(Item::Bow, 1).add(Item::Poison, 1)),
            Item::String => Some(Bundle::new().add(Item::Hemp, 3)),
            Item::LeatherArmor => Some(Bundle::new().add(Item::Leather, 8).add(Item::String, 3)),
            Item::Sword => Some(
                Bundle::new()
                    .add(Item::Wood, 1)
                    .add(Item::Iron, 5)
            ),
            Item::Longsword => Some(
                Bundle::new()
                    .add(Item::Wood, 1)
                    .add(Item::Iron, 10)
            ),
            Item::Spear => Some(
                Bundle::new()
                    .add(Item::Wood, 3)
                    .add(Item::Iron, 2)
            ),
            Item::Dagger => Some(
                Bundle::new()
                    .add(Item::Iron, 3)
            ),
            Item::TigerFangDagger => Some(
                Bundle::new()
                    .add(Item::TigerFang, 1)
                    .add(Item::Dagger, 1)
            ),
            Item::PoisonedSpear => Some(Bundle::new().add(Item::Spear, 1).add(Item::Poison, 1)),
            Item::Dragon => Some(Bundle::new().add(Item::DragonsEgg, 1).add(Item::Coal, 100)),
            Item::BakedPotato => Some(Bundle::new().add(Item::Potato, 1).add(Item::Coal, 1)),
            Item::BlueberryCake => Some(
                Bundle::new()
                    .add(Item::Blueberry, 5)
                    .add(Item::Flour, 3)
                    .add(Item::Egg, 2)
                    .add(Item::Milk, 1),
            ),
            Item::ApplePie => Some(
                Bundle::new()
                    .add(Item::Apple, 5)
                    .add(Item::Flour, 3)
                    .add(Item::Egg, 2)
                    .add(Item::Milk, 1),
            ),
            Item::Bread => Some(Bundle::new().add(Item::Flour, 3)),
            Item::Flour => Some(Bundle::new().add(Item::Wheat, 3)),
            Item::Soup => Some(Bundle::new().add(Item::Potato, 3).add(Item::Carrot, 3)),
            Item::Pickaxe => Some(
                Bundle::new()
                    .add(Item::Wood, 5)
                    .add(Item::Iron, 10)
            ),
            Item::Axe => Some(
                Bundle::new()
                    .add(Item::Wood, 5)
                    .add(Item::Iron, 10)
            ),
            Item::Pitchfork => Some(
                Bundle::new()
                    .add(Item::Wood, 5)
                    .add(Item::Iron, 10)
            ),
            Item::Crossbow => Some(
                Bundle::new()
                    .add(Item::Wood, 5)
                    .add(Item::Iron, 10)
                    .add(Item::Nail, 3)
            ),
            Item::BlackPowder => Some(
                Bundle::new()
                    .add(Item::Coal, 2)
                    .add(Item::Sulfur, 1)
            ),
            Item::Musket => Some(
                Bundle::new()
                    .add(Item::Wood, 10)
                    .add(Item::Iron, 20)
                    .add(Item::BlackPowder, 5)
            ),
            Item::Dynamite => Some(
                Bundle::new()
                    .add(Item::BlackPowder, 10)
                    .add(Item::Fabric, 1)
            ),
            Item::Fabric => Some(
                Bundle::new()
                    .add(Item::String, 3)
            ),
            Item::Backpack => Some(
                Bundle::new()
                    .add(Item::String, 2)
                    .add(Item::Leather, 5)
            ),
            Item::Bag => Some(
                Bundle::new()
                    .add(Item::String, 1)
                    .add(Item::Fabric, 2)
            ),
            Item::Helmet => Some(
                Bundle::new()
                    .add(Item::Iron, 3)
                    .add(Item::Leather, 1)
                    .add(Item::String, 1)
            ),
            Item::RhinoHornHelmet => Some(
                Bundle::new()
                    .add(Item::RhinoHorn, 1)
                    .add(Item::Helmet, 1)
            ),
            Item::FishingRod => Some(
                Bundle::new()
                    .add(Item::Wood, 3)
                    .add(Item::String, 3)
                    .add(Item::Iron, 1)
            ),
            Item::FishingHat => Some(
                Bundle::new()
                    .add(Item::Fabric, 5)
            ),
            Item::Map => Some(
                Bundle::new()
                    .add(Item::Fabric, 5)
            ),
            Item::Overall => Some(
                Bundle::new()
                    .add(Item::Fabric, 5)
                    .add(Item::String, 5)
            ),
            Item::Boots => Some(
                Bundle::new()
                    .add(Item::Leather, 5)
                    .add(Item::String, 2)
            ),
            Item::BearClawBoots => Some(
                Bundle::new()
                    .add(Item::BearClaw, 1)
                    .add(Item::Boots, 1)
            ),
            Item::Gloves => Some(
                Bundle::new()
                    .add(Item::Leather, 5)
                    .add(Item::String, 2)
            ),
            Item::BearClawGloves => Some(
                Bundle::new()
                    .add(Item::BearClaw, 1)
                    .add(Item::Gloves, 1)
            ),
            Item::Wheel => Some(
                Bundle::new()
                    .add(Item::Iron, 3)
                    .add(Item::Wood, 5)
                    .add(Item::Nail, 5)
            ),
            Item::Wheelbarrow => Some(
                Bundle::new()
                    .add(Item::Wheel, 1)
                    .add(Item::Iron, 2)
                    .add(Item::Nail, 5)
            ),
            Item::Plough => Some(
                Bundle::new()
                    .add(Item::Wheel, 2)
                    .add(Item::Iron, 10)
                    .add(Item::Nail, 5)
                    .add(Item::Chain, 5)
            ),
            Item::Lantern => Some(
                Bundle::new()
                    .add(Item::Iron, 3)
                    .add(Item::String, 1)
            ),
            Item::Gold => Some(Bundle::new().add(Item::GoldOre, 1).add(Item::Coal, 1)),
            Item::GoldenRing => Some(Bundle::new().add(Item::Gold, 3)),
            Item::RingOfIntelligence => Some(
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Fluorite, 1)
            ),
            Item::RingOfStrength => Some(
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Agate, 1)
            ),
            Item::RingOfPerception => Some(
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Sodalite, 1)
            ),
            Item::RingOfEndurance => Some(
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Ruby, 1)
            ),
            Item::RingOfAgility => Some(
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Selenite, 1)
            ),
            Item::CrystalNecklace => Some(
                Bundle::new()
                    .add(Item::String, 1)
                    .add(Item::Fluorite, 1)
                    .add(Item::Agate, 1)
                    .add(Item::Sodalite, 1)
                    .add(Item::Ruby, 1)
                    .add(Item::Selenite, 1)
            ),
            Item::FishingNet => Some(
                Bundle::new()
                    .add(Item::String, 20)
                    .add(Item::Iron, 2)
            ),
            _ => None,
        }
    }
}

impl BundleType for Item {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dwarf {
    pub name: String,
    pub participates_in_quest: Option<(QuestType, usize, usize)>,
    pub occupation: Occupation,
    pub occupation_duration: u64,
    pub stats: Stats,
    pub equipment: HashMap<ItemType, Option<Item>>,
    pub health: Health,
}

impl Dwarf {
    fn name(seed: Seed) -> String {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);
        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let consonants = [
            'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'q', 'r', 's', 't', 'v',
            'w', 'x', 'y', 'z',
        ];

        let len = (2..8).choose(&mut rng).unwrap();

        let mut name = String::new();

        name.push(
            consonants
                .choose(&mut rng)
                .unwrap()
                .to_uppercase()
                .next()
                .unwrap(),
        );
        name.push(*vowels.choose(&mut rng).unwrap());

        for _ in 0..len {
            let mut rev_chars = name.chars().rev();
            let last_is_consonant = consonants.contains(&rev_chars.next().unwrap());
            let second_last_is_consonant = consonants.contains(&rev_chars.next().unwrap());
            if last_is_consonant {
                if second_last_is_consonant {
                    name.push(*vowels.choose(&mut rng).unwrap());
                } else {
                    if rng.gen_bool(0.4) {
                        name.push(*vowels.choose(&mut rng).unwrap());
                    } else {
                        if rng.gen_bool(0.7) {
                            name.push(*consonants.choose(&mut rng).unwrap());
                        } else {
                            let last = name.pop().unwrap();
                            name.push(last);
                            name.push(last);
                        }
                    }
                }
            } else {
                name.push(*consonants.choose(&mut rng).unwrap());
            }
        }

        name
    }

    fn new(seed: Seed) -> Self {
        let name = Dwarf::name(seed);

        Dwarf {
            name,
            occupation: Occupation::Idling,
            occupation_duration: 0,
            stats: Stats::random(seed),
            equipment: enum_iterator::all()
                .map(|item_type| (item_type, None))
                .collect(),
            health: MAX_HEALTH,
            participates_in_quest: None,
        }
    }

    pub fn dead(&self) -> bool {
        self.health == 0
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

    pub fn equipment_usefulness(&self, occupation: Occupation, item: Item) -> u64 {
        if item.requires_stats().is_zero() {
            item.usefulness_for(occupation)
        } else {
            item.usefulness_for(occupation) * self.effective_stats().cross(item.requires_stats()) / 100
        }
    }

    // output 0 - 10
    pub fn effectiveness(&self, occupation: Occupation) -> u64 {
        let mut usefulness = 0;
        for item in self.equipment.values().flatten() {
            usefulness += self.equipment_usefulness(occupation, *item);
        }
        usefulness /= self.equipment.len() as u64;

        assert!(usefulness <= 10);

        usefulness
    }

    pub fn work(&mut self, inventory: &mut Inventory, seed: Seed) {
        let mut rng = SmallRng::seed_from_u64(seed);

        for _ in 0..=self.effectiveness(self.occupation) {
            for item in enum_iterator::all::<Item>() {
                if let Some(ItemProbability {
                    starting_from_tick,
                    expected_ticks_per_drop,
                }) = item.item_probability(self.occupation)
                {
                    if self.occupation_duration >= starting_from_tick {
                        if rng.gen_ratio(1, expected_ticks_per_drop as u32) {
                            inventory.items.add_checked(Bundle::new().add(item, 1));
                        }
                    }
                }
            }
        }

        self.occupation_duration += 1;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, Sequence, PartialEq, Eq, Display)]
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
    Rockhounding
}

impl Occupation {
    pub fn health_cost_per_second(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 2,
            Occupation::Logging => 2,
            Occupation::Hunting => 2,
            Occupation::Gathering => 1,
            Occupation::Fishing => 1,
            Occupation::Fighting => 3,
            Occupation::Exploring => 1,
            Occupation::Farming => 1,
            Occupation::Rockhounding => 2,
        }
    }

    pub fn unlocked_at_level(self) -> u64 {
        match self {
            Occupation::Idling => 1,
            Occupation::Mining => 1,
            Occupation::Logging => 1,
            Occupation::Hunting => 1,
            Occupation::Gathering => 2,
            Occupation::Fishing => 3,
            Occupation::Fighting => 4,
            Occupation::Exploring => 6,
            Occupation::Farming => 8,
            Occupation::Rockhounding => 10,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

    pub fn num_dwarfs(&self) -> usize {
        self.curr_level as usize * 2
    }

    pub fn upgrade_cost(&self) -> Option<Bundle<Item>> {
        if self.curr_level < self.max_level() {
            Some(
                Bundle::new()
                    .add(Item::Wood, self.curr_level * 50)
                    .add(Item::Stone, self.curr_level * 50)
                    .add(Item::Nail, self.curr_level * 5),
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
            5 => VillageType::Town,
            6 => VillageType::LargeTown,
            7 => VillageType::City,
            8 => VillageType::LargeCity,
            9 => VillageType::Metropolis,
            10 => VillageType::Megalopolis,
            _ => panic!(),
        }
    }
}

#[derive(Display)]
#[strum(serialize_all = "title_case")]
pub enum VillageType {
    Outpost,
    Dwelling,
    Hamlet,
    Village,
    Town,
    LargeTown,
    City,
    LargeCity,
    Metropolis,
    Megalopolis,
}

pub type Seed = u64;
pub type DwarfId = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    Tick,
    AddPlayer(UserId, String),
    EditPlayer(UserId, String),
    RemovePlayer(UserId),
    Message(String),
    ChangeOccupation(DwarfId, Occupation),
    Craft(Item),
    UpgradeBase,
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
    OpenLootCrate,
    AssignToQuest(usize, usize, Option<DwarfId>),
    AddToFoodStorage(Item),
    Prestige,
    Restart,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RandEvent {}

impl EventData {
    pub fn filter(&self, _receiver: UserId) -> bool {
        /*
        let EventData { event, user_id } = self;
        let user_id = *user_id;

        match event {
            _ => true,
        }
        */
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Chat {
    pub messages: VecDeque<(UserId, String)>,
}

impl Chat {
    pub fn add_message(&mut self, user_id: UserId, message: String) {
        self.messages.push_back((user_id, message));
        if self.messages.len() > 100 {
            self.messages.pop_front();
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Quest {
    pub contestants: HashMap<UserId, Contestant>,
    pub time_left: u64,
    pub quest_type: QuestType,
}

impl Quest {
    pub fn new(quest_type: QuestType) -> Self {
        Quest {
            contestants: HashMap::new(),
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
            .map(|(user_id, c)| {
                (*user_id, num * c.achieved_score / total_score)
            })
            .collect()
    }

    pub fn chance_by_score(&self, seed: Seed) -> Option<UserId> {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);
        let total_score: u64 = self.contestants.values().map(|c| c.achieved_score).sum();
        self.contestants
            .iter()
            .map(|(user_id, c)| (*user_id, c.achieved_score as f64 / total_score as f64))
            .collect::<Vec<_>>()
            .choose_weighted(&mut rng, |elem| elem.1)
            .ok()
            .map(|item| item.0)
    }

    pub fn add_contenstant(&mut self, user_id: UserId) {
        self.contestants.insert(
            user_id,
            Contestant {
                dwarfs: HashMap::new(),
                achieved_score: 0,
            },
        );
    }

    pub fn run(&mut self, players: &HashMap<UserId, Player>) {
        if self.time_left > 0 {
            self.time_left -= 1;
            for (user_id, contestant) in &mut self.contestants {
                for dwarf_id in contestant.dwarfs.values() {
                    contestant.achieved_score += players
                        .get(user_id)
                        .unwrap()
                        .dwarfs
                        .get(dwarf_id)
                        .unwrap()
                        .effectiveness(self.quest_type.occupation())
                        + 1;
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
    Prestige,
    NewDwarf(usize),
    BecomeKing,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Contestant {
    pub dwarfs: HashMap<usize, DwarfId>,
    pub achieved_score: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Sequence, Hash)]
pub enum QuestType {
    KillTheDragon,
    ArenaFight,
    ExploreNewLands,
    FreeTheVillage,
    FeastForAGuest,
    SearchForNewDwarfs,
    AFishingFriend,
    ADwarfInDanger,
    ForTheKing,
    DrunkFishing,
    CollapsedCave,
}

impl std::fmt::Display for QuestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuestType::ArenaFight => write!(f, "Arena Fight"),
            QuestType::KillTheDragon => write!(f, "Kill the Dragon"),
            QuestType::ExploreNewLands => write!(f, "Explore New Lands"),
            QuestType::FreeTheVillage => write!(f, "Free the Elven Village"),
            QuestType::FeastForAGuest => write!(f, "A Feast for a Guest"),
            QuestType::SearchForNewDwarfs => write!(f, "A Dwarf got Lost"),
            QuestType::AFishingFriend => write!(f, "A Fishing Friend"),
            QuestType::ADwarfInDanger => write!(f, "A Dwarf in Danger"),
            QuestType::ForTheKing => write!(f, "For the King!"),
            QuestType::DrunkFishing => write!(f, "Drunk Fishing Contest"),
            QuestType::CollapsedCave => write!(f, "Trapped in the Collapsed Cave"),
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
            Self::FreeTheVillage => RewardMode::SplitFairly(1000),
            Self::FeastForAGuest => RewardMode::NewDwarf(1),
            Self::SearchForNewDwarfs => RewardMode::NewDwarf(1),
            Self::AFishingFriend => RewardMode::NewDwarf(1),
            Self::ADwarfInDanger => RewardMode::NewDwarf(1),
            Self::ForTheKing => RewardMode::BecomeKing,
            Self::DrunkFishing => RewardMode::BestGetsAll(1000),
            Self::CollapsedCave => RewardMode::NewDwarf(1),
        }
    }

    pub fn duration(self) -> u64 {
        match self {
            Self::KillTheDragon => ONE_HOUR * 3,
            Self::ArenaFight => ONE_DAY / 2,
            Self::ExploreNewLands => ONE_DAY / 2,
            Self::FreeTheVillage => ONE_HOUR * 3,
            Self::FeastForAGuest => ONE_HOUR * 3,
            Self::SearchForNewDwarfs => ONE_HOUR * 3,
            Self::AFishingFriend => ONE_HOUR * 3,
            Self::ADwarfInDanger => ONE_HOUR * 3,
            Self::ForTheKing => ONE_DAY / 2,
            Self::DrunkFishing => ONE_HOUR * 3,
            Self::CollapsedCave => ONE_HOUR * 3,
        }
    }

    pub fn occupation(self) -> Occupation {
        match self {
            Self::KillTheDragon => Occupation::Fighting,
            Self::ArenaFight => Occupation::Fighting,
            Self::ExploreNewLands => Occupation::Exploring,
            Self::FreeTheVillage => Occupation::Fighting,
            Self::FeastForAGuest => Occupation::Hunting,
            Self::SearchForNewDwarfs => Occupation::Exploring,
            Self::AFishingFriend => Occupation::Fishing,
            Self::ADwarfInDanger => Occupation::Fighting,
            Self::ForTheKing => Occupation::Fighting,
            Self::DrunkFishing => Occupation::Fishing,
            Self::CollapsedCave => Occupation::Mining,
        }
    }

    pub fn max_dwarfs(self) -> usize {
        match self {
            Self::KillTheDragon => 3,
            Self::ArenaFight => 1,
            Self::ExploreNewLands => 2,
            Self::FreeTheVillage => 3,
            Self::FeastForAGuest => 1,
            Self::SearchForNewDwarfs => 1,
            Self::AFishingFriend => 1,
            Self::ADwarfInDanger => 1,
            Self::ForTheKing => 3,
            Self::DrunkFishing => 1,
            Self::CollapsedCave => 1,
        }
    }
}
