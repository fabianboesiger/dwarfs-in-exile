use enum_iterator::Sequence;
use rand::{
    rngs::SmallRng,
    seq::{IteratorRandom, SliceRandom},
    Rng, SeedableRng,
};
use serde::{Deserialize, Serialize};

#[cfg(not(debug_assertions))]
pub const MILLIS_PER_TICK: u64 = 1000;
#[cfg(debug_assertions)]
pub const MILLIS_PER_TICK: u64 = 100;
pub const MAX_HEALTH: Health = 86400;
pub const ONE_DAY: u64 = 60 * 60 * 24;
pub const ONE_HOUR: u64 = 60 * 60;
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

#[derive(Serialize, Deserialize, Clone)]
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

        match event {
            Event::Tick => {
                let mut rng: SmallRng = SmallRng::seed_from_u64(seed.unwrap());

                for (_, player) in &mut self.players {
                    // Let the dwarfs eat!
                    let mut sorted_by_health = player.dwarfs.values_mut().collect::<Vec<_>>();
                    sorted_by_health.sort_by_key(|dwarf| dwarf.health);
                    for dwarf in sorted_by_health {
                        if player.base.food > 0 {
                            player.base.food -= 1;
                            dwarf.incr_health(1);
                        } else {
                            dwarf.decr_health(1);
                        }
                    }

                    // Let the dwarfs work!
                    for (_, dwarf) in &mut player.dwarfs {
                        if !dwarf.dead() {
                            dwarf.work(&mut player.inventory, seed.unwrap());
                        }
                    }

                    player.dwarfs.retain(|_, dwarf| !dwarf.dead());
                }

                // Continue the active quests.
                for quest in &mut self.quests {
                    quest.run(&self.players);
                    if quest.done() {
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
                    }
                }

                self.quests.retain(|quest| !quest.done());

                // Add quests.
                while self.quests.len() < 3 {
                    let active_quests = self
                        .quests
                        .iter()
                        .map(|q| q.quest_type)
                        .collect::<HashSet<_>>();
                    let all_quests = enum_iterator::all::<QuestType>().collect::<HashSet<_>>();
                    let potential_quests = &all_quests - &active_quests;

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
                let mut player = Player {
                    username,
                    dwarfs: HashMap::new(),
                    base: Base::new(),
                    inventory: Inventory::new(),
                    log: Log::default(),
                    money: 0,
                };

                for players in self.players.values_mut() {
                    players.log.add(self.time, LogMsg::NewPlayer(user_id));
                }

                player.new_dwarf(seed.unwrap(), &mut self.next_dwarf_id);

                self.players.insert(user_id, player);
            }
            Event::EditPlayer(user_id, username) => {
                self.players.get_mut(&user_id).unwrap().username = username;
            }
            Event::RemovePlayer(user_id) => {
                self.players.remove(&user_id);
            }
            Event::Message(message) => {
                self.chat.add_message(user_id.unwrap(), message);
            }
            Event::ChangeOccupation(dwarf_id, occupation) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

                let dwarf = player.dwarfs.get_mut(&dwarf_id)?;

                if dwarf.participates_in_quest.is_none() {
                    dwarf.change_occupation(occupation);
                }
            }
            Event::Craft(item) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

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
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

                if let Some(requires) = player.base.upgrade_cost() {
                    if player.inventory.items.remove_checked(requires) {
                        player.base.upgrade();
                    }
                }
            }
            Event::ChangeEquipment(dwarf_id, item_type, item) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

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
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

                player.open_loot_crate(seed.unwrap());
            }
            Event::AssignToQuest(quest_idx, dwarf_idx, dwarf_id) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();
                let quest = self.quests.get_mut(quest_idx)?;
                let contestant = quest.contestants.entry(user_id.unwrap()).or_default();

                if let Some(dwarf_id) = dwarf_id {
                    let dwarf = player.dwarfs.get_mut(&dwarf_id)?;
                    if dwarf.participates_in_quest.is_none() {
                        dwarf.change_occupation(quest.quest_type.occupation());
                        dwarf.participates_in_quest = Some(quest.quest_type);

                        if dwarf_idx < quest.quest_type.max_dwarfs() {
                            contestant.dwarfs.insert(dwarf_idx, dwarf_id);
                        }
                    }
                } else {
                    let old_dwarf_id = contestant.dwarfs.remove(&dwarf_idx);

                    if let Some(old_dwarf_id) = old_dwarf_id {
                        let dwarf = player.dwarfs.get_mut(&old_dwarf_id)?;
                        dwarf.change_occupation(Occupation::Idling);
                        dwarf.participates_in_quest = None;
                    }
                }
            }
            Event::AddToFoodStorage(item) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

                if player
                    .inventory
                    .items
                    .remove_checked(Bundle::new().add(item, 1))
                {
                    if let Some(food) = item.item_food() {
                        player.base.food += food;
                    }
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

impl<T: BundleType + Ord> Bundle<T> {
    pub fn sorted(self) -> Vec<(T, u64)> {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort();
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

/*
impl<'a, T: BundleType + 'a> FromIterator<&'a (T, u32)> for Bundle<T> {
    fn from_iter<I: IntoIterator<Item = &'a (T, u32)>>(iter: I) -> Self {
        Bundle(iter.into_iter().cloned().collect())
    }
}
*/

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
    QuestCompleted(Vec<DwarfId>, QuestType),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Player {
    pub username: String,
    pub base: Base,
    pub dwarfs: HashMap<DwarfId, Dwarf>,
    pub inventory: Inventory,
    pub log: Log,
    pub money: Money,
}

impl Player {
    pub fn new_dwarf(&mut self, seed: Seed, next_dwarf_id: &mut DwarfId) {
        self.dwarfs.insert(*next_dwarf_id, Dwarf::new(seed));
        *next_dwarf_id += 1;
    }

    pub fn open_loot_crate(&mut self, seed: Seed) {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);

        if self.money >= LOOT_CRATE_COST {
            self.money -= LOOT_CRATE_COST;
            let possible_items: Vec<Item> = enum_iterator::all::<Item>()
                .filter(|item| {
                    matches!(item.item_rarity(), ItemRarity::Epic | ItemRarity::Legendary)
                })
                .collect();
            let item = *possible_items.choose(&mut rng).unwrap();
            self.inventory.got_from_loot_crate = Some(item);
            self.inventory.items.add_checked(Bundle::new().add(item, 1));
        }
    }

    pub fn can_prestige(&self) -> bool {
        self.base.prestige < 10 && self.dwarfs.len() as u64 == self.base.num_dwarfs()
    }

    pub fn prestige(&mut self) {
        self.base.prestige += 1;
        self.base.food = 0;
        self.dwarfs.clear();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub items: Bundle<Item>,
    //pub loot_crates: NumLootCrates,
    pub got_from_loot_crate: Option<Item>,
}

impl Inventory {
    fn new() -> Self {
        Inventory {
            items: Bundle::new(),
            //loot_crates: 0,
            got_from_loot_crate: None,
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
    Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Sequence, PartialOrd, Ord,
)]
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

            Item::Bow => Some(ItemType::Tool),
            Item::PoisonedBow => Some(ItemType::Tool),
            Item::Sword => Some(ItemType::Tool),
            Item::Longsword => Some(ItemType::Tool),
            Item::Spear => Some(ItemType::Tool),
            Item::PoisonedSpear => Some(ItemType::Tool),

            Item::Parrot => Some(ItemType::Pet),
            Item::Wolf => Some(ItemType::Pet),
            Item::Cat => Some(ItemType::Pet),
            Item::Dragon => Some(ItemType::Pet),
            Item::Donkey => Some(ItemType::Pet),
            _ => None,
        }
    }

    pub fn item_stats(self) -> Stats {
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
            _ => Stats::default(),
        }
    }

    pub fn item_food(self) -> Option<Food> {
        match self {
            Item::Apple => Some(5),
            _ => None,
        }
    }

    // Stats, usefulness from 1-10
    pub fn item_usefulness(self, occupation: Occupation) -> Option<(Stats, i8)> {
        match (self, occupation) {
            (Item::Bow, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 3,
                    agility: 7,
                    ..Default::default()
                },
                4,
            )),
            (Item::PoisonedBow, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 3,
                    agility: 7,
                    ..Default::default()
                },
                5,
            )),
            (Item::Spear, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 5,
                    agility: 5,
                    ..Default::default()
                },
                3,
            )),
            (Item::PoisonedSpear, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 5,
                    agility: 5,
                    ..Default::default()
                },
                4,
            )),
            (Item::Sword, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 7,
                    agility: 3,
                    ..Default::default()
                },
                6,
            )),
            (Item::Longsword, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    strength: 8,
                    agility: 2,
                    ..Default::default()
                },
                7,
            )),
            (Item::Dragon, Occupation::Hunting | Occupation::Fighting) => Some((
                Stats {
                    intelligence: 10,
                    ..Default::default()
                },
                7,
            )),
            _ => None,
        }
    }

    pub fn item_probability(self, occupation: Occupation) -> Option<ItemProbability> {
        match occupation {
            Occupation::Mining => match self {
                Item::Stone => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 30,
                }),
                Item::IronOre => Some(ItemProbability {
                    starting_from_tick: 500,
                    expected_ticks_per_drop: 250,
                }),
                Item::Coal => Some(ItemProbability {
                    starting_from_tick: 100,
                    expected_ticks_per_drop: 150,
                }),
                _ => None,
            },
            Occupation::Logging => match self {
                Item::Wood => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 30,
                }),
                Item::Parrot => Some(ItemProbability {
                    starting_from_tick: 1000,
                    expected_ticks_per_drop: 1500,
                }),
                _ => None,
            },
            Occupation::Hunting => match self {
                Item::RawMeat => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 100,
                }),
                Item::Leather => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 150,
                }),
                Item::Bone => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 120,
                }),
                Item::Wolf => Some(ItemProbability {
                    starting_from_tick: 2000,
                    expected_ticks_per_drop: 10000,
                }),
                _ => None,
            },
            Occupation::Gathering => match self {
                Item::Blueberry => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 200,
                }),
                Item::Apple => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 200,
                }),
                Item::Hemp => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 150,
                }),
                Item::Cat => Some(ItemProbability {
                    starting_from_tick: 2000,
                    expected_ticks_per_drop: 10000,
                }),
                _ => None,
            },
            Occupation::Fishing => match self {
                Item::RawFish => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 80,
                }),
                Item::PufferFish => Some(ItemProbability {
                    starting_from_tick: 500,
                    expected_ticks_per_drop: 700,
                }),
                _ => None,
            },
            Occupation::Fighting => None,
            Occupation::Exploring => None,
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

    pub fn item_rarity(self) -> ItemRarity {
        ItemRarity::from(self.item_rarity_num())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl std::fmt::Display for ItemRarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemRarity::Common => write!(f, "Common"),
            ItemRarity::Uncommon => write!(f, "Uncommon"),
            ItemRarity::Rare => write!(f, "Rare"),
            ItemRarity::Epic => write!(f, "Epic"),
            ItemRarity::Legendary => write!(f, "Legendary"),
        }
    }
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
    pub charisma: i8,
}

