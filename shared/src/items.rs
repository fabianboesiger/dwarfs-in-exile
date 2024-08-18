use crate::{
    Bundle, BundleType, Craftable, Food, Money, Occupation, Stats, ONE_DAY, ONE_HOUR, ONE_MINUTE,
};
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use strum::Display;

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
    Fluorite,           // Intelligence
    Agate,              // Strength
    Sodalite,           // Perception
    Ruby,               // Endurance
    Selenite,           // Agility
    RingOfIntelligence, // Intelligence
    RingOfStrength,     // Strength
    RingOfPerception,   // Perception
    RingOfEndurance,    // Endurance
    RingOfAgility,      // Agility
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
    Headlamp,
    // Diamond
    // Diamond Axe
    // Diamond Pickaxe
    // Diamand Sword
    // Enchanted Bow + Sodalite -> +Perception
    // Enchanted Longsword + Agate -> +Strength
    // Enchanted Helmet + Fluorite -> +Intelligence
    // Enchanted Boots + Selenite -> +Agility
    // Enchanted Gloves + Ruby -> +Endurance
    // Magic Lantern
    Diamond,
    DiamondAxe,
    DiamondPickaxe,
    DiamondSword,
    RhinoHornPants,
    DynamiteCrossbow,
    Dolphin,
    BoneHelmet,
    BoneNecklace,
    HorseCarriage,
    HotAirBalloon,
}

