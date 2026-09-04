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

#[cfg(test)]
mod component_type_tests {
    use crate::generated::components::ComponentType;
    use ferrumc_net_codec::version::ProtocolVersion;

    #[test]
    fn every_kind_the_registry_has_is_here() {
        assert_eq!(ComponentType::ALL.len(), 111);
        assert_eq!(
            ComponentType::from_name("minecraft:custom_name"),
            Some(ComponentType::CustomName)
        );
        assert_eq!(ComponentType::CustomName.to_name(), "custom_name");
    }

    /// A component's number is a place in the reader's own registry, and that registry went from
    /// 57 kinds to 111 across the supported versions.
    #[test]
    fn the_same_kind_is_a_different_number_to_a_different_client() {
        assert_eq!(
            ComponentType::CustomName.wire_id(ProtocolVersion::V26_2),
            Some(6)
        );
        assert_eq!(
            ComponentType::CustomName.wire_id(ProtocolVersion::V1_21),
            Some(5)
        );
    }

    /// One a version has never heard of has no number, and saying so is the only safe answer: a
    /// component carries no length, so a wrong one makes the rest of the stack unreadable.
    #[test]
    fn a_kind_a_version_does_not_know_has_no_number() {
        assert_eq!(ComponentType::Weapon.wire_id(ProtocolVersion::V1_21), None);
        assert!(ComponentType::Weapon
            .wire_id(ProtocolVersion::V26_2)
            .is_some());
    }

