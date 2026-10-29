//! Every list shows its cursor row one way (house rules 1, 2, and 10):
//! the pointer mark in the 2-cell gutter just inside the frame, the row
//! highlighted from the theme's selection pair, active while the list
//! has focus and inactive while it does not, and short choice lists
//! wrapping at their ends. The marker alone tells the cursor row apart,
//! so a terminal that draws no reverse video still shows it. No drawing
//! code writes a cursor glyph or a reversed row of its own.

use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::style::{Color, Modifier};
use tuikit::glyphs::marks;

use super::glyphs::sources;
use super::layout::{Open, app_with, builders, modal_rect, press};
use super::source_tree::{self, Tree};
use super::*;
use tuikit::testkit::source::{Literal, production_code, string_literals};

/// Modals that list rows a cursor moves over.
const LISTS: &[&str] = &[
    "Controls",
    "Search",
    "Messages",
    "WorktreeMenu",
    "NewMux",
    "MuxMenu",
    "MuxLaunch",
    "PinEdit",
    "RowMenu",
    "Picker",
];

/// The list modals, with Messages holding entries so it lists some.
fn list_builders() -> Vec<(&'static str, View, Open)> {
    let mut out: Vec<_> = builders()
        .into_iter()
        .filter(|(name, _, _)| LISTS.contains(name) && *name != "Messages")
        .collect();
    out.push(("Messages", View::Pinned, |app| {
        for text in ["attached tmux:a", "renamed tmux:b", "launched tmux:c"] {
            app.update(Msg::Report(Box::new(
                <crate::tui::messages::LogEntry as crate::tui::messages::LogEntryExt>::success(
                    text,
                ),
            )));
        }
        app.open_messages_overlay();
    }));
    out
}

/// The modal's frame in `with`, found against the same screen without it.
fn rect_of(view: View, open: Open, app: &mut App) -> (u16, u16, u16, u16) {
    let with = buffer_of(app, 120, 40);
    let mut bare = app_with(view, open);
    bare.pop_top_modal();
    let without = buffer_of(&mut bare, 120, 40);
    modal_rect(&with, &without).expect("a modal frame")
}

/// The rows of the modal in `rect` whose gutter holds the pointer mark.
fn marked_rows(buf: &Buffer, rect: (u16, u16, u16, u16)) -> Vec<u16> {
    let (x0, y0, _, y1) = rect;
    (y0 + 1..y1)
        .filter(|&y| buf[(x0 + 1, y)].symbol() == marks::POINTER)
        .collect()
}

/// The last column of row `y` inside the frame that draws a character.
fn last_text_column(buf: &Buffer, rect: (u16, u16, u16, u16), y: u16) -> u16 {
    let (x0, _, x1, _) = rect;
    (x0 + 1..x1)
        .rev()
        .find(|&x| buf[(x, y)].symbol() != " ")
        .unwrap_or(x0 + 1)
}

#[test]
fn every_list_marks_one_cursor_row_in_its_gutter_and_highlights_it() {
    let theme = crate::tui::Theme::default();
    let mut seen = Vec::new();
    for (name, view, open) in list_builders() {
        let mut app = app_with(view, open);
        let rect = rect_of(view, open, &mut app);
        let buf = buffer_of(&mut app, 120, 40);
        let marked = marked_rows(&buf, rect);
        assert!(marked.len() <= 1, "{name}: one cursor row, got {marked:?}");
        let Some(&y) = marked.first() else {
            continue;
        };
        let (x0, ..) = rect;
        assert_eq!(buf[(x0 + 2, y)].symbol(), " ", "{name}: a 2-cell gutter");
        for x in x0 + 1..=last_text_column(&buf, rect, y) {
            let cell = &buf[(x, y)];
            assert!(
                cell.modifier.contains(theme.selection_active) || cell.bg != Color::Reset,
                "{name}: cell {x} of the cursor row is outside the highlight"
            );
        }
        // Rows off the cursor are not highlighted.
        for other in (rect.1 + 1..rect.3).filter(|row| *row != y) {
            let text: String = (x0 + 1..rect.2).map(|x| buf[(x, other)].symbol()).collect();
            if text.trim().is_empty() {
                continue;
            }
            let reversed =
                (x0 + 1..x0 + 3).any(|x| buf[(x, other)].modifier.contains(Modifier::REVERSED));
            assert!(!reversed, "{name}: row {other} is highlighted too: {text}");
        }
        seen.push(name);
    }
    for name in LISTS {
        assert!(seen.contains(name), "{name}: no cursor row was drawn");
    }
}

#[test]
fn without_selection_modifiers_the_marker_alone_shows_the_cursor_row() {
    let mut seen = 0;
    for (name, view, open) in list_builders() {
        let mut app = app_with(view, open);
        let rect = rect_of(view, open, &mut app);
        let marked_with_pair = marked_rows(&buffer_of(&mut app, 120, 40), rect);
        let theme = &mut app.config_mut().theme;
        theme.selection_active = Modifier::empty();
        theme.selection_inactive = Modifier::empty();
        let marked = marked_rows(&buffer_of(&mut app, 120, 40), rect);
        assert_eq!(
            marked, marked_with_pair,
            "{name}: the marker does not need the pair"
        );
        seen += marked.len();
    }
    assert!(seen >= LISTS.len(), "every list showed its marker");
}

