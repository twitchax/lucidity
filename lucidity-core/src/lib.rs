//! The `lucidity-core` crate.  Abstracts away core functionality, and functionality for build scripts.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use core::time::Duration;

use lunatic::{AbstractProcess, ap::ProcessRef};

pub use lunatic;
//pub use lunatic_log;
pub use serde;
pub use rand;

/// A job is a process that can be spawned and shutdown.
/// 
/// This type is usually created with the [`lucidity::job`] macro on the async methods.
pub struct Job<T> where T: AbstractProcess {
    /// The process reference.
    pub process: ProcessRef<T>,
}

impl<T> Drop for Job<T> where T: AbstractProcess {
    fn drop(&mut self) {
        //use lunatic::host::{node_id, process_id};
        //println!("[{},{}] drop", node_id(), process_id());

        loop {
            if let Ok(r) = self.process.with_timeout(Duration::from_millis(100)).shutdown() {
                break r;
            }
        } 
    }
}