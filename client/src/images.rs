use std::convert::TryInto;

use rand::Rng;
use rand_chacha::{rand_core::SeedableRng, ChaCha8Rng};
use seed::virtual_dom::{AsAtValue, AtValue};
use sha2::{Digest, Sha256};
use shared::{Dwarf, Item, Occupation, QuestType, Territory, VillageType, WorldEvent, FEMALE_PROBABILITY};
use strum::Display;

#[derive(Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Image {
    #[allow(unused)]
    Placeholder,
    Dwarf(u64),
    FemaleDwarf(u64),
    ChildDwarf(u64),
    ChildFemaleDwarf(u64),
    Blueberry,
    ChainMail,
    Coal,
    Helmet,
    Nail,
    Stone,
    Wood,
    Apple,
    ApplePie,
    Backpack,
    BlackPowder,
    Cat,
    Chain,
    Dagger,
    Donkey,
    Dragon,
    FishingRod,
    Horse,
    Parrot,
    Poison,
    RawMeat,
    Boots,
    Sulfur,
    Sword,
    Wolf,
    ADwarfInDanger,
    AFishingFriend,
    CollapsedCave,
    DrunkFishing,
    FreeTheVillage,
    KillTheDragon,
    ForTheKing,
    ADwarfGotLost,
    ArenaFight,
    FeastForAGuest,
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
    Idling,
    Mining,
    Logging,
    Farming,
    Rockhounding,
    Fishing,
    Hunting,
    Gathering,
    Fighting,
    Exploring,
    BakedPotato,
    Lantern,
    Pickaxe,
    Potato,
    BlueberryCake,
    Wheat,
    BearClaw,
    Map,
    Egg,
    Bow,
    Crossbow,
    RhinoHorn,
    Axe,
    BearClawGloves,
    Pitchfork,
    RhinoHornHelmet,
    Spear,
    CookedMeat,
    RawFish,
    DragonEgg,
    Milk,
    Soup,
    Plough,
    PoisonedBow,
    BearClawBoots,
    Dynamite,
    Bird,
    Bread,
    TigerFang,
    Longsword,
    TigerFangDagger,
    Flour,
    CookedFish,
    PoisonedSpear,
    FishingNet,
    Overall,
    String,
    Wheel,
    FishingHat,
    Ruby,
    RingOfEndurance,
    Hemp,
    Musket,
    Fluorite,
    RingOfIntelligence,
    RingOfStrength,
    Agate,
    Sodalite,
    RingOfPerception,
    RingOfAgility,
    Selenite,
    CrystalNecklace,
    Wheelbarrow,
    GoldenRing,
    Bone,
    IronOre,
    GoldOre,
    Iron,
    Gold,
    Pufferfish,
    LeatherArmor,
    Carrot,
    Bag,
    Fabric,
    Gloves,
    Leather,
    Headlamp,
    Diamond,
    DiamondAxe,
    DiamondPickaxe,
    DiamondSword,
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
    DynamiteCrossbow,
    RhinoHornPants,
    Dolphin,
    Drought,
    Flood,
    Earthquake,
    Plague,
    BoneNecklace,
    BoneHelmet,
    King,
    Manager,
    Tornado,
    Concert,
    MagicalBerries,
    EatingContest,
    Socializing,
    HotAirBalloon,
    HorseCarriage,
    Carnival,
    TheElvenMagician,
    FullMoon,
    Starvation,
    Vest,
    Boat,
    ExploreNewLands,
    Ox,
    Kobold,
    Revolution,
    KnightsArmor,
    DeepInTheCaves,
    MinersLuck,
    AbandonedOrkCamp,
    MiningGear,
    LoggingGear,
    Fairy,
    Dog,
    Wildcat,
    Forest,
    Mountains,
    Plains,
    Swamp,
    Desert,
    GodsBlessing,
    HuntingTrip,
    LoggingContest,
    Rat,
    AncientSpellbook,
    TigerFangPowder,
    BearClawPowder,
    RhinoHornPowder,
    KnowledgeOfTheEldest,
    RevolutionarySpirit,
    BlessingOfTheGods,
    ForestArtifact,
    DesertArtifact,
    SwampArtifact,
    MountainsArtifact,
    PlainsArtifact,
    HireDwarf,
    Pegasus,
    MysticFields,
    BearHunting,
}

