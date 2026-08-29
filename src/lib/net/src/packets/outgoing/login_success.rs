use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::prefixed_optional::PrefixedOptional;
use std::sync::LazyLock;
use uuid::Uuid;

/// Identifies the run of the server a client joined, shared by everyone connected at the same time.
/// Vanilla clears it once the last connection drops and picks a new one on the next join; keeping
/// one per process is indistinguishable to a client and avoids the bookkeeping.
static SESSION_ID: LazyLock<u128> = LazyLock::new(|| Uuid::new_v4().as_u128());

#[derive(NetEncode)]
#[packet(packet_id = "login_finished", state = "login")]
pub struct LoginSuccessPacket<'a> {
    pub uuid: u128,
    pub username: &'a str,
    pub properties: LengthPrefixedVec<LoginSuccessProperties<'a>>,
    pub session_id: u128,
}

impl<'a> LoginSuccessPacket<'a> {
    pub fn new(uuid: u128, username: &'a str) -> Self {
        Self {
            uuid,
            username,
            properties: LengthPrefixedVec::new(vec![]),
            session_id: session_id(),
        }
    }
}

/// The session id sent to every client for the lifetime of this server process.
pub fn session_id() -> u128 {
    *SESSION_ID
}

#[derive(NetEncode, Clone)]
pub struct LoginSuccessProperties<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub signature: PrefixedOptional<&'a str>,
}
