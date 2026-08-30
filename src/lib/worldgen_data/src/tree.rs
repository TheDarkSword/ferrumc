//! What makes a tree: how the trunk goes up, how the leaves sit on it, and what hangs off it.
//!
//! Every trunk placer shares a base height and two random spreads; every foliage placer a radius
//! and an offset. What is left is what tells one shape of tree from another.

use crate::state::BlockStateProvider;
use crate::value::IntProvider;
use ferrumc_datapack::Identifier;
use serde_json::Value;

/// How tall a trunk is, before the shape of it.
#[derive(Clone, Copy, Debug)]
pub struct TrunkHeight {
    pub base: i32,
    pub random_a: i32,
    pub random_b: i32,
}

impl TrunkHeight {
    fn parse(object: &serde_json::Map<String, Value>) -> Option<Self> {
        let int = |name: &str| object.get(name)?.as_i64().map(|v| v as i32);
        Some(Self {
            base: int("base_height")?,
            random_a: int("height_rand_a")?,
            random_b: int("height_rand_b")?,
        })
    }
}

/// The shape a trunk grows in.
#[derive(Clone, Debug)]
pub enum TrunkPlacer {
    Straight(TrunkHeight),
    Forking(TrunkHeight),
    Giant(TrunkHeight),
    MegaJungle(TrunkHeight),
    DarkOak(TrunkHeight),
    Fancy(TrunkHeight),
    Bending {
        height: TrunkHeight,
        min_height_for_leaves: i32,
        bend_length: IntProvider,
    },
    UpwardsBranching {
        height: TrunkHeight,
        extra_branch_steps: IntProvider,
        place_branch_per_log_probability: f32,
        extra_branch_length: IntProvider,
        can_grow_through: Identifier,
    },
    Cherry {
        height: TrunkHeight,
        branch_count: IntProvider,
        branch_horizontal_length: IntProvider,
        branch_start_offset_from_top: (i32, i32),
        branch_end_offset_from_top: IntProvider,
    },
}

impl TrunkPlacer {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let height = TrunkHeight::parse(object)?;
        let provider = |name: &str| object.get(name).and_then(IntProvider::parse);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "straight_trunk_placer" => Self::Straight(height),
            "forking_trunk_placer" => Self::Forking(height),
            "giant_trunk_placer" => Self::Giant(height),
            "mega_jungle_trunk_placer" => Self::MegaJungle(height),
            "dark_oak_trunk_placer" => Self::DarkOak(height),
            "fancy_trunk_placer" => Self::Fancy(height),
            "bending_trunk_placer" => Self::Bending {
                height,
                min_height_for_leaves: object
                    .get("min_height_for_leaves")
                    .and_then(Value::as_i64)
                    .map(|v| v as i32)
                    .unwrap_or(1),
                bend_length: provider("bend_length")?,
            },
            "upwards_branching_trunk_placer" => Self::UpwardsBranching {
                height,
                extra_branch_steps: provider("extra_branch_steps")?,
                place_branch_per_log_probability: object
                    .get("place_branch_per_log_probability")?
                    .as_f64()? as f32,
                extra_branch_length: provider("extra_branch_length")?,
                can_grow_through: Identifier::parse(
                    object
                        .get("can_grow_through")?
                        .as_str()?
                        .trim_start_matches('#'),
                )
                .ok()?,
            },
            "cherry_trunk_placer" => Self::Cherry {
                height,
                branch_count: provider("branch_count")?,
                branch_horizontal_length: provider("branch_horizontal_length")?,
                branch_start_offset_from_top: {
                    let offset = object.get("branch_start_offset_from_top")?;
                    (
                        offset.get("min_inclusive")?.as_i64()? as i32,
                        offset.get("max_inclusive")?.as_i64()? as i32,
                    )
                },
                branch_end_offset_from_top: provider("branch_end_offset_from_top")?,
            },
            _ => return None,
        })
    }
}

