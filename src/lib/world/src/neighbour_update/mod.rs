//! Telling blocks that something around them changed.
//!
//! Placing or breaking a block sets off a chain: its neighbours are told, they may change, and
//! theirs are told in turn. The order is observable — most of redstone's character comes from it —
//! so the two orders vanilla uses are kept exactly, and they are not the same order:
//!
//! - neighbours are *told* west, east, down, up, north, south;
//! - neighbours *recompute their own state* west, east, north, south, down, up.
//!
//! The chain is walked with a stack rather than by recursion. Vanilla has both and uses the queued
//! one on the server, because a large contraption is deep enough to exhaust the stack.

use crate::block_behaviour::{behaviour_at, BlockWorld, NeighbourChanged};
use crate::block_state::{BlockId, Direction};
use crate::block_state_id::BlockStateId;
use crate::pos::BlockPos;
use tracing::error;

/// The order neighbours are told in.
pub const UPDATE_ORDER: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
];

/// The order neighbours recompute their own state in, which is not the same.
pub const UPDATE_SHAPE_ORDER: [Direction; 6] = [
    Direction::West,
    Direction::East,
    Direction::North,
    Direction::South,
    Direction::Down,
    Direction::Up,
];

/// How many updates one chain may make before the rest are dropped. Vanilla's
/// `max-chained-neighbor-updates`, which a server owner can lower.
pub const MAX_CHAINED: usize = 1_000_000;

/// How far a shape update may cascade. Separate from the chain limit and much smaller, because a
/// shape update that keeps producing shape updates is a loop rather than a contraption.
const SHAPE_UPDATE_LIMIT: u32 = 512;

/// One piece of work. A group yields several: telling all six neighbours is one entry that runs six
/// times, which is what lets a new update cut in between two of them the way vanilla does.
enum Pending {
    /// One block is told that `source` changed beside it.
    Told { pos: BlockPos, source: BlockId },
    /// Every neighbour of `pos` is told, in [`UPDATE_ORDER`], except the one it came from.
    TellAround {
        pos: BlockPos,
        source: BlockId,
        skip: Option<Direction>,
        next: usize,
    },
    /// One block recomputes its own state because the neighbour `towards` it changed.
    Reshape {
        pos: BlockPos,
        towards: Direction,
        neighbour: BlockStateId,
        limit: u32,
    },
}

/// Walks the chain of updates a change sets off.
pub struct NeighbourUpdater {
    stack: Vec<Pending>,
    added: Vec<Pending>,
    count: usize,
    max_chained: usize,
    running: bool,
}

impl Default for NeighbourUpdater {
    fn default() -> Self {
        Self::new(MAX_CHAINED)
    }
}

impl NeighbourUpdater {
    #[must_use]
    pub fn new(max_chained: usize) -> Self {
        Self {
            stack: Vec::new(),
            added: Vec::new(),
            count: 0,
            max_chained,
            running: false,
        }
    }

    /// A block changed at `pos`: tell everything around it, and let those recompute their shapes.
    ///
    /// This is what vanilla's ordinary `setBlock` does, and what placing or breaking a block wants.
    pub fn block_changed(
        &mut self,
        world: &mut dyn BlockWorld,
        pos: BlockPos,
        state: BlockStateId,
    ) {
        self.update_shapes_around(world, pos, state);
        if let Some(block) = state.block() {
            self.tell_neighbours(world, pos, block, None);
        }
    }

    /// Tells one block that `source` changed beside it.
    ///
    /// Used where a change reaches a particular block rather than everything around a position,
    /// which is how redstone carries a signal along a line.
    pub fn tell_neighbour(&mut self, world: &mut dyn BlockWorld, pos: BlockPos, source: BlockId) {
        self.add_and_run(world, Pending::Told { pos, source });
    }

    /// Tells every neighbour of `pos` that `source` changed, except the one in `skip`.
    pub fn tell_neighbours(
        &mut self,
        world: &mut dyn BlockWorld,
        pos: BlockPos,
        source: BlockId,
        skip: Option<Direction>,
    ) {
        self.add_and_run(
            world,
            Pending::TellAround {
                pos,
                source,
                skip,
                next: 0,
            },
        );
    }

    /// Lets every neighbour of `pos` recompute its own state against the one now there.
    pub fn update_shapes_around(
        &mut self,
        world: &mut dyn BlockWorld,
        pos: BlockPos,
        state: BlockStateId,
    ) {
        for direction in UPDATE_SHAPE_ORDER {
            self.add_and_run(
                world,
                Pending::Reshape {
                    pos: pos.relative(direction),
                    towards: direction.opposite(),
                    neighbour: state,
                    limit: SHAPE_UPDATE_LIMIT,
                },
            );
        }
    }

