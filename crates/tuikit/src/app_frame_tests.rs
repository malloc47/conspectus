use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

/// A screen that records which part was drawn where, and reports the
/// body's area.
#[derive(Default)]
struct Recorder {
    fills: bool,
    calls: Vec<(&'static str, Rect)>,
}

impl Screen for Recorder {
    type Report = Option<Rect>;

    fn header(&mut self, area: Rect, _buf: &mut Buffer) {
        self.calls.push(("header", area));
    }

    fn body(&mut self, area: Rect, _buf: &mut Buffer) -> Option<Rect> {
        self.calls.push(("body", area));
        Some(area)
    }

    fn footer(&mut self, area: Rect, _buf: &mut Buffer) {
        self.calls.push(("footer", area));
    }

    fn overlays_fill_screen(&self) -> bool {
        self.fills
    }

    fn overlays(&mut self, area: Rect, _buf: &mut Buffer) {
        self.calls.push(("overlays", area));
    }

    fn toasts(&mut self, area: Rect, _buf: &mut Buffer) {
        self.calls.push(("toasts", area));
    }
}

#[test]
fn the_parts_draw_in_layer_order_and_the_body_report_comes_back() {
    let area = Rect::new(0, 0, 50, 12);
    let mut buf = Buffer::empty(area);
    let mut screen = Recorder::default();
    let report = AppFrame::new().render(area, &mut buf, &mut screen);
    assert_eq!(report, Some(Rect::new(0, 1, 50, 10)));
    assert_eq!(
        screen.calls,
        [
            ("header", Rect::new(0, 0, 50, 1)),
            ("body", Rect::new(0, 1, 50, 10)),
            ("footer", Rect::new(0, 11, 50, 1)),
            ("overlays", area),
            ("toasts", area),
        ]
    );
}

#[test]
fn an_overlay_that_fills_the_screen_hides_the_chrome() {
    let area = Rect::new(0, 0, 50, 12);
    let mut buf = Buffer::empty(area);
    let mut screen = Recorder {
        fills: true,
        ..Recorder::default()
    };
    let report = AppFrame::new().render(area, &mut buf, &mut screen);
    assert_eq!(report, None, "the report's default: no body was drawn");
    assert_eq!(screen.calls, [("overlays", area), ("toasts", area)]);
}

#[test]
fn the_header_and_footer_rows_are_settable_and_the_body_takes_the_rest() {
    let area = Rect::new(2, 3, 40, 20);
    let areas = AppFrame::new().header_rows(2).footer_rows(3).areas(area);
    assert_eq!(areas.header, Rect::new(2, 3, 40, 2));
    assert_eq!(areas.body, Rect::new(2, 5, 40, 15));
    assert_eq!(areas.footer, Rect::new(2, 20, 40, 3));

    let bare = AppFrame::new().header_rows(0).footer_rows(0).areas(area);
    assert_eq!(bare.body, area);
    assert_eq!((bare.header.height, bare.footer.height), (0, 0));
    assert_eq!(AppFrame::default(), AppFrame::new());
}

#[test]
fn on_a_short_screen_the_body_keeps_its_rows_first() {
    let frame = AppFrame::new();
    for height in 0..=8 {
        let areas = frame.areas(Rect::new(0, 0, 30, height));
        let total = areas.header.height + areas.body.height + areas.footer.height;
        assert_eq!(total, height, "{height}: the parts fill the screen");
        assert_eq!(
            areas.body.height,
            height.min(AppFrame::MIN_BODY).max(height.saturating_sub(2)),
            "{height}"
        );
        assert_eq!(areas.header.y, 0, "{height}");
        assert_eq!(areas.body.y, areas.header.bottom(), "{height}");
        assert_eq!(areas.footer.y, areas.body.bottom(), "{height}");
    }
}

#[test]
fn the_frame_draws_inside_the_buffer() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 6));
    let mut screen = Recorder::default();
    let report = AppFrame::new().render(Rect::new(0, 0, 80, 24), &mut buf, &mut screen);
    assert_eq!(report, Some(Rect::new(0, 1, 20, 4)));
    assert_eq!(
        screen.calls.last(),
        Some(&("toasts", Rect::new(0, 0, 20, 6)))
    );
}
