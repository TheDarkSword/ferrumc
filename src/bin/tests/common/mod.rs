//! Harness for tests that need a server actually running.
//!
//! Unit tests cover logic that can be called directly; this covers what only shows up on a socket —
//! version negotiation, the login exchange, packet layout. Those were previously checked by hand:
//! start a server, start a proxy, run a bot, read the logs. Everything here is meant to make that a
//! plain `cargo test`.
//!
//! The server reads its configuration and world from the directory holding its executable, so an
//! instance is isolated by copying the binary into a temporary directory of its own. That is not
//! cheap, so tests share one instance unless they need to mutate the world.

#![allow(dead_code)] // grows a test at a time; not every helper has a caller yet

use ferrumc_net_codec::version::ProtocolVersion;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How long to wait for a freshly started server to accept connections.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How many ports to try before giving up on starting a server.
const PORT_ATTEMPTS: usize = 5;

pub struct TestServer {
    address: SocketAddr,
    // Kept so the directory outlives the server and is removed with it.
    _directory: tempfile::TempDir,
    _child: Child,
}

impl TestServer {
    /// The instance shared by every test in this binary, started on first use.
    ///
    /// Starting a server takes seconds, so tests that only read state share one. A test that
    /// changes the world should call [`TestServer::start`] for an instance of its own.
    pub fn shared() -> &'static TestServer {
        static SHARED: OnceLock<TestServer> = OnceLock::new();
        SHARED.get_or_init(|| TestServer::start().expect("shared test server starts"))
    }

    /// Also fails when the server is up but never answers, which a listening port alone does not
    /// rule out.
    ///
    /// Starts a server with its own world.
    ///
    /// A port is chosen by binding one and letting it go, so two servers starting at the same
    /// moment can be handed the same one; the loser fails to bind and exits. That is detected by
    /// checking the child is still alive once something is listening, and retried.
    pub fn start() -> io::Result<TestServer> {
        let mut last = None;
        for _ in 0..PORT_ATTEMPTS {
            match TestServer::try_start() {
                Ok(server) => return Ok(server),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| io::Error::other("could not start a test server on any port")))
    }

    fn try_start() -> io::Result<TestServer> {
        // Under the target directory rather than the system temporary one: the server binary is
        // hundreds of megabytes, and an instance needs its own copy, which fills a tmpfs the
        // moment a few tests run at once. Being on the same filesystem also lets that copy be a
        // hard link.
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        std::fs::create_dir_all(&base)?;
        sweep_stale(&base);
        let directory = tempfile::TempDir::new_in(&base)?;
        let root = directory.path();

        // `get_root_path()` resolves to the executable's directory, so the binary has to live
        // beside the configuration and world this instance should use.
        let binary = root.join("ferrumc");
        if std::fs::hard_link(server_binary(), &binary).is_err() {
            std::fs::copy(server_binary(), &binary)?;
        }

        let port = free_port()?;
        // The dashboard binds a port of its own and defaults to a fixed one, so several test
        // servers running at once would fight over it and all but the first would exit.
        let dashboard_port = free_port()?;
        std::fs::create_dir_all(root.join("configs"))?;
        std::fs::write(
            root.join("configs/config.toml"),
            // Compression is off because the client below does not implement it, and encryption
            // and online mode are off so a test can log in without a Mojang account.
            format!(
                "host = \"127.0.0.1\"\n\
                 port = {port}\n\
                 online_mode = false\n\
                 encryption_enabled = false\n\
                 network_compression_threshold = -1\n\
                 \n\
                 [dashboard]\n\
                 port = {dashboard_port}\n"
            ),
        )?;

        let log = root.join("server.log");
        let mut child = spawn_server(&binary, &log)?;
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        wait_until_answering(address)?;

        // Something is listening, but it is only ours if our own process is still running: a
        // server that lost the race for this port has already exited.
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "server on port {port} exited during startup with {status}:\n{}",
                std::fs::read_to_string(&log).unwrap_or_default()
            )));
        }

        Ok(TestServer {
            address,
            _directory: directory,
            _child: child,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Opens a connection to this server.
    pub fn connect(&self) -> io::Result<TestClient> {
        TestClient::connect(self.address)
    }
}

/// Nothing a test server made survives it for longer than this.
const STALE_AFTER: Duration = Duration::from_secs(60 * 60);

/// Throws away what earlier runs left behind.
///
/// A test server's directory goes when the value holding it drops, which does not happen if the
/// test binary is killed — and each one pins a copy of the server binary, which is hundreds of
/// megabytes. A few interrupted runs are enough to fill a disk, so every run clears out what is
/// too old to belong to anything still running.
fn sweep_stale(base: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .map(|at| at.elapsed().unwrap_or_default() > STALE_AFTER)
            .unwrap_or(false);
        if old && entry.path().is_dir() {
            // Whatever is left is another run's leavings, and a failure to remove it is not this
            // run's problem.
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ferrumc"))
}

/// Asks the operating system for a port, then releases it. A different process could take it in
/// between, which is unlikely enough to be worth the simplicity.
fn free_port() -> io::Result<u16> {
    TcpListener::bind("127.0.0.1:0")?
        .local_addr()
        .map(|a| a.port())
}

fn spawn_server(binary: &PathBuf, log: &std::path::Path) -> io::Result<Child> {
    // Kept rather than discarded: a server that exits during startup says why here, and the
    // failure is otherwise just a port that never opens.
    let output = std::fs::File::create(log)?;
    let mut command = Command::new(binary);
    command
        .arg("--log=warn")
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output));

    // Without this a failed or interrupted test run leaves a server holding a port and a world.
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }

    command.spawn()
}

