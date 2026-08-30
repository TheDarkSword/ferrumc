//! Structures: what is built, where, and how the world around it is treated.

use crate::holders::IdSet as BiomeSet;
use ferrumc_datapack::Identifier;
use serde_json::Value;
use std::collections::BTreeMap;

/// Which round of world generation a structure is placed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    FluidSprings,
    VegetalDecoration,
    TopLayerModification,
}

impl GenerationStep {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "raw_generation" => Self::RawGeneration,
            "lakes" => Self::Lakes,
            "local_modifications" => Self::LocalModifications,
            "underground_structures" => Self::UndergroundStructures,
            "surface_structures" => Self::SurfaceStructures,
            "strongholds" => Self::Strongholds,
            "underground_ores" => Self::UndergroundOres,
            "underground_decoration" => Self::UndergroundDecoration,
            "fluid_springs" => Self::FluidSprings,
            "vegetal_decoration" => Self::VegetalDecoration,
            "top_layer_modification" => Self::TopLayerModification,
            _ => return None,
        })
    }
}

/// How the ground is treated around what is built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainAdaptation {
    #[default]
    None,
    Beard,
    BeardThin,
    BeardBox,
    Bury,
    Encapsulate,
}

impl TerrainAdaptation {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Self::None,
            "beard_thin" => Self::BeardThin,
            "beard_box" => Self::BeardBox,
            "bury" => Self::Bury,
            "encapsulate" => Self::Encapsulate,
            "beard" => Self::Beard,
            _ => return None,
        })
    }
}

/// A structure: what it is, where it may go, and what lives in it once built.
#[derive(Clone, Debug)]
pub struct Structure {
    /// Which kind of structure it is, which decides how it is built.
    pub kind: Identifier,
    /// The biomes it may start in: a tag, or a list of names.
    pub biomes: BiomeSet,
    pub step: GenerationStep,
    pub terrain_adaptation: TerrainAdaptation,
    /// What spawns inside it, by mob group, overriding what the biome says.
    pub spawn_overrides: BTreeMap<String, Value>,
    /// Everything else the kind reads, kept as written: what a jigsaw does with a start pool is
    /// its own business, and nothing here builds one yet.
    pub config: Value,
}

impl Structure {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            kind: Identifier::parse(object.get("type")?.as_str()?).ok()?,
            biomes: BiomeSet::parse(object.get("biomes")?)?,
            step: GenerationStep::parse(object.get("step")?.as_str()?)?,
            terrain_adaptation: object
                .get("terrain_adaptation")
                .and_then(Value::as_str)
                .map_or(Some(TerrainAdaptation::None), TerrainAdaptation::parse)?,
            spawn_overrides: object
                .get("spawn_overrides")
                .and_then(Value::as_object)
                .map(|overrides| {
                    overrides
                        .iter()
                        .map(|(group, value)| (group.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            config: value.clone(),
        })
    }
}

/// How often a structure is tried, and where.
#[derive(Clone, Debug)]
pub struct StructureSet {
    /// The structures in the set, each with the weight it is chosen by.
    pub structures: Vec<(Identifier, i32)>,
    pub placement: StructurePlacement,
}

/// Where the game looks for a place to build.
#[derive(Clone, Debug)]
pub enum StructurePlacement {
    /// One try per cell of a grid, nudged within it.
    RandomSpread {
        spacing: i32,
        separation: i32,
        /// Whether the nudge is even or crowded towards the middle.
        spread_type: String,
        salt: i32,
        frequency: f32,
    },
    /// Rings around the world's centre, which is how strongholds are laid out.
    ConcentricRings {
        distance: i32,
        spread: i32,
        count: i32,
        preferred_biomes: BiomeSet,
        salt: i32,
    },
}

impl StructureSet {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let placement = object.get("placement")?;
        let int = |name: &str, default: i32| {
            placement
                .get(name)
                .and_then(Value::as_i64)
                .map_or(default, |v| v as i32)
        };
        Some(Self {
            structures: object
                .get("structures")?
                .as_array()?
                .iter()
                .map(|entry| {
                    Some((
                        Identifier::parse(entry.get("structure")?.as_str()?).ok()?,
                        entry.get("weight")?.as_i64()? as i32,
                    ))
                })
                .collect::<Option<_>>()?,
            placement: match placement.get("type")?.as_str()? {
                "minecraft:random_spread" => StructurePlacement::RandomSpread {
                    spacing: int("spacing", 1),
                    separation: int("separation", 0),
                    spread_type: placement
                        .get("spread_type")
                        .and_then(Value::as_str)
                        .unwrap_or("linear")
                        .to_owned(),
                    salt: int("salt", 0),
                    frequency: placement
                        .get("frequency")
                        .and_then(Value::as_f64)
                        .map_or(1.0, |v| v as f32),
                },
                "minecraft:concentric_rings" => StructurePlacement::ConcentricRings {
                    distance: int("distance", 1),
                    spread: int("spread", 1),
                    count: int("count", 1),
                    preferred_biomes: BiomeSet::parse(placement.get("preferred_biomes")?)?,
                    salt: int("salt", 0),
                },
                _ => return None,
            },
        })
    }
}
