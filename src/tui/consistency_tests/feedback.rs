//! Feedback says how much it matters (house rule 11): the status line
//! draws a message in its severity's color, a toast frames itself in it,
//! a report puts its summary on the status line at its severity, and a
//! gesture that is refused or fails never reports like a success.

use ratatui::buffer::Buffer;
use ratatui::style::Color;
use tuikit::feedback::Severity;

use super::*;
use crate::tui::Effect;
use crate::tui::messages::{LogEntry, LogEntryExt};

/// Give the four severities four different colors (by default info and
/// success share the green).
fn distinct_severities(app: &mut App) {
    let theme = &mut app.config_mut().theme;
    theme.info = Color::Blue;
    theme.success = Color::Green;
    theme.warning = Color::Yellow;
    theme.error = Color::Red;
}

/// The first cell where `text` starts.
fn find(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    let area = buf.area;
    (area.top()..area.bottom()).find_map(|y| {
        let row: Vec<&str> = (area.left()..area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect();
        (0..row.len())
            .find(|&start| row[start..].concat().starts_with(text))
            .map(|start| (area.x + u16::try_from(start).expect("fits"), y))
    })
}

#[test]
fn the_status_line_draws_each_severity_in_its_own_color() {
    for severity in Severity::ALL {
        let mut app = app_in(View::Mux);
        distinct_severities(&mut app);
        app.update(Msg::SetStatus(Some(Feedback::new(
            severity,
            "status words",
        ))));
        let buf = buffer_of(&mut app, 120, 30);
        let (x, y) = find(&buf, "status words").expect("the message is drawn");
        assert_eq!(y, 29, "{severity:?}: on the status line");
        assert_eq!(
            buf[(x, y)].fg,
            severity.color(app.theme()),
            "{severity:?}: drawn in its own color"
        );
    }
}

#[test]
fn a_toast_frames_itself_in_its_severity_color() {
    for severity in Severity::ALL {
        let mut app = app_in(View::Mux);
        distinct_severities(&mut app);
        app.post_toast(Feedback::new(severity, "toast words"));
        let buf = buffer_of(&mut app, 100, 30);
        let (x, y) = find(&buf, "toast words").expect("the toast is drawn");
        // Its text sits inside a cell of padding and the border.
        let corner = &buf[(x - 2, y - 2)];
        assert_eq!(
            corner.symbol(),
            "\u{259b}",
            "{severity:?}: the box's corner"
        );
        assert_eq!(corner.fg, severity.color(app.theme()), "{severity:?}");
        assert_eq!(buf[(x, y)].fg, app.theme().toast_text, "{severity:?}");
        assert_eq!(y + 2, 30 - 1, "{severity:?}: on the bottom edge");
    }
}

#[test]
fn a_report_shows_its_summary_at_its_severity() {
    for severity in Severity::ALL {
        let mut app = app_in(View::Mux);
        app.report(LogEntry::of(severity, "outcome"));
        let status = app.status().expect("a report sets the status line");
        assert_eq!(status.severity, severity);
        assert_eq!(
            status.text.ends_with("· ! details"),
            severity.needs_attention(),
            "{severity:?}: only trouble points at the log"
        );
    }
}

#[test]
fn a_refused_gesture_never_reports_like_a_success() {
    // Nothing is loaded and nothing is selected, so every one of these is
    // refused, and says so.
    let mut app = App::new(RunConfig::defaults());
    let mut toasts = 0;
    for msg in [
        Msg::Undo,
        Msg::AttachSelected,
        Msg::ResumeSelected,
        Msg::ViewSelected,
        Msg::OpenRowMenu,
        Msg::TogglePin,
        Msg::RevealSelected,
        Msg::OpenSearchHistory,
    ] {
        let label = format!("{msg:?}");
        for effect in app.update(msg) {
            if let Effect::Toast(note) = effect {
                assert!(
                    note.severity.needs_attention(),
                    "{label}: `{}` is a {:?}",
                    note.text,
                    note.severity
                );
                toasts += 1;
            }
        }
    }
    assert!(toasts >= 6, "the refusals were said: {toasts}");

    app.open_value_modal_for_cursor();
    let status = app.status().expect("`o` with nothing to open says so");
    assert_eq!(status.severity, Severity::Warning, "{}", status.text);
}
