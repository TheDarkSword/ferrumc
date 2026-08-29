//! Getting all the way into the world, on every supported version.
//!
//! Everything before this point can look healthy while a client still never joins: the login
//! exchange completes, configuration completes, and then a packet with the wrong shape resets the
//! connection with nothing logged.

mod common;

use common::TestServer;
use ferrumc_net_codec::version::ProtocolVersion;

#[test]
fn every_supported_version_reaches_the_play_state() {
    let server = TestServer::shared();
    let mut failures = Vec::new();

    for version in ProtocolVersion::ALL {
        let mut client = match server.connect() {
            Ok(client) => client,
            Err(e) => {
                failures.push(format!("{version}: could not connect: {e}"));
                continue;
            }
        };

        match client.reach_play(version.number(), "tester") {
            Ok(session) => {
                assert!(
                    !session.login_body.is_empty(),
                    "{version}: play login packet was empty"
                );
            }
            Err(e) => failures.push(format!("{version}: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "versions that never reached play:\n  {}",
        failures.join("\n  ")
    );
}
