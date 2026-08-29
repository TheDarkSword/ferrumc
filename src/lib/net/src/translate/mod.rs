//! Translating packets between the version the server speaks and the version a client speaks.
//!
//! The rest of the server only ever builds packets in its own version — currently 26.2. What a
//! client older than that receives is decided here, by a chain of hops:
//!
//! ```text
//! clientbound   26.2 -> 26.1 -> 1.21.11 -> ... -> the client's version
//! serverbound   the client's version -> ... -> 1.21.11 -> 26.1 -> 26.2
//! ```
//!
//! Minecraft changes between adjacent versions, so this is the shape the work actually has: each
//! hop is one small module that knows about one release boundary, and a new version is a new hop
//! rather than an edit to every packet.
//!
//! # What lives where
//!
//! **Packet ids are not translated here.** They come from tables generated for every supported
//! version, so the `NetEncode` derive already writes the right one and no hop has to care.
//!
//! **Payload differences are what hops are for**, and there are far fewer of them than there are
//! packets. A packet whose body changed points at a hop function with `#[downgrade_with(..)]`; the
//! derive writes the id, then hands the body to that function, which returns `None` when the
//! client is new enough to read the native form.
//!
//! # Ordering
//!
//! A hop that is the only one to touch a packet writes the older body directly. Where several
//! boundaries change the same packet the newest one lists the fields once, as a [`Body`], and the
//! hops below it take that body and apply their own delta — dropping a field, appending one,
//! replacing one. A field that moved or vanished mid-body therefore costs one line in the hop that
//! changed it, rather than a second copy of the field list.
//!
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use std::io::Write;

pub mod to_1_21;
pub mod to_1_21_11;
pub mod to_1_21_2;
pub mod to_1_21_4;
pub mod to_1_21_7;
pub mod to_26_1;

/// What a clientbound hop returns: `None` when the client reads the packet's native form,
/// otherwise the result of having written the older form.
pub type Translated = Option<Result<(), NetEncodeError>>;

/// What a serverbound hop returns: `None` when the client already sends the native form, otherwise
/// that body rewritten into it.
///
/// Serverbound hops run in the opposite order to clientbound ones — the client's own version first,
/// each hop handing the next one up a body it understands — so they work on bytes rather than on
/// the packet type. Only the body needs this: ids are matched per version when the packet is
/// dispatched.
/// Writes the id `packet` carries in the version being encoded for.
///
/// Hops write their own id because a downgrade is sometimes onto a different packet: what 26.2
/// sends as an `entity_position_sync` reaches 1.21 as a `teleport_entity`, and the hop is the only
/// place that knows which packet the older client is going to read.
pub fn write_id<W: Write>(
    writer: &mut W,
    opts: &NetEncodeOpts,
    packet: &'static str,
    ids: [Option<i32>; 10],
) -> Result<(), NetEncodeError> {
    let Some(id) = ids[opts.version.index()] else {
        return Err(NetEncodeError::PacketNotInVersion {
            packet,
            version: opts.version,
        });
    };
    ferrumc_net_codec::net_types::var_int::VarInt::new(id).encode(writer, &opts.nested())
}

/// Writes the id of the packet an older client is about to read.
macro_rules! packet_id {
    ($writer:expr, $opts:expr, $state:literal, $name:literal) => {
        $crate::translate::write_id(
            $writer,
            $opts,
            $name,
            ferrumc_macros::lookup_packet_versioned!($state, "clientbound", $name),
        )
    };
}
pub(crate) use packet_id;

pub type Upgraded = Option<Result<Vec<u8>, ferrumc_net_codec::decode::errors::NetDecodeError>>;

/// A packet body under construction, as the ordered fields a client will read.
///
/// Hops pass one of these down the chain and edit it by name, which is what lets a boundary drop a
/// field from the middle without the hops below it restating the fields around it. Only packets
/// that more than one boundary changes need this; a single hop writes its body directly.
pub struct Body<'a, W> {
    fields: Vec<(&'static str, FieldWriter<'a, W>)>,
}

/// One field of a [`Body`], kept as a closure so a hop can move or drop it without knowing its type.
type FieldWriter<'a, W> = Box<dyn Fn(&mut W, &NetEncodeOpts) -> Result<(), NetEncodeError> + 'a>;

impl<'a, W: Write> Body<'a, W> {
    #[must_use]
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Appends a field. The name is what hops below address it by, and matches the packet's own
    /// field name.
    #[must_use]
    pub fn field<T: NetEncode + 'a>(mut self, name: &'static str, value: &'a T) -> Self {
        self.fields
            .push((name, Box::new(move |w, opts| value.encode(w, opts))));
        self
    }

    /// Drops a field a version does not read.
    #[must_use]
    pub fn without(mut self, name: &str) -> Self {
        let before = self.fields.len();
        self.fields.retain(|(field, _)| *field != name);
        debug_assert!(
            self.fields.len() < before,
            "`{name}` is not in this body: a hop is addressing a field the version above it \
             already removed, or the name is misspelt"
        );
        self
    }

    pub fn write(self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let opts = opts.nested();
        for (_, encode) in self.fields {
            encode(writer, &opts)?;
        }
        Ok(())
    }
}

impl<W: Write> Default for Body<'_, W> {
    fn default() -> Self {
        Self::new()
    }
}
