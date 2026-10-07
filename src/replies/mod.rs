//! Site-owned reply provenance and recovery (§FS-005-dispatch.13).
//! Descriptors stay opaque: only the carrier interprets a target.

pub mod binding;
pub mod storage;

pub use binding::Binding;
pub use storage::{Intent, Record, Status, Store};
