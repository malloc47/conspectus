use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Padding;

use super::size::centered;
use super::*;
use crate::theme::Theme;

fn screen(width: u16, height: u16) -> Rect {
    Rect::new(0, 0, width, height)
}

/// A body that counts its measures and records the area it drew in.
struct Probe<'a> {
    rows: u16,
    measured: &'a Cell<u32>,
    width: &'a Cell<u16>,
}

impl ModalBody for Probe<'_> {
    type Output = Rect;

    fn measure(&mut self, width: u16) -> u16 {
        self.measured.set(self.measured.get() + 1);
        self.width.set(width);
        self.rows
    }

    fn render(self, area: Rect, _buf: &mut Buffer) -> Rect {
        area
    }
}

fn draw(frame: ModalFrame<'_>, area: Rect, rows: u16) -> (Buffer, Rect, u32, u16) {
    let measured = Cell::new(0);
    let width = Cell::new(0);
    let mut buf = Buffer::filled(area, ratatui::buffer::Cell::new("x"));
    let inner = frame.render(
        area,
        &mut buf,
        Probe {
            rows,
            measured: &measured,
            width: &width,
        },
    );
    (buf, inner, measured.get(), width.get())
}

fn row_text(buf: &Buffer, y: u16, xs: std::ops::Range<u16>) -> String {
    xs.map(|x| buf[(x, y)].symbol().to_string()).collect()
}

#[test]
fn an_extent_applies_cap_share_and_margin_then_the_minimum_then_the_screen() {
    let extent = Extent::new(64, 40, 2);
    assert_eq!(extent.cells(300, u16::MAX), 64, "cap");
    assert_eq!(extent.cells(66, u16::MAX), 62, "margin");
    assert_eq!(
        extent.cells(44, u16::MAX),
        40,
        "the minimum beats the margin"
    );
    assert_eq!(extent.cells(41, u16::MAX), 40);
    assert_eq!(
        extent.cells(30, u16::MAX),
        30,
        "the screen beats the minimum"
    );
    assert_eq!(extent.cells(0, u16::MAX), 0);
    assert_eq!(extent.cells(300, 50), 50, "content under the cap");
    assert_eq!(extent.cells(300, 10), 40, "content under the minimum");

    let share = Extent::new(200, 60, 2).share(85);
    assert_eq!(share.cells(100, u16::MAX), 85);
    assert_eq!(share.cells(220, u16::MAX), 187);
    assert_eq!(share.cells(300, u16::MAX), 200, "cap");
    assert_eq!(share.cells(62, u16::MAX), 60, "the minimum beats the share");
    assert_eq!(share.cells(50, u16::MAX), 50);
    // A share out of range reads as the nearest end.
    assert_eq!(Extent::new(500, 0, 0).share(0).cells(100, u16::MAX), 1);
    assert_eq!(Extent::new(500, 0, 0).share(250).cells(100, u16::MAX), 100);
}

#[test]
fn the_named_policies_size_as_their_table_says() {
    // (policy, screen, rows wanted with the frame) -> (width, height)
    type Case = (ModalSize, (u16, u16), u16, (u16, u16));
    let cases: &[Case] = &[
        (ModalSize::COMPACT, (100, 30), 3, (60, 3)),
        (ModalSize::COMPACT, (40, 12), 6, (36, 6)),
        (ModalSize::COMPACT, (22, 12), 6, (20, 6)),
        (ModalSize::COMPACT, (100, 30), 80, (60, 28)),
        (ModalSize::MENU, (100, 30), 12, (64, 12)),
        (ModalSize::MENU, (60, 20), 12, (56, 12)),
        (ModalSize::MENU, (40, 12), 30, (40, 10)),
        (ModalSize::MENU, (100, 30), 1, (64, 3)),
        (ModalSize::FORM, (100, 30), 12, (80, 12)),
        (ModalSize::FORM, (60, 20), 12, (56, 12)),
        (ModalSize::FORM, (46, 20), 12, (44, 12)),
        (ModalSize::FORM, (40, 12), 12, (40, 10)),
        (ModalSize::WIDE, (220, 50), 3, (100, 48)),
        (ModalSize::WIDE, (100, 30), 3, (96, 28)),
        (ModalSize::WIDE, (40, 12), 3, (40, 10)),
        (ModalSize::WIDE, (30, 8), 3, (30, 8)),
        (ModalSize::PALETTE, (300, 80), 3, (200, 50)),
        (ModalSize::PALETTE, (220, 50), 3, (187, 40)),
        (ModalSize::PALETTE, (100, 30), 3, (85, 24)),
        (ModalSize::PALETTE, (60, 20), 3, (60, 16)),
        (ModalSize::PALETTE, (40, 12), 3, (40, 12)),
    ];
    for &(size, (width, height), wanted, expected) in cases {
        let rect = size.rect(screen(width, height), wanted);
        assert_eq!(
            (rect.width, rect.height),
            expected,
            "{size:?} on {width}x{height} wanting {wanted}"
        );
    }
}