impl Craftable for Item {
    fn requires(self) -> Option<(u64, Bundle<Item>)> {
        match self {
            Item::Iron => Some((1, Bundle::new().add(Item::IronOre, 1).add(Item::Coal, 1))),
            Item::Coal => Some((1, Bundle::new().add(Item::Wood, 3))),
            Item::CookedMeat => Some((1, Bundle::new().add(Item::RawMeat, 1).add(Item::Coal, 1))),
            Item::CookedFish => Some((1, Bundle::new().add(Item::RawFish, 1).add(Item::Coal, 1))),
            Item::Pickaxe => Some((2, Bundle::new().add(Item::Wood, 5).add(Item::Iron, 10))),
            Item::Axe => Some((2, Bundle::new().add(Item::Wood, 5).add(Item::Iron, 10))),
            Item::Dagger => Some((3, Bundle::new().add(Item::Iron, 3))),
            Item::Spear => Some((4, Bundle::new().add(Item::Wood, 3).add(Item::Iron, 2))),
            Item::Sword => Some((9, Bundle::new().add(Item::Wood, 1).add(Item::Iron, 5))),
            Item::Pitchfork => Some((25, Bundle::new().add(Item::Wood, 5).add(Item::Iron, 10))),

            Item::Nail => Some((4, Bundle::new().add(Item::Iron, 1).add(Item::Coal, 1))),

            Item::Chain => Some((7, Bundle::new().add(Item::Iron, 5).add(Item::Coal, 2))),
            Item::ChainMail => Some((8, Bundle::new().add(Item::Chain, 5))),

            Item::Poison => Some((10, Bundle::new().add(Item::PufferFish, 1))),
            Item::PoisonedBow => Some((11, Bundle::new().add(Item::Bow, 1).add(Item::Poison, 1))),
            Item::PoisonedSpear => {
                Some((12, Bundle::new().add(Item::Spear, 1).add(Item::Poison, 1)))
            }

            Item::String => Some((6, Bundle::new().add(Item::Hemp, 3))),
            Item::FishingRod => Some((
                7,
                Bundle::new()
                    .add(Item::Wood, 3)
                    .add(Item::String, 3)
                    .add(Item::Iron, 1),
            )),
            Item::Bow => Some((7, Bundle::new().add(Item::Wood, 3).add(Item::String, 1))),
            Item::Fabric => Some((8, Bundle::new().add(Item::String, 3))),
            Item::Backpack => Some((9, Bundle::new().add(Item::String, 2).add(Item::Leather, 5))),
            Item::Bag => Some((10, Bundle::new().add(Item::String, 1).add(Item::Fabric, 2))),
            Item::LeatherArmor => {
                Some((10, Bundle::new().add(Item::Leather, 8).add(Item::String, 3)))
            }
            Item::Helmet => Some((
                7,
                Bundle::new()
                    .add(Item::Iron, 3)
                    .add(Item::Leather, 1)
                    .add(Item::String, 1),
            )),
            Item::Lantern => Some((12, Bundle::new().add(Item::Iron, 3).add(Item::String, 1))),
            Item::Headlamp => Some((13, Bundle::new().add(Item::Helmet, 1).add(Item::Lantern, 1))),
            Item::Map => Some((13, Bundle::new().add(Item::Fabric, 5))),
            Item::FishingHat => Some((11, Bundle::new().add(Item::Fabric, 5))),
            Item::FishingNet => Some((25, Bundle::new().add(Item::String, 20).add(Item::Iron, 2))),
            Item::Boots => Some((9, Bundle::new().add(Item::Leather, 5).add(Item::String, 2))),
            Item::BearClawBoots => {
                Some((15, Bundle::new().add(Item::BearClaw, 1).add(Item::Boots, 1)))
            }
            Item::Gloves => Some((10, Bundle::new().add(Item::Leather, 5).add(Item::String, 2))),
            Item::BearClawGloves => Some((
                16,
                Bundle::new().add(Item::BearClaw, 1).add(Item::Gloves, 1),
            )),
            Item::Overall => Some((17, Bundle::new().add(Item::Fabric, 5).add(Item::String, 5))),

            Item::BakedPotato => Some((18, Bundle::new().add(Item::Potato, 1).add(Item::Coal, 1))),
            Item::Flour => Some((21, Bundle::new().add(Item::Wheat, 3))),
            Item::Bread => Some((22, Bundle::new().add(Item::Flour, 3))),
            Item::BlueberryCake => Some((
                23,
                Bundle::new()
                    .add(Item::Blueberry, 5)
                    .add(Item::Flour, 3)
                    .add(Item::Egg, 2)
                    .add(Item::Milk, 1),
            )),
            Item::ApplePie => Some((
                23,
                Bundle::new()
                    .add(Item::Apple, 5)
                    .add(Item::Flour, 3)
                    .add(Item::Egg, 2)
                    .add(Item::Milk, 1),
            )),

            Item::Soup => Some((24, Bundle::new().add(Item::Potato, 3).add(Item::Carrot, 3))),

            Item::Crossbow => Some((
                21,
                Bundle::new()
                    .add(Item::Wood, 5)
                    .add(Item::Iron, 10)
                    .add(Item::Nail, 3),
            )),

            Item::BlackPowder => Some((28, Bundle::new().add(Item::Coal, 2).add(Item::Sulfur, 1))),
            Item::Musket => Some((
                29,
                Bundle::new()
                    .add(Item::Wood, 10)
                    .add(Item::Iron, 20)
                    .add(Item::BlackPowder, 5),
            )),
            Item::Dynamite => Some((
                30,
                Bundle::new()
                    .add(Item::BlackPowder, 10)
                    .add(Item::Fabric, 1),
            )),
            Item::DynamiteCrossbow => Some((
                32,
                Bundle::new().add(Item::Dynamite, 1).add(Item::Crossbow, 1),
            )),

            Item::Wheel => Some((
                36,
                Bundle::new()
                    .add(Item::Iron, 3)
                    .add(Item::Wood, 5)
                    .add(Item::Nail, 5),
            )),
            Item::Wheelbarrow => Some((
                38,
                Bundle::new()
                    .add(Item::Wheel, 1)
                    .add(Item::Iron, 2)
                    .add(Item::Nail, 5),
            )),
            Item::Plough => Some((
                40,
                Bundle::new()
                    .add(Item::Wheel, 2)
                    .add(Item::Iron, 20)
                    .add(Item::Nail, 20)
                    .add(Item::Wood, 50)
                    .add(Item::Chain, 5),
            )),
            Item::BoneNecklace => Some((42, Bundle::new().add(Item::String, 5).add(Item::Bone, 5))),
            Item::BoneHelmet => Some((44, Bundle::new().add(Item::Helmet, 1).add(Item::Bone, 5))),

            Item::Gold => Some((26, Bundle::new().add(Item::GoldOre, 1).add(Item::Coal, 1))),
            Item::GoldenRing => Some((28, Bundle::new().add(Item::Gold, 3))),
            Item::RingOfIntelligence => Some((
                50,
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Fluorite, 1),
            )),
            Item::RingOfStrength => Some((
                52,
                Bundle::new().add(Item::GoldenRing, 1).add(Item::Agate, 1),
            )),
            Item::RingOfPerception => Some((
                54,
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Sodalite, 1),
            )),
            Item::RingOfEndurance => Some((
                56,
                Bundle::new().add(Item::GoldenRing, 1).add(Item::Ruby, 1),
            )),
            Item::RingOfAgility => Some((
                58,
                Bundle::new()
                    .add(Item::GoldenRing, 1)
                    .add(Item::Selenite, 1),
            )),
            Item::CrystalNecklace => Some((
                60,
                Bundle::new()
                    .add(Item::String, 5)
                    .add(Item::Fluorite, 1)
                    .add(Item::Agate, 1)
                    .add(Item::Sodalite, 1)
                    .add(Item::Ruby, 1)
                    .add(Item::Selenite, 1),
            )),

