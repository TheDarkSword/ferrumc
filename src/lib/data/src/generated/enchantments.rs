#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enchantment {
    pub id: u16,
    pub name: &'static str,
    pub description: &'static str,
    pub min_cost: Cost,
    pub max_cost: Cost,
    pub anvil_cost: u8,
    pub slots: &'static [EnchantmentSlot],
    pub supported_items: &'static str,
    pub weight: u8,
    pub max_level: u8,
    pub exclusive_set: Option<&'static str>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub base: f32,
    pub per_level_above_first: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnchantmentSlot {
    MAINHAND,
    OFFHAND,
    HEAD,
    CHEST,
    LEGS,
    FEET,
    ARMOR,
    ANY,
    HAND,
}
impl Enchantment {
    pub const AQUA_AFFINITY: Enchantment = Enchantment {
        id: 0,
        name: "aqua_affinity",
        description: "enchantment.minecraft.aqua_affinity",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 41.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::HEAD],
        supported_items: "#minecraft:enchantable/head_armor",
        weight: 2,
        max_level: 1,
        exclusive_set: None,
    };
    pub const BANE_OF_ARTHROPODS: Enchantment = Enchantment {
        id: 1,
        name: "bane_of_arthropods",
        description: "enchantment.minecraft.bane_of_arthropods",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/weapon",
        weight: 5,
        max_level: 5,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const BINDING_CURSE: Enchantment = Enchantment {
        id: 2,
        name: "binding_curse",
        description: "enchantment.minecraft.binding_curse",
        min_cost: Cost {
            base: 25.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/equippable",
        weight: 1,
        max_level: 1,
        exclusive_set: None,
    };
    pub const BLAST_PROTECTION: Enchantment = Enchantment {
        id: 3,
        name: "blast_protection",
        description: "enchantment.minecraft.blast_protection",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 13.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/armor",
        weight: 2,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/armor"),
    };
    pub const BREACH: Enchantment = Enchantment {
        id: 4,
        name: "breach",
        description: "enchantment.minecraft.breach",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mace",
        weight: 2,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const CHANNELING: Enchantment = Enchantment {
        id: 5,
        name: "channeling",
        description: "enchantment.minecraft.channeling",
        min_cost: Cost {
            base: 25.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/trident",
        weight: 1,
        max_level: 1,
        exclusive_set: None,
    };
    pub const DENSITY: Enchantment = Enchantment {
        id: 6,
        name: "density",
        description: "enchantment.minecraft.density",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mace",
        weight: 5,
        max_level: 5,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const DEPTH_STRIDER: Enchantment = Enchantment {
        id: 7,
        name: "depth_strider",
        description: "enchantment.minecraft.depth_strider",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::FEET],
        supported_items: "#minecraft:enchantable/foot_armor",
        weight: 2,
        max_level: 3,
        exclusive_set: Some("#minecraft:exclusive_set/boots"),
    };
    pub const EFFICIENCY: Enchantment = Enchantment {
        id: 8,
        name: "efficiency",
        description: "enchantment.minecraft.efficiency",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 51.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 1,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mining",
        weight: 10,
        max_level: 5,
        exclusive_set: None,
    };
    pub const FEATHER_FALLING: Enchantment = Enchantment {
        id: 9,
        name: "feather_falling",
        description: "enchantment.minecraft.feather_falling",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 6.0,
        },
        max_cost: Cost {
            base: 11.0,
            per_level_above_first: 6.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/foot_armor",
        weight: 5,
        max_level: 4,
        exclusive_set: None,
    };
    pub const FIRE_ASPECT: Enchantment = Enchantment {
        id: 10,
        name: "fire_aspect",
        description: "enchantment.minecraft.fire_aspect",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 20.0,
        },
        max_cost: Cost {
            base: 60.0,
            per_level_above_first: 20.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/fire_aspect",
        weight: 2,
        max_level: 2,
        exclusive_set: None,
    };
    pub const FIRE_PROTECTION: Enchantment = Enchantment {
        id: 11,
        name: "fire_protection",
        description: "enchantment.minecraft.fire_protection",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 18.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/armor",
        weight: 5,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/armor"),
    };
    pub const FLAME: Enchantment = Enchantment {
        id: 12,
        name: "flame",
        description: "enchantment.minecraft.flame",
        min_cost: Cost {
            base: 20.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/bow",
        weight: 2,
        max_level: 1,
        exclusive_set: None,
    };
    pub const FORTUNE: Enchantment = Enchantment {
        id: 13,
        name: "fortune",
        description: "enchantment.minecraft.fortune",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mining_loot",
        weight: 2,
        max_level: 3,
        exclusive_set: Some("#minecraft:exclusive_set/mining"),
    };
    pub const FROST_WALKER: Enchantment = Enchantment {
        id: 14,
        name: "frost_walker",
        description: "enchantment.minecraft.frost_walker",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::FEET],
        supported_items: "#minecraft:enchantable/foot_armor",
        weight: 2,
        max_level: 2,
        exclusive_set: Some("#minecraft:exclusive_set/boots"),
    };
    pub const IMPALING: Enchantment = Enchantment {
        id: 15,
        name: "impaling",
        description: "enchantment.minecraft.impaling",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 21.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/trident",
        weight: 2,
        max_level: 5,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const INFINITY: Enchantment = Enchantment {
        id: 16,
        name: "infinity",
        description: "enchantment.minecraft.infinity",
        min_cost: Cost {
            base: 20.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/bow",
        weight: 1,
        max_level: 1,
        exclusive_set: Some("#minecraft:exclusive_set/bow"),
    };
    pub const KNOCKBACK: Enchantment = Enchantment {
        id: 17,
        name: "knockback",
        description: "enchantment.minecraft.knockback",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 20.0,
        },
        max_cost: Cost {
            base: 55.0,
            per_level_above_first: 20.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/melee_weapon",
        weight: 5,
        max_level: 2,
        exclusive_set: None,
    };
    pub const LOOTING: Enchantment = Enchantment {
        id: 18,
        name: "looting",
        description: "enchantment.minecraft.looting",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/melee_weapon",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    pub const LOYALTY: Enchantment = Enchantment {
        id: 19,
        name: "loyalty",
        description: "enchantment.minecraft.loyalty",
        min_cost: Cost {
            base: 12.0,
            per_level_above_first: 7.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/trident",
        weight: 5,
        max_level: 3,
        exclusive_set: None,
    };
    pub const LUCK_OF_THE_SEA: Enchantment = Enchantment {
        id: 20,
        name: "luck_of_the_sea",
        description: "enchantment.minecraft.luck_of_the_sea",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/fishing",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    pub const LUNGE: Enchantment = Enchantment {
        id: 21,
        name: "lunge",
        description: "enchantment.minecraft.lunge",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::HAND],
        supported_items: "#minecraft:enchantable/lunge",
        weight: 5,
        max_level: 3,
        exclusive_set: None,
    };
    pub const LURE: Enchantment = Enchantment {
        id: 22,
        name: "lure",
        description: "enchantment.minecraft.lure",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/fishing",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    pub const MENDING: Enchantment = Enchantment {
        id: 23,
        name: "mending",
        description: "enchantment.minecraft.mending",
        min_cost: Cost {
            base: 25.0,
            per_level_above_first: 25.0,
        },
        max_cost: Cost {
            base: 75.0,
            per_level_above_first: 25.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::ANY],
        supported_items: "#minecraft:enchantable/durability",
        weight: 2,
        max_level: 1,
        exclusive_set: None,
    };
    pub const MULTISHOT: Enchantment = Enchantment {
        id: 24,
        name: "multishot",
        description: "enchantment.minecraft.multishot",
        min_cost: Cost {
            base: 20.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/crossbow",
        weight: 2,
        max_level: 1,
        exclusive_set: Some("#minecraft:exclusive_set/crossbow"),
    };
    pub const PIERCING: Enchantment = Enchantment {
        id: 25,
        name: "piercing",
        description: "enchantment.minecraft.piercing",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 1,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/crossbow",
        weight: 10,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/crossbow"),
    };
    pub const POWER: Enchantment = Enchantment {
        id: 26,
        name: "power",
        description: "enchantment.minecraft.power",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 16.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 1,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/bow",
        weight: 10,
        max_level: 5,
        exclusive_set: None,
    };
    pub const PROJECTILE_PROTECTION: Enchantment = Enchantment {
        id: 27,
        name: "projectile_protection",
        description: "enchantment.minecraft.projectile_protection",
        min_cost: Cost {
            base: 3.0,
            per_level_above_first: 6.0,
        },
        max_cost: Cost {
            base: 9.0,
            per_level_above_first: 6.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/armor",
        weight: 5,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/armor"),
    };
    pub const PROTECTION: Enchantment = Enchantment {
        id: 28,
        name: "protection",
        description: "enchantment.minecraft.protection",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 11.0,
        },
        max_cost: Cost {
            base: 12.0,
            per_level_above_first: 11.0,
        },
        anvil_cost: 1,
        slots: &[EnchantmentSlot::ARMOR],
        supported_items: "#minecraft:enchantable/armor",
        weight: 10,
        max_level: 4,
        exclusive_set: Some("#minecraft:exclusive_set/armor"),
    };
    pub const PUNCH: Enchantment = Enchantment {
        id: 29,
        name: "punch",
        description: "enchantment.minecraft.punch",
        min_cost: Cost {
            base: 12.0,
            per_level_above_first: 20.0,
        },
        max_cost: Cost {
            base: 37.0,
            per_level_above_first: 20.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/bow",
        weight: 2,
        max_level: 2,
        exclusive_set: None,
    };
    pub const QUICK_CHARGE: Enchantment = Enchantment {
        id: 30,
        name: "quick_charge",
        description: "enchantment.minecraft.quick_charge",
        min_cost: Cost {
            base: 12.0,
            per_level_above_first: 20.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND, EnchantmentSlot::OFFHAND],
        supported_items: "#minecraft:enchantable/crossbow",
        weight: 5,
        max_level: 3,
        exclusive_set: None,
    };
    pub const RESPIRATION: Enchantment = Enchantment {
        id: 31,
        name: "respiration",
        description: "enchantment.minecraft.respiration",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 40.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::HEAD],
        supported_items: "#minecraft:enchantable/head_armor",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    pub const RIPTIDE: Enchantment = Enchantment {
        id: 32,
        name: "riptide",
        description: "enchantment.minecraft.riptide",
        min_cost: Cost {
            base: 17.0,
            per_level_above_first: 7.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::HAND],
        supported_items: "#minecraft:enchantable/trident",
        weight: 2,
        max_level: 3,
        exclusive_set: Some("#minecraft:exclusive_set/riptide"),
    };
    pub const SHARPNESS: Enchantment = Enchantment {
        id: 33,
        name: "sharpness",
        description: "enchantment.minecraft.sharpness",
        min_cost: Cost {
            base: 1.0,
            per_level_above_first: 11.0,
        },
        max_cost: Cost {
            base: 21.0,
            per_level_above_first: 11.0,
        },
        anvil_cost: 1,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/sharp_weapon",
        weight: 10,
        max_level: 5,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const SILK_TOUCH: Enchantment = Enchantment {
        id: 34,
        name: "silk_touch",
        description: "enchantment.minecraft.silk_touch",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mining_loot",
        weight: 1,
        max_level: 1,
        exclusive_set: Some("#minecraft:exclusive_set/mining"),
    };
    pub const SMITE: Enchantment = Enchantment {
        id: 35,
        name: "smite",
        description: "enchantment.minecraft.smite",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/weapon",
        weight: 5,
        max_level: 5,
        exclusive_set: Some("#minecraft:exclusive_set/damage"),
    };
    pub const SOUL_SPEED: Enchantment = Enchantment {
        id: 36,
        name: "soul_speed",
        description: "enchantment.minecraft.soul_speed",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 10.0,
        },
        max_cost: Cost {
            base: 25.0,
            per_level_above_first: 10.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::FEET],
        supported_items: "#minecraft:enchantable/foot_armor",
        weight: 1,
        max_level: 3,
        exclusive_set: None,
    };
    pub const SWEEPING_EDGE: Enchantment = Enchantment {
        id: 37,
        name: "sweeping_edge",
        description: "enchantment.minecraft.sweeping_edge",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 20.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/sweeping",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    pub const SWIFT_SNEAK: Enchantment = Enchantment {
        id: 38,
        name: "swift_sneak",
        description: "enchantment.minecraft.swift_sneak",
        min_cost: Cost {
            base: 25.0,
            per_level_above_first: 25.0,
        },
        max_cost: Cost {
            base: 75.0,
            per_level_above_first: 25.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::LEGS],
        supported_items: "#minecraft:enchantable/leg_armor",
        weight: 1,
        max_level: 3,
        exclusive_set: None,
    };
    pub const THORNS: Enchantment = Enchantment {
        id: 39,
        name: "thorns",
        description: "enchantment.minecraft.thorns",
        min_cost: Cost {
            base: 10.0,
            per_level_above_first: 20.0,
        },
        max_cost: Cost {
            base: 60.0,
            per_level_above_first: 20.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::ANY],
        supported_items: "#minecraft:enchantable/armor",
        weight: 1,
        max_level: 3,
        exclusive_set: None,
    };
    pub const UNBREAKING: Enchantment = Enchantment {
        id: 40,
        name: "unbreaking",
        description: "enchantment.minecraft.unbreaking",
        min_cost: Cost {
            base: 5.0,
            per_level_above_first: 8.0,
        },
        max_cost: Cost {
            base: 55.0,
            per_level_above_first: 8.0,
        },
        anvil_cost: 2,
        slots: &[EnchantmentSlot::ANY],
        supported_items: "#minecraft:enchantable/durability",
        weight: 5,
        max_level: 3,
        exclusive_set: None,
    };
    pub const VANISHING_CURSE: Enchantment = Enchantment {
        id: 41,
        name: "vanishing_curse",
        description: "enchantment.minecraft.vanishing_curse",
        min_cost: Cost {
            base: 25.0,
            per_level_above_first: 0.0,
        },
        max_cost: Cost {
            base: 50.0,
            per_level_above_first: 0.0,
        },
        anvil_cost: 8,
        slots: &[EnchantmentSlot::ANY],
        supported_items: "#minecraft:enchantable/vanishing",
        weight: 1,
        max_level: 1,
        exclusive_set: None,
    };
    pub const WIND_BURST: Enchantment = Enchantment {
        id: 42,
        name: "wind_burst",
        description: "enchantment.minecraft.wind_burst",
        min_cost: Cost {
            base: 15.0,
            per_level_above_first: 9.0,
        },
        max_cost: Cost {
            base: 65.0,
            per_level_above_first: 9.0,
        },
        anvil_cost: 4,
        slots: &[EnchantmentSlot::MAINHAND],
        supported_items: "#minecraft:enchantable/mace",
        weight: 2,
        max_level: 3,
        exclusive_set: None,
    };
    #[doc = r" Try to parse an `Enchantment` from a resource location string."]
    pub fn from_name(name: &str) -> Option<&'static Self> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        match name {
            "aqua_affinity" => Some(&Self::AQUA_AFFINITY),
            "bane_of_arthropods" => Some(&Self::BANE_OF_ARTHROPODS),
            "binding_curse" => Some(&Self::BINDING_CURSE),
            "blast_protection" => Some(&Self::BLAST_PROTECTION),
            "breach" => Some(&Self::BREACH),
            "channeling" => Some(&Self::CHANNELING),
            "density" => Some(&Self::DENSITY),
            "depth_strider" => Some(&Self::DEPTH_STRIDER),
            "efficiency" => Some(&Self::EFFICIENCY),
            "feather_falling" => Some(&Self::FEATHER_FALLING),
            "fire_aspect" => Some(&Self::FIRE_ASPECT),
            "fire_protection" => Some(&Self::FIRE_PROTECTION),
            "flame" => Some(&Self::FLAME),
            "fortune" => Some(&Self::FORTUNE),
            "frost_walker" => Some(&Self::FROST_WALKER),
            "impaling" => Some(&Self::IMPALING),
            "infinity" => Some(&Self::INFINITY),
            "knockback" => Some(&Self::KNOCKBACK),
            "looting" => Some(&Self::LOOTING),
            "loyalty" => Some(&Self::LOYALTY),
            "luck_of_the_sea" => Some(&Self::LUCK_OF_THE_SEA),
            "lunge" => Some(&Self::LUNGE),
            "lure" => Some(&Self::LURE),
            "mending" => Some(&Self::MENDING),
            "multishot" => Some(&Self::MULTISHOT),
            "piercing" => Some(&Self::PIERCING),
            "power" => Some(&Self::POWER),
            "projectile_protection" => Some(&Self::PROJECTILE_PROTECTION),
            "protection" => Some(&Self::PROTECTION),
            "punch" => Some(&Self::PUNCH),
            "quick_charge" => Some(&Self::QUICK_CHARGE),
            "respiration" => Some(&Self::RESPIRATION),
            "riptide" => Some(&Self::RIPTIDE),
            "sharpness" => Some(&Self::SHARPNESS),
            "silk_touch" => Some(&Self::SILK_TOUCH),
            "smite" => Some(&Self::SMITE),
            "soul_speed" => Some(&Self::SOUL_SPEED),
            "sweeping_edge" => Some(&Self::SWEEPING_EDGE),
            "swift_sneak" => Some(&Self::SWIFT_SNEAK),
            "thorns" => Some(&Self::THORNS),
            "unbreaking" => Some(&Self::UNBREAKING),
            "vanishing_curse" => Some(&Self::VANISHING_CURSE),
            "wind_burst" => Some(&Self::WIND_BURST),
            _ => None,
        }
    }
    #[doc = r" Try to get an `Enchantment` from its ID."]
    pub const fn from_id(id: u16) -> Option<&'static Self> {
        match id {
            0 => Some(&Self::AQUA_AFFINITY),
            1 => Some(&Self::BANE_OF_ARTHROPODS),
            2 => Some(&Self::BINDING_CURSE),
            3 => Some(&Self::BLAST_PROTECTION),
            4 => Some(&Self::BREACH),
            5 => Some(&Self::CHANNELING),
            6 => Some(&Self::DENSITY),
            7 => Some(&Self::DEPTH_STRIDER),
            8 => Some(&Self::EFFICIENCY),
            9 => Some(&Self::FEATHER_FALLING),
            10 => Some(&Self::FIRE_ASPECT),
            11 => Some(&Self::FIRE_PROTECTION),
            12 => Some(&Self::FLAME),
            13 => Some(&Self::FORTUNE),
            14 => Some(&Self::FROST_WALKER),
            15 => Some(&Self::IMPALING),
            16 => Some(&Self::INFINITY),
            17 => Some(&Self::KNOCKBACK),
            18 => Some(&Self::LOOTING),
            19 => Some(&Self::LOYALTY),
            20 => Some(&Self::LUCK_OF_THE_SEA),
            21 => Some(&Self::LUNGE),
            22 => Some(&Self::LURE),
            23 => Some(&Self::MENDING),
            24 => Some(&Self::MULTISHOT),
            25 => Some(&Self::PIERCING),
            26 => Some(&Self::POWER),
            27 => Some(&Self::PROJECTILE_PROTECTION),
            28 => Some(&Self::PROTECTION),
            29 => Some(&Self::PUNCH),
            30 => Some(&Self::QUICK_CHARGE),
            31 => Some(&Self::RESPIRATION),
            32 => Some(&Self::RIPTIDE),
            33 => Some(&Self::SHARPNESS),
            34 => Some(&Self::SILK_TOUCH),
            35 => Some(&Self::SMITE),
            36 => Some(&Self::SOUL_SPEED),
            37 => Some(&Self::SWEEPING_EDGE),
            38 => Some(&Self::SWIFT_SNEAK),
            39 => Some(&Self::THORNS),
            40 => Some(&Self::UNBREAKING),
            41 => Some(&Self::VANISHING_CURSE),
            42 => Some(&Self::WIND_BURST),
            _ => None,
        }
    }
    #[doc = r" Calculate the minimum cost for this enchantment at the given level."]
    pub const fn min_cost(&self, level: u8) -> f32 {
        self.min_cost.base + self.min_cost.per_level_above_first * (level - 1) as f32
    }
    #[doc = r" Calculate the maximum cost for this enchantment at the given level."]
    pub const fn max_cost(&self, level: u8) -> f32 {
        self.max_cost.base + self.max_cost.per_level_above_first * (level - 1) as f32
    }
    #[doc = r" The number a client speaking `version` reads this as, if it knows it at all."]
    #[doc = r""]
    #[doc = r" [`None`] means the enchantment was added after that version."]
    #[must_use]
    pub const fn wire_id(
        &self,
        version: ferrumc_net_codec::version::ProtocolVersion,
    ) -> Option<u16> {
        match ENCHANTMENT_IDS[version.index()][self.id as usize] {
            -1 => None,
            id => Some(id as u16),
        }
    }
    #[doc = r" Which enchantment a client speaking `version` means by a number."]
    #[must_use]
    pub fn from_wire_id(
        id: u16,
        version: ferrumc_net_codec::version::ProtocolVersion,
    ) -> Option<&'static Self> {
        let theirs = &ENCHANTMENT_IDS[version.index()];
        let at = theirs.iter().position(|known| *known == i32::from(id))?;
        Self::from_id(u16::try_from(at).ok()?)
    }
}
#[doc = r" Where each enchantment sits in each supported version's registry, or -1 where the"]
#[doc = r" version does not have it."]
const ENCHANTMENT_IDS: [[i32; 43usize]; 10usize] = [
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, 21i32, 22i32, 23i32, 24i32, 25i32,
        26i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32,
        39i32, 40i32, 41i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32,
    ],
];
