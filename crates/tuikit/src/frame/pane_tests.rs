use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;
use crate::testkit::buffer::rows;

fn draw(frame: PaneFrame<'_>, width: u16, height: u16) -> (Buffer, Rect) {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let inner = frame.render(area, &mut buf);
    (buf, inner)
}

/// Every cell of the border of a `width` by `height` pane, the top row's
/// title cells left out (`title` cells after the corner).
fn border_cells(width: u16, height: u16, title: u16) -> Vec<(u16, u16)> {
    let mut cells = Vec::new();
    for x in 0..width {
        if x == 0 || x > title {
            cells.push((x, 0));
        }
        cells.push((x, height - 1));
    }
    for y in 1..height - 1 {
        cells.push((0, y));
        cells.push((width - 1, y));
    }
    cells
}

#[test]
fn the_focused_pane_takes_the_focus_accent_on_its_border_and_marker() {
    let theme = Theme::default();
    let title = Line::from(vec![
        Span::styled("tabs", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(" more", Style::default().fg(Color::Green)),
    ]);
    let (buf, inner) = draw(PaneFrame::new(&theme).title(title).focused(true), 24, 5);
    let text = rows(&buf);
    assert_eq!(text[0], "┌ ► tabs more ─────────┐");
    assert_eq!(inner, Rect::new(1, 1, 22, 3));
    for (x, y) in border_cells(24, 5, 13) {
        assert_eq!(buf[(x, y)].fg, theme.panel_focus_accent, "({x}, {y})");
    }
    assert_eq!(buf[(2, 0)].symbol(), marks::POINTER);
    assert_eq!(buf[(2, 0)].fg, theme.panel_focus_accent);
    // The title keeps its own looks: the border's color does not reach
    // the spans that set none, nor the spaces around them.
    for x in [1, 4, 7, 13] {
        assert_eq!(buf[(x, 0)].fg, Color::Reset, "cell {x} of the title");
    }
    assert!(buf[(4, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(9, 0)].fg, Color::Green);
}

#[test]
fn an_unfocused_pane_keeps_the_plain_border_and_its_title_in_place() {
    let theme = Theme::default();
    let frame = || PaneFrame::new(&theme).title(Line::from("tabs"));
    let (unfocused, _) = draw(frame(), 16, 4);
    let (focused, _) = draw(frame().focused(true), 16, 4);
    assert_eq!(rows(&unfocused)[0], "┌   tabs ──────┐");
    assert_eq!(rows(&focused)[0], "┌ ► tabs ──────┐");
    for (x, y) in border_cells(16, 4, 8) {
        assert_eq!(unfocused[(x, y)].fg, Color::Reset, "({x}, {y})");
    }
    // Only the marker's cell and the border change as focus moves.
    for x in 1..9 {
        let same = unfocused[(x, 0)] == focused[(x, 0)];
        assert_eq!(same, x != 2, "cell {x} of the title");
    }
}

#[test]
fn the_title_drops_the_spaces_at_its_ends_and_closes_with_one_when_it_fits() {
    let theme = Theme::default();
    let (buf, _) = draw(
        PaneFrame::new(&theme)
            .title(Line::from("  tabs  "))
            .focused(true),
        16,
        3,
    );
    assert_eq!(rows(&buf)[0], "┌ ► tabs ──────┐");

    // A title that fills the room leaves no cell for the closing space.
    let width = 12;
    let room = PaneFrame::title_width(width);
    assert_eq!(room, 7);
    let fill = |text: &'static str| {
        rows(
            &draw(
                PaneFrame::new(&theme).title(Line::from(text)).focused(true),
                width,
                3,
            )
            .0,
        )
        .remove(0)
    };
    assert_eq!(fill("abcdefg"), "┌ ► abcdefg┐");
    assert_eq!(fill("abcdef"), "┌ ► abcdef ┐");
    // Wider than the room, the corner cuts it.
    assert_eq!(fill("abcdefghij"), "┌ ► abcdefg┐");
}

#[test]
fn a_pane_without_a_title_shows_the_marker_only_while_it_has_focus() {
    let theme = Theme::default();
    let (unfocused, _) = draw(PaneFrame::new(&theme), 10, 3);
    assert_eq!(rows(&unfocused)[0], "┌────────┐");
    let (focused, _) = draw(PaneFrame::new(&theme).focused(true), 10, 3);
    assert_eq!(rows(&focused)[0], "┌ ► ─────┐");
    let (spaces, _) = draw(PaneFrame::new(&theme).title(Line::from("   ")), 10, 3);
    assert_eq!(rows(&spaces)[0], "┌────────┐", "a title of spaces is none");
}

#[test]
fn the_focus_accent_follows_the_theme() {
    let mut theme = Theme::default();
    theme.panel_focus_accent = Color::Magenta;
    let (buf, _) = draw(PaneFrame::new(&theme).title("x").focused(true), 8, 3);
    assert_eq!(buf[(0, 0)].fg, Color::Magenta);
    assert_eq!(buf[(2, 0)].fg, Color::Magenta);
    assert_eq!(buf[(7, 2)].fg, Color::Magenta);
}

#[test]
fn the_inner_area_is_the_area_less_the_border_and_a_tiny_pane_draws() {
    let theme = Theme::default();
    let frame = PaneFrame::new(&theme);
    assert_eq!(frame.inner(Rect::new(2, 3, 10, 6)), Rect::new(3, 4, 8, 4));
    assert_eq!(PaneFrame::title_width(0), 0);
    assert_eq!(PaneFrame::title_width(4), 0);
    for (width, height) in [(0, 0), (1, 1), (2, 2), (3, 1), (5, 2)] {
        let (_, inner) = draw(
            PaneFrame::new(&theme).title("tabs").focused(true),
            width,
            height,
        );
        assert!(inner.width <= width && inner.height <= height);
    }
}
