//! The `lucidity` crate.  Re-exports the core components and provides a macro for creating jobs.
#![doc = include_str!("../../README.md")]
#![warn(rustdoc::broken_intra_doc_links, rust_2018_idioms, clippy::all, missing_docs)]

pub use lucidity_core::Job;
pub use lucidity_macros::job;

pub use lucidity_core::lunatic;
//pub use lucidity_core::lunatic_log;
pub use lucidity_core::rand;
pub use lucidity_core::serde;

pub use lucidity_core::lunatic::abstract_process;

#[cfg(feature = "fly")]
pub mod fly;
