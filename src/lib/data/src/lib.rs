// Include generated modules

pub mod generated;

// Include build-generated blocks module
include!(concat!(env!("OUT_DIR"), "/blocks.rs"));

// Re-export all generated types for convenience
pub use generated::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod damage_type_tests {
    use crate::generated::damage_types::DamageType;

    /// The kinds the packs added most recently, which a dump beside them did not have.
    #[test]
    fn every_kind_the_packs_define_is_here() {
        assert!(DamageType::from_name("spear").is_some());
        assert!(DamageType::from_name("sulfur_cube_hot").is_some());
    }

    /// Which group a kind is in decides what softens it, so these come from the packs rather than
    /// from memory.
    #[test]
    fn a_kind_knows_which_groups_it_is_in() {
        // Starving goes through armour and through resistance; a sword does neither.
        assert!(DamageType::Starve.goes_through_armour());
        assert!(!DamageType::PlayerAttack.goes_through_armour());

        assert!(DamageType::Fall.is_fall());
        assert!(!DamageType::Fall.is_fire());
        assert!(DamageType::InFire.is_fire());
        assert!(DamageType::Drown.is_drowning());
        assert!(DamageType::Explosion.is_explosion());

        // The void is not stopped by anything at all.
        assert!(DamageType::OutOfWorld.goes_through_invulnerability());
    }
}

#[cfg(test)]
mod damage_type_wire_tests {
    use crate::generated::damage_types::DamageType;
    use ferrumc_net_codec::version::ProtocolVersion;

    /// The number a client reads is a place in its own registry, and the registry has grown four
    /// times across the supported versions. Sending the newest number to an older client names a
    /// different kind of damage.
    #[test]
    fn the_same_kind_has_different_numbers_on_different_versions() {
        // `spear` was added last and sits before `sonic_boom`, so everything after it shifts.
        let newest = DamageType::SonicBoom.wire_id(ProtocolVersion::V26_2);
        let oldest = DamageType::SonicBoom.wire_id(ProtocolVersion::V1_21);
        assert!(newest.is_some() && oldest.is_some());
        assert_ne!(newest, oldest, "the registry grew in between");
    }

    /// A kind a version has never heard of has no number, and saying so is better than sending one
    /// that means something else.
    #[test]
    fn a_kind_a_version_does_not_know_has_no_number() {
        assert_eq!(DamageType::Spear.wire_id(ProtocolVersion::V1_21), None);
        assert!(DamageType::Spear.wire_id(ProtocolVersion::V26_2).is_some());
    }

    /// Every version knows the kinds that have always been there.
    #[test]
    fn every_version_knows_the_old_kinds() {
        for version in ProtocolVersion::ALL {
            for kind in [
                DamageType::Fall,
                DamageType::Drown,
                DamageType::OnFire,
                DamageType::InFire,
                DamageType::Lava,
                DamageType::OutOfWorld,
                DamageType::PlayerAttack,
            ] {
                assert!(kind.wire_id(version).is_some(), "{kind:?} on {version:?}");
            }
        }
    }
}

#[cfg(test)]
mod item_tests {
    use crate::generated::items::Item;

    /// The numbers items travel as come from the registry the client is sent, so the two cannot
    /// disagree. The dump this replaced had 1389 of 1416 wrong.
    #[test]
    fn an_items_number_is_the_registrys_own() {
        let sword = Item::from_registry_key("minecraft:diamond_sword").expect("it is an item");
        assert_eq!(sword.id, Item::DIAMOND_SWORD.id);
        assert_eq!(
            Item::from_id(Item::DIAMOND_SWORD.id).map(|item| item.registry_key),
            Some("minecraft:diamond_sword"),
            "the number and the name have to agree in both directions"
        );
    }

    /// The kinds added most recently, which the dump did not have at all.
    #[test]
    fn the_items_the_newest_version_added_are_here() {
        for name in [
            "minecraft:copper_sword",
            "minecraft:copper_axe",
            "minecraft:cinnabar",
            "minecraft:bamboo_shelf",
        ] {
            assert!(Item::from_registry_key(name).is_some(), "{name}");
        }
    }
}

#[cfg(test)]
mod attribute_tests {
    use crate::attributes::Attribute;

    /// The numbers attributes travel as are the registry's own, and the registry grew by five.
    #[test]
    fn every_attribute_the_game_has_is_here() {
        for name in [
            "minecraft:air_drag_modifier",
            "minecraft:below_name_distance",
            "minecraft:bounciness",
            "minecraft:friction_modifier",
            "minecraft:name_tag_distance",
        ] {
            assert!(Attribute::from_name(name).is_some(), "{name}");
        }
        assert_eq!(Attribute::from_name("gravity").map(|a| a.id), Some(18));
    }

    /// Written out in full rather than rounded: gravity used to read as 0.1.
    #[test]
    fn a_default_is_not_rounded_off() {
        let gravity = Attribute::from_name("gravity").expect("gravity is an attribute");
        assert!((gravity.default_value - 0.08).abs() < 1e-12, "{gravity:?}");
    }

    /// An attribute holds a value to its own range, which is what stops a modifier going silly.
    #[test]
    fn an_attribute_holds_a_value_to_its_own_range() {
        let armour = Attribute::from_name("armor").expect("armour is an attribute");
        assert_eq!(armour.clamp(-5.0), 0.0);
        assert_eq!(armour.clamp(100.0), 30.0);
        assert_eq!(armour.clamp(12.0), 12.0);
    }
}

#[cfg(test)]
mod consumable_tests {
    use crate::generated::items::{Aftermath, DataComponent, Item};

    fn aftermath(name: &str) -> &'static [crate::generated::items::ConsumeEffect] {
        let item = Item::from_registry_key(name).expect("it is an item");
        item.components
            .iter()
            .find_map(|(id, data)| {
                (id == &DataComponent::Consumable).then(|| {
                    data.as_any()
                        .downcast_ref::<crate::generated::items::ConsumableImpl>()
                        .map(|held| held.after)
                })
            })
            .flatten()
            .expect("it is a consumable")
    }

    /// A golden apple is worth reading: the effects it gives are what makes it what it is, and
    /// they are on the item rather than on any list here.
    #[test]
    fn a_golden_apple_gives_regeneration_and_absorption() {
        let after = aftermath("minecraft:golden_apple");
        let Aftermath::Apply(effects) = after[0].what else {
            panic!("a golden apple applies effects");
        };
        assert_eq!(
            effects,
            &[("regeneration", 1, 100), ("absorption", 0, 2400),]
        );
    }

    /// Rotten flesh only makes a player hungry four times in five.
    #[test]
    fn some_of_it_only_happens_sometimes() {
        let after = aftermath("minecraft:rotten_flesh");
        assert!((after[0].probability - 0.8).abs() < 1e-6);
    }

    /// Milk takes everything away; honey takes away only poison.
    #[test]
    fn milk_clears_everything_and_honey_clears_one_thing() {
        assert!(matches!(
            aftermath("minecraft:milk_bucket")[0].what,
            Aftermath::ClearEverything
        ));
        let Aftermath::Remove(named) = aftermath("minecraft:honey_bottle")[0].what else {
            panic!("honey takes something away");
        };
        assert_eq!(named, &["poison"]);
    }
}