/// How wide the leaves sit and how far above the trunk they start.
#[derive(Clone, Debug)]
pub struct FoliageSize {
    pub radius: IntProvider,
    pub offset: IntProvider,
}

/// The shape the leaves take.
#[derive(Clone, Debug)]
pub enum FoliagePlacer {
    Blob {
        size: FoliageSize,
        height: i32,
    },
    Bush {
        size: FoliageSize,
        height: i32,
    },
    Fancy {
        size: FoliageSize,
        height: i32,
    },
    Jungle {
        size: FoliageSize,
        height: i32,
    },
    Pine {
        size: FoliageSize,
        height: IntProvider,
    },
    Acacia {
        size: FoliageSize,
    },
    DarkOak {
        size: FoliageSize,
    },
    Spruce {
        size: FoliageSize,
        trunk_height: IntProvider,
    },
    MegaPine {
        size: FoliageSize,
        crown_height: IntProvider,
    },
    RandomSpread {
        size: FoliageSize,
        foliage_height: IntProvider,
        leaf_placement_attempts: i32,
    },
    Cherry {
        size: FoliageSize,
        height: i32,
        wide_bottom_layer_hole_chance: f32,
        corner_hole_chance: f32,
        hanging_leaves_chance: f32,
        hanging_leaves_extension_chance: f32,
    },
}

impl FoliagePlacer {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let size = FoliageSize {
            radius: IntProvider::parse(object.get("radius")?)?,
            offset: IntProvider::parse(object.get("offset")?)?,
        };
        let int = |name: &str| object.get(name)?.as_i64().map(|v| v as i32);
        let float = |name: &str| object.get(name)?.as_f64().map(|v| v as f32);
        let provider = |name: &str| object.get(name).and_then(IntProvider::parse);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "blob_foliage_placer" => Self::Blob {
                size,
                height: int("height")?,
            },
            "bush_foliage_placer" => Self::Bush {
                size,
                height: int("height")?,
            },
            "fancy_foliage_placer" => Self::Fancy {
                size,
                height: int("height")?,
            },
            "jungle_foliage_placer" => Self::Jungle {
                size,
                height: int("height")?,
            },
            "pine_foliage_placer" => Self::Pine {
                size,
                height: provider("height")?,
            },
            "acacia_foliage_placer" => Self::Acacia { size },
            "dark_oak_foliage_placer" => Self::DarkOak { size },
            "spruce_foliage_placer" => Self::Spruce {
                size,
                trunk_height: provider("trunk_height")?,
            },
            "mega_pine_foliage_placer" => Self::MegaPine {
                size,
                crown_height: provider("crown_height")?,
            },
            "random_spread_foliage_placer" => Self::RandomSpread {
                size,
                foliage_height: provider("foliage_height")?,
                leaf_placement_attempts: int("leaf_placement_attempts")?,
            },
            "cherry_foliage_placer" => Self::Cherry {
                size,
                height: int("height")?,
                wide_bottom_layer_hole_chance: float("wide_bottom_layer_hole_chance")?,
                corner_hole_chance: float("corner_hole_chance")?,
                hanging_leaves_chance: float("hanging_leaves_chance")?,
                hanging_leaves_extension_chance: float("hanging_leaves_extension_chance")?,
            },
            _ => return None,
        })
    }
}

/// Where the roots go, for the trees that have them.
#[derive(Clone, Debug)]
pub enum RootPlacer {
    Mangrove {
        trunk_offset_y: IntProvider,
        root_provider: BlockStateProvider,
        above_root_placement: Option<AboveRoot>,
        max_root_width: i32,
        max_root_length: i32,
        random_skew_chance: f32,
        can_grow_through: Identifier,
        muddy_roots_in: Identifier,
        muddy_roots_provider: BlockStateProvider,
    },
}

