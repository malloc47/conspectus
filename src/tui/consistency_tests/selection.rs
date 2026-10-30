//! Every list shows its cursor row one way (house rules 1, 2, and 10),
//! the left pane's tree included: the pointer mark in the 2-cell gutter
//! just inside the frame, on one row, with nothing after it moving as the
//! cursor does, the row
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
use crate::tui::app::Focus;
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
    "CleanUp",
    "Checklist",
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

/// The last column of row `y` inside the frame that draws a character of
/// the row: the scrollbar of a list taller than its frame has a column
/// of its own at the right edge, which is not part of the row.
fn last_text_column(buf: &Buffer, rect: (u16, u16, u16, u16), y: u16) -> u16 {
    const BAR: [&str; 4] = ["║", "█", "▲", "▼"];
    let (x0, _, x1, _) = rect;
    (x0 + 1..x1)
        .rev()
        .filter(|&x| x + 1 != x1 || !BAR.contains(&buf[(x, y)].symbol()))
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
        "CleanUp",
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
    let right = panes(width, height).right;
    let cursor_row = |buf: &Buffer| {
        (1..height - 1)
            .find(|&y| buf[(right.x + 1, y)].symbol() == marks::POINTER)
            .expect("the explorer marks its cursor row")
    };

    // The value column: past the border, the gutter, and the 14-cell
    // label.
    let value = right.x + 1 + 2 + 14;

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

/// Every view's tree with rows to list (the Search view's Sessions and
/// Messages listings after a finished search, History with the records
/// pins point at) and every group open, the cursor on its first row.
fn tree_apps() -> Vec<(View, App)> {
    use super::search_view::{hit, summary, two};
    let mut apps: Vec<(View, App)> = [View::Sessions, View::Mux, View::Prs, View::Pinned]
        .into_iter()
        .map(|view| (view, app_in(view)))
        .collect();
    apps.push((View::History, app_listing_all(View::History)));
    let (a, b) = two();
    let found = [(&a, summary(2, 4, 1_000)), (&b, summary(1, 0, 900))];
    apps.push((View::Search, super::search_view::searched(&found)));
    let turns = vec![
        (&a, vec![hit(4, Some(1_000)), hit(10, Some(950))]),
        (&b, vec![hit(0, Some(900))]),
    ];
    apps.push((View::Search, super::search_flat::flat(&found, turns)));
    for (_, app) in &mut apps {
        let ids: Vec<RowId> = app
            .tree()
            .rows
            .iter()
            .filter(|row| row.expandable)
            .map(|row| row.id.clone())
            .collect();
        for id in ids {
            app.set_selection(id);
            app.update(Msg::ExpandRow);
        }
        app.update(Msg::Home);
    }
    apps
}

/// Tall enough that the tree shows every row with no scrollbar.
const TREE_SIZE: (u16, u16) = (160, 60);

/// The left pane's frame: the bordered pane whose title is the tab strip.
fn tree_rect(buf: &Buffer) -> (u16, u16, u16, u16) {
    let area = buf.area;
    let y0 = (0..area.height)
        .find(|&y| {
            buf[(0, y)].symbol() == "┌"
                && (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("sessions")
        })
        .expect("the left pane's title");
    let x1 = (1..area.width)
        .find(|&x| buf[(x, y0)].symbol() == "┐")
        .expect("a corner");
    let y1 = (y0 + 1..area.height)
        .find(|&y| buf[(0, y)].symbol() == "└")
        .expect("a corner");
    (0, y0, x1, y1)
}

/// The tree's cursor row, its text after the gutter on every row, and
/// whether each cell of the cursor row from the gutter to its last
/// character carries `modifier` (or a fill of its own, a chip).
struct TreeFrame {
    marked: Vec<u16>,
    after_gutter: Vec<String>,
    highlighted: bool,
    reversed_elsewhere: Vec<u16>,
}

fn tree_frame(app: &mut App, modifier: Modifier) -> TreeFrame {
    let buf = buffer_of(app, TREE_SIZE.0, TREE_SIZE.1);
    let rect = tree_rect(&buf);
    let (x0, y0, x1, y1) = rect;
    let marked = marked_rows(&buf, rect);
    let after_gutter = (y0 + 1..y1)
        .map(|y| (x0 + 3..x1).map(|x| buf[(x, y)].symbol()).collect())
        .collect();
    let highlighted = marked.first().is_some_and(|&y| {
        buf[(x0 + 2, y)].symbol() == " "
            && (x0 + 1..=last_text_column(&buf, rect, y)).all(|x| {
                let cell = &buf[(x, y)];
                cell.modifier.contains(modifier) || cell.bg != Color::Reset
            })
    });
    let reversed_elsewhere = (y0 + 1..y1)
        .filter(|y| !marked.contains(y))
        .filter(|&y| (x0 + 1..x0 + 3).any(|x| buf[(x, y)].modifier.contains(Modifier::REVERSED)))
        .collect();
    TreeFrame {
        marked,
        after_gutter,
        highlighted,
        reversed_elsewhere,
    }
}

#[test]
fn the_tree_marks_one_cursor_row_in_its_gutter_and_highlights_it_in_every_view() {
    let theme = crate::tui::Theme::default();
    for (view, mut app) in tree_apps() {
        let rows = app.visible_rows().len();
        assert!(rows > 1, "{view:?}: the tree lists rows");
        for step in 0..rows {
            for focused in [true, false] {
                if focused != (app.focus() == Focus::Left) {
                    app.update(Msg::CycleFocus);
                }
                let modifier = if focused {
                    theme.selection_active
                } else {
                    theme.selection_inactive
                };
                let frame = tree_frame(&mut app, modifier);
                assert_eq!(
                    frame.marked.len(),
                    1,
                    "{view:?} row {step}, focused {focused}: one cursor row, got {:?}",
                    frame.marked
                );
                assert!(
                    frame.highlighted,
                    "{view:?} row {step}, focused {focused}: the marked row is highlighted"
                );
                assert!(
                    frame.reversed_elsewhere.is_empty(),
                    "{view:?} row {step}: rows {:?} are highlighted too",
                    frame.reversed_elsewhere
                );
            }
            if app.focus() != Focus::Left {
                app.update(Msg::CycleFocus);
            }
            app.update(Msg::NavDown);
        }
    }
}

#[test]
fn no_column_of_the_tree_moves_when_its_cursor_does() {
    for (view, mut app) in tree_apps() {
        let rows = app.visible_rows().len();
        let first = tree_frame(&mut app, Modifier::empty());
        let mut cursor_rows = vec![first.marked.clone()];
        for _ in 1..rows {
            app.update(Msg::NavDown);
            let frame = tree_frame(&mut app, Modifier::empty());
            assert_eq!(
                frame.after_gutter, first.after_gutter,
                "{view:?}: the text after the gutter moved with the cursor"
            );
            cursor_rows.push(frame.marked);
        }
        let mut seen: Vec<u16> = cursor_rows.into_iter().flatten().collect();
        seen.dedup();
        assert_eq!(seen.len(), rows, "{view:?}: the marker walked every row");
    }
}

#[test]
fn without_selection_modifiers_the_marker_alone_shows_the_trees_cursor_row() {
    for (view, mut app) in tree_apps() {
        app.update(Msg::NavDown);
        let with_pair = tree_frame(&mut app, Modifier::empty()).marked;
        let theme = &mut app.config_mut().theme;
        theme.selection_active = Modifier::empty();
        theme.selection_inactive = Modifier::empty();
        let without = tree_frame(&mut app, Modifier::empty());
        assert_eq!(with_pair.len(), 1, "{view:?}: {with_pair:?}");
        assert_eq!(
            without.marked, with_pair,
            "{view:?}: the marker does not need the pair"
        );
        assert!(without.reversed_elsewhere.is_empty(), "{view:?}");
    }
}

/// A reviewed use of the reversed modifier that draws no list cursor.
struct Reversed {
    path: &'static str,
    reason: &'static str,
}

/// None today: the readers' current find match takes the theme's
/// `selection_active` look through the framework's find.
const REVERSED_ELSEWHERE: &[Reversed] = &[];

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
