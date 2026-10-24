//! Paint find matches on the composed transcript lines.
//!
//! Matching is on the rendered text after the gutter, so a match the
//! wrapping splits across two lines is not painted, and a match in
//! Markdown source whose markers are stripped on screen (`**bold**`) is
//! painted only when the visible text contains it. The match the viewer
//! is on is drawn reversed; the others use the theme's match style, the
//! same as a match in any other snippet.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use crate::viewer::find::FindState;
use crate::viewer::search::find_ci;
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
        *line = paint(line, cache.leader, &find.query, style);
    }
}

/// `line` with every case-insensitive occurrence of `needle` after the
/// first `leader` columns patched with `style`.
fn paint(line: &Line<'static>, leader: usize, needle: &str, style: Style) -> Line<'static> {
    let chars: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|span| span.content.chars().map(move |c| (c, span.style)))
        .collect();
    let mut column = 0usize;
    let mut body_from = chars.len();
    for (index, (c, _)) in chars.iter().enumerate() {
        if column >= leader {
            body_from = index;
            break;
        }
        column += c.width().unwrap_or(0);
    }
    let body: String = chars[body_from..].iter().map(|(c, _)| *c).collect();
    let starts: Vec<usize> = body.char_indices().map(|(i, _)| i).collect();
    let mut marked = vec![false; chars.len()];
    let mut from = 0usize;
    while let Some(found) = find_ci(&body[from..], needle) {
        let begin = from + found;
        let end = begin + needle.len();
        let first = starts.partition_point(|&b| b < begin);
        let last = starts.partition_point(|&b| b < end);
        for flag in &mut marked[body_from + first..body_from + last] {
            *flag = true;
        }
        from = end;
    }
    if !marked.iter().any(|m| *m) {
        return line.clone();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    let mut run_marked = false;
    for (index, (c, base)) in chars.iter().enumerate() {
        let is_marked = marked[index];
        let effective = if is_marked { base.patch(style) } else { *base };
        if !run.is_empty() && (effective != run_style || is_marked != run_marked) {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        run_style = effective;
        run_marked = is_marked;
        run.push(*c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    Line {
        spans,
        ..line.clone()
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
