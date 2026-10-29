//! The keymap help, the transcript viewer's help, and the pane viewer's
//! help are three pages of one modal: same frame, same size, same section
//! headings, same key and description columns, same footer. They are
//! drawn by one page; these tests keep it that way if any is changed.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;
use tuikit::help::{HelpPage, HelpScroll};
use tuikit::testkit::conformance::same_chrome;

use crate::tui::Theme;
use crate::tui::keybindings::overlay::{PANE_VIEWER_KEYS, VIEWER_KEYS};
use crate::tui::widgets::help::{HelpOverlayState, HelpOverlayWidget};
use crate::tui::widgets::viewer_help;

const SIZES: [(u16, u16); 4] = [(60, 20), (80, 24), (120, 40), (160, 50)];

/// The three pages, by name.
const PAGES: [&str; 3] = ["keymap", "viewer", "pane viewer"];

fn page_at(page: &str, area: Rect, theme: &Theme, scroll: usize) -> Buffer {
    let mut buf = Buffer::empty(area);
    let scroll = HelpScroll::at(scroll);
    match page {
        "keymap" => {
            let state = HelpOverlayState { scroll };
            HelpOverlayWidget::new(&state, theme).render(area, &mut buf);
        }
        "viewer" => {
            viewer_help::render(VIEWER_KEYS, area, &mut buf, theme, &scroll);
        }
        _ => {
            viewer_help::render(PANE_VIEWER_KEYS, area, &mut buf, theme, &scroll);
        }
    }
    buf
}

fn page(page: &str, area: Rect, theme: &Theme) -> Buffer {
    page_at(page, area, theme, 0)
}

type Cell = (String, Color, Color, Modifier);

fn cell(buf: &Buffer, x: u16, y: u16) -> Cell {
    let c = &buf[(x, y)];
    (c.symbol().to_string(), c.fg, c.bg, c.modifier)
}

#[test]
fn all_three_helps_draw_the_same_frame() {
    let theme = Theme::default();
    for (w, h) in SIZES {
        let area = Rect::new(0, 0, w, h);
        let rect = HelpPage::SIZE.rect(area, 0);
        let a = page(PAGES[0], area, &theme);
        for other in &PAGES[1..] {
            // Corners, the whole top and bottom borders (title included),
            // and the sides.
            let differ = same_chrome(&a, &page(other, area, &theme), rect);
            assert!(differ.is_empty(), "{other} at {w}x{h}: {differ:?}");
        }
    }
}

/// What the rows inside a page's frame show: the styles of headings
/// (bold first cell), key cells (accent-colored after the indent), and
/// footer cells (muted), and the columns where a key starts and where the
/// description beside it starts.
#[derive(Default)]
struct Rows {
    headings: Vec<Modifier>,
    keys: Vec<Color>,
    footers: Vec<Modifier>,
    key_columns: Vec<u16>,
    description_columns: Vec<u16>,
}

fn rows_of(buf: &Buffer, area: Rect, theme: &Theme) -> Rows {
    let rect = HelpPage::SIZE.rect(area, 0);
    let inner = rect.x + 1..rect.x + rect.width - 1;
    let mut rows = Rows::default();
    for y in rect.y + 1..rect.y + rect.height - 1 {
        let first = cell(buf, inner.start, y);
        let key = cell(buf, inner.start + 2, y);
        if first.3.contains(Modifier::BOLD) && first.0 != " " {
            rows.headings.push(first.3);
        } else if key.1 == theme.accent && key.0 != " " {
            rows.keys.push(key.1);
            rows.key_columns.push(inner.start + 2 - rect.x);
            // The description starts at the first text after the key
            // column's accent cells.
            let after_key = inner
                .clone()
                .find(|&x| buf[(x, y)].fg != theme.accent && buf[(x, y)].symbol() != " ");
            if let Some(x) = after_key {
                rows.description_columns.push(x - rect.x);
            }
        } else if first.3.contains(theme.muted) && first.0 != " " {
            rows.footers.push(first.3);
        }
    }
    rows
}

#[test]
fn all_three_helps_style_headings_keys_and_footer_alike_in_one_column_grid() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 120, 60);
    let pages: Vec<(&str, Rows, Rows)> = PAGES
        .iter()
        .map(|name| {
            (
                *name,
                rows_of(&page(name, area, &theme), area, &theme),
                // The footer closes the page, so look at the bottom of each.
                rows_of(&page_at(name, area, &theme, usize::MAX), area, &theme),
            )
        })
        .collect();
    let (_, keymap, keymap_end) = &pages[0];
    for (name, top, end) in &pages {
        assert!(!top.headings.is_empty(), "{name}: section headings");
        assert!(!top.keys.is_empty(), "{name}: key column");
        assert!(!end.footers.is_empty(), "{name}: footer");
        assert_eq!(top.headings[0], keymap.headings[0], "{name}: headings");
        assert_eq!(top.keys[0], keymap.keys[0], "{name}: keys");
        assert_eq!(end.footers[0], keymap_end.footers[0], "{name}: footer");
        // One grid: every key starts at the same column, and every
        // description beside its key at the same column.
        assert!(
            top.key_columns.iter().all(|x| *x == keymap.key_columns[0]),
            "{name}: key columns {:?}",
            top.key_columns
        );
        assert!(
            !top.description_columns.is_empty()
                && top
                    .description_columns
                    .iter()
                    .all(|x| *x == keymap.description_columns[0]),
            "{name}: description columns {:?}",
            top.description_columns
        );
    }
}

#[test]
fn all_three_helps_end_with_the_same_footer() {
    let theme = Theme::default();
    for (w, h) in SIZES {
        let area = Rect::new(0, 0, w, h);
        let rect = HelpPage::SIZE.rect(area, 0);
        let footer_row = rect.y + rect.height - 2;
        let a = page_at(PAGES[0], area, &theme, usize::MAX);
        let row = |buf: &Buffer| -> Vec<Cell> {
            (rect.x + 1..rect.x + rect.width - 2)
                .map(|x| cell(buf, x, footer_row))
                .collect()
        };
        let footer: String = row(&a).iter().map(|c| c.0.as_str()).collect();
        assert!(
            footer.starts_with(&tuikit::keys::footer(
                crate::tui::keybindings::overlay::HELP_KEYS
            )),
            "{footer:?} at {w}x{h}"
        );
        for other in &PAGES[1..] {
            let b = page_at(other, area, &theme, usize::MAX);
            assert_eq!(row(&a), row(&b), "{other}: the footer row at {w}x{h}");
        }
    }
}

#[test]
fn the_viewer_helps_scroll_where_the_keymap_help_does() {
    let theme = Theme::default();
    for table in [0, 1] {
        let max_scroll = |area: Rect| {
            let mut buf = Buffer::empty(area);
            let scroll = HelpScroll::new();
            let window = if table == 0 {
                viewer_help::render(VIEWER_KEYS, area, &mut buf, &theme, &scroll)
            } else {
                viewer_help::render(PANE_VIEWER_KEYS, area, &mut buf, &theme, &scroll)
            };
            window.last_offset()
        };
        assert!(max_scroll(Rect::new(0, 0, 60, 12)) > 0);
        assert_eq!(max_scroll(Rect::new(0, 0, 100, 60)), 0);
    }
}
