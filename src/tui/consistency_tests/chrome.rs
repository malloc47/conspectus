//! The frame around the rows agrees with the view: the header's session
//! count says what the active view lists.
//!
//! It is checked on the rendered frame, the way the operator reads it,
//! so it holds whichever control draws the header.

use super::*;
use crate::filter::{HarnessFilter, PrStateFilter, PrStateKey};
use crate::tui::rows::RowKind;

/// The header's `N/M sessions` and `M sessions` cell, as numbers.
fn header_count(app: &mut App) -> (Option<usize>, usize) {
    let shown = frame(app, 160, 12);
    let header = shown.lines().next().expect("the header line").to_string();
    let cell = header
        .split(" · ")
        .find_map(|segment| segment.trim().strip_suffix(" sessions"))
        .unwrap_or_else(|| panic!("no session count: {header}"));
    let cell = cell.rsplit(' ').next().unwrap_or(cell);
    match cell.split_once('/') {
        Some((shown, total)) => (
            Some(shown.parse().expect("a count")),
            total.parse().expect("a total"),
        ),
        None => (None, cell.parse().expect("a total")),
    }
}

fn sessions_in(snapshot: &GraphSnapshot) -> usize {
    snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::AgentSession(_)))
        .count()
}

fn session_rows(app: &App) -> usize {
    app.tree()
        .rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .count()
}

/// With nothing narrowing it, no view reads as if sessions were filtered
/// out: Mux and PRs list only the sessions with their muxes and PRs,
/// Pinned and History list entities of every kind, and Search lists
/// nothing before a search, and each shows the plain total.
#[test]
fn the_header_counts_every_session_when_nothing_narrows_the_view() {
    let total = sessions_in(&world());
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        assert_eq!(
            header_count(&mut app),
            (None, total),
            "{view:?} lists {} session rows",
            session_rows(&app)
        );
    }
}

/// A filter narrows the count where the view honours it, to the sessions
/// the view still lists, and leaves it whole where the view ignores
/// filters.
#[test]
fn the_header_count_follows_a_filter_in_the_views_that_apply_it() {
    let total = sessions_in(&world());
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    };
    for view in ALL_VIEWS {
        let mut app = app_over_filtered(view, &world(), filter.clone());
        let expected = match view {
            View::Sessions | View::Prs => Some(session_rows(&app)),
            View::Mux => Some(
                app.tree()
                    .rows
                    .iter()
                    .filter_map(|row| match &row.kind {
                        RowKind::MuxSession(mux) => Some(mux.attached_count),
                        _ => None,
                    })
                    .sum(),
            ),
            View::Pinned | View::History | View::Search => None,
        };
        let expected = expected.filter(|shown| *shown != total);
        assert_eq!(header_count(&mut app), (expected, total), "{view:?}");
        if matches!(view, View::Sessions | View::Mux | View::Prs) {
            assert!(
                expected.is_some(),
                "{view:?}: the codex filter leaves sessions out of the world"
            );
        }
    }
}

/// A PR state filter narrows only the PRs view, and its count follows.
#[test]
fn a_pr_filter_narrows_the_prs_count() {
    let total = sessions_in(&world());
    let filter = RowFilter {
        pr_state: Some(PrStateFilter::from_values([PrStateKey::Merged])),
        ..RowFilter::default()
    };
    let mut app = app_over_filtered(View::Prs, &world(), filter);
    let listed = session_rows(&app);
    let expected = (listed != total).then_some(listed);
    assert_eq!(header_count(&mut app), (expected, total));
}

/// The Search view counts the sessions its search found, whether it
/// lists them as session rows or, flat, as the hits that name them.
#[test]
fn the_header_counts_the_sessions_a_search_found() {
    let total = sessions_in(&world());
    let mut app = searched(View::Search, RowFilter::default());
    assert_eq!(header_count(&mut app), (Some(1), total));
    // Pinned and History ignore the search the lists are filtered by.
    for view in [View::Pinned, View::History] {
        let mut app = searched(view, RowFilter::default());
        assert_eq!(header_count(&mut app), (None, total), "{view:?}");
    }
}
