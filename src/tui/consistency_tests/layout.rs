//! The modals the layout checks build, and the layout rules that are
//! Conspectus's own.
//!
//! [`builders`] opens every framed modal over the shared world; the
//! framework's conformance suite (`conventions.rs`) checks each at a range
//! of sizes: nothing it draws escapes the frame, the frame and title take
//! the one look, moving the selection moves no text, and no word is cut
//! at the edge. What stays here is particular to Conspectus's modals: the
//! search modal's sizing rule and title, and how a prompt draws over
//! another modal.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use tuikit::testkit::conformance::{HEIGHTS, WIDTHS};
use tuikit::testkit::keys;

use super::*;
use crate::tui::Modal;

pub(super) type Open = fn(&mut App);

/// One key into the open search overlay, through the modal stack.
pub(super) fn search_key(app: &mut App, code: KeyCode) {
    crate::tui::runtime::modal_key_pure(app, keys::press(code));
}

pub(super) fn type_into_search(app: &mut App, text: &str) {
    for c in text.chars() {
        search_key(app, KeyCode::Char(c));
    }
}

/// The name of a modal, as its registration spells it.
pub(super) fn name_of(modal: &Modal) -> &'static str {
    modal.name()
}

fn open_pinned(app: &mut App, message: Msg) {
    select(app, "conspectus-main");
    app.update(message);
}

/// Every modal that can be built, by name.
pub(super) fn builders() -> Vec<(&'static str, View, Open)> {
    vec![
        ("Help", View::Pinned, |app| app.open_help_overlay()),
        ("Controls", View::Pinned, |app| app.open_controls_overlay()),
        ("Rename", View::Pinned, |app| {
            open_pinned(app, Msg::OpenSlotRename("conspectus-main".into()));
        }),
        ("Search", View::Pinned, |app| app.open_search_overlay()),
        // The same overlay in content mode, with a search running.
        ("Search", View::Pinned, |app| {
            app.update(Msg::StartContentSearch {
                text: "needle".into(),
                kinds: crate::viewer::search::ContentKinds::default(),
            });
            app.open_content_search_overlay();
        }),
        // Quick select on the wider reach, with a query that matches.
        ("Search", View::Pinned, |app| {
            app.open_search_overlay();
            type_into_search(app, "con");
            search_key(app, KeyCode::Tab);
        }),
        ("ValueModal", View::Pinned, |app| {
            app.open_value_modal(crate::tui::widgets::value_modal::ValueModalState::new(
                "cwd",
                "/home/operator/src/a/very/long/path/that/keeps/going/past/any/modal/width/to/see/what/happens",
            ));
        }),
        ("Messages", View::Pinned, |app| app.open_messages_overlay()),
        ("WorktreeMenu", View::Pinned, |app| {
            let ctx = crate::tui::widgets::worktree_menu::WorktreeMenuContext {
                context: crate::tui::worktree_actions::WorktreeContext::Worktree,
                repo_root: Some("/wt/feature".into()),
                branch: Some("feature".into()),
                guard_sessions: vec!["session-one".into()],
                can_mutate: true,
                reveal_target: None,
            };
            let state = crate::tui::widgets::worktree_menu::WorktreeMenuState::new(ctx)
                .expect("a worktree menu");
            app.open_worktree_menu(state);
        }),
        ("NewMux", View::Pinned, |app| {
            app.open_new_mux_form(crate::tui::widgets::new_mux::form(
                "scratch",
                "/home/operator/src/conspectus",
                Vec::new(),
            ));
        }),
        ("MuxMenu", View::Pinned, |app| {
            app.open_mux_menu(crate::tui::actions::mux_menu());
        }),
        ("MuxLaunch", View::Pinned, |app| {
            open_pinned(app, Msg::OpenMuxLaunchForm);
        }),
        ("PinEdit", View::Pinned, |app| {
            open_pinned(app, Msg::OpenSlotEdit("conspectus-main".into()));
        }),
        ("RowMenu", View::Pinned, |app| {
            open_pinned(app, Msg::OpenRowMenu);
        }),
        ("CleanUp", View::History, |app| {
            app.update(Msg::OpenCleanUp);
        }),
        ("Picker", View::Pinned, |app| {
            open_pinned(app, Msg::OpenSlotRetarget("conspectus-main".into()));
        }),
    ]
}

/// One key into the top modal, through the stack, its outcome left
/// unsettled. `false` when the modal reads live App context that moves
/// with the key (its layout is checked without moving).
pub(super) fn press(app: &mut App, code: KeyCode) -> bool {
    let fixed = matches!(
        app.top_modal_name(),
        None | Some("Controls" | "Search" | "Messages" | "Viewer" | "PaneViewer")
    );
    if !fixed {
        let _ = app.with_modals(|stack, app| stack.route(keys::press(code), app));
    }
    !fixed
}

/// The frame with the modal open, and the same app state without it.
pub(super) fn framed_and_bare(view: View, open: Open, width: u16, height: u16) -> (Buffer, Buffer) {
    let mut app = app_with(view, open);
    let with = buffer_of(&mut app, width, height);
    app.pop_top_modal();
    let without = buffer_of(&mut app, width, height);
    (with, without)
}

