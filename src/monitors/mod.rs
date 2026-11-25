//! Monitors: a quantity a project declares, read on its own cadence and kept,
//! whose drop is a matter (§FS-021-monitors).
//!
//! What lives here is the pure core: every rule of the specification that
//! needs no file, no process and no clock read. Each function that depends on
//! the time takes `now`, and each that depends on the local clock takes a
//! [`chrono::TimeZone`], so the same rules run under test with a fixed offset
//! or a zone that changes its clocks. Reading files, running commands and the
//! store's locks live above it (§AR-001-layers.1).
//!
//! - [`model`] is what a monitor and a sample are, in the published shapes;
//! - [`shape`] reads one declaration or one sample from its JSON, with the
//!   sentence a refusal shows;
//! - [`condition`] judges a value, and [`words`] says a condition and a value
//!   the way the page, the command and a matter's title say them;
//! - [`overlay`] resolves a monitor from its homes, field by field, and the
//!   rules that hold only on the whole;
//! - [`cadence`], [`slots`] and [`schedule`] say when a monitor is read, and
//!   [`freshness`] how long what it read holds;
//! - [`sample`] checks one sample against its monitor, and [`route`] gives an
//!   answer's samples to the monitors they are for;
//! - [`history`] is a reading's line and the history judged again for
//!   display, and [`retention`] what of it is kept.

pub mod cadence;
pub mod condition;
pub mod freshness;
pub mod history;
pub mod model;
pub mod overlay;
pub mod retention;
pub mod route;
pub mod sample;
pub mod schedule;
pub mod shape;
pub mod slots;
pub mod words;

#[cfg(test)]
mod test_zone;
