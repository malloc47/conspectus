//! Scrolling and paging keep content on screen: a jump to the end shows
//! the last lines, a page key moves by the rows the pane or modal drew,
//! scrolling past the end is taken back at once, and every menu and pager
//! draws on a short terminal.
//!
//! Keys go in the way the live loop sends them: the screen is drawn, then
//! the key is handed to whatever owns it, so a pager knows what it showed
//! when the next key arrives.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode};

use super::layout::{app_with, builders, framed_and_bare, modal_rect};
use super::*;
use crate::tui::keymap::Action;
use crate::tui::messages::{LogEntry, LogEntryExt};
use crate::tui::widgets::value_modal::ValueText;
use tuikit::testkit::keys::typed_key;

/// The sizes every check runs at: a common terminal and a small one.
const SIZES: [(u16, u16); 2] = [(80, 24), (40, 10)];

/// One frame of the whole screen, the full-screen viewers included.
fn screen(app: &mut App, (width, height): (u16, u16)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| crate::tui::runtime::draw_screen(app, frame))
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// What the live loop does for one key: draw the screen, then give the
/// key to whatever owns it. Returns the next frame.
fn press(app: &mut App, size: (u16, u16), code: KeyCode) -> Buffer {
    screen(app, size);
    let key = typed_key(code);
    match crate::tui::runtime::action_for_event(app, Event::Key(key)) {
        Some(Action::Msg(msg)) => {
            app.update(*msg);
        }
        Some(Action::ModalKey(key)) => crate::tui::runtime::modal_key_pure(app, key),
        other => panic!("unexpected action for {code:?}: {other:?}"),
    }
    screen(app, size)
}

fn press_n(app: &mut App, size: (u16, u16), code: KeyCode, times: usize) -> Buffer {
    let mut last = screen(app, size);
    for _ in 0..times {
        last = press(app, size, code);
    }
    last
}

