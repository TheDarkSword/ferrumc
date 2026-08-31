//! What has to be true of a place before a mob appears there.
//!
//! Vanilla holds one of these per entity type as a method reference. The ones more than one mob
//! shares are here; the rest belong to a single mob and to its behaviour, and a type carrying one
//! is refused rather than let through, so nothing appears somewhere it should not.

use crate::{Pos, SpawnWorld};
use ferrumc_entities::entity_type::{EntityType, SpawnRule};
use rand::Rng;

/// How much daylight a place may have and still be dark enough for a monster.
///
/// Vanilla rolls against thirty-two rather than comparing to a number, so a place in half-light
/// sometimes passes and sometimes does not.
const SKY_LIGHT_ROLL: u8 = 32;

/// How much light a block may give off before a monster will not stand near it, in the overworld.
///
/// The dimension carries this; the overworld's is nothing at all, so a single torch is enough.
const MONSTER_BLOCK_LIGHT_LIMIT: u8 = 0;

/// How dark it has to be overall for a monster: no light at all reaching the place.
const MONSTER_BRIGHTNESS_LIMIT: u8 = 0;

/// How bright it has to be for an animal.
const ANIMAL_BRIGHTNESS_FLOOR: u8 = 8;

/// Whether a mob of this kind may appear here, beyond where it may stand.
#[must_use]
pub fn rule_holds(world: &dyn SpawnWorld, kind: EntityType, at: Pos, rng: &mut impl Rng) -> bool {
    match kind.def().spawn_rule {
        SpawnRule::None => true,
        SpawnRule::Standable | SpawnRule::AnyLight => world.standable(at - Pos::Y),
        SpawnRule::Dark => dark_enough(world, at, rng) && world.standable(at - Pos::Y),
        SpawnRule::DarkUnderSky => {
            dark_enough(world, at, rng) && world.standable(at - Pos::Y) && world.can_see_sky(at)
        }
        SpawnRule::Animal => {
            // Vanilla asks a block tag here rather than the block's own spawn answer, and until
            // something reads that tag, standing on it is the closest true thing.
            world.standable(at - Pos::Y) && world.brightness(at) > ANIMAL_BRIGHTNESS_FLOOR
        }
        SpawnRule::SurfaceWater => world.is_water(at) && world.is_water(at - Pos::Y),
        // A rule of the mob's own, which nothing works out yet.
        SpawnRule::OwnRule => false,
    }
}

/// Whether it is dark enough here for a monster.
///
/// Three separate questions, and vanilla asks them in this order: daylight first, since a lit sky
/// rules out most of the world at once; then whether any block is giving off light; then how dark
/// it is all told.
fn dark_enough(world: &dyn SpawnWorld, at: Pos, rng: &mut impl Rng) -> bool {
    if world.sky_light(at) > rng.gen_range(0..SKY_LIGHT_ROLL) {
        return false;
    }
    if world.block_light(at) > MONSTER_BLOCK_LIGHT_LIMIT {
        return false;
    }
    world.brightness(at) == MONSTER_BRIGHTNESS_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_world::block_state_id::BlockStateId;
    use rand::SeedableRng;

    /// A world that answers however a test wants it to.
    #[derive(Default)]
    struct Flat {
        sky: u8,
        block: u8,
        bright: u8,
        standable: bool,
        water: bool,
        open: bool,
    }

    impl SpawnWorld for Flat {
        fn block_at(&self, _pos: Pos) -> BlockStateId {
            BlockStateId::default()
        }
        fn block_light(&self, _pos: Pos) -> u8 {
            self.block
        }
        fn sky_light(&self, _pos: Pos) -> u8 {
            self.sky
        }
        fn can_see_sky(&self, _pos: Pos) -> bool {
            self.open
        }
        fn brightness(&self, _pos: Pos) -> u8 {
            self.bright
        }
        fn spawners_at(
            &self,
            _pos: Pos,
            _c: ferrumc_entities::entity_type::MobCategory,
        ) -> &[crate::Spawner] {
            &[]
        }
        fn surface_at(&self, _x: i32, _z: i32) -> i32 {
            64
        }
        fn min_y(&self) -> i32 {
            -64
        }
        fn nearest_player_sqr(&self, _x: f64, _y: f64, _z: f64) -> Option<f64> {
            None
        }
        fn fits(&self, _k: EntityType, _x: f64, _y: f64, _z: f64) -> bool {
            true
        }
        fn standable(&self, _pos: Pos) -> bool {
            self.standable
        }
        fn is_water(&self, _pos: Pos) -> bool {
            self.water
        }
        fn is_solid(&self, _pos: Pos) -> bool {
            false
        }
    }

    fn rng() -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(1)
    }

    #[test]
    fn a_monster_wants_the_dark() {
        let dark = Flat {
            standable: true,
            ..Flat::default()
        };
        assert!(rule_holds(&dark, EntityType::Zombie, Pos::ZERO, &mut rng()));

        let lit = Flat {
            sky: 15,
            bright: 15,
            standable: true,
            ..Flat::default()
        };
        assert!(!rule_holds(&lit, EntityType::Zombie, Pos::ZERO, &mut rng()));
    }

    #[test]
    fn one_torch_is_enough_to_keep_a_monster_away() {
        let torchlit = Flat {
            block: 1,
            standable: true,
            ..Flat::default()
        };
        assert!(!rule_holds(
            &torchlit,
            EntityType::Zombie,
            Pos::ZERO,
            &mut rng()
        ));
    }

    #[test]
    fn a_monster_wants_something_to_stand_on() {
        let midair = Flat::default();
        assert!(!rule_holds(
            &midair,
            EntityType::Zombie,
            Pos::ZERO,
            &mut rng()
        ));
    }

    #[test]
    fn an_animal_wants_the_light() {
        let bright = Flat {
            bright: 15,
            standable: true,
            ..Flat::default()
        };
        assert!(rule_holds(&bright, EntityType::Pig, Pos::ZERO, &mut rng()));

        let gloomy = Flat {
            bright: ANIMAL_BRIGHTNESS_FLOOR,
            standable: true,
            ..Flat::default()
        };
        assert!(!rule_holds(&gloomy, EntityType::Pig, Pos::ZERO, &mut rng()));
    }

    #[test]
    fn a_fish_wants_water() {
        let sea = Flat {
            water: true,
            ..Flat::default()
        };
        assert!(rule_holds(&sea, EntityType::Cod, Pos::ZERO, &mut rng()));

        let beach = Flat {
            standable: true,
            ..Flat::default()
        };
        assert!(!rule_holds(&beach, EntityType::Cod, Pos::ZERO, &mut rng()));
    }

    #[test]
    fn a_mob_with_a_rule_of_its_own_is_refused_rather_than_let_through() {
        // A slime decides for itself, by chunk and by depth, and nothing works that out yet. It
        // has to be refused: letting it through would put slimes everywhere rather than nowhere.
        let anywhere = Flat {
            standable: true,
            bright: 15,
            ..Flat::default()
        };
        assert_eq!(EntityType::Slime.def().spawn_rule, SpawnRule::OwnRule);
        assert!(!rule_holds(
            &anywhere,
            EntityType::Slime,
            Pos::ZERO,
            &mut rng()
        ));
    }
}
