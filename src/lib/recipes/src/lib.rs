//! Recipes: what the game can be made into.
//!
//! Every recipe is a file saying what goes in and what comes out. The shapes differ — a grid, a
//! bag of ingredients, a furnace, a stonecutter, a smithing table — but all of them are read here
//! and matched against what a player has put down.
//!
//! The handful vanilla writes as code rather than data (a firework, a repaired tool, a copied
//! book) stay code there and are read here as the special cases they are.

pub mod crafting;
pub mod ingredient;

use crafting::{matches_shapeless, CraftingInput, ShapedPattern};
use ferrumc_datapack::manager::FileToId;
use ferrumc_datapack::tag::TagRegistry;
use ferrumc_datapack::{Identifier, ResourceManager};
use ingredient::{Ingredient, ResultStack};
use serde_json::Value;
use std::collections::BTreeMap;
use tracing::error;

pub use crafting::CraftingInput as Grid;

/// Where a pack keeps its recipes.
pub const DIRECTORY: &str = "recipe";

/// Why a recipe could not be read.
#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
    #[error("recipe is not an object")]
    NotAnObject,
    #[error("recipe has no type")]
    NoType,
    #[error("unknown recipe type '{0}'")]
    UnknownType(String),
    #[error("recipe '{kind}' is missing or malformed: {field}")]
    BadField { kind: String, field: String },
}

/// Which appliance cooks a thing, which decides how long it takes by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookingKind {
    Furnace,
    BlastFurnace,
    Smoker,
    Campfire,
}

