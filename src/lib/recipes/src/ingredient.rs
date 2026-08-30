//! What a recipe asks for in a slot, and what it gives back.

use ferrumc_datapack::tag::TagRegistry;
use ferrumc_predicates::HolderSet;
use serde_json::Value;

/// One slot's worth of what a recipe accepts: an item, a list of them, or a tag.
///
/// Vanilla's `Ingredient`, which is a set over the item registry and nothing more. An empty one is
/// refused when it is read, since a recipe asking for nothing would match everything.
#[derive(Clone, Debug)]
pub struct Ingredient(HolderSet);

impl Ingredient {
    pub fn parse(value: &Value) -> Option<Self> {
        let set = HolderSet::parse(value)?;
        if let HolderSet::Direct(ref items) = set {
            if items.is_empty() {
                return None;
            }
        }
        Some(Self(set))
    }

    /// Whether this item would do.
    #[must_use]
    pub fn matches(&self, tags: &TagRegistry, item: i32) -> bool {
        let Ok(id) = u32::try_from(item) else {
            return false;
        };
        let Some(name) = ferrumc_registry::lookup_item_name(item) else {
            return false;
        };
        self.0.contains(tags, id, name)
    }
}

/// What a recipe produces.
///
/// Vanilla's `ItemStackTemplate` carries components as well — a written book's pages, a potion's
/// effect. Nothing here does yet, so a recipe whose whole point is the components it sets gives
/// back the plain item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultStack {
    /// The item's registry id.
    pub item: i32,
    pub count: i32,
    /// Whether the recipe wrote components that are not carried.
    pub has_unread_components: bool,
}

impl ResultStack {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let name = object.get("id")?.as_str()?;
        Some(Self {
            item: ferrumc_registry::lookup_item_protocol_id(name)?,
            count: object
                .get("count")
                .and_then(Value::as_i64)
                .and_then(|c| i32::try_from(c).ok())
                .unwrap_or(1),
            has_unread_components: object.contains_key("components"),
        })
    }
}
