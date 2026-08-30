//! Scheduled block ticks.
//!
//! Some block behaviour does not happen instantly: fluids spread a few ticks after being
//! disturbed, and other mechanics (planned for later) tick on their own cadence. This module
//! provides the bookkeeping for "do something at this block position N ticks from now".
//!
//! # Design
//!
//! Scheduled ticks are partitioned **per chunk**. Each [`ChunkPos`] owns its own queue of pending
//! ticks. This partitioning is deliberate: it lets a future parallel fluid stage process disjoint
//! sets of chunks on separate threads without contending over a single global structure. The
//! scheduler itself does no locking; callers decide how to share it (for example, behind the
//! existing chunk cache or a dedicated resource).
//!
//! Scheduling is **idempotent per `(position, kind)` within a tick bucket**: scheduling the same
//! block for the same work at an already-pending time does not create duplicate entries. This
//! mirrors vanilla, where a block cannot have two identical pending ticks, and keeps the queues
//! from growing without bound when many neighbours re-trigger the same block.

use crate::pos::{BlockPos, ChunkPos};
use std::collections::{HashMap, HashSet};

/// The category of work a scheduled tick performs.
///
/// Kept separate from the fluid module so the scheduler has no dependency on fluid specifics;
/// new tick kinds (redstone, crops, random block ticks) can be added here without touching the
/// queue machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickKind {
    /// A fluid block should re-evaluate its spread.
    FluidSpread,
    /// A block asked to be given a turn later: a redstone torch burning out, a sapling growing, a
    /// piston finishing its push.
    Block,
}

/// Which ticks of one game tick go first.
///
/// Redstone reads as it does because the order within a tick is observable: a repeater that
/// updates before its neighbour behaves differently from one that updates after. Vanilla's values
/// are kept so that ordering carries across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TickPriority {
    ExtremelyHigh,
    VeryHigh,
    High,
    #[default]
    Normal,
    Low,
    VeryLow,
    ExtremelyLow,
}

impl TickPriority {
    /// The number this priority is written as, which is what a saved tick carries.
    #[must_use]
    pub const fn value(self) -> i8 {
        match self {
            Self::ExtremelyHigh => -3,
            Self::VeryHigh => -2,
            Self::High => -1,
            Self::Normal => 0,
            Self::Low => 1,
            Self::VeryLow => 2,
            Self::ExtremelyLow => 3,
        }
    }

    /// The priority a number means, clamped to the range rather than refused.
    #[must_use]
    pub const fn from_value(value: i8) -> Self {
        match value {
            i8::MIN..=-3 => Self::ExtremelyHigh,
            -2 => Self::VeryHigh,
            -1 => Self::High,
            0 => Self::Normal,
            1 => Self::Low,
            2 => Self::VeryLow,
            3..=i8::MAX => Self::ExtremelyLow,
        }
    }
}

/// A pending tick as it is written to disk.
///
/// The delay is counted from whatever tick the world was saved on rather than being absolute, so a
/// world reloaded later resumes where it left off instead of firing everything at once. The
/// sub-tick order is not kept: it only orders ticks against others of the same age, and after a
/// reload there are none.
///
/// The field names are vanilla's, so a chunk written here can be read there and the other way
/// about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedTick {
    pub pos: BlockPos,
    pub kind: TickKind,
    /// Ticks from the save to when it is due. Negative would mean overdue, which is written as
    /// zero.
    pub delay: u32,
    pub priority: TickPriority,
}

/// A single pending block update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScheduledTick {
    pub pos: BlockPos,
    pub kind: TickKind,
    /// The absolute tick number (from `TickCounter`) at which this should run.
    pub target_tick: u64,
    pub priority: TickPriority,
    /// Where this tick came in the order they were scheduled, which settles ties between two of
    /// the same priority. Without it the order within a tick would depend on the map's iteration.
    pub sub_tick_order: u64,
}

