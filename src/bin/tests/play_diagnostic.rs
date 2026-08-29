mod common;
use common::TestServer;
use ferrumc_net_codec::version::ProtocolVersion;

#[test]
#[ignore = "diagnostic: prints what the server sends after the play login"]
fn dump_post_login_packets() {
    let server = TestServer::shared();
    for version in [ProtocolVersion::V26_2, ProtocolVersion::V1_21_4] {
        let mut client = server.connect().expect("connects");
        let session = client
            .reach_play(version.number(), "tester")
            .expect("reaches play");
        let after = client.drain_play(40);
        println!("=== {version}");
        println!("  config packets: {:?}", session.configuration_packets);
        println!(
            "  play login: id {} len {}",
            session.login_id,
            session.login_body.len()
        );
        println!("  after login: {after:?}");
    }
}
