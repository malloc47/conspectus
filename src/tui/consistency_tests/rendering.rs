//! Rows look alike wherever they appear: one column grid, one chip
//! style, one glyph vocabulary, one focus highlight, one scroll rule.

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use tuikit::testkit::buffer::{count_cells, find_in, rows, tokens};

use super::*;
use crate::tui::app::Focus;

/// Column where the label starts and where the age ends, for the left
/// pane's row mentioning `label`.
fn row_grid(lines: &[String], label: &str) -> (usize, Option<usize>) {
    let line = lines
        .iter()
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("no row for {label}:\n{}", lines.join("\n")));
    let toks = tokens(line);
    let label_col = toks
        .iter()
        .find(|token| token.text.contains(label))
        .map(|token| token.start)
        .expect("label token");
    let age_end = toks
        .iter()
        .find(|token| is_age(&token.text))
        .map(|token| token.end);
    (label_col, age_end)
}

#[test]
fn live_and_gone_mux_rows_share_one_column_grid() {
    let mut app = app_in(View::Mux);
    let lines = left_pane_lines(&frame(&mut app, 220, 24), 220);
    let live = row_grid(&lines, "conspectus-main");
    let idle = row_grid(&lines, "scratch");
    let gone = row_grid(&lines, "config-agent3");
    assert_eq!(live.0, gone.0, "labels start in one column");
    assert_eq!(idle.0, gone.0, "labels start in one column");
    assert!(live.1.is_some() && gone.1.is_some(), "{live:?} {gone:?}");
    assert_eq!(live.1, gone.1, "ages end in one column");
}

#[test]
fn the_pinned_view_keeps_its_columns_aligned_across_kinds() {
    let mut app = app_in(View::Pinned);
    let lines = left_pane_lines(&frame(&mut app, 220, 30), 220);
    let ends: Vec<usize> = ["conspectus-main", "config-agent3", "e44d01cb", "ledger-pr"]
        .iter()
        .filter_map(|label| row_grid(&lines, label).1)
        .collect();
    assert!(ends.len() >= 3, "{ends:?}");
    assert!(ends.windows(2).all(|pair| pair[0] == pair[1]), "{ends:?}");
}

#[test]
fn a_pin_with_nothing_running_wears_a_hollow_pin_glyph_and_a_running_one_does_not() {
    let mut app = app_in(View::Pinned);
    let frame = frame(&mut app, 200, 30);
    let line_of = |label: &str| {
        frame
            .lines()
            .find(|line| line.contains(label))
            .unwrap_or_else(|| panic!("no row for {label}:\n{frame}"))
            .chars()
            .take(100)
            .collect::<String>()
    };
    for idle in ["nightly", "config-agent3"] {
        assert!(line_of(idle).contains('◎'), "{}", line_of(idle));
        assert!(
            !line_of(idle).contains("gone"),
            "a pin isn't gone, its entity is: {}",
            line_of(idle)
        );
    }
    for running in ["conspectus-main", "e44d01cb"] {
        assert!(!line_of(running).contains('◎'), "{}", line_of(running));
    }
}

#[test]
fn a_pinned_entity_in_its_own_view_wears_the_filled_pin_marker() {
    let mut app = app_in(View::Mux);
    let frame = frame(&mut app, 200, 24);
    let line = frame
        .lines()
        .find(|line| line.contains("conspectus-main"))
        .expect("row");
    assert!(line.chars().take(100).any(|c| c == '◉'), "{line}");
    let scratch = frame.lines().find(|l| l.contains("scratch")).expect("row");
    assert!(!scratch.chars().take(100).any(|c| c == '◉'), "{scratch}");
}

/// Whether the cells of the first `needle` on `row_label`'s line have a
/// filled background (the badge look every view gives an agent name).
fn token_is_a_chip(buffer: &Buffer, row_label: &str, needle: &str) -> Option<bool> {
    let y = rows(buffer)
        .iter()
        .position(|line| line.contains(row_label) && line.contains(needle))?;
    let row = Rect::new(0, u16::try_from(y).ok()?, buffer.area.width, 1);
    let at = find_in(buffer, row, needle)?;
    let cell = &buffer[(at.x, at.y)];
    Some(cell.bg != Color::Reset || cell.modifier.contains(Modifier::REVERSED))
}

