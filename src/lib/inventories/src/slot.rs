//! One slot's worth of items, on the wire and on disk.
//!
//! A slot is a count, a kind, and a patch of components over what that kind already says. An empty
//! slot is a count of nothing and nothing after it.
//!
//! What this replaced read the component **ids** and not their payloads, which meant any stack
//! carrying a component left its payload in the buffer and everything after it — the rest of a
//! container, the rest of the packet — was read at the wrong offset.

use crate::components::Components;
use crate::item::ItemID;
use bitcode_derive::{Decode, Encode};
use ferrumc_net_codec::decode::errors::NetDecodeError;
use ferrumc_net_codec::decode::{NetDecode, NetDecodeOpts};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::registry_remap::NetworkItemId;
use std::fmt::Display;
use std::io::{Read, Write};
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Default, PartialEq, Decode, Encode)]
pub struct InventorySlot {
    pub count: VarInt,
    pub item_id: Option<ItemID>,
    /// What this stack says that its kind does not.
    pub components: Components,
}

impl InventorySlot {
    /// Nothing at all.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            count: VarInt(0),
            item_id: None,
            components: Components::none(),
        }
    }

    /// A plain stack of something, with nothing said about it.
    #[must_use]
    pub fn of(item: ItemID, count: i32) -> Self {
        Self {
            count: VarInt(count),
            item_id: Some(item),
            components: Components::none(),
        }
    }

    /// Whether there is anything here.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count.0 <= 0 || self.item_id.is_none()
    }

    /// Whether two stacks are the same thing, and so may be merged.
    ///
    /// The same kind is not enough: a named sword and a plain one are two different things however
    /// alike they look, which is why the whole patch is compared.
    #[must_use]
    pub fn same_thing_as(&self, other: &Self) -> bool {
        self.item_id == other.item_id && self.components == other.components
    }
}

impl Display for InventorySlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "nothing");
        }
        write!(f, "{} of {:?}", self.count.0, self.item_id)?;
        if !self.components.is_empty() {
            write!(
                f,
                ", with {} components set",
                self.components.iter().count()
            )?;
        }
        Ok(())
    }
}

impl NetDecode for InventorySlot {
    fn decode<R: Read>(reader: &mut R, opts: &NetDecodeOpts) -> Result<Self, NetDecodeError> {
        let count = VarInt::decode(reader, opts)?;
        if count.0 == 0 {
            return Ok(Self::empty());
        }
        Ok(Self {
            count,
            item_id: Some(ItemID(VarInt::decode(reader, opts)?)),
            components: Components::decode(reader, opts)?,
        })
    }

    async fn decode_async<R: AsyncRead + Unpin>(
        _reader: &mut R,
        _opts: &NetDecodeOpts,
    ) -> Result<Self, NetDecodeError> {
        Err(NetDecodeError::ExternalError(
            "a slot is read from a buffer rather than a stream".into(),
        ))
    }
}

impl NetEncode for InventorySlot {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        self.count.encode(writer, opts)?;
        if self.count.0 == 0 {
            return Ok(());
        }

        match &self.item_id {
            // As the number the reading client's own version gives it.
            Some(item) => NetworkItemId(item.0.0 as u32).encode(writer, opts)?,
            None => VarInt::new(0).encode(writer, opts)?,
        }
        self.components.encode(writer, opts)
    }

    async fn encode_async<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        buffer.encode_async(writer, &opts.nested()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Value;
    use ferrumc_data::generated::components::ComponentType;
    use ferrumc_net_codec::encode::Framing;
    use ferrumc_net_codec::version::ProtocolVersion;
    use ferrumc_text::ComponentBuilder;
    use std::io::Cursor;

    fn there_and_back(slot: &InventorySlot) -> InventorySlot {
        let mut bytes = Vec::new();
        slot.encode(
            &mut bytes,
            &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
        )
        .expect("a slot writes to a buffer");

        let mut reader = Cursor::new(&bytes);
        let read =
            InventorySlot::decode(&mut reader, &NetDecodeOpts::default()).expect("it reads back");
        assert_eq!(
            reader.position() as usize,
            bytes.len(),
            "the whole slot was read"
        );
        read
    }

    #[test]
    fn nothing_is_one_byte() {
        let mut bytes = Vec::new();
        InventorySlot::empty()
            .encode(
                &mut bytes,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("nothing writes");
        assert_eq!(bytes, vec![0]);
    }

    #[test]
    fn a_plain_stack_survives_a_round_trip() {
        let stack = InventorySlot::of(ItemID::new(1), 64);
        assert_eq!(there_and_back(&stack), stack);
    }

    /// What this replaced could not do: a stack carrying a component left its payload behind, and
    /// everything after it was read at the wrong offset.
    #[test]
    fn a_stack_with_components_leaves_nothing_behind_it() {
        let mut stack = InventorySlot::of(ItemID::new(895), 1);
        stack
            .components
            .set_name(&ComponentBuilder::text("Sting").build());
        stack
            .components
            .set(ComponentType::Damage, Value::Number(12));
        assert_eq!(there_and_back(&stack), stack);
    }

    /// Which is what makes reading a whole container safe.
    #[test]
    fn two_stacks_in_a_row_each_stop_where_they_end() {
        let mut first = InventorySlot::of(ItemID::new(895), 1);
        first
            .components
            .set(ComponentType::Damage, Value::Number(5));
        let second = InventorySlot::of(ItemID::new(1), 32);

        let opts = NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT);
        let mut bytes = Vec::new();
        first.encode(&mut bytes, &opts).expect("the first writes");
        second.encode(&mut bytes, &opts).expect("the second writes");

        let mut reader = Cursor::new(&bytes);
        let opts = NetDecodeOpts::default();
        assert_eq!(
            InventorySlot::decode(&mut reader, &opts).expect("the first reads"),
            first
        );
        assert_eq!(
            InventorySlot::decode(&mut reader, &opts).expect("the second reads"),
            second
        );
    }

    #[test]
    fn a_named_sword_is_not_the_same_thing_as_a_plain_one() {
        let plain = InventorySlot::of(ItemID::new(895), 1);
        let mut named = plain.clone();
        named
            .components
            .set_name(&ComponentBuilder::text("Sting").build());

        assert!(plain.same_thing_as(&plain.clone()));
        assert!(
            !plain.same_thing_as(&named),
            "merging these would lose the name"
        );
    }

    #[test]
    fn a_stack_survives_being_written_out_and_read_back() {
        let mut stack = InventorySlot::of(ItemID::new(895), 1);
        stack
            .components
            .set(ComponentType::Damage, Value::Number(12));
        stack
            .components
            .set_name(&ComponentBuilder::text("Sting").build());

        let written = bitcode::encode(&stack);
        let read: InventorySlot = bitcode::decode(&written).expect("what was written reads back");
        assert_eq!(read, stack);
    }
}
