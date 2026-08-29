use crate::encode::errors::NetEncodeError;
use crate::version::ProtocolVersion;
use std::io::Write;
use tokio::io::AsyncWrite;

pub mod errors;
mod primitives;

/// How a value frames itself on the wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Framing {
    /// Written inline, as a field of something else.
    #[default]
    None,
    /// Prefixed with its own length, as a whole packet is.
    WithLength,
    /// Prefixed with a size the reader uses to bound itself.
    SizePrefixed,
}

/// Everything an encoder needs beyond the value itself: how to frame it, and which client is going
/// to read it. The version reaches all the way down, because packet ids and several field layouts
/// differ between the versions this server speaks.
#[derive(Debug, Clone, Copy)]
pub struct NetEncodeOpts {
    pub framing: Framing,
    pub version: ProtocolVersion,
}

impl Default for NetEncodeOpts {
    fn default() -> Self {
        Self {
            framing: Framing::None,
            version: ProtocolVersion::CURRENT,
        }
    }
}

impl NetEncodeOpts {
    #[must_use]
    pub const fn new(framing: Framing, version: ProtocolVersion) -> Self {
        Self { framing, version }
    }

    /// Options for a whole packet written to a client speaking `version`.
    #[must_use]
    pub const fn packet(version: ProtocolVersion) -> Self {
        Self::new(Framing::WithLength, version)
    }

    /// Options for something written inside this value: the same client, no framing of its own.
    #[must_use]
    pub const fn nested(&self) -> Self {
        Self::new(Framing::None, self.version)
    }

    /// The same client, framed differently.
    #[must_use]
    pub const fn framed(&self, framing: Framing) -> Self {
        Self::new(framing, self.version)
    }
}

pub trait NetEncode {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError>;

    #[expect(async_fn_in_trait)]
    async fn encode_async<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError>;
}
