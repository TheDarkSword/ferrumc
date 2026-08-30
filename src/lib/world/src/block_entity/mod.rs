//! What a block holds beyond its state id.
//!
//! A chest's contents, a sign's text, a furnace's progress: none of it fits in a state id, so those
//! blocks carry a block entity alongside. 186 of the 1196 blocks do; which block carries which
//! comes from the game itself, keyed on the block rather than the state, since every state of a
//! chest is a chest.
//!
//! Block entities live in the chunk and are written with it, so what a sign says survives a
//! restart.

use crate::block_data::block_entity_type;
use crate::block_state::BlockId;
use crate::pos::ChunkBlockPos;
use bitcode_derive::{Decode, Encode};
use deepsize::DeepSizeOf;
use ferrumc_text::TextComponent;

/// One block entity, at a position inside its chunk.
///
/// The position is kept as plain coordinates rather than a `ChunkBlockPos` so the whole thing can
/// be written with the chunk without a hand-written encoding.
#[derive(Debug, Clone, PartialEq, Encode, Decode, DeepSizeOf)]
pub struct BlockEntity {
    pub x: u8,
    pub y: i16,
    pub z: u8,
    /// Which kind, as the game's own registry id, which is also what goes on the wire.
    pub kind: u16,
    pub data: BlockEntityData,
}

/// What the block entity holds.
#[derive(Debug, Clone, PartialEq, Encode, Decode, DeepSizeOf)]
pub enum BlockEntityData {
    /// Boxed because a sign carries eight lines of text and most block entities carry nothing, and
    /// every one of them would otherwise be as large as the largest.
    Sign(Box<Sign>),
    /// A kind that is not modelled yet. The block entity is there — a client is told it exists, and
    /// it is written with the chunk — but it carries nothing.
    Empty,
}

/// A sign's two faces and whether it can still be edited.
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode, DeepSizeOf)]
pub struct Sign {
    pub front: SignFace,
    pub back: SignFace,
    /// Waxed signs cannot be edited again.
    pub waxed: bool,
}

/// One face of a sign.
///
/// A line is a text component, as it is in the game: it can be coloured, translated or carry a
/// click event, none of which a bare string says. What is kept here is the component written out,
/// because that is what survives being stored — the same split vanilla has between the component it
/// works with and the codec it saves through.
#[derive(Debug, Clone, PartialEq, Encode, Decode, DeepSizeOf)]
pub struct SignFace {
    written: [String; LINES],
    /// A dye colour name, black unless dyed.
    pub colour: String,
    pub glowing: bool,
}

/// Every sign has four lines, whether or not anything is on them.
pub const LINES: usize = 4;

impl Default for SignFace {
    fn default() -> Self {
        Self {
            written: std::array::from_fn(|_| empty_line()),
            colour: "black".to_string(),
            glowing: false,
        }
    }
}

/// A component with nothing in it, which is what a blank line is.
fn empty_line() -> String {
    serde_json::to_string(&TextComponent::default()).unwrap_or_else(|_| "\"\"".to_string())
}

impl SignFace {
    /// What one line says. A line that cannot be read back is treated as blank rather than
    /// refusing the whole sign.
    #[must_use]
    pub fn line(&self, index: usize) -> TextComponent {
        self.written
            .get(index)
            .and_then(|written| serde_json::from_str(written).ok())
            .unwrap_or_default()
    }

    /// Every line, in order.
    #[must_use]
    pub fn lines(&self) -> [TextComponent; LINES] {
        std::array::from_fn(|index| self.line(index))
    }

    /// Writes one line. Lines past the fourth are ignored, as they are in the game.
    pub fn set_line(&mut self, index: usize, line: &TextComponent) {
        if let Some(slot) = self.written.get_mut(index) {
            if let Ok(written) = serde_json::to_string(line) {
                *slot = written;
            }
        }
    }

    /// The lines as they are stored, which is also what goes on the wire.
    pub(crate) fn written(&self) -> &[String; LINES] {
        &self.written
    }
}

impl BlockEntity {
    /// The block entity a block would carry, if it carries one.
    ///
    /// A sign is given empty text rather than nothing, so it is a sign from the moment it is
    /// placed.
    #[must_use]
    pub fn for_block(block: BlockId, pos: ChunkBlockPos) -> Option<Self> {
        let kind = block_entity_type(block)?;
        let data = if is_sign(block) {
            BlockEntityData::Sign(Box::default())
        } else {
            BlockEntityData::Empty
        };
        Some(Self {
            x: pos.x(),
            y: pos.y(),
            z: pos.z(),
            kind,
            data,
        })
    }

    /// Where it is in its chunk.
    #[must_use]
    pub fn pos(&self) -> ChunkBlockPos {
        ChunkBlockPos::new(self.x, self.y, self.z)
    }
}

/// Whether this block is one of the signs, which all share a block entity.
fn is_sign(block: BlockId) -> bool {
    crate::block_tag::tag("minecraft:all_signs").is_some_and(|tag| tag.contains(block))
}

