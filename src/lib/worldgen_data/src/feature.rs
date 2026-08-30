//! What a feature puts in the world.
//!
//! Every one of the game's feature types is read here. What each does with its config is worldgen's
//! own; this says what the config is.

use crate::placement::PlacedFeatureRef;
use crate::predicate::BlockPredicate;
use crate::state::{parse_block_state, BlockStateProvider, RuleTest};
use crate::tree::{TreeConfig, TreeDecorator};
use crate::value::{weighted, IntProvider};
use ferrumc_datapack::Identifier;
use ferrumc_world::block_state::BlockId;
use ferrumc_world::block_state_id::BlockStateId;
use serde_json::Value;

/// Blocks named outright, or a tag naming them.
#[derive(Clone, Debug)]
pub enum BlockSet {
    Direct(Vec<BlockId>),
    Tag(Identifier),
}

impl BlockSet {
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(one) => Some(match one.strip_prefix('#') {
                Some(tag) => Self::Tag(Identifier::parse(tag).ok()?),
                None => Self::Direct(vec![BlockId::from_name(one)?]),
            }),
            Value::Array(many) => Some(Self::Direct(
                many.iter()
                    .map(|name| BlockId::from_name(name.as_str()?))
                    .collect::<Option<_>>()?,
            )),
            _ => None,
        }
    }
}

/// One ore's worth: what it replaces and what it becomes.
#[derive(Clone, Debug)]
pub struct OreTarget {
    pub target: RuleTest,
    pub state: BlockStateId,
}

/// A feature, with whatever it needs to place itself.
#[derive(Clone, Debug)]
pub enum ConfiguredFeature {
    /// A vein of something, replacing what the targets match.
    Ore {
        targets: Vec<OreTarget>,
        size: i32,
        discard_chance_on_air_exposure: f32,
        /// Whether the vein is scattered rather than a blob.
        scattered: bool,
    },
    Tree(Box<TreeConfig>),
    /// One block, where the placement allows.
    SimpleBlock {
        to_place: BlockStateProvider,
        schedule_tick: bool,
    },
    /// A flat patch of something in the ground.
    Disk {
        state_provider: BlockStateProvider,
        target: BlockPredicate,
        radius: IntProvider,
        half_height: i32,
    },
    /// A heap of something on the ground.
    BlockPile {
        state_provider: BlockStateProvider,
    },
    /// A source of fluid in a wall, with room around it.
    Spring {
        /// The fluid, by name: a spring names a fluid state rather than a block one, so its
        /// properties are the fluid's and not any block's.
        fluid: Identifier,
        rock_count: i32,
        hole_count: i32,
        requires_block_below: bool,
        valid_blocks: BlockSet,
    },
    /// A patch of ground turned over and planted.
    VegetationPatch {
        surface: String,
        depth: IntProvider,
        vertical_range: i32,
        extra_bottom_block_chance: f32,
        extra_edge_column_chance: f32,
        vegetation_chance: f32,
        xz_radius: IntProvider,
        ground_state: BlockStateProvider,
        replaceable: Identifier,
        vegetation_feature: PlacedFeatureRef,
        /// Whether the patch is filled with water.
        waterlogged: bool,
    },
    /// Undergrowth, spreading outwards from where it starts.
    NetherForestVegetation {
        state_provider: BlockStateProvider,
        spread_width: i32,
        spread_height: i32,
    },
    /// Grass under the sea, some of it tall.
    Seagrass {
        probability: f32,
    },
    Bamboo {
        probability: f32,
    },
    SeaPickle {
        count: IntProvider,
    },
    /// A column of one thing, layer by layer.
    BlockColumn {
        layers: Vec<(IntProvider, BlockStateProvider)>,
        direction: String,
        allowed_placement: BlockPredicate,
        prioritize_tip: bool,
    },
    /// Blobs of one block put through another.
    NetherrackReplaceBlobs {
        target: BlockStateId,
        state: BlockStateId,
        radius: IntProvider,
    },
    /// Vines that climb or hang.
    TwistingVines {
        spread_width: i32,
        spread_height: i32,
        max_height: i32,
    },
    /// A lone log on the ground.
    FallenTree {
        trunk_provider: BlockStateProvider,
        log_length: IntProvider,
        log_decorators: Vec<TreeDecorator>,
        stump_decorators: Vec<TreeDecorator>,
    },
    /// One of the children, chosen at random by weight, or the default.
    RandomSelector {
        features: Vec<(PlacedFeatureRef, f32)>,
        default: PlacedFeatureRef,
    },
    /// One of the children, each as likely as another.
    SimpleRandomSelector {
        features: Vec<PlacedFeatureRef>,
    },
    /// One of two, on a coin toss.
    RandomBooleanSelector {
        feature_true: PlacedFeatureRef,
        feature_false: PlacedFeatureRef,
    },
    /// One of the children by weight.
    WeightedRandomSelector {
        features: Vec<(PlacedFeatureRef, i32)>,
    },
    /// All of them, in order.
    Sequence {
        features: Vec<PlacedFeatureRef>,
    },
    /// A feature the game has and whose config is not modelled yet. Read so a pack carrying one
    /// still loads, and named so a generator can say what it is skipping.
    NotModelled {
        kind: &'static str,
        config: Box<Value>,
    },
    /// A feature the game has that takes no config at all.
    NoConfig(&'static str),
}

impl ConfiguredFeature {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let bare = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let empty = Value::Object(serde_json::Map::new());
        let config = object.get("config").unwrap_or(&empty);
        let c = config.as_object()?;

