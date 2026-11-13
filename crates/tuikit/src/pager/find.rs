//! Find in a pager: the prompt, the active query and its matches, which
//! match the pager is on, and stepping between them.

use std::ops::Range;

use ratatui::crossterm::event::KeyEvent;
use ratatui::style::Style;
use ratatui::text::Line;

use crate::chip::{StatusChip, Tone};
use crate::glyphs::marks;
use crate::text;
use crate::text_edit::LineEdit;
use crate::theme::Theme;

/// The most cells the query takes in the find chip, the ellipsis of a
/// cut query included.
pub(super) const CHIP_QUERY_CELLS: usize = 17;

/// A pager's find: the prompt while it is open, and the active query
/// with the units that hold it once confirmed.
///
/// The prompt is a [`LineEdit`], so it edits as every text field does;
/// it opens holding the active query, so `/` then `Enter` finds the same
/// text again from where the pager is. Confirming an empty query clears
/// the find. A match is a *unit* of the content (a line, a paragraph, an
/// entry), numbered by the content; [`matches`](Self::matches) lists them
/// in order and [`current`](Self::current) is the one the pager is on.
///
/// The pager draws matches in two looks, so the one it is on stands out:
/// every other match in the theme's [`match_style`](Theme::match_style),
/// and the current one patched with the theme's `selection_active`
/// modifier (reversed and bold by default), the look of what is selected.
/// [`paint`](Self::paint) applies them to a row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Find {
    prompt: Option<LineEdit>,
    query: String,
    matches: Vec<usize>,
    current: Option<usize>,
    /// The unit the next draw scrolls to, once it has laid the rows out.
    landing: Option<usize>,
}

/// What stepping to the next or the previous match did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// No find is active.
    Inactive,
    /// The find has no match to go to.
    Nothing,
    /// The find went to a match, wrapping past an end when `wrapped`.
    Moved {
        /// Whether it went past the last match to the first, or back.
        wrapped: bool,
    },
}

impl Find {
    /// The active query, trimmed; empty when no find is active.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether a find is active: confirmed with a query that is not
    /// empty, matches or none.
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// The units that hold the query, in order.
    pub fn matches(&self) -> &[usize] {
        &self.matches
    }

    /// Which of the [`matches`](Self::matches) the pager is on, if any.
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The unit the pager is on, if any.
    pub fn current_unit(&self) -> Option<usize> {
        self.current.and_then(|at| self.matches.get(at).copied())
    }

    /// Whether `unit` holds a match.
    pub fn holds(&self, unit: usize) -> bool {
        self.matches.contains(&unit)
    }

    /// The prompt, while it is open.
    pub fn prompt(&self) -> Option<&LineEdit> {
        self.prompt.as_ref()
    }

    /// The look of a match the pager is not on: the theme's match style.
    pub fn match_style(theme: &Theme) -> Style {
        theme.match_style()
    }

    /// The look of the match the pager is on: the theme's
    /// `selection_active` modifier, patched over the text's own style.
    pub fn current_style(theme: &Theme) -> Style {
        Style::default().add_modifier(theme.selection_active)
    }