    /// Takes one more piece of work, or drops it if this chain has gone on long enough.
    ///
    /// Everything is counted, including the work an update produces while running: the limit is
    /// there for a chain that feeds itself, and only counting what starts one would never catch it.
    fn add(&mut self, update: Pending) {
        let too_many = self.count >= self.max_chained;
        self.count += 1;
        if too_many {
            if self.count - 1 == self.max_chained {
                error!("too many chained neighbour updates; the rest of this chain is dropped");
            }
            return;
        }
        if self.running {
            self.added.push(update);
        } else {
            self.stack.push(update);
        }
    }

    fn add_and_run(&mut self, world: &mut dyn BlockWorld, update: Pending) {
        let running_already = self.running;
        self.add(update);
        if !running_already {
            self.run(world);
        }
    }

    fn run(&mut self, world: &mut dyn BlockWorld) {
        self.running = true;
        while !self.stack.is_empty() || !self.added.is_empty() {
            // Whatever was added while the last group ran goes on top, oldest first, so it runs
            // before the group that produced it gets another turn.
            while let Some(update) = self.added.pop() {
                self.stack.push(update);
            }

            let Some(mut current) = self.stack.pop() else {
                break;
            };
            loop {
                if !self.run_next(world, &mut current) {
                    break;
                }
                if !self.added.is_empty() {
                    // Something cut in; this group waits below it.
                    self.stack.push(current);
                    break;
                }
            }
        }

        self.stack.clear();
        self.added.clear();
        self.count = 0;
        self.running = false;
    }

