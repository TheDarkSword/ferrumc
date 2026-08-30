//! Server Links packet: the links a client offers in its pause and disconnect screens.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_nbt::NBT;
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_text::TextComponent;

/// What a link is called. A known kind is named by the client in its own language; anything else
/// carries the label to show.
pub enum ServerLinkLabel {
    Known(VarInt),
    /// Boxed because a component dwarfs the id beside it, and most links are a known kind.
    Custom(Box<NBT<TextComponent>>),
}

impl NetEncode for ServerLinkLabel {
    fn encode<W: std::io::Write>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        // A boolean chooses between the two, rather than the varint an enum usually leads with.
        match self {
            Self::Known(kind) => {
                true.encode(writer, opts)?;
                kind.encode(writer, opts)
            }
            Self::Custom(label) => {
                false.encode(writer, opts)?;
                label.encode(writer, opts)
            }
        }
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        <W as tokio::io::AsyncWriteExt>::write_all(writer, &buffer).await?;
        Ok(())
    }
}

#[derive(NetEncode)]
pub struct ServerLink {
    pub label: ServerLinkLabel,
    pub url: String,
}

#[derive(NetEncode)]
#[packet(packet_id = "server_links", state = "play")]
pub struct ServerLinks {
    pub links: LengthPrefixedVec<ServerLink>,
}
