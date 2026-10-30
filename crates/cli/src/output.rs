//! How the command line writes what it reads. In a terminal, for a person: a
//! table with a header, colors and relative durations. In a pipe, for a
//! script: columns separated by tabs, without a header or colors, with RFC
//! 3339 times. It decides nothing on what it writes.

use std::io::Write;

use cihub_domain::ItemState;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Where the output goes, and whether it takes colors.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    pub is_terminal: bool,
    pub has_color: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Color {
    Green,
    Yellow,
    Red,
    Dim,
    Bold,
}

impl Color {
    const fn code(self) -> &'static str {
        match self {
            Self::Green => "32",
            Self::Yellow => "33",
            Self::Red => "31",
            Self::Dim => "2",
            Self::Bold => "1",
        }
    }
}

/// A cell of a table, and its color in a terminal.
pub(crate) struct Cell {
    text: String,
    color: Option<Color>,
}

impl Cell {
    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
        }
    }

    pub(crate) fn painted(text: impl Into<String>, color: Option<Color>) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

impl Style {
    /// Colors only in a terminal, and never with `NO_COLOR`
    /// (<https://no-color.org>).
    pub(crate) const fn new(is_terminal: bool, is_no_color: bool) -> Self {
        Self {
            is_terminal,
            has_color: is_terminal && !is_no_color,
        }
    }

    pub(crate) fn paint(self, text: &str, color: Option<Color>) -> String {
        match color {
            Some(color) if self.has_color => format!("\x1b[{}m{text}\x1b[0m", color.code()),
            _ => text.to_owned(),
        }
    }

    /// The rows under their header, aligned, in a terminal; the rows alone,
    /// separated by tabs, in a pipe.
    pub(crate) fn table(self, header: &[&str], rows: &[Vec<Cell>]) -> String {
        if !self.is_terminal {
            return rows.iter().map(|row| tab_line(row) + "\n").collect();
        }

        let mut widths: Vec<usize> = header.iter().map(|title| title.chars().count()).collect();
        for row in rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(one_line(&cell.text).chars().count());
            }
        }
        let header: Vec<Cell> = header
            .iter()
            .map(|title| Cell::painted(*title, Some(Color::Bold)))
            .collect();

        std::iter::once(&header)
            .chain(rows)
            .map(|row| self.aligned_line(row, &widths) + "\n")
            .collect()
    }

    /// A line of the table, each cell padded to its column, colors outside
    /// the padding; the last cell is not padded.
    fn aligned_line(self, row: &[Cell], widths: &[usize]) -> String {
        let last = row.len().saturating_sub(1);
        let cells: Vec<String> = row
            .iter()
            .zip(widths)
            .enumerate()
            .map(|(index, (cell, width))| {
                let text = one_line(&cell.text);
                let padding = if index == last {
                    0
                } else {
                    width - text.chars().count()
                };
                self.paint(&text, cell.color) + &" ".repeat(padding)
            })
            .collect();

        cells.join("  ")
    }

    /// How long ago `at` was in a terminal (`4m`); `at` in RFC 3339 in a
    /// pipe.
    pub(crate) fn time(self, at: OffsetDateTime, now: OffsetDateTime) -> String {
        if self.is_terminal {
            return duration((now - at).whole_seconds());
        }

        at.format(&Rfc3339).unwrap_or_default()
    }
}

/// What a watch writes on each change: in a terminal, the new text over the
/// last one; in a pipe, after it.
pub(crate) struct Screen {
    style: Style,
    drawn_lines: usize,
}

impl Screen {
    pub(crate) const fn new(style: Style) -> Self {
        Self {
            style,
            drawn_lines: 0,
        }
    }

    pub(crate) fn draw(&mut self, out: &mut impl Write, text: &str) -> std::io::Result<()> {
        if self.style.is_terminal && self.drawn_lines > 0 {
            // Back to the first line drawn, then clear to the end.
            write!(out, "\x1b[{}F\x1b[J", self.drawn_lines)?;
        }

        out.write_all(text.as_bytes())?;
        out.flush()?;
        self.drawn_lines = text.lines().count().max(1);
        Ok(())
    }
}

/// A row of a pipe: one cell cannot add a column or a line.
fn tab_line(row: &[Cell]) -> String {
    row.iter()
        .map(|cell| one_line(&cell.text))
        .collect::<Vec<_>>()
        .join("\t")
}

fn one_line(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// `seconds` in its largest unit: `45s`, `12m`, `3h`, `2d`.
pub(crate) fn duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h", seconds / 3600),
        _ => format!("{}d", seconds / 86_400),
    }
}

/// The color of a state in a terminal: green when it merges or will, yellow
/// while it tests, red when it failed, dim when it waits outside the queue or
/// left it.
pub(crate) const fn state_color(state: ItemState) -> Option<Color> {
    match state {
        ItemState::Merged | ItemState::TestsPassed => Some(Color::Green),
        ItemState::Testing => Some(Color::Yellow),
        ItemState::Failed | ItemState::PendingFailure => Some(Color::Red),
        ItemState::NotReady | ItemState::Cancelled => Some(Color::Dim),
        ItemState::Pending => None,
    }
}

/// A state as a person reads it: `tests passed`.
pub(crate) fn state_words(state: ItemState) -> String {
    state.as_str().replace('_', " ")
}

/// The one place where the command line reads the clock: the durations that
/// it shows.
pub(crate) fn now() -> OffsetDateTime {
    #[expect(
        clippy::disallowed_methods,
        reason = "the command line shows how long ago a state began"
    )]
    OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMINAL: Style = Style::new(true, false);
    const NO_COLOR: Style = Style::new(true, true);
    const PIPE: Style = Style::new(false, false);

    fn rows() -> Vec<Vec<Cell>> {
        vec![
            vec![
                Cell::plain("1"),
                Cell::painted("testing", Some(Color::Yellow)),
            ],
            vec![Cell::plain("10"), Cell::plain("a\ttab")],
        ]
    }

    #[test]
    fn a_terminal_aligns_the_table_under_its_header() {
        assert_eq!(
            NO_COLOR.table(&["POS", "STATE"], &rows()),
            "POS  STATE\n1    testing\n10   a tab\n"
        );
        assert_eq!(
            TERMINAL.table(&["POS", "STATE"], &rows()),
            "\x1b[1mPOS\x1b[0m  \x1b[1mSTATE\x1b[0m\n1    \x1b[33mtesting\x1b[0m\n10   a tab\n"
        );
    }

    #[test]
    fn a_pipe_gets_the_rows_alone_separated_by_tabs() {
        assert_eq!(
            PIPE.table(&["POS", "STATE"], &rows()),
            "1\ttesting\n10\ta tab\n"
        );
        assert_eq!(PIPE.paint("x", Some(Color::Red)), "x");
    }

    #[test]
    fn a_terminal_shows_how_long_ago_and_a_pipe_the_time() {
        let at = OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap();
        let now = at + time::Duration::minutes(4);

        assert_eq!(TERMINAL.time(at, now), "4m");
        assert_eq!(PIPE.time(at, now), "2026-09-21T14:13:20Z");
        for (seconds, text) in [
            (-5, "0s"),
            (59, "59s"),
            (3599, "59m"),
            (7200, "2h"),
            (172_800, "2d"),
        ] {
            assert_eq!(duration(seconds), text);
        }
    }
}
