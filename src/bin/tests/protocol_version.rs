//! What the server tells a client about the version it speaks.
//!
//! The server holds one world in its own version and translates on the wire, so the version it
//! reports has to be the one the *client* asked with, or a supported client is told it is
//! outdated and never tries to connect.

mod common;

use common::TestServer;
use ferrumc_net_codec::version::ProtocolVersion;

#[test]
fn status_echoes_the_version_the_client_speaks() {
    let server = TestServer::shared();

    for version in ProtocolVersion::ALL {
        let mut client = server.connect().expect("connects");
        let status = client
            .status(version.number())
            .unwrap_or_else(|e| panic!("status for {version}: {e}"));

        assert_eq!(
            status["version"]["protocol"],
            version.number(),
            "a client on {version} should be answered in its own protocol"
        );
        assert_eq!(
            status["version"]["name"],
            version.name(),
            "a client on {version} should be answered with its own version name"
        );
    }
}

#[test]
fn status_falls_back_to_the_servers_version_for_unsupported_clients() {
    let server = TestServer::shared();
    let current = ProtocolVersion::CURRENT;

    // 47 is 1.8, far below anything supported; 9999 is beyond anything that exists.
    for unsupported in [47, ProtocolVersion::OLDEST.number() - 1, 9999] {
        let mut client = server.connect().expect("connects");
        let status = client
            .status(unsupported)
            .unwrap_or_else(|e| panic!("status for protocol {unsupported}: {e}"));

        assert_eq!(
            status["version"]["protocol"],
            current.number(),
            "an unsupported client should be shown what the server actually speaks"
        );
    }
}

#[test]
fn status_reports_a_server_that_is_up() {
    let server = TestServer::shared();
    let mut client = server.connect().expect("connects");
    let status = client
        .status(ProtocolVersion::CURRENT.number())
        .expect("status");

    assert!(
        status["players"]["max"].as_i64().is_some_and(|max| max > 0),
        "status should carry a player limit, got {status}"
    );
    assert!(
        status["description"].is_object(),
        "status should carry a description"
    );
}
