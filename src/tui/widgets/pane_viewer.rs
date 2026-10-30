//! Full-screen viewer for a mux pane's scrollback: the framework's pager
//! (`tuikit::pager`) over a snapshot of the pane's lines.
//!
//! The pager owns the reading: scrolling and paging, `/` with `n`/`N`, the
//! `?` help page, `Esc` stepping back and `q` leaving, and the footer. What
//! is the pane's is here: the header (the mux, window, and pane, its
//! command, and how many lines), the numbered lines, which wrap under
//! their number (`w` cuts them at the edge instead), a find that matches
//! lines, and `a`, which asks to attach to the pane's mux.
//!
//! The viewer holds a snapshot of the scrollback taken when it opened, so
//! it can scroll and find without touching tmux again. Opening it from a
//! search result lands on the matching line with that text picked out.

use std::ops::Range;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::chip::{ToggleChip, Tone};
use tuikit::empty_state::EmptyState;
use tuikit::feedback::Feedback;
use tuikit::help::{HelpAction, HelpEntry};
use tuikit::keys::{KeyRow, PagerKey};
use tuikit::pager::{CONTEXT_ROWS, Find, Pager, PagerContent, PagerEffect};
use tuikit::scroll::WrappedRows;
use tuikit::theme::Theme;

use crate::content_search::PaneRef;
use crate::tui::Msg;
use crate::tui::keybindings::overlay::PANE_KEYS;

/// The pane viewer: the framework's pager over a pane's scrollback.
pub type PaneViewer = Pager<PaneScrollback>;

/// Cells the line numbers take: five digits and a space.
const GUTTER: usize = 6;

/// The pane viewer's own keys, beside the pager's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneKey {
    /// Attach to the pane's mux (`a`).
    Attach,
    /// Wrap long lines, or cut them at the edge (`w`).
    Wrap,
}

impl HelpAction for PaneKey {
    const LISTED: &'static [Self] = &[PaneKey::Attach, PaneKey::Wrap];

    fn help(self) -> HelpEntry {
        match self {
            PaneKey::Attach => HelpEntry::new(
                "Pane",
                "Attach to the pane's mux (refused when it is stopped or gone)",
            ),
            PaneKey::Wrap => HelpEntry::new("Pane", "Wrap long lines, or cut them at the edge"),
        }
    }
}

/// A snapshot of a pane's scrollback, laid out as numbered lines.
#[derive(Clone, Debug)]
pub struct PaneScrollback {
    pub pane: PaneRef,
    pub lines: Vec<String>,
    wrap: bool,
    rows: WrappedRows,
    laid_out: Option<(u16, bool)>,
}

impl PaneScrollback {
    pub fn new(pane: PaneRef, lines: Vec<String>) -> Self {
        let (wrap, rows, laid_out) = (true, WrappedRows::default(), None);
        Self {
            pane,
            lines,
            wrap,
            rows,
            laid_out,
        }
    }
}

impl PagerContent for PaneScrollback {
    type Message = Msg;
    type Action = PaneKey;
    const KEYS: &'static [KeyRow<PaneKey>] = PANE_KEYS;

    fn layout(&mut self, width: u16, theme: &Theme) -> usize {
        if self.laid_out != Some((width, self.wrap)) {
            let number = Style::default().fg(theme.inert);
            let lines: Vec<Line<'_>> = (self.lines.iter().enumerate())
                .map(|(at, text)| {
                    let at = Span::styled(format!("{:>5} ", at + 1), number);
                    Line::from(vec![at, Span::raw(text.as_str())])
                })
                .collect();
            self.rows = if self.wrap {
                WrappedRows::hanging(&lines, usize::from(width), GUTTER)
            } else {
                WrappedRows::cut(&lines, usize::from(width))
            };
        }
        self.laid_out = Some((width, self.wrap));
        self.rows.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.rows.rows()[range].to_vec()
    }

    fn header(&self, _width: usize, theme: &Theme) -> Line<'static> {
        let (pane, dim) = (&self.pane, Tone::Inert.style(theme));
        let at = format!("{}.{}", pane.window_index, pane.pane_index);
        let label = format!("{} {at} scrollback", pane.mux.session);
        let command = pane.command.as_deref().unwrap_or("(unknown command)");
        let count = self.lines.len();
        let lines = if count == 1 { "line" } else { "lines" };
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                label,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" · {command} · {count} {lines}"), dim),
        ])
    }

    fn chips(&self, theme: &Theme) -> Vec<Span<'static>> {
        let state = if self.wrap { "on" } else { "off" };
        let chip = ToggleChip::new("wrap", state, self.wrap).states(&["on", "off"]);
        vec![chip.key("w").span(theme)]
    }

    /// The lines that hold `query`: a match is a line.
    fn find(&self, query: &str) -> Vec<usize> {
        let lines = self.lines.iter().enumerate();
        let holds = |(_, text): &(usize, &String)| tuikit::text::find_ci(text, query).is_some();
        lines.filter(holds).map(|(at, _)| at).collect()
    }

    fn match_rows(&self, line: usize) -> Range<usize> {
        self.rows.rows_of(line)
    }

    /// Every match in the theme's match look and the current line's in
    /// the current-match look, with its number in the accent.
    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
        for (row, at) in rows.iter_mut().zip(first..) {
            let line = self.rows.line_of(at).unwrap_or_default();
            let current = find.current_unit() == Some(line);
            if find.holds(line) {
                *row = find.paint(row, GUTTER, current, theme);
            }
            if current && self.rows.rows_of(line).start == at {
                row.spans[0].style = Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD);
            }
        }
    }

    fn describe(key: PagerKey) -> Option<&'static str> {
        match key {
            PagerKey::Top => Some("Jump to the oldest line"),
            PagerKey::Bottom => Some("Jump to the newest line"),
            PagerKey::Find => Some("Find in the scrollback"),
            _ => None,
        }
    }

    fn empty(&self) -> EmptyState {
        EmptyState::new("the pane has no scrollback")
    }

    fn apply(&mut self, key: PaneKey) -> PagerEffect<Msg> {
        match key {
            // Whether the pane's mux can be attached is the reducer's call.
            PaneKey::Attach => PagerEffect::Request(Msg::AttachPane),
            PaneKey::Wrap => {
                self.wrap = !self.wrap;
                PagerEffect::Relayout
            }
        }
    }
}

/// A viewer over `lines`, opened on the line nearest `line` (from 1) that
/// holds `find`. The scrollback may have moved since the search read it,
/// so the match is looked for rather than the line number trusted; with
/// no match it says so and opens on `line`.
pub fn open(pane: PaneRef, lines: Vec<String>, line: usize, find: &str) -> PaneViewer {
    let wanted = line.saturating_sub(1).min(lines.len().saturating_sub(1));
    let mut viewer = Pager::new(PaneScrollback::new(pane, lines));
    if viewer.land(find, wanted) {
        return viewer;
    }
    if !find.trim().is_empty() {
        viewer.set_note(Feedback::warning(
            "the scrollback no longer holds the match",
        ));
    }
    viewer.at_row(wanted.saturating_sub(CONTEXT_ROWS))
}

#[cfg(test)]
#[path = "pane_viewer_tests.rs"]
mod tests;