/// Waits until the server answers, rather than until something is listening.
///
/// The port opens before the registries and packs are loaded, so a test that starts the moment it
/// can connect gets its first question dropped. Asking one and reading the answer is the only way
/// to know the server is up rather than merely bound.
fn wait_until_answering(address: SocketAddr) -> io::Result<()> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        let answered = TestClient::connect(address)
            .and_then(|mut client| client.status(ProtocolVersion::CURRENT.number()));
        if answered.is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("server on {address} did not answer within {STARTUP_TIMEOUT:?}"),
    ))
}

/// A minimal protocol client.
///
/// Framing and varints are written by hand rather than reused from `ferrumc-net-codec` on purpose:
/// a client built out of the server's own encoder agrees with it by construction and could never
/// catch it being wrong. Everything above the frame is parsed as plain data.
pub struct TestClient {
    stream: TcpStream,
}

impl TestClient {
    pub fn connect(address: SocketAddr) -> io::Result<TestClient> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        Ok(TestClient { stream })
    }

    /// Sends the handshake that opens every connection. `next_state` is 1 for a status request and
    /// 2 to log in.
    pub fn handshake(&mut self, protocol: i32, next_state: i32) -> io::Result<()> {
        let address = self.stream.peer_addr()?;
        let host = address.ip().to_string();

        let mut body = Vec::new();
        write_varint(&mut body, protocol);
        write_string(&mut body, &host);
        body.extend_from_slice(&address.port().to_be_bytes());
        write_varint(&mut body, next_state);
        self.send(0x00, &body)
    }

    /// Performs a server list ping and returns the JSON the server answered with.
    pub fn status(&mut self, protocol: i32) -> io::Result<serde_json::Value> {
        self.handshake(protocol, 1)?;
        self.send(0x00, &[])?;

        let (id, payload) = self.receive()?;
        assert_eq!(
            id, 0x00,
            "expected a status response, got packet 0x{id:02X}"
        );

        let mut cursor = payload.as_slice();
        let json = read_string(&mut cursor)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("status json: {e}")))
    }

    /// Writes one uncompressed packet: length, id, body.
    fn send(&mut self, id: i32, body: &[u8]) -> io::Result<()> {
        let mut framed = Vec::new();
        write_varint(&mut framed, id);
        framed.extend_from_slice(body);

        let mut out = Vec::new();
        write_varint(&mut out, framed.len() as i32);
        out.extend_from_slice(&framed);
        self.stream.write_all(&out)
    }

    /// Reads one uncompressed packet and returns its id and body.
    fn receive(&mut self) -> io::Result<(i32, Vec<u8>)> {
        let length = read_varint(&mut self.stream)?;
        let mut frame = vec![0u8; length as usize];
        self.stream.read_exact(&mut frame)?;

        let mut cursor = frame.as_slice();
        let id = read_varint(&mut cursor)?;
        Ok((id, cursor.to_vec()))
    }
}

fn write_varint(out: &mut Vec<u8>, mut value: i32) {
    loop {
        let byte = (value & 0x7F) as u8;
        value = ((value as u32) >> 7) as i32;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_varint(out, value.len() as i32);
    out.extend_from_slice(value.as_bytes());
}

fn read_varint(reader: &mut impl Read) -> io::Result<i32> {
    let mut value = 0i32;
    for shift in (0..35).step_by(7) {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value |= i32::from(byte[0] & 0x7F) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "varint too long",
    ))
}

fn read_string(reader: &mut impl Read) -> io::Result<String> {
    let length = read_varint(reader)? as usize;
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("not utf-8: {e}")))
}

/// What the server answered with when a login was attempted.
pub struct LoginFinished {
    pub uuid: [u8; 16],
    pub username: String,
    pub property_count: i32,
    /// Bytes after the game profile. 26.2 appends a session id here; earlier versions append
    /// nothing, and sending them sixteen bytes they never read stalls the rest of the exchange.
    pub trailing: Vec<u8>,
}