impl ScheduledTick {
    /// The order ticks run in: by the tick they are due, then priority, then age.
    fn drain_order(&self) -> (u64, i8, u64) {
        (self.target_tick, self.priority.value(), self.sub_tick_order)
    }
}

/// Per-chunk queue of pending ticks.
///
/// Entries are kept in a flat vector and filtered by due time on drain. A dedup set guards against
/// inserting an identical `(pos, kind, target_tick)` more than once. For the small per-chunk
/// populations expected from fluid spreading this is cheaper and simpler than a binary heap; the
/// structure can be upgraded later if profiling shows it matters.
#[derive(Debug, Default)]
struct ChunkTickQueue {
    pending: Vec<ScheduledTick>,
    /// One pending tick per block and kind, as vanilla has it: a block that is already waiting for
    /// its turn does not get a second one.
    seen: HashSet<(BlockPos, TickKind)>,
}

impl ChunkTickQueue {
    fn schedule(&mut self, tick: ScheduledTick) -> bool {
        let key = (tick.pos, tick.kind);
        if self.seen.insert(key) {
            self.pending.push(tick);
            true
        } else {
            false
        }
    }

    fn drain_due(&mut self, current_tick: u64, kind: TickKind, out: &mut Vec<ScheduledTick>) {
        // Compact in place: move due ticks into `out` and keep the rest. `retain` shifts the kept
        // elements down without allocating, where the previous implementation allocated a fresh
        // full-size buffer on every call — for every chunk, every tick, even when nothing was due.
        // `seen` is borrowed separately from `pending` so the closure does not capture all of `self`.
        let seen = &mut self.seen;
        self.pending.retain(|tick| {
            if tick.kind == kind && tick.target_tick <= current_tick {
                seen.remove(&(tick.pos, tick.kind));
                out.push(*tick);
                false
            } else {
                true
            }
        });
    }

    /// Drains at most `budget` due ticks into `out`, leaving any remaining due ticks queued (they
    /// stay due and will be returned by a later drain). Returns how many were drained.
    fn drain_due_capped(
        &mut self,
        current_tick: u64,
        kind: TickKind,
        out: &mut Vec<ScheduledTick>,
        budget: usize,
    ) -> usize {
        // Compact in place (see `drain_due`). Order is preserved, so once the budget is exhausted
        // every still-due tick is simply kept and picked up by a later drain.
        let seen = &mut self.seen;
        let mut taken = 0;
        self.pending.retain(|tick| {
            if taken < budget && tick.kind == kind && tick.target_tick <= current_tick {
                seen.remove(&(tick.pos, tick.kind));
                out.push(*tick);
                taken += 1;
                false
            } else {
                true
            }
        });
        taken
    }

    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// Scheduler holding per-chunk tick queues.
///
/// This is a plain data structure with no internal synchronization. It is intended to live in a
/// single owner (e.g. an ECS resource) and be advanced once per game tick.
#[derive(Debug, Default)]
pub struct BlockTickScheduler {
    chunks: HashMap<ChunkPos, ChunkTickQueue>,
    /// Counts every tick ever scheduled, so two of the same priority run in the order they were
    /// asked for.
    next_sub_tick_order: u64,
}

impl BlockTickScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedules `kind` work at `pos` to run `delay` ticks after `current_tick`.
    ///
    /// A `delay` of 0 schedules the work for the current tick (it will be returned by the next
    /// [`drain_due`](Self::drain_due) call for `current_tick`). Returns `true` if a new tick was
    /// added, or `false` if an identical tick was already pending.
    pub fn schedule(
        &mut self,
        pos: BlockPos,
        kind: TickKind,
        current_tick: u64,
        delay: u64,
    ) -> bool {
        self.schedule_with_priority(pos, kind, current_tick, delay, TickPriority::Normal)
    }

