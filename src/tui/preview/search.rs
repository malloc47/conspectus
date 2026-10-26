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
use crate::tui::preview::{PreviewSubject, preview_subject};
use crate::tui::rows::RowId;
use crate::viewer::highlight::paint;
use crate::viewer::state::ViewerState;

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

/// Point a Preview transcript's find at `search`, or clear it. The
/// matches are recomputed only when the query changes.
pub fn apply(state: &mut ViewerState, search: Option<&PreviewSearch>) {
    let Some(search) = search else {
        if state.find.is_active() {
            state.find.clear();
        }
        return;
    };
    if state.find.query != search.query {
        state.start_find(&search.query);
    }
    let current = search
        .turn
        .and_then(|turn| state.find.matches.binary_search(&turn).ok());
    if state.find.current != current {
        state.find.current = current;
    }
    // The Preview positions itself; the find's own scroll is unused.
    state.find.pending_scroll = None;
}

/// The first line of the rows to show so the current match is in view
/// with a line of context above it, `None` when there is no current
/// match.
pub fn match_top(state: &ViewerState) -> Option<usize> {
    let turn = state.find.current_turn()?;
    let cache = state.rendered.as_ref()?;
    cache
        .turn_starts
        .iter()
        .find(|start| start.turn_index == turn)
        .map(|start| start.line.saturating_sub(1))
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
        .filter(|(_, row)| crate::viewer::search::find_ci(&row_text(row), query).is_some())
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
                paint(row, 0, query, style)
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