fn rows_of(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

/// The row span (first, last) whose text differs between two frames.
fn changed_rows(a: &Buffer, b: &Buffer) -> Option<(usize, usize)> {
    let (a, b) = (rows_of(a), rows_of(b));
    let changed: Vec<usize> = (0..a.len()).filter(|&y| a[y] != b[y]).collect();
    Some((*changed.first()?, *changed.last()?))
}

/// Rows of a scrolling body: the span that moves when the content
/// scrolls by one line from `top` with `down`.
fn scrolling_rows(app: &mut App, size: (u16, u16), down: KeyCode) -> usize {
    let top = screen(app, size);
    let moved = press(app, size, down);
    let (first, last) = changed_rows(&top, &moved).expect("one line of scroll changes the body");
    last - first + 1
}

/// How far a page key moved, in lines: the number of single-line steps
/// `up` needs to bring back the frame the page started from. One `down`
/// after the page proves the page did not stop at the end, where the
/// distance would measure the content instead.
fn page_distance(app: &mut App, size: (u16, u16), keys: [KeyCode; 3]) -> usize {
    let [page, down, up] = keys;
    let start = screen(app, size);
    let paged = press(app, size, page);
    let mut frame = press(app, size, down);
    assert_ne!(
        rows_of(&frame),
        rows_of(&paged),
        "the page reached the end; the check needs longer content\n{}",
        buffer_to_string(&paged)
    );
    for steps in 1..=200 {
        frame = press(app, size, up);
        if rows_of(&frame) == rows_of(&start) {
            return steps - 1;
        }
    }
    panic!(
        "{up:?} never brought the first page back\n{}",
        buffer_to_string(&frame)
    );
}

const PAGE_DOWN_J_K: [KeyCode; 3] = [KeyCode::PageDown, KeyCode::Char('j'), KeyCode::Char('k')];

/// `line-01` .. `line-NN`, one per line.
fn numbered(count: usize) -> String {
    (1..=count)
        .map(|n| format!("line-{n:02}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The numbers of the `line-NN` lines on screen, top to bottom.
fn shown_numbers(buf: &Buffer) -> Vec<usize> {
    rows_of(buf)
        .iter()
        .filter_map(|row| {
            let at = row.find("line-")?;
            row[at + 5..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect()
}

fn describe(what: &str, (width, height): (u16, u16)) -> String {
    format!("{what} at {width}x{height}")
}

// ---- value modal ----------------------------------------------------------

fn value_modal_app(value: &str) -> App {
    let mut app = app_in(View::Pinned);
    app.open_value_modal(ValueText::new("detail", value));
    app
}

#[test]
fn jumping_to_the_end_of_a_value_shows_its_last_line_and_k_moves_at_once() {
    for size in SIZES {
        let label = describe("value modal", size);
        let mut app = value_modal_app(&numbered(60));
        let end = press(&mut app, size, KeyCode::Char('G'));
        let shown = shown_numbers(&end);
        assert_eq!(
            shown.last(),
            Some(&60),
            "{label}: G shows the last line\n{}",
            buffer_to_string(&end)
        );
        let up = press(&mut app, size, KeyCode::Char('k'));
        assert_eq!(
            shown_numbers(&up).last(),
            Some(&59),
            "{label}: k after G scrolls up one line\n{}",
            buffer_to_string(&up)
        );
    }
}

#[test]
fn paging_down_a_short_value_keeps_it_on_screen() {
    for size in SIZES {
        let label = describe("value modal", size);
        let mut app = value_modal_app(&numbered(3));
        let paged = press(&mut app, size, KeyCode::PageDown);
        assert_eq!(
            shown_numbers(&paged),
            [1, 2, 3],
            "{label}: the whole value stays visible\n{}",
            buffer_to_string(&paged)
        );
    }
}

#[test]
fn paging_a_value_moves_by_the_rows_the_modal_shows() {
    for size in SIZES {
        let label = describe("value modal", size);
        let mut app = value_modal_app(&numbered(60));
        let first = shown_numbers(&screen(&mut app, size));
        let paged = press(&mut app, size, KeyCode::PageDown);
        assert_eq!(
            shown_numbers(&paged).first(),
            Some(&(first.last().expect("lines shown") + 1)),
            "{label}: the next page starts below the last line shown\n{}",
            buffer_to_string(&paged)
        );
        let back = press(&mut app, size, KeyCode::PageUp);
        assert_eq!(shown_numbers(&back), first, "{label}: PageUp goes back");
    }
}

// ---- Messages detail -------------------------------------------------------

fn messages_app(detail: &str) -> App {
    let mut app = app_in(View::Pinned);
    app.update(Msg::Report(Box::new(
        LogEntry::error("launch failed").with_detail(detail),
    )));
    app.open_messages_overlay();
    app
}

#[test]
fn paging_the_messages_detail_past_its_end_keeps_the_last_lines() {
    for size in SIZES {
        let label = describe("Messages", size);
        let mut app = messages_app(&numbered(30));
        let paged = press_n(&mut app, size, KeyCode::PageDown, 12);
        assert_eq!(
            shown_numbers(&paged).last(),
            Some(&30),
            "{label}: the detail still shows its last line\n{}",
            buffer_to_string(&paged)
        );
        let up = press(&mut app, size, KeyCode::Char('K'));
        assert_eq!(
            shown_numbers(&up).last(),
            Some(&29),
            "{label}: K after the end scrolls up one line\n{}",
            buffer_to_string(&up)
        );

        let mut short = messages_app(&numbered(2));
        let paged = press(&mut short, size, KeyCode::PageDown);
        assert_eq!(
            shown_numbers(&paged),
            [1, 2],
            "{label}: a short detail stays visible\n{}",
            buffer_to_string(&paged)
        );
    }
}

#[test]
fn paging_the_messages_detail_moves_by_the_rows_it_shows() {
    for size in SIZES {
        let label = describe("Messages", size);
        let mut app = messages_app(&numbered(60));
        let first = shown_numbers(&screen(&mut app, size));
        let paged = press(&mut app, size, KeyCode::PageDown);
        assert_eq!(
            shown_numbers(&paged).first(),
            Some(&(first.last().expect("lines shown") + 1)),
            "{label}: the next page starts below the last line shown\n{}",
            buffer_to_string(&paged)
        );
    }
}

// ---- help pages ------------------------------------------------------------

#[test]
fn scrolling_past_the_end_of_the_help_is_taken_back_at_once() {
    for size in SIZES {
        let label = describe("help", size);
        let mut app = app_in(View::Pinned);
        app.open_help_overlay();
        let end = press(&mut app, size, KeyCode::Char('G'));
        let up = press(&mut app, size, KeyCode::Char('k'));
        assert_ne!(
            rows_of(&up),
            rows_of(&end),
            "{label}: k after G scrolls up\n{}",
            buffer_to_string(&up)
        );
        press_n(&mut app, size, KeyCode::Char('j'), 6);
        let up = press(&mut app, size, KeyCode::Char('k'));
        assert_ne!(
            rows_of(&up),
            rows_of(&end),
            "{label}: j past the end is not stored up\n{}",
            buffer_to_string(&up)
        );
    }
}

#[test]
fn paging_the_help_moves_by_the_rows_it_shows() {
    for size in SIZES {
        let label = describe("help", size);
        let mut app = app_in(View::Pinned);
        app.open_help_overlay();
        let rows = scrolling_rows(&mut app, size, KeyCode::Char('j'));
        press(&mut app, size, KeyCode::Char('g'));
        let distance = page_distance(&mut app, size, PAGE_DOWN_J_K);
        assert_eq!(distance, rows, "{label}: a page is the rows on screen");
    }
}

/// The viewers' help pages are shorter than the keymap's, so they are
/// paged on terminals short enough to hold several pages.
const VIEWER_HELP_SIZES: [(u16, u16); 2] = [(80, 16), (40, 10)];

#[test]
fn paging_the_transcript_viewers_help_moves_by_the_rows_it_shows() {
    for size in VIEWER_HELP_SIZES {
        let label = describe("viewer help", size);
        let mut app = app_in(View::Pinned);
        app.open_viewer_modal(crate::tui::viewer_bridge::viewer(
            crate::viewer::model::TranscriptDocument::default(),
            None,
        ));
        press(&mut app, size, KeyCode::Char('?'));
        let rows = scrolling_rows(&mut app, size, KeyCode::Char('j'));
        press(&mut app, size, KeyCode::Char('g'));
        let distance = page_distance(&mut app, size, PAGE_DOWN_J_K);
        assert_eq!(distance, rows, "{label}: a page is the rows on screen");
    }
}

#[test]
fn paging_the_pane_viewers_help_moves_by_the_rows_it_shows() {
    use crate::content_search::{MuxRef, PaneRef};
    use crate::tui::widgets::pane_viewer;
    for size in VIEWER_HELP_SIZES {
        let label = describe("pane viewer help", size);
        let mut app = app_in(View::Pinned);
        let pane = PaneRef {
            mux: MuxRef {
                backend: "tmux".into(),
                socket: None,
                session: "shell".into(),
            },
            pane_id: "%0".into(),
            window_index: "0".into(),
            pane_index: "0".into(),
            command: Some("bash".into()),
        };
        let lines = (1..=100).map(|n| format!("output {n}")).collect();
        app.open_pane_viewer(pane_viewer::open(pane, lines, 1, ""));
        press(&mut app, size, KeyCode::Char('?'));
        let rows = scrolling_rows(&mut app, size, KeyCode::Char('j'));
        press(&mut app, size, KeyCode::Char('g'));
        let distance = page_distance(&mut app, size, PAGE_DOWN_J_K);
        assert_eq!(distance, rows, "{label}: a page is the rows on screen");
    }
}

// ---- the tree --------------------------------------------------------------

/// Rows inside the left pane's frame: the first box drawn from the
/// screen's left edge.
fn tree_rows(buf: &Buffer) -> usize {
    let rows = rows_of(buf);
    let top = rows
        .iter()
        .position(|row| row.starts_with('┌'))
        .expect("the left pane's top border");
    // Counted from the row below the top border, the bottom border's
    // index is the number of rows between them.
    rows.iter()
        .skip(top + 1)
        .position(|row| row.starts_with('└'))
        .expect("the left pane's bottom border")
}

fn selected_index(app: &App) -> usize {
    let selected = app.selection().expect("a selection").clone();
    app.visible_rows()
        .iter()
        .position(|row| row.id == selected)
        .expect("the selection is visible")
}

#[test]
fn paging_the_tree_moves_the_selection_by_the_rows_it_shows() {
    let world = world_with(60);
    // Side by side, and stacked under the narrow-layout threshold.
    for size in [(120_u16, 24_u16), (80, 24), (40, 10)] {
        let label = describe("tree", size);
        let mut app = app_over(View::Mux, &world);
        let rows = tree_rows(&screen(&mut app, size));
        let before = selected_index(&app);
        press(&mut app, size, KeyCode::PageDown);
        let after = selected_index(&app);
        assert_eq!(after - before, rows, "{label}: PageDown moves a page");
        press(&mut app, size, KeyCode::PageUp);
        assert_eq!(selected_index(&app), before, "{label}: PageUp moves back");
    }
}

#[test]
fn a_tree_that_shrinks_leaves_no_blank_rows_below_it() {
    let world = world_with(60);
    let size = (120, 24);
    let mut app = app_over(View::Pinned, &world);
    // Scroll to the end, then step up onto the last group row, which
    // holds the rows below it, and collapse it.
    press(&mut app, size, KeyCode::Char('G'));
    let total = app.visible_rows().len();
    let group = app
        .visible_rows()
        .iter()
        .rposition(|row| row.expandable)
        .expect("an expandable row");
    for _ in group..total - 1 {
        press(&mut app, size, KeyCode::Char('k'));
    }
    assert_eq!(selected_index(&app), group);
    let collapsed = press(&mut app, size, KeyCode::Char('h'));
    let shown = app.visible_rows().len();
    assert!(shown < total, "collapsing hid rows ({total} -> {shown})");
    let rows = tree_rows(&collapsed);
    let expected_scroll = shown.saturating_sub(rows);
    assert!(
        app.left_scroll() <= expected_scroll,
        "the tree scrolled past its end (offset {}, {shown} rows in {rows})\n{}",
        app.left_scroll(),
        buffer_to_string(&collapsed)
    );
}

/// The renderer reports what it measured by returning it: no part of the
/// main screen's draw sends the App a message, so a drawn frame changes
/// nothing but the windows its caller keeps.
#[test]
fn the_renderer_sends_no_messages() {
    let renderer = [
        ("ui.rs", include_str!("../ui.rs")),
        ("ui/compact.rs", include_str!("../ui/compact.rs")),
        ("ui/header.rs", include_str!("../ui/header.rs")),
        ("ui/left_panel.rs", include_str!("../ui/left_panel.rs")),
        ("ui/preview_pane.rs", include_str!("../ui/preview_pane.rs")),
        ("ui/right_panel.rs", include_str!("../ui/right_panel.rs")),
    ];
    for (file, source) in renderer {
        for needle in [".update(", "Msg::"] {
            assert!(
                !source.contains(needle),
                "src/tui/{file} sends a message from the draw (`{needle}`); \
                 return what it measured in the frame's ViewportReport instead"
            );
        }
    }
}

#[test]
fn a_frame_keeps_the_windows_it_drew() {
    let world = world_with(60);
    let size = (80, 24);
    let mut app = app_over(View::Mux, &world);
    press(&mut app, size, KeyCode::Char('G'));
    let kept = app.left_window();
    assert!(
        kept.overflows(),
        "the tree is taller than its pane: {kept:?}"
    );
    assert_eq!(kept.offset(), kept.last_offset(), "G scrolled to the end");
    assert_eq!(
        usize::from(app.left_viewport()),
        kept.viewport(),
        "page keys move by the kept window"
    );
    // Drawing again from the same state keeps the same window.
    screen(&mut app, size);
    assert_eq!(app.left_window(), kept);
}

// ---- menus and every modal on a short terminal -----------------------------

#[test]
fn menus_draw_on_a_terminal_shorter_than_their_frame() {
    let mut failures = Vec::new();
    for (name, view, open) in builders() {
        if !matches!(name, "RowMenu" | "MuxMenu" | "WorktreeMenu") {
            continue;
        }
        for width in [40_u16, 80] {
            for height in 3..=12_u16 {
                let label = describe(name, (width, height));
                let drawn = std::panic::catch_unwind(|| framed_and_bare(view, open, width, height));
                let Ok((with, without)) = drawn else {
                    failures.push(format!("{label}: panicked"));
                    continue;
                };
                if height < 10 {
                    continue;
                }
                let cursor_shown = modal_rect(&with, &without)
                    .is_some_and(|rect| inner_text(&with, rect).contains('►'));
                if !cursor_shown {
                    failures.push(format!(
                        "{label}: the cursor entry is not visible\n{}",
                        buffer_to_string(&with)
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The text inside a modal's frame, frame excluded.
fn inner_text(buf: &Buffer, (x0, y0, x1, y1): (u16, u16, u16, u16)) -> String {
    ((y0 + 1)..y1)
        .flat_map(|y| ((x0 + 1)..x1).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn every_modal_shows_its_body_at_40x10() {
    for (name, view, open) in builders() {
        // Compared with the screen under the modal's whole flow: at 40x10
        // a step (a dialog) and the menu under it both take the frame's
        // full size, so the step's frame is the menu's.
        let mut app = app_with(view, open);
        let with = buffer_of(&mut app, 40, 10);
        app.with_modals(|stack, _| {
            stack.close_flow();
        });
        let without = buffer_of(&mut app, 40, 10);
        let label = describe(name, (40, 10));
        let Some(rect) = modal_rect(&with, &without) else {
            panic!("{label}: no frame\n{}", buffer_to_string(&with));
        };
        assert!(
            !inner_text(&with, rect).trim().is_empty(),
            "{label}: the body is blank\n{}",
            buffer_to_string(&with)
        );
    }
}

/// The row menu with the most entries the fixture offers: a gone
/// record's in the History view.
fn long_row_menu() -> App {
    let mut app = app_in(View::History);
    let record = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(row.id, RowId::History { .. }))
        .expect("a history record")
        .id
        .clone();
    app.set_selection(record);
    app.update(Msg::OpenRowMenu);
    assert!(app.row_menu().is_some(), "the row menu opened");
    app
}

#[test]
fn the_row_menu_keeps_its_footer_and_its_cursor_in_view() {
    let footer = tuikit::keys::footer(tuikit::keys::MENU_KEYS);
    let last_word = footer
        .split_whitespace()
        .last()
        .expect("a footer")
        .to_string();
    for size in [(100_u16, 24_u16), (80, 14), (40, 10)] {
        let label = describe("row menu", size);
        let mut app = long_row_menu();
        let entries = app.row_menu().expect("menu").entries().len();
        let mut frame = screen(&mut app, size);
        for step in 0..entries {
            let text = buffer_to_string(&frame);
            let cursor = app.row_menu().expect("menu").cursor();
            let entry = app.row_menu().expect("menu").entries()[cursor]
                .label
                .clone();
            assert!(
                text.contains(&last_word),
                "{label}, step {step}: the footer is visible\n{text}"
            );
            assert!(
                text.lines()
                    .any(|line| line.contains('►') && line.contains(&entry)),
                "{label}, step {step}: the cursor entry `{entry}` is visible\n{text}"
            );
            frame = press(&mut app, size, KeyCode::Down);
        }
    }
}
