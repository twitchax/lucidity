//! The `lucidity-core` crate.  Abstracts away core functionality, and functionality for build scripts.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use std::{cell::RefCell, time::Duration};

use lunatic::{LinkDiedSignal, Mailbox, MessageSignal, Process, ProcessConfig, Tag};
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

/// What a watcher reports back to the caller.
type Outcome<T> = Result<T, Failed>;

/// The job's process died before producing a result.
#[derive(Serialize, Deserialize)]
struct Failed;

/// What a watcher receives.
#[derive(Serialize, Deserialize)]
enum Watch<T> {
    Done(T),
    Cancel,
}

enum State<T> {
    Pending,
    Done(T),
}

/// A job running in its own process, possibly on another node.
///
/// A job is spawned as a small watcher process on the target node, which spawns the job's process linked to itself
/// on that same node (links don't cross nodes) and reports exactly one outcome back to the caller's mailbox, under
/// a tag unique to this job: the result, or that the job's process died.  The caller never polls or retries, so a
/// slow job is only ever slow, and a dead one panics the caller instead of hanging it.
///
/// Dropping a job before its result arrives cancels it.
pub struct Job<J: JobFn> {
    node: u64,
    watcher: Process<Watch<J::Output>>,
    tag: Tag,
    state: RefCell<State<J::Output>>,
}

impl<J: JobFn> Job<J> {
    /// Spawns the job on `node` (which may be this one).
    pub fn spawn(node: u64, args: J::Args) -> Self {
        let tag = Tag::new();
        // SAFETY: only `Outcome<J::Output>` messages are ever sent under `tag`, and they're only received as such.
        let caller = unsafe { Process::<Outcome<J::Output>>::this() };
        let watcher = Process::spawn_node_config(node, &config::<J>(), (caller, tag, args), watch::<J>);

        Self {
            node,
            watcher,
            tag,
            state: RefCell::new(State::Pending),
        }
    }

    /// Returns the result if the job has finished, without blocking.
    ///
    /// Panics if the job's process died.
    pub fn try_get(&self) -> Option<J::Output> {
        self.receive(Some(Duration::ZERO));

        match &*self.state.borrow() {
            State::Done(output) => Some(output.clone()),
            State::Pending => None,
        }
    }

    /// Blocks until the job finishes, and returns its result.
    ///
    /// Panics if the job's process died.
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
            Err(Failed) => panic!("lucidity: job `{}` on node {} died before returning a result", J::NAME, self.node),
        }
    }
}

impl<J: JobFn> Drop for Job<J> {
    fn drop(&mut self) {
        if let State::Pending = *self.state.borrow() {
            // The watcher may have just exited after sending the outcome; the message is then dropped.
            self.watcher.send(Watch::Cancel);
        }
    }
}

/// Spawns one job per set of arguments, on nodes picked by `fanout`, and blocks until all of them finish.
///
/// The results are in the same order as the arguments.  Panics if any job's process died.
pub fn fanout<J: JobFn>(args: Vec<J::Args>, fanout: Fanout) -> Vec<J::Output> {
    let nodes = remote_nodes();

    let jobs = args
        .into_iter()
        .enumerate()
        .map(|(k, args)| {
            let node = match fanout {
                Fanout::RoundRobin => nodes[k % nodes.len()],
                Fanout::Random => pick(&nodes),
            };

            Job::<J>::spawn(node, args)
        })
        .collect::<Vec<_>>();

    jobs.iter().map(Job::await_get).collect()
}

/// The id of the node this process runs on.
pub fn local_node() -> u64 {
    lunatic::host::node_id()
}

/// A random node from the distributed pool.
///
/// Panics if no other nodes are connected.
pub fn random_node() -> u64 {
    pick(&remote_nodes())
}

fn remote_nodes() -> Vec<u64> {
    let nodes = lunatic::distributed::nodes();
    assert!(!nodes.is_empty(), "lucidity: no remote nodes are connected");
    nodes
}

fn pick(nodes: &[u64]) -> u64 {
    nodes[rand::rng().random_range(0..nodes.len())]
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

// Runs on the job's node: spawns the job linked to itself, and forwards one outcome to the caller.
fn watch<J: JobFn>((caller, tag, args): (Process<Outcome<J::Output>>, Tag, J::Args), mailbox: Mailbox<Watch<J::Output>>) {
    let mailbox = mailbox.catch_link_failure();
    let job = Process::spawn_link_config(&config::<J>(), (mailbox.this(), args), run::<J>);

    match mailbox.receive() {
        MessageSignal::Message(Watch::Done(output)) => caller.tag_send(tag, Ok(output)),
        MessageSignal::Message(Watch::Cancel) => job.kill(),
        MessageSignal::Signal(LinkDiedSignal(_)) => caller.tag_send(tag, Err(Failed)),
    }
}

// Runs the job itself.
fn run<J: JobFn>((watcher, args): (Process<Watch<J::Output>>, J::Args), _: Mailbox<()>) {
    watcher.send(Watch::Done(J::call(args)));
}
