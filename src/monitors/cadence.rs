//! A monitor's cadence: `every`, `at`, their defaults and how they pair
//! (§FS-021-monitors.6.1, §FS-021-monitors.6.2), and the lengths counted from
//! its interval: the grace (§FS-021-monitors.7.1) and the hold's default
//! (§FS-021-monitors.10.1).

use chrono::{NaiveTime, TimeDelta, Weekday};
use serde::{Deserialize, Serialize};

use crate::interval::{self, Unit, Unread};

/// Every unit a monitor's `every` may be written in (§FS-021-monitors.6.1).
const UNITS: &[Unit] = &[Unit::Minute, Unit::Hour, Unit::Day, Unit::Week];

/// How long a command may run before it is stopped, where no monitor it reads
/// says otherwise (§FS-021-monitors.5.5).
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

/// Where `at` pins the slots: a time of the local clock, and a day of the
/// week where it names one (§FS-021-monitors.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct At {
    pub day: Option<Weekday>,
    pub time: NaiveTime,
}

impl At {
    /// `HH:MM`, or a day and a time such as `Mon 06:00`, with the days `Mon`
    /// to `Sun` (§FS-021-monitors.6.1). Nothing else reads, so a pattern the
    /// schema refuses is refused here too.
    pub fn parse(text: &str) -> Option<At> {
        let (day, clock) = match text.split_once(' ') {
            Some((day, clock)) => (Some(weekday(day)?), clock),
            None => (None, text),
        };
        let (hours, minutes) = clock.split_once(':')?;
        if hours.len() != 2 || minutes.len() != 2 {
            return None;
        }
        if !hours
            .chars()
            .chain(minutes.chars())
            .all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let time = NaiveTime::from_hms_opt(hours.parse().ok()?, minutes.parse().ok()?, 0)?;
        Some(At { day, time })
    }

    /// Back in the spelling it is written in.
    pub fn say(&self) -> String {
        let clock = self.time.format("%H:%M");
        match self.day {
            Some(day) => format!("{} {clock}", weekday_name(day)),
            None => clock.to_string(),
        }
    }
}

fn weekday(word: &str) -> Option<Weekday> {
    Some(match word {
        "Mon" => Weekday::Mon,
        "Tue" => Weekday::Tue,
        "Wed" => Weekday::Wed,
        "Thu" => Weekday::Thu,
        "Fri" => Weekday::Fri,
        "Sat" => Weekday::Sat,
        "Sun" => Weekday::Sun,
        _ => return None,
    })
}

fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

/// A monitor's effective cadence: how often it is read, and where `at` pins
/// its slots (§FS-021-monitors.6.2). Kept in the store as it was written, so
/// a change of it is seen and the next slot counted again
/// (§FS-021-monitors.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "Spelled", try_from = "Spelled")]
pub struct Cadence {
    pub every: TimeDelta,
    pub at: Option<At>,
}

impl Cadence {
    /// The cadence of a monitor that states none: every ten minutes, which is
    /// also how a custom-status answer's samples are judged
    /// (§FS-021-monitors.6.1, §FS-021-monitors.6.8).
    pub fn default_every() -> Cadence {
        Cadence {
            every: TimeDelta::minutes(10),
            at: None,
        }
    }

    /// The effective cadence from what the homes state, with the defaults
    /// filled in and the pairing held (§FS-021-monitors.6.1): `10m`, or
    /// `1d` with an `at`, or `1w` where that `at` names a day. A pairing whose
    /// slots would wander off the clock `at` pins them to is refused with the
    /// sentence the page shows.
    pub fn resolve(every: Option<&str>, at: Option<&str>) -> Result<Cadence, String> {
        let at = match at {
            Some(text) => Some(At::parse(text).ok_or_else(|| {
                format!("at '{text}' is not a time of day: HH:MM, or a day and a time such as Mon 06:00")
            })?),
            None => None,
        };
        let every = match every {
            Some(text) => read_every(text)?,
            None => match at {
                Some(At { day: Some(_), .. }) => TimeDelta::weeks(1),
                Some(At { day: None, .. }) => TimeDelta::days(1),
                None => TimeDelta::minutes(10),
            },
        };
        let cadence = Cadence { every, at };
        let minutes = every.num_minutes();
        match at {
            Some(pin @ At { day: Some(_), .. }) if minutes % (7 * 1440) != 0 => Err(format!(
                "at {} needs an every of whole weeks, and {} is not one",
                pin.say(),
                cadence.say_every()
            )),
            Some(pin @ At { day: None, .. }) if 1440 % minutes != 0 && minutes % 1440 != 0 => {
                Err(format!(
                    "at {} needs an every that divides a day evenly or is whole days, \
                     and {} is neither",
                    pin.say(),
                    cadence.say_every()
                ))
            }
            _ => Ok(cadence),
        }
    }

