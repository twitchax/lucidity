//! The `lucidity-core` crate.  Abstracts away core functionality, and functionality for build scripts.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use std::{cell::RefCell, time::Duration};

use lunatic::{function::process::IntoProcess, LinkDiedSignal, LunaticError, Mailbox, MessageSignal, Process, ProcessConfig, ProcessDiedSignal, Tag};
use rand::RngExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub use lunatic;
pub use rand;
pub use serde;

/// A function that can run as a job, usually implemented by the [`lucidity::job`] macro.
pub trait JobFn: 'static {
    /// The arguments, as one value: `()`, the single argument, or a tuple of them.
    type Args: Serialize + DeserializeOwned + 'static;
    /// The return type.
    type Output: Serialize + DeserializeOwned + Clone + 'static;

    /// The function name, used in failure messages.
    const NAME: &'static str;
    /// The maximum memory, in bytes, of the job's process.
    const MAX_MEMORY: u64;
    /// The maximum fuel (in units of ~100,000 instructions) of the job's process, or `None` for no limit.
    const MAX_FUEL: Option<u64>;

    /// Calls the function.
    fn call(args: Self::Args) -> Self::Output;
}

/// How [`fanout`] picks a node for each job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fanout {
    /// Cycle through the nodes in order.
    RoundRobin,
    /// Pick a random node for each job.
    Random,
}

/// What the caller gets back.
type Outcome<T> = Result<T, Failed>;

/// Why a job produced no result.
#[derive(Serialize, Deserialize)]
enum Failed {
    /// The job's process died.
    Died,
    /// The job's watcher died, which in practice means that its node went down.
    Lost,
}

/// What a relay receives, for a job with arguments `A` and output `T`.
#[derive(Serialize, Deserialize)]
enum Relay<A, T> {
    /// The job's watcher, from the caller.
    Watch(Process<Watch<A, T>>),
    /// The job's outcome, from its watcher.
    Report(Outcome<T>),
    /// The caller dropped the job before it finished, or found no node to spawn it on.
    Cancel,
}

/// What a watcher receives, for a job with arguments `A` and output `T`.
#[derive(Serialize, Deserialize)]
enum Watch<A, T> {
    /// The job's arguments, from the caller.
    Start(A),
    /// The job's result, from the job.
    Done(T),
    /// The caller dropped the job before it finished.
    Cancel,
}

type RelayOf<J> = Relay<<J as JobFn>::Args, <J as JobFn>::Output>;
type WatchOf<J> = Watch<<J as JobFn>::Args, <J as JobFn>::Output>;

enum State<T> {
    Pending,
    Done(T),
}

/// A job running in its own process, possibly on another node.
///
/// A job is spawned as a small watcher process on the target node, which spawns the job's process linked to itself
/// on that same node (links don't cross nodes), and reports exactly one outcome: the result, or that the job's process
/// died.  The report goes to a relay process on the caller's node, which monitors the watcher (monitors do cross
/// nodes), and forwards the report to the caller's mailbox under a tag unique to this job, or reports the job lost if
/// the watcher dies first, which happens when its node goes down.  The relay keeps the monitor's signals out of the
/// caller's mailbox.  The caller never polls or retries a started job, so a slow job is only ever slow, and a dead or
/// lost one panics the caller instead of hanging it.
///
/// The caller sends the watcher the job's arguments once the watcher is spawned, rather than with the spawn, which
/// drops them when the node isn't in the cluster: that way the caller still has them to spawn the job on another node.
///
/// Dropping a job before its result arrives cancels it.
pub struct Job<J: JobFn> {
    node: u64,
    relay: Process<RelayOf<J>>,
    watcher: Process<WatchOf<J>>,
    tag: Tag,
    state: RefCell<State<J::Output>>,
}

impl<J: JobFn> Job<J> {
    /// Spawns the job on `node` (which may be this one).
    ///
    /// Panics if `node` isn't in the cluster.
    pub fn spawn(node: u64, args: J::Args) -> Self {
        let (tag, relay) = spawn_relay::<J>();

        match spawn_watcher::<J>(node, &config::<J>(), relay) {
            Ok(watcher) => Self::start(node, tag, relay, watcher, args),
            Err(error) => panic!("lucidity: cannot spawn job `{}` on node {node}: {error}", J::NAME),
        }
    }

    /// Spawns the job on a random node of `nodes` (which may include this one).  A node that isn't in the cluster (any
    /// more) is skipped, and another one picked.
    ///
    /// Panics if none of `nodes` is in the cluster.
    pub fn spawn_any(nodes: &[u64], args: J::Args) -> Self {
        Self::spawn_first(&mut nodes.to_vec(), pick, args).unwrap_or_else(|| no_node_left::<J>(nodes))
    }

