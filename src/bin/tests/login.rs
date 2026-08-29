//! The shape of the login exchange, per protocol version.
//!
//! `login_finished` gained a session id in 26.2. Sending it to an older client leaves sixteen
//! bytes that client never reads, and the exchange stalls until the handshake times out — a
//! failure that shows up as a connection that never joins, with nothing logged.

mod common;

use common::TestServer;
use ferrumc_net_codec::version::ProtocolVersion;

/// The session id is sixteen bytes appended after the game profile.
const SESSION_ID_LENGTH: usize = 16;
/// A single boolean, which 1.21 and older still expect.
const STRICT_ERROR_HANDLING_LENGTH: usize = 1;

#[test]
fn login_finished_carries_a_session_id_only_from_26_2() {
    let server = TestServer::shared();

    for version in ProtocolVersion::ALL {
        let mut client = server.connect().expect("connects");
        let finished = client
            .login(version.number(), "tester")
            .unwrap_or_else(|e| panic!("login on {version}: {e}"));

        assert_eq!(finished.username, "tester", "on {version}");

        // 26.2 appends a session id; 1.21 still reads a strict error handling flag that 1.21.2
        // dropped. Everything between them reads nothing after the game profile.
        let expected = if version >= ProtocolVersion::V26_2 {
            SESSION_ID_LENGTH
        } else if version < ProtocolVersion::V1_21_2 {
            STRICT_ERROR_HANDLING_LENGTH
        } else {
            0
        };
        assert_eq!(
            finished.trailing.len(),
            expected,
            "on {version}, login_finished should have {expected} bytes after the game profile, \
             found {}",
            finished.trailing.len()
        );
    }
}
