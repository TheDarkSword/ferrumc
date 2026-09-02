//! How long a block takes to break.
//!
//! One division and a handful of multipliers, but the multipliers are where the surprises are. The
//! two worth knowing before reading the rest:
//!
//! - Using the **wrong tool** does not only stop the drop — it makes the block take **more than
//!   three times as long**, because the divisor goes from thirty to a hundred.
//! - Breaking a block **in mid-air** takes five times as long, and **underwater** five times again
//!   unless the player has aqua affinity. The two stack.
//!
//! Nothing here touches the world. What tool is in hand and what the block is are passed in.

use ferrumc_data::generated::items::{DataComponent, Item, ToolImpl};

/// What a tool with no rule for a block is worth, and what a bare hand is worth.
pub const BARE_HANDS: f32 = 1.0;

/// What the progress is divided by when the tool is right for the block.
const RIGHT_TOOL: f32 = 30.0;

/// And when it is not, which is why the wrong tool is more than three times slower.
const WRONG_TOOL: f32 = 100.0;

/// How much slower breaking a block is with nothing underfoot.
const IN_MID_AIR: f32 = 5.0;

/// What each level of haste adds.
const HASTE_PER_LEVEL: f32 = 0.2;

/// What each level of mining fatigue leaves, which falls away very fast.
const FATIGUE_LEAVES: [f32; 4] = [0.3, 0.09, 0.0027, 8.1e-4];

/// What the digger brings to the block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Digger {
    /// What the held tool is worth against this particular block.
    pub tool_speed: f32,
    /// Whether the held tool is the right one for it to drop anything.
    pub right_tool: bool,
    /// What efficiency adds, which is an attribute an enchantment moves.
    pub mining_efficiency: f32,
    /// The level of haste, where the digger has it. One means haste I.
    pub haste: u8,
    /// The level of mining fatigue.
    pub fatigue: u8,
    /// A plain multiplier on everything, which is an attribute.
    pub block_break_speed: f32,
    /// What is left of the speed with the digger's head under water.
    ///
    /// A fifth by default; aqua affinity moves the attribute to one.
    pub submerged_speed: f32,
    pub eyes_in_water: bool,
    pub on_ground: bool,
}

impl Default for Digger {
    /// Someone standing on the ground with nothing in their hands.
    fn default() -> Self {
        Self {
            tool_speed: BARE_HANDS,
            right_tool: false,
            mining_efficiency: 0.0,
            haste: 0,
            fatigue: 0,
            block_break_speed: 1.0,
            submerged_speed: 0.2,
            eyes_in_water: false,
            on_ground: true,
        }
    }
}

impl Digger {
    /// How fast the digger works, before the block is taken into account.
    ///
    /// The order is vanilla's: efficiency is added to the tool's own speed and only where the tool
    /// beats a bare hand, which is why efficiency on a hoe against stone does nothing.
    #[must_use]
    pub fn speed(&self) -> f32 {
        let mut speed = self.tool_speed;
        if speed > BARE_HANDS {
            speed += self.mining_efficiency;
        }
        if self.haste > 0 {
            speed *= 1.0 + f32::from(self.haste) * HASTE_PER_LEVEL;
        }
        if self.fatigue > 0 {
            let at = usize::from(self.fatigue - 1).min(FATIGUE_LEAVES.len() - 1);
            speed *= FATIGUE_LEAVES[at];
        }
        speed *= self.block_break_speed;
        if self.eyes_in_water {
            speed *= self.submerged_speed;
        }
        if !self.on_ground {
            speed /= IN_MID_AIR;
        }
        speed
    }
}

/// How much of a block is broken in one tick, from nothing to one.
///
/// Nothing at all means it never breaks: the block does not yield, or the digger is working too
/// slowly to matter.
#[must_use]
pub fn progress_per_tick(hardness: f32, needs_the_right_tool: bool, digger: &Digger) -> f32 {
    if hardness < 0.0 {
        return 0.0;
    }
    if hardness == 0.0 {
        return 1.0;
    }
    // A block that needs no particular tool is always "right", however it is being hit.
    let right = !needs_the_right_tool || digger.right_tool;
    let divisor = if right { RIGHT_TOOL } else { WRONG_TOOL };
    (digger.speed() / hardness / divisor).max(0.0)
}

/// How many ticks it takes to break, or nothing where it never does.
#[must_use]
pub fn ticks_to_break(hardness: f32, needs_the_right_tool: bool, digger: &Digger) -> Option<u32> {
    let per_tick = progress_per_tick(hardness, needs_the_right_tool, digger);
    if per_tick <= 0.0 {
        return None;
    }
    if per_tick >= 1.0 {
        return Some(0);
    }
    Some((1.0 / per_tick).ceil() as u32)
}