/// What sits on top of a root, and how often.
#[derive(Clone, Debug)]
pub struct AboveRoot {
    pub provider: BlockStateProvider,
    pub placement_chance: f32,
}

impl RootPlacer {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let mangrove = object.get("mangrove_root_placement")?;
        let tag = |name: &str| {
            Identifier::parse(mangrove.get(name)?.as_str()?.trim_start_matches('#')).ok()
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "mangrove_root_placer" => Self::Mangrove {
                trunk_offset_y: IntProvider::parse(object.get("trunk_offset_y")?)?,
                root_provider: BlockStateProvider::parse(object.get("root_provider")?)?,
                above_root_placement: object.get("above_root_placement").and_then(|above| {
                    Some(AboveRoot {
                        provider: BlockStateProvider::parse(above.get("above_root_provider")?)?,
                        placement_chance: above.get("above_root_placement_chance")?.as_f64()?
                            as f32,
                    })
                }),
                max_root_width: mangrove.get("max_root_width")?.as_i64()? as i32,
                max_root_length: mangrove.get("max_root_length")?.as_i64()? as i32,
                random_skew_chance: mangrove.get("random_skew_chance")?.as_f64()? as f32,
                can_grow_through: tag("can_grow_through")?,
                muddy_roots_in: tag("muddy_roots_in")?,
                muddy_roots_provider: BlockStateProvider::parse(
                    mangrove.get("muddy_roots_provider")?,
                )?,
            },
            _ => return None,
        })
    }
}

/// How much room a tree needs before it will grow.
#[derive(Clone, Copy, Debug)]
pub enum FeatureSize {
    /// A trunk and a crown.
    TwoLayers {
        limit: i32,
        lower_size: i32,
        upper_size: i32,
        min_clipped_height: Option<i32>,
    },
    /// The same with a middle.
    ThreeLayers {
        limit: i32,
        upper_limit: i32,
        lower_size: i32,
        middle_size: i32,
        upper_size: i32,
        min_clipped_height: Option<i32>,
    },
}

impl FeatureSize {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        // Vanilla's defaults, which most of the data leans on rather than writing out.
        let int = |name: &str, default: i32| {
            object
                .get(name)
                .and_then(Value::as_i64)
                .map_or(default, |v| v as i32)
        };
        let clipped = object
            .get("min_clipped_height")
            .and_then(Value::as_i64)
            .map(|v| v as i32);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "two_layers_feature_size" => Self::TwoLayers {
                limit: int("limit", 1),
                lower_size: int("lower_size", 0),
                upper_size: int("upper_size", 1),
                min_clipped_height: clipped,
            },
            "three_layers_feature_size" => Self::ThreeLayers {
                limit: int("limit", 1),
                upper_limit: int("upper_limit", 1),
                lower_size: int("lower_size", 0),
                middle_size: int("middle_size", 1),
                upper_size: int("upper_size", 1),
                min_clipped_height: clipped,
            },
            _ => return None,
        })
    }
}

/// Something that hangs off a tree once it has grown.
#[derive(Clone, Debug)]
pub enum TreeDecorator {
    /// Vines down the trunk.
    TrunkVine,
    /// Vines off the leaves.
    LeaveVine {
        probability: f32,
    },
    Cocoa {
        probability: f32,
    },
    Beehive {
        probability: f32,
    },
    CreakingHeart {
        probability: f32,
    },
    /// Blocks stuck to the logs, facing the ways given.
    AttachedToLogs {
        probability: f32,
        block_provider: BlockStateProvider,
        directions: Vec<String>,
    },
    /// The same on the leaves, keeping its distance from its own kind.
    AttachedToLeaves {
        probability: f32,
        exclusion_radius_xz: i32,
        exclusion_radius_y: i32,
        required_empty_blocks: i32,
        block_provider: BlockStateProvider,
        directions: Vec<String>,
    },
    /// The ground under it turns to something else.
    AlterGround {
        provider: BlockStateProvider,
    },
    /// Things scattered on the ground around it.
    PlaceOnGround {
        tries: i32,
        radius: i32,
        height: i32,
        block_state_provider: BlockStateProvider,
    },
    PaleMoss {
        leaves_probability: f32,
        trunk_probability: f32,
        ground_probability: f32,
    },
}