    /// `row` with every occurrence of the query after its first `skip`
    /// cells (a gutter the content draws) patched with the match look,
    /// or with the current match's when `current`. A match a wrapped row
    /// splits is not painted; nothing is painted while no find is active.
    pub fn paint(
        &self,
        row: &Line<'static>,
        skip: usize,
        current: bool,
        theme: &Theme,
    ) -> Line<'static> {
        if self.query.is_empty() {
            return row.clone();
        }
        let look = if current {
            Self::current_style(theme)
        } else {
            Self::match_style(theme)
        };
        text::paint_matches(row, skip, &self.query, look)
    }

    /// The chip the footer shows while a find is active: the query and
    /// where the pager is among its matches (`find "q" 2/5`, in the
    /// success tone), or `no match` and the content's `detail` (in the
    /// warning tone). Bold. `None` while no find is active.
    pub fn chip(&self, detail: Option<String>) -> Option<StatusChip<'static>> {
        if !self.is_active() {
            return None;
        }
        let query = text::truncate_end(&self.query, CHIP_QUERY_CELLS);
        let chip = match (self.current, self.matches.is_empty()) {
            (Some(at), _) => StatusChip::new(
                format!("find \"{query}\" {}/{}", at + 1, self.matches.len()),
                Tone::Success,
            ),
            (None, false) => {
                let count = self.matches.len();
                let noun = if count == 1 { "match" } else { "matches" };
                StatusChip::new(format!("find \"{query}\" {count} {noun}"), Tone::Success)
            }
            (None, true) => {
                let mut text = format!("find \"{query}\" no match");
                if let Some(detail) = detail {
                    text.push_str(marks::SPACED_SEPARATOR);
                    text.push_str(&detail);
                }
                StatusChip::new(text, Tone::Warning)
            }
        };
        Some(chip.bold())
    }

    pub(super) fn open_prompt(&mut self) {
        self.prompt = Some(LineEdit::new(self.query.as_str()));
    }

    pub(super) fn close_prompt(&mut self) {
        self.prompt = None;
    }

    /// The prompt's text, closing the prompt.
    pub(super) fn take_prompt(&mut self) -> Option<String> {
        self.prompt.take().map(|prompt| prompt.value().to_string())
    }

    /// Gives `key` to the prompt's editor.
    pub(super) fn edit(&mut self, key: KeyEvent) {
        if let Some(prompt) = &mut self.prompt {
            prompt.handle(key);
        }
    }

    /// Makes `query` the active find with `matches`, on none of them.
    pub(super) fn set(&mut self, query: &str, matches: Vec<usize>) {
        query.trim().clone_into(&mut self.query);
        self.matches = if self.query.is_empty() {
            Vec::new()
        } else {
            matches
        };
        self.current = None;
        self.landing = None;
    }

    /// Forgets the active find, keeping an open prompt.
    pub(super) fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current = None;
        self.landing = None;
    }

    /// Goes to match `at` and asks the next draw to scroll to it.
    pub(super) fn go(&mut self, at: usize) {
        if let Some(&unit) = self.matches.get(at) {
            self.current = Some(at);
            self.landing = Some(unit);
        }
    }

    /// The unit a draw should scroll to, once.
    pub(super) fn take_landing(&mut self) -> Option<usize> {
        self.landing.take()
    }

    /// Stays on `unit` after the matches were found again, without
    /// scrolling: on none of them when it no longer matches.
    pub(super) fn stay_on(&mut self, unit: Option<usize>) {
        self.current = unit.and_then(|unit| self.matches.iter().position(|&m| m == unit));
    }

    /// The match nearest `unit` (the first of two as near).
    pub(super) fn nearest(&self, unit: usize) -> Option<usize> {
        self.matches
            .iter()
            .enumerate()
            .min_by_key(|(_, m)| m.abs_diff(unit))
            .map(|(at, _)| at)
    }

    /// The first match at or below the window's top row: one whose rows
    /// end after `top`, or the first match when none does. `rows_of` is
    /// `None` before the rows have been laid out.
    pub(super) fn first_from(
        &self,
        rows_of: Option<&dyn Fn(usize) -> Range<usize>>,
        top: usize,
    ) -> usize {
        rows_of
            .and_then(|rows_of| {
                self.matches
                    .iter()
                    .position(|&unit| rows_of(unit).end > top)
            })
            .unwrap_or(0)
    }

    /// Steps to the next (`forward`) or previous match, wrapping at the
    /// ends. With no match to step from, it goes to the first match at
    /// or below the top of `window` going forward, or the last one above
    /// its bottom going back.
    pub(super) fn step(
        &mut self,
        forward: bool,
        rows_of: Option<&dyn Fn(usize) -> Range<usize>>,
        window: Range<usize>,
    ) -> Step {
        if !self.is_active() {
            return Step::Inactive;
        }
        let count = self.matches.len();
        if count == 0 {
            return Step::Nothing;
        }
        let (next, wrapped) = match self.current {
            Some(at) if forward => ((at + 1) % count, at + 1 >= count),
            Some(at) => ((at + count - 1) % count, at == 0),
            None => {
                let found = rows_of.and_then(|rows_of| {
                    if forward {
                        self.matches
                            .iter()
                            .position(|&unit| rows_of(unit).end > window.start)
                    } else {
                        self.matches
                            .iter()
                            .rposition(|&unit| rows_of(unit).start < window.end)
                    }
                });
                match (found, forward) {
                    (Some(at), _) => (at, false),
                    (None, true) => (0, rows_of.is_some()),
                    (None, false) => (count - 1, rows_of.is_some()),
                }
            }
        };
        self.go(next);
        Step::Moved { wrapped }
    }
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
