//! Paint find matches on the composed transcript lines.
//!
//! Matching is on the rendered text after the gutter, so a match the
//! wrapping splits across two lines is not painted, and a match in
//! Markdown source whose markers are stripped on screen (`**bold**`) is
//! painted only when the visible text contains it. The match the viewer
//! is on takes the theme's `selection_active` look, as every pager's
//! does; the others use the theme's match style, the same as a match in
//! any other snippet.

use ratatui::text::Line;
use tuikit::pager::Find;
use tuikit::text;
use tuikit::theme::Theme;

use crate::viewer::find::FindState;
use crate::viewer::state::RenderCache;

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
    let current = find.current_turn();
    for (offset, line) in lines.iter_mut().enumerate() {
        let Some(turn) = turn_at(cache, first_line + offset) else {
            continue;
        };
        if find.matches.binary_search(&turn).is_err() {
            continue;
        }
        let style = if current == Some(turn) {
            Find::current_style(theme)
        } else {
            Find::match_style(theme)
        };
        *line = text::paint_matches(line, cache.leader, &find.query, style);
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