#[test]
fn the_sessions_view_draws_the_agent_name_as_a_chip() {
    let mut app = app_in(View::Sessions);
    let buffer = buffer_of(&mut app, 200, 24);
    assert_eq!(token_is_a_chip(&buffer, "e44d01cb", "claude"), Some(true));
}

#[test]
fn pinned_and_history_rows_draw_the_agent_name_as_the_same_chip() {
    for (view, row) in [
        (View::Pinned, "conspectus-main"),
        (View::History, "old-experiment"),
        (View::Mux, "config-agent3"),
        (View::Search, "solo0000"),
    ] {
        let mut app = if view == View::Search {
            searched(view, RowFilter::default())
        } else {
            app_in(view)
        };
        let buffer = buffer_of(&mut app, 200, 30);
        assert_eq!(
            token_is_a_chip(&buffer, row, "claude"),
            Some(true),
            "{view:?}/{row}:\n{}",
            buffer_to_string(&buffer)
        );
        let text = buffer_to_string(&buffer);
        let line = text
            .lines()
            .find(|l| l.contains(row) && l.contains("claude"))
            .expect("row");
        assert!(!line.contains(" · "), "{view:?}: no dot delimiter: {line}");
    }
}

#[test]
fn session_rows_show_the_agent_glyph_in_every_view_that_lists_them() {
    for view in [View::Sessions, View::Pinned] {
        let mut app = app_in(view);
        let frame = frame(&mut app, 200, 30);
        let line = frame
            .lines()
            .find(|line| line.contains("e44d01cb"))
            .unwrap_or_else(|| panic!("no session row in {view:?}:\n{frame}"));
        assert!(line.chars().take(100).any(|c| c == '●'), "{view:?}: {line}");
    }
}

/// Cells of one half of the frame drawn REVERSED, header and status rows
/// left out. Chips are reversed too, so a test compares the count with and
/// without focus rather than expecting zero.
fn reversed_cells(buffer: &Buffer, left: bool) -> usize {
    let half = buffer.area.width / 2;
    let (from, to) = if left {
        (0, half)
    } else {
        (half, buffer.area.width)
    };
    let body = Rect::new(from, 1, to - from, buffer.area.height.saturating_sub(2));
    count_cells(buffer, body, |cell| {
        cell.modifier.contains(Modifier::REVERSED)
    })
}

/// Only the focused pane reverses its cursor row; the other keeps its
/// cursor row in the inactive look, which is bold rather than reversed
/// by default (see `selection.rs`).
#[test]
fn only_the_focused_pane_reverses_its_cursor_row() {
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    assert_eq!(app.focus(), Focus::Left);
    let left_focused = buffer_of(&mut app, 160, 30);

    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    let right_focused = buffer_of(&mut app, 160, 30);

    // Each pane draws strictly more reversed cells while it has focus:
    // the difference is its highlighted row.
    assert!(
        reversed_cells(&left_focused, true) > reversed_cells(&right_focused, true),
        "the left pane reverses its row only while focused"
    );
    assert!(
        reversed_cells(&right_focused, false) > reversed_cells(&left_focused, false),
        "the right pane reverses its row only while focused"
    );
}

#[test]
fn regrouping_a_view_scrolls_back_to_the_top() {
    let world = world_with(40);
    for view in [View::Pinned, View::Mux, View::Sessions] {
        let mut app = app_over(view, &world);
        let default = Grouping::default_for(view);
        let other = *Grouping::values_for(view)
            .iter()
            .rev()
            .find(|grouping| **grouping != default)
            .expect("another grouping");
        // Short viewport so the tree outgrows it, then go to the end.
        frame(&mut app, 120, 14);
        let last = app.tree().rows.last().expect("rows").id.clone();
        app.set_selection(last);
        frame(&mut app, 120, 14);
        assert!(app.left_scroll() > 0, "{view:?} scrolled down to the end");

        app.update(Msg::SetGrouping(other));
        let after = frame(&mut app, 120, 14);
        assert_eq!(
            app.left_scroll(),
            0,
            "{view:?}: regrouping must show the top again:\n{after}"
        );
    }
}

