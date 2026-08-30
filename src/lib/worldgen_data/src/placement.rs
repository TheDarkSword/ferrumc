//! Where a feature is put, and how often.
//!
//! A placed feature is a configured feature and a list of modifiers. Each modifier takes the
//! positions handed to it and gives back none, one, or many — so a count multiplies, a filter
//! drops, and a heightmap moves.

use crate::predicate::BlockPredicate;
use crate::value::{HeightProvider, IntProvider};
use ferrumc_datapack::Identifier;
use serde_json::Value;

/// Which surface a position is snapped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heightmap {
    MotionBlocking,
    MotionBlockingNoLeaves,
    OceanFloor,
    OceanFloorWg,
    WorldSurface,
    WorldSurfaceWg,
}

impl Heightmap {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "MOTION_BLOCKING" => Self::MotionBlocking,
            "MOTION_BLOCKING_NO_LEAVES" => Self::MotionBlockingNoLeaves,
            "OCEAN_FLOOR" => Self::OceanFloor,
            "OCEAN_FLOOR_WG" => Self::OceanFloorWg,
            "WORLD_SURFACE" => Self::WorldSurface,
            "WORLD_SURFACE_WG" => Self::WorldSurfaceWg,
            _ => return None,
        })
    }
}

/// One step of working out where a feature goes.
#[derive(Clone, Debug)]
pub enum PlacementModifier {
    /// Keep the position only where the biome there is one that asked for this feature.
    Biome,
    /// Scatter within the chunk.
    InSquare,
    /// Make this many of each position.
    Count(IntProvider),
    /// The same, once per layer of ground found in the column.
    CountOnEveryLayer(IntProvider),
    /// Keep a position only where the block there passes.
    BlockPredicateFilter(BlockPredicate),
    /// Move the position to a surface.
    Heightmap(Heightmap),
    /// Move it to a height drawn from this.
    HeightRange(HeightProvider),
    /// Nudge it about.
    RandomOffset {
        xz_spread: IntProvider,
        y_spread: IntProvider,
    },
    /// Keep it one time in this many.
    RarityFilter { chance: i32 },
    /// Keep it only where the water above is no deeper than this.
    SurfaceWaterDepthFilter { max_water_depth: i32 },
    /// Keep it only within this far of a surface.
    SurfaceRelativeThresholdFilter {
        heightmap: Heightmap,
        min: Option<i32>,
        max: Option<i32>,
    },
    /// Walk up or down until the ground looks right.
    EnvironmentScan {
        direction_of_search: Direction,
        target_condition: BlockPredicate,
        allowed_search_condition: Option<BlockPredicate>,
        max_steps: i32,
    },
    /// Make a count out of a noise field, so a thing thins out where the noise does.
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        noise_offset: f64,
    },
    /// One count above a noise level and another below it.
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    /// Exactly these places, and nowhere else.
    FixedPlacement(Vec<(i32, i32, i32)>),
}

/// Up or down, which is as far as a scan goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

