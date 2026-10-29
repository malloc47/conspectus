//! Toasts: replacing and stacking, per-toast lifetimes, and the drawn
//! box (its place, its size, and its colors by severity).

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::*;
use crate::feedback::Severity;

/// The text of row `y` of `buf`.
fn row(buf: &Buffer, y: u16) -> String {
    cells(buf, buf.area.x, y, buf.area.width)
}

/// The text of `width` cells of `buf` from `x` on row `y`.
fn cells(buf: &Buffer, x: u16, y: u16, width: u16) -> String {
    (x..x + width).map(|x| buf[(x, y)].symbol()).collect()
}

/// Draw `toasts` on an empty `width` x `height` screen.
fn drawn(toasts: &Toasts, width: u16, height: u16, theme: &Theme) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    toasts.render(area, &mut buf, theme);
    buf
}

fn one(feedback: Feedback) -> Toasts {
    let mut toasts = Toasts::new();
    toasts.show(feedback, Instant::now());
    toasts
}

#[test]
fn a_fresh_stack_shows_nothing() {
    let toasts = Toasts::default();
    assert!(toasts.is_empty());
    assert_eq!(toasts.len(), 0);
    assert!(toasts.areas(Rect::new(0, 0, 80, 24)).is_empty());
}

#[test]
fn show_replaces_and_push_stacks() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    toasts.push(Feedback::success("a"), now);
    toasts.push(Feedback::success("b"), now);
    assert_eq!(toasts.len(), 2);

    toasts.show(Feedback::warning("c"), now);
    let texts: Vec<&str> = toasts.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(texts, ["c"], "showing one takes the others away");

    toasts.clear();
    assert!(toasts.is_empty());
}

#[test]
fn a_full_stack_drops_its_oldest() {
    let now = Instant::now();
    let mut toasts = Toasts::with_capacity(2);
    for text in ["a", "b", "c"] {
        toasts.push(Feedback::info(text), now);
    }
    let texts: Vec<&str> = toasts.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(texts, ["b", "c"]);
    assert_eq!(
        Toasts::with_capacity(0).capacity,
        1,
        "it keeps at least one"
    );
}

#[test]
fn each_toast_goes_when_its_own_time_is_up() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    toasts.push(Feedback::success("copied"), now);
    toasts.push(Feedback::error("copy failed"), now);

    assert!(!toasts.tick(now), "nothing is due yet");
    assert!(!toasts.tick(now + Severity::Success.ttl() - Duration::from_millis(1)));
    assert_eq!(toasts.len(), 2);

    assert!(
        toasts.tick(now + Severity::Success.ttl()),
        "the success went"
    );
    let left: Vec<Severity> = toasts.iter().map(|f| f.severity).collect();
    assert_eq!(left, [Severity::Error], "the error stays longer");

    assert!(toasts.tick(now + Severity::Error.ttl()));
    assert!(toasts.is_empty());
}

#[test]
fn a_lifetime_past_the_clock_stays_until_cleared() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    toasts.show(Feedback::warning("sticky").with_ttl(Duration::MAX), now);
    assert!(!toasts.tick(now + Duration::from_secs(86_400)));
    assert_eq!(toasts.len(), 1);
    toasts.clear();
    assert!(toasts.is_empty());
}

#[test]
fn a_feedback_lifetime_overrides_its_severity() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    toasts.show(
        Feedback::error("brief").with_ttl(Duration::from_millis(10)),
        now,
    );
    assert!(toasts.tick(now + Duration::from_millis(10)));
}

#[test]
fn a_success_toast_is_a_quadrant_box_centered_on_the_bottom_edge() {
    let theme = Theme::default();
    let buf = drawn(&one(Feedback::success("copied: cwd")), 100, 30, &theme);
    // 11 cells of text, a cell of padding and a border on each side: 15
    // cells across, 5 rows down, the odd cell of the 85 spare before it.
    assert_eq!(
        one(Feedback::success("copied: cwd")).areas(Rect::new(0, 0, 100, 30)),
        [Rect::new(43, 25, 15, 5)]
    );
    let boxed: Vec<String> = (25..30).map(|y| cells(&buf, 43, y, 15)).collect();
    assert_eq!(
        boxed,
        [
            "\u{259b}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{2580}\u{259c}",
            "\u{258c}             \u{2590}",
            "\u{258c} copied: cwd \u{2590}",
            "\u{258c}             \u{2590}",
            "\u{2599}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{2584}\u{259f}",
        ]
    );
    // The border takes the success color, the text the toast text color,
    // both on the surface.
    let corner = &buf[(43, 25)];
    assert_eq!((corner.fg, corner.bg), (theme.success, theme.surface));
    let letter = &buf[(45, 27)];
    assert_eq!(letter.symbol(), "c");
    assert_eq!((letter.fg, letter.bg), (theme.toast_text, theme.surface));
    assert_eq!(theme.toast_text, Color::White, "the default text color");
    let padding = &buf[(44, 27)];
    assert_eq!((padding.fg, padding.bg), (Color::Reset, theme.surface));
}