            Item::DiamondAxe => Some((62, Bundle::new().add(Item::Axe, 1).add(Item::Diamond, 3))),
            Item::DiamondPickaxe => Some((
                64,
                Bundle::new().add(Item::Pickaxe, 1).add(Item::Diamond, 3),
            )),
            Item::DiamondSword => {
                Some((66, Bundle::new().add(Item::Sword, 1).add(Item::Diamond, 3)))
            }
            Item::Longsword => Some((44, Bundle::new().add(Item::Wood, 1).add(Item::Iron, 10))),
            Item::Dragon => Some((
                48,
                Bundle::new().add(Item::DragonsEgg, 1).add(Item::Coal, 100),
            )),
            Item::RhinoHornPants => Some((
                70,
                Bundle::new()
                    .add(Item::RhinoHorn, 1)
                    .add(Item::LeatherArmor, 1),
            )),
            Item::TigerFangDagger => Some((
                72,
                Bundle::new().add(Item::TigerFang, 1).add(Item::Dagger, 1),
            )),
            Item::RhinoHornHelmet => Some((
                74,
                Bundle::new().add(Item::RhinoHorn, 1).add(Item::Helmet, 1),
            )),
            Item::HorseCarriage => Some((
                76,
                Bundle::new()
                    .add(Item::Wheel, 4)
                    .add(Item::Iron, 40)
                    .add(Item::Nail, 40)
                    .add(Item::Wood, 100)
                    .add(Item::Chain, 5),
            )),
            Item::HotAirBalloon => Some((
                78,
                Bundle::new()
                    .add(Item::Fabric, 100)
                    .add(Item::String, 100)
                    .add(Item::Nail, 10)
                    .add(Item::Wood, 100)
            )),
            _ => None,
        }
    }
}

impl Into<usize> for Item {
    fn into(self) -> usize {
        self as usize
    }
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Hash, PartialEq, Eq, Sequence, PartialOrd, Ord,
)]
pub enum ItemType {
    Tool,
    Clothing,
    Pet,
    Food,
    Jewelry,
}

impl ItemType {
    pub fn equippable(&self) -> bool {
        matches!(self, Self::Tool | Self::Clothing | Self::Pet | Self::Jewelry)
    }
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Tool => write!(f, "Tool"),
            ItemType::Clothing => write!(f, "Clothing"),
            ItemType::Pet => write!(f, "Pet"),
            ItemType::Food => write!(f, "Food"),
            ItemType::Jewelry => write!(f, "Jewelry"),
        }
    }
}

impl Item {
    pub fn unlocked_at_level(&self) -> u64 {
        self.requires().map(|r| r.0).unwrap_or(1)
    }

