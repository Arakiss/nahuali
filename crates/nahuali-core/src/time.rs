//! A moment, kept as seconds since 1970 in UTC and written as RFC 3339 so a
//! person can read the log.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub u64);

impl Time {
    pub fn from_millis(ms: u64) -> Self {
        Self(ms / 1000)
    }

    /// `2026-11-12T15:20:11Z`.
    pub fn rfc3339(self) -> String {
        let (days, rest) = (self.0 / 86_400, self.0 % 86_400);
        let (y, m, d) = civil(days as i64);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
    }

    /// Parse what [`Time::rfc3339`] writes. Only UTC, only whole seconds.
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() != 20
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
            || b[19] != b'Z'
        {
            return None;
        }
        let n = |from: usize, to: usize| text.get(from..to)?.parse::<u64>().ok();
        let (y, mo, d, h, mi, s) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
        if y < 1970 || !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
            return None;
        }
        let days = days_from_civil(y as i64, mo as i64, d as i64);
        Some(Self(days as u64 * 86_400 + h * 3600 + mi * 60 + s))
    }

    /// Seconds from `self` to `later`, zero when `later` is earlier.
    pub fn until(self, later: Self) -> u64 {
        later.0.saturating_sub(self.0)
    }

    /// `just now`, `12 min ago`, `3 h ago`, `2 d ago`.
    pub fn ago(self, now: Self) -> String {
        match self.until(now) {
            s if s < 60 => "just now".to_string(),
            s if s < 3600 => format!("{} min ago", s / 60),
            s if s < 86_400 => format!("{} h ago", s / 3600),
            s => format!("{} d ago", s / 86_400),
        }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.rfc3339())
    }
}

impl Serialize for Time {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.rfc3339())
    }
}

impl<'de> Deserialize<'de> for Time {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text)
            .ok_or_else(|| serde::de::Error::custom("a time is written like 2026-11-12T15:20:11Z"))
    }
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_reads_back_as_it_was_written() {
        for secs in [0, 59, 86_399, 86_400, 951_782_400, 1_794_496_811, 4_102_444_799] {
            let text = Time(secs).rfc3339();
            assert_eq!(Time::parse(&text), Some(Time(secs)), "{text}");
        }
        assert_eq!(Time(951_782_400).rfc3339(), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn anything_else_is_not_a_time() {
        for text in ["", "2026-11-12", "2026-11-12T15:20:11+01:00", "2026-13-01T00:00:00Z", "yesterday"] {
            assert_eq!(Time::parse(text), None, "{text}");
        }
    }

    #[test]
    fn ages_are_said_in_the_largest_unit_that_fits() {
        let now = Time(1_000_000);
        assert_eq!(Time(999_990).ago(now), "just now");
        assert_eq!(Time(999_280).ago(now), "12 min ago");
        assert_eq!(Time(1_000_000 - 3 * 3600).ago(now), "3 h ago");
        assert_eq!(Time(1_000_000 - 2 * 86_400).ago(now), "2 d ago");
        assert_eq!(Time(1_000_500).ago(now), "just now", "a clock that runs ahead is not an error");
    }
}
