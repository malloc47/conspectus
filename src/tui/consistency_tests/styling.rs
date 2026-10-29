//! Things that are not running look the same wherever they appear, and
//! the accelerators that add state take it away again on the same key.

use ratatui::style::Modifier;
use tuikit::testkit::buffer::find_in;

use super::*;
use crate::tui::Effect;
use crate::tui::effect::StoreOp;

/// Whether the first cell of `label` in the left half of the frame has
/// the ghost look (the theme's `inert` color, in italics).
fn label_wears_ghost_style(app: &mut App, label: &str, width: u16) -> bool {
    let buffer = buffer_of(app, width, 40);
    let theme = crate::tui::Theme::default();
    let left = Rect::new(0, 0, width / 2, buffer.area.height);
    let at = find_in(&buffer, left, label)
        .unwrap_or_else(|| panic!("no row for {label} in:\n{}", buffer_to_string(&buffer)));
    let cell = &buffer[(at.x, at.y)];
    cell.fg == theme.inert && cell.modifier.contains(Modifier::ITALIC)
}

pub(super) fn all_stopped_muxes() -> RowFilter {
    RowFilter {
        show_stopped_muxes: true,
        show_pinned_history: true,
        ..RowFilter::default()
    }
}

#[test]
fn gone_and_unstarted_entities_wear_the_ghost_style_and_running_ones_do_not() {
    // (view, rows that are not running, rows that are)
    let cases: [(View, &[&str], &[&str]); 3] = [
        (
            View::Pinned,
            &["config-agent3", "nightly", "ledger-worktree"],
            &["conspectus-main", "e44d01cb"],
        ),
        (
            View::Mux,
            &["config-agent3", "old-experiment"],
            &["conspectus-main", "scratch"],
        ),
        (View::History, &["old-experiment", "config-agent3"], &[]),
    ];
    for (view, ghosts, running) in cases {
        let mut app = app_over_filtered(view, &world(), all_stopped_muxes());
        for label in ghosts {
            assert!(
                label_wears_ghost_style(&mut app, label, 200),
                "{view:?}: {label} should be a ghost"
            );
        }
        for label in running {
            assert!(
                !label_wears_ghost_style(&mut app, label, 200),
                "{view:?}: {label} is running and should not look like a ghost"
            );
        }
    }
}

#[test]
fn a_pin_that_is_not_running_wears_the_hollow_glyph_in_the_ghost_color() {
    let mut app = app_in(View::Pinned);
    let buffer = buffer_of(&mut app, 200, 40);
    let theme = crate::tui::Theme::default();
    let left = Rect::new(0, 0, 100, buffer.area.height);
    for label in ["config-agent3", "nightly", "ledger-worktree"] {
        let at = find_in(&buffer, left, label).unwrap_or_else(|| panic!("no row for {label}"));
        let row = Rect::new(0, at.y, 100, 1);
        let glyph =
            find_in(&buffer, row, "◎").unwrap_or_else(|| panic!("no hollow glyph before {label}"));
        assert_eq!(buffer[(glyph.x, glyph.y)].fg, theme.inert, "{label}");
    }
}

#[test]
fn no_pin_row_is_labelled_gone() {
    // `gone` names an entity that existed and is no longer there; the
    // pin pointing at it reads stopped, removed, or missing.
    let mut app = app_in(View::Pinned);
    let lines = left_pane_lines(&frame(&mut app, 200, 40), 200);
    // The header's `N gone` count isn't a pin row.
    for line in lines.iter().skip(1) {
        assert!(
            !line.split_whitespace().any(|word| word == "gone"),
            "a pin row says gone: {line}"
        );
    }
}

#[test]
fn p_has_an_immediate_inverse_on_the_same_key_in_every_view() {
    for view in [
        View::Sessions,
        View::Mux,
        View::Prs,
        View::Pinned,
        View::Search,
    ] {
        let mut app = if view == View::Search {
            searched(view, all_stopped_muxes())
        } else {
            app_over_filtered(view, &world(), all_stopped_muxes())
        };
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        let mut unpinned = 0;
        let mut pinned = 0;
        for id in ids {
            app.set_selection(id.clone());
            let effects = app.update(Msg::TogglePin);
            assert!(
                app.row_menu().is_none() || effects.is_empty(),
                "{}: p opened a menu where a single pin or unpin was possible",
                describe(view, &id)
            );
            match effects.as_slice() {
                [Effect::WriteStore(StoreOp::Unpin { .. })] => pinned += 1,
                [Effect::WriteStore(StoreOp::PinNode { .. } | StoreOp::PinRecord { .. })] => {
                    unpinned += 1;
                }
                // A group, or something that can't be pinned: a toast.
                [Effect::Toast(_)] | [] => {}
                other => panic!("{}: unexpected effects {other:?}", describe(view, &id)),
            }
            app.close_row_menu();
        }
        assert!(
            pinned > 0,
            "{view:?}: no pinned row was found to test the inverse on"
        );
        // Pinned lists only pins, and the Search view's one match here is
        // the pinned session, so neither has an unpinned row.
        if !matches!(view, View::Pinned | View::Search) {
            assert!(unpinned > 0, "{view:?}: no unpinned row was found");
        }
    }
}