pub(super) fn app_with(view: View, open: Open) -> App {
    let mut app = app_in(view);
    open(&mut app);
    app
}

/// The corners `(x0, y0, x1, y1)` of the frame the modal drew: the kit's
/// [`frame_rect`](tuikit::testkit::buffer::frame_rect), from its corner
/// glyphs among the cells that differ from the frame without it.
pub(super) fn modal_rect(with: &Buffer, without: &Buffer) -> Option<(u16, u16, u16, u16)> {
    tuikit::testkit::buffer::frame_rect(with, without)
        .map(|rect| (rect.x, rect.y, rect.right() - 1, rect.bottom() - 1))
}

fn describe_size(name: &str, width: u16, height: u16) -> String {
    format!("{name} at {width}x{height}")
}

#[test]
fn the_search_modal_is_wide_and_follows_the_sizing_rule_at_every_size() {
    use crate::tui::widgets::search::SEARCH_SIZE;
    let modal_size = |width, height| {
        let rect = SEARCH_SIZE.rect(ratatui::layout::Rect::new(0, 0, width, height), 0);
        (rect.width, rect.height)
    };
    let mut checked = 0;
    for (name, view, open) in builders() {
        if name != "Search" {
            continue;
        }
        for width in WIDTHS {
            for height in HEIGHTS {
                let (with, without) = framed_and_bare(view, open, width, height);
                let label = describe_size(name, width, height);
                let (x0, y0, x1, y1) =
                    modal_rect(&with, &without).unwrap_or_else(|| panic!("{label}: no frame"));
                assert_eq!(
                    (x1 - x0 + 1, y1 - y0 + 1),
                    modal_size(width, height),
                    "{label}: the frame is not the size the rule gives"
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 3 * WIDTHS.len() * HEIGHTS.len(), "{checked}");
    // The wide terminal gets a wide modal: 85% of it, up to 200 cells.
    assert_eq!(modal_size(220, 50).0, 187);
    assert_eq!(modal_size(300, 50).0, 200);
}

#[test]
fn the_quick_select_title_shows_both_reaches_and_does_not_shift_when_toggled() {
    let mut app = app_with(View::Pinned, App::open_search_overlay);
    for width in WIDTHS {
        let top_border = |app: &mut App| -> (String, String) {
            let text = buffer_to_string(&buffer_of(app, width, 40));
            let line = text
                .lines()
                .find(|line| line.contains('┌') && line.contains("quick select"))
                .unwrap_or_else(|| panic!("{width}: no title\n{text}"))
                .to_string();
            // The modal's own corner, not the panels behind it.
            let start = line.find("┌ / quick").unwrap();
            let end = start + line[start..].find('┐').unwrap();
            (line.clone(), line[start..end + '┐'.len_utf8()].to_string())
        };
        let (current_line, current) = top_border(&mut app);
        assert!(
            current.contains("[ Current ] All"),
            "{width}: {current_line}"
        );
        search_key(&mut app, KeyCode::Tab);
        let (all_line, all) = top_border(&mut app);
        assert!(all.contains("Current [ All ]"), "{width}: {all_line}");
        assert_eq!(
            current.chars().count(),
            all.chars().count(),
            "{width}: the frame changed width with the reach"
        );
        search_key(&mut app, KeyCode::Tab);
    }
}

/// A stacked pair draws as its two halves: over any modal, a prompt
/// draws exactly as it does alone (it clears its own frame), and outside
/// the prompt the modal under it draws exactly as it does alone.
#[test]
fn a_modal_over_another_draws_over_it_and_leaves_the_rest_alone() {
    let prompt = || {
        tuikit::text_prompt::TextPrompt::new("Above", "typed", |_: &str| {
            tuikit::text_prompt::PromptResult::Stay
        })
    };
    for (name, view, open) in builders() {
        for (width, height) in [(80_u16, 24_u16), (120, 40)] {
            let label = describe_size(name, width, height);
            let mut app = app_with(view, open);
            let lower = buffer_of(&mut app, width, height);
            app.open_rename_overlay(prompt());
            let both = buffer_of(&mut app, width, height);
            let mut app = app_with(view, open);
            app.pop_top_modal();
            let bare = buffer_of(&mut app, width, height);
            app.open_rename_overlay(prompt());
            let top = buffer_of(&mut app, width, height);
            let (x0, y0, x1, y1) =
                modal_rect(&top, &bare).unwrap_or_else(|| panic!("{label}: no prompt frame"));
            for y in 0..both.area.height {
                for x in 0..both.area.width {
                    let inside = (x0..=x1).contains(&x) && (y0..=y1).contains(&y);
                    let expected = if inside { &top } else { &lower };
                    assert_eq!(
                        both[(x, y)],
                        expected[(x, y)],
                        "{label}: cell ({x},{y}) of the pair is neither the prompt's nor the modal's\n{}",
                        buffer_to_string(&both)
                    );
                }
            }
        }
    }
}