#[test]
fn a_modal_never_leaves_its_screen() {
    let policies = [
        ModalSize::COMPACT,
        ModalSize::MENU,
        ModalSize::FORM,
        ModalSize::WIDE,
        ModalSize::PALETTE,
    ];
    for size in policies {
        for width in 0..=120 {
            for height in [0, 1, 3, 7, 12, 30] {
                let area = Rect::new(5, 2, width, height);
                for wanted in [0, 3, 40, u16::MAX] {
                    let rect = size.rect(area, wanted);
                    assert_eq!(rect.intersection(area), rect, "{size:?} {area:?} {wanted}");
                }
            }
        }
    }
}

#[test]
fn centering_puts_the_odd_cell_first_like_ratatui() {
    for outer in 0..=40 {
        for inner in 0..=outer {
            let area = Rect::new(3, 1, outer, outer);
            let ours = centered(area, inner, inner);
            let theirs = area.centered(Constraint::Length(inner), Constraint::Length(inner));
            assert_eq!(ours, theirs, "{inner} in {outer}");
        }
    }
}

#[test]
fn the_frame_measures_once_at_its_inner_width_and_centers_on_the_rows() {
    let theme = Theme::default();
    let area = screen(80, 24);
    let frame = ModalFrame::new(&theme, ModalSize::MENU);
    assert_eq!(frame.inner_width(area), 62);
    let (buf, inner, measured, width) = draw(frame, area, 5);
    assert_eq!(measured, 1, "the body is measured once");
    assert_eq!(width, 62);
    assert_eq!(inner, Rect::new(9, 10, 62, 5));
    assert_eq!(buf[(8, 9)].symbol(), "┌");
    assert_eq!(buf[(71, 15)].symbol(), "┘");
}

#[test]
fn a_fill_frame_ignores_the_rows() {
    let theme = Theme::default();
    let area = screen(100, 30);
    let (_, small, ..) = draw(ModalFrame::new(&theme, ModalSize::WIDE), area, 1);
    let (_, large, ..) = draw(ModalFrame::new(&theme, ModalSize::WIDE), area, 200);
    assert_eq!(small, large);
    assert_eq!(small, Rect::new(3, 2, 94, 26));
}

#[test]
fn the_frame_clears_what_is_under_it_and_paints_the_surface() {
    let mut theme = Theme::default();
    theme.surface = Color::Blue;
    let area = screen(40, 12);
    let (buf, inner, ..) = draw(ModalFrame::new(&theme, ModalSize::COMPACT), area, 2);
    let rect = Rect::new(inner.x - 1, inner.y - 1, inner.width + 2, inner.height + 2);
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(x, y)];
            if rect.contains((x, y).into()) {
                assert_ne!(
                    cell.symbol(),
                    "x",
                    "({x},{y}) still shows what was under it"
                );
                assert_eq!(cell.bg, Color::Blue, "({x},{y})");
            } else {
                assert_eq!(cell.symbol(), "x", "({x},{y}) is outside the frame");
            }
        }
    }
}

#[test]
fn the_border_takes_the_accent() {
    let mut theme = Theme::default();
    theme.accent = Color::Magenta;
    let area = screen(40, 12);
    let (buf, inner, ..) = draw(ModalFrame::new(&theme, ModalSize::COMPACT), area, 1);
    let corner = &buf[(inner.x - 1, inner.y - 1)];
    assert_eq!(corner.symbol(), "┌");
    assert_eq!(corner.fg, Color::Magenta);
    assert_eq!(buf[(inner.x - 1, inner.y)].symbol(), "│");
    assert_eq!(buf[(inner.x - 1, inner.y)].fg, Color::Magenta);
}

#[test]
fn every_title_has_one_form() {
    let theme = Theme::default();
    let area = screen(60, 12);
    for title in [
        Line::from("Rename"),
        Line::from(" Rename "),
        Line::from("   Rename"),
        Line::from(vec![Span::raw("  "), Span::raw("Rename"), Span::raw(" ")]),
    ] {
        let (buf, inner, ..) = draw(
            ModalFrame::new(&theme, ModalSize::COMPACT).title(title.clone()),
            area,
            1,
        );
        let y = inner.y - 1;
        let x = inner.x - 1;
        assert_eq!(
            row_text(&buf, y, x..x + 10),
            "┌ Rename ─",
            "{title:?} draws as one form"
        );
        for x in x + 1..x + 9 {
            let cell = &buf[(x, y)];
            assert_eq!(
                cell.fg, theme.accent,
                "the title takes the border's color: {title:?}"
            );
            assert!(
                cell.modifier.is_empty(),
                "the title is regular weight: {title:?}"
            );
        }
    }
}

