use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

use hourbook::{Date, Entry};

fn main() -> ExitCode {
    let mut daily = false;
    let mut from: Option<Date> = None;
    let mut to: Option<Date> = None;
    let mut sources: Vec<String> = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--daily" => daily = true,
            "--from" | "--to" => {
                let Some(value) = args.next() else {
                    eprintln!("hourbook: {} needs a YYYY-MM-DD argument", arg);
                    return ExitCode::FAILURE;
                };
                let date = match Date::parse(&value) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("hourbook: {}: {}", arg, e);
                        return ExitCode::FAILURE;
                    }
                };
                if arg == "--from" {
                    from = Some(date);
                } else {
                    to = Some(date);
                }
            }
            _ => sources.push(arg),
        }
    }
    if sources.is_empty() {
        sources.push("-".to_string());
    }

    let mut entries: Vec<Entry> = Vec::new();
    let mut had_error = false;

    for source in &sources {
        let ok = if source == "-" {
            let stdin = io::stdin();
            read_entries(stdin.lock(), source, &mut entries)
        } else {
            match File::open(source) {
                Ok(file) => read_entries(BufReader::new(file), source, &mut entries),
                Err(e) => {
                    eprintln!("hourbook: {}: {}", source, e);
                    false
                }
            }
        };
        had_error |= !ok;
    }

    let entries = hourbook::filter_by_date_range(&entries, from, to);

    if daily {
        let day_totals = hourbook::summarize_by_day(&entries);
        for (date, minutes) in &day_totals {
            println!("{:<24} {:>4}:{:02}", date, minutes / 60, minutes % 60);
        }
        println!();
    }

    let totals = hourbook::summarize_by_project(&entries);
    for (project, minutes) in &totals {
        println!("{:<24} {:>4}:{:02}", project, minutes / 60, minutes % 60);
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Reads timesheet lines from `reader`, appending parsed entries to `entries`.
/// Returns false if any line failed to parse, after reporting every failure
/// (rather than stopping at the first bad line in a long timesheet).
fn read_entries<R: BufRead>(reader: R, source: &str, entries: &mut Vec<Entry>) -> bool {
    let mut ok = true;
    for (i, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("hourbook: {}: {}", source, e);
                ok = false;
                continue;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match hourbook::parse_line(trimmed) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                eprintln!("hourbook: {}:{}: {}", source, i + 1, e);
                ok = false;
            }
        }
    }
    ok
}
