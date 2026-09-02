#[doc = r" What each block state is like to break, packed by state id."]
#[doc = r""]
#[doc = r" Four bytes of hardness and one of light with a flag on top, which is small enough to"]
#[doc = r" stay in cache while a player mines."]
static PACKED : & [u8] = include_bytes ! ("/home/michele/Sviluppo/RustRoverProjects/ferrumc/target/quick/build/ferrumc-data/9d42ba9b443c9eea/out/block_properties.bin") ;
#[doc = r" How many states there are."]
pub const STATES: usize = 32366;
const PER_STATE: usize = 5;
const NEEDS_THE_RIGHT_TOOL: u8 = 0x80u8;
#[doc = r" How hard a state is to break."]
#[doc = r""]
#[doc = r" A negative answer means nothing breaks it — bedrock, the portal frame, the void. A zero"]
#[doc = r" means it goes at a touch. Anything this server does not know about answers zero, since"]
#[doc = r" treating an unknown block as unbreakable would strand a player."]
#[must_use]
pub fn hardness(state: u32) -> f32 {
    let at = state as usize * PER_STATE;
    match PACKED.get(at..at + 4) {
        Some(&[a, b, c, d]) => f32::from_le_bytes([a, b, c, d]),
        _ => 0.0,
    }
}
#[doc = r" Whether the right tool is needed for it to drop anything."]
#[doc = r""]
#[doc = r" Dirt drops with a fist; stone does not. This is also what decides how much slower a"]
#[doc = r" wrong tool breaks it."]
#[must_use]
pub fn needs_the_right_tool(state: u32) -> bool {
    let at = state as usize * PER_STATE + 4;
    PACKED
        .get(at)
        .is_some_and(|flags| flags & NEEDS_THE_RIGHT_TOOL != 0)
}
#[doc = r" How much light it gives off, from nothing to fifteen."]
#[must_use]
pub fn light(state: u32) -> u8 {
    let at = state as usize * PER_STATE + 4;
    PACKED.get(at).map_or(0, |flags| flags & 0x0F)
}
