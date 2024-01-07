//! The `lucidity-core` crate.  Abstracts away core functionality, and functionality for build scripts.

#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

use core::time::Duration;

use lunatic::{ap::ProcessRef, AbstractProcess};

pub use lunatic;
pub use rand;
pub use serde;

/// A job is a process that can be spawned and shutdown.
///
/// This type is usually created with the [`lucidity::job`] macro on the async methods.
pub struct Job<T>
where
    T: AbstractProcess,
{
    /// The process reference.
    pub process: ProcessRef<T>,
}

impl<T> Drop for Job<T>
where
    T: AbstractProcess,
{
    fn drop(&mut self) {
        loop {
            if let Ok(r) = self.process.with_timeout(Duration::from_millis(100)).shutdown() {
                break r;
            }
        }
    }
}