impl AsAtValue for Image {
    fn as_at_value(&self) -> seed::prelude::AtValue {
        match self {
            Image::Placeholder => AtValue::Some("/images/placeholder.png".to_string()),
            Image::Dwarf(id) => AtValue::Some(format!("/images/dwarf-{}.jpg", id)),
            Image::FemaleDwarf(id) => AtValue::Some(format!("/images/dwarf-female-{}.jpg", id)),
            Image::ChildDwarf(id) => AtValue::Some(format!("/images/dwarf-child-{}.jpg", id)),
            Image::ChildFemaleDwarf(id) => {
                AtValue::Some(format!("/images/dwarf-female-child-{}.jpg", id))
            }
            _ => AtValue::Some(format!("/images/{self}.jpg")),
        }
    }
}

impl Image {
    pub fn from_dwarf(dwarf: &Dwarf) -> Image {
        let mut rng = Self::rng_from_str(&dwarf.name);
        if dwarf.is_adult() {
            if dwarf.is_female {
                Image::female_dwarf_from_name(&mut rng)
            } else {
                Image::dwarf_from_name(&mut rng)
            }
        } else if dwarf.is_female {
            Image::child_female_dwarf_from_name(&mut rng)
        } else {
            Image::child_dwarf_from_name(&mut rng)
        }
    }

    pub fn from_dwarf_str(s: &str) -> Image {
        let mut rng = Self::rng_from_str(s);
        if rng.gen_bool(FEMALE_PROBABILITY) {
            Self::female_dwarf_from_name(&mut rng)
        } else {
            Self::dwarf_from_name(&mut rng)
        }
    }

    pub fn rng_from_str(name: &str) -> ChaCha8Rng {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let slice = &hasher.finalize()[..];
        assert_eq!(slice.len(), 32, "slice length wasn't {}", slice.len());
        let bytes: [u8; 32] = slice.try_into().unwrap();
        ChaCha8Rng::from_seed(bytes)
    }

    fn dwarf_from_name(rng: &mut impl Rng) -> Image {
        Image::Dwarf(rng.next_u64() % 32)
    }

    fn female_dwarf_from_name(rng: &mut impl Rng) -> Image {
        Image::FemaleDwarf(rng.next_u64() % 16)
    }

    fn child_dwarf_from_name(rng: &mut impl Rng) -> Image {
        Image::ChildDwarf(rng.next_u64() % 16)
    }

    fn child_female_dwarf_from_name(rng: &mut impl Rng) -> Image {
        Image::ChildFemaleDwarf(rng.next_u64() % 8)
    }
}

