//! A zone that changes its clocks, for the tests of the rules that read the
//! local clock (§FS-021-monitors.6.3).
//!
//! Central European time in 2026: UTC+1, and UTC+2 from 29 March 01:00 UTC to
//! 25 October 01:00 UTC. So 02:00–03:00 on 29 March does not exist, and
//! 02:00–03:00 on 25 October happens twice.

use chrono::{DateTime, FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

#[derive(Debug, Clone, Copy)]
pub struct Shifting;

fn winter() -> FixedOffset {
    FixedOffset::east_opt(3600).unwrap()
}

fn summer() -> FixedOffset {
    FixedOffset::east_opt(7200).unwrap()
}

/// An instant written as UTC, `2026-03-29T01:00:00Z`.
pub fn utc(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

impl TimeZone for Shifting {
    type Offset = FixedOffset;

    fn from_offset(_: &FixedOffset) -> Self {
        Shifting
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        self.offset_from_local_datetime(&local.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        // Summer first: where a time happens twice, its summer reading is the
        // earlier instant.
        let fits: Vec<FixedOffset> = [summer(), winter()]
            .into_iter()
            .filter(|offset| {
                let instant = *local - *offset;
                self.offset_from_utc_datetime(&instant) == *offset
            })
            .collect();
        match fits.as_slice() {
            [] => MappedLocalTime::None,
            [one] => MappedLocalTime::Single(*one),
            [first, second, ..] => MappedLocalTime::Ambiguous(*first, *second),
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        let spring = self::utc("2026-03-29T01:00:00Z").naive_utc();
        let autumn = self::utc("2026-10-25T01:00:00Z").naive_utc();
        if (spring..autumn).contains(utc) {
            summer()
        } else {
            winter()
        }
    }
}
