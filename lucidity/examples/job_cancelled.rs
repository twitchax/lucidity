//! Dropping a job before it finishes kills the job's process.
//!
//! Run it inside a `lunatic node` with a peer (see the `pythagorean` example): it should print "cancelled: ok".

use std::time::Duration;

use lucidity::lunatic::{Mailbox, MessageSignal, Process, ProcessDiedSignal};

fn main() {
    // SAFETY: the job sends this process its own process, untagged; the job's outcome comes under the job's tag.
    let mailbox = unsafe { Mailbox::<Process<()>>::new() }.monitorable();
    let this = unsafe { Process::<Process<()>>::this() };

    let job = linger_remote_async(this);
    let MessageSignal::Message(lingering) = mailbox.receive() else {
        panic!("unreachable: a signal before the job started");
    };
    mailbox.monitor(lingering);
    drop(job);

    loop {
        if let MessageSignal::Signal(ProcessDiedSignal { node_id, process_id }) = mailbox.receive() {
            if (node_id, process_id) == (lingering.node_id(), lingering.id()) {
                break;
            }
        }
    }
    println!("cancelled: ok");
}

/// Sends `caller` its own process, and sleeps long enough to outlast any test.
#[lucidity::job]
fn linger(caller: Process<Process<()>>) -> u64 {
    // SAFETY: nothing is sent to the job's process.
    caller.send(unsafe { Process::<()>::this() });
    lucidity::lunatic::sleep(Duration::from_secs(600));
    0
}