impl Stats {
    pub fn random(seed: Seed) -> Self {
        let mut rng: SmallRng = SmallRng::seed_from_u64(seed);

        Stats {
            strength: rng.gen_range(1..=10),
            endurance: rng.gen_range(1..=10),
            agility: rng.gen_range(1..=10),
            intelligence: rng.gen_range(1..=10),
            charisma: rng.gen_range(1..=10),
        }
    }

    pub fn sum(self, other: Self) -> Self {
        Stats {
            strength: (self.strength + other.strength).min(10).max(1),
            endurance: (self.endurance + other.endurance).min(10).max(1),
            agility: (self.agility + other.agility).min(10).max(1),
            intelligence: (self.intelligence + other.intelligence).min(10).max(1),
            charisma: (self.charisma + other.charisma).min(10).max(1),
        }
    }

    pub fn cross(self, other: Self) -> i8 {
        let out = (self.strength * other.strength / 10
            + self.endurance * other.endurance / 10
            + self.agility * other.agility / 10
            + self.intelligence * other.intelligence / 10
            + self.charisma * other.charisma / 10)
            / 5;

        assert!(1 <= out && out <= 10);

        out
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Stone => write!(f, "Stone"),
            Item::Wood => write!(f, "Wood"),
            Item::Iron => write!(f, "Iron"),
            Item::Nail => write!(f, "Nail"),
            Item::Coal => write!(f, "Coal"),
            Item::IronOre => write!(f, "Iron Ore"),
            Item::Chain => write!(f, "Chain"),
            Item::ChainMail => write!(f, "Chain Mail"),
            Item::Bow => write!(f, "Bow"),
            Item::RawMeat => write!(f, "Raw Meat"),
            Item::CookedMeat => write!(f, "Cooked Meat"),
            Item::Leather => write!(f, "Leather"),
            Item::Bone => write!(f, "Bone"),
            Item::Blueberry => write!(f, "Blueberry"),
            Item::RawFish => write!(f, "Raw Fish"),
            Item::CookedFish => write!(f, "Cooked Fish"),
            Item::PufferFish => write!(f, "Puffer Fish"),
            Item::PoisonedBow => write!(f, "Poisoned Bow"),
            Item::Poison => write!(f, "Poison"),
            Item::Parrot => write!(f, "Parrot"),
            Item::Hemp => write!(f, "Hemp"),
            Item::String => write!(f, "String"),
            Item::Wolf => write!(f, "Wolf"),
            Item::LeatherArmor => write!(f, "Leather Armor"),
            Item::Sword => write!(f, "Sword"),
            Item::Longsword => write!(f, "Longsword"),
            Item::Spear => write!(f, "Spear"),
            Item::PoisonedSpear => write!(f, "Poisoned Spear"),
            Item::Cat => write!(f, "Cat"),
            Item::Apple => write!(f, "Apple"),
            Item::DragonsEgg => write!(f, "Dragons Egg"),
            Item::Dragon => write!(f, "Dragon"),
            Item::Donkey => write!(f, "Donkey"),
        }
    }
}

