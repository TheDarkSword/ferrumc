//! What a client is told about an entity.
//!
//! An entity carries a short row of values — its flags, its health, its name, the pose it is in —
//! that the server keeps and the client renders. Each sits at an index the game fixes by walking
//! the entity class tree, and travels tagged with the number of the serializer that writes it.
//!
//! Only what changed is sent. A value that is written back the same is not a change, so setting a
//! health to what it already was costs nothing.
//!
//! The server holds one entity in the newest version's terms and translates on the way out: a
//! client whose version puts a field somewhere else is told the place its own version uses, and one
//! whose version has no such field is not told about it at all.

mod generated;
mod value;

pub use generated::{
    ABSENT, Arm, ArmadilloState, CopperGolemState, Direction, Pose, Serializer, SnifferState,
    WeatheringState, fields,
};
pub use value::{DataField, DataValue, Field};

use crate::entity_type::EntityType;
use bevy_ecs::prelude::Component;
use ferrumc_net_codec::version::ProtocolVersion;
use generated::{LAYOUTS, place_of};

/// A bit of the byte every entity carries as its first field.
///
/// Vanilla keeps these as shift amounts on `Entity`; they are masks here because that is how they
/// are read. The third bit is not one of them: it belonged to a flag the game stopped sending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntityFlag {
    OnFire = 1 << 0,
    Crouching = 1 << 1,
    Sprinting = 1 << 3,
    Swimming = 1 << 4,
    Invisible = 1 << 5,
    Glowing = 1 << 6,
    FallFlying = 1 << 7,
}

/// A bit of the byte every living entity carries beside it.
///
/// Vanilla keeps these as masks on `LivingEntity` already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LivingFlag {
    UsingItem = 1,
    OffHand = 2,
    SpinAttack = 4,
}

/// Everything a client is told about one entity.
#[derive(Debug, Clone, Component)]
pub struct SyncedData {
    kind: EntityType,
    values: Box<[DataValue]>,
    /// One bit per field. No entity type has anywhere near sixty-four of them, which the
    /// generator checks, so a single word covers every one of them.
    changed: u64,
}

impl SyncedData {
    /// A new entity's values, each starting where the game starts it.
    #[must_use]
    pub fn new(kind: EntityType) -> Self {
        let values = LAYOUTS[kind as usize]
            .iter()
            .map(|slot| slot.default.clone())
            .collect();
        Self {
            kind,
            values,
            changed: 0,
        }
    }

    /// The type whose layout these values follow.
    #[must_use]
    pub const fn kind(&self) -> EntityType {
        self.kind
    }

    /// How many fields this entity carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether this entity carries no fields at all, which nothing in the game does.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// What a field holds, or nothing where this entity type has no such field.
    #[must_use]
    pub fn get<T: DataField>(&self, field: Field<T>) -> Option<&T> {
        self.values
            .get(field.index() as usize)
            .and_then(T::from_value)
    }

    /// Writes a field, marking it to be sent if it is not what was already there.
    ///
    /// A field this entity type does not carry is not written. That is a mistake in the caller
    /// rather than something a client can cause, so it trips an assertion in a debug build and is
    /// ignored in a release one, where dropping one value beats dropping the connection.
    pub fn set<T: DataField>(&mut self, field: Field<T>, value: T) {
        let index = field.index() as usize;
        let Some(slot) = self.values.get_mut(index) else {
            debug_assert!(
                false,
                "{:?} carries no field {index}, only {}",
                self.kind,
                self.values.len()
            );
            return;
        };

        // Three floats are three floats; which of the two shapes they are is the field's business,
        // not the caller's, and writing the field's own shape keeps a rewrite from reading as a
        // change.
        let mut written = value.into_value();
        if let (DataValue::Vector3(_), DataValue::Rotations(axes)) = (&*slot, &written) {
            written = DataValue::Vector3(*axes);
        }

        if *slot == written {
            return;
        }
        *slot = written;
        self.changed |= 1 << index;
    }