#[test]
fn short_choice_lists_wrap_from_the_first_row_to_the_last() {
    let mut checked = Vec::new();
    for (name, view, open) in list_builders() {
        let mut app = app_with(view, open);
        let rect = rect_of(view, open, &mut app);
        let before = marked_rows(&buffer_of(&mut app, 120, 40), rect);
        let moved = press(&mut app, KeyCode::Up);
        if !moved || before.is_empty() {
            continue;
        }
        let after = marked_rows(&buffer_of(&mut app, 120, 40), rect);
        assert_eq!(after.len(), 1, "{name}: {after:?}");
        assert!(
            after[0] > before[0],
            "{name}: up from the first row lands on the last (from {} to {})",
            before[0],
            after[0]
        );
        checked.push(name);
    }
    for name in [
        "RowMenu",
        "MuxMenu",
        "WorktreeMenu",
        "Picker",
        "PinEdit",
        "MuxLaunch",
    ] {
        assert!(checked.contains(&name), "{name}: no wrap was checked");
    }
}

#[test]
fn the_unfocused_explorer_keeps_its_cursor_row_in_the_inactive_pair() {
    let theme = crate::tui::Theme::default();
    let mut app = app_in(View::Mux);
    select(&mut app, "conspectus-main");
    let (width, height) = (160_u16, 30_u16);
    let half = width / 2;
    let cursor_row = |buf: &Buffer| {
        (1..height - 1)
            .find(|&y| buf[(half + 1, y)].symbol() == marks::POINTER)
            .expect("the explorer marks its cursor row")
    };

    // The value column: past the gutter and the 14-cell label.
    let value = half + 1 + 2 + 14;

    let unfocused = buffer_of(&mut app, width, height);
    let y = cursor_row(&unfocused);
    let cell = &unfocused[(value, y)];
    assert_ne!(cell.symbol(), " ", "the value is drawn");
    assert!(cell.modifier.contains(theme.selection_inactive), "{cell:?}");
    assert!(!cell.modifier.contains(Modifier::REVERSED), "{cell:?}");

    app.update(Msg::CycleFocus);
    let focused = buffer_of(&mut app, width, height);
    let y = cursor_row(&focused);
    assert!(
        focused[(value, y)]
            .modifier
            .contains(theme.selection_active)
    );
}

/// A reviewed use of the reversed modifier that draws no list cursor.
struct Reversed {
    path: &'static str,
    reason: &'static str,
}

const REVERSED_ELSEWHERE: &[Reversed] = &[
    Reversed {
        path: "src/tui/widgets/input.rs",
        reason: "the text cursor cell of the one-line prompt",
    },
    Reversed {
        path: "src/viewer/widget.rs",
        reason: "the text cursor cell of the reader's find prompt",
    },
    Reversed {
        path: "src/viewer/highlight.rs",
        reason: "the current find match in the reader",
    },
];

const DRAWING_SOURCE: &[Tree] = &[Tree::new("src/tui", 100), Tree::new("src/viewer", 15)];

/// Every hand-written list cursor in the drawing code: a string that
/// starts with `> `, or the reversed modifier named outside the reviewed
/// files. Comments and test code are skipped.
fn hand_written_cursors(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut used = vec![false; REVERSED_ELSEWHERE.len()];
    let mut out = Vec::new();
    for (path, text) in sources(root, trees) {
        for Literal {
            line,
            text: literal,
        } in string_literals(&text)
        {
            if literal.starts_with("> ") {
                out.push(format!(
                    "{path}:{line}: \"{literal}\" draws a cursor of its own; draw the row \
                     with `tuikit::selection::row`"
                ));
            }
        }
        for (index, line) in production_code(&text).lines().enumerate() {
            if !line.contains("Modifier::REVERSED") {
                continue;
            }
            match REVERSED_ELSEWHERE
                .iter()
                .position(|entry| entry.path == path)
            {
                Some(entry) => used[entry] = true,
                None => out.push(format!(
                    "{path}:{}: a hand-written reverse; highlight a cursor row with \
                     `tuikit::selection::highlight` and a chip with the theme's `badge`",
                    index + 1
                )),
            }
        }
    }
    for (entry, used) in REVERSED_ELSEWHERE.iter().zip(used) {
        if !used {
            out.push(format!(
                "stale exception for {} ({}): it matches nothing, so remove it",
                entry.path, entry.reason
            ));
        }
    }
    out
}

#[test]
fn no_drawing_code_writes_a_list_cursor_of_its_own() {
    let findings = hand_written_cursors(source_tree::workspace_root(), DRAWING_SOURCE);
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn a_hand_written_cursor_is_caught() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let path = scratch.path().join("src/list.rs");
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("create tree");
    std::fs::write(
        &path,
        "fn row(on: bool) -> String {\n    let mark = if on { \"> \" } else { \"  \" };\n    \
         mark.to_string()\n}\nfn style() -> Style { Style::default().add_modifier(Modifier::REVERSED) }\n\
         // Modifier::REVERSED in a comment\n",
    )
    .expect("plant the file");
    let findings = hand_written_cursors(scratch.path(), &[Tree::new("src", 1)]);
    let planted: Vec<&String> = findings
        .iter()
        .filter(|finding| finding.starts_with("src/list.rs"))
        .collect();
    assert_eq!(planted.len(), 2, "{findings:?}");
    assert!(planted[0].contains(":2:"), "{findings:?}");
    assert!(planted[1].contains(":5:"), "{findings:?}");
}
