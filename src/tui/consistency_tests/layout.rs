//! Modals and forms keep a stable layout: nothing they draw escapes the
//! frame, and moving the selection inside one changes which row is
//! highlighted, not where any text sits.
//!
//! Every [`Modal`] variant is either built here and checked at a range of
//! sizes, or listed with the reason it can't be. A new variant fails the
//! coverage test until it is added to one list or the other.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::tui::Modal;
use crate::tui::modal::Overlay;

const WIDTHS: [u16; 4] = [60, 80, 120, 160];
const HEIGHTS: [u16; 2] = [20, 40];

pub(super) type Open = fn(&mut App);

/// The name of a modal, from an exhaustive match: a new variant does not
/// compile until it is named here.
pub(super) fn name_of(modal: &Modal) -> &'static str {
    match modal {
        Modal::Help(_) => "Help",
        Modal::Controls(_) => "Controls",
        Modal::Rename(_) => "Rename",
        Modal::Search(_) => "Search",
        Modal::ValueModal(_) => "ValueModal",
        Modal::Messages(_) => "Messages",
        Modal::Viewer(_) => "Viewer",
        Modal::WorktreeMenu(_) => "WorktreeMenu",
        Modal::NewMux(_) => "NewMux",
        Modal::MuxMenu(_) => "MuxMenu",
        Modal::MuxLaunch(_) => "MuxLaunch",
        Modal::PinEdit(_) => "PinEdit",
        Modal::RowMenu(_) => "RowMenu",
        Modal::CleanUp(_) => "CleanUp",
        Modal::Picker(_) => "Picker",
    }
}