#[test]
fn a_title_keeps_the_look_of_its_chips() {
    let theme = Theme::default();
    let chip = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::REVERSED);
    let title = Line::from(vec![
        Span::raw("find  "),
        Span::styled("[ All ]", chip),
        Span::raw("  "),
    ]);
    let area = screen(60, 12);
    let (buf, inner, ..) = draw(
        ModalFrame::new(&theme, ModalSize::COMPACT).title(title),
        area,
        1,
    );
    let (x, y) = (inner.x - 1, inner.y - 1);
    assert_eq!(row_text(&buf, y, x..x + 17), "┌ find  [ All ] ─");
    assert!(buf[(x + 8, y)].modifier.contains(Modifier::REVERSED));
    assert!(buf[(x + 2, y)].modifier.is_empty());
}

#[test]
fn a_frame_without_a_title_draws_an_unbroken_border() {
    let theme = Theme::default();
    let area = screen(40, 12);
    for title in [Line::default(), Line::from("   ")] {
        let (buf, inner, ..) = draw(
            ModalFrame::new(&theme, ModalSize::COMPACT).title(title),
            area,
            1,
        );
        let y = inner.y - 1;
        assert_eq!(
            row_text(&buf, y, inner.x..inner.x + inner.width),
            "─".repeat(usize::from(inner.width))
        );
    }
}

#[test]
fn padding_moves_the_body_in_and_the_rows_count_it() {
    let theme = Theme::default();
    let area = screen(100, 30);
    let frame = ModalFrame::new(&theme, ModalSize::MENU).padding(Padding::new(2, 1, 1, 1));
    assert_eq!(frame.inner_width(area), 64 - 2 - 3);
    assert_eq!(frame.rect(area, 4).height, 4 + 2 + 2);
    let (buf, inner, _, width) = draw(frame, area, 4);
    assert_eq!(width, 59);
    assert_eq!(inner.height, 4);
    let rect = frame_rect(&buf);
    assert_eq!((inner.x - rect.x, inner.y - rect.y), (3, 2));
}

/// The rect of the only frame in `buf`, from its corners.
fn frame_rect(buf: &Buffer) -> Rect {
    let area = buf.area;
    let mut found = None;
    for y in 0..area.height {
        for x in 0..area.width {
            if buf[(x, y)].symbol() == "┌" {
                found = Some((x, y));
            }
        }
    }
    let (x0, y0) = found.expect("a frame");
    let x1 = (x0..area.width)
        .find(|&x| buf[(x, y0)].symbol() == "┐")
        .expect("a right corner");
    let y1 = (y0..area.height)
        .find(|&y| buf[(x0, y)].symbol() == "└")
        .expect("a bottom corner");
    Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

#[test]
fn a_tiny_screen_draws_without_panicking() {
    let theme = Theme::default();
    for (width, height) in [(0, 0), (1, 1), (2, 2), (3, 1), (5, 3)] {
        let area = screen(width, height);
        let (_, inner, ..) = draw(ModalFrame::new(&theme, ModalSize::WIDE), area, 3);
        assert!(inner.width <= width && inner.height <= height);
    }
}

#[test]
fn lines_ask_for_a_row_each_and_follow_their_span() {
    let theme = Theme::default();
    let area = screen(60, 10);
    let lines: Vec<Line> = (0..30).map(|n| Line::from(format!("line {n}"))).collect();
    let mut buf = Buffer::empty(area);
    let window = ModalFrame::new(&theme, ModalSize::MENU).render(
        area,
        &mut buf,
        Lines::new(lines.clone()).following(20..22),
    );
    // Ten rows less a margin row each side and the border: six shown.
    assert_eq!(window.viewport(), 6);
    assert_eq!(window.visible(), 16..22);
    let rect = frame_rect(&buf);
    assert_eq!(
        row_text(&buf, rect.y + 1, rect.x + 1..rect.x + 8),
        "line 16"
    );

    // Lines that fit are not scrolled, and the frame hugs them.
    let mut buf = Buffer::empty(area);
    let window = ModalFrame::new(&theme, ModalSize::MENU).render(
        area,
        &mut buf,
        Lines::new(lines[..3].to_vec()).following(2..3),
    );
    assert_eq!(window.visible(), 0..3);
    assert_eq!(frame_rect(&buf).height, 5);
}