    /// Schedules work that has to run before or after other work due on the same tick.
    pub fn schedule_with_priority(
        &mut self,
        pos: BlockPos,
        kind: TickKind,
        current_tick: u64,
        delay: u64,
        priority: TickPriority,
    ) -> bool {
        let target_tick = current_tick.saturating_add(delay);
        let sub_tick_order = self.next_sub_tick_order;
        let added = self
            .chunks
            .entry(pos.chunk())
            .or_default()
            .schedule(ScheduledTick {
                pos,
                kind,
                target_tick,
                priority,
                sub_tick_order,
            });
        if added {
            self.next_sub_tick_order += 1;
        }
        added
    }

    /// Every tick of one kind that is due, in the order it has to run in.
    ///
    /// The order is across the whole world, not per chunk: two redstone components in different
    /// chunks still resolve by priority and then by which was scheduled first. Grouping by chunk
    /// first, as the fluid path does, would make the result depend on the map's iteration order.
    ///
    /// `max_ticks` bounds one game tick's work; anything left stays due and is taken next tick.
    /// Zero means no bound.
    pub fn drain_ordered(
        &mut self,
        current_tick: u64,
        max_ticks: usize,
        kind: TickKind,
    ) -> Vec<ScheduledTick> {
        let mut due: Vec<ScheduledTick> = Vec::new();
        for queue in self.chunks.values_mut() {
            queue.pending.retain(|tick| {
                if tick.kind == kind && tick.target_tick <= current_tick {
                    due.push(*tick);
                    false
                } else {
                    true
                }
            });
        }
        due.sort_unstable_by_key(ScheduledTick::drain_order);

        // Anything over the bound goes back where it came from, still due.
        if max_ticks > 0 && due.len() > max_ticks {
            for tick in due.split_off(max_ticks) {
                self.chunks
                    .entry(tick.pos.chunk())
                    .or_default()
                    .pending
                    .push(tick);
            }
        }
        for tick in &due {
            if let Some(queue) = self.chunks.get_mut(&tick.pos.chunk()) {
                queue.seen.remove(&(tick.pos, tick.kind));
            }
        }
        self.chunks.retain(|_, queue| !queue.is_empty());
        due
    }

    /// Removes and returns every tick due at or before `current_tick`, grouped by chunk.
    ///
    /// Only chunks that actually have due ticks appear in the result. Chunks whose queues become
    /// empty are dropped to keep the map from accumulating idle entries. Grouping by chunk lets the
    /// caller hand each chunk's work to a separate worker.
    pub fn drain_due(
        &mut self,
        current_tick: u64,
        kind: TickKind,
    ) -> Vec<(ChunkPos, Vec<ScheduledTick>)> {
        let mut result = Vec::new();
        let mut emptied = Vec::new();

        for (chunk_pos, queue) in self.chunks.iter_mut() {
            let mut due = Vec::new();
            queue.drain_due(current_tick, kind, &mut due);
            if !due.is_empty() {
                result.push((*chunk_pos, due));
            }
            if queue.is_empty() {
                emptied.push(*chunk_pos);
            }
        }

        for chunk_pos in emptied {
            self.chunks.remove(&chunk_pos);
        }

        result
    }

    /// Like [`drain_due`](Self::drain_due) but drains at most `max_ticks` due ticks in total this
    /// call, leaving any remaining due ticks queued for a later call. This lets the caller bound how
    /// much work a single game tick performs, so a large fluid cascade is spread across several ticks
    /// (settling a little slower) instead of freezing one tick for hundreds of milliseconds.
    ///
    /// Chunks are visited in map order until the budget is exhausted, so a chunk with a huge backlog
    /// can defer later chunks to subsequent ticks; forward progress is still guaranteed because every
    /// remaining tick stays due. `max_ticks == 0` means unbounded (equivalent to `drain_due`).
    pub fn drain_due_capped(
        &mut self,
        current_tick: u64,
        max_ticks: usize,
        kind: TickKind,
    ) -> Vec<(ChunkPos, Vec<ScheduledTick>)> {
        if max_ticks == 0 {
            return self.drain_due(current_tick, kind);
        }
        let mut result = Vec::new();
        let mut emptied = Vec::new();
        let mut budget = max_ticks;

        for (chunk_pos, queue) in self.chunks.iter_mut() {
            if budget > 0 {
                let mut due = Vec::new();
                let taken = queue.drain_due_capped(current_tick, kind, &mut due, budget);
                budget -= taken;
                if !due.is_empty() {
                    result.push((*chunk_pos, due));
                }
            }
            if queue.is_empty() {
                emptied.push(*chunk_pos);
            }
        }

        for chunk_pos in emptied {
            self.chunks.remove(&chunk_pos);
        }

        result
    }

