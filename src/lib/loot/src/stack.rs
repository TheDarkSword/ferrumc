//! What a loot table produces.

/// An item and how many of it.
///
/// Vanilla's `ItemStack` also carries the item's components — its enchantments, its damage, its
/// name. None of those exist yet, so this is the half of it a loot table can fill in. It moves
/// into the item model when there is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    /// The item's registry id.
    pub item: i32,
    pub count: i32,
}

impl ItemStack {
    #[must_use]
    pub fn new(item: i32) -> Self {
        Self { item, count: 1 }
    }

    /// A stack of nothing, which is what a count of nought or less means.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count <= 0
    }

    /// The item's name, for a message or a log.
    #[must_use]
    pub fn name(&self) -> Option<&'static str> {
        ferrumc_registry::lookup_item_name(self.item)
    }
}