impl From<Item> for Image {
    fn from(item: Item) -> Self {
        match item {
            Item::Wood => Image::Wood,
            Item::Stone => Image::Stone,
            Item::Blueberry => Image::Blueberry,
            Item::ChainMail => Image::ChainMail,
            Item::Coal => Image::Coal,
            Item::Nail => Image::Nail,
            Item::Apple => Image::Apple,
            Item::ApplePie => Image::ApplePie,
            Item::Backpack => Image::Backpack,
            Item::BlackPowder => Image::BlackPowder,
            Item::Cat => Image::Cat,
            Item::Chain => Image::Chain,
            Item::Dagger => Image::Dagger,
            Item::Donkey => Image::Donkey,
            Item::Dragon => Image::Dragon,
            Item::FishingRod => Image::FishingRod,
            Item::Helmet => Image::Helmet,
            Item::Horse => Image::Horse,
            Item::Parrot => Image::Parrot,
            Item::Poison => Image::Poison,
            Item::RawMeat => Image::RawMeat,
            Item::Boots => Image::Boots,
            Item::Sulfur => Image::Sulfur,
            Item::Sword => Image::Sword,
            Item::Wolf => Image::Wolf,
            Item::BakedPotato => Image::BakedPotato,
            Item::Lantern => Image::Lantern,
            Item::Pickaxe => Image::Pickaxe,
            Item::Potato => Image::Potato,
            Item::BlueberryCake => Image::BlueberryCake,
            Item::Wheat => Image::Wheat,
            Item::BearClaw => Image::BearClaw,
            Item::Map => Image::Map,
            Item::Egg => Image::Egg,
            Item::Bow => Image::Bow,
            Item::Axe => Image::Axe,
            Item::Crossbow => Image::Crossbow,
            Item::RhinoHorn => Image::RhinoHorn,
            Item::BearClawGloves => Image::BearClawGloves,
            Item::Pitchfork => Image::Pitchfork,
            Item::RhinoHornHelmet => Image::RhinoHornHelmet,
            Item::Spear => Image::Spear,
            Item::CookedMeat => Image::CookedMeat,
            Item::RawFish => Image::RawFish,
            Item::DragonsEgg => Image::DragonEgg,
            Item::Milk => Image::Milk,
            Item::Soup => Image::Soup,
            Item::Plough => Image::Plough,
            Item::PoisonedBow => Image::PoisonedBow,
            Item::BearClawBoots => Image::BearClawBoots,
            Item::Dynamite => Image::Dynamite,
            Item::Bird => Image::Bird,
            Item::Bread => Image::Bread,
            Item::TigerFang => Image::TigerFang,
            Item::Longsword => Image::Longsword,
            Item::TigerFangDagger => Image::TigerFangDagger,
            Item::Flour => Image::Flour,
            Item::CookedFish => Image::CookedFish,
            Item::PoisonedSpear => Image::PoisonedSpear,
            Item::FishingNet => Image::FishingNet,
            Item::Overall => Image::Overall,
            Item::String => Image::String,
            Item::Wheel => Image::Wheel,
            Item::FishingHat => Image::FishingHat,
            Item::Hemp => Image::Hemp,
            Item::Ruby => Image::Ruby,
            Item::RingOfEndurance => Image::RingOfEndurance,
            Item::RingOfIntelligence => Image::RingOfIntelligence,
            Item::RingOfStrength => Image::RingOfStrength,
            Item::Musket => Image::Musket,
            Item::Fluorite => Image::Fluorite,
            Item::Agate => Image::Agate,
            Item::Sodalite => Image::Sodalite,
            Item::RingOfPerception => Image::RingOfPerception,
            Item::Selenite => Image::Selenite,
            Item::RingOfAgility => Image::RingOfAgility,
            Item::CrystalNecklace => Image::CrystalNecklace,
            Item::Wheelbarrow => Image::Wheelbarrow,
            Item::GoldenRing => Image::GoldenRing,
            Item::Bone => Image::Bone,
            Item::IronOre => Image::IronOre,
            Item::GoldOre => Image::GoldOre,
            Item::Iron => Image::Iron,
            Item::Gold => Image::Gold,
            Item::PufferFish => Image::Pufferfish,
            Item::LeatherArmor => Image::LeatherArmor,
            Item::Carrot => Image::Carrot,
            Item::Leather => Image::Leather,
            Item::Fabric => Image::Fabric,
            Item::Gloves => Image::Gloves,
            Item::Bag => Image::Bag,
            Item::Headlamp => Image::Headlamp,
            Item::Diamond => Image::Diamond,
            Item::DiamondAxe => Image::DiamondAxe,
            Item::DiamondPickaxe => Image::DiamondPickaxe,
            Item::DiamondSword => Image::DiamondSword,
            Item::RhinoHornPants => Image::RhinoHornPants,
            Item::DynamiteCrossbow => Image::DynamiteCrossbow,
            Item::Dolphin => Image::Dolphin,
            Item::BoneNecklace => Image::BoneNecklace,
            Item::BoneHelmet => Image::BoneHelmet,
            Item::HorseCarriage => Image::HorseCarriage,
            Item::HotAirBalloon => Image::HotAirBalloon,
            Item::Vest => Image::Vest,
            Item::Boat => Image::Boat,
            Item::Ox => Image::Ox,
            Item::Kobold => Image::Kobold,
            Item::KnightsArmor => Image::KnightsArmor,
            Item::LoggingGear => Image::LoggingGear,
            Item::MiningGear => Image::MiningGear,
            Item::Fairy => Image::Fairy,
            Item::Dog => Image::Dog,
            Item::Wildcat => Image::Wildcat,
            Item::Rat => Image::Rat,
            Item::AncientSpellbook => Image::AncientSpellbook,
            Item::TigerFangPowder => Image::TigerFangPowder,
            Item::BearClawPowder => Image::BearClawPowder,
            Item::RhinoHornPowder => Image::RhinoHornPowder,
            Item::KnowledgeOfTheEldest => Image::KnowledgeOfTheEldest,
            Item::RevolutionarySpirit => Image::RevolutionarySpirit,
            Item::BlessingOfTheGods => Image::BlessingOfTheGods,
            Item::ForestArtifact => Image::ForestArtifact,
            Item::DesertArtifact => Image::DesertArtifact,
            Item::SwampArtifact => Image::SwampArtifact,
            Item::MountainsArtifact => Image::MountainsArtifact,
            Item::PlainsArtifact => Image::PlainsArtifact,
            Item::Pegasus => Image::Pegasus,
        }
    }
}

