//! Paint find matches on the composed transcript lines.
//!
//! Matching is on the rendered text after the gutter, so a match the
//! wrapping splits across two lines is not painted, and a match in
//! Markdown source whose markers are stripped on screen (`**bold**`) is
//! painted only when the visible text contains it. The match the viewer
//! is on is drawn reversed; the others use the theme's match style, the
//! same as a match in any other snippet.

use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use tuikit::text;

use crate::viewer::find::FindState;
use crate::viewer::state::RenderCache;
use crate::viewer::theme::Theme;

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
        let absolute = first_line + offset;
        let at = cache
            .turn_starts
            .partition_point(|start| start.line <= absolute);
        let Some(start) = at.checked_sub(1).and_then(|i| cache.turn_starts.get(i)) else {
            continue;
        };
        if start.collapsed || find.matches.binary_search(&start.turn_index).is_err() {
            continue;
        }
        let style = if current == Some(start.turn_index) {
            Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            theme.match_style()
        };
        *line = text::paint_matches(line, cache.leader, &find.query, style);
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
