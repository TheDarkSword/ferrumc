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
