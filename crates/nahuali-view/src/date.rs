//! Just enough calendar arithmetic for display: civil dates, minutes and
//! RFC 3339 prefixes. Time zones are the source's job: every [`Stamp`] the
//! view model receives is already local time.

use std::fmt;

const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTHS_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// A civil date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub const fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Parse `YYYY-MM-DD`, also accepting a longer RFC 3339 string.
    pub fn parse(text: &str) -> Option<Self> {
        let year = text.get(0..4)?.parse().ok()?;
        if text.get(4..5)? != "-" || text.get(7..8)? != "-" {
            return None;
        }
        let month: u8 = text.get(5..7)?.parse().ok()?;
        let day: u8 = text.get(8..10)?.parse().ok()?;
        let date = Self::new(year, month, day);
        ((1..=12).contains(&month) && day >= 1 && day <= date.days_in_month()).then_some(date)
    }

    fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    pub fn days_in_month(self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ if Self::is_leap(self.year) => 29,
            _ => 28,
        }
    }

    /// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
    pub fn ordinal(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The inverse of [`Date::ordinal`].
    pub fn from_ordinal(days: i64) -> Self {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self::new(year, month, day)
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_ordinal(self.ordinal() + days)
    }

    /// Whole days from `self` to `other` (positive when `other` is later).
    pub fn days_until(self, other: Self) -> i64 {
        other.ordinal() - self.ordinal()
    }

    /// Monday is 0, Sunday is 6.
    pub fn weekday(self) -> u8 {
        // 1970-01-01 was a Thursday (index 3).
        ((self.ordinal() + 3).rem_euclid(7)) as u8
    }

    pub fn weekday_name(self) -> &'static str {
        WEEKDAYS[usize::from(self.weekday())]
    }

    pub fn month_name(self) -> &'static str {
        MONTHS[usize::from(self.month - 1)]
    }

    pub fn month_long(self) -> &'static str {
        MONTHS_LONG[usize::from(self.month - 1)]
    }

    pub fn first_of_month(self) -> Self {
        Self::new(self.year, self.month, 1)
    }

    /// `Nov 5`.
    pub fn short(self) -> String {
        format!("{} {}", self.month_name(), self.day)
    }

    /// `Wed 4 Nov`.
    pub fn weekday_short(self) -> String {
        format!("{} {} {}", self.weekday_name(), self.day, self.month_name())
    }

    /// `Wed 4 Nov 2026`.
    pub fn long(self) -> String {
        format!("{} {}", self.weekday_short(), self.year)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A local date and minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    pub date: Date,
    pub hour: u8,
    pub minute: u8,
}

impl Stamp {
    pub const fn new(date: Date, hour: u8, minute: u8) -> Self {
        Self { date, hour, minute }
    }

    /// Parse the `YYYY-MM-DDTHH:MM` prefix of an RFC 3339 string.
    pub fn parse(text: &str) -> Option<Self> {
        let date = Date::parse(text)?;
        let rest = text.get(10..)?;
        if rest.is_empty() {
            return Some(Self::new(date, 0, 0));
        }
        let sep = rest.get(0..1)?;
        if sep != "T" && sep != " " {
            return None;
        }
        let hour: u8 = rest.get(1..3)?.parse().ok()?;
        let minute: u8 = rest.get(4..6)?.parse().ok()?;
        (hour < 24 && minute < 60).then_some(Self::new(date, hour, minute))
    }

    pub fn minutes(self) -> i64 {
        self.date.ordinal() * 1440 + i64::from(self.hour) * 60 + i64::from(self.minute)
    }

    /// `18:40`.
    pub fn clock(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// Compact age relative to `now`: `now`, `12m`, `3h`, `2d`, `5w`.
    pub fn age(self, now: Stamp) -> String {
        let minutes = (now.minutes() - self.minutes()).max(0);
        match minutes {
            // Every screen says "… ago": less than a minute is "moments ago".
            0 => "moments".to_string(),
            1..=59 => format!("{minutes}m"),
            60..=1439 => format!("{}h", minutes / 60),
            1440..=20159 => format!("{}d", minutes / 1440),
            _ => format!("{}w", minutes / 10080),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_round_trips() {
        for days in [-1_000_000, -1, 0, 1, 20_397, 1_000_000] {
            assert_eq!(Date::from_ordinal(days).ordinal(), days);
        }
        assert_eq!(Date::new(1970, 1, 1).ordinal(), 0);
    }

    #[test]
    fn weekdays_match_the_calendar() {
        assert_eq!(Date::new(2026, 11, 5).weekday_name(), "Thu");
        assert_eq!(Date::new(2026, 11, 1).weekday_name(), "Sun");
        assert_eq!(Date::new(2024, 2, 29).weekday_name(), "Thu");
    }

    #[test]
    fn parses_rfc3339_prefixes() {
        let stamp = Stamp::parse("2026-11-05T16:52:10Z").unwrap();
        assert_eq!(stamp, Stamp::new(Date::new(2026, 11, 5), 16, 52));
        assert_eq!(Date::parse("2026-02-30"), None);
        assert_eq!(Date::parse("2026/11/05"), None);
        assert_eq!(Stamp::parse("2026-11-05T25:00"), None);
    }

    #[test]
    fn ages_are_compact() {
        let now = Stamp::new(Date::new(2026, 11, 5), 18, 40);
        assert_eq!(Stamp::new(Date::new(2026, 11, 5), 18, 38).age(now), "2m");
        assert_eq!(Stamp::new(Date::new(2026, 11, 5), 4, 40).age(now), "14h");
        assert_eq!(Stamp::new(Date::new(2026, 11, 2), 18, 40).age(now), "3d");
        assert_eq!(Date::new(2026, 11, 2).days_until(now.date), 3);
    }

    #[test]
    fn an_age_reads_well_before_the_word_ago() {
        let at = Stamp::new(Date::new(2026, 11, 12), 10, 20);
        assert_eq!(at.age(at), "moments");
        assert_eq!(at.age(Stamp::new(Date::new(2026, 11, 12), 10, 32)), "12m");
    }
}
