//! Paint find matches on the composed transcript lines.
//!
//! Matching is on the rendered text after the gutter, so a match the
//! wrapping splits across two lines is not painted, and a match in
//! Markdown source whose markers are stripped on screen (`**bold**`) is
//! painted only when the visible text contains it. The match the viewer
//! is on takes the theme's `selection_active` look, as every pager's
//! does; the others use the theme's match style, the same as a match in
//! any other snippet. One painter serves the pager's find and the marks
//! a transcript drawn outside a pager carries (the inline preview's).

use ratatui::text::Line;
use tuikit::pager::Find;
use tuikit::text;
use tuikit::theme::Theme;

use crate::viewer::find::FindState;
use crate::viewer::state::RenderCache;

/// A query's matches marked on a transcript drawn outside a pager (the
/// inline preview): the query, the shown turns that hold it, and the
/// turn the reader is on, if it is one of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    /// The query, trimmed; empty marks nothing.
    pub query: String,
    /// The shown turns that hold the query, in order.
    pub matches: Vec<usize>,
    /// The turn the reader is on, when it holds the query.
    pub current: Option<usize>,
}

impl Marks {
    /// Whether a query is marked.
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Paint the marks on `lines`, the cache lines from `first_line`.
    pub fn paint(
        &self,
        lines: &mut [Line<'static>],
        first_line: usize,
        cache: &RenderCache,
        theme: &Theme,
    ) {
        if !self.is_active() || self.matches.is_empty() {
            return;
        }
        let holds = |turn: usize| self.matches.binary_search(&turn).is_ok();
        paint_turns(
            lines,
            first_line,
            cache,
            &self.query,
            holds,
            self.current,
            theme,
        );
    }
}

/// Paint the matches of the active find on `lines`, which are the cache
/// lines starting at `first_line`.
pub fn highlight_matches(
    lines: &mut [Line<'static>],
    first_line: usize,
    cache: &RenderCache,
    find: &FindState,
    theme: &Theme,
) {
    if !find.is_active() || find.matches.is_empty() {
        return;
    }
    let holds = |turn: usize| find.matches.binary_search(&turn).is_ok();
    let current = find.current_turn();
    paint_turns(lines, first_line, cache, &find.query, holds, current, theme);
}

/// Paint every occurrence of `query` after the gutter on `lines`, the
/// cache lines from `first_line`, in the rows of the turns `holds`
/// names: the `current` turn's in the current-match look, the others in
/// the match look.
pub fn paint_turns(
    lines: &mut [Line<'static>],
    first_line: usize,
    cache: &RenderCache,
    query: &str,
    holds: impl Fn(usize) -> bool,
    current: Option<usize>,
    theme: &Theme,
) {
    for (line, row) in lines.iter_mut().zip(first_line..) {
        let Some(turn) = turn_at(cache, row).filter(|&turn| holds(turn)) else {
            continue;
        };
        let style = if current == Some(turn) {
            Find::current_style(theme)
        } else {
            Find::match_style(theme)
        };
        *line = text::paint_matches(line, cache.leader, query, style);
    }
}

/// The turn whose text row `row` of the composition shows, or `None`
/// before the first turn and on a tool run collapsed to one summary line
/// (a count of calls, not any turn's text, so nothing in it is a match).
pub fn turn_at(cache: &RenderCache, row: usize) -> Option<usize> {
    let at = cache.turn_starts.partition_point(|start| start.line <= row);
    let start = cache.turn_starts.get(at.checked_sub(1)?)?;
    (!start.collapsed).then_some(start.turn_index)
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