impl CookingKind {
    /// How long it takes when the recipe does not say.
    #[must_use]
    pub fn default_time(self) -> i32 {
        match self {
            Self::Furnace => 200,
            Self::BlastFurnace | Self::Smoker | Self::Campfire => 100,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Recipe {
    /// A shape laid out on the grid.
    Shaped {
        pattern: ShapedPattern,
        result: ResultStack,
    },
    /// A bag of ingredients in any arrangement.
    Shapeless {
        ingredients: Vec<Ingredient>,
        result: ResultStack,
    },
    /// One item put in a furnace, a blast furnace, a smoker or on a campfire.
    Cooking {
        kind: CookingKind,
        ingredient: Ingredient,
        result: ResultStack,
        experience: f32,
        cooking_time: i32,
    },
    /// One item cut on a stonecutter.
    Stonecutting {
        ingredient: Ingredient,
        result: ResultStack,
    },
    /// One item and some material, giving the item back changed — dyeing a shulker box.
    Transmute {
        input: Ingredient,
        material: Ingredient,
        result: ResultStack,
    },
    /// A smithing table turning one item into another.
    SmithingTransform {
        template: Option<Ingredient>,
        base: Ingredient,
        addition: Option<Ingredient>,
        result: ResultStack,
    },
    /// A smithing table putting a trim on armour, which is a component it cannot carry yet.
    SmithingTrim {
        template: Ingredient,
        base: Ingredient,
        addition: Ingredient,
    },
    /// One vanilla writes as code, every one of which builds its result out of components.
    Special(&'static str),
}

impl Recipe {
    pub fn parse(value: &Value) -> Result<Self, RecipeError> {
        let object = value.as_object().ok_or(RecipeError::NotAnObject)?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(RecipeError::NoType)?;
        let bare = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let bad = |field: &str| RecipeError::BadField {
            kind: kind.to_owned(),
            field: field.to_owned(),
        };
        let ingredient = |name: &str| object.get(name).and_then(Ingredient::parse);
        let result = || object.get("result").and_then(ResultStack::parse);

        Ok(match bare {
            "crafting_shaped" => Self::Shaped {
                pattern: ShapedPattern::parse(
                    object.get("pattern").ok_or_else(|| bad("pattern"))?,
                    object.get("key").ok_or_else(|| bad("key"))?,
                )
                .ok_or_else(|| bad("pattern"))?,
                result: result().ok_or_else(|| bad("result"))?,
            },
            "crafting_shapeless" => Self::Shapeless {
                ingredients: object
                    .get("ingredients")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("ingredients"))?
                    .iter()
                    .map(Ingredient::parse)
                    .collect::<Option<_>>()
                    .ok_or_else(|| bad("ingredients"))?,
                result: result().ok_or_else(|| bad("result"))?,
            },
            "smelting" | "blasting" | "smoking" | "campfire_cooking" => {
                let cooking = match bare {
                    "smelting" => CookingKind::Furnace,
                    "blasting" => CookingKind::BlastFurnace,
                    "smoking" => CookingKind::Smoker,
                    _ => CookingKind::Campfire,
                };
                Self::Cooking {
                    kind: cooking,
                    ingredient: ingredient("ingredient").ok_or_else(|| bad("ingredient"))?,
                    result: result().ok_or_else(|| bad("result"))?,
                    experience: object
                        .get("experience")
                        .and_then(Value::as_f64)
                        .unwrap_or_default() as f32,
                    cooking_time: object
                        .get("cookingtime")
                        .and_then(Value::as_i64)
                        .and_then(|t| i32::try_from(t).ok())
                        .unwrap_or_else(|| cooking.default_time()),
                }
            }
            "stonecutting" => Self::Stonecutting {
                ingredient: ingredient("ingredient").ok_or_else(|| bad("ingredient"))?,
                result: result().ok_or_else(|| bad("result"))?,
            },
            "crafting_transmute" => Self::Transmute {
                input: ingredient("input").ok_or_else(|| bad("input"))?,
                material: ingredient("material").ok_or_else(|| bad("material"))?,
                result: result().ok_or_else(|| bad("result"))?,
            },
            "smithing_transform" => Self::SmithingTransform {
                template: ingredient("template"),
                base: ingredient("base").ok_or_else(|| bad("base"))?,
                addition: ingredient("addition"),
                result: result().ok_or_else(|| bad("result"))?,
            },
            "smithing_trim" => Self::SmithingTrim {
                template: ingredient("template").ok_or_else(|| bad("template"))?,
                base: ingredient("base").ok_or_else(|| bad("base"))?,
                addition: ingredient("addition").ok_or_else(|| bad("addition"))?,
            },
            other if SPECIAL.contains(&other) => Self::Special(
                SPECIAL
                    .iter()
                    .find(|known| **known == other)
                    .copied()
                    .unwrap_or("unknown"),
            ),
            _ => return Err(RecipeError::UnknownType(kind.to_owned())),
        })
    }

    /// Whether this recipe is one the crafting grid can make.
    #[must_use]
    pub fn is_crafting(&self) -> bool {
        matches!(
            self,
            Self::Shaped { .. } | Self::Shapeless { .. } | Self::Transmute { .. }
        )
    }

    /// Whether a grid holds what this recipe wants.
    #[must_use]
    pub fn matches_grid(&self, tags: &TagRegistry, input: &CraftingInput) -> bool {
        match self {
            Self::Shaped { pattern, .. } => pattern.matches(tags, input),
            Self::Shapeless { ingredients, .. } => matches_shapeless(tags, ingredients, input),
            // One of the input, and between one and eight of the material, whose count the recipe
            // may bound. What comes out is the input changed rather than something new, so it is
            // only a match when the two would differ.
            Self::Transmute {
                input: wanted,
                material,
                result,
            } => {
                let mut found_input = None;
                let mut materials = 0;
                for item in input.items() {
                    if wanted.matches(tags, item) && found_input.is_none() {
                        found_input = Some(item);
                    } else if material.matches(tags, item) {
                        materials += 1;
                    } else {
                        return false;
                    }
                }
                found_input.is_some_and(|found| found != result.item)
                    && (1..=8).contains(&materials)
            }
            _ => false,
        }
    }

    /// What comes out, where the recipe makes something on its own.
    #[must_use]
    pub fn result(&self) -> Option<ResultStack> {
        match self {
            Self::Shaped { result, .. }
            | Self::Shapeless { result, .. }
            | Self::Cooking { result, .. }
            | Self::Stonecutting { result, .. }
            | Self::Transmute { result, .. }
            | Self::SmithingTransform { result, .. } => Some(*result),
            // A trim and the special recipes all build their result out of components.
            Self::SmithingTrim { .. } | Self::Special(_) => None,
        }
    }
}

/// The recipes vanilla writes as code. Every one of them reads or writes an item's components —
/// the pages of a book, the colours of a firework, the damage on a repaired tool — so they are
/// read and match nothing until an item can carry any.
const SPECIAL: &[&str] = &[
    "crafting_special_bannerduplicate",
    "crafting_special_bookcloning",
    "crafting_special_firework_rocket",
    "crafting_special_firework_star",
    "crafting_special_firework_star_fade",
    "crafting_special_mapextending",
    "crafting_special_repairitem",
    "crafting_special_shielddecoration",
    "crafting_decorated_pot",
    "crafting_dye",
    "crafting_imbue",
];

/// Every recipe the loaded packs declare.
#[derive(Debug, Default)]
pub struct RecipeBook {
    by_name: BTreeMap<String, Recipe>,
}

impl RecipeBook {
    /// Reads every recipe in a pack stack.
    #[must_use]
    pub fn load(manager: &ResourceManager) -> Self {
        let mut by_name = BTreeMap::new();
        for (id, resource) in FileToId::json(DIRECTORY).list(manager) {
            match serde_json::from_slice(&resource.data)
                .map_err(|e| e.to_string())
                .and_then(|value: Value| Recipe::parse(&value).map_err(|e| e.to_string()))
            {
                Ok(recipe) => {
                    by_name.insert(id.as_str().to_owned(), recipe);
                }
                Err(e) => error!(
                    "couldn't read recipe {id} from data pack {}: {e}",
                    resource.source
                ),
            }
        }
        Self { by_name }
    }

    #[must_use]
    pub fn get(&self, name: &Identifier) -> Option<&Recipe> {
        self.by_name.get(name.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Recipe)> {
        self.by_name.iter().map(|(name, recipe)| (&**name, recipe))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// The first recipe a grid makes, if any.
    #[must_use]
    pub fn match_grid(&self, tags: &TagRegistry, input: &CraftingInput) -> Option<&Recipe> {
        if input.is_empty() {
            return None;
        }
        self.by_name
            .values()
            .filter(|recipe| recipe.is_crafting())
            .find(|recipe| recipe.matches_grid(tags, input))
    }

    /// What a furnace of this kind makes of an item.
    #[must_use]
    pub fn match_cooking(
        &self,
        tags: &TagRegistry,
        kind: CookingKind,
        item: i32,
    ) -> Option<&Recipe> {
        self.by_name.values().find(|recipe| match recipe {
            Recipe::Cooking {
                kind: recipe_kind,
                ingredient,
                ..
            } => *recipe_kind == kind && ingredient.matches(tags, item),
            _ => false,
        })
    }

    /// Everything a stonecutter can cut an item into.
    pub fn match_stonecutting<'a>(
        &'a self,
        tags: &'a TagRegistry,
        item: i32,
    ) -> impl Iterator<Item = &'a Recipe> {
        self.by_name.values().filter(move |recipe| match recipe {
            Recipe::Stonecutting { ingredient, .. } => ingredient.matches(tags, item),
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests;
