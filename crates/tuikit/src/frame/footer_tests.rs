use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::*;
use crate::keys::{Footer, KeyRow, MENU_KEYS, MenuKey, Segment, with_availability};
use crate::testkit::buffer::rows;

fn theme() -> Theme {
    Theme::default()
}

fn text(lines: &[Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

/// `MENU_KEYS`' footer: one row at 40 cells, three at 12
/// (`↑/↓ move`, `Enter pick`, `Esc close`).
fn menu(theme: &Theme) -> ModalFooter<'_> {
    ModalFooter::new(Footer::leaving_last(MENU_KEYS), theme)
}

#[test]
fn the_keys_recede_with_no_color_of_their_own() {
    let theme = theme();
    let keys = menu(&theme).keys(40);
    assert_eq!(text(&keys), ["↑/↓ move · Enter pick · Esc close"]);
    for span in keys.iter().flat_map(|line| &line.spans) {
        assert_eq!(span.style, ModalFooter::style(&theme), "{span:?}");
        assert_eq!(span.style.fg, None);
    }
    assert_eq!(
        ModalFooter::style(&theme),
        Style::default().add_modifier(theme.muted)
    );
}

#[test]
fn a_segment_that_cannot_act_keeps_its_own_look_and_recedes() {
    let table: Vec<KeyRow<MenuKey>> = with_availability(MENU_KEYS, |_| false);
    let theme = theme();
    let keys = ModalFooter::new(Footer::new(&table), &theme).keys(40);
    let segment = keys[0]
        .spans
        .iter()
        .find(|span| span.content == "Esc close")
        .expect("the segment");
    assert_eq!(segment.style.fg, Some(theme.inert));
    assert!(segment.style.add_modifier.contains(theme.muted));
}

#[test]
fn the_footer_asks_for_its_blank_row_its_status_and_its_keys() {
    let theme = theme();
    assert_eq!(menu(&theme).height(40), 2);
    assert_eq!(menu(&theme).height(12), 4);
    let status = Some(Line::from("an error"));
    assert_eq!(menu(&theme).status(status.clone()).height(40), 3);
    let empty = ModalFooter::new(Footer::default(), &theme);
    assert_eq!(empty.height(40), 0);
    assert_eq!(empty.clone().status(status).height(40), 2);
    let fit = empty.fit(40, 10);
    assert!(fit.kept.is_empty() && fit.spill.is_empty());
}

#[test]
fn with_room_the_footer_keeps_its_blank_row_its_status_and_every_key_row() {
    let theme = theme();
    let error = Line::from(Span::styled("an error", Style::default().fg(theme.error)));
    let fit = menu(&theme).status(Some(error.clone())).fit(12, 9);
    assert_eq!(
        text(&fit.kept),
        ["", "an error", "↑/↓ move", "Enter pick", "Esc close"]
    );
    assert_eq!(fit.kept[1], error, "the status line keeps its own style");
    assert!(fit.spill.is_empty());
    assert_eq!(fit.width, 12);
    assert_eq!(fit.height(), 5);
}

#[test]
fn a_short_frame_gives_up_the_blank_row_first() {
    let theme = theme();
    let fit = menu(&theme).fit(12, 3);
    assert_eq!(text(&fit.kept), ["↑/↓ move", "Enter pick", "Esc close"]);
    assert!(fit.spill.is_empty(), "nothing but the blank row went");
}

#[test]
fn then_the_keys_keep_their_last_rows_and_their_first_rows_spill() {
    let theme = theme();
    let fit = menu(&theme).fit(12, 2);
    assert_eq!(text(&fit.kept), ["Enter pick", "Esc close"]);
    assert_eq!(text(&fit.spill), ["", "↑/↓ move"]);
    let fit = menu(&theme).fit(12, 1);
    assert_eq!(text(&fit.kept), ["Esc close"]);
    assert_eq!(text(&fit.spill), ["", "↑/↓ move", "Enter pick"]);
    let fit = menu(&theme).fit(12, 0);
    assert!(fit.kept.is_empty());
    assert_eq!(fit.spill.len(), 4);
}

#[test]
fn the_status_line_stays_ahead_of_every_key_row() {
    let theme = theme();
    let footer = menu(&theme).status(Some(Line::from("/query")));
    let fit = footer.fit(12, 2);
    assert_eq!(text(&fit.kept), ["/query", "Esc close"]);
    assert_eq!(text(&fit.spill), ["", "↑/↓ move", "Enter pick"]);
    let fit = footer.fit(12, 1);
    assert_eq!(text(&fit.kept), ["/query"]);
}

#[test]
fn a_very_narrow_frame_keeps_the_keys_on_one_row() {
    let theme = theme();
    let keys = menu(&theme).keys(8);
    assert_eq!(text(&keys), ["↑/↓ move · Enter pick · Esc close"]);
    assert_eq!(menu(&theme).keys(10).len(), 3);
}

#[test]
fn on_a_bottom_border_the_footer_keeps_its_last_row() {
    let theme = theme();
    let line = menu(&theme).border(12);
    assert_eq!(line.to_string(), "Esc close");
    assert!(
        line.spans
            .iter()
            .all(|span| span.style.add_modifier.contains(Modifier::DIM))
    );
    assert_eq!(
        menu(&theme).border(40).to_string(),
        "↑/↓ move · Enter pick · Esc close"
    );
    let empty = ModalFooter::new(Footer::default(), &theme);
    assert_eq!(empty.border(40), Line::default());
}

#[test]
fn beside_a_body_that_overflows_the_footer_is_laid_out_a_cell_narrower() {
    let theme = theme();
    // `↑/↓ move · Enter pick` is 21 cells: one row at 21, two at 20.
    let footer = ModalFooter::new(
        [Segment::note("↑/↓ move"), Segment::note("Enter pick")]
            .into_iter()
            .collect(),
        &theme,
    );
    let area = Rect::new(0, 0, 21, 7);
    let fits = footer.fit_beside(area, 1, |_| 3);
    assert_eq!(fits.width, 21);
    assert_eq!(text(&fits.kept), ["", "↑/↓ move · Enter pick"]);
    let overflows = footer.fit_beside(area, 1, |_| 10);
    assert_eq!(overflows.width, 20);
    assert_eq!(text(&overflows.kept), ["", "↑/↓ move", "Enter pick"]);
    // Four rows above the footer are too few for the bar's track, so the
    // body keeps its column and the footer its width.
    let short = footer.fit_beside(Rect { height: 6, ..area }, 1, |_| 10);
    assert_eq!(short.width, 21);
    assert_eq!(text(&short.kept), ["", "↑/↓ move · Enter pick"]);
}

#[test]
fn a_fit_splits_its_area_and_draws_its_kept_rows_on_the_last() {
    // The frame page's example: three rows, a body of two entries.
    let theme = theme();
    let area = Rect::new(0, 0, 14, 3);
    let mut buf = Buffer::empty(area);
    let entries = vec![Line::from("Open"), Line::from("Rename")];
    let mut fit = menu(&theme).fit_beside(area, 1, |_| entries.len());
    let [body, keys] = fit.split(area);
    assert_eq!(body, Rect::new(0, 0, 14, 1));
    assert_eq!(keys, Rect::new(0, 1, 14, 2));
    let mut lines = entries;
    lines.append(&mut fit.spill);
    Paragraph::new(lines).render(body, &mut buf);
    fit.render(keys, &mut buf);
    let drawn = rows(&buf);
    assert_eq!(drawn[0].trim_end(), "Open");
    assert_eq!(drawn[1].trim_end(), "Enter pick");
    assert_eq!(drawn[2].trim_end(), "Esc close");
}