impl Craftable for Item {
    fn requires(self) -> Option<Bundle<Item>> {
        match self {
            Item::Iron => Some(Bundle::new().add(Item::IronOre, 1).add(Item::Coal, 1)),
            Item::Nail => Some(Bundle::new().add(Item::Iron, 2).add(Item::Coal, 1)),
            Item::Chain => Some(Bundle::new().add(Item::Iron, 5).add(Item::Coal, 2)),
            Item::ChainMail => Some(Bundle::new().add(Item::Chain, 5)),
            Item::Coal => Some(Bundle::new().add(Item::Wood, 3)),
            Item::Bow => Some(Bundle::new().add(Item::Wood, 3).add(Item::String, 1)),
            Item::CookedMeat => Some(Bundle::new().add(Item::RawMeat, 1).add(Item::Coal, 1)),
            Item::CookedFish => Some(Bundle::new().add(Item::RawFish, 1).add(Item::Coal, 1)),
            Item::Poison => Some(Bundle::new().add(Item::PufferFish, 1)),
            Item::PoisonedBow => Some(Bundle::new().add(Item::Bow, 1).add(Item::Poison, 1)),
            Item::String => Some(Bundle::new().add(Item::Hemp, 10)),
            Item::LeatherArmor => Some(Bundle::new().add(Item::Leather, 3).add(Item::String, 3)),
            Item::Sword => Some(
                Bundle::new()
                    .add(Item::Wood, 1)
                    .add(Item::Iron, 5)
                    .add(Item::Coal, 10),
            ),
            Item::Longsword => Some(
                Bundle::new()
                    .add(Item::Wood, 1)
                    .add(Item::Iron, 10)
                    .add(Item::Coal, 10),
            ),
            Item::Spear => Some(
                Bundle::new()
                    .add(Item::Wood, 3)
                    .add(Item::Iron, 2)
                    .add(Item::Coal, 3),
            ),
            Item::PoisonedSpear => Some(Bundle::new().add(Item::Spear, 1).add(Item::Poison, 1)),
            Item::Dragon => Some(Bundle::new().add(Item::DragonsEgg, 1).add(Item::Coal, 100)),
            _ => None,
        }
    }
}

