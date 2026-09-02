#[doc = r" Every kind of thing an item stack can carry beyond its name and its count."]
#[doc = r""]
#[doc = r" Custom name, damage, enchantments, what it is worth in a fight, what it does when eaten:"]
#[doc = r" modern item identity is the type plus a map of these. The variants are in the order of"]
#[doc = r" this server's own registry."]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bitcode_derive :: Encode,
    bitcode_derive :: Decode,
)]
pub enum ComponentType {
    #[doc = " `minecraft:custom_data`"]
    CustomData,
    #[doc = " `minecraft:max_stack_size`"]
    MaxStackSize,
    #[doc = " `minecraft:max_damage`"]
    MaxDamage,
    #[doc = " `minecraft:damage`"]
    Damage,
    #[doc = " `minecraft:unbreakable`"]
    Unbreakable,
    #[doc = " `minecraft:use_effects`"]
    UseEffects,
    #[doc = " `minecraft:custom_name`"]
    CustomName,
    #[doc = " `minecraft:minimum_attack_charge`"]
    MinimumAttackCharge,
    #[doc = " `minecraft:damage_type`"]
    DamageType,
    #[doc = " `minecraft:item_name`"]
    ItemName,
    #[doc = " `minecraft:item_model`"]
    ItemModel,
    #[doc = " `minecraft:lore`"]
    Lore,
    #[doc = " `minecraft:rarity`"]
    Rarity,
    #[doc = " `minecraft:enchantments`"]
    Enchantments,
    #[doc = " `minecraft:can_place_on`"]
    CanPlaceOn,
    #[doc = " `minecraft:can_break`"]
    CanBreak,
    #[doc = " `minecraft:attribute_modifiers`"]
    AttributeModifiers,
    #[doc = " `minecraft:custom_model_data`"]
    CustomModelData,
    #[doc = " `minecraft:tooltip_display`"]
    TooltipDisplay,
    #[doc = " `minecraft:repair_cost`"]
    RepairCost,
    #[doc = " `minecraft:creative_slot_lock`"]
    CreativeSlotLock,
    #[doc = " `minecraft:enchantment_glint_override`"]
    EnchantmentGlintOverride,
    #[doc = " `minecraft:intangible_projectile`"]
    IntangibleProjectile,
    #[doc = " `minecraft:food`"]
    Food,
    #[doc = " `minecraft:consumable`"]
    Consumable,
    #[doc = " `minecraft:use_remainder`"]
    UseRemainder,
    #[doc = " `minecraft:use_cooldown`"]
    UseCooldown,
    #[doc = " `minecraft:damage_resistant`"]
    DamageResistant,
    #[doc = " `minecraft:tool`"]
    Tool,
    #[doc = " `minecraft:weapon`"]
    Weapon,
    #[doc = " `minecraft:attack_range`"]
    AttackRange,
    #[doc = " `minecraft:enchantable`"]
    Enchantable,
    #[doc = " `minecraft:equippable`"]
    Equippable,
    #[doc = " `minecraft:repairable`"]
    Repairable,
    #[doc = " `minecraft:glider`"]
    Glider,
    #[doc = " `minecraft:tooltip_style`"]
    TooltipStyle,
    #[doc = " `minecraft:death_protection`"]
    DeathProtection,
    #[doc = " `minecraft:blocks_attacks`"]
    BlocksAttacks,
    #[doc = " `minecraft:piercing_weapon`"]
    PiercingWeapon,
    #[doc = " `minecraft:kinetic_weapon`"]
    KineticWeapon,
    #[doc = " `minecraft:swing_animation`"]
    SwingAnimation,
    #[doc = " `minecraft:additional_trade_cost`"]
    AdditionalTradeCost,
    #[doc = " `minecraft:stored_enchantments`"]
    StoredEnchantments,
    #[doc = " `minecraft:dye`"]
    Dye,
    #[doc = " `minecraft:dyed_color`"]
    DyedColor,
    #[doc = " `minecraft:map_color`"]
    MapColor,
    #[doc = " `minecraft:map_id`"]
    MapId,
    #[doc = " `minecraft:map_decorations`"]
    MapDecorations,
    #[doc = " `minecraft:map_post_processing`"]
    MapPostProcessing,
    #[doc = " `minecraft:charged_projectiles`"]
    ChargedProjectiles,
    #[doc = " `minecraft:bundle_contents`"]
    BundleContents,
    #[doc = " `minecraft:potion_contents`"]
    PotionContents,
    #[doc = " `minecraft:potion_duration_scale`"]
    PotionDurationScale,
    #[doc = " `minecraft:suspicious_stew_effects`"]
    SuspiciousStewEffects,
    #[doc = " `minecraft:writable_book_content`"]
    WritableBookContent,
    #[doc = " `minecraft:written_book_content`"]
    WrittenBookContent,
    #[doc = " `minecraft:trim`"]
    Trim,
    #[doc = " `minecraft:debug_stick_state`"]
    DebugStickState,
    #[doc = " `minecraft:entity_data`"]
    EntityData,
    #[doc = " `minecraft:bucket_entity_data`"]
    BucketEntityData,
    #[doc = " `minecraft:block_entity_data`"]
    BlockEntityData,
    #[doc = " `minecraft:instrument`"]
    Instrument,
    #[doc = " `minecraft:provides_trim_material`"]
    ProvidesTrimMaterial,
    #[doc = " `minecraft:ominous_bottle_amplifier`"]
    OminousBottleAmplifier,
    #[doc = " `minecraft:jukebox_playable`"]
    JukeboxPlayable,
    #[doc = " `minecraft:provides_banner_patterns`"]
    ProvidesBannerPatterns,
    #[doc = " `minecraft:recipes`"]
    Recipes,
    #[doc = " `minecraft:lodestone_tracker`"]
    LodestoneTracker,
    #[doc = " `minecraft:firework_explosion`"]
    FireworkExplosion,
    #[doc = " `minecraft:fireworks`"]
    Fireworks,
    #[doc = " `minecraft:profile`"]
    Profile,
    #[doc = " `minecraft:note_block_sound`"]
    NoteBlockSound,
    #[doc = " `minecraft:banner_patterns`"]
    BannerPatterns,
    #[doc = " `minecraft:base_color`"]
    BaseColor,
    #[doc = " `minecraft:pot_decorations`"]
    PotDecorations,
    #[doc = " `minecraft:container`"]
    Container,
    #[doc = " `minecraft:block_state`"]
    BlockState,
    #[doc = " `minecraft:bees`"]
    Bees,
    #[doc = " `minecraft:sulfur_cube_content`"]
    SulfurCubeContent,
    #[doc = " `minecraft:lock`"]
    Lock,
    #[doc = " `minecraft:container_loot`"]
    ContainerLoot,
    #[doc = " `minecraft:break_sound`"]
    BreakSound,
    #[doc = " `minecraft:villager/variant`"]
    VillagerVariant,
    #[doc = " `minecraft:wolf/variant`"]
    WolfVariant,
    #[doc = " `minecraft:wolf/sound_variant`"]
    WolfSoundVariant,
    #[doc = " `minecraft:wolf/collar`"]
    WolfCollar,
    #[doc = " `minecraft:fox/variant`"]
    FoxVariant,
    #[doc = " `minecraft:salmon/size`"]
    SalmonSize,
    #[doc = " `minecraft:parrot/variant`"]
    ParrotVariant,
    #[doc = " `minecraft:tropical_fish/pattern`"]
    TropicalFishPattern,
    #[doc = " `minecraft:tropical_fish/base_color`"]
    TropicalFishBaseColor,
    #[doc = " `minecraft:tropical_fish/pattern_color`"]
    TropicalFishPatternColor,
    #[doc = " `minecraft:mooshroom/variant`"]
    MooshroomVariant,
    #[doc = " `minecraft:rabbit/variant`"]
    RabbitVariant,
    #[doc = " `minecraft:pig/variant`"]
    PigVariant,
    #[doc = " `minecraft:pig/sound_variant`"]
    PigSoundVariant,
    #[doc = " `minecraft:cow/variant`"]
    CowVariant,
    #[doc = " `minecraft:cow/sound_variant`"]
    CowSoundVariant,
    #[doc = " `minecraft:chicken/variant`"]
    ChickenVariant,
    #[doc = " `minecraft:chicken/sound_variant`"]
    ChickenSoundVariant,
    #[doc = " `minecraft:zombie_nautilus/variant`"]
    ZombieNautilusVariant,
    #[doc = " `minecraft:frog/variant`"]
    FrogVariant,
    #[doc = " `minecraft:horse/variant`"]
    HorseVariant,
    #[doc = " `minecraft:painting/variant`"]
    PaintingVariant,
    #[doc = " `minecraft:llama/variant`"]
    LlamaVariant,
    #[doc = " `minecraft:axolotl/variant`"]
    AxolotlVariant,
    #[doc = " `minecraft:cat/variant`"]
    CatVariant,
    #[doc = " `minecraft:cat/sound_variant`"]
    CatSoundVariant,
    #[doc = " `minecraft:cat/collar`"]
    CatCollar,
    #[doc = " `minecraft:sheep/color`"]
    SheepColor,
    #[doc = " `minecraft:shulker/color`"]
    ShulkerColor,
}
impl ComponentType {
    #[doc = r" Every kind there is, in the registry's own order."]
    pub const ALL: [Self; 111usize] = [
        Self::CustomData,
        Self::MaxStackSize,
        Self::MaxDamage,
        Self::Damage,
        Self::Unbreakable,
        Self::UseEffects,
        Self::CustomName,
        Self::MinimumAttackCharge,
        Self::DamageType,
        Self::ItemName,
        Self::ItemModel,
        Self::Lore,
        Self::Rarity,
        Self::Enchantments,
        Self::CanPlaceOn,
        Self::CanBreak,
        Self::AttributeModifiers,
        Self::CustomModelData,
        Self::TooltipDisplay,
        Self::RepairCost,
        Self::CreativeSlotLock,
        Self::EnchantmentGlintOverride,
        Self::IntangibleProjectile,
        Self::Food,
        Self::Consumable,
        Self::UseRemainder,
        Self::UseCooldown,
        Self::DamageResistant,
        Self::Tool,
        Self::Weapon,
        Self::AttackRange,
        Self::Enchantable,
        Self::Equippable,
        Self::Repairable,
        Self::Glider,
        Self::TooltipStyle,
        Self::DeathProtection,
        Self::BlocksAttacks,
        Self::PiercingWeapon,
        Self::KineticWeapon,
        Self::SwingAnimation,
        Self::AdditionalTradeCost,
        Self::StoredEnchantments,
        Self::Dye,
        Self::DyedColor,
        Self::MapColor,
        Self::MapId,
        Self::MapDecorations,
        Self::MapPostProcessing,
        Self::ChargedProjectiles,
        Self::BundleContents,
        Self::PotionContents,
        Self::PotionDurationScale,
        Self::SuspiciousStewEffects,
        Self::WritableBookContent,
        Self::WrittenBookContent,
        Self::Trim,
        Self::DebugStickState,
        Self::EntityData,
        Self::BucketEntityData,
        Self::BlockEntityData,
        Self::Instrument,
        Self::ProvidesTrimMaterial,
        Self::OminousBottleAmplifier,
        Self::JukeboxPlayable,
        Self::ProvidesBannerPatterns,
        Self::Recipes,
        Self::LodestoneTracker,
        Self::FireworkExplosion,
        Self::Fireworks,
        Self::Profile,
        Self::NoteBlockSound,
        Self::BannerPatterns,
        Self::BaseColor,
        Self::PotDecorations,
        Self::Container,
        Self::BlockState,
        Self::Bees,
        Self::SulfurCubeContent,
        Self::Lock,
        Self::ContainerLoot,
        Self::BreakSound,
        Self::VillagerVariant,
        Self::WolfVariant,
        Self::WolfSoundVariant,
        Self::WolfCollar,
        Self::FoxVariant,
        Self::SalmonSize,
        Self::ParrotVariant,
        Self::TropicalFishPattern,
        Self::TropicalFishBaseColor,
        Self::TropicalFishPatternColor,
        Self::MooshroomVariant,
        Self::RabbitVariant,
        Self::PigVariant,
        Self::PigSoundVariant,
        Self::CowVariant,
        Self::CowSoundVariant,
        Self::ChickenVariant,
        Self::ChickenSoundVariant,
        Self::ZombieNautilusVariant,
        Self::FrogVariant,
        Self::HorseVariant,
        Self::PaintingVariant,
        Self::LlamaVariant,
        Self::AxolotlVariant,
        Self::CatVariant,
        Self::CatSoundVariant,
        Self::CatCollar,
        Self::SheepColor,
        Self::ShulkerColor,
    ];
    #[doc = r" The number it travels as, in this server's own version."]
    #[must_use]
    pub const fn id(self) -> u16 {
        self as u16
    }
    #[doc = r" Try to read one from a resource location."]
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        match name {
            "custom_data" => Some(Self::CustomData),
            "max_stack_size" => Some(Self::MaxStackSize),
            "max_damage" => Some(Self::MaxDamage),
            "damage" => Some(Self::Damage),
            "unbreakable" => Some(Self::Unbreakable),
            "use_effects" => Some(Self::UseEffects),
            "custom_name" => Some(Self::CustomName),
            "minimum_attack_charge" => Some(Self::MinimumAttackCharge),
            "damage_type" => Some(Self::DamageType),
            "item_name" => Some(Self::ItemName),
            "item_model" => Some(Self::ItemModel),
            "lore" => Some(Self::Lore),
            "rarity" => Some(Self::Rarity),
            "enchantments" => Some(Self::Enchantments),
            "can_place_on" => Some(Self::CanPlaceOn),
            "can_break" => Some(Self::CanBreak),
            "attribute_modifiers" => Some(Self::AttributeModifiers),
            "custom_model_data" => Some(Self::CustomModelData),
            "tooltip_display" => Some(Self::TooltipDisplay),
            "repair_cost" => Some(Self::RepairCost),
            "creative_slot_lock" => Some(Self::CreativeSlotLock),
            "enchantment_glint_override" => Some(Self::EnchantmentGlintOverride),
            "intangible_projectile" => Some(Self::IntangibleProjectile),
            "food" => Some(Self::Food),
            "consumable" => Some(Self::Consumable),
            "use_remainder" => Some(Self::UseRemainder),
            "use_cooldown" => Some(Self::UseCooldown),
            "damage_resistant" => Some(Self::DamageResistant),
            "tool" => Some(Self::Tool),
            "weapon" => Some(Self::Weapon),
            "attack_range" => Some(Self::AttackRange),
            "enchantable" => Some(Self::Enchantable),
            "equippable" => Some(Self::Equippable),
            "repairable" => Some(Self::Repairable),
            "glider" => Some(Self::Glider),
            "tooltip_style" => Some(Self::TooltipStyle),
            "death_protection" => Some(Self::DeathProtection),
            "blocks_attacks" => Some(Self::BlocksAttacks),
            "piercing_weapon" => Some(Self::PiercingWeapon),
            "kinetic_weapon" => Some(Self::KineticWeapon),
            "swing_animation" => Some(Self::SwingAnimation),
            "additional_trade_cost" => Some(Self::AdditionalTradeCost),
            "stored_enchantments" => Some(Self::StoredEnchantments),
            "dye" => Some(Self::Dye),
            "dyed_color" => Some(Self::DyedColor),
            "map_color" => Some(Self::MapColor),
            "map_id" => Some(Self::MapId),
            "map_decorations" => Some(Self::MapDecorations),
            "map_post_processing" => Some(Self::MapPostProcessing),
            "charged_projectiles" => Some(Self::ChargedProjectiles),
            "bundle_contents" => Some(Self::BundleContents),
            "potion_contents" => Some(Self::PotionContents),
            "potion_duration_scale" => Some(Self::PotionDurationScale),
            "suspicious_stew_effects" => Some(Self::SuspiciousStewEffects),
            "writable_book_content" => Some(Self::WritableBookContent),
            "written_book_content" => Some(Self::WrittenBookContent),
            "trim" => Some(Self::Trim),
            "debug_stick_state" => Some(Self::DebugStickState),
            "entity_data" => Some(Self::EntityData),
            "bucket_entity_data" => Some(Self::BucketEntityData),
            "block_entity_data" => Some(Self::BlockEntityData),
            "instrument" => Some(Self::Instrument),
            "provides_trim_material" => Some(Self::ProvidesTrimMaterial),
            "ominous_bottle_amplifier" => Some(Self::OminousBottleAmplifier),
            "jukebox_playable" => Some(Self::JukeboxPlayable),
            "provides_banner_patterns" => Some(Self::ProvidesBannerPatterns),
            "recipes" => Some(Self::Recipes),
            "lodestone_tracker" => Some(Self::LodestoneTracker),
            "firework_explosion" => Some(Self::FireworkExplosion),
            "fireworks" => Some(Self::Fireworks),
            "profile" => Some(Self::Profile),
            "note_block_sound" => Some(Self::NoteBlockSound),
            "banner_patterns" => Some(Self::BannerPatterns),
            "base_color" => Some(Self::BaseColor),
            "pot_decorations" => Some(Self::PotDecorations),
            "container" => Some(Self::Container),
            "block_state" => Some(Self::BlockState),
            "bees" => Some(Self::Bees),
            "sulfur_cube_content" => Some(Self::SulfurCubeContent),
            "lock" => Some(Self::Lock),
            "container_loot" => Some(Self::ContainerLoot),
            "break_sound" => Some(Self::BreakSound),
            "villager/variant" => Some(Self::VillagerVariant),
            "wolf/variant" => Some(Self::WolfVariant),
            "wolf/sound_variant" => Some(Self::WolfSoundVariant),
            "wolf/collar" => Some(Self::WolfCollar),
            "fox/variant" => Some(Self::FoxVariant),
            "salmon/size" => Some(Self::SalmonSize),
            "parrot/variant" => Some(Self::ParrotVariant),
            "tropical_fish/pattern" => Some(Self::TropicalFishPattern),
            "tropical_fish/base_color" => Some(Self::TropicalFishBaseColor),
            "tropical_fish/pattern_color" => Some(Self::TropicalFishPatternColor),
            "mooshroom/variant" => Some(Self::MooshroomVariant),
            "rabbit/variant" => Some(Self::RabbitVariant),
            "pig/variant" => Some(Self::PigVariant),
            "pig/sound_variant" => Some(Self::PigSoundVariant),
            "cow/variant" => Some(Self::CowVariant),
            "cow/sound_variant" => Some(Self::CowSoundVariant),
            "chicken/variant" => Some(Self::ChickenVariant),
            "chicken/sound_variant" => Some(Self::ChickenSoundVariant),
            "zombie_nautilus/variant" => Some(Self::ZombieNautilusVariant),
            "frog/variant" => Some(Self::FrogVariant),
            "horse/variant" => Some(Self::HorseVariant),
            "painting/variant" => Some(Self::PaintingVariant),
            "llama/variant" => Some(Self::LlamaVariant),
            "axolotl/variant" => Some(Self::AxolotlVariant),
            "cat/variant" => Some(Self::CatVariant),
            "cat/sound_variant" => Some(Self::CatSoundVariant),
            "cat/collar" => Some(Self::CatCollar),
            "sheep/color" => Some(Self::SheepColor),
            "shulker/color" => Some(Self::ShulkerColor),
            _ => None,
        }
    }
    #[doc = r" What it is called, without the namespace."]
    #[must_use]
    pub const fn to_name(self) -> &'static str {
        match self {
            Self::CustomData => "custom_data",
            Self::MaxStackSize => "max_stack_size",
            Self::MaxDamage => "max_damage",
            Self::Damage => "damage",
            Self::Unbreakable => "unbreakable",
            Self::UseEffects => "use_effects",
            Self::CustomName => "custom_name",
            Self::MinimumAttackCharge => "minimum_attack_charge",
            Self::DamageType => "damage_type",
            Self::ItemName => "item_name",
            Self::ItemModel => "item_model",
            Self::Lore => "lore",
            Self::Rarity => "rarity",
            Self::Enchantments => "enchantments",
            Self::CanPlaceOn => "can_place_on",
            Self::CanBreak => "can_break",
            Self::AttributeModifiers => "attribute_modifiers",
            Self::CustomModelData => "custom_model_data",
            Self::TooltipDisplay => "tooltip_display",
            Self::RepairCost => "repair_cost",
            Self::CreativeSlotLock => "creative_slot_lock",
            Self::EnchantmentGlintOverride => "enchantment_glint_override",
            Self::IntangibleProjectile => "intangible_projectile",
            Self::Food => "food",
            Self::Consumable => "consumable",
            Self::UseRemainder => "use_remainder",
            Self::UseCooldown => "use_cooldown",
            Self::DamageResistant => "damage_resistant",
            Self::Tool => "tool",
            Self::Weapon => "weapon",
            Self::AttackRange => "attack_range",
            Self::Enchantable => "enchantable",
            Self::Equippable => "equippable",
            Self::Repairable => "repairable",
            Self::Glider => "glider",
            Self::TooltipStyle => "tooltip_style",
            Self::DeathProtection => "death_protection",
            Self::BlocksAttacks => "blocks_attacks",
            Self::PiercingWeapon => "piercing_weapon",
            Self::KineticWeapon => "kinetic_weapon",
            Self::SwingAnimation => "swing_animation",
            Self::AdditionalTradeCost => "additional_trade_cost",
            Self::StoredEnchantments => "stored_enchantments",
            Self::Dye => "dye",
            Self::DyedColor => "dyed_color",
            Self::MapColor => "map_color",
            Self::MapId => "map_id",
            Self::MapDecorations => "map_decorations",
            Self::MapPostProcessing => "map_post_processing",
            Self::ChargedProjectiles => "charged_projectiles",
            Self::BundleContents => "bundle_contents",
            Self::PotionContents => "potion_contents",
            Self::PotionDurationScale => "potion_duration_scale",
            Self::SuspiciousStewEffects => "suspicious_stew_effects",
            Self::WritableBookContent => "writable_book_content",
            Self::WrittenBookContent => "written_book_content",
            Self::Trim => "trim",
            Self::DebugStickState => "debug_stick_state",
            Self::EntityData => "entity_data",
            Self::BucketEntityData => "bucket_entity_data",
            Self::BlockEntityData => "block_entity_data",
            Self::Instrument => "instrument",
            Self::ProvidesTrimMaterial => "provides_trim_material",
            Self::OminousBottleAmplifier => "ominous_bottle_amplifier",
            Self::JukeboxPlayable => "jukebox_playable",
            Self::ProvidesBannerPatterns => "provides_banner_patterns",
            Self::Recipes => "recipes",
            Self::LodestoneTracker => "lodestone_tracker",
            Self::FireworkExplosion => "firework_explosion",
            Self::Fireworks => "fireworks",
            Self::Profile => "profile",
            Self::NoteBlockSound => "note_block_sound",
            Self::BannerPatterns => "banner_patterns",
            Self::BaseColor => "base_color",
            Self::PotDecorations => "pot_decorations",
            Self::Container => "container",
            Self::BlockState => "block_state",
            Self::Bees => "bees",
            Self::SulfurCubeContent => "sulfur_cube_content",
            Self::Lock => "lock",
            Self::ContainerLoot => "container_loot",
            Self::BreakSound => "break_sound",
            Self::VillagerVariant => "villager/variant",
            Self::WolfVariant => "wolf/variant",
            Self::WolfSoundVariant => "wolf/sound_variant",
            Self::WolfCollar => "wolf/collar",
            Self::FoxVariant => "fox/variant",
            Self::SalmonSize => "salmon/size",
            Self::ParrotVariant => "parrot/variant",
            Self::TropicalFishPattern => "tropical_fish/pattern",
            Self::TropicalFishBaseColor => "tropical_fish/base_color",
            Self::TropicalFishPatternColor => "tropical_fish/pattern_color",
            Self::MooshroomVariant => "mooshroom/variant",
            Self::RabbitVariant => "rabbit/variant",
            Self::PigVariant => "pig/variant",
            Self::PigSoundVariant => "pig/sound_variant",
            Self::CowVariant => "cow/variant",
            Self::CowSoundVariant => "cow/sound_variant",
            Self::ChickenVariant => "chicken/variant",
            Self::ChickenSoundVariant => "chicken/sound_variant",
            Self::ZombieNautilusVariant => "zombie_nautilus/variant",
            Self::FrogVariant => "frog/variant",
            Self::HorseVariant => "horse/variant",
            Self::PaintingVariant => "painting/variant",
            Self::LlamaVariant => "llama/variant",
            Self::AxolotlVariant => "axolotl/variant",
            Self::CatVariant => "cat/variant",
            Self::CatSoundVariant => "cat/sound_variant",
            Self::CatCollar => "cat/collar",
            Self::SheepColor => "sheep/color",
            Self::ShulkerColor => "shulker/color",
        }
    }
    #[doc = r" The number a client speaking `version` reads this as, if it knows it at all."]
    #[doc = r""]
    #[doc = r" [`None`] means the version has no such component. Sending one anyway would name"]
    #[doc = r" whatever now sits at that number, and since a component carries no length the rest"]
    #[doc = r" of the stack would be read as nonsense."]
    #[must_use]
    pub const fn wire_id(
        self,
        version: ferrumc_net_codec::version::ProtocolVersion,
    ) -> Option<u16> {
        match COMPONENT_IDS[version.index()][self as usize] {
            -1 => None,
            id => Some(id as u16),
        }
    }
    #[doc = r" Which kind a client speaking `version` means by a number."]
    #[must_use]
    pub fn from_wire_id(
        id: u16,
        version: ferrumc_net_codec::version::ProtocolVersion,
    ) -> Option<Self> {
        let theirs = &COMPONENT_IDS[version.index()];
        let at = theirs.iter().position(|known| *known == i32::from(id))?;
        Self::ALL.get(at).copied()
    }
}
#[doc = r" Where each kind sits in each supported version's registry, or -1 where the version does"]
#[doc = r" not have it. Read from each version's own report."]
const COMPONENT_IDS: [[i32; 111usize]; 10usize] = [
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, -1i32, 7i32, 8i32, 9i32,
        10i32, 11i32, 12i32, 13i32, -1i32, 16i32, 17i32, 18i32, 19i32, 20i32, -1i32, -1i32, -1i32,
        -1i32, 22i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, 23i32, -1i32, 24i32, 25i32, 26i32, 27i32, 28i32, 29i32, 30i32, 31i32, -1i32,
        32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, -1i32, 41i32, 42i32, -1i32,
        43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32, 53i32, 54i32, -1i32,
        55i32, 56i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, -1i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32,
        25i32, 26i32, -1i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, 33i32, -1i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, -1i32,
        42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, -1i32, 51i32, 52i32, -1i32,
        53i32, 54i32, 55i32, 56i32, 57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, -1i32,
        65i32, 66i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, -1i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32,
        25i32, 26i32, -1i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, 33i32, -1i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, -1i32,
        42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, -1i32, 51i32, 52i32, -1i32,
        53i32, 54i32, 55i32, 56i32, 57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, -1i32,
        65i32, 66i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
        -1i32, -1i32, -1i32, -1i32, -1i32, -1i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32,
        24i32, 25i32, 26i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, -1i32, -1i32,
        -1i32, -1i32, 34i32, -1i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, 42i32, 43i32,
        44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32, 53i32, 54i32, 55i32, 56i32,
        57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32, 66i32, 67i32, 68i32, -1i32,
        69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, 78i32, 79i32, 80i32, 81i32,
        82i32, 83i32, 84i32, -1i32, 85i32, -1i32, 86i32, -1i32, -1i32, 87i32, 88i32, 89i32, 90i32,
        91i32, 92i32, -1i32, 93i32, 94i32, 95i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32,
        24i32, 25i32, 26i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, -1i32, -1i32,
        -1i32, -1i32, 34i32, -1i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, 42i32, 43i32,
        44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32, 53i32, 54i32, 55i32, 56i32,
        57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32, 66i32, 67i32, 68i32, -1i32,
        69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, 78i32, 79i32, 80i32, 81i32,
        82i32, 83i32, 84i32, -1i32, 85i32, -1i32, 86i32, -1i32, -1i32, 87i32, 88i32, 89i32, 90i32,
        91i32, 92i32, -1i32, 93i32, 94i32, 95i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32,
        24i32, 25i32, 26i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, -1i32, -1i32,
        -1i32, -1i32, 34i32, -1i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, 42i32, 43i32,
        44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32, 53i32, 54i32, 55i32, 56i32,
        57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32, 66i32, 67i32, 68i32, -1i32,
        69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, 78i32, 79i32, 80i32, 81i32,
        82i32, 83i32, 84i32, -1i32, 85i32, -1i32, 86i32, -1i32, -1i32, 87i32, 88i32, 89i32, 90i32,
        91i32, 92i32, -1i32, 93i32, 94i32, 95i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, -1i32, 5i32, -1i32, -1i32, 6i32, 7i32, 8i32, 9i32, 10i32,
        11i32, 12i32, 13i32, 14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32,
        24i32, 25i32, 26i32, -1i32, 27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, -1i32, -1i32,
        -1i32, -1i32, 34i32, -1i32, 35i32, 36i32, 37i32, 38i32, 39i32, 40i32, 41i32, 42i32, 43i32,
        44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32, 53i32, 54i32, 55i32, 56i32,
        57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32, 66i32, 67i32, 68i32, -1i32,
        69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, 78i32, 79i32, 80i32, 81i32,
        82i32, 83i32, 84i32, -1i32, 85i32, -1i32, 86i32, -1i32, -1i32, 87i32, 88i32, 89i32, 90i32,
        91i32, 92i32, -1i32, 93i32, 94i32, 95i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, -1i32, 41i32, -1i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32,
        51i32, 52i32, 53i32, 54i32, 55i32, 56i32, 57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32,
        64i32, 65i32, 66i32, 67i32, 68i32, 69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, -1i32,
        76i32, 77i32, 78i32, 79i32, 80i32, 81i32, 82i32, 83i32, 84i32, 85i32, 86i32, 87i32, 88i32,
        89i32, 90i32, 91i32, -1i32, 92i32, -1i32, 93i32, -1i32, 94i32, 95i32, 96i32, 97i32, 98i32,
        99i32, 100i32, -1i32, 101i32, 102i32, 103i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32,
        53i32, 54i32, 55i32, 56i32, 57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32,
        66i32, 67i32, 68i32, 69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, -1i32,
        78i32, 79i32, 80i32, 81i32, 82i32, 83i32, 84i32, 85i32, 86i32, 87i32, 88i32, 89i32, 90i32,
        91i32, 92i32, 93i32, 94i32, 95i32, 96i32, 97i32, 98i32, 99i32, 100i32, 101i32, 102i32,
        103i32, 104i32, 105i32, 106i32, 107i32, 108i32, 109i32,
    ],
    [
        0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32, 7i32, 8i32, 9i32, 10i32, 11i32, 12i32, 13i32,
        14i32, 15i32, 16i32, 17i32, 18i32, 19i32, 20i32, 21i32, 22i32, 23i32, 24i32, 25i32, 26i32,
        27i32, 28i32, 29i32, 30i32, 31i32, 32i32, 33i32, 34i32, 35i32, 36i32, 37i32, 38i32, 39i32,
        40i32, 41i32, 42i32, 43i32, 44i32, 45i32, 46i32, 47i32, 48i32, 49i32, 50i32, 51i32, 52i32,
        53i32, 54i32, 55i32, 56i32, 57i32, 58i32, 59i32, 60i32, 61i32, 62i32, 63i32, 64i32, 65i32,
        66i32, 67i32, 68i32, 69i32, 70i32, 71i32, 72i32, 73i32, 74i32, 75i32, 76i32, 77i32, 78i32,
        79i32, 80i32, 81i32, 82i32, 83i32, 84i32, 85i32, 86i32, 87i32, 88i32, 89i32, 90i32, 91i32,
        92i32, 93i32, 94i32, 95i32, 96i32, 97i32, 98i32, 99i32, 100i32, 101i32, 102i32, 103i32,
        104i32, 105i32, 106i32, 107i32, 108i32, 109i32, 110i32,
    ],
];
