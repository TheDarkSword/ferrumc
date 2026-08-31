//! What an entity is, before anything it does.
//!
//! A type is an enum whose variants carry the registry's own numbers, so the value that goes on the
//! wire is the variant itself: no lookup, no table, no string. Everything else a type knows — how
//! big it is, how far it is tracked, where it may spawn — is one index into a flat table beside it.

pub mod generated;

pub use generated::{
    CategoryDef, EntityDef, EntityType, MobCategory, SpawnHeightmap, SpawnPlacement, SpawnRule,
};

use generated::ENTITY_TYPES;

/// How much smaller a young one is than a grown one.
///
/// Vanilla asks the mob rather than the type, and a few of them answer something else; every one
/// that does is a mob class, which is Phase 7's to write. This is what the rest of them say.
const BABY_SCALE: f32 = 0.5;

impl EntityType {
    /// The number the wire carries for this type.
    #[must_use]
    pub const fn protocol_id(self) -> u16 {
        self as u16
    }

    /// The type that number means, or nothing where the number is not one.
    #[must_use]
    pub fn from_protocol_id(id: u16) -> Option<Self> {
        // The variants are numbered from zero with no gaps, which the generator holds to and a
        // test checks, so a number in range is a variant.
        (usize::from(id) < ENTITY_TYPES.len()).then(|| {
            // SAFETY: the discriminants are exactly `0..ENTITY_TYPES.len()`, checked above and by
            // `every_id_round_trips`.
            unsafe { std::mem::transmute::<u16, Self>(id) }
        })
    }

