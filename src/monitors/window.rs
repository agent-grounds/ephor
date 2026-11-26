//! The Monitors page's window (§FS-021-monitors.18.8): `6h`, `1d`, `7d`,
//! `30d`, `90d` or `1y`, `7d` unless the reader says otherwise. Its own type
//! rather than the Burn page's, whose windows are another set
//! (§FS-013-burn.6).

use chrono::TimeDelta;
use serde::Serialize;

/// How far back the strip, the change and the chart look
/// (§FS-021-monitors.18.8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
pub enum Window {
    #[value(name = "6h")]
    #[serde(rename = "6h")]
    SixHours,
    #[value(name = "1d")]
    #[serde(rename = "1d")]
    Day,
    #[default]
    #[value(name = "7d")]
    #[serde(rename = "7d")]
    Week,
    #[value(name = "30d")]
    #[serde(rename = "30d")]
    Month,
    #[value(name = "90d")]
    #[serde(rename = "90d")]
    Quarter,
    #[value(name = "1y")]
    #[serde(rename = "1y")]
    Year,
}

impl Window {
    /// Every window, in the order `w` cycles them.
    pub const ALL: [Window; 6] = [
        Window::SixHours,
        Window::Day,
        Window::Week,
        Window::Month,
        Window::Quarter,
        Window::Year,
    ];

    /// The window as the header and the argument say it.
    pub fn label(self) -> &'static str {
        match self {
            Window::SixHours => "6h",
            Window::Day => "1d",
            Window::Week => "7d",
            Window::Month => "30d",
            Window::Quarter => "90d",
            Window::Year => "1y",
        }
    }

    /// How long it is.
    pub fn span(self) -> TimeDelta {
        match self {
            Window::SixHours => TimeDelta::hours(6),
            Window::Day => TimeDelta::days(1),
            Window::Week => TimeDelta::days(7),
            Window::Month => TimeDelta::days(30),
            Window::Quarter => TimeDelta::days(90),
            Window::Year => TimeDelta::days(365),
        }
    }

    /// The next window round the cycle: what `w` does, and nothing more
    /// (§FS-021-monitors.18.10).
    pub fn next(self) -> Window {
        let at = Window::ALL
            .iter()
            .position(|window| *window == self)
            .unwrap_or(0);
        Window::ALL[(at + 1) % Window::ALL.len()]
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
