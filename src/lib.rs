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
    EndBeforeStart,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::BadFormat => write!(f, "expected 'DATE START-END PROJECT [NOTE]'"),
            ParseError::BadDate(s) => write!(f, "bad date '{}'", s),
            ParseError::BadTime(s) => write!(f, "bad time '{}'", s),
            ParseError::EndBeforeStart => write!(f, "end time is not after start time"),
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

impl Entry {
    pub fn duration_minutes(&self) -> u32 {
        self.end.minutes_since_midnight() - self.start.minutes_since_midnight()
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
    if end.minutes_since_midnight() <= start.minutes_since_midnight() {
        return Err(ParseError::EndBeforeStart);
    }

    Ok(Entry { date, start, end, project, note })
}

/// Total minutes worked per project, in project name order.
pub fn summarize_by_project(entries: &[Entry]) -> BTreeMap<String, u32> {
    let mut totals: BTreeMap<String, u32> = BTreeMap::new();
    for entry in entries {
        *totals.entry(entry.project.clone()).or_insert(0) += entry.duration_minutes();
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
    fn rejects_end_before_start() {
        let err = parse_line("2026-08-25 12:00-09:00 acme").unwrap_err();
        assert!(matches!(err, ParseError::EndBeforeStart));
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
}
