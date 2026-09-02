#[doc = r" Whether an effect helps, hurts, or does neither. A client draws the three differently"]
#[doc = r" and milk takes all of them away regardless."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Beneficial,
    Neutral,
    Harmful,
}
#[doc = r" How a modifier changes what an attribute is worth."]
#[doc = r""]
#[doc = r" The same three the attribute system has; named again here so this module can be read"]
#[doc = r" without one, and matched across at the one place they meet."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}
#[doc = r" One number an effect moves, and by how much for a single level."]
#[doc = r""]
#[doc = r" The amount is what one level is worth; every level after it is a multiple, which is why"]
#[doc = r" speed II is exactly twice speed I rather than a separate modifier."]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectModifier {
    pub attribute: &'static str,
    pub amount: f64,
    pub operation: Operation,
    #[doc = r" What the modifier is called, which is how it is taken away again."]
    pub name: &'static str,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, bitcode_derive :: Encode, bitcode_derive :: Decode,
)]
pub enum Effect {
    Absorption,
    BadOmen,
    Blindness,
    BreathOfTheNautilus,
    ConduitPower,
    Darkness,
    DolphinsGrace,
    FireResistance,
    Glowing,
    Haste,
    HealthBoost,
    HeroOfTheVillage,
    Hunger,
    Infested,
    InstantDamage,
    InstantHealth,
    Invisibility,
    JumpBoost,
    Levitation,
    Luck,
    MiningFatigue,
    Nausea,
    NightVision,
    Oozing,
    Poison,
    RaidOmen,
    Regeneration,
    Resistance,
    Saturation,
    SlowFalling,
    Slowness,
    Speed,
    Strength,
    TrialOmen,
    Unluck,
    WaterBreathing,
    Weakness,
    Weaving,
    WindCharged,
    Wither,
}
impl Effect {
    #[doc = r" The number it travels as, in this server's own version."]
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::Absorption => 21,
            Self::BadOmen => 30,
            Self::Blindness => 14,
            Self::BreathOfTheNautilus => 39,
            Self::ConduitPower => 28,
            Self::Darkness => 32,
            Self::DolphinsGrace => 29,
            Self::FireResistance => 11,
            Self::Glowing => 23,
            Self::Haste => 2,
            Self::HealthBoost => 20,
            Self::HeroOfTheVillage => 31,
            Self::Hunger => 16,
            Self::Infested => 38,
            Self::InstantDamage => 6,
            Self::InstantHealth => 5,
            Self::Invisibility => 13,
            Self::JumpBoost => 7,
            Self::Levitation => 24,
            Self::Luck => 25,
            Self::MiningFatigue => 3,
            Self::Nausea => 8,
            Self::NightVision => 15,
            Self::Oozing => 37,
            Self::Poison => 18,
            Self::RaidOmen => 34,
            Self::Regeneration => 9,
            Self::Resistance => 10,
            Self::Saturation => 22,
            Self::SlowFalling => 27,
            Self::Slowness => 1,
            Self::Speed => 0,
            Self::Strength => 4,
            Self::TrialOmen => 33,
            Self::Unluck => 26,
            Self::WaterBreathing => 12,
            Self::Weakness => 17,
            Self::Weaving => 36,
            Self::WindCharged => 35,
            Self::Wither => 19,
        }
    }
    #[doc = r" Whether it helps, hurts, or does neither."]
    #[must_use]
    pub const fn category(self) -> Category {
        match self {
            Self::Absorption => Category::Beneficial,
            Self::BadOmen => Category::Neutral,
            Self::Blindness => Category::Harmful,
            Self::BreathOfTheNautilus => Category::Beneficial,
            Self::ConduitPower => Category::Beneficial,
            Self::Darkness => Category::Harmful,
            Self::DolphinsGrace => Category::Beneficial,
            Self::FireResistance => Category::Beneficial,
            Self::Glowing => Category::Neutral,
            Self::Haste => Category::Beneficial,
            Self::HealthBoost => Category::Beneficial,
            Self::HeroOfTheVillage => Category::Beneficial,
            Self::Hunger => Category::Harmful,
            Self::Infested => Category::Harmful,
            Self::InstantDamage => Category::Harmful,
            Self::InstantHealth => Category::Beneficial,
            Self::Invisibility => Category::Beneficial,
            Self::JumpBoost => Category::Beneficial,
            Self::Levitation => Category::Harmful,
            Self::Luck => Category::Beneficial,
            Self::MiningFatigue => Category::Harmful,
            Self::Nausea => Category::Harmful,
            Self::NightVision => Category::Beneficial,
            Self::Oozing => Category::Harmful,
            Self::Poison => Category::Harmful,
            Self::RaidOmen => Category::Neutral,
            Self::Regeneration => Category::Beneficial,
            Self::Resistance => Category::Beneficial,
            Self::Saturation => Category::Beneficial,
            Self::SlowFalling => Category::Beneficial,
            Self::Slowness => Category::Harmful,
            Self::Speed => Category::Beneficial,
            Self::Strength => Category::Beneficial,
            Self::TrialOmen => Category::Neutral,
            Self::Unluck => Category::Harmful,
            Self::WaterBreathing => Category::Beneficial,
            Self::Weakness => Category::Harmful,
            Self::Weaving => Category::Harmful,
            Self::WindCharged => Category::Harmful,
            Self::Wither => Category::Harmful,
        }
    }
    #[doc = r" The colour a client draws it, as packed red, green and blue."]
    #[must_use]
    pub const fn colour(self) -> i32 {
        match self {
            Self::Absorption => 2445989,
            Self::BadOmen => 745784,
            Self::Blindness => 2039587,
            Self::BreathOfTheNautilus => 65518,
            Self::ConduitPower => 1950417,
            Self::Darkness => 2696993,
            Self::DolphinsGrace => 8954814,
            Self::FireResistance => 16750848,
            Self::Glowing => 9740385,
            Self::Haste => 14270531,
            Self::HealthBoost => 16284963,
            Self::HeroOfTheVillage => 4521796,
            Self::Hunger => 5797459,
            Self::Infested => 9214860,
            Self::InstantDamage => 11101546,
            Self::InstantHealth => 16262179,
            Self::Invisibility => 16185078,
            Self::JumpBoost => 16646020,
            Self::Levitation => 13565951,
            Self::Luck => 5882118,
            Self::MiningFatigue => 4866583,
            Self::Nausea => 5578058,
            Self::NightVision => 12779366,
            Self::Oozing => 10092451,
            Self::Poison => 8889187,
            Self::RaidOmen => 14565464,
            Self::Regeneration => 13458603,
            Self::Resistance => 9520880,
            Self::Saturation => 16262179,
            Self::SlowFalling => 15978425,
            Self::Slowness => 9154528,
            Self::Speed => 3402751,
            Self::Strength => 16762624,
            Self::TrialOmen => 1484454,
            Self::Unluck => 12624973,
            Self::WaterBreathing => 10017472,
            Self::Weakness => 4738376,
            Self::Weaving => 7891290,
            Self::WindCharged => 12438015,
            Self::Wither => 7561558,
        }
    }
    #[doc = r" Whether it lands all at once rather than lasting."]
    #[doc = r""]
    #[doc = r" The three that do are healing, harming and saturation: they are applied once and"]
    #[doc = r" never held."]
    #[must_use]
    pub const fn is_instant(self) -> bool {
        matches!(
            self,
            Self::InstantDamage | Self::InstantHealth | Self::Saturation
        )
    }
    #[doc = r" Which of the holder's numbers it moves."]
    #[doc = r""]
    #[doc = r" Twelve of the forty move anything at all; the rest are read by the code that cares"]
    #[doc = r" about them, or by a client."]
    #[must_use]
    pub const fn modifiers(self) -> &'static [EffectModifier] {
        match self {
            Self::Absorption => &[EffectModifier {
                attribute: "max_absorption",
                amount: 4.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.absorption",
            }],
            Self::BadOmen => &[],
            Self::Blindness => &[],
            Self::BreathOfTheNautilus => &[],
            Self::ConduitPower => &[],
            Self::Darkness => &[],
            Self::DolphinsGrace => &[],
            Self::FireResistance => &[],
            Self::Glowing => &[],
            Self::Haste => &[EffectModifier {
                attribute: "attack_speed",
                amount: 0.10000000149011612,
                operation: Operation::AddMultipliedTotal,
                name: "minecraft:effect.haste",
            }],
            Self::HealthBoost => &[EffectModifier {
                attribute: "max_health",
                amount: 4.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.health_boost",
            }],
            Self::HeroOfTheVillage => &[],
            Self::Hunger => &[],
            Self::Infested => &[],
            Self::InstantDamage => &[],
            Self::InstantHealth => &[],
            Self::Invisibility => &[EffectModifier {
                attribute: "waypoint_transmit_range",
                amount: -1.0,
                operation: Operation::AddMultipliedTotal,
                name: "minecraft:effect.waypoint_transmit_range_hide",
            }],
            Self::JumpBoost => &[EffectModifier {
                attribute: "safe_fall_distance",
                amount: 1.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.jump_boost",
            }],
            Self::Levitation => &[],
            Self::Luck => &[EffectModifier {
                attribute: "luck",
                amount: 1.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.luck",
            }],
            Self::MiningFatigue => &[EffectModifier {
                attribute: "attack_speed",
                amount: -0.10000000149011612,
                operation: Operation::AddMultipliedTotal,
                name: "minecraft:effect.mining_fatigue",
            }],
            Self::Nausea => &[],
            Self::NightVision => &[],
            Self::Oozing => &[],
            Self::Poison => &[],
            Self::RaidOmen => &[],
            Self::Regeneration => &[],
            Self::Resistance => &[],
            Self::Saturation => &[],
            Self::SlowFalling => &[],
            Self::Slowness => &[EffectModifier {
                attribute: "movement_speed",
                amount: -0.15000000596046448,
                operation: Operation::AddMultipliedTotal,
                name: "minecraft:effect.slowness",
            }],
            Self::Speed => &[EffectModifier {
                attribute: "movement_speed",
                amount: 0.20000000298023224,
                operation: Operation::AddMultipliedTotal,
                name: "minecraft:effect.speed",
            }],
            Self::Strength => &[EffectModifier {
                attribute: "attack_damage",
                amount: 3.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.strength",
            }],
            Self::TrialOmen => &[],
            Self::Unluck => &[EffectModifier {
                attribute: "luck",
                amount: -1.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.unluck",
            }],
            Self::WaterBreathing => &[],
            Self::Weakness => &[EffectModifier {
                attribute: "attack_damage",
                amount: -4.0,
                operation: Operation::AddValue,
                name: "minecraft:effect.weakness",
            }],
            Self::Weaving => &[],
            Self::WindCharged => &[],
            Self::Wither => &[],
        }
    }
    #[doc = r" Try to parse an `Effect` from a resource location string."]
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        match name {
            "absorption" => Some(Self::Absorption),
            "bad_omen" => Some(Self::BadOmen),
            "blindness" => Some(Self::Blindness),
            "breath_of_the_nautilus" => Some(Self::BreathOfTheNautilus),
            "conduit_power" => Some(Self::ConduitPower),
            "darkness" => Some(Self::Darkness),
            "dolphins_grace" => Some(Self::DolphinsGrace),
            "fire_resistance" => Some(Self::FireResistance),
            "glowing" => Some(Self::Glowing),
            "haste" => Some(Self::Haste),
            "health_boost" => Some(Self::HealthBoost),
            "hero_of_the_village" => Some(Self::HeroOfTheVillage),
            "hunger" => Some(Self::Hunger),
            "infested" => Some(Self::Infested),
            "instant_damage" => Some(Self::InstantDamage),
            "instant_health" => Some(Self::InstantHealth),
            "invisibility" => Some(Self::Invisibility),
            "jump_boost" => Some(Self::JumpBoost),
            "levitation" => Some(Self::Levitation),
            "luck" => Some(Self::Luck),
            "mining_fatigue" => Some(Self::MiningFatigue),
            "nausea" => Some(Self::Nausea),
            "night_vision" => Some(Self::NightVision),
            "oozing" => Some(Self::Oozing),
            "poison" => Some(Self::Poison),
            "raid_omen" => Some(Self::RaidOmen),
            "regeneration" => Some(Self::Regeneration),
            "resistance" => Some(Self::Resistance),
            "saturation" => Some(Self::Saturation),
            "slow_falling" => Some(Self::SlowFalling),
            "slowness" => Some(Self::Slowness),
            "speed" => Some(Self::Speed),
            "strength" => Some(Self::Strength),
            "trial_omen" => Some(Self::TrialOmen),
            "unluck" => Some(Self::Unluck),
            "water_breathing" => Some(Self::WaterBreathing),
            "weakness" => Some(Self::Weakness),
            "weaving" => Some(Self::Weaving),
            "wind_charged" => Some(Self::WindCharged),
            "wither" => Some(Self::Wither),
            _ => None,
        }
    }
    pub const fn to_name(&self) -> &'static str {
        match self {
            Self::Absorption => "absorption",
            Self::BadOmen => "bad_omen",
            Self::Blindness => "blindness",
            Self::BreathOfTheNautilus => "breath_of_the_nautilus",
            Self::ConduitPower => "conduit_power",
            Self::Darkness => "darkness",
            Self::DolphinsGrace => "dolphins_grace",
            Self::FireResistance => "fire_resistance",
            Self::Glowing => "glowing",
            Self::Haste => "haste",
            Self::HealthBoost => "health_boost",
            Self::HeroOfTheVillage => "hero_of_the_village",
            Self::Hunger => "hunger",
            Self::Infested => "infested",
            Self::InstantDamage => "instant_damage",
            Self::InstantHealth => "instant_health",
            Self::Invisibility => "invisibility",
            Self::JumpBoost => "jump_boost",
            Self::Levitation => "levitation",
            Self::Luck => "luck",
            Self::MiningFatigue => "mining_fatigue",
            Self::Nausea => "nausea",
            Self::NightVision => "night_vision",
            Self::Oozing => "oozing",
            Self::Poison => "poison",
            Self::RaidOmen => "raid_omen",
            Self::Regeneration => "regeneration",
            Self::Resistance => "resistance",
            Self::Saturation => "saturation",
            Self::SlowFalling => "slow_falling",
            Self::Slowness => "slowness",
            Self::Speed => "speed",
            Self::Strength => "strength",
            Self::TrialOmen => "trial_omen",
            Self::Unluck => "unluck",
            Self::WaterBreathing => "water_breathing",
            Self::Weakness => "weakness",
            Self::Weaving => "weaving",
            Self::WindCharged => "wind_charged",
            Self::Wither => "wither",
        }
    }
}