impl BundleType for Item {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dwarf {
    pub name: String,
    pub participates_in_quest: Option<QuestType>,
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

    pub fn effectiveness(&self, occupation: Occupation) -> u64 {
        let mut stats = self.stats.clone();
        for item in self.equipment.values().flatten() {
            stats = stats.sum(item.item_stats());
        }

        let mut usefulness = self.equipment.len() as u64;
        for item in self.equipment.values().flatten() {
            if let Some((ideal_stats, item_usefulness)) = item.item_usefulness(occupation) {
                usefulness += (item_usefulness * stats.cross(ideal_stats) / 10) as u64;
            }
        }
        usefulness /= self.equipment.len() as u64;

        assert!(1 <= usefulness && usefulness <= 11);

        usefulness
    }

    pub fn work(&mut self, inventory: &mut Inventory, seed: Seed) {
        let mut rng = SmallRng::seed_from_u64(seed);

        for _ in 0..self.effectiveness(self.occupation) {
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

#[derive(Serialize, Deserialize, Clone, Debug, Copy, Sequence, PartialEq, Eq)]
pub enum Occupation {
    Idling,
    Mining,
    Logging,
    Hunting,
    Gathering,
    Fishing,
    Fighting,
    Exploring,
}

impl std::fmt::Display for Occupation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Occupation::Idling => write!(f, "Idling"),
            Occupation::Mining => write!(f, "Mining"),
            Occupation::Logging => write!(f, "Logging"),
            Occupation::Hunting => write!(f, "Hunting"),
            Occupation::Gathering => write!(f, "Gathering"),
            Occupation::Fishing => write!(f, "Fishing"),
            Occupation::Fighting => write!(f, "Fighting"),
            Occupation::Exploring => write!(f, "Exploring"),
        }
    }
}

impl Occupation {
    pub fn all() -> impl Iterator<Item = Occupation> {
        enum_iterator::all()
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

    pub fn num_dwarfs(&self) -> u64 {
        self.curr_level * 2
    }

    pub fn upgrade_cost(&self) -> Option<Bundle<Item>> {
        if self.curr_level < self.max_level() {
            Some(
                Bundle::new()
                    .add(Item::Wood, self.curr_level * 100)
                    .add(Item::Stone, self.curr_level * 100)
                    .add(Item::Nail, self.curr_level * 10),
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

impl std::fmt::Display for VillageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VillageType::Outpost => write!(f, "Outpost"),
            VillageType::Dwelling => write!(f, "Dwelling"),
            VillageType::Hamlet => write!(f, "Hamlet"),
            VillageType::Village => write!(f, "Village"),
            VillageType::Town => write!(f, "Town"),
            VillageType::LargeTown => write!(f, "Large Town"),
            VillageType::City => write!(f, "City"),
            VillageType::LargeCity => write!(f, "Large City"),
            VillageType::Metropolis => write!(f, "Metropolis"),
            VillageType::Megalopolis => write!(f, "Megalopolis"),
        }
    }
}

/*
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Sequence, PartialOrd, Ord,
)]
pub enum Building {
    Huts,
    Storage,
    Wall,
}

impl std::fmt::Display for Building {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Building::Huts => write!(f, "Huts"),
            Building::Storage => write!(f, "Storage"),
            Building::Wall => write!(f, "Wall"),
        }
    }
}

impl Craftable for Building {
    fn requires(self) -> Option<Bundle<Item>> {
        match self {
            Building::Huts => Some(Bundle::new().add(Item::Wood, 10)),
            _ => None,
        }
    }
}

impl BundleType for Building {}
*/

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
                        .effectiveness(self.quest_type.occupation());
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
        }
    }
}

