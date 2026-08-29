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
//! A hop function receives the packet in its native form and writes the body for the target
//! version directly, rather than each hop rewriting the previous hop's output. Where a packet
//! changed at more than one boundary the lower hop accounts for both — explicitly, in one place.
//! That keeps every hop readable without an intermediate representation for packets to be
//! progressively rewritten through.

use ferrumc_net_codec::encode::errors::NetEncodeError;

pub mod to_26_1;

/// What a hop returns: `None` when the client reads the packet's native form, otherwise the result
/// of having written the older form.
pub type Translated = Option<Result<(), NetEncodeError>>;