impl PlacementModifier {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let int = |name: &str| object.get(name).and_then(Value::as_i64).map(|v| v as i32);
        let float = |name: &str| object.get(name).and_then(Value::as_f64);
        let count = || object.get("count").and_then(IntProvider::parse);
        let heightmap = || {
            object
                .get("heightmap")
                .and_then(Value::as_str)
                .and_then(Heightmap::parse)
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "biome" => Self::Biome,
            "in_square" => Self::InSquare,
            "count" => Self::Count(count()?),
            "count_on_every_layer" => Self::CountOnEveryLayer(count()?),
            "block_predicate_filter" => {
                Self::BlockPredicateFilter(BlockPredicate::parse(object.get("predicate")?)?)
            }
            "heightmap" => Self::Heightmap(heightmap()?),
            "height_range" => Self::HeightRange(HeightProvider::parse(object.get("height")?)?),
            "random_offset" => Self::RandomOffset {
                xz_spread: IntProvider::parse(object.get("xz_spread")?)?,
                y_spread: IntProvider::parse(object.get("y_spread")?)?,
            },
            "rarity_filter" => Self::RarityFilter {
                chance: int("chance")?,
            },
            "surface_water_depth_filter" => Self::SurfaceWaterDepthFilter {
                max_water_depth: int("max_water_depth")?,
            },
            "surface_relative_threshold_filter" => Self::SurfaceRelativeThresholdFilter {
                heightmap: heightmap()?,
                min: int("min_inclusive"),
                max: int("max_inclusive"),
            },
            "environment_scan" => Self::EnvironmentScan {
                direction_of_search: match object.get("direction_of_search")?.as_str()? {
                    "up" => Direction::Up,
                    "down" => Direction::Down,
                    _ => return None,
                },
                target_condition: BlockPredicate::parse(object.get("target_condition")?)?,
                allowed_search_condition: object
                    .get("allowed_search_condition")
                    .and_then(BlockPredicate::parse),
                max_steps: int("max_steps")?,
            },
            "noise_based_count" => Self::NoiseBasedCount {
                noise_to_count_ratio: int("noise_to_count_ratio")?,
                noise_factor: float("noise_factor")?,
                noise_offset: float("noise_offset").unwrap_or_default(),
            },
            "noise_threshold_count" => Self::NoiseThresholdCount {
                noise_level: float("noise_level")?,
                below_noise: int("below_noise")?,
                above_noise: int("above_noise")?,
            },
            "fixed_placement" => Self::FixedPlacement(
                object
                    .get("positions")?
                    .as_array()?
                    .iter()
                    .map(|at| {
                        let at = at.as_array()?;
                        Some((
                            at.first()?.as_i64()? as i32,
                            at.get(1)?.as_i64()? as i32,
                            at.get(2)?.as_i64()? as i32,
                        ))
                    })
                    .collect::<Option<_>>()?,
            ),
            _ => return None,
        })
    }
}

/// A feature and where it goes.
#[derive(Clone, Debug)]
pub struct PlacedFeature {
    /// The configured feature to place, by name, or one written out in place.
    pub feature: FeatureRef,
    pub placement: Vec<PlacementModifier>,
}

/// A feature named, or written out where it is used.
#[derive(Clone, Debug)]
pub enum FeatureRef {
    Named(Identifier),
    /// Written in place, which the data does for one-off features.
    Inline(Box<crate::feature::ConfiguredFeature>),
}

impl PlacedFeature {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let feature = object.get("feature")?;
        Some(Self {
            feature: match feature.as_str() {
                Some(name) => FeatureRef::Named(Identifier::parse(name).ok()?),
                None => {
                    FeatureRef::Inline(Box::new(crate::feature::ConfiguredFeature::parse(feature)?))
                }
            },
            placement: object
                .get("placement")?
                .as_array()?
                .iter()
                .map(PlacementModifier::parse)
                .collect::<Option<_>>()?,
        })
    }
}

/// A placed feature named, or written out where it is used.
#[derive(Clone, Debug)]
pub enum PlacedFeatureRef {
    Named(Identifier),
    Inline(Box<PlacedFeature>),
}

impl PlacedFeatureRef {
    pub fn parse(value: &Value) -> Option<Self> {
        match value.as_str() {
            Some(name) => Some(Self::Named(Identifier::parse(name).ok()?)),
            None => Some(Self::Inline(Box::new(PlacedFeature::parse(value)?))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modifier_reads_its_own_shape() {
        let count = PlacementModifier::parse(&serde_json::json!({
            "type": "minecraft:count", "count": 4
        }))
        .expect("a count");
        assert!(matches!(
            count,
            PlacementModifier::Count(IntProvider::Constant(4))
        ));

        let range = PlacementModifier::parse(&serde_json::json!({
            "type": "minecraft:height_range",
            "height": {"type": "minecraft:uniform",
                       "min_inclusive": {"above_bottom": 0},
                       "max_inclusive": {"below_top": 0}}
        }))
        .expect("a height range");
        assert!(matches!(range, PlacementModifier::HeightRange(_)));
    }

    #[test]
    fn a_heightmap_is_named_the_way_the_data_names_it() {
        assert_eq!(
            Heightmap::parse("MOTION_BLOCKING_NO_LEAVES"),
            Some(Heightmap::MotionBlockingNoLeaves)
        );
        assert_eq!(Heightmap::parse("motion_blocking"), None);
    }
}