#[test]
fn the_history_view_hides_pinned_records_by_default() {
    let app = app_in(View::History);
    let snapshot = world();
    let shown: Vec<String> = app
        .tree()
        .rows
        .iter()
        .filter_map(|row| match &row.id {
            RowId::History { record_id } => Some(record_id.clone()),
            _ => None,
        })
        .collect();
    assert!(!shown.is_empty(), "unpinned gone records remain");
    let pinned: Vec<&String> = shown
        .iter()
        .filter(|id| !crate::tui::rows::history::pinned_by(&snapshot, id).is_empty())
        .collect();
    assert!(pinned.is_empty(), "pinned records are listed: {pinned:?}");
}

#[test]
fn the_history_controls_offer_an_unpinned_only_filter_that_starts_checked() {
    let mut app = app_in(View::History);
    app.open_controls_overlay();
    let frame = frame(&mut app, 120, 40);
    let line = frame
        .lines()
        .find(|line| line.to_lowercase().contains("unpinned only"))
        .unwrap_or_else(|| panic!("no unpinned only filter:\n{frame}"));
    assert!(
        line.contains("[x]") || line.contains('√') || line.contains('☑'),
        "{line}"
    );
}

#[test]
fn the_history_empty_state_says_when_pinned_records_are_hidden() {
    let mut snapshot = world();
    // Keep only the records a pin points at.
    let keep: std::collections::BTreeSet<String> = snapshot
        .history
        .metas()
        .filter(|(_, meta)| !crate::tui::rows::history::pinned_by(&snapshot, &meta.id).is_empty())
        .map(|(_, meta)| meta.id.clone())
        .collect();
    snapshot.history.muxes.retain(|r| keep.contains(&r.meta.id));
    snapshot
        .history
        .checkouts
        .retain(|r| keep.contains(&r.meta.id));
    snapshot.history.repos.retain(|r| keep.contains(&r.meta.id));
    snapshot.history.prs.retain(|r| keep.contains(&r.meta.id));

    let mut app = app_over(View::History, &snapshot);
    let shown = frame(&mut app, 140, 20);
    assert!(
        shown.contains("is pinned") && shown.contains("Unpinned only"),
        "an empty History explains the filter:\n{shown}"
    );
}

#[test]
fn listing_pinned_history_marks_those_records_with_the_hollow_pin() {
    let mut app = app_listing_all(View::History);
    let shown = frame(&mut app, 200, 30);
    let line = shown
        .lines()
        .find(|line| line.contains("config-agent3"))
        .unwrap_or_else(|| panic!("no pinned record row:\n{shown}"));
    // Every history record is stopped or removed, so its pin is hollow.
    assert!(line.chars().take(100).any(|c| c == '◎'), "{line}");
    let unpinned = shown
        .lines()
        .find(|line| line.contains("old-experiment"))
        .expect("unpinned row");
    assert!(!unpinned.chars().take(100).any(|c| c == '◎'), "{unpinned}");
}

/// The left pane lays each row out from the row and what the App worked
/// out about it before the frame (`App::tree_row_states`): whether it is
/// a ghost, whether it awaits the hand-off, and its pin marker. The
/// renderer itself asks the snapshot and the hand-off nothing.
#[test]
fn the_left_pane_lays_rows_out_without_looking_anything_up() {
    let renderer = include_str!("../ui/left_panel.rs");
    for needle in [
        "pinned_nodes(",
        ".presence(",
        "row_is_frozen(",
        "row_awaits_handoff(",
    ] {
        assert!(
            !renderer.contains(needle),
            "src/tui/ui/left_panel.rs looks a row's state up (`{needle}`); \
             work it out in `App::tree_row_state` and read it from the row's context"
        );
    }
}
