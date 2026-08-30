//! What a block state is, rather than what number it is.
//!
//! A state id is an index into one block's cartesian product of property values, and the blocks
//! partition the id space in order with no gaps. Every question here is therefore arithmetic on the
//! id: which block it belongs to is a binary search over the block bases, reading a property is a
//! division, and changing one is a subtraction and an addition.
//!
//! The tables come from the vanilla block report; see `scripts/build_block_states.py`.

pub mod generated;

use crate::block_state_id::BlockStateId;
pub use generated::*;

use std::marker::PhantomData;

/// A value a property can hold.
///
/// Properties travel as strings in block state ids and commands, but nothing else should have to:
/// a facing is a [`generated::Direction`], not the word "north", so a typo is a compile error
/// rather than a state that silently fails to change.
pub trait PropertyValue: Sized + Copy {
    /// How the value is written in a block state id.
    fn name(self) -> &'static str;
    /// The value of this name, if the type has one.
    fn from_name(name: &str) -> Option<Self>;
}

impl PropertyValue for bool {
    fn name(self) -> &'static str {
        if self {
            "true"
        } else {
            "false"
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
}

/// Integer properties run from zero to the largest any block uses.
const NUMBERS: [&str; 26] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24", "25",
];

impl PropertyValue for u8 {
    fn name(self) -> &'static str {
        NUMBERS.get(self as usize).copied().unwrap_or("")
    }

    fn from_name(name: &str) -> Option<Self> {
        name.parse().ok()
    }
}

/// A property together with what its values are.
///
/// The same name means different things on different blocks - a stair's `half` is its top or
/// bottom, a door's is its upper or lower - so the constants in [`properties`] pair each name with
/// the type it carries there. Asking a block for a property of the wrong type finds nothing, the
/// same as asking for one it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockProperty<T> {
    property: Property,
    values: PhantomData<T>,
}

impl<T> BlockProperty<T> {
    #[must_use]
    pub const fn new(property: Property) -> Self {
        Self {
            property,
            values: PhantomData,
        }
    }

    /// The name this property is written under.
    #[must_use]
    pub const fn property(self) -> Property {
        self.property
    }
}

/// A block, without a particular state of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(u16);

impl BlockId {
    fn def(self) -> &'static generated::BlockDef {
        &BLOCKS[self.0 as usize]
    }

    /// The block's name, namespace included.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.def().name
    }

    /// The state a block is placed in when nothing says otherwise.
    #[must_use]
    pub fn default_state(self) -> BlockStateId {
        BlockStateId::new(self.def().default_state)
    }

    /// Every property this block carries, in the order the ids are built from.
    pub fn properties(self) -> impl Iterator<Item = Property> {
        self.def()
            .properties
            .iter()
            .map(|&(values, _)| PROPERTY_VALUES[values as usize].property)
    }

    /// The block of this name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        BLOCKS
            .iter()
            .position(|block| block.name == name)
            .map(|index| Self(index as u16))
    }
}

/// Where a property sits in a state: how far apart two states differing only in it are, and which
/// of its values the state holds.
struct Digit {
    values: &'static [&'static str],
    stride: u32,
    index: u32,
}

fn digit(state: BlockStateId, property: Property) -> Option<(BlockId, Digit)> {
    let block = block_of(state)?;
    let def = block.def();
    let offset = state.raw() - def.base_state;
    let &(values, stride) = def
        .properties
        .iter()
        .find(|&&(values, _)| PROPERTY_VALUES[values as usize].property == property)?;
    let values = PROPERTY_VALUES[values as usize].values;
    Some((
        block,
        Digit {
            values,
            stride,
            index: (offset / stride) % values.len() as u32,
        },
    ))
}

/// The block a state belongs to. `None` only for an id no version of the game defines.
#[must_use]
pub fn block_of(state: BlockStateId) -> Option<BlockId> {
    let id = state.raw();
    let index = match BLOCKS.binary_search_by_key(&id, |block| block.base_state) {
        Ok(exact) => exact,
        // The state sits inside the block before the one that starts after it.
        Err(0) => return None,
        Err(after) => after - 1,
    };
    let block = &BLOCKS[index];
    (id < block.base_state + block.state_count).then_some(BlockId(index as u16))
}

