# hourbook

I keep my timesheet in a text file because every "timesheet app" I've tried
wants me to click through a week view to log fifteen minutes of work. This
is the opposite: one line per block of time, in an editor, and a command
that adds it up.

## Format

One entry per line:

```
DATE START-END PROJECT [NOTE]
```

```
2026-08-25 09:00-12:30 acme setup and standup
2026-08-25 13:00-17:15 acme feature work
2026-08-26 09:00-10:30 globex bug triage
```

- `DATE` is `YYYY-MM-DD`.
- `START-END` is two `HH:MM` times (24-hour) joined by a dash. If `END` is
  at or before `START`, the shift is assumed to run past midnight and
  counted through to that time the next day (`22:00-02:00` is 4 hours).
  Start and end can't be equal — that's rejected as ambiguous.
- `PROJECT` is a single word (no spaces).
- Anything after the project is a free-text note, kept for your own record
  but not currently used in the summary.
- Blank lines and lines starting with `#` are skipped, so you can leave
  yourself comments in the file.

## Usage

Summarize a file:

```
$ hourbook timesheet.txt
acme                       8:45
globex                     1:30
```

Summarize several files at once:

```
$ hourbook week1.txt week2.txt
```

Read from stdin — this is the point of having a `-` source, since it lets
the tool sit in a pipeline instead of only ever reading a named file:

```
$ cat timesheet.txt | hourbook
$ pbpaste | hourbook -
```

With no arguments and no piped input, it reads from your terminal until
EOF (Ctrl-D).

Add `--daily` to also print a per-day breakdown, ahead of the per-project
totals:

```
$ hourbook --daily timesheet.txt
2026-08-25                 8:45
2026-08-26                 1:30

acme                       8:45
globex                     1:30
```

Parse errors are printed to stderr with the source and line number, and
don't stop the rest of the file from being processed:

```
$ hourbook timesheet.txt
hourbook: timesheet.txt:4: bad time '25:00'
acme                       8:45
```

## Library

The CLI is a thin wrapper around the `hourbook` crate. The pieces:

- `parse_line(&str) -> Result<Entry, ParseError>`
- `summarize_by_project(&[Entry]) -> BTreeMap<String, u32>` (minutes)
- `summarize_by_day(&[Entry]) -> BTreeMap<Date, u32>` (minutes; an entry
  that crosses midnight is attributed to its start date)

`Entry` is public, so anything that reads a `BufRead` — a file, stdin, a
`Vec<u8>` in a test — can be turned into entries and summarized without
going through the binary at all.

## Status

This is the first pass: parsing (including overnight shifts), per-project
and per-day summaries, and stdin support. No third-party crates, and none
are planned.

Known gaps, in the order I'll probably get to them:

- filtering by date range
- CSV output for pasting into invoices
- configurable rounding (e.g. round to nearest 15 minutes)
