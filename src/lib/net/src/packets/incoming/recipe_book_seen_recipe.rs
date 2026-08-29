//! Recipe Book Seen Recipe packet: a recipe stops being highlighted as new.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::var_int::VarInt;

#[derive(NetDecode, Debug)]
#[packet(packet_id = "recipe_book_seen_recipe", state = "play")]
pub struct RecipeBookSeenRecipe {
    pub recipe: VarInt,
}