/// Modals the layout checks can't build here, and why.
const SKIPPED: &[(&str, &str)] = &[(
    "Viewer",
    "the full-screen transcript viewer needs a session transcript on disk; its own tests cover its layout",
)];

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
        // The same overlay on its contents scope, with a search running.
        ("Search", View::Pinned, |app| {
            app.update(Msg::StartContentSearch {
                text: "needle".into(),
                kinds: crate::viewer::search::ContentKinds::default(),
            });
            app.open_search_overlay();
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
            app.open_new_mux_form(crate::tui::widgets::new_mux::NewMuxFormState::new(
                "scratch",
                "/home/operator/src/conspectus",
            ));
        }),
        ("MuxMenu", View::Pinned, |app| {
            app.open_mux_menu(crate::tui::widgets::mux_menu::MuxMenuState::new());
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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// One key into the top modal's own handler. `false` when the modal
/// needs live app context to handle keys (its layout is checked without
/// moving).
fn press(modal: &mut Modal, code: KeyCode) -> bool {
    let key = key(code);
    match modal {
        Modal::Help(state) => {
            state.handle((), key);
        }
        Modal::Rename(state) => {
            state.handle((), key);
        }
        Modal::ValueModal(state) => {
            state.handle((), key);
        }
        Modal::WorktreeMenu(state) => {
            state.handle((), key);
        }
        Modal::NewMux(state) => {
            state.handle((), key);
        }
        Modal::MuxMenu(state) => {
            state.handle((), key);
        }
        Modal::MuxLaunch(state) => {
            state.handle((), key);
        }
        Modal::PinEdit(state) => {
            state.handle((), key);
        }
        Modal::RowMenu(state) => {
            state.handle((), key);
        }
        Modal::CleanUp(state) => {
            state.handle((), key);
        }
        Modal::Picker(state) => {
            state.handle((), key);
        }
        Modal::Controls(_) | Modal::Search(_) | Modal::Messages(_) | Modal::Viewer(_) => {
            return false;
        }
    }
    true
}

/// Modals whose whole body scrolls with Up and Down, so their text
/// legitimately moves.
const SCROLLS: &[&str] = &["Help", "ValueModal"];

/// The frame with the modal open, and the same app state without it.
fn framed_and_bare(view: View, open: Open, width: u16, height: u16) -> (Buffer, Buffer) {
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

fn symbol(buf: &Buffer, x: u16, y: u16) -> &str {
    buf[(x, y)].symbol()
}

fn corner(symbol: &str) -> bool {
    matches!(
        symbol,
        "┌" | "┐" | "└" | "┘" | "╭" | "╮" | "╰" | "╯" | "╔" | "╗" | "╚" | "╝"
    )
}

fn vertical(symbol: &str) -> bool {
    matches!(symbol, "│" | "┃" | "║")
}

/// The rectangle of the frame the modal drew, from its corner glyphs
/// among the cells that differ from the frame without it.
fn modal_rect(with: &Buffer, without: &Buffer) -> Option<(u16, u16, u16, u16)> {
    let area = with.area;
    let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0, 0);
    let mut found = false;
    for y in 0..area.height {
        for x in 0..area.width {
            if with[(x, y)] != without[(x, y)] && corner(symbol(with, x, y)) {
                found = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    found.then_some((x0, y0, x1, y1))
}

fn describe_size(name: &str, width: u16, height: u16) -> String {
    format!("{name} at {width}x{height}")
}

#[test]
fn every_modal_is_checked_or_has_a_reason() {
    let source = include_str!("../modal.rs");
    let body = source
        .split("pub enum Modal {")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("the Modal enum");
    let variants: Vec<&str> = body
        .lines()
        .filter(|line| line.starts_with("    ") && !line.starts_with("     "))
        .filter_map(|line| {
            let name = line.trim().split('(').next()?;
            (name.chars().next()?.is_ascii_uppercase() && line.contains('(')).then_some(name)
        })
        .collect();
    assert!(variants.len() >= 10, "found the variants: {variants:?}");
    let built: Vec<&str> = builders().iter().map(|(name, _, _)| *name).collect();
    for variant in variants {
        assert!(
            built.contains(&variant) || SKIPPED.iter().any(|(name, _)| *name == variant),
            "Modal::{variant} is neither built in layout.rs nor listed with a reason"
        );
    }
}

#[test]
fn each_builder_opens_the_modal_it_names() {
    for (name, view, open) in builders() {
        let app = app_with(view, open);
        let top = app
            .modal_stack()
            .last()
            .unwrap_or_else(|| panic!("{name}: nothing opened"));
        assert_eq!(name_of(top), name);
    }
}

#[test]
fn nothing_a_modal_draws_escapes_its_frame() {
    for (name, view, open) in builders() {
        for width in WIDTHS {
            for height in HEIGHTS {
                let (with, without) = framed_and_bare(view, open, width, height);
                let label = describe_size(name, width, height);
                let Some((x0, y0, x1, y1)) = modal_rect(&with, &without) else {
                    continue;
                };
                for y in 0..with.area.height {
                    for x in 0..with.area.width {
                        let inside = (x0..=x1).contains(&x) && (y0..=y1).contains(&y);
                        assert!(
                            inside || with[(x, y)] == without[(x, y)],
                            "{label}: cell ({x},{y}) outside the frame ({x0},{y0})-({x1},{y1}) changed to {:?}",
                            symbol(&with, x, y)
                        );
                    }
                }
                for y in (y0 + 1)..y1 {
                    for edge in [x0, x1] {
                        assert!(
                            vertical(symbol(&with, edge, y)),
                            "{label}: text runs into the frame at ({edge},{y}): {:?}\n{}",
                            symbol(&with, edge, y),
                            buffer_to_string(&with)
                        );
                    }
                }
            }
        }
    }
}

/// The rows whose styling changed between two frames: the rows that
/// move with the selection.
fn restyled_rows(a: &Buffer, b: &Buffer) -> Vec<u16> {
    (0..a.area.height)
        .filter(|&y| (0..a.area.width).any(|x| a[(x, y)].style() != b[(x, y)].style()))
        .collect()
}

#[test]
fn moving_the_selection_in_a_modal_moves_no_text() {
    let mut compared: Vec<&str> = Vec::new();
    for (name, view, open) in builders() {
        for width in [80_u16, 120] {
            let height = 40_u16;
            if SCROLLS.contains(&name) {
                continue;
            }
            let (first, without) = framed_and_bare(view, open, width, height);
            let mut app = app_with(view, open);
            let mut frames = vec![first];
            let mut steps = 0;
            for code in [
                KeyCode::Down,
                KeyCode::Down,
                KeyCode::Down,
                KeyCode::Down,
                KeyCode::Up,
            ] {
                let moved = app.top_modal_mut().is_some_and(|modal| press(modal, code));
                if !moved {
                    break;
                }
                steps += 1;
                frames.push(buffer_of(&mut app, width, height));
            }
            if steps == 0 {
                continue;
            }
            let label = describe_size(name, width, height);
            let rect = modal_rect(&frames[0], &without);
            for pair in frames.windows(2) {
                if let Some(rect) = rect {
                    assert_eq!(
                        modal_rect(&pair[1], &without),
                        Some(rect),
                        "{label}: the frame moved or resized with the selection"
                    );
                }
            }
            // Rows that restyle as the selection moves hold the choices;
            // anything above and including the last of them keeps its
            // text in place except the two-cell selection gutter. Rows
            // below carry explanations that follow the selection.
            let mut selectable = Vec::new();
            for pair in frames.windows(2) {
                selectable.extend(restyled_rows(&pair[0], &pair[1]));
            }
            let Some(&last) = selectable.iter().max() else {
                continue;
            };
            compared.push(name);
            let (x0, x1) = rect.map_or((0, frames[0].area.width - 1), |r| (r.0, r.2));
            for pair in frames.windows(2) {
                for y in 0..=last {
                    for x in (x0 + 3)..x1 {
                        assert_eq!(
                            symbol(&pair[0], x, y),
                            symbol(&pair[1], x, y),
                            "{label}: text at ({x},{y}) changed when only the selection moved\nbefore:\n{}\nafter:\n{}",
                            buffer_to_string(&pair[0]),
                            buffer_to_string(&pair[1])
                        );
                    }
                }
            }
        }
    }
    // The check has to bite: these forms and menus all have a moving
    // selection, so a pass that compared nothing would be a false one.
    for name in ["PinEdit", "RowMenu", "Picker", "MuxLaunch", "NewMux"] {
        assert!(
            compared.contains(&name),
            "{name}: no selection movement was compared"
        );
    }
}

/// The text of each row inside the modal's frame, frame excluded.
fn inner_rows(buf: &Buffer, rect: (u16, u16, u16, u16)) -> Vec<String> {
    let (x0, y0, x1, y1) = rect;
    ((y0 + 1)..y1)
        .map(|y| ((x0 + 1)..x1).map(|x| symbol(buf, x, y)).collect())
        .collect()
}

fn tokens_of(rows: &[String]) -> std::collections::BTreeSet<String> {
    rows.iter()
        .flat_map(|row| row.split_whitespace().map(str::to_string))
        .collect()
}

#[test]
fn no_word_in_a_modal_is_cut_off_at_its_edge() {
    // The same modal drawn on a very wide screen shows every word whole.
    // A word that touches the narrow modal's right edge and is only the
    // start of a longer word there was clipped; a deliberate cut ends in
    // an ellipsis.
    for (name, view, open) in builders() {
        let (wide, wide_bare) = framed_and_bare(view, open, 240, 40);
        let Some(wide_rect) = modal_rect(&wide, &wide_bare) else {
            continue;
        };
        let wide_tokens = tokens_of(&inner_rows(&wide, wide_rect));
        for width in WIDTHS {
            for height in HEIGHTS {
                let (with, without) = framed_and_bare(view, open, width, height);
                let Some(rect) = modal_rect(&with, &without) else {
                    continue;
                };
                for (index, row) in inner_rows(&with, rect).iter().enumerate() {
                    // The last text column: the inner column before the
                    // right frame, or before a scrollbar column.
                    let Some(last) = row.split_whitespace().last() else {
                        continue;
                    };
                    let touches_edge = row.chars().count() <= row.trim_end().chars().count() + 1;
                    if !touches_edge || last.ends_with('…') || wide_tokens.contains(last) {
                        continue;
                    }
                    // A word longer than the modal has to be broken
                    // somewhere; that is wrapping, not clipping.
                    let inner = usize::from(rect.2 - rect.0 - 1);
                    let clipped = wide_tokens.iter().any(|full| {
                        full.len() > last.len() && full.len() <= inner && full.starts_with(last)
                    });
                    assert!(
                        !clipped,
                        "{}: row {index} ends in {last:?}, the start of a longer word, at the modal's edge\n{}",
                        describe_size(name, width, height),
                        buffer_to_string(&with)
                    );
                }
            }
        }
    }
}