/// What one item is worth against one block, and whether it is the right one for it.
///
/// A tool carries rules, each naming some blocks and giving a speed, whether the block drops, or
/// both. The **first** rule that names the block wins, so their order is the tool's own answer and
/// not something to sort.
///
/// `names` is asked whether a rule's blocks include the one being broken; it is a tag more often
/// than not, so the caller does the asking.
#[must_use]
pub fn tool_against(held: Option<&'static Item>, names: impl Fn(&str) -> bool) -> (f32, bool) {
    let Some(tool) = held.and_then(|item| {
        item.components.iter().find_map(|(id, data)| {
            (*id == DataComponent::Tool)
                .then(|| data.as_any().downcast_ref::<ToolImpl>())
                .flatten()
        })
    }) else {
        return (BARE_HANDS, false);
    };

    let mut speed = None;
    let mut right = None;
    for rule in tool.rules {
        if !names(rule.blocks) {
            continue;
        }
        if speed.is_none() {
            speed = rule.speed;
        }
        if right.is_none() {
            right = rule.correct_for_drops;
        }
        if speed.is_some() && right.is_some() {
            break;
        }
    }
    (
        speed.unwrap_or(tool.default_mining_speed),
        right.unwrap_or(false),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stone's own hardness, which is what the table says.
    const STONE: f32 = 1.5;
    const DIRT: f32 = 0.5;
    const OBSIDIAN: f32 = 50.0;

    fn with_a_pickaxe(speed: f32) -> Digger {
        Digger {
            tool_speed: speed,
            right_tool: true,
            ..Digger::default()
        }
    }

    #[test]
    fn a_block_that_yields_to_nothing_never_breaks() {
        assert_eq!(ticks_to_break(-1.0, false, &Digger::default()), None);
    }

    #[test]
    fn something_of_no_hardness_goes_at_a_touch() {
        assert_eq!(ticks_to_break(0.0, false, &Digger::default()), Some(0));
    }

    /// The numbers a player would recognise: stone with a bare hand is a long seven and a half
    /// seconds, and with a diamond pickaxe a quarter of one.
    #[test]
    fn stone_takes_what_vanilla_says_it_takes() {
        let bare = ticks_to_break(STONE, true, &Digger::default()).expect("it breaks");
        assert_eq!(bare, 150, "seven and a half seconds with a fist");

        let diamond = ticks_to_break(STONE, true, &with_a_pickaxe(8.0)).expect("it breaks");
        assert_eq!(diamond, 6, "under a third of a second");
    }

    /// The part people do not expect: the wrong tool is more than three times slower, not just
    /// dropless.
    #[test]
    fn the_wrong_tool_is_more_than_three_times_slower() {
        let right = ticks_to_break(STONE, true, &with_a_pickaxe(8.0)).expect("it breaks");
        let wrong = ticks_to_break(
            STONE,
            true,
            &Digger {
                tool_speed: 8.0,
                right_tool: false,
                ..Digger::default()
            },
        )
        .expect("it breaks");
        assert!(wrong as f32 / right as f32 > 3.0, "{wrong} against {right}");
    }

    /// For a block that needs no tool there is no such thing as the *wrong* one: the divisor stays
    /// at thirty however it is being hit, so nothing is slowed for using a fist.
    #[test]
    fn dirt_takes_no_penalty_for_the_tool_being_wrong() {
        let fist = ticks_to_break(DIRT, false, &Digger::default());
        let holding_something_useless = ticks_to_break(
            DIRT,
            false,
            &Digger {
                right_tool: false,
                ..Digger::default()
            },
        );
        assert_eq!(fist, holding_something_useless);

        // Which is not the same as saying nothing is faster. A shovel is a shovel.
        let shovel = ticks_to_break(DIRT, false, &with_a_pickaxe(8.0)).expect("it breaks");
        assert!(
            shovel < fist.expect("it breaks"),
            "a shovel should still be quicker than a fist"
        );
    }

    /// The numbers a player would recognise, and the pair the whole distinction rests on: dirt is
    /// quicker with a shovel and comes up all the same without one.
    #[test]
    fn dirt_takes_what_vanilla_says_with_a_shovel_and_without() {
        assert_eq!(
            ticks_to_break(DIRT, false, &Digger::default()),
            Some(15),
            "three quarters of a second with a fist"
        );
        assert_eq!(
            ticks_to_break(DIRT, false, &with_a_pickaxe(8.0)),
            Some(2),
            "a tenth of one with a diamond shovel"
        );
    }

    /// A real shovel, read off the game rather than made up: it is worth eight against anything a
    /// shovel is for, and it is the right tool for it.
    #[test]
    fn a_diamond_shovel_knows_what_it_is_worth_against_dirt() {
        let shovel = Item::from_registry_key("minecraft:diamond_shovel");
        let (speed, right) = tool_against(shovel, |blocks| blocks.contains("shovel"));
        assert_eq!(speed, 8.0);
        assert!(right);
    }

    /// And the flag that says whether a fist is enough is the block's, not the tool's. Dirt says
    /// no tool is needed; stone says one is.
    #[test]
    fn what_needs_a_tool_is_the_blocks_own_answer() {
        let fist = Digger::default();

        // Dirt: no penalty, so a fist is a fist.
        let dirt_by_hand = ticks_to_break(DIRT, false, &fist).expect("it breaks");
        // Stone: a fist is not the right tool, so it takes the hundred divisor.
        let stone_by_hand = ticks_to_break(STONE, true, &fist).expect("it breaks");
        let stone_with_a_pick = ticks_to_break(STONE, true, &with_a_pickaxe(1.0)).expect("breaks");

        assert!(
            stone_by_hand > stone_with_a_pick,
            "stone punishes the wrong tool"
        );
        assert_eq!(
            dirt_by_hand,
            ticks_to_break(DIRT, false, &with_a_pickaxe(1.0)).expect("it breaks"),
            "and dirt does not, at the same tool speed"
        );
    }

    /// Compared as progress rather than as ticks: ticks are rounded up, so five times slower is
    /// not five times the whole number of ticks.
    fn slower_by(digger: &Digger) -> f32 {
        progress_per_tick(STONE, true, &with_a_pickaxe(8.0))
            / progress_per_tick(STONE, true, digger)
    }

    #[test]
    fn breaking_in_mid_air_takes_five_times_as_long() {
        let falling = Digger {
            on_ground: false,
            ..with_a_pickaxe(8.0)
        };
        assert!((slower_by(&falling) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn breaking_underwater_takes_five_times_as_long_unless_the_digger_is_used_to_it() {
        let wet = Digger {
            eyes_in_water: true,
            ..with_a_pickaxe(8.0)
        };
        assert!((slower_by(&wet) - 5.0).abs() < 1e-4);

        // Aqua affinity moves the attribute to one, which takes the whole penalty away.
        let used_to_it = Digger {
            submerged_speed: 1.0,
            ..wet
        };
        assert!((slower_by(&used_to_it) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_two_penalties_stack() {
        let both = Digger {
            eyes_in_water: true,
            on_ground: false,
            ..with_a_pickaxe(8.0)
        };
        assert!(
            (slower_by(&both) - 25.0).abs() < 1e-3,
            "five times for each, one after the other"
        );
    }

    #[test]
    fn efficiency_does_nothing_for_a_tool_no_better_than_a_hand() {
        // Which is why efficiency on a hoe against stone changes nothing.
        let plain = Digger {
            tool_speed: BARE_HANDS,
            ..Digger::default()
        };
        let enchanted = Digger {
            mining_efficiency: 15.0,
            ..plain
        };
        assert_eq!(plain.speed(), enchanted.speed());
    }

    #[test]
    fn efficiency_adds_to_a_real_tool() {
        let plain = with_a_pickaxe(8.0);
        let enchanted = Digger {
            mining_efficiency: 12.0,
            ..plain
        };
        assert_eq!(enchanted.speed(), plain.speed() + 12.0);
    }

    #[test]
    fn haste_adds_a_fifth_a_level_and_fatigue_takes_almost_everything() {
        let plain = with_a_pickaxe(8.0);
        let hasty = Digger { haste: 2, ..plain };
        assert!((hasty.speed() - plain.speed() * 1.4).abs() < 1e-4);

        let tired = Digger {
            fatigue: 1,
            ..plain
        };
        assert!((tired.speed() - plain.speed() * 0.3).abs() < 1e-4);

        // And past the third level it barely moves at all.
        let exhausted = Digger {
            fatigue: 9,
            ..plain
        };
        assert!(exhausted.speed() < plain.speed() * 0.001);
    }

    #[test]
    fn obsidian_with_a_diamond_pickaxe_is_the_long_wait_it_should_be() {
        let ticks = ticks_to_break(OBSIDIAN, true, &with_a_pickaxe(8.0)).expect("it breaks");
        assert_eq!(ticks, 188, "nine and a bit seconds");
    }

    #[test]
    fn a_diamond_pickaxe_knows_what_it_is_worth_against_stone() {
        let pickaxe = Item::from_registry_key("minecraft:diamond_pickaxe");
        // The rule naming stone is the one about mineable blocks, so a caller that says yes to it
        // gets the pickaxe's real speed.
        let (speed, right) = tool_against(pickaxe, |blocks| blocks.contains("pickaxe"));
        assert_eq!(speed, 8.0);
        assert!(right);
    }

    #[test]
    fn a_tool_with_no_rule_for_a_block_falls_back_to_its_own_default() {
        let pickaxe = Item::from_registry_key("minecraft:diamond_pickaxe");
        let (speed, right) = tool_against(pickaxe, |_| false);
        assert_eq!(speed, BARE_HANDS, "a pickaxe against wool is a fist");
        assert!(!right);
    }

    #[test]
    fn an_empty_hand_is_an_empty_hand() {
        assert_eq!(tool_against(None, |_| true), (BARE_HANDS, false));
        let dirt = Item::from_registry_key("minecraft:dirt");
        assert_eq!(tool_against(dirt, |_| true), (BARE_HANDS, false));
    }
}