impl TestClient {
    /// Logs in as `username` and returns the server's `login_finished`, without acknowledging it.
    ///
    /// Only valid against a server with encryption and online mode off, which is how
    /// [`TestServer`] configures itself.
    pub fn login(&mut self, protocol: i32, username: &str) -> io::Result<LoginFinished> {
        self.handshake(protocol, 2)?;

        let mut body = Vec::new();
        write_string(&mut body, username);
        // An offline-mode uuid; the server does not check it.
        body.extend_from_slice(&[0u8; 16]);
        self.send(0x00, &body)?;

        // Compression may be negotiated first; this client does not implement it, so a server that
        // asks for it would need the test config to disable it.
        let (id, payload) = self.receive()?;
        assert_eq!(
            id, 0x02,
            "expected login_finished (0x02), got packet 0x{id:02X}"
        );

        let mut cursor = payload.as_slice();
        let mut uuid = [0u8; 16];
        cursor.read_exact(&mut uuid)?;
        let username = read_string(&mut cursor)?;
        let property_count = read_varint(&mut cursor)?;
        assert_eq!(
            property_count, 0,
            "offline login should carry no profile properties"
        );

        Ok(LoginFinished {
            uuid,
            username,
            property_count,
            trailing: cursor.to_vec(),
        })
    }
}

/// How far a connection got, and what it saw on the way.
pub struct PlaySession {
    /// Ids of the configuration packets received, in order. Useful when a version gets stuck.
    pub configuration_packets: Vec<i32>,
    /// The play `login` packet's id and body.
    pub login_id: i32,
    pub login_body: Vec<u8>,
}

impl TestClient {
    /// Reads up to `count` further packets, stopping early if the server stops sending. Returns
    /// each packet's id and length.
    pub fn drain_play(&mut self, count: usize) -> Vec<(i32, usize)> {
        let mut seen = Vec::new();
        for _ in 0..count {
            match self.receive() {
                Ok((id, body)) => seen.push((id, body.len())),
                Err(_) => break,
            }
        }
        seen
    }
}

// Ids used to drive the exchange. These are stable across the supported range, which is why they
// can be constants here; `docs/networking/multi-version.md` lists what does move.
const LOGIN_ACKNOWLEDGED: i32 = 0x03;
const CONFIG_CLIENT_INFORMATION: i32 = 0x00;
const CONFIG_SELECT_KNOWN_PACKS_IN: i32 = 0x07;
const CONFIG_FINISH_IN: i32 = 0x03;
const CONFIG_SELECT_KNOWN_PACKS_OUT: i32 = 14;
const CONFIG_FINISH_OUT: i32 = 3;
/// Give up rather than hang if the server never finishes configuration.
const MAX_CONFIGURATION_PACKETS: usize = 200;

impl TestClient {
    /// Logs in and drives the exchange all the way to the play state, returning the play `login`
    /// packet. Fails with what it last saw if the server stops partway.
    pub fn reach_play(&mut self, protocol: i32, username: &str) -> io::Result<PlaySession> {
        self.login(protocol, username)?;
        self.send(LOGIN_ACKNOWLEDGED, &[])?;
        self.send_client_information()?;

        let mut configuration_packets = Vec::new();
        loop {
            if configuration_packets.len() > MAX_CONFIGURATION_PACKETS {
                return Err(io::Error::other(format!(
                    "configuration did not finish after {} packets: {configuration_packets:?}",
                    configuration_packets.len()
                )));
            }
            let (id, _) = self.receive()?;
            configuration_packets.push(id);

            if id == CONFIG_SELECT_KNOWN_PACKS_OUT {
                // Agree to nothing; the server then sends its own registries.
                let mut body = Vec::new();
                write_varint(&mut body, 0);
                self.send(CONFIG_SELECT_KNOWN_PACKS_IN, &body)?;
            } else if id == CONFIG_FINISH_OUT {
                self.send(CONFIG_FINISH_IN, &[])?;
                break;
            }
        }

        let (login_id, login_body) = self.receive()?;
        Ok(PlaySession {
            configuration_packets,
            login_id,
            login_body,
        })
    }

    fn send_client_information(&mut self) -> io::Result<()> {
        let mut body = Vec::new();
        write_string(&mut body, "en_us");
        body.push(8); // view distance
        write_varint(&mut body, 0); // chat mode: enabled
        body.push(1); // chat colours
        body.push(0x7F); // every skin part
        write_varint(&mut body, 1); // main hand: right
        body.push(0); // text filtering
        body.push(1); // allow server listings
        write_varint(&mut body, 0); // particle status: all
        self.send(CONFIG_CLIENT_INFORMATION, &body)
    }
}