    /// Spawns the job on a random node in the distributed pool, other than this one, as [`Job::spawn_any`] does with
    /// [`remote_nodes`].
    ///
    /// Panics if no other nodes are connected.
    pub fn spawn_remote(args: J::Args) -> Self {
        Self::spawn_any(&connected_remote_nodes(), args)
    }

    // Spawns the job on the node `choose` picks from `nodes`, after removing from `nodes` each pick that isn't in the
    // cluster (any more).  Returns `None` once `nodes` is empty.
    fn spawn_first(nodes: &mut Vec<u64>, choose: impl Fn(&[u64]) -> u64, args: J::Args) -> Option<Self> {
        let (tag, relay) = spawn_relay::<J>();
        let config = config::<J>();

        while !nodes.is_empty() {
            let node = choose(nodes);
            match spawn_watcher::<J>(node, &config, relay) {
                Ok(watcher) => return Some(Self::start(node, tag, relay, watcher, args)),
                Err(LunaticError::NodeNotFound(_)) => nodes.retain(|&other| other != node),
                Err(error) => panic!("lucidity: cannot spawn job `{}` on node {node}: {error}", J::NAME),
            }
        }

        relay.send(Relay::Cancel);
        None
    }

    // Hands the job's arguments to its new watcher, and the watcher to its relay.
    fn start(node: u64, tag: Tag, relay: Process<RelayOf<J>>, watcher: Process<WatchOf<J>>, args: J::Args) -> Self {
        watcher.send(Watch::Start(args));
        relay.send(Relay::Watch(watcher));

        Self {
            node,
            relay,
            watcher,
            tag,
            state: RefCell::new(State::Pending),
        }
    }

    /// Returns the result if the job has finished, without blocking.
    ///
    /// Panics if the job's process died, or if its node went down.
    pub fn try_get(&self) -> Option<J::Output> {
        self.receive(Some(Duration::ZERO));

        match &*self.state.borrow() {
            State::Done(output) => Some(output.clone()),
            State::Pending => None,
        }
    }

    /// Blocks until the job finishes, and returns its result.
    ///
    /// Panics if the job's process died, or if its node went down.
    pub fn await_get(&self) -> J::Output {
        self.receive(None);

        match &*self.state.borrow() {
            State::Done(output) => output.clone(),
            State::Pending => unreachable!("receiving without a timeout always produces an outcome"),
        }
    }

    // Moves the outcome, if it arrives within `timeout`, from the mailbox into `state`.
    fn receive(&self, timeout: Option<Duration>) {
        if let State::Done(_) = *self.state.borrow() {
            return;
        }

        // SAFETY: see `spawn`.
        let mailbox = unsafe { Mailbox::<Outcome<J::Output>>::new() };
        let outcome = match timeout {
            Some(timeout) => match mailbox.tag_receive_timeout(&[self.tag], timeout) {
                Ok(outcome) => outcome,
                Err(_) => return,
            },
            None => mailbox.tag_receive(&[self.tag]),
        };

        match outcome {
            Ok(output) => *self.state.borrow_mut() = State::Done(output),
            Err(Failed::Died) => panic!("lucidity: job `{}` on node {} died before returning a result", J::NAME, self.node),
            Err(Failed::Lost) => panic!("lucidity: job `{}` on node {} was lost: its node went down before it returned a result", J::NAME, self.node),
        }
    }
}

impl<J: JobFn> Drop for Job<J> {
    fn drop(&mut self) {
        if let State::Pending = *self.state.borrow() {
            // Messages from one process arrive in order, so the watcher gets the cancel after the job's arguments.
            // Either process may have just exited after the outcome was sent; its message is then dropped.
            self.watcher.send(Watch::Cancel);
            self.relay.send(Relay::Cancel);
        }
    }
}

/// Spawns one job per set of arguments, on nodes picked by `fanout`, and blocks until all of them finish.
///
/// A node that isn't in the cluster (any more) is skipped for the rest of the fanout, and the job spawned on the next
/// pick.  The results are in the same order as the arguments.  Panics if any job's process died, or if any job's node
/// went down, or if no other nodes are connected.
pub fn fanout<J: JobFn>(args: Vec<J::Args>, fanout: Fanout) -> Vec<J::Output> {
    let remote = connected_remote_nodes();
    let mut nodes = remote.clone();

    let jobs = args
        .into_iter()
        .enumerate()
        .map(|(k, args)| {
            let job = match fanout {
                Fanout::RoundRobin => Job::<J>::spawn_first(&mut nodes, |nodes| nodes[k % nodes.len()], args),
                Fanout::Random => Job::<J>::spawn_first(&mut nodes, pick, args),
            };

            job.unwrap_or_else(|| no_node_left::<J>(&remote))
        })
        .collect::<Vec<_>>();

    jobs.iter().map(Job::await_get).collect()
}