        let int = |name: &str| c.get(name)?.as_i64().map(|v| v as i32);
        let float = |name: &str| c.get(name)?.as_f64().map(|v| v as f32);
        let flag = |name: &str| c.get(name).and_then(Value::as_bool).unwrap_or_default();
        let provider = |name: &str| c.get(name).and_then(BlockStateProvider::parse);
        let ints = |name: &str| c.get(name).and_then(IntProvider::parse);
        let tag =
            |name: &str| Identifier::parse(c.get(name)?.as_str()?.trim_start_matches('#')).ok();
        let features = |name: &str| -> Option<Vec<PlacedFeatureRef>> {
            c.get(name)?
                .as_array()?
                .iter()
                .map(PlacedFeatureRef::parse)
                .collect()
        };

        Some(match bare {
            "ore" | "scattered_ore" => Self::Ore {
                targets: c
                    .get("targets")?
                    .as_array()?
                    .iter()
                    .map(|target| {
                        Some(OreTarget {
                            target: RuleTest::parse(target.get("target")?)?,
                            state: parse_block_state(target.get("state")?)?,
                        })
                    })
                    .collect::<Option<_>>()?,
                size: int("size")?,
                discard_chance_on_air_exposure: float("discard_chance_on_air_exposure")
                    .unwrap_or_default(),
                scattered: bare == "scattered_ore",
            },
            "tree" => Self::Tree(Box::new(TreeConfig::parse(config)?)),
            "simple_block" => Self::SimpleBlock {
                to_place: provider("to_place")?,
                schedule_tick: flag("schedule_tick"),
            },
            "disk" => Self::Disk {
                state_provider: {
                    // The provider sits behind a rule of its own here, unlike everywhere else.
                    let inner = c.get("state_provider")?;
                    BlockStateProvider::parse(inner.get("state_provider").unwrap_or(inner))?
                },
                target: BlockPredicate::parse(c.get("target")?)?,
                radius: ints("radius")?,
                half_height: int("half_height")?,
            },
            "block_pile" => Self::BlockPile {
                state_provider: provider("state_provider")?,
            },
            "spring_feature" => Self::Spring {
                fluid: Identifier::parse(c.get("state")?.get("Name")?.as_str()?).ok()?,
                rock_count: int("rock_count").unwrap_or(4),
                hole_count: int("hole_count").unwrap_or(1),
                requires_block_below: c
                    .get("requires_block_below")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                valid_blocks: BlockSet::parse(c.get("valid_blocks")?)?,
            },
            "vegetation_patch" | "waterlogged_vegetation_patch" => Self::VegetationPatch {
                surface: c.get("surface")?.as_str()?.to_owned(),
                depth: ints("depth")?,
                vertical_range: int("vertical_range")?,
                extra_bottom_block_chance: float("extra_bottom_block_chance")?,
                extra_edge_column_chance: float("extra_edge_column_chance")?,
                vegetation_chance: float("vegetation_chance")?,
                xz_radius: ints("xz_radius")?,
                ground_state: provider("ground_state")?,
                replaceable: tag("replaceable")?,
                vegetation_feature: PlacedFeatureRef::parse(c.get("vegetation_feature")?)?,
                waterlogged: bare == "waterlogged_vegetation_patch",
            },
            "nether_forest_vegetation" => Self::NetherForestVegetation {
                state_provider: provider("state_provider")?,
                spread_width: int("spread_width")?,
                spread_height: int("spread_height")?,
            },
            "seagrass" => Self::Seagrass {
                probability: float("probability")?,
            },
            "bamboo" => Self::Bamboo {
                probability: float("probability")?,
            },
            "sea_pickle" => Self::SeaPickle {
                count: ints("count")?,
            },
            "block_column" => Self::BlockColumn {
                layers: c
                    .get("layers")?
                    .as_array()?
                    .iter()
                    .map(|layer| {
                        Some((
                            IntProvider::parse(layer.get("height")?)?,
                            BlockStateProvider::parse(layer.get("provider")?)?,
                        ))
                    })
                    .collect::<Option<_>>()?,
                direction: c.get("direction")?.as_str()?.to_owned(),
                allowed_placement: BlockPredicate::parse(c.get("allowed_placement")?)?,
                prioritize_tip: flag("prioritize_tip"),
            },
            "netherrack_replace_blobs" => Self::NetherrackReplaceBlobs {
                target: parse_block_state(c.get("target")?)?,
                state: parse_block_state(c.get("state")?)?,
                radius: ints("radius")?,
            },
            "twisting_vines" => Self::TwistingVines {
                spread_width: int("spread_width")?,
                spread_height: int("spread_height")?,
                max_height: int("max_height")?,
            },
            "fallen_tree" => Self::FallenTree {
                trunk_provider: provider("trunk_provider")?,
                log_length: ints("log_length")?,
                log_decorators: decorators(c.get("log_decorators"))?,
                stump_decorators: decorators(c.get("stump_decorators"))?,
            },
            "random_selector" => Self::RandomSelector {
                features: c
                    .get("features")?
                    .as_array()?
                    .iter()
                    .map(|entry| {
                        Some((
                            PlacedFeatureRef::parse(entry.get("feature")?)?,
                            entry.get("chance")?.as_f64()? as f32,
                        ))
                    })
                    .collect::<Option<_>>()?,
                default: PlacedFeatureRef::parse(c.get("default")?)?,
            },
            "simple_random_selector" => Self::SimpleRandomSelector {
                features: features("features")?,
            },
            "random_boolean_selector" => Self::RandomBooleanSelector {
                feature_true: PlacedFeatureRef::parse(c.get("feature_true")?)?,
                feature_false: PlacedFeatureRef::parse(c.get("feature_false")?)?,
            },
            "weighted_random_selector" => Self::WeightedRandomSelector {
                features: weighted(c.get("features")?, PlacedFeatureRef::parse)?,
            },
            "sequence" => Self::Sequence {
                features: features("features")?,
            },
            other if NO_CONFIG.contains(&other) => Self::NoConfig(known(NO_CONFIG, other)),
            other if NOT_MODELLED.contains(&other) => Self::NotModelled {
                kind: known(NOT_MODELLED, other),
                config: Box::new(config.clone()),
            },
            _ => return None,
        })
    }
}

