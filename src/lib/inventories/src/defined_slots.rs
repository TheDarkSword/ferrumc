pub mod player {
    pub const CRAFT_SLOT_OUTPUT: u8 = 0;
    pub const CRAFT_SLOT_1: u8 = 1;
    pub const CRAFT_SLOT_2: u8 = 2;
    pub const CRAFT_SLOT_3: u8 = 3;
    pub const CRAFT_SLOT_4: u8 = 4;

    pub const HEAD_SLOT: u8 = 5;
    pub const CHEST_SLOT: u8 = 6;
    pub const LEGS_SLOT: u8 = 7;
    pub const FEET_SLOT: u8 = 8;

    pub const HOTBAR_SLOT_1: u8 = 36;
    pub const HOTBAR_SLOT_2: u8 = 37;
    pub const HOTBAR_SLOT_3: u8 = 38;
    pub const HOTBAR_SLOT_4: u8 = 39;
    pub const HOTBAR_SLOT_5: u8 = 40;
    pub const HOTBAR_SLOT_6: u8 = 41;
    pub const HOTBAR_SLOT_7: u8 = 42;
    pub const HOTBAR_SLOT_8: u8 = 43;
    pub const HOTBAR_SLOT_9: u8 = 44;

    pub const OFFHAND_SLOT: u8 = 45;

    /// Where the nine hotbar slots run.
    pub const HOTBAR: std::ops::Range<usize> = 36..45;

    /// Where the twenty-seven main slots run.
    pub const MAIN: std::ops::Range<usize> = 9..36;

    /// Where the four armour slots run.
    pub const ARMOUR: std::ops::Range<usize> = 5..9;

    /// Where the crafting grid runs, output first.
    pub const CRAFTING: std::ops::Range<usize> = 0..5;
}
