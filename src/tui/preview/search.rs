//! The content search a Preview shows its match for (ADR 0149).
//!
//! The search is whichever the left pane is listing: the Search view's
//! viewed search, or the applied filter in any other view. Nothing here
//! is stored; the highlight is derived from that search on every draw, so
//! it follows history paging and goes away with the search.

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};

use crate::model::NodeId;
use crate::tui::View;
use crate::tui::app::App;
use crate::tui::preview::{PreviewSubject, TranscriptPreview, preview_subject};
use crate::tui::rows::RowId;
use crate::viewer::highlight::Marks;

/// What to mark in the Preview of the selected row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSearch {
    /// The query text.
    pub query: String,
    /// The transcript turn to bring into view: the hit row's own turn, or
    /// a session row's best hit. `None` marks the query without moving.
    pub turn: Option<usize>,
}

/// The search to show on the Preview, when the Preview is of the selected
/// row and a search is on display.
pub fn preview_search(app: &App) -> Option<PreviewSearch> {
    if preview_subject(app) != PreviewSubject::Selection {
        return None;
    }
    let search = if app.active_view() == View::Search {
        app.viewed_search()?
    } else {
        app.content()?
    };
    if search.text.is_empty() {
        return None;
    }
    let turn = match app.selection()? {
        RowId::Hit { turn, .. } => Some(*turn),
        RowId::AgentSession(NodeId::AgentSession(id)) => search
            .hits()
            .get(&id.harness_key, &id.session_key)
            .map(|hits| hits.best.turn_index),
        _ => None,
    };
    Some(PreviewSearch {
        query: search.text.clone(),
        turn,
    })
}

/// Mark `search` on a Preview transcript, or clear its marks. The
/// matches are recomputed only when the query changes; the hit's turn is
/// the current match when it is one of them.
pub fn apply(preview: &mut TranscriptPreview, search: Option<&PreviewSearch>) {
    let Some(search) = search else {
        if preview.marks.is_active() {
            preview.marks = Marks::default();
        }
        return;
    };
    let query = search.query.trim();
    if preview.marks.query != query {
        preview.marks = Marks {
            query: query.to_string(),
            matches: preview.transcript.matching(query),
            current: None,
        };
    }
    let marks = &mut preview.marks;
    marks.current = search
        .turn
        .filter(|turn| marks.matches.binary_search(turn).is_ok());
}

/// The first line of the rows to show so the current match is in view
/// with a line of context above it, `None` when there is no current
/// match.
pub fn match_top(preview: &TranscriptPreview) -> Option<usize> {
    let turn = preview.marks.current?;
    preview.transcript.rendered.as_ref()?;
    Some(preview.transcript.rows_of(turn).start.saturating_sub(1))
}

/// Mark every occurrence of the query in a pane capture's rows, and the
/// row to start the window at so the newest match is on screen when the
/// bottom `height` rows would not show one.
pub fn mark_rows(
    rows: Vec<Line<'static>>,
    query: &str,
    style: Style,
    height: usize,
) -> Text<'static> {
    let height = height.max(1);
    let matching: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| tuikit::text::find_ci(&row_text(row), query).is_some())
        .map(|(index, _)| index)
        .collect();
    let bottom = rows.len().saturating_sub(height);
    let start = match matching.last() {
        Some(&last) if last < bottom => last.saturating_sub(height / 2).min(bottom),
        _ => bottom,
    };
    let end = (start + height).min(rows.len());
    let lines = rows[start..end]
        .iter()
        .enumerate()
        .map(|(offset, row)| {
            if matching.binary_search(&(start + offset)).is_ok() {
                tuikit::text::paint_matches(row, 0, query, style)
            } else {
                row.clone()
            }
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn row_text(row: &Line<'static>) -> String {
    row.spans
        .iter()
        .map(|Span { content, .. }| content.as_ref())
        .collect()
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