    pub fn item_type(self) -> Option<ItemType> {
        match self {
            Item::ChainMail
            | Item::LeatherArmor
            | Item::Backpack
            | Item::Helmet
            | Item::FishingHat
            | Item::Overall
            | Item::Boots
            | Item::RhinoHornHelmet
            | Item::Gloves
            | Item::BearClawGloves
            | Item::Headlamp
            | Item::BearClawBoots
            | Item::RhinoHornPants
            | Item::BoneHelmet=> Some(ItemType::Clothing),

            Item::RingOfIntelligence
            | Item::RingOfStrength
            | Item::RingOfPerception
            | Item::RingOfEndurance
            | Item::RingOfAgility
            | Item::GoldenRing
            | Item::CrystalNecklace
            | Item::BoneNecklace => Some(ItemType::Jewelry),

            Item::Bow
            | Item::PoisonedBow
            | Item::Sword
            | Item::Longsword
            | Item::Spear
            | Item::PoisonedSpear
            | Item::Crossbow
            | Item::Pickaxe
            | Item::Axe
            | Item::Pitchfork
            | Item::Musket
            | Item::Dynamite
            | Item::FishingRod
            | Item::Map
            | Item::Wheelbarrow
            | Item::Plough
            | Item::Lantern
            | Item::FishingNet
            | Item::Dagger
            | Item::TigerFangDagger
            | Item::Bag
            | Item::DiamondAxe
            | Item::DiamondPickaxe
            | Item::DiamondSword
            | Item::DynamiteCrossbow
            | Item::HotAirBalloon
            | Item::HorseCarriage => Some(ItemType::Tool),

            Item::Parrot
            | Item::Wolf
            | Item::Cat
            | Item::Dragon
            | Item::Donkey
            | Item::Bird
            | Item::Horse
            | Item::Dolphin => Some(ItemType::Pet),

            Item::Apple
            | Item::Blueberry
            | Item::Bread
            | Item::BlueberryCake
            | Item::CookedFish
            | Item::CookedMeat
            | Item::BakedPotato
            | Item::Soup
            | Item::ApplePie => Some(ItemType::Food),
            _ => None,
        }
    }

