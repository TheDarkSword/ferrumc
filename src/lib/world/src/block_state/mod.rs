//! What a block state is, rather than what number it is.
//!
//! A state id is an index into one block's cartesian product of property values, and the blocks
//! partition the id space in order with no gaps. Every question here is therefore arithmetic on the
//! id: which block it belongs to is a binary search over the block bases, reading a property is a
//! division, and changing one is a subtraction and an addition.
//!
//! The tables come from the vanilla block report; see `scripts/build_block_states.py`.

mod generated;

use crate::block_state_id::BlockStateId;
pub use generated::Property;
use generated::{BLOCKS, PROPERTY_VALUES};

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

    /// What this state holds for a property, or `None` where its block has no such property.
    #[must_use]
    pub fn get(self, property: Property) -> Option<&'static str> {
        let (_, digit) = digit(self, property)?;
        Some(digit.values[digit.index as usize])
    }

    /// The same block with one property changed, or `None` where the block has no such property or
    /// the property does not take that value.
    #[must_use]
    pub fn with(self, property: Property, value: &str) -> Option<Self> {
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
            .filter_map(move |property| Some((property, self.get(property)?)))
    }
}

#[cfg(test)]
mod tests {
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
                    state.with(property, value),
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

        for value in ["north", "south", "east", "west"] {
            let turned = state.with(Property::Facing, value).expect("stairs face");
            assert_eq!(turned.get(Property::Facing), Some(value));
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
        assert_eq!(state.get(Property::Facing), None);
        assert_eq!(state.with(Property::Facing, "north"), None);

        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        assert_eq!(stairs.default_state().with(Property::Facing, "up"), None);
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
                    state.get(property),
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

    /// A stair faces four ways and a piston six, under the same property name.
    #[test]
    fn the_same_property_can_take_different_values() {
        let stairs = BlockId::from_name("minecraft:oak_stairs").expect("oak stairs exist");
        let piston = BlockId::from_name("minecraft:piston").expect("pistons exist");
        assert_eq!(stairs.default_state().with(Property::Facing, "up"), None);
        assert!(piston
            .default_state()
            .with(Property::Facing, "up")
            .is_some());
    }
}