impl From<QuestType> for Image {
    fn from(quest_type: QuestType) -> Self {
        match quest_type {
            QuestType::ADwarfInDanger => Image::ADwarfInDanger,
            QuestType::AFishingFriend => Image::AFishingFriend,
            QuestType::DrunkFishing => Image::DrunkFishing,
            QuestType::CollapsedCave => Image::CollapsedCave,
            QuestType::FreeTheVillage => Image::FreeTheVillage,
            QuestType::ForTheKing => Image::ForTheKing,
            QuestType::KillTheDragon => Image::KillTheDragon,
            QuestType::ADwarfGotLost => Image::ADwarfGotLost,
            QuestType::FeastForAGuest => Image::FeastForAGuest,
            QuestType::ArenaFight => Image::ArenaFight,
            QuestType::TheHiddenTreasure => Image::TheHiddenTreasure,
            QuestType::CatStuckOnATree => Image::CatStuckOnATree,
            QuestType::AttackTheOrks => Image::AttackTheOrks,
            QuestType::FreeTheDwarf => Image::FreeTheDwarf,
            QuestType::FarmersContest => Image::FarmersContest,
            QuestType::CrystalsForTheElves => Image::CrystalsForTheElves,
            QuestType::ADarkSecret => Image::ADarkSecret,
            QuestType::ElvenVictory => Image::ElvenVictory,
            QuestType::TheMassacre => Image::TheMassacre,
            QuestType::TheElvenWar => Image::TheElvenWar,
            QuestType::Concert => Image::Concert,
            QuestType::MagicalBerries => Image::MagicalBerries,
            QuestType::EatingContest => Image::EatingContest,
            QuestType::Socializing => Image::Socializing,
            QuestType::TheElvenMagician => Image::TheElvenMagician,
            QuestType::ExploreNewLands => Image::ExploreNewLands,
            QuestType::DeepInTheCaves => Image::DeepInTheCaves,
            QuestType::MinersLuck => Image::MinersLuck,
            QuestType::AbandonedOrkCamp => Image::AbandonedOrkCamp,
            QuestType::GodsBlessing => Image::GodsBlessing,
            QuestType::LoggingContest => Image::LoggingContest,
            QuestType::HuntingTrip => Image::HuntingTrip,
            QuestType::BearHunting => Image::BearHunting,
            QuestType::MysticFields => Image::MysticFields,
        }
    }
}

impl From<Occupation> for Image {
    fn from(occupation: Occupation) -> Self {
        match occupation {
            Occupation::Idling => Image::Idling,
            Occupation::Mining => Image::Mining,
            Occupation::Logging => Image::Logging,
            Occupation::Rockhounding => Image::Rockhounding,
            Occupation::Farming => Image::Farming,
            Occupation::Fishing => Image::Fishing,
            Occupation::Hunting => Image::Hunting,
            Occupation::Gathering => Image::Gathering,
            Occupation::Fighting => Image::Fighting,
            Occupation::Exploring => Image::Exploring,
        }
    }
}

impl From<VillageType> for Image {
    fn from(village_type: VillageType) -> Self {
        match village_type {
            VillageType::Outpost => Image::Outpost,
            VillageType::Dwelling => Image::Dwelling,
            VillageType::Hamlet => Image::Hamlet,
            VillageType::Village => Image::Village,
            VillageType::SmallTown => Image::SmallTown,
            VillageType::LargeTown => Image::LargeTown,
            VillageType::SmallCity => Image::SmallCity,
            VillageType::LargeCity => Image::LargeCity,
            VillageType::Metropolis => Image::Metropolis,
            VillageType::Megalopolis => Image::Megalopolis,
        }
    }
}

impl From<WorldEvent> for Image {
    fn from(world_event: WorldEvent) -> Self {
        match world_event {
            WorldEvent::Drought => Image::Drought,
            WorldEvent::Flood => Image::Flood,
            WorldEvent::Earthquake => Image::Earthquake,
            WorldEvent::Plague => Image::Plague,
            WorldEvent::Tornado => Image::Tornado,
            WorldEvent::Carnival => Image::Carnival,
            WorldEvent::FullMoon => Image::FullMoon,
            WorldEvent::Revolution => Image::Revolution,
        }
    }
}


impl From<Territory> for Image {
    fn from(territory: Territory) -> Self {
        match territory {
            Territory::Mountains => Image::Mountains,
            Territory::Forest => Image::Forest,
            Territory::Plains => Image::Plains,
            Territory::Swamp => Image::Swamp,
            Territory::Desert => Image::Desert,
        }
    }
}