#[test]
fn the_border_takes_the_color_of_each_severity() {
    let mut theme = Theme::default();
    theme.info = Color::Blue;
    theme.success = Color::Magenta;
    for severity in Severity::ALL {
        let buf = drawn(&one(Feedback::new(severity, "done")), 40, 10, &theme);
        let rect = one(Feedback::new(severity, "done")).areas(buf.area)[0];
        for (x, y) in [
            (rect.x, rect.y),
            (rect.right() - 1, rect.y),
            (rect.x, rect.bottom() - 1),
            (rect.x + 3, rect.y),
            (rect.x, rect.y + 2),
        ] {
            assert_eq!(
                buf[(x, y)].fg,
                severity.color(&theme),
                "{severity:?} at {x},{y}"
            );
        }
        assert_eq!(buf[(rect.x + 2, rect.y + 2)].fg, theme.toast_text);
    }
}

#[test]
fn a_long_text_wraps_at_the_widest_toast() {
    let theme = Theme::default();
    let text = "saved notes to slot 1; press u to undo the change you just made here";
    let toasts = one(Feedback::success(text));
    let [rect] = toasts.areas(Rect::new(0, 0, 100, 30))[..] else {
        panic!("one toast");
    };
    assert_eq!(
        (rect.width, rect.height),
        (50, 6),
        "46 cells of text and two rows"
    );
    let buf = drawn(&toasts, 100, 30, &theme);
    assert_eq!(
        cells(&buf, 27, 26, 46).trim_end(),
        "saved notes to slot 1; press u to undo the"
    );
    assert_eq!(
        cells(&buf, 27, 27, 46).trim_end(),
        "change you just made here"
    );
}

#[test]
fn on_a_narrow_screen_the_text_wraps_inside_it_instead_of_being_cut() {
    let theme = Theme::default();
    let text = "could not write the store: permission denied";
    let toasts = one(Feedback::error(text));
    let [rect] = toasts.areas(Rect::new(0, 0, 30, 12))[..] else {
        panic!("one toast");
    };
    assert_eq!(rect.width, 30, "the box fills the screen's width");
    let buf = drawn(&toasts, 30, 12, &theme);
    let body: String = (rect.y + 2..rect.bottom() - 2)
        .map(|y| row(&buf, y).replace(['\u{258c}', '\u{2590}'], ""))
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(body, text, "every word is on screen");
}

#[test]
fn wide_characters_are_measured_in_cells() {
    let toasts = one(Feedback::info("\u{65e5}\u{672c}"));
    let [rect] = toasts.areas(Rect::new(0, 0, 40, 10))[..] else {
        panic!("one toast");
    };
    assert_eq!(rect.width, 8, "two characters, four cells, and the chrome");
}

#[test]
fn a_stack_draws_the_newest_at_the_bottom_and_older_ones_above() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    toasts.push(Feedback::success("first"), now);
    toasts.push(Feedback::error("second"), now);
    let area = Rect::new(0, 0, 40, 20);
    let areas = toasts.areas(area);
    assert_eq!(areas.len(), 2);
    assert_eq!(
        areas[1].bottom(),
        area.bottom(),
        "the newest is at the bottom"
    );
    assert_eq!(areas[0].bottom(), areas[1].y, "the older one sits on it");

    let theme = Theme::default();
    let buf = drawn(&toasts, 40, 20, &theme);
    assert_eq!(buf[(areas[0].x, areas[0].y)].fg, theme.success);
    assert_eq!(buf[(areas[1].x, areas[1].y)].fg, theme.error);
}

#[test]
fn toasts_with_no_room_left_are_not_drawn() {
    let now = Instant::now();
    let mut toasts = Toasts::new();
    for text in ["a", "b", "c"] {
        toasts.push(Feedback::info(text), now);
    }
    let areas = toasts.areas(Rect::new(0, 0, 20, 8));
    assert_eq!(areas.len(), 2, "two five-row boxes do not fit eight rows");
    assert_eq!(areas[1].height, 5);
    assert_eq!(areas[0].height, 3, "the older one gets what is left");
    assert!(toasts.areas(Rect::new(0, 0, 0, 0)).is_empty());
}
