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

pub type UserId = i64;

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
    collections::{HashMap, VecDeque},
    hash::Hash,
    ops::Deref,
};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct State {
    pub players: HashMap<UserId, Player>,
    pub next_dwarf_id: DwarfId,
    pub chat: Chat,
}

impl State {
    pub fn update(
        &mut self,
        EventData {
            event,
            seed,
            user_id,
        }: EventData,
    ) {
        match event {
            Event::Tick => {
                // Let the dwarfs work!
                for (_, player) in &mut self.players {
                    for (_, dwarf) in &mut player.dwarfs {
                        dwarf.work(&mut player.inventory, seed.unwrap());
                    }
                }
            }
            Event::AddPlayer(user_id, username) => {
                let mut player = Player {
                    username,
                    dwarfs: HashMap::new(),
                    base: Base::new(),
                    inventory: Inventory::new(),
                };

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

                player
                    .dwarfs
                    .get_mut(&dwarf_id)
                    .unwrap()
                    .change_occupation(occupation);
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
            Event::Build(building) => {
                let player = self.players.get_mut(&user_id.unwrap()).unwrap();

                if let Some(requires) = building.requires() {
                    if player.inventory.items.remove_checked(requires) {
                        player
                            .base
                            .buildings
                            .add_checked(Bundle::new().add(building, 1));
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
                            .get_mut(&dwarf_id)
                            .unwrap()
                            .equipment
                            .get_mut(&item_type)
                            .unwrap();
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
                        .get_mut(&dwarf_id)
                        .unwrap()
                        .equipment
                        .get_mut(&item_type)
                        .unwrap();
                    let old_item = equipment.take();
                    if let Some(old_item) = old_item {
                        player
                            .inventory
                            .items
                            .add_checked(Bundle::new().add(old_item, 1));
                    }
                }
            }
        }
    }

    pub fn view(&self, _receiver: UserId) -> Self {
        State { ..self.clone() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bundle<T: BundleType>(HashMap<T, u32>);

impl<T: BundleType> Bundle<T> {
    pub fn new() -> Self {
        Bundle(HashMap::new())
    }

    pub fn add(mut self, t: T, n: u32) -> Self {
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
    pub fn sorted(self) -> Vec<(T, u32)> {
        let mut vec: Vec<_> = self.0.into_iter().collect();
        vec.sort();
        vec
    }
}

impl<T: BundleType> Deref for Bundle<T> {
    type Target = HashMap<T, u32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: BundleType> FromIterator<(T, u32)> for Bundle<T> {
    fn from_iter<I: IntoIterator<Item = (T, u32)>>(iter: I) -> Self {
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
    fn max(self) -> Option<u32> {
        None
    }
}

pub trait Craftable: Sequence + BundleType {
    fn requires(self) -> Option<Bundle<Item>>;
    fn all() -> Bundle<Self> {
        enum_iterator::all().map(|t| (t, 0)).collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Player {
    pub username: String,
    pub base: Base,
    pub dwarfs: HashMap<DwarfId, Dwarf>,
    pub inventory: Inventory,
}

impl Player {
    pub fn new_dwarf(&mut self, seed: Seed, next_dwarf_id: &mut DwarfId) {
        self.dwarfs.insert(*next_dwarf_id, Dwarf::new(seed));
        *next_dwarf_id += 1;
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
            Item::Bow => Some(ItemType::Tool),
            Item::PoisonedBow => Some(ItemType::Tool),
            Item::Parrot => Some(ItemType::Pet),
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
            Item::Parrot => Stats {
                charisma: 2,
                ..Default::default()
            },
            _ => Stats::default(),
        }
    }

    pub fn item_ideal_stats(self) -> Stats {
        match self {
            Item::Bow => Stats {
                strength: 5,
                agility: 5,
                ..Default::default()
            },
            Item::PoisonedBow => Stats {
                strength: 5,
                agility: 5,
                ..Default::default()
            },
            _ => Stats::default(),
        }
    }

    pub fn item_usefulness(self, occupation: Occupation) -> Option<i8> {
        match (self, occupation) {
            (Item::Bow, Occupation::Hunting) => Some(4),
            (Item::PoisonedBow, Occupation::Hunting) => Some(5),
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
                    expected_ticks_per_drop: 500,
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
                _ => None,
            },
            Occupation::Gathering => match self {
                Item::Blueberry => Some(ItemProbability {
                    starting_from_tick: 0,
                    expected_ticks_per_drop: 100,
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
                    expected_ticks_per_drop: 500,
                }),
                _ => None,
            },
            Occupation::Idling => None,
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
        (self.strength * other.strength / 10
            + self.endurance * other.endurance / 10
            + self.agility * other.agility / 10
            + self.intelligence * other.intelligence / 10
            + self.charisma * other.charisma / 10)
            / 5
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
        }
    }
}

impl Craftable for Item {
    fn requires(self) -> Option<Bundle<Item>> {
        match self {
            Item::Iron => Some(Bundle::new().add(Item::IronOre, 1).add(Item::Coal, 3)),
            Item::Nail => Some(Bundle::new().add(Item::Iron, 2).add(Item::Coal, 3)),
            Item::Chain => Some(Bundle::new().add(Item::Iron, 5).add(Item::Coal, 3)),
            Item::ChainMail => Some(Bundle::new().add(Item::Chain, 5)),
            Item::Coal => Some(Bundle::new().add(Item::Wood, 3)),
            Item::Bow => Some(Bundle::new().add(Item::Wood, 3)),
            Item::CookedMeat => Some(Bundle::new().add(Item::RawMeat, 1).add(Item::Coal, 1)),
            Item::CookedFish => Some(Bundle::new().add(Item::RawFish, 1).add(Item::Coal, 1)),
            Item::Poison => Some(Bundle::new().add(Item::PufferFish, 1)),
            Item::PoisonedBow => Some(Bundle::new().add(Item::Bow, 1).add(Item::Poison, 1)),
            _ => None,
        }
    }
}

impl BundleType for Item {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dwarf {
    pub name: String,
    pub occupation: Occupation,
    pub occupation_duration: u64,
    pub stats: Stats,
    pub equipment: HashMap<ItemType, Option<Item>>,
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
        }
    }

    pub fn change_occupation(&mut self, occupation: Occupation) {
        self.occupation = occupation;
        self.occupation_duration = 0;
    }

    pub fn work(&mut self, inventory: &mut Inventory, seed: Seed) {
        let mut rng = SmallRng::seed_from_u64(seed);

        let mut stats = self.stats.clone();
        for item in self.equipment.values().flatten() {
            stats = stats.sum(item.item_stats());
        }

        let mut usefulness = 0;
        for item in self.equipment.values().flatten() {
            usefulness += item
                .item_usefulness(self.occupation)
                .map(|usefulness| usefulness * stats.cross(item.item_ideal_stats()) / 10)
                .unwrap_or_default();
        }
        usefulness /= self.equipment.len() as i8;

        assert!(0 <= usefulness && usefulness <= 10);

        for _ in 0..=usefulness {
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
    pub max: u32,
    pub buildings: Bundle<Building>,
}

impl Base {
    pub fn new() -> Base {
        Base {
            max: 10,
            buildings: Bundle::new(),
        }
    }
}

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
    Build(Building),
    ChangeEquipment(DwarfId, ItemType, Option<Item>),
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
