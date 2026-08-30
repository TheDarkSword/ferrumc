//! What a predicate is asked about.
//!
//! Vanilla calls this a loot context: a bag of parameters saying what is going on, a source of
//! randomness, and a way to reach the world. Each thing that evaluates predicates declares which
//! parameters it provides, and a predicate that wants one that is not there fails rather than
//! throwing — which is what makes `killed_by_player` mean "there was a player" at all.

use ferrumc_registry::tags::GameTags;
use ferrumc_world::block_state_id::BlockStateId;
use ferrumc_world::light::LightLayer;
use ferrumc_world::pos::BlockPos;
use rand::RngCore;
use std::sync::Arc;

/// A position in the world, which is a loot context's `origin`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Origin {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Origin {
    #[must_use]
    pub fn block(self) -> BlockPos {
        BlockPos::of(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.z.floor() as i32,
        )
    }
}

/// The item a predicate is asked about, as much of one as exists so far.
///
/// Vanilla asks an item stack for its components as well; nothing here carries any, so a matcher
/// on them never matches — which is the right answer for a tool with no enchantments and the
/// wrong one for a tool with some, once those can exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemRef {
    /// The item's registry id.
    pub id: i32,
    pub count: i32,
}

/// The parameters a predicate may be given.
///
/// Vanilla keys these by a typed context key and refuses at load time to evaluate a predicate that
/// wants one its caller does not provide. The set is fixed by the game, so it is named here
/// instead; absent is absent either way.
#[derive(Clone, Copy, Debug, Default)]
pub struct LootParams {
    /// Where this is happening.
    pub origin: Option<Origin>,
    /// The block being broken or looked at.
    pub block_state: Option<BlockStateId>,
    /// What is being used on it.
    pub tool: Option<ItemRef>,
    /// How big the explosion was, where one caused this.
    pub explosion_radius: Option<f32>,
    /// Whether a player struck the last blow.
    pub killed_by_player: bool,
    /// The level of the enchantment that caused this, where one did.
    pub enchantment_level: Option<i32>,
    /// Whether that enchantment is active.
    pub enchantment_active: Option<bool>,
}

/// The world, as much of it as a predicate may ask about.
///
/// A position that is not loaded reads as absent, which is how vanilla answers as well: it checks
/// `isLoaded` first and fails the whole predicate when it is not.
pub trait LootWorld {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId>;
    fn light(&self, pos: BlockPos, layer: LightLayer) -> Option<u8>;
    fn can_see_sky(&self, pos: BlockPos) -> Option<bool>;
    /// The dimension this is happening in, namespaced.
    fn dimension(&self) -> &str;
    /// Ticks since the world began, which `time_check` reads.
    fn time(&self) -> i64;
    fn is_raining(&self) -> bool;
    fn is_thundering(&self) -> bool;
}

/// Everything a predicate is evaluated against.
pub struct LootContext<'a> {
    pub params: LootParams,
    pub random: &'a mut dyn RngCore,
    /// The tags as they stood when this evaluation began, so a condition asking whether a block is
    /// a log does not reach for the global once per block.
    pub tags: Arc<GameTags>,
    /// Absent where there is no world to ask — a predicate that wants one then behaves as it does
    /// for a position that is not loaded.
    pub world: Option<&'a dyn LootWorld>,
    /// The predicates a `reference` can name.
    pub predicates: Option<&'a crate::condition::Predicates>,
    /// Which references are being followed, so a loop is caught rather than chased.
    pub(crate) visiting: Vec<String>,
}

impl<'a> LootContext<'a> {
    pub fn new(params: LootParams, random: &'a mut dyn RngCore) -> Self {
        Self {
            params,
            random,
            tags: ferrumc_registry::tags::current(),
            world: None,
            predicates: None,
            visiting: Vec::new(),
        }
    }

    /// Evaluates against a particular set of tags rather than the ones the server is on.
    #[must_use]
    pub fn with_tags(mut self, tags: Arc<GameTags>) -> Self {
        self.tags = tags;
        self
    }

    #[must_use]
    pub fn with_world(mut self, world: &'a dyn LootWorld) -> Self {
        self.world = Some(world);
        self
    }

    #[must_use]
    pub fn with_predicates(mut self, predicates: &'a crate::condition::Predicates) -> Self {
        self.predicates = Some(predicates);
        self
    }

    /// A roll in `[0, 1)`, as vanilla's `nextFloat` gives.
    pub(crate) fn next_float(&mut self) -> f32 {
        // The same construction as java's: the top 24 bits over 2^24.
        (self.random.next_u32() >> 8) as f32 / (1 << 24) as f32
    }

    /// A whole number in `[min, max]`, as vanilla's `Mth.nextInt` gives.
    pub(crate) fn next_int(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let span = (max - min) as u32 + 1;
        min + (self.random.next_u32() % span) as i32
    }
}
