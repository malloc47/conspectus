//! The frame around the rows reads right in every view and at every
//! width: the left pane's tab strip never cuts a view's name, the
//! header's session count agrees with what the active view lists, and
//! the focused pane is the one framed in the focus accent.
//!
//! Each is checked on the rendered frame, the way the operator reads
//! it, so it holds whichever control draws the strip, the header, or the
//! panes.

use super::*;
use crate::filter::{HarnessFilter, PrStateFilter, PrStateKey};
use crate::tui::rows::RowKind;

/// The word the tab strip uses for each view, in strip order.
const TAB_LABELS: [(View, &str); 6] = [
    (View::Sessions, "sessions"),
    (View::Mux, "mux"),
    (View::Prs, "prs"),
    (View::Pinned, "pinned"),
    (View::History, "history"),
    (View::Search, "search"),
];

fn tab_label(view: View) -> &'static str {
    TAB_LABELS
        .iter()
        .find(|(candidate, _)| *candidate == view)
        .map(|(_, label)| *label)
        .expect("every view has a tab")
}

/// The words of the left pane's title: what lies between its top-left
/// corner and the next corner, without the border, the focus marker,
/// and the separators.
fn tab_strip_words(frame: &str) -> Vec<String> {
    let line = frame.lines().nth(1).expect("the panes' top border");
    let start = line.find('┌').expect("the left pane's top-left corner") + '┌'.len_utf8();
    let end = line[start..]
        .find('┐')
        .map_or(line.len(), |offset| start + offset);
    line[start..end]
        .replace(['─', '►'], " ")
        .split_whitespace()
        .filter(|word| *word != "·")
        .map(str::to_string)
        .collect()
}

/// Every width from 40 to 120 columns, both layouts (stacked below the
/// default threshold of 100, side by side from it), every view, and both
/// focus states: the strip shows whole view names in order, always the
/// active one, and ends with `…` when it leaves any out.
#[test]
fn the_tab_strip_never_cuts_a_view_name_at_any_width() {
    let labels: Vec<&str> = TAB_LABELS.iter().map(|(_, label)| *label).collect();
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for focus_right in [false, true] {
            if focus_right {
                app.update(Msg::CycleFocus);
            }
            for width in 40..=120 {
                let shown = frame(&mut app, width, 12);
                let words = tab_strip_words(&shown);
                let context = format!(
                    "{view:?} at {width} columns, right pane focused: {focus_right}: {words:?}\n{}",
                    shown.lines().nth(1).unwrap_or_default()
                );
                let shortened = words.last().is_some_and(|word| word == "…");
                let names = &words[..words.len() - usize::from(shortened)];
                for name in names {
                    assert!(
                        labels.contains(&name.as_str()),
                        "`{name}` is not a whole view name: {context}"
                    );
                }
                assert!(
                    names.iter().any(|name| name == tab_label(view)),
                    "the active view's name is missing: {context}"
                );
                let order: Vec<usize> = names
                    .iter()
                    .map(|name| labels.iter().position(|label| label == name).unwrap())
                    .collect();
                assert!(
                    order.windows(2).all(|pair| pair[0] < pair[1]),
                    "the names are out of order: {context}"
                );
                assert_eq!(
                    shortened,
                    names.len() < labels.len(),
                    "the strip ends with `…` exactly when it leaves a view out: {context}"
                );
            }
        }
    }
}

/// Below 40 columns the strip keeps shortening, down to a cut active
/// name, without the frame failing to draw.
#[test]
fn the_tab_strip_draws_in_any_narrow_pane() {
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        for width in 1..40 {
            let shown = frame(&mut app, width, 12);
            assert!(shown.lines().count() > 1, "{view:?} at {width} columns");
        }
    }
}

/// A pane wide enough for the whole strip names every view; one a few
/// cells short of it drops the names after the active one first.
#[test]
fn the_tab_strip_names_every_view_when_they_fit() {
    let mut app = app_in(View::Sessions);
    // Side by side at 108 columns the left pane is 54 wide.
    let words = tab_strip_words(&frame(&mut app, 108, 12));
    assert_eq!(words, TAB_LABELS.map(|(_, label)| label.to_string()));
    // At 100 columns it is 50 wide: `search` gives way.
    let words = tab_strip_words(&frame(&mut app, 100, 12));
    assert_eq!(
        words,
        ["sessions", "mux", "prs", "pinned", "history", "…"].map(String::from)
    );
    // With Search active, `history` gives way instead.
    let mut app = app_in(View::Search);
    let words = tab_strip_words(&frame(&mut app, 100, 12));
    assert_eq!(
        words,
        ["sessions", "mux", "prs", "pinned", "search", "…"].map(String::from)
    );
}

/// The cells of a pane's border other than its top row, which carries
/// the title: the bottom row and the two sides.
fn border_cells(pane: Rect) -> Vec<(u16, u16)> {
    let mut cells: Vec<(u16, u16)> = (pane.x..pane.right())
        .map(|x| (x, pane.bottom() - 1))
        .collect();
    for y in pane.y..pane.bottom() - 1 {
        cells.push((pane.x, y));
        cells.push((pane.right() - 1, y));
    }
    cells
}

/// The focus cue (house rule 9): the pane with focus draws its border and
/// the `►` before its title in the theme's focus accent, and the other
/// keeps the plain border and a blank where the marker goes, in every
/// view, side by side and stacked, whichever pane has focus. The titles'
/// own text keeps its look.
#[test]
fn only_the_focused_pane_is_framed_in_the_focus_accent() {
    use ratatui::style::Color;

    let theme = crate::tui::Theme::default();
    for (width, height) in [(160_u16, 30_u16), (80, 30)] {
        let areas = panes(width, height);
        for view in ALL_VIEWS {
            let mut app = app_in(view);
            for _ in 0..2 {
                let buf = buffer_of(&mut app, width, height);
                let label = format!("{view:?} at {width}x{height}, {:?} focused", app.focus());
                let (focused, other) = match app.focus() {
                    crate::tui::app::Focus::Left => (areas.left, areas.right),
                    crate::tui::app::Focus::Right => (areas.right, areas.left),
                };
                for (x, y) in border_cells(focused) {
                    assert_eq!(
                        buf[(x, y)].fg,
                        theme.panel_focus_accent,
                        "{label}: ({x}, {y})"
                    );
                }
                for (x, y) in border_cells(other) {
                    assert_eq!(buf[(x, y)].fg, Color::Reset, "{label}: ({x}, {y})");
                }
                for (pane, mark) in [(focused, tuikit::glyphs::marks::POINTER), (other, " ")] {
                    let cell = &buf[(pane.x + 2, pane.y)];
                    assert_eq!(cell.symbol(), mark, "{label}: the marker");
                    assert_eq!(cell.fg, theme.panel_focus_accent, "{label}: the marker");
                    for x in [pane.x, pane.right() - 1] {
                        let corner = buf[(x, pane.y)].fg;
                        let tinted = pane == focused;
                        assert_eq!(corner == theme.panel_focus_accent, tinted, "{label}: ({x})");
                    }
                }
                app.update(Msg::CycleFocus);
            }
        }
    }
    // The right pane's title names the node's kind in bold, in no color
    // of the border's.
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    app.update(Msg::CycleFocus);
    let buf = buffer_of(&mut app, 160, 30);
    let title = Rect {
        height: 1,
        ..panes(160, 30).right
    };
    let at = tuikit::testkit::buffer::find_in(&buf, title, "mux").expect("the right title");
    assert_eq!(buf[(at.x, at.y)].fg, Color::Reset);
    assert!(
        buf[(at.x, at.y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
}

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