    /// Does one step of a group and says whether the group has more to do.
    fn run_next(&mut self, world: &mut dyn BlockWorld, update: &mut Pending) -> bool {
        match update {
            Pending::Told { pos, source } => {
                let state = world.block_at(*pos);
                if let Some(behaviour) = behaviour_at(state) {
                    let mut ctx = NeighbourChanged {
                        world,
                        pos: *pos,
                        source: *source,
                    };
                    behaviour.neighbour_changed(state, &mut ctx);
                }
                false
            }
            Pending::TellAround {
                pos,
                source,
                skip,
                next,
            } => {
                while *next < UPDATE_ORDER.len() && Some(UPDATE_ORDER[*next]) == *skip {
                    *next += 1;
                }
                let Some(&direction) = UPDATE_ORDER.get(*next) else {
                    return false;
                };
                *next += 1;

                let neighbour_pos = pos.relative(direction);
                let state = world.block_at(neighbour_pos);
                if let Some(behaviour) = behaviour_at(state) {
                    let mut ctx = NeighbourChanged {
                        world,
                        pos: neighbour_pos,
                        source: *source,
                    };
                    behaviour.neighbour_changed(state, &mut ctx);
                }
                *next < UPDATE_ORDER.len()
            }
            Pending::Reshape {
                pos,
                towards,
                neighbour,
                limit,
            } => {
                let current = world.block_at(*pos);
                let Some(behaviour) = behaviour_at(current) else {
                    return false;
                };
                let updated = behaviour.update_shape(current, world, *pos, *towards, *neighbour);
                if updated != current && *limit > 0 {
                    world.set_block(*pos, updated);
                    // What it became now has neighbours of its own to tell, one step further from
                    // the change that started it.
                    for direction in UPDATE_SHAPE_ORDER {
                        self.add(Pending::Reshape {
                            pos: pos.relative(direction),
                            towards: direction.opposite(),
                            neighbour: updated,
                            limit: *limit - 1,
                        });
                    }
                    if let Some(block) = updated.block() {
                        self.add(Pending::TellAround {
                            pos: *pos,
                            source: block,
                            skip: None,
                            next: 0,
                        });
                    }
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_behaviour::BlockWorld;
    use crate::scheduler::TickPriority;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Blocks(HashMap<(i32, i32, i32), BlockStateId>);

    impl Blocks {
        fn air() -> BlockStateId {
            BlockId::from_name("minecraft:air")
                .expect("air exists")
                .default_state()
        }

        fn put(&mut self, pos: BlockPos, name: &str) {
            let block = BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"));
            self.set_block(pos, block.default_state());
        }

        fn name_at(&mut self, pos: BlockPos) -> &'static str {
            self.block_at(pos)
                .block()
                .map_or("<none>", crate::block_state::BlockId::name)
        }
    }

    impl BlockWorld for Blocks {
        fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
            self.0
                .get(&(pos.pos.x, pos.pos.y, pos.pos.z))
                .copied()
                .unwrap_or_else(Self::air)
        }

        fn set_block(&mut self, pos: BlockPos, state: BlockStateId) {
            self.0.insert((pos.pos.x, pos.pos.y, pos.pos.z), state);
        }

        fn schedule_tick(
            &mut self,
            _pos: BlockPos,
            _kind: crate::scheduler::TickKind,
            _delay: u64,
            _priority: TickPriority,
        ) {
        }
    }

    /// The case the phase set out to get right: take away what a torch stands on and the torch
    /// goes with it.
    #[test]
    fn a_torch_pops_when_its_support_goes() {
        let mut world = Blocks::default();
        let floor = BlockPos::of(0, 63, 0);
        let torch = BlockPos::of(0, 64, 0);
        world.put(floor, "minecraft:stone");
        world.put(torch, "minecraft:torch");

        // Break the floor.
        let air = Blocks::air();
        world.set_block(floor, air);
        NeighbourUpdater::default().block_changed(&mut world, floor, air);

        assert_eq!(
            world.name_at(torch),
            "minecraft:air",
            "the torch should have popped"
        );
    }

    /// A torch on something that only holds its centre stays: a fence post is not a full block.
    #[test]
    fn a_torch_on_a_fence_stays() {
        let mut world = Blocks::default();
        let fence = BlockPos::of(0, 63, 0);
        let torch = BlockPos::of(0, 64, 0);
        world.put(fence, "minecraft:oak_fence");
        world.put(torch, "minecraft:torch");

        let state = world.block_at(fence);
        NeighbourUpdater::default().block_changed(&mut world, fence, state);

        assert_eq!(world.name_at(torch), "minecraft:torch");
    }

    /// One block can be told on its own, rather than everything around a position.
    #[test]
    fn a_single_neighbour_can_be_told() {
        let mut world = Blocks::default();
        let torch = BlockPos::of(0, 64, 0);
        world.put(torch, "minecraft:torch");
        let stone = BlockId::from_name("minecraft:stone").expect("stone exists");

        // Nothing under it, but being told is not the same as being asked to recheck its shape:
        // a torch only goes when what is under it changes.
        NeighbourUpdater::default().tell_neighbour(&mut world, torch, stone);
        assert_eq!(world.name_at(torch), "minecraft:torch");
    }

    /// A change beside a torch is not a change under it.
    #[test]
    fn a_torch_ignores_what_happens_beside_it() {
        let mut world = Blocks::default();
        world.put(BlockPos::of(0, 63, 0), "minecraft:stone");
        let torch = BlockPos::of(0, 64, 0);
        world.put(torch, "minecraft:torch");

        let beside = BlockPos::of(1, 64, 0);
        world.put(beside, "minecraft:stone");
        let state = world.block_at(beside);
        NeighbourUpdater::default().block_changed(&mut world, beside, state);

        assert_eq!(world.name_at(torch), "minecraft:torch");
    }

    /// A chain that would go on for ever stops at the limit rather than running out of memory or
    /// stack. A column of torches on one block is exactly that shape: each one popping takes the
    /// next with it.
    #[test]
    fn a_chain_stops_at_the_limit() {
        let mut world = Blocks::default();
        world.put(BlockPos::of(0, 0, 0), "minecraft:stone");
        for y in 1..40 {
            world.put(BlockPos::of(0, y, 0), "minecraft:torch");
        }

        let air = Blocks::air();
        world.set_block(BlockPos::of(0, 0, 0), air);
        // A budget far below what the whole column needs.
        let mut updater = NeighbourUpdater::new(8);
        updater.block_changed(&mut world, BlockPos::of(0, 0, 0), air);

        // The bottom of the column went; the top is still standing, and nothing hung or overflowed.
        assert_eq!(world.name_at(BlockPos::of(0, 1, 0)), "minecraft:air");
        assert_eq!(world.name_at(BlockPos::of(0, 39, 0)), "minecraft:torch");
    }

    /// With room to run, the whole column goes.
    #[test]
    fn a_chain_runs_to_the_end_when_it_fits() {
        let mut world = Blocks::default();
        world.put(BlockPos::of(0, 0, 0), "minecraft:stone");
        for y in 1..40 {
            world.put(BlockPos::of(0, y, 0), "minecraft:torch");
        }

        let air = Blocks::air();
        world.set_block(BlockPos::of(0, 0, 0), air);
        NeighbourUpdater::default().block_changed(&mut world, BlockPos::of(0, 0, 0), air);

        for y in 1..40 {
            assert_eq!(
                world.name_at(BlockPos::of(0, y, 0)),
                "minecraft:air",
                "the torch at {y} should have popped"
            );
        }
    }
}