fn decorators(value: Option<&Value>) -> Option<Vec<TreeDecorator>> {
    let Some(list) = value else {
        return Some(Vec::new());
    };
    list.as_array()?.iter().map(TreeDecorator::parse).collect()
}

fn known(list: &'static [&'static str], name: &str) -> &'static str {
    list.iter()
        .find(|known| **known == name)
        .copied()
        .unwrap_or("unknown")
}

/// Features that take nothing: what they place is fixed.
const NO_CONFIG: &[&str] = &[
    "basalt_pillar",
    "blue_ice",
    "bonus_chest",
    "chorus_plant",
    "desert_well",
    "end_island",
    "end_platform",
    "freeze_top_layer",
    "glowstone_blob",
    "kelp",
    "monster_room",
    "vines",
    "void_start_platform",
    "weeping_vines",
];

/// Features whose config is read and kept rather than taken apart, because nothing runs them yet.
/// Each is one that puts down a shape of its own — a geode, a fossil, a dripstone cluster — and
/// modelling its dozen fields before a generator asks for them would be guessing at what it wants.
const NOT_MODELLED: &[&str] = &[
    "basalt_columns",
    "block_blob",
    "coral_claw",
    "coral_mushroom",
    "coral_tree",
    "delta_feature",
    "end_gateway",
    "end_spike",
    "fill_layer",
    "fossil",
    "geode",
    "huge_brown_mushroom",
    "huge_fungus",
    "huge_red_mushroom",
    "iceberg",
    "lake",
    "large_dripstone",
    "multiface_growth",
    "no_op",
    "replace_single_block",
    "root_system",
    "sculk_patch",
    "speleothem",
    "speleothem_cluster",
    "spike",
    "template",
    "underwater_magma",
];