    /// Whether anything has changed since it was last sent.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.changed != 0
    }

    /// Forgets what changed, once it has been sent.
    pub fn take_changes(&mut self) {
        self.changed = 0;
    }

    /// The values that changed, each with the place it sits in the server's own terms.
    pub fn changes(&self) -> impl Iterator<Item = (u8, &DataValue)> {
        self.everything()
            .filter(|(index, _)| self.changed & (1 << index) != 0)
    }

    /// Every value there is to say, each with the place it sits in the server's own terms.
    ///
    /// Vanilla leaves out anything still at its default. Sending it costs a few bytes once, when
    /// an entity comes into view, and saves keeping a second copy of every default to compare
    /// against.
    pub fn everything(&self) -> impl Iterator<Item = (u8, &DataValue)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| Some((u8::try_from(index).ok()?, value)))
    }

    /// Where a value sits and what number its kind travels as, for a client speaking `version`.
    #[must_use]
    pub fn place_for(&self, index: u8, version: ProtocolVersion) -> Option<(u8, u8)> {
        place_for(self.kind, index, version)
    }

    /// What kind of value sits at a field, whatever version is being written to.
    #[must_use]
    pub fn serializer_at(&self, index: u8) -> Option<Serializer> {
        Some(LAYOUTS[self.kind as usize].get(index as usize)?.serializer)
    }

    /// Whether one of the bits of the shared flags byte is set.
    #[must_use]
    pub fn flag(&self, flag: EntityFlag) -> bool {
        self.get(fields::entity::SHARED_FLAGS)
            .is_some_and(|flags| flags & flag as u8 != 0)
    }

    /// Sets or clears one bit of the shared flags byte, leaving the others alone.
    ///
    /// Several unrelated things share this one byte — burning, crouching, sprinting, swimming —
    /// so each has to be written as a bit rather than as a byte, or one of them clears the rest.
    pub fn set_flag(&mut self, flag: EntityFlag, on: bool) {
        let Some(&flags) = self.get(fields::entity::SHARED_FLAGS) else {
            return;
        };
        let bit = flag as u8;
        self.set(
            fields::entity::SHARED_FLAGS,
            if on { flags | bit } else { flags & !bit },
        );
    }

    /// Whether one of the bits of the living flags byte is set.
    #[must_use]
    pub fn living_flag(&self, flag: LivingFlag) -> bool {
        self.get(fields::living_entity::LIVING_ENTITY_FLAGS)
            .is_some_and(|flags| flags & flag as u8 != 0)
    }

    /// Sets or clears one bit of the living flags byte, leaving the others alone.
    pub fn set_living_flag(&mut self, flag: LivingFlag, on: bool) {
        let Some(&flags) = self.get(fields::living_entity::LIVING_ENTITY_FLAGS) else {
            return;
        };
        let bit = flag as u8;
        self.set(
            fields::living_entity::LIVING_ENTITY_FLAGS,
            if on { flags | bit } else { flags & !bit },
        );
    }
}

