//! Runs lucidity examples on a real local cluster: a control node, peer nodes, and an app node running the example.
//!
//! Needs the sibling runtime built (`cd ../lunatic && cargo build --release`), or `LUNATIC_BIN` pointing at a `lunatic`
//! binary.  A hang shows up as a timeout, with every node's log in the failure message.

use std::{
    ffi::OsStr,
    fs,
    net::{TcpListener, UdpSocket},
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use tempfile::TempDir;

mod common;

use common::{example, lunatic};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOG_TAIL_LINES: usize = 80;

#[test]
fn every_job_flavor_returns_its_result() {
    let wasm = example("pythagorean");
    let mut cluster = Cluster::start(2);
    cluster.spawn_node("app", Some(&wasm));

    let outcome = cluster.wait_for_any("app", &["remote: ok", "remote: skipped", "panicked"], Duration::from_secs(60));

    assert_eq!(outcome, "remote: ok", "{}", cluster.dump());
    assert!(cluster.log("app").contains("local: ok"), "{}", cluster.dump());
}

#[test]
fn a_dead_job_panics_its_caller() {
    let wasm = example("job_dies");
    let mut cluster = Cluster::start(0);
    cluster.spawn_node("app", Some(&wasm));

    cluster.wait_for_any("app", &["job `spin` on node"], Duration::from_secs(30));

    assert!(cluster.log("app").contains("died before returning a result"), "{}", cluster.dump());
    assert!(!cluster.log("app").contains("unreachable: "), "{}", cluster.dump());
}

#[test]
fn a_job_whose_node_goes_down_panics_its_caller() {
    let wasm = example("node_dies");
    let mut cluster = Cluster::start(1);
    cluster.spawn_node("app", Some(&wasm));

    cluster.wait_for_any("peer1", &["lingering on node"], Duration::from_secs(60));
    cluster.kill("peer1");

    // The control node drops a silent node within its lease, and the app's node then fails the job.
    cluster.wait_for_any("app", &["job `linger` on node", "unreachable: "], Duration::from_secs(30));

    assert!(cluster.log("app").contains("was lost: its node went down"), "{}", cluster.dump());
    assert!(!cluster.log("app").contains("unreachable: "), "{}", cluster.dump());
}

/// A control node and its nodes, each logging to a file in a temporary directory, killed on drop.
struct Cluster {
    dir: TempDir,
    control_url: String,
    nodes: Vec<(String, Child)>,
}

impl Cluster {
    fn start(peers: usize) -> Self {
        let port = free_tcp_port();
        let mut cluster = Self {
            dir: TempDir::new().unwrap(),
            control_url: format!("http://[::1]:{port}/"),
            nodes: Vec::new(),
        };

        cluster.spawn("control", ["control", "--bind-socket", &format!("[::1]:{port}")]);
        cluster.wait_for_any("control", &["Register URL"], STARTUP_TIMEOUT);

        for k in 1..=peers {
            let name = format!("peer{k}");
            cluster.spawn_node(&name, None);
            cluster.wait_for_any(&name, &["Registration successful"], STARTUP_TIMEOUT);
        }

        cluster
    }

    fn spawn_node(&mut self, name: &str, wasm: Option<&Path>) {
        let socket = format!("[::1]:{}", free_udp_port());
        let control_url = self.control_url.clone();
        let mut args = vec![OsStr::new("node"), OsStr::new("--bind-socket"), OsStr::new(&socket)];
        if let Some(wasm) = wasm {
            args.extend([OsStr::new("--wasm"), wasm.as_os_str()]);
        }
        args.push(OsStr::new(&control_url));

        self.spawn(name, args);
    }

    fn spawn<I, S>(&mut self, name: &str, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let log = fs::File::create(self.log_path(name)).unwrap();
        let child = Command::new(lunatic())
            .args(args)
            .env("RUST_LOG", "lunatic=debug")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();

        self.nodes.push((name.to_owned(), child));
    }

    /// Kills `name` outright, as a crash would.
    fn kill(&mut self, name: &str) {
        let (_, child) = self.nodes.iter_mut().find(|(node, _)| node == name).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }

    fn log_path(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("{name}.log"))
    }

    fn log(&self, name: &str) -> String {
        fs::read_to_string(self.log_path(name)).unwrap_or_default()
    }

    /// Waits until `name`'s log contains one of `patterns`, and returns it.
    fn wait_for_any<'a>(&self, name: &str, patterns: &[&'a str], timeout: Duration) -> &'a str {
        let deadline = Instant::now() + timeout;

        loop {
            let log = self.log(name);
            if let Some(pattern) = patterns.iter().find(|pattern| log.contains(*pattern)) {
                return pattern;
            }

            assert!(
                Instant::now() < deadline,
                "timed out after {timeout:?} waiting for one of {patterns:?} in `{name}`'s log\n{}",
                self.dump()
            );
            thread::sleep(POLL_INTERVAL);
        }
    }

    /// The tail of every node's log.
    fn dump(&self) -> String {
        self.nodes
            .iter()
            .map(|(name, _)| {
                let log = self.log(name);
                let lines = log.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>();
                let tail = lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n");

                format!("===== {name} (last {LOG_TAIL_LINES} lines) =====\n{tail}\n")
            })
            .collect()
    }
}

impl Drop for Cluster {
    fn drop(&mut self) {
        for (_, child) in &mut self.nodes {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn free_tcp_port() -> u16 {
    TcpListener::bind("[::1]:0").unwrap().local_addr().unwrap().port()
}

// Nodes talk QUIC, so their sockets are UDP.
fn free_udp_port() -> u16 {
    UdpSocket::bind("[::1]:0").unwrap().local_addr().unwrap().port()
}