    /// Everything the game says about this type.
    #[must_use]
    pub const fn def(self) -> &'static EntityDef {
        &ENTITY_TYPES[self as usize]
    }

    /// The name the registry gives it, namespace included.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.def().name
    }

    /// The type of this name. Accepts the bare form as well, since a command takes either.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let wanted = if name.contains(':') {
            name
        } else {
            return Self::from_name(&format!("minecraft:{name}"));
        };
        ENTITY_TYPES
            .iter()
            .position(|def| def.name == wanted)
            .and_then(|index| u16::try_from(index).ok())
            .and_then(Self::from_protocol_id)
    }

    #[must_use]
    pub const fn category(self) -> MobCategory {
        self.def().category
    }

    /// How wide and tall it is. An entity's box is square in plan.
    #[must_use]
    pub const fn size(self) -> (f32, f32) {
        (self.def().width, self.def().height)
    }

    /// How far up its box it looks from.
    #[must_use]
    pub const fn eye_height(self) -> f32 {
        self.def().eye_height
    }

    /// How many chunks away a client is still told about it.
    #[must_use]
    pub const fn tracking_range(self) -> u8 {
        self.def().tracking_range
    }

    /// How many ticks between updates, or nothing where it is never updated again.
    #[must_use]
    pub const fn update_interval(self) -> Option<u32> {
        self.def().update_interval
    }

    /// What a living one starts with, or nothing where it is not a living entity.
    #[must_use]
    pub const fn max_health(self) -> Option<f32> {
        self.def().max_health
    }

    /// Whether it is a mob rather than a thing: everything but the miscellaneous group.
    #[must_use]
    pub const fn is_mob(self) -> bool {
        !matches!(self.def().category, MobCategory::Misc)
    }

    #[must_use]
    pub const fn fire_immune(self) -> bool {
        self.def().fire_immune
    }

    /// Whether it is written with the chunk it is in.
    #[must_use]
    pub const fn serialize(self) -> bool {
        self.def().serialize
    }

    /// How big it is and where its eyes are, grown or young.
    ///
    /// A type of fixed size is the same either way: vanilla refuses to scale one, so a baby
    /// slime is not a smaller slime.
    #[must_use]
    pub fn size_of(self, baby: bool) -> (f32, f32, f32) {
        let def = self.def();
        if !baby || def.fixed_size {
            return (def.width, def.height, def.eye_height);
        }
        (
            def.width * BABY_SCALE,
            def.height * BABY_SCALE,
            def.eye_height * BABY_SCALE,
        )
    }

    /// How big it is and what its box is, grown or young.
    ///
    /// Vanilla asks the type rather than keeping a copy on every entity, and so does this: the
    /// answer is arithmetic on a table entry, which is cheaper than the lookup it replaces.
    #[must_use]
    pub fn physical(self, baby: bool) -> crate::components::PhysicalProperties {
        let (width, height, eye_height) = self.size_of(baby);
        crate::components::PhysicalProperties {
            bounding_box: crate::components::BoundingBox::of(width, height),
            eye_height,
            fire_immune: self.fire_immune(),
        }
    }

    /// How a tick moves one of these when nothing else does.
    #[must_use]
    pub const fn motion(self) -> &'static ferrumc_physics::Motion {
        &self.def().motion
    }

    /// Every type there is.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..ENTITY_TYPES.len())
            .filter_map(|index| u16::try_from(index).ok())
            .filter_map(Self::from_protocol_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the numbering: what goes on the wire is the variant. If the generator
    /// ever produced a gap, transmuting a number back would be reading a variant that is not there.
    #[test]
    fn every_id_round_trips() {
        for (index, def) in ENTITY_TYPES.iter().enumerate() {
            let id = u16::try_from(index).expect("there are not sixty thousand entity types");
            let kind = EntityType::from_protocol_id(id).expect("a number in range is a type");
            assert_eq!(kind.protocol_id(), id);
            assert_eq!(kind.name(), def.name, "at {index}");
        }
        assert!(
            EntityType::from_protocol_id(u16::try_from(ENTITY_TYPES.len()).expect("in range"))
                .is_none()
        );
    }

    /// The numbers are the game's own, not ours: this is what the stale table got wrong, and every
    /// entity was spawned as whatever sat at the old index.
    #[test]
    fn the_numbers_are_the_registrys_own() {
        for kind in EntityType::all() {
            let registered =
                ferrumc_registry::tags::protocol_id("minecraft:entity_type", kind.name());
            assert_eq!(
                registered,
                Some(i32::from(kind.protocol_id())),
                "{} should sit where the registry puts it",
                kind.name()
            );
        }
    }

    #[test]
    fn a_type_is_found_by_name_either_way_round() {
        assert_eq!(
            EntityType::from_name("minecraft:pig"),
            Some(EntityType::Pig)
        );
        assert_eq!(EntityType::from_name("pig"), Some(EntityType::Pig));
        assert_eq!(EntityType::from_name("mypack:invented"), None);
    }

    #[test]
    fn a_type_knows_what_the_game_says_about_it() {
        let pig = EntityType::Pig;
        assert_eq!(pig.category(), MobCategory::Creature);
        assert_eq!(pig.max_health(), Some(10.0));
        assert!(pig.is_mob());
        assert_eq!(pig.def().placement, SpawnPlacement::OnGround);

        // A boat is a thing rather than a mob, and has no health of its own.
        let boat = EntityType::AcaciaBoat;
        assert_eq!(boat.category(), MobCategory::Misc);
        assert_eq!(boat.max_health(), None);
        assert!(!boat.is_mob());
    }

    /// What 4.5 needs of a type, and what the old table did not carry at all.
    #[test]
    fn a_type_says_how_it_is_tracked() {
        assert_eq!(EntityType::Pig.tracking_range(), 10);
        assert_eq!(EntityType::Pig.update_interval(), Some(3));

        // A painting never moves, so it is never updated again once sent.
        assert_eq!(EntityType::Painting.update_interval(), None);
    }

    /// A category is what decides whether a mob despawns and how many fit in a chunk.
    #[test]
    fn a_type_is_moved_the_way_the_game_moves_it() {
        // A dropped thing is pulled down at half the rate a mob is, and the things that hang
        // where they are put at none at all. None of that is guessable from the type — a squid is
        // pulled down as hard as a zombie, and it is the water that holds it up.
        assert_eq!(EntityType::Zombie.motion().gravity, 0.08);
        assert_eq!(EntityType::Squid.motion().gravity, 0.08);
        assert_eq!(EntityType::Item.motion().gravity, 0.04);
        assert_eq!(EntityType::Arrow.motion().gravity, 0.05);
        assert_eq!(EntityType::ExperienceOrb.motion().gravity, 0.03);
        assert_eq!(EntityType::Painting.motion().gravity, 0.0);

        assert!(EntityType::Zombie.motion().living);
        assert!(!EntityType::Item.motion().living);

        // A mob steps up a slab; a dropped thing walks into it.
        assert_eq!(EntityType::Zombie.motion().step_height, 0.6);
        assert_eq!(EntityType::Item.motion().step_height, 0.0);
    }

    #[test]
    fn a_category_carries_its_limits() {
        let monster = MobCategory::Monster.def();
        assert_eq!(monster.name, "monster");
        assert!(!monster.friendly);
        assert!(monster.max_per_chunk.is_some_and(|max| max > 0));
        assert!(monster.despawn_distance > monster.no_despawn_distance);

        // Nothing caps how many loose things a chunk may hold.
        assert_eq!(MobCategory::Misc.def().max_per_chunk, None);
    }
}