impl QuestType {
    pub fn reward_mode(self) -> RewardMode {
        match self {
            Self::KillTheDragon => RewardMode::BestGetsItems(Bundle::new().add(Item::DragonsEgg, 1)),
            Self::ArenaFight => RewardMode::BestGetsAll(1000),
            Self::ExploreNewLands => RewardMode::Prestige,
            Self::FreeTheVillage => RewardMode::SplitFairly(500),
            Self::FeastForAGuest => RewardMode::NewDwarf(1),
            Self::SearchForNewDwarfs => RewardMode::NewDwarf(1),
            Self::AFishingFriend => RewardMode::NewDwarf(1),
            Self::ADwarfInDanger => RewardMode::NewDwarf(1),
        }
    }

    pub fn duration(self) -> u64 {
        match self {
            Self::KillTheDragon => ONE_HOUR * 3,
            Self::ArenaFight => ONE_DAY,
            Self::ExploreNewLands => ONE_DAY,
            Self::FreeTheVillage => ONE_HOUR * 3,
            Self::FeastForAGuest => ONE_HOUR,
            Self::SearchForNewDwarfs => ONE_HOUR,
            Self::AFishingFriend => ONE_HOUR,
            Self::ADwarfInDanger => ONE_HOUR,
        }
    }

    pub fn occupation(self) -> Occupation {
        match self {
            Self::KillTheDragon => Occupation::Fighting,
            Self::ArenaFight => Occupation::Fighting,
            Self::ExploreNewLands => Occupation::Exploring,
            Self::FreeTheVillage => Occupation::Fighting,
            Self::FeastForAGuest => Occupation::Hunting,
            Self::SearchForNewDwarfs => Occupation::Gathering,
            Self::AFishingFriend => Occupation::Fishing,
            Self::ADwarfInDanger => Occupation::Fighting,
        }
    }

    pub fn max_dwarfs(self) -> usize {
        match self {
            Self::KillTheDragon => 3,
            Self::ArenaFight => 1,
            Self::ExploreNewLands => 3,
            Self::FreeTheVillage => 3,
            Self::FeastForAGuest => 1,
            Self::SearchForNewDwarfs => 1,
            Self::AFishingFriend => 1,
            Self::ADwarfInDanger => 1,
        }
    }
}

