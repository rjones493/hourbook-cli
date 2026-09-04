//! Parsing and summarizing for a plain-text timesheet format.
//!
//! One entry per line:
//!
//!     2026-08-25 09:00-12:30 acme setup and standup
//!     2026-08-25 13:00-17:15 acme feature work
//!
//! `DATE START-END PROJECT [NOTE]`. Blank lines and lines starting with
//! `#` are ignored so timesheets can carry comments.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    BadFormat,
    BadDate(String),
    BadTime(String),
    ZeroDuration,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::BadFormat => write!(f, "expected 'DATE START-END PROJECT [NOTE]'"),
            ParseError::BadDate(s) => write!(f, "bad date '{}'", s),
            ParseError::BadTime(s) => write!(f, "bad time '{}'", s),
            ParseError::ZeroDuration => write!(f, "start and end time are the same"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub fn parse(s: &str) -> Result<Date, ParseError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return Err(ParseError::BadDate(s.to_string()));
        }
        let year: u16 = parts[0].parse().map_err(|_| ParseError::BadDate(s.to_string()))?;
        let month: u8 = parts[1].parse().map_err(|_| ParseError::BadDate(s.to_string()))?;
        let day: u8 = parts[2].parse().map_err(|_| ParseError::BadDate(s.to_string()))?;
        if month == 0 || month > 12 {
            return Err(ParseError::BadDate(s.to_string()));
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(ParseError::BadDate(s.to_string()));
        }
        Ok(Date { year, month, day })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
}

impl Time {
    pub fn parse(s: &str) -> Result<Time, ParseError> {
        let (h, m) = s.split_once(':').ok_or_else(|| ParseError::BadTime(s.to_string()))?;
        let hour: u8 = h.parse().map_err(|_| ParseError::BadTime(s.to_string()))?;
        let minute: u8 = m.parse().map_err(|_| ParseError::BadTime(s.to_string()))?;
        if hour > 23 || minute > 59 {
            return Err(ParseError::BadTime(s.to_string()));
        }
        Ok(Time { hour, minute })
    }

