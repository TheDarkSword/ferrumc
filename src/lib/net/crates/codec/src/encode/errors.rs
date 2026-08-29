use crate::version::ProtocolVersion;

#[derive(Debug, thiserror::Error)]
pub enum NetEncodeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("External error: {0}")]
    ExternalError(#[from] Box<dyn std::error::Error + Send + Sync>),
    /// A registry id was written for a client whose version has nothing meaning the same thing.
    /// The packet carrying it cannot be sent to that client at all, so the send paths drop it for
    /// that one recipient rather than treating this as a failure.
    #[error("no {registry} in {version} means the same as id {id}")]
    NoSuchId {
        registry: &'static str,
        id: u32,
        version: ProtocolVersion,
    },
    /// A packet was written for a client whose version has no such packet. The send site is
    /// missing a version check; see the feature gating in `docs/networking/`.
    #[error("packet `{packet}` does not exist in {version}")]
    PacketNotInVersion {
        packet: &'static str,
        version: ProtocolVersion,
    },
}