/// The id of the node this process runs on.
pub fn local_node() -> u64 {
    lunatic::host::node_id()
}

/// A random node from the distributed pool, other than this one.
///
/// Panics if no other nodes are connected.
pub fn random_node() -> u64 {
    pick(&connected_remote_nodes())
}

/// The nodes in the distributed pool, other than this one: where `remote` jobs run.
pub fn remote_nodes() -> Vec<u64> {
    let local = local_node();
    lunatic::distributed::nodes().into_iter().filter(|&node| node != local).collect()
}

fn connected_remote_nodes() -> Vec<u64> {
    let nodes = remote_nodes();
    assert!(!nodes.is_empty(), "lucidity: no remote nodes are connected");
    nodes
}

fn pick(nodes: &[u64]) -> u64 {
    nodes[rand::rng().random_range(0..nodes.len())]
}

fn no_node_left<J: JobFn>(nodes: &[u64]) -> ! {
    panic!("lucidity: cannot spawn job `{}`: none of the nodes {nodes:?} is in the cluster", J::NAME)
}

// Spawns the relay of a job whose outcome goes to this process, and returns the job's tag, under which it does.
fn spawn_relay<J: JobFn>() -> (Tag, Process<RelayOf<J>>) {
    let tag = Tag::new();
    // SAFETY: only `Outcome<J::Output>` messages are ever sent under `tag`, and they're only received as such.
    let caller = unsafe { Process::<Outcome<J::Output>>::this() };
    // Linked, so that the relay doesn't outlive a caller that dies, e.g. because spawning the watcher failed.  The
    // watcher is spawned by the caller, rather than by the relay, so that this failure happens in the caller.
    (tag, Process::spawn_link((caller, tag), relay::<J>))
}

// Spawns a job's watcher on `node`, reporting to `relay`.
fn spawn_watcher<J: JobFn>(node: u64, config: &ProcessConfig, relay: Process<RelayOf<J>>) -> Result<Process<WatchOf<J>>, LunaticError> {
    <Mailbox<WatchOf<J>> as IntoProcess<_, _>>::spawn(relay, watch::<J>, None, None, Some(config), Some(node))
}

// Both the watcher and the job run with these limits, and may spawn (nested) jobs of their own.
fn config<J: JobFn>() -> ProcessConfig {
    let mut config = ProcessConfig::new().unwrap_or_else(|e| panic!("lucidity: cannot create a process config for job `{}`: {e}", J::NAME));
    config.set_can_spawn_processes(true);
    config.set_can_create_configs(true);
    config.set_can_compile_modules(true);
    config.set_max_memory(J::MAX_MEMORY);
    if let Some(fuel) = J::MAX_FUEL {
        config.set_max_fuel(fuel);
    }
    config
}

// Runs on the caller's node: forwards the watcher's report to the caller, or reports the job lost if the watcher dies
// without one.  The watcher's death can't overtake its report, since a monitor's signal arrives after every message
// the monitored process sent.
fn relay<J: JobFn>((caller, tag): (Process<Outcome<J::Output>>, Tag), mailbox: Mailbox<RelayOf<J>>) {
    let mailbox = mailbox.monitorable();

    loop {
        match mailbox.receive() {
            MessageSignal::Message(Relay::Watch(watcher)) => mailbox.monitor(watcher),
            MessageSignal::Message(Relay::Report(outcome)) => return caller.tag_send(tag, outcome),
            // The caller cancels the watcher itself.
            MessageSignal::Message(Relay::Cancel) => return,
            MessageSignal::Signal(ProcessDiedSignal { .. }) => return caller.tag_send(tag, Err(Failed::Lost)),
        }
    }
}

// Runs on the job's node: spawns the job linked to itself once the caller sends its arguments, and reports one outcome
// to the relay.
fn watch<J: JobFn>(relay: Process<RelayOf<J>>, mailbox: Mailbox<WatchOf<J>>) {
    let mailbox = mailbox.catch_link_failure();
    let mut job = None;

    loop {
        match mailbox.receive() {
            MessageSignal::Message(Watch::Start(args)) => {
                job = Some(Process::spawn_link_config(&config::<J>(), (mailbox.this(), args), run::<J>));
            }
            MessageSignal::Message(Watch::Done(output)) => return relay.send(Relay::Report(Ok(output))),
            MessageSignal::Message(Watch::Cancel) => {
                // The caller sends the arguments before it can cancel, so the job always exists here.
                if let Some(job) = job {
                    job.kill();
                }
                return;
            }
            MessageSignal::Signal(LinkDiedSignal(_)) => return relay.send(Relay::Report(Err(Failed::Died))),
        }
    }
}

// Runs the job itself.
fn run<J: JobFn>((watcher, args): (Process<WatchOf<J>>, J::Args), _: Mailbox<()>) {
    watcher.send(Watch::Done(J::call(args)));
}