    pub fn minutes_since_midnight(&self) -> u32 {
        self.hour as u32 * 60 + self.minute as u32
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub date: Date,
    pub start: Time,
    pub end: Time,
    pub project: String,
    pub note: Option<String>,
}

const MINUTES_PER_DAY: u32 = 24 * 60;

impl Entry {
    /// Minutes worked. An end time at or before the start time means the
    /// shift ran past midnight, so it's counted through to that time on
    /// the following day.
    pub fn duration_minutes(&self) -> u32 {
        let start = self.start.minutes_since_midnight();
        let end = self.end.minutes_since_midnight();
        if end > start {
            end - start
        } else {
            (MINUTES_PER_DAY - start) + end
        }
    }
}

/// Parses one line of the timesheet format. Callers own comment/blank
/// filtering, since that varies with the source (a file vs. piped stdin).
pub fn parse_line(line: &str) -> Result<Entry, ParseError> {
    let mut fields = line.split_whitespace();
    let date_str = fields.next().ok_or(ParseError::BadFormat)?;
    let range_str = fields.next().ok_or(ParseError::BadFormat)?;
    let project = fields.next().ok_or(ParseError::BadFormat)?.to_string();
    let rest: Vec<&str> = fields.collect();
    let note = if rest.is_empty() { None } else { Some(rest.join(" ")) };

    let date = Date::parse(date_str)?;

    let (start_str, end_str) = range_str
        .split_once('-')
        .ok_or(ParseError::BadFormat)?;
    let start = Time::parse(start_str)?;
    let end = Time::parse(end_str)?;
    if start == end {
        return Err(ParseError::ZeroDuration);
    }

    Ok(Entry { date, start, end, project, note })
}

/// Keeps only entries whose date falls within `[from, to]`. Either bound
/// can be omitted to leave that side of the range open.
pub fn filter_by_date_range(entries: &[Entry], from: Option<Date>, to: Option<Date>) -> Vec<Entry> {
    entries
        .iter()
        .filter(|e| match from {
            Some(d) => e.date >= d,
            None => true,
        })
        .filter(|e| match to {
            Some(d) => e.date <= d,
            None => true,
        })
        .cloned()
        .collect()
}

/// Total minutes worked per project, in project name order.
pub fn summarize_by_project(entries: &[Entry]) -> BTreeMap<String, u32> {
    let mut totals: BTreeMap<String, u32> = BTreeMap::new();
    for entry in entries {
        *totals.entry(entry.project.clone()).or_insert(0) += entry.duration_minutes();
    }
    totals
}

/// Total minutes worked per day, in date order.
///
/// An entry that crosses midnight is attributed to its start date, not
/// split across the two days it touches — that keeps this a straight sum
/// of `duration_minutes()` and matches how someone would log it by hand.
pub fn summarize_by_day(entries: &[Entry]) -> BTreeMap<Date, u32> {
    let mut totals: BTreeMap<Date, u32> = BTreeMap::new();
    for entry in entries {
        *totals.entry(entry.date).or_insert(0) += entry.duration_minutes();
    }
    totals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_entry() {
        let entry = parse_line("2026-08-25 09:00-12:30 acme morning standup").unwrap();
        assert_eq!(entry.date, Date { year: 2026, month: 8, day: 25 });
        assert_eq!(entry.start, Time { hour: 9, minute: 0 });
        assert_eq!(entry.end, Time { hour: 12, minute: 30 });
        assert_eq!(entry.project, "acme");
        assert_eq!(entry.note.as_deref(), Some("morning standup"));
        assert_eq!(entry.duration_minutes(), 210);
    }

    #[test]
    fn note_is_optional() {
        let entry = parse_line("2026-08-25 09:00-10:00 acme").unwrap();
        assert!(entry.note.is_none());
    }

    #[test]
    fn rejects_zero_duration() {
        let err = parse_line("2026-08-25 09:00-09:00 acme").unwrap_err();
        assert!(matches!(err, ParseError::ZeroDuration));
    }

    #[test]
    fn crosses_midnight() {
        let entry = parse_line("2026-08-25 22:00-02:00 acme night shift").unwrap();
        assert_eq!(entry.duration_minutes(), 240);
    }

    #[test]
    fn crosses_midnight_at_the_boundary() {
        let entry = parse_line("2026-08-25 23:59-00:01 acme").unwrap();
        assert_eq!(entry.duration_minutes(), 2);
    }

    #[test]
    fn rejects_feb_30() {
        assert!(Date::parse("2026-02-30").is_err());
    }

    #[test]
    fn accepts_leap_day() {
        assert!(Date::parse("2028-02-29").is_ok());
    }

    #[test]
    fn rejects_missing_fields() {
        assert!(parse_line("2026-08-25 09:00-10:00").is_err());
    }

    #[test]
    fn summarizes_across_entries() {
        let entries = vec![
            parse_line("2026-08-25 09:00-12:00 acme").unwrap(),
            parse_line("2026-08-25 13:00-17:00 acme").unwrap(),
            parse_line("2026-08-26 09:00-10:30 globex").unwrap(),
        ];
        let totals = summarize_by_project(&entries);
        assert_eq!(totals.get("acme"), Some(&420));
        assert_eq!(totals.get("globex"), Some(&90));
    }

    #[test]
    fn summarizes_by_day() {
        let entries = vec![
            parse_line("2026-08-25 09:00-12:00 acme").unwrap(),
            parse_line("2026-08-25 13:00-17:00 globex").unwrap(),
            parse_line("2026-08-26 09:00-10:30 acme").unwrap(),
        ];
        let totals = summarize_by_day(&entries);
        assert_eq!(totals.get(&Date { year: 2026, month: 8, day: 25 }), Some(&420));
        assert_eq!(totals.get(&Date { year: 2026, month: 8, day: 26 }), Some(&90));
    }

    #[test]
    fn overnight_entry_is_attributed_to_its_start_date() {
        let entries = vec![parse_line("2026-08-25 22:00-02:00 acme night shift").unwrap()];
        let totals = summarize_by_day(&entries);
        assert_eq!(totals.get(&Date { year: 2026, month: 8, day: 25 }), Some(&240));
        assert_eq!(totals.get(&Date { year: 2026, month: 8, day: 26 }), None);
    }

    fn sample_entries() -> Vec<Entry> {
        vec![
            parse_line("2026-08-24 09:00-10:00 acme").unwrap(),
            parse_line("2026-08-25 09:00-10:00 acme").unwrap(),
            parse_line("2026-08-26 09:00-10:00 acme").unwrap(),
        ]
    }

    #[test]
    fn filter_by_date_range_with_both_bounds() {
        let entries = sample_entries();
        let from = Date { year: 2026, month: 8, day: 25 };
        let to = Date { year: 2026, month: 8, day: 25 };
        let filtered = filter_by_date_range(&entries, Some(from), Some(to));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].date, from);
    }

    #[test]
    fn filter_by_date_range_is_inclusive_of_both_ends() {
        let entries = sample_entries();
        let from = Date { year: 2026, month: 8, day: 24 };
        let to = Date { year: 2026, month: 8, day: 26 };
        let filtered = filter_by_date_range(&entries, Some(from), Some(to));
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn filter_by_date_range_with_open_lower_bound() {
        let entries = sample_entries();
        let to = Date { year: 2026, month: 8, day: 25 };
        let filtered = filter_by_date_range(&entries, None, Some(to));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_date_range_with_open_upper_bound() {
        let entries = sample_entries();
        let from = Date { year: 2026, month: 8, day: 25 };
        let filtered = filter_by_date_range(&entries, Some(from), None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_by_date_range_with_no_bounds_keeps_everything() {
        let entries = sample_entries();
        let filtered = filter_by_date_range(&entries, None, None);
        assert_eq!(filtered.len(), 3);
    }
}