/// A sign as the client reads it.
///
/// The lines are text components, and a component may be written as a plain string when it is only
/// text, which is what a sign's lines are until something styles them.
#[derive(ferrumc_macros::NBTSerialize)]
struct SignNbt {
    front_text: SignFaceNbt,
    back_text: SignFaceNbt,
    is_waxed: bool,
}

#[derive(ferrumc_macros::NBTSerialize)]
struct SignFaceNbt {
    messages: Vec<String>,
    color: String,
    has_glowing_text: bool,
}

impl From<&SignFace> for SignFaceNbt {
    fn from(face: &SignFace) -> Self {
        Self {
            messages: face.written().to_vec(),
            color: face.colour.clone(),
            has_glowing_text: face.glowing,
        }
    }
}

impl BlockEntity {
    /// What this block entity says on the wire.
    ///
    /// A kind that is not modelled yet says nothing, which is a valid answer: the client is told
    /// the block entity is there and reads no fields from it.
    #[must_use]
    pub fn to_nbt(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.data {
            BlockEntityData::Sign(sign) => {
                let nbt = SignNbt {
                    front_text: SignFaceNbt::from(&sign.front),
                    back_text: SignFaceNbt::from(&sign.back),
                    is_waxed: sign.waxed,
                };
                ferrumc_nbt::NBTSerializable::serialize(
                    &nbt,
                    &mut out,
                    &ferrumc_nbt::NBTSerializeOptions::Network,
                );
            }
            BlockEntityData::Empty => {
                // An empty compound, which is what a block entity with nothing to say looks like.
                ferrumc_nbt::NBTSerializable::serialize(
                    &EmptyNbt {},
                    &mut out,
                    &ferrumc_nbt::NBTSerializeOptions::Network,
                );
            }
        }
        out
    }
}

#[derive(ferrumc_macros::NBTSerialize)]
struct EmptyNbt {}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str) -> BlockId {
        BlockId::from_name(name).unwrap_or_else(|| panic!("{name} exists"))
    }

    /// A block that carries one gets one; a block that does not, does not.
    #[test]
    fn a_block_entity_follows_its_block() {
        let pos = ChunkBlockPos::new(3, 64, 9);

        let sign =
            BlockEntity::for_block(block("minecraft:oak_sign"), pos).expect("signs carry one");
        assert!(matches!(sign.data, BlockEntityData::Sign(_)));
        assert_eq!(sign.pos(), pos);

        // A chest carries one too, it is just not modelled yet.
        let chest =
            BlockEntity::for_block(block("minecraft:chest"), pos).expect("chests carry one");
        assert_eq!(chest.data, BlockEntityData::Empty);
        assert_ne!(chest.kind, sign.kind);

        assert!(BlockEntity::for_block(block("minecraft:stone"), pos).is_none());
    }

    /// A line keeps everything a component carries, not only its text: a coloured line comes back
    /// coloured. This is why lines are components rather than strings.
    #[test]
    fn a_line_keeps_more_than_its_text() {
        use ferrumc_text::{Color, NamedColor, TextComponentBuilder};

        let mut face = SignFace::default();
        let written = TextComponentBuilder::new("Beware".to_string())
            .color(Color::Named(NamedColor::Red))
            .bold()
            .build();
        face.set_line(1, &written);

        let read = face.line(1);
        assert_eq!(read, written);
        assert_eq!(
            face.line(0),
            TextComponent::default(),
            "the other lines are untouched"
        );
    }

    /// What a sign says has to survive being written with its chunk and read back.
    #[test]
    fn a_sign_survives_being_stored() {
        use ferrumc_text::TextComponentBuilder;

        let mut sign = Sign::default();
        sign.front
            .set_line(0, &TextComponentBuilder::new("first".to_string()).build());
        sign.back
            .set_line(3, &TextComponentBuilder::new("last".to_string()).build());
        sign.waxed = true;

        let entity = BlockEntity {
            x: 1,
            y: -60,
            z: 2,
            kind: 7,
            data: BlockEntityData::Sign(Box::new(sign)),
        };
        let bytes = bitcode::encode(&entity);
        let back: BlockEntity = bitcode::decode(&bytes).expect("reads back");

        assert_eq!(back, entity);
        let BlockEntityData::Sign(sign) = back.data else {
            panic!("a sign should read back as a sign");
        };
        assert_eq!(
            sign.front.line(0),
            TextComponentBuilder::new("first".to_string()).build()
        );
        assert_eq!(
            sign.back.line(3),
            TextComponentBuilder::new("last".to_string()).build()
        );
        assert!(sign.waxed);
    }

    /// A sign starts blank rather than absent, so it is a sign as soon as it is placed.
    #[test]
    fn a_new_sign_is_blank() {
        let sign = BlockEntity::for_block(
            block("minecraft:oak_wall_sign"),
            ChunkBlockPos::new(0, 0, 0),
        )
        .expect("signs carry one");
        let BlockEntityData::Sign(sign) = sign.data else {
            panic!("a sign should hold sign data");
        };
        assert_eq!(
            sign.front.lines(),
            std::array::from_fn(|_| TextComponent::default())
        );
        assert_eq!(sign.front.colour, "black");
        assert!(!sign.waxed);
    }
}
