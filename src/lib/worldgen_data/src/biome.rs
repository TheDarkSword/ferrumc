//! A biome: what the weather is, what lives there, and what is built in it.

use crate::holders::IdSet;
use crate::placement::PlacedFeatureRef;
use ferrumc_datapack::Identifier;
use serde_json::Value;
use std::collections::BTreeMap;

/// How many rounds of decoration a biome's features are split into. Everything in a round happens
/// before anything in the next, across the whole neighbourhood, which is what keeps a tree from
/// growing through a house.
pub const DECORATION_STEPS: usize = 11;

/// One kind of mob a biome makes, and how likely it is.
#[derive(Clone, Debug)]
pub struct Spawner {
    pub entity: Identifier,
    pub weight: i32,
    pub min_count: i32,
    pub max_count: i32,
}

/// What a biome looks like where the game has to choose a colour.
#[derive(Clone, Debug, Default)]
pub struct BiomeEffects {
    pub water_color: Option<String>,
    pub foliage_color: Option<String>,
    pub dry_foliage_color: Option<String>,
    pub grass_color: Option<String>,
    pub grass_color_modifier: Option<String>,
}

/// How the temperature is read: plainly, or falling with height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TemperatureModifier {
    #[default]
    None,
    Frozen,
}

#[derive(Clone, Debug)]
pub struct Biome {
    pub has_precipitation: bool,
    pub temperature: f32,
    pub temperature_modifier: TemperatureModifier,
    pub downfall: f32,
    /// How readily a mob of the creature group appears when the chunk is first made.
    pub creature_spawn_probability: Option<f32>,
    pub effects: BiomeEffects,
    /// The environment attributes 26.x moved the sky, fog and water colours into. Their values are
    /// typed per attribute and are read as written, since nothing here reads them yet.
    pub attributes: BTreeMap<String, Value>,
    pub carvers: IdSet,
    /// What is placed here, one list per round of decoration.
    pub features: Vec<Vec<PlacedFeatureRef>>,
    pub spawners: BTreeMap<String, Vec<Spawner>>,
    /// What it costs to have another of a mob nearby, which is what keeps the nether from filling
    /// up with piglins.
    pub spawn_costs: BTreeMap<Identifier, (f64, f64)>,
}

impl Biome {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let float = |name: &str| object.get(name)?.as_f64().map(|v| v as f32);
        let effects = object.get("effects").and_then(Value::as_object);
        let colour = |name: &str| {
            effects
                .and_then(|effects| effects.get(name))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };

        Some(Self {
            has_precipitation: object.get("has_precipitation")?.as_bool()?,
            temperature: float("temperature")?,
            temperature_modifier: match object
                .get("temperature_modifier")
                .and_then(Value::as_str)
                .unwrap_or("none")
            {
                "frozen" => TemperatureModifier::Frozen,
                "none" => TemperatureModifier::None,
                _ => return None,
            },
            downfall: float("downfall")?,
            creature_spawn_probability: float("creature_spawn_probability"),
            effects: BiomeEffects {
                water_color: colour("water_color"),
                foliage_color: colour("foliage_color"),
                dry_foliage_color: colour("dry_foliage_color"),
                grass_color: colour("grass_color"),
                grass_color_modifier: colour("grass_color_modifier"),
            },
            attributes: object
                .get("attributes")
                .and_then(Value::as_object)
                .map(|attributes| {
                    attributes
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            carvers: IdSet::parse(object.get("carvers")?)?,
            features: object
                .get("features")?
                .as_array()?
                .iter()
                .map(|step| {
                    step.as_array()?
                        .iter()
                        .map(PlacedFeatureRef::parse)
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<_>>()?,
            spawners: object
                .get("spawners")?
                .as_object()?
                .iter()
                .map(|(group, entries)| {
                    let entries = entries
                        .as_array()?
                        .iter()
                        .map(|entry| {
                            Some(Spawner {
                                entity: Identifier::parse(entry.get("type")?.as_str()?).ok()?,
                                weight: entry.get("weight")?.as_i64()? as i32,
                                min_count: entry.get("minCount")?.as_i64()? as i32,
                                max_count: entry.get("maxCount")?.as_i64()? as i32,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some((group.clone(), entries))
                })
                .collect::<Option<_>>()?,
            spawn_costs: object
                .get("spawn_costs")?
                .as_object()?
                .iter()
                .map(|(entity, cost)| {
                    Some((
                        Identifier::parse(entity).ok()?,
                        (
                            cost.get("energy_budget")?.as_f64()?,
                            cost.get("charge")?.as_f64()?,
                        ),
                    ))
                })
                .collect::<Option<_>>()?,
        })
    }
}