impl BlockStateId {
    /// The block this state is of.
    #[must_use]
    pub fn block(self) -> Option<BlockId> {
        block_of(self)
    }

    /// What this state holds for a property.
    ///
    /// `None` where the block has no property of that name, or where the one it has holds values
    /// of another type.
    #[must_use]
    pub fn get<T: PropertyValue>(self, property: BlockProperty<T>) -> Option<T> {
        T::from_name(self.get_raw(property.property())?)
    }

    /// The same block with one property changed, or `None` where the block has no such property or
    /// does not take that value.
    #[must_use]
    pub fn with<T: PropertyValue>(self, property: BlockProperty<T>, value: T) -> Option<Self> {
        self.with_raw(property.property(), value.name())
    }

    /// What this state holds for a property, as it is written. Prefer [`Self::get`], which names a
    /// type; this is for the places that only have a name, such as commands.
    #[must_use]
    pub fn get_raw(self, property: Property) -> Option<&'static str> {
        let (_, digit) = digit(self, property)?;
        Some(digit.values[digit.index as usize])
    }

    /// The same block with one property changed, written as a name. Prefer [`Self::with`].
    #[must_use]
    pub fn with_raw(self, property: Property, value: &str) -> Option<Self> {
        let (_, digit) = digit(self, property)?;
        let wanted = digit.values.iter().position(|&known| known == value)? as u32;
        let raw = self.raw() - digit.index * digit.stride + wanted * digit.stride;
        Some(Self::new(raw))
    }

    /// Every property of this state and what it holds.
    pub fn properties(self) -> impl Iterator<Item = (Property, &'static str)> {
        let block = self.block();
        block
            .into_iter()
            .flat_map(move |block| block.properties())
            .filter_map(move |property| Some((property, self.get_raw(property)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::generated::{Direction, Half, SlabType};
    use super::*;

    /// The whole point: every id in the space resolves to a block, and reading its properties back
    /// and setting them to what they already were lands on the same id. A wrong stride anywhere
    /// shows up here.
    #[test]
    fn every_state_round_trips_through_its_properties() {
        let last = BLOCKS
            .last()
            .map(|block| block.base_state + block.state_count)
            .expect("blocks exist");

        for raw in 0..last {
            let state = BlockStateId::new(raw);
            let block = state.block().expect("every id belongs to a block");
            for (property, value) in state.properties() {
                assert_eq!(
                    state.with_raw(property, value),
                    Some(state),
                    "{} state {raw}: setting {} to what it already is moved it",
                    block.name(),
                    property.name()
                );
            }
        }
    }

    /// Setting a property has to reach every state of the block and nothing outside it.
    #[test]
    fn setting_a_property_stays_within_the_block() {
        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        let state = stairs.default_state();
        let def = stairs.def();

        for value in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            let turned = state.with(properties::FACING, value).expect("stairs face");
            assert_eq!(turned.get(properties::FACING), Some(value));
            assert_eq!(turned.block(), Some(stairs));
            assert!(turned.raw() >= def.base_state);
            assert!(turned.raw() < def.base_state + def.state_count);
        }
    }

    /// A property the block does not have is not silently accepted, and neither is a value the
    /// property does not take.
    #[test]
    fn a_property_a_block_lacks_reads_as_nothing() {
        let stone = BlockId::from_name("minecraft:stone").expect("stone exists");
        let state = stone.default_state();
        assert_eq!(state.get(properties::FACING), None);
        assert_eq!(state.with(properties::FACING, Direction::North), None);

        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        assert_eq!(
            stairs
                .default_state()
                .with(properties::FACING, Direction::Up),
            None
        );
    }

    /// The generated tables and `blockstates.json` are two independent renderings of the same
    /// report, read by different parts of the server: these tables answer questions about a state,
    /// while the json is what the `block!` macro and the world importer resolve names through.
    ///
    /// They have to agree, and the reason to check is that they have not always: the json spent a
    /// while holding 1.21.5 ids while the server spoke 26.2, which no test noticed because nothing
    /// compared it to anything.
    #[test]
    fn the_tables_and_the_json_describe_the_same_states() {
        use crate::block_state_id::ID2BLOCK;

        let last = BLOCKS
            .last()
            .map(|block| block.base_state + block.state_count)
            .expect("blocks exist");
        assert_eq!(
            ID2BLOCK.len() as u32,
            last,
            "the two disagree about how many states there are"
        );

        for raw in 0..last {
            let state = BlockStateId::new(raw);
            let json = &ID2BLOCK[raw as usize];
            let block = state.block().expect("every id belongs to a block");
            assert_eq!(block.name(), json.name, "state {raw} is a different block");

            let mut counted = 0;
            for (name, value) in json.properties.iter().flatten() {
                let property =
                    Property::from_name(name).unwrap_or_else(|| panic!("unknown property {name}"));
                assert_eq!(
                    state.get_raw(property),
                    Some(value.as_str()),
                    "state {raw} of {} holds a different {name}",
                    block.name()
                );
                counted += 1;
            }
            assert_eq!(
                state.properties().count(),
                counted,
                "state {raw} of {} has a different number of properties",
                block.name()
            );
        }
    }

    /// The same name carries different types on different blocks, which is the whole reason the
    /// constants pair a name with one: a stair's half is its top or bottom, a door's is its upper
    /// or lower, and neither reads as the other.
    #[test]
    fn a_property_of_the_wrong_type_reads_as_nothing() {
        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        let door = BlockId::from_name("minecraft:oak_door").expect("oak doors exist");
        let slab = BlockId::from_name("minecraft:oak_slab").expect("oak slabs exist");

        assert_eq!(
            stairs.default_state().get(properties::HALF),
            Some(Half::Bottom)
        );
        assert_eq!(
            stairs.default_state().get(properties::DOUBLE_BLOCK_HALF),
            None
        );
        assert!(door
            .default_state()
            .get(properties::DOUBLE_BLOCK_HALF)
            .is_some());
        assert_eq!(door.default_state().get(properties::HALF), None);

        // A slab's `type` and a chest's are both written `type` and are not the same thing.
        assert_eq!(
            slab.default_state().get(properties::SLAB_TYPE),
            Some(SlabType::Bottom)
        );
        assert_eq!(slab.default_state().get(properties::CHEST_TYPE), None);
    }

    /// Integer and boolean properties are read as numbers and flags rather than as their spelling.
    #[test]
    fn numbers_and_flags_are_not_strings() {
        let wire = BlockId::from_name("minecraft:redstone_wire").expect("redstone exists");
        let powered = wire
            .default_state()
            .with(properties::POWER, 9)
            .expect("wire carries power");
        assert_eq!(powered.get(properties::POWER), Some(9));
        assert_eq!(powered.get_raw(Property::Power), Some("9"));
        // Fifteen is the most a wire carries, so sixteen is not a state it has.
        assert_eq!(powered.with(properties::POWER, 16), None);

        let slab = BlockId::from_name("minecraft:oak_slab").expect("oak slabs exist");
        let wet = slab
            .default_state()
            .with(properties::WATERLOGGED, true)
            .expect("slabs waterlog");
        assert_eq!(wet.get(properties::WATERLOGGED), Some(true));
    }

    /// A stair faces four ways and a piston six, under the same property name.
    #[test]
    fn the_same_property_can_take_different_values() {
        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        let piston = BlockId::from_name("minecraft:piston").expect("pistons exist");
        assert_eq!(
            stairs
                .default_state()
                .with(properties::FACING, Direction::Up),
            None
        );
        assert!(piston
            .default_state()
            .with(properties::FACING, Direction::Up)
            .is_some());
    }
}
