//! Signed Chat Command packet: a command carrying a signature for each argument that is chat.
//!
//! A command with nothing signable in it arrives as an ordinary
//! [`super::command::ChatCommandPacket`] instead.

use ferrumc_macros::{packet, NetDecode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;

/// One argument's signature, named so the server knows which argument it covers.
#[derive(NetDecode, Debug)]
pub struct ArgumentSignature {
    pub name: String,
    pub signature: [u8; 256],
}

#[derive(NetDecode, Debug)]
#[upgrade_with(crate::translate::to_1_21_4::chat_command_signed)]
#[packet(packet_id = "chat_command_signed", state = "play")]
pub struct ChatCommandSigned {
    pub command: String,
    /// When the client sent it, in milliseconds, which bounds how long a signature stays good for.
    pub timestamp: u64,
    pub salt: u64,
    pub signatures: LengthPrefixedVec<ArgumentSignature>,
    /// How far the client's view of the chat has moved on since it last said so.
    pub message_count: VarInt,
    /// Twenty bits saying which of the last messages the client has seen.
    pub acknowledged: [u8; 3],
    /// Zero asks the server not to check it.
    pub checksum: u8,
}