    /// A monitor's interval: its effective `every`, which no change of clocks
    /// changes (§FS-021-monitors.6.3).
    pub fn interval(&self) -> TimeDelta {
        self.every
    }

    /// How long past its next slot a value still holds: a quarter of the
    /// interval, and at least an hour (§FS-021-monitors.7.1).
    pub fn grace(&self) -> TimeDelta {
        (self.every / 4).max(TimeDelta::hours(1))
    }

    /// How many passing readings in a row end a drop where the monitor does
    /// not say: two under an hour, so one good reading in a flap does not
    /// close the matter, and one at an hour or longer
    /// (§FS-021-monitors.10.1).
    pub fn recover_after(&self) -> u32 {
        if self.every < TimeDelta::hours(1) {
            2
        } else {
            1
        }
    }

    /// `every` in the shortest whole unit, such as `10m`, `4h`, `1d`, `2w`.
    pub fn say_every(&self) -> String {
        spell(self.every)
    }

    /// The cadence as the detail view says it: `every 1d at 06:00`
    /// (§FS-021-monitors.19).
    pub fn say(&self) -> String {
        match self.at {
            Some(at) => format!("every {} at {}", self.say_every(), at.say()),
            None => format!("every {}", self.say_every()),
        }
    }

    /// The cadence column's word: `10m`, `1h`, `4h`, `8h`, `12h`, `daily` or
    /// `weekly`, and any other as its duration (§FS-021-monitors.18.4).
    pub fn column(&self) -> String {
        if self.every == TimeDelta::days(1) {
            "daily".to_string()
        } else if self.every == TimeDelta::weeks(1) {
            "weekly".to_string()
        } else {
            self.say_every()
        }
    }
}

/// `every` read as a length (§FS-021-monitors.6.1). The shape has already
/// held it to a whole number from 1 and a unit; what is left to refuse is a
/// length the calendar cannot hold.
pub fn read_every(text: &str) -> Result<TimeDelta, String> {
    interval::read(text, UNITS)
        .map_err(|unread| match unread {
            Unread::TooLong => format!("every {text} is longer than anything can wait"),
            Unread::NoUnit | Unread::NotACount | Unread::Negative => format!(
            "every {text} is not a duration: a whole number and m, h, d or w, such as 10m or 1d"
        ),
        })
        .and_then(|every| {
            if every > TimeDelta::zero() {
                Ok(every)
            } else {
                Err(format!(
                    "every {text} is no time at all; a monitor is read at most every minute"
                ))
            }
        })
}

/// A length in the largest unit that divides it.
pub fn spell(every: TimeDelta) -> String {
    let minutes = every.num_minutes();
    for unit in [Unit::Week, Unit::Day, Unit::Hour] {
        if minutes != 0 && minutes % unit.minutes() == 0 {
            return format!("{}{}", minutes / unit.minutes(), unit.suffix());
        }
    }
    format!("{minutes}m")
}

/// The longest limit among the monitors one run reads, so a run that reads
/// several gets the longest any of them allows (§FS-021-monitors.5.5).
pub fn run_timeout(timeouts: impl IntoIterator<Item = u64>) -> u64 {
    timeouts
        .into_iter()
        .max()
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
}

/// The fastest cadence among those given: the cadence of a `from`'s answer
/// for the monitors read from the answer that brings them
/// (§FS-021-monitors.6.8).
pub fn fastest(cadences: impl IntoIterator<Item = Cadence>) -> Option<Cadence> {
    cadences.into_iter().min_by_key(|cadence| cadence.every)
}

/// The cadence as the store keeps it: as written.
#[derive(Serialize, Deserialize)]
struct Spelled {
    every: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    at: Option<String>,
}

impl From<Cadence> for Spelled {
    fn from(cadence: Cadence) -> Spelled {
        Spelled {
            every: cadence.say_every(),
            at: cadence.at.map(|at| at.say()),
        }
    }
}

impl TryFrom<Spelled> for Cadence {
    type Error = String;

    fn try_from(spelled: Spelled) -> Result<Cadence, String> {
        Cadence::resolve(Some(&spelled.every), spelled.at.as_deref())
    }
}

#[cfg(test)]
#[path = "cadence_tests.rs"]
mod tests;
