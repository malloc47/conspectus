//! The keymap help and the transcript viewer's help are two pages of one
//! modal: same frame, same size, same section headings, same key column,
//! same footer styling. They are drawn by one renderer; these tests keep
//! it that way if either is changed.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;

use crate::tui::Theme;
use crate::tui::widgets::help::{HelpOverlayState, HelpOverlayWidget, help_modal_rect};
use crate::tui::widgets::viewer_help;

const SIZES: [(u16, u16); 4] = [(60, 20), (80, 24), (120, 40), (160, 50)];

fn main_help_at(area: Rect, theme: &Theme, scroll: u16) -> Buffer {
    let mut buf = Buffer::empty(area);
    let state = HelpOverlayState {
        scroll,
        ..HelpOverlayState::default()
    };
    HelpOverlayWidget::new(&state, theme).render(area, &mut buf);
    buf
}

fn main_help(area: Rect, theme: &Theme) -> Buffer {
    main_help_at(area, theme, 0)
}

fn viewer_help_page_at(area: Rect, theme: &Theme, scroll: usize) -> Buffer {
    let mut buf = Buffer::empty(area);
    viewer_help::render(area, &mut buf, theme, scroll);
    buf
}

fn viewer_help_page(area: Rect, theme: &Theme) -> Buffer {
    viewer_help_page_at(area, theme, 0)
}

type Cell = (String, Color, Color, Modifier);

fn cell(buf: &Buffer, x: u16, y: u16) -> Cell {
    let c = &buf[(x, y)];
    (c.symbol().to_string(), c.fg, c.bg, c.modifier)
}

#[test]
fn both_helps_draw_the_same_frame() {
    let theme = Theme::default();
    for (w, h) in SIZES {
        let area = Rect::new(0, 0, w, h);
        let rect = help_modal_rect(area);
        let (a, b) = (main_help(area, &theme), viewer_help_page(area, &theme));
        let (right, bottom) = (rect.x + rect.width - 1, rect.y + rect.height - 1);
        // Corners, and the whole top and bottom borders up to the title.
        for (x, y) in [
            (rect.x, rect.y),
            (right, rect.y),
            (rect.x, bottom),
            (right, bottom),
        ] {
            assert_eq!(
                cell(&a, x, y),
                cell(&b, x, y),
                "corner ({x},{y}) at {w}x{h}"
            );
        }
        for x in rect.x..=right {
            assert_eq!(
                cell(&a, x, rect.y),
                cell(&b, x, rect.y),
                "title row x={x} at {w}x{h}"
            );
            assert_eq!(
                cell(&a, x, bottom),
                cell(&b, x, bottom),
                "bottom row x={x} at {w}x{h}"
            );
        }
        // The side borders.
        for y in rect.y..=bottom {
            assert_eq!(cell(&a, rect.x, y), cell(&b, rect.x, y), "left edge y={y}");
            assert_eq!(cell(&a, right, y), cell(&b, right, y), "right edge y={y}");
        }
    }
}

/// The styles of the rows inside the frame that start a section (bold
/// first cell), carry a key (accent-colored cells after the indent), or
/// close the page (placeholder-colored text).
fn row_styles(
    buf: &Buffer,
    area: Rect,
    theme: &Theme,
) -> (Vec<Modifier>, Vec<Color>, Vec<Modifier>) {
    let rect = help_modal_rect(area);
    let (mut headings, mut keys, mut footers) = (Vec::new(), Vec::new(), Vec::new());
    for y in rect.y + 1..rect.y + rect.height - 1 {
        let first = cell(buf, rect.x + 1, y);
        let key = cell(buf, rect.x + 3, y);
        if first.3.contains(Modifier::BOLD) && first.0 != " " {
            headings.push(first.3);
        } else if key.1 == theme.accent && key.0 != " " {
            keys.push(key.1);
        } else if first.3.contains(theme.muted) && first.0 != " " {
            footers.push(first.3);
        }
    }
    (headings, keys, footers)
}

#[test]
fn both_helps_style_headings_keys_and_footer_alike() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 120, 60);
    let (main_h, main_k, _) = row_styles(&main_help(area, &theme), area, &theme);
    let (view_h, view_k, _) = row_styles(&viewer_help_page(area, &theme), area, &theme);
    // The footer closes the page, so look at the bottom of each.
    let (_, _, main_f) = row_styles(&main_help_at(area, &theme, u16::MAX), area, &theme);
    let (_, _, view_f) = row_styles(&viewer_help_page_at(area, &theme, usize::MAX), area, &theme);
    assert!(!main_h.is_empty() && !view_h.is_empty(), "section headings");
    assert!(!main_k.is_empty() && !view_k.is_empty(), "key column");
    assert!(!main_f.is_empty() && !view_f.is_empty(), "footer");
    assert_eq!(main_h[0], view_h[0], "headings are styled alike");
    assert_eq!(main_k[0], view_k[0], "keys are colored alike");
    assert_eq!(main_f[0], view_f[0], "footers are colored alike");
}

#[test]
fn both_helps_end_with_the_same_footer() {
    let theme = Theme::default();
    let footer =
        crate::tui::keybindings::overlay::footer(crate::tui::keybindings::overlay::HELP_KEYS);
    for lines in [
        crate::tui::widgets::help::footer_lines_for_test(&theme, 100),
        viewer_help::help_lines(&theme, Some(100))
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect(),
    ] {
        let text: Vec<String> = lines;
        assert!(
            text.iter().any(|l| l.contains(&footer)),
            "footer `{footer}` missing: {text:?}"
        );
    }
}

#[test]
fn the_viewer_help_scrolls_where_the_keymap_help_does() {
    let theme = Theme::default();
    let max_scroll =
        |area: Rect| viewer_help::render(area, &mut Buffer::empty(area), &theme, 0).max_scroll;
    assert!(max_scroll(Rect::new(0, 0, 60, 12)) > 0);
    assert_eq!(max_scroll(Rect::new(0, 0, 100, 60)), 0);
}