/// Where a field of `kind` sits for a client speaking `version` and what number the kind of value
/// it holds travels as, or nothing where that version has no place for it.
///
/// Two separate things can be missing. A version may not put the field anywhere, and it may not
/// have that kind of value at all — 1.21 has no place for two thirds of what 26.2 can write. Both
/// mean the field cannot be sent: a number that version keeps something else at would be read as
/// that something else, and since the kind decides how many bytes follow, everything after it in
/// the row would be read at the wrong offset.
#[must_use]
pub fn place_for(kind: EntityType, index: u8, version: ProtocolVersion) -> Option<(u8, u8)> {
    let serializer = LAYOUTS[kind as usize].get(index as usize)?.serializer;
    let place = place_of(kind, index, version);
    (place != ABSENT)
        .then_some(place)
        .zip(serializer.wire_id(version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_entity_starts_where_the_game_starts_it() {
        let zombie = SyncedData::new(EntityType::Zombie);

        // Air supply is the one base field that does not start at nothing.
        assert_eq!(zombie.get(fields::entity::AIR_SUPPLY), Some(&300));
        assert_eq!(zombie.get(fields::entity::POSE), Some(&Pose::Standing));
        assert_eq!(zombie.get(fields::zombie::BABY), Some(&false));
        assert!(!zombie.has_changes());
    }

    #[test]
    fn one_bit_of_the_flags_byte_leaves_the_others_alone() {
        let mut player = SyncedData::new(EntityType::Player);

        player.set_flag(EntityFlag::Sprinting, true);
        player.set_flag(EntityFlag::Crouching, true);
        assert!(player.flag(EntityFlag::Sprinting));
        assert!(player.flag(EntityFlag::Crouching));

        player.set_flag(EntityFlag::Sprinting, false);
        assert!(!player.flag(EntityFlag::Sprinting));
        assert!(
            player.flag(EntityFlag::Crouching),
            "crouching and sprinting share a byte, and writing one is not writing the other"
        );
    }

    #[test]
    fn a_field_is_only_a_change_when_it_changes() {
        let mut zombie = SyncedData::new(EntityType::Zombie);

        zombie.set(fields::zombie::BABY, false);
        assert!(!zombie.has_changes());

        zombie.set(fields::zombie::BABY, true);
        assert!(zombie.has_changes());
        assert_eq!(zombie.get(fields::zombie::BABY), Some(&true));
    }

    #[test]
    fn only_what_changed_is_sent() {
        let mut player = SyncedData::new(EntityType::Player);
        player.set(fields::entity::POSE, Pose::Crouching);
        player.set(fields::living_entity::HEALTH, 12.0);

        let sent: Vec<_> = player
            .changes()
            .filter_map(|(index, _)| player.place_for(index, ProtocolVersion::CURRENT))
            .collect();
        assert_eq!(
            sent,
            vec![
                (
                    6,
                    Serializer::Pose
                        .wire_id(ProtocolVersion::CURRENT)
                        .expect("26.2 has poses")
                ),
                (
                    9,
                    Serializer::Float
                        .wire_id(ProtocolVersion::CURRENT)
                        .expect("and floats")
                ),
            ],
            "a change reaches a client where the game puts it, tagged as the game tags it"
        );

        player.take_changes();
        assert_eq!(player.changes().count(), 0);
    }

    #[test]
    fn everything_is_sent_when_an_entity_comes_into_view() {
        let player = SyncedData::new(EntityType::Player);
        assert_eq!(player.everything().count(), player.len());
    }

    #[test]
    fn a_field_reaches_an_older_client_where_that_version_put_it() {
        let mut slime = SyncedData::new(EntityType::Slime);
        slime.set(fields::abstract_cube_mob::SIZE, 4);

        // 26.2 made a slime an ageable mob, which pushed its size two places down the row.
        let places = |version| -> Vec<u8> {
            slime
                .changes()
                .filter_map(|(index, _)| slime.place_for(index, version))
                .map(|(place, _)| place)
                .collect()
        };
        let here = places(ProtocolVersion::V26_2);
        let there = places(ProtocolVersion::V26_1);
        assert_eq!(here, vec![18]);
        assert_eq!(there, vec![16]);
    }

    #[test]
    fn a_field_an_older_version_never_had_is_left_out() {
        let mut slime = SyncedData::new(EntityType::Slime);
        slime.set(fields::ageable_mob::BABY, true);

        let reaching = |version| {
            slime
                .changes()
                .filter_map(|(index, _)| slime.place_for(index, version))
                .count()
        };
        assert_eq!(reaching(ProtocolVersion::V26_2), 1);
        assert_eq!(
            reaching(ProtocolVersion::V26_1),
            0,
            "26.1 has no such field, and a number it does not know reads as another field"
        );
    }

    #[test]
    fn an_entity_a_version_never_had_is_told_nothing() {
        let cube = SyncedData::new(EntityType::SulfurCube);
        let reaching = |version| {
            cube.everything()
                .filter_map(|(index, _)| cube.place_for(index, version))
                .count()
        };
        assert!(reaching(ProtocolVersion::V26_2) > 0);
        assert_eq!(reaching(ProtocolVersion::V26_1), 0);
    }

    #[test]
    fn every_type_fits_in_the_word_that_tracks_it() {
        for kind in EntityType::all() {
            assert!(
                SyncedData::new(kind).len() <= 64,
                "{kind:?} carries more fields than the change word has bits"
            );
        }
    }

    #[test]
    fn every_field_holds_the_shape_its_slot_says_it_does() {
        // A layout says what kind of value sits in each slot, and the default put there has to be
        // that kind, or a client reads the bytes as something else entirely.
        for kind in EntityType::all() {
            let data = SyncedData::new(kind);
            for (index, value) in data.everything() {
                let serializer = data
                    .serializer_at(index)
                    .expect("every field it holds has a kind");
                let matches = matches!(
                    (serializer, value),
                    (_, DataValue::Raw(_))
                        | (Serializer::Byte, DataValue::Byte(_))
                        | (Serializer::Int, DataValue::Int(_))
                        | (Serializer::Long, DataValue::Long(_))
                        | (Serializer::Float, DataValue::Float(_))
                        | (Serializer::Boolean, DataValue::Boolean(_))
                        | (Serializer::String, DataValue::Text(_))
                        | (Serializer::Component, DataValue::Component(_))
                        | (
                            Serializer::OptionalComponent,
                            DataValue::OptionalComponent(_)
                        )
                        | (Serializer::ItemStack, DataValue::Item(_))
                        | (Serializer::Rotations, DataValue::Rotations(_))
                        | (Serializer::Vector3, DataValue::Vector3(_))
                        | (Serializer::Quaternion, DataValue::Quaternion(_))
                        | (Serializer::BlockPos, DataValue::BlockPos(_))
                        | (Serializer::OptionalBlockPos, DataValue::OptionalBlockPos(_))
                        | (Serializer::Direction, DataValue::Direction(_))
                        | (Serializer::Pose, DataValue::Pose(_))
                        | (Serializer::HumanoidArm, DataValue::Arm(_))
                        | (Serializer::SnifferState, DataValue::SnifferState(_))
                        | (Serializer::ArmadilloState, DataValue::ArmadilloState(_))
                        | (Serializer::CopperGolemState, DataValue::CopperGolemState(_))
                        | (
                            Serializer::WeatheringCopperState,
                            DataValue::WeatheringState(_)
                        )
                );
                assert!(
                    matches,
                    "{kind:?} field {index} is a {serializer:?} holding {value:?}"
                );
            }
        }
    }
}