    #[test]
    fn a_number_reads_back_as_the_kind_that_client_meant() {
        for version in ProtocolVersion::ALL {
            for kind in [
                ComponentType::CustomName,
                ComponentType::Damage,
                ComponentType::Enchantments,
                ComponentType::CustomData,
            ] {
                let Some(id) = kind.wire_id(version) else {
                    continue;
                };
                assert_eq!(
                    ComponentType::from_wire_id(id, version),
                    Some(kind),
                    "{kind:?} on {version:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod enchantment_tests {
    use crate::generated::enchantments::Enchantment;
    use ferrumc_net_codec::version::ProtocolVersion;

    #[test]
    fn the_enchantment_the_newest_version_added_is_here() {
        assert!(Enchantment::from_name("minecraft:lunge").is_some());
    }

    /// `lunge` was added in 26.1 in the middle of the alphabet, which moved twenty-one of the
    /// forty-two after it. A level of sharpness sent as 26.2 numbers it is a different
    /// enchantment to an older client.
    #[test]
    fn an_enchantment_is_a_different_number_to_a_different_client() {
        let sharpness = Enchantment::from_name("sharpness").expect("it is an enchantment");
        assert_ne!(
            sharpness.wire_id(ProtocolVersion::V26_2),
            sharpness.wire_id(ProtocolVersion::V1_21)
        );
    }

    #[test]
    fn one_a_version_does_not_know_has_no_number() {
        let lunge = Enchantment::from_name("lunge").expect("it is an enchantment");
        assert_eq!(lunge.wire_id(ProtocolVersion::V1_21), None);
        assert!(lunge.wire_id(ProtocolVersion::V26_2).is_some());
    }

    #[test]
    fn a_number_reads_back_as_the_enchantment_that_client_meant() {
        for version in ProtocolVersion::ALL {
            for name in ["sharpness", "mending", "protection", "unbreaking"] {
                let known = Enchantment::from_name(name).expect("it is an enchantment");
                let Some(id) = known.wire_id(version) else {
                    continue;
                };
                assert_eq!(
                    Enchantment::from_wire_id(id, version).map(|read| read.name),
                    Some(known.name),
                    "{name} on {version:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod block_property_tests {
    use crate::generated::block_properties::{hardness, light, needs_the_right_tool, STATES};

    /// Every state the world can hold, read straight off the game.
    #[test]
    fn every_state_the_newest_version_has_is_here() {
        assert_eq!(STATES, 32366);
    }

    #[test]
    fn the_numbers_are_the_games_own() {
        assert_eq!(hardness(1), 1.5, "stone");
        assert_eq!(hardness(10), 0.5, "dirt");
        assert_eq!(hardness(3369), 50.0, "obsidian");
        assert_eq!(hardness(0), 0.0, "air goes at a touch");
    }

    /// A negative answer means nothing breaks it.
    #[test]
    fn bedrock_is_not_breakable() {
        assert!(hardness(85) < 0.0);
    }

    /// Which decides both what it drops and how much slower the wrong tool is.
    #[test]
    fn stone_needs_a_pickaxe_and_dirt_needs_nothing() {
        assert!(needs_the_right_tool(1), "stone");
        assert!(!needs_the_right_tool(10), "dirt");
        assert!(!needs_the_right_tool(137), "an oak log drops to a fist");
    }

    /// A state past the end answers with something a player can dig rather than something that
    /// strands them.
    #[test]
    fn a_state_this_server_does_not_know_is_not_unbreakable() {
        assert_eq!(hardness(u32::MAX), 0.0);
        assert!(!needs_the_right_tool(u32::MAX));
        assert_eq!(light(u32::MAX), 0);
    }
}

#[cfg(test)]
mod what_drops_by_hand {
    use crate::generated::block_properties::needs_the_right_tool;

    /// The distinction the whole thing rests on, checked against the states the game gave.
    ///
    /// A shovel is *faster* on dirt and grass, and a fist still drops them. What decides the drop
    /// is the block's own flag and not what is held.
    #[test]
    fn the_soft_blocks_drop_to_a_bare_hand() {
        // dirt, grass block, sand, gravel: their default states.
        for (name, state) in [
            ("dirt", 10),
            ("grass block", 9),
            ("sand", 118),
            ("gravel", 124),
        ] {
            assert!(
                !needs_the_right_tool(state),
                "{name} should come up with a fist"
            );
        }
    }

    #[test]
    fn stone_and_ore_do_not() {
        for (name, state) in [("stone", 1), ("iron ore", 131)] {
            assert!(
                needs_the_right_tool(state),
                "{name} should need a pickaxe to leave anything"
            );
        }
    }
}

#[cfg(test)]
mod crafting_remainder_tests {
    use crate::generated::items::{crafting_remainder, Item};

    fn id(name: &str) -> u16 {
        Item::from_registry_key(name).expect("it is an item").id
    }

    /// A bucket of milk in a cake leaves the bucket.
    #[test]
    fn a_bucket_comes_back() {
        assert_eq!(
            crafting_remainder(id("minecraft:milk_bucket")),
            Some(id("minecraft:bucket"))
        );
        assert_eq!(
            crafting_remainder(id("minecraft:water_bucket")),
            Some(id("minecraft:bucket"))
        );
        assert_eq!(
            crafting_remainder(id("minecraft:honey_bottle")),
            Some(id("minecraft:glass_bottle"))
        );
    }

    /// Everything else is used up.
    #[test]
    fn most_things_leave_nothing() {
        for name in ["minecraft:dirt", "minecraft:diamond", "minecraft:wheat"] {
            assert_eq!(crafting_remainder(id(name)), None, "{name}");
        }
    }
}

#[cfg(test)]
mod enchantment_effect_tests {
    use crate::generated::enchantments::{
        Effect, Enchantment, Hook, LevelValue, Operation, Requires,
    };

    fn effects(name: &str) -> &'static [Effect] {
        Enchantment::from_name(name)
            .expect("it is an enchantment")
            .effects
    }

    /// Sharpness adds one at level one and half a point a level after.
    #[test]
    fn sharpness_adds_what_the_packs_say() {
        let [sharpness] = effects("sharpness") else {
            panic!("sharpness does one thing");
        };
        assert_eq!(sharpness.hook, Hook::Damage);
        assert_eq!(sharpness.value.at(1), 1.0);
        assert_eq!(
            sharpness.value.at(5),
            3.0,
            "one, and half a point four times"
        );
        assert_eq!(sharpness.requires, Requires::Always);
    }

    /// Efficiency runs away because it is the level *squared*.
    #[test]
    fn efficiency_is_the_level_squared() {
        let [efficiency] = effects("efficiency") else {
            panic!("efficiency does one thing");
        };
        assert!(matches!(
            efficiency.hook,
            Hook::Attribute {
                attribute: "mining_efficiency",
                operation: Operation::AddValue,
                ..
            }
        ));
        assert_eq!(efficiency.value.at(1), 2.0, "one squared and one added");
        assert_eq!(efficiency.value.at(5), 26.0, "twenty-five and one");
    }

    /// Protection guards against anything; feather falling only against falling. Getting the
    /// second one wrong would have it protect against everything.
    #[test]
    fn feather_falling_only_applies_to_a_fall() {
        let [protection] = effects("protection") else {
            panic!("protection does one thing");
        };
        assert_eq!(protection.hook, Hook::Protection);
        assert_eq!(
            protection.requires,
            Requires::DamageTags(&[("bypasses_invulnerability", false)])
        );

        let [feather] = effects("feather_falling") else {
            panic!("feather falling does one thing");
        };
        assert_eq!(
            feather.requires,
            Requires::DamageTags(&[("is_fall", true), ("bypasses_invulnerability", false)])
        );
        assert_eq!(feather.value.at(4), 12.0, "three a level");
    }

    #[test]
    fn knockback_adds_one_a_level() {
        let [knockback] = effects("knockback") else {
            panic!("knockback does one thing");
        };
        assert_eq!(knockback.hook, Hook::Knockback);
        assert_eq!(knockback.value.at(2), 2.0);
    }

    /// Aqua affinity takes the underwater penalty away by multiplying the attribute up.
    #[test]
    fn aqua_affinity_multiplies_rather_than_adds() {
        let [aqua] = effects("aqua_affinity") else {
            panic!("aqua affinity does one thing");
        };
        assert!(matches!(
            aqua.hook,
            Hook::Attribute {
                attribute: "submerged_mining_speed",
                operation: Operation::AddMultipliedTotal,
                ..
            }
        ));
        assert_eq!(aqua.value.at(1), 4.0);
    }

    /// A level of one is a level of one, not a level of zero.
    #[test]
    fn a_level_counts_from_one() {
        let linear = LevelValue::Linear {
            base: 1.0,
            per_level: 0.5,
        };
        assert_eq!(linear.at(1), 1.0);
        assert_eq!(linear.at(0), 1.0, "and nothing is treated as one");
    }

    /// Something whose shape this does not read never applies, rather than applying always.
    #[test]
    fn an_effect_behind_an_unread_condition_never_applies() {
        // Every effect that was read carries a requirement that can be answered.
        for (_, enchantment) in [("sharpness", "sharpness")] {
            for effect in effects(enchantment) {
                assert!(
                    effect.requires != Requires::SomethingUnread || effect.hook == Hook::Damage,
                    "an unread requirement should be the cautious answer"
                );
            }
        }
    }
}