    /// Total number of chunks that currently have pending ticks. Primarily for diagnostics.
    /// Takes everything pending in one chunk, ready to be written with it.
    ///
    /// The chunk is left with nothing, so a chunk being unloaded does not keep ticking.
    pub fn take_chunk(&mut self, chunk: ChunkPos, current_tick: u64) -> Vec<SavedTick> {
        let Some(queue) = self.chunks.remove(&chunk) else {
            return Vec::new();
        };
        queue
            .pending
            .into_iter()
            .map(|tick| SavedTick {
                pos: tick.pos,
                kind: tick.kind,
                delay: tick.target_tick.saturating_sub(current_tick) as u32,
                priority: tick.priority,
            })
            .collect()
    }

    /// Puts back what a chunk was carrying, due the same distance ahead as when it was saved.
    pub fn restore_chunk(&mut self, saved: &[SavedTick], current_tick: u64) {
        for tick in saved {
            self.schedule_with_priority(
                tick.pos,
                tick.kind,
                current_tick,
                u64::from(tick.delay),
                tick.priority,
            );
        }
    }

    pub fn active_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Total number of pending ticks across all chunks. Primarily for diagnostics.
    pub fn pending_count(&self) -> usize {
        self.chunks.values().map(|q| q.pending.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::of(x, y, z)
    }

    #[test]
    fn schedule_then_drain_at_target() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 100, 5);

        // Nothing due before the target tick.
        assert!(sched.drain_due(104, TickKind::FluidSpread).is_empty());
        assert_eq!(sched.pending_count(), 1);