impl TreeDecorator {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let float = |name: &str| object.get(name)?.as_f64().map(|v| v as f32);
        let int = |name: &str, default: i32| {
            object
                .get(name)
                .and_then(Value::as_i64)
                .map_or(default, |v| v as i32)
        };
        let provider = |name: &str| object.get(name).and_then(BlockStateProvider::parse);
        let directions = || -> Option<Vec<String>> {
            object
                .get("directions")?
                .as_array()?
                .iter()
                .map(|d| Some(d.as_str()?.to_owned()))
                .collect()
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "trunk_vine" => Self::TrunkVine,
            "leave_vine" => Self::LeaveVine {
                probability: float("probability")?,
            },
            "cocoa" => Self::Cocoa {
                probability: float("probability")?,
            },
            "beehive" => Self::Beehive {
                probability: float("probability")?,
            },
            "creaking_heart" => Self::CreakingHeart {
                probability: float("probability")?,
            },
            "attached_to_logs" => Self::AttachedToLogs {
                probability: float("probability")?,
                block_provider: provider("block_provider")?,
                directions: directions()?,
            },
            "attached_to_leaves" => Self::AttachedToLeaves {
                probability: float("probability")?,
                exclusion_radius_xz: int("exclusion_radius_xz", 0),
                exclusion_radius_y: int("exclusion_radius_y", 0),
                required_empty_blocks: int("required_empty_blocks", 1),
                block_provider: provider("block_provider")?,
                directions: directions()?,
            },
            "alter_ground" => Self::AlterGround {
                provider: provider("provider")?,
            },
            "place_on_ground" => Self::PlaceOnGround {
                tries: int("tries", 0),
                radius: int("radius", 1),
                height: int("height", 1),
                block_state_provider: provider("block_state_provider")?,
            },
            "pale_moss" => Self::PaleMoss {
                leaves_probability: float("leaves_probability")?,
                trunk_probability: float("trunk_probability")?,
                ground_probability: float("ground_probability")?,
            },
            _ => return None,
        })
    }
}

/// A whole tree.
#[derive(Clone, Debug)]
pub struct TreeConfig {
    pub trunk_provider: BlockStateProvider,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProvider,
    pub foliage_placer: FoliagePlacer,
    /// What the trunk stands in, where it differs from the trunk itself.
    pub below_trunk_provider: Option<BlockStateProvider>,
    pub root_placer: Option<RootPlacer>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<TreeDecorator>,
    pub ignore_vines: bool,
    pub force_dirt: bool,
}

impl TreeConfig {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            trunk_provider: BlockStateProvider::parse(object.get("trunk_provider")?)?,
            trunk_placer: TrunkPlacer::parse(object.get("trunk_placer")?)?,
            foliage_provider: BlockStateProvider::parse(object.get("foliage_provider")?)?,
            foliage_placer: FoliagePlacer::parse(object.get("foliage_placer")?)?,
            below_trunk_provider: object
                .get("below_trunk_provider")
                .and_then(BlockStateProvider::parse),
            root_placer: object.get("root_placer").and_then(RootPlacer::parse),
            minimum_size: FeatureSize::parse(object.get("minimum_size")?)?,
            decorators: object
                .get("decorators")?
                .as_array()?
                .iter()
                .map(TreeDecorator::parse)
                .collect::<Option<_>>()?,
            ignore_vines: object
                .get("ignore_vines")
                .and_then(Value::as_bool)
                .unwrap_or_default(),
            force_dirt: object
                .get("force_dirt")
                .and_then(Value::as_bool)
                .unwrap_or_default(),
        })
    }
}
