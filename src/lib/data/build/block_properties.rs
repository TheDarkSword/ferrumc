use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use syn::{LitInt, LitStr};

/// What the game says each block state is like to break, asked of it by
/// `scripts/extract_block_properties.py`.
const PROPERTIES: &str = "../../../assets/extracted/26.2/block_properties.json";

/// What is known about one state.
#[derive(Deserialize)]
struct StateProperties {
    hardness: f32,
    needs_the_right_tool: bool,
    light: u8,
}

/// How many bytes each state takes in the packed table: four of hardness, one of flags and light.
const PER_STATE: usize = 5;

/// Which bit of the last byte says the right tool is needed.
const NEEDS_THE_RIGHT_TOOL: u8 = 0x80;

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed={PROPERTIES}");

    let properties: BTreeMap<String, StateProperties> = serde_json::from_str(
        &fs::read_to_string(PROPERTIES).expect("what each block state is like to break"),
    )
    .expect("the block properties are valid json");

    let count = properties
        .keys()
        .filter_map(|id| id.parse::<usize>().ok())
        .max()
        .map_or(0, |highest| highest + 1);

    // Packed into a file rather than emitted as a literal array. Thirty-two thousand states is a
    // token per number, which is minutes of compile time for a table that is only ever read.
    let mut packed = vec![0u8; count * PER_STATE];
    for (id, state) in &properties {
        let Ok(id) = id.parse::<usize>() else {
            continue;
        };
        let at = id * PER_STATE;
        packed[at..at + 4].copy_from_slice(&state.hardness.to_le_bytes());
        // The light a block gives off never passes fifteen, so the top bit is free for the flag.
        packed[at + 4] = state.light.min(15)
            | if state.needs_the_right_tool {
                NEEDS_THE_RIGHT_TOOL
            } else {
                0
            };
    }

    let out = Path::new(&std::env::var("OUT_DIR").expect("cargo says where to write"))
        .join("block_properties.bin");
    fs::write(&out, &packed).expect("the packed table writes");
    let out = LitStr::new(
        out.to_str().expect("a path that is text"),
        proc_macro2::Span::call_site(),
    );

    let count = LitInt::new(&count.to_string(), proc_macro2::Span::call_site());
    let per_state = LitInt::new(&PER_STATE.to_string(), proc_macro2::Span::call_site());
    let flag = LitInt::new(
        &format!("0x{NEEDS_THE_RIGHT_TOOL:02X}u8"),
        proc_macro2::Span::call_site(),
    );

    quote! {
        /// What each block state is like to break, packed by state id.
        ///
        /// Four bytes of hardness and one of light with a flag on top, which is small enough to
        /// stay in cache while a player mines.
        static PACKED: &[u8] = include_bytes!(#out);

        /// How many states there are.
        pub const STATES: usize = #count;

        const PER_STATE: usize = #per_state;
        const NEEDS_THE_RIGHT_TOOL: u8 = #flag;

        /// How hard a state is to break.
        ///
        /// A negative answer means nothing breaks it — bedrock, the portal frame, the void. A zero
        /// means it goes at a touch. Anything this server does not know about answers zero, since
        /// treating an unknown block as unbreakable would strand a player.
        #[must_use]
        pub fn hardness(state: u32) -> f32 {
            let at = state as usize * PER_STATE;
            match PACKED.get(at..at + 4) {
                Some(&[a, b, c, d]) => f32::from_le_bytes([a, b, c, d]),
                _ => 0.0,
            }
        }

        /// Whether the right tool is needed for it to drop anything.
        ///
        /// Dirt drops with a fist; stone does not. This is also what decides how much slower a
        /// wrong tool breaks it.
        #[must_use]
        pub fn needs_the_right_tool(state: u32) -> bool {
            let at = state as usize * PER_STATE + 4;
            PACKED
                .get(at)
                .is_some_and(|flags| flags & NEEDS_THE_RIGHT_TOOL != 0)
        }

        /// How much light it gives off, from nothing to fifteen.
        #[must_use]
        pub fn light(state: u32) -> u8 {
            let at = state as usize * PER_STATE + 4;
            PACKED.get(at).map_or(0, |flags| flags & 0x0F)
        }
    }
}