        // Due exactly at the target tick.
        let due = sched.drain_due(105, TickKind::FluidSpread);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].1.len(), 1);
        assert_eq!(due[0].1[0].pos, pos(0, 64, 0));
        assert_eq!(sched.pending_count(), 0);
    }

    #[test]
    fn drain_due_capped_bounds_and_defers() {
        let mut sched = BlockTickScheduler::new();
        // Five ticks all due at tick 1, spread across two chunks so the budget must span chunks.
        for i in 0..5 {
            sched.schedule(pos(i, 64, 0), TickKind::FluidSpread, 0, 1);
        }
        for i in 0..5 {
            sched.schedule(pos(100 + i, 64, 0), TickKind::FluidSpread, 0, 1);
        }
        assert_eq!(sched.pending_count(), 10);

        // A capped drain returns at most the budget, leaving the rest still due.
        let first: usize = sched
            .drain_due_capped(1, 4, TickKind::FluidSpread)
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        assert_eq!(first, 4, "capped drain must not exceed the budget");
        assert_eq!(sched.pending_count(), 6, "the rest stay queued and due");

        let second: usize = sched
            .drain_due_capped(1, 4, TickKind::FluidSpread)
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        assert_eq!(second, 4);
        let third: usize = sched
            .drain_due_capped(1, 4, TickKind::FluidSpread)
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        assert_eq!(third, 2, "only the remaining due ticks are returned");
        assert_eq!(sched.pending_count(), 0);

        // A budget of 0 means unbounded.
        sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 0, 1);
        sched.schedule(pos(1, 64, 0), TickKind::FluidSpread, 0, 1);
        let all: usize = sched
            .drain_due_capped(1, 0, TickKind::FluidSpread)
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        assert_eq!(all, 2);
    }

    #[test]
    fn dedup_identical_ticks() {
        let mut sched = BlockTickScheduler::new();
        let first = sched.schedule(pos(1, 64, 1), TickKind::FluidSpread, 0, 5);
        let second = sched.schedule(pos(1, 64, 1), TickKind::FluidSpread, 0, 5);
        assert!(first);
        assert!(!second, "identical pending tick should be deduplicated");
        assert_eq!(sched.pending_count(), 1);
    }

    #[test]
    /// A block waiting for its turn does not get a second one, whatever tick the second would be
    /// due on. This is vanilla's rule, and it is what stops a block whose neighbours all update at
    /// once from queueing one tick per neighbour.
    fn a_block_has_at_most_one_pending_tick_per_kind() {
        let mut sched = BlockTickScheduler::new();
        assert!(sched.schedule(pos(1, 64, 1), TickKind::FluidSpread, 0, 5));
        assert!(!sched.schedule(pos(1, 64, 1), TickKind::FluidSpread, 0, 6));
        assert_eq!(sched.pending_count(), 1);

        // A different kind of work at the same block is a different tick.
        assert!(sched.schedule(pos(1, 64, 1), TickKind::Block, 0, 6));
        assert_eq!(sched.pending_count(), 2);

        // And once it has run, the block can be scheduled again.
        sched.drain_due(5, TickKind::FluidSpread);
        assert!(sched.schedule(pos(1, 64, 1), TickKind::FluidSpread, 5, 5));
    }

    /// Ticks due on the same game tick run by priority, and ties by which was scheduled first.
    /// Redstone reads as it does because this order is observable.
    #[test]
    fn ticks_run_in_priority_then_scheduling_order() {
        let mut sched = BlockTickScheduler::new();
        // Scheduled last but highest priority, and in a different chunk, so a per-chunk order
        // would put it elsewhere.
        sched.schedule_with_priority(pos(0, 64, 0), TickKind::Block, 0, 1, TickPriority::Normal);
        sched.schedule_with_priority(pos(1, 64, 0), TickKind::Block, 0, 1, TickPriority::Normal);
        sched.schedule_with_priority(pos(64, 64, 64), TickKind::Block, 0, 1, TickPriority::High);

        let order: Vec<_> = sched
            .drain_ordered(1, 0, TickKind::Block)
            .into_iter()
            .map(|tick| tick.pos)
            .collect();
        assert_eq!(order, vec![pos(64, 64, 64), pos(0, 64, 0), pos(1, 64, 0)]);
    }

    /// A tick due earlier runs before one due later, whatever their priorities.
    #[test]
    fn the_tick_they_are_due_on_comes_first() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule_with_priority(
            pos(0, 64, 0),
            TickKind::Block,
            0,
            2,
            TickPriority::ExtremelyHigh,
        );
        sched.schedule_with_priority(
            pos(1, 64, 0),
            TickKind::Block,
            0,
            1,
            TickPriority::ExtremelyLow,
        );

        let order: Vec<_> = sched
            .drain_ordered(5, 0, TickKind::Block)
            .into_iter()
            .map(|tick| tick.pos)
            .collect();
        assert_eq!(order, vec![pos(1, 64, 0), pos(0, 64, 0)]);
    }

    /// One kind of work does not take another's ticks: fluids and blocks are drained separately,
    /// as they are in vanilla.
    #[test]
    fn draining_one_kind_leaves_the_others() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule(pos(0, 64, 0), TickKind::Block, 0, 1);
        sched.schedule(pos(0, 64, 1), TickKind::FluidSpread, 0, 1);

        assert_eq!(sched.drain_ordered(1, 0, TickKind::Block).len(), 1);
        assert_eq!(sched.pending_count(), 1);
        assert_eq!(sched.drain_ordered(1, 0, TickKind::FluidSpread).len(), 1);
        assert_eq!(sched.pending_count(), 0);
    }

    /// A world that stops and starts again has to resume where it left off, not fire everything
    /// at once. What is saved is the wait, not the tick number.
    #[test]
    fn saved_ticks_keep_their_remaining_wait() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule_with_priority(pos(0, 64, 0), TickKind::Block, 100, 7, TickPriority::High);
        sched.schedule(pos(1, 64, 0), TickKind::FluidSpread, 100, 2);

        let saved = sched.take_chunk(pos(0, 64, 0).chunk(), 100);
        assert_eq!(saved.len(), 2);
        assert_eq!(sched.pending_count(), 0, "the chunk keeps nothing behind");

        let block = saved
            .iter()
            .find(|tick| tick.kind == TickKind::Block)
            .expect("the block tick was saved");
        assert_eq!(block.delay, 7);
        assert_eq!(block.priority, TickPriority::High);

        // Loaded again on a completely different tick, it still has seven to wait.
        sched.restore_chunk(&saved, 5_000);
        assert!(sched.drain_ordered(5_006, 0, TickKind::Block).is_empty());
        let due = sched.drain_ordered(5_007, 0, TickKind::Block);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].priority, TickPriority::High);
    }

    /// A tick that was already overdue when the world was saved is due immediately on return,
    /// rather than wrapping around to never.
    #[test]
    fn an_overdue_tick_stays_overdue() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule(pos(0, 64, 0), TickKind::Block, 0, 1);

        let saved = sched.take_chunk(pos(0, 64, 0).chunk(), 500);
        assert_eq!(saved[0].delay, 0);
        sched.restore_chunk(&saved, 500);
        assert_eq!(sched.drain_ordered(500, 0, TickKind::Block).len(), 1);
    }

    /// What does not fit in a tick's budget stays due rather than being lost.
    #[test]
    fn what_does_not_fit_stays_due() {
        let mut sched = BlockTickScheduler::new();
        for x in 0..5 {
            sched.schedule(pos(x, 64, 0), TickKind::Block, 0, 1);
        }

        let first = sched.drain_ordered(1, 2, TickKind::Block);
        assert_eq!(first.len(), 2);
        assert_eq!(sched.pending_count(), 3);
        assert_eq!(sched.drain_ordered(1, 0, TickKind::Block).len(), 3);
    }

    #[test]
    fn groups_by_chunk() {
        let mut sched = BlockTickScheduler::new();
        // Two positions in different chunks (16 blocks apart horizontally).
        sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 0, 1);
        sched.schedule(pos(32, 64, 0), TickKind::FluidSpread, 0, 1);
        sched.schedule(pos(1, 64, 0), TickKind::FluidSpread, 0, 1); // same chunk as first

        let mut due = sched.drain_due(1, TickKind::FluidSpread);
        due.sort_by_key(|(c, _)| c.x());
        assert_eq!(due.len(), 2, "two distinct chunks should be present");
        // First chunk has two ticks, second has one.
        assert_eq!(due[0].1.len(), 2);
        assert_eq!(due[1].1.len(), 1);
    }

    #[test]
    fn drain_leaves_future_ticks() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 0, 1);
        sched.schedule(pos(0, 65, 0), TickKind::FluidSpread, 0, 10);

        let due = sched.drain_due(1, TickKind::FluidSpread);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].1.len(), 1);
        assert_eq!(sched.pending_count(), 1);

        // Re-scheduling the drained position is allowed again (dedup entry was cleared).
        assert!(sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 1, 1));
    }

    #[test]
    fn empty_chunks_are_pruned() {
        let mut sched = BlockTickScheduler::new();
        sched.schedule(pos(0, 64, 0), TickKind::FluidSpread, 0, 1);
        assert_eq!(sched.active_chunk_count(), 1);
        sched.drain_due(1, TickKind::FluidSpread);
        assert_eq!(sched.active_chunk_count(), 0);
    }
}