    pub fn provides_stats(self) -> Stats {
        match self {
            Item::LeatherArmor => Stats {
                ..Default::default()
            },
            Item::Parrot => Stats {
                perception: 4,
                intelligence: 4,
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
            Item::Cat => Stats {
                agility: 6,
                perception: 6,
                ..Default::default()
            },
            Item::Boots => Stats {
                endurance: 4,
                ..Default::default()
            },
            Item::Gloves => Stats {
                agility: 4,
                ..Default::default()
            },
            Item::BearClawBoots => Stats {
                endurance: 4,
                strength: 4,
                ..Default::default()
            },
            Item::BearClawGloves => Stats {
                agility: 4,
                strength: 4,
                ..Default::default()
            },
            Item::TigerFangDagger => Stats {
                agility: 4,
                perception: 4,
                ..Default::default()
            },
            Item::Map => Stats {
                intelligence: 2,
                ..Default::default()
            },
            Item::Lantern => Stats {
                perception: 4,
                ..Default::default()
            },
            Item::Headlamp => Stats {
                perception: 6,
                ..Default::default()
            },
            Item::GoldenRing => Stats {
                strength: 1,
                endurance: 1,
                agility: 1,
                intelligence: 1,
                perception: 1,
                ..Default::default()
            },
            Item::RingOfIntelligence => Stats {
                intelligence: 8,
                ..Default::default()
            },
            Item::RingOfStrength => Stats {
                strength: 8,
                ..Default::default()
            },
            Item::RingOfPerception => Stats {
                perception: 8,
                ..Default::default()
            },
            Item::RingOfEndurance => Stats {
                endurance: 8,
                ..Default::default()
            },
            Item::RingOfAgility => Stats {
                agility: 8,
                ..Default::default()
            },
            Item::CrystalNecklace => Stats {
                strength: 6,
                endurance: 6,
                agility: 6,
                intelligence: 6,
                perception: 6,
            },
            Item::BoneNecklace => Stats {
                strength: 2,
                endurance: 2,
                agility: 2,
                intelligence: 2,
                perception: 2,
            },
            Item::RhinoHornPants => Stats {
                strength: 8,
                endurance: 8,
                intelligence: -4,
                ..Default::default()
            },
            Item::RhinoHornHelmet => Stats {
                strength: 2,
                endurance: 2,
                intelligence: -4,
                ..Default::default()
            },
            Item::Dolphin => Stats {
                agility: 4,
                ..Default::default()
            },
            _ => Stats::default(),
        }
    }

    pub fn nutritional_value(self) -> Option<Food> {
        if self.item_type() == Some(ItemType::Food) {
            let nutrition = self.item_rarity_num() / 200 * (self.crafting_depth() + 1);
            Some(nutrition.max(1))
        } else {
            None
        }
    }

    pub fn money_value(self, qty: u64) -> Money {
        self.item_rarity_num() * qty / 5000
    }

    // sefulness from 0 - 10
    pub fn usefulness_for(self, occupation: Occupation) -> u64 {
        match (self, occupation) {
            (Item::Crossbow, Occupation::Hunting | Occupation::Fighting) => 7,
            (Item::DynamiteCrossbow, Occupation::Hunting) => 5,
            (Item::DynamiteCrossbow, Occupation::Fighting) => 9,
            (Item::Bow, Occupation::Hunting | Occupation::Fighting) => 5,
            (Item::PoisonedBow, Occupation::Hunting | Occupation::Fighting) => 8,
            (Item::Spear, Occupation::Hunting | Occupation::Fighting) => 4,
            (Item::PoisonedSpear, Occupation::Hunting | Occupation::Fighting) => 7,
            (Item::Sword, Occupation::Fighting) => 6,
            (Item::DiamondSword, Occupation::Fighting) => 10,
            (Item::Longsword, Occupation::Fighting) => 7,
            (Item::Dagger, Occupation::Fighting) => 5,
            (Item::TigerFangDagger, Occupation::Fighting) => 8,
            (Item::Dragon, Occupation::Hunting) => 4,
            (Item::Dragon, Occupation::Fighting) => 10,
            (Item::Donkey, Occupation::Gathering) => 6,
            (Item::Donkey, Occupation::Farming) => 6,
            (Item::Donkey, Occupation::Exploring) => 6,
            (Item::Wolf, Occupation::Hunting) => 10,
            (Item::Wolf, Occupation::Fighting) => 4,
            (Item::Axe, Occupation::Logging) => 6,
            (Item::Axe, Occupation::Fighting) => 3,
            (Item::DiamondAxe, Occupation::Logging) => 10,
            (Item::DiamondAxe, Occupation::Fighting) => 3,
            (Item::Pickaxe, Occupation::Mining | Occupation::Rockhounding) => 6,
            (Item::DiamondPickaxe, Occupation::Mining | Occupation::Rockhounding) => 10,
            (Item::Pitchfork, Occupation::Farming) => 6,
            (Item::ChainMail, Occupation::Fighting) => 8,
            (Item::LeatherArmor, Occupation::Fighting) => 4,
            (Item::RhinoHornPants, Occupation::Fighting) => 6,
            (Item::Bird, Occupation::Mining | Occupation::Rockhounding) => 3,
            (Item::Musket, Occupation::Hunting) => 10,
            (Item::Musket, Occupation::Fighting) => 6,
            (Item::Dynamite, Occupation::Fighting) => 5,
            (Item::Dynamite, Occupation::Mining) => 10,
            (Item::HorseCarriage, Occupation::Gathering) => 10,
            (Item::Backpack, Occupation::Gathering) => 7,
            (Item::Bag, Occupation::Gathering) => 5,
            (Item::Helmet, Occupation::Mining | Occupation::Logging | Occupation::Rockhounding) => {
                4
            }
            (Item::Helmet, Occupation::Fighting) => 6,
            (Item::Headlamp, Occupation::Mining | Occupation::Rockhounding) => 8,
            (Item::RhinoHornHelmet, Occupation::Fighting) => 9,
            (Item::BoneHelmet, Occupation::Fighting) => 8,
            (Item::Horse, Occupation::Fighting | Occupation::Exploring) => 4,
            (Item::Horse, Occupation::Farming | Occupation::Logging) => 7,
            (Item::HotAirBalloon, Occupation::Exploring) => 10,
            (Item::Map, Occupation::Exploring) => 6,
            (Item::Map, Occupation::Gathering) => 4,
            (Item::FishingHat, Occupation::Fishing) => 6,
            (Item::FishingRod, Occupation::Fishing) => 6,
            (Item::FishingNet, Occupation::Fishing) => 10,
            (Item::Overall, Occupation::Farming | Occupation::Logging) => 8,
            (
                Item::Boots | Item::BearClawBoots,
                Occupation::Hunting | Occupation::Gathering | Occupation::Exploring,
            ) => 4,
            (
                Item::Gloves | Item::BearClawGloves,
                Occupation::Mining | Occupation::Logging | Occupation::Rockhounding,
            ) => 4,
            (Item::BearClawBoots | Item::BearClawGloves, Occupation::Fighting) => 6,
            (Item::Wheelbarrow, Occupation::Gathering) => 8,
            (Item::Plough, Occupation::Farming) => 10,
            (Item::Lantern, Occupation::Mining | Occupation::Rockhounding) => 4,
            (Item::Dolphin, Occupation::Fishing) => 10,
            _ => 0,
        }
    }

    pub fn item_probability(self, occupation: Occupation) -> Option<ItemProbability> {
        match occupation {
            Occupation::Mining => match self {
                Item::Stone => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE,
                }),
                Item::IronOre => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Coal => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Sulfur => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR,
                }),
                Item::GoldOre => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR * 2,
                }),
                _ => None,
            },
            Occupation::Rockhounding => match self {
                Item::Fluorite => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Agate => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Sodalite => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Ruby => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Selenite => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::Diamond => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                _ => None,
            },
            Occupation::Logging => match self {
                Item::Wood => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE,
                }),
                Item::Apple => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Bird => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                _ => None,
            },
            Occupation::Hunting => match self {
                Item::RawMeat => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Leather => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Bone => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                _ => None,
            },
            Occupation::Gathering => match self {
                Item::Blueberry => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Apple => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Hemp => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 3,
                }),
                Item::Parrot => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                _ => None,
            },
            Occupation::Fishing => match self {
                Item::RawFish => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::PufferFish => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR,
                }),
                Item::Boots => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR * 2,
                }),
                Item::Gloves => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR * 2,
                }),
                Item::GoldenRing => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_HOUR * 6,
                }),
                Item::Dolphin => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                _ => None,
            },
            Occupation::Fighting => match self {
                Item::Wolf => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                Item::TigerFang => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::BearClaw => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::RhinoHorn => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY,
                }),
                Item::RawMeat => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 20,
                }),
                Item::Leather => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 20,
                }),
                Item::Bone => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 50,
                }),
                _ => None,
            },
            Occupation::Exploring => match self {
                Item::Cat => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                Item::Donkey => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                Item::Horse => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_DAY * 7,
                }),
                _ => None,
            },
            Occupation::Farming => match self {
                Item::Milk => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Egg => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 5,
                }),
                Item::Wheat => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Potato => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Carrot => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE * 10,
                }),
                Item::Hemp => Some(ItemProbability {
                    expected_ticks_per_drop: ONE_MINUTE,
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
                    .1
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
            if let Some(max_depth) = requires
                .1
                .iter()
                .map(|(item, _)| item.crafting_depth())
                .max()
            {
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
        if value < 500 {
            ItemRarity::Common
        } else if value < 2000 {
            ItemRarity::Uncommon
        } else if value < 10000 {
            ItemRarity::Rare
        } else if value < 40000 {
            ItemRarity::Epic
        } else {
            ItemRarity::Legendary
        }
    }
}

pub struct ItemProbability {
    pub expected_ticks_per_drop: u64,
}

impl BundleType for Item {}
