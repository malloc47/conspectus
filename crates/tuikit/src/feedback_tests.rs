//! Severity and feedback: each severity reads its own theme token, the
//! failures need attention, and errors stay longest.

use std::time::Duration;

use ratatui::style::{Color, Style};

use super::*;

#[test]
fn each_severity_takes_its_own_token() {
    let mut theme = Theme::default();
    theme.info = Color::Blue;
    theme.success = Color::Green;
    theme.warning = Color::Yellow;
    theme.error = Color::Red;
    let colors: Vec<Color> = Severity::ALL.iter().map(|s| s.color(&theme)).collect();
    assert_eq!(
        colors,
        [Color::Blue, Color::Green, Color::Yellow, Color::Red]
    );
    for severity in Severity::ALL {
        assert_eq!(
            severity.style(&theme),
            Style::default().fg(severity.color(&theme))
        );
        assert_eq!(
            severity.tone().style(&theme),
            severity.style(&theme),
            "{severity:?}: the chip tone and the severity agree"
        );
    }
}

#[test]
fn only_warnings_and_errors_need_attention() {
    let attention: Vec<bool> = Severity::ALL.iter().map(|s| s.needs_attention()).collect();
    assert_eq!(attention, [false, false, true, true]);
    assert!(Severity::Info < Severity::Success);
    assert!(Severity::Warning < Severity::Error);
}

#[test]
fn errors_stay_longest_and_confirmations_keep_the_short_lifetime() {
    assert_eq!(Severity::Success.ttl(), Duration::from_millis(1500));
    assert_eq!(Severity::Info.ttl(), Severity::Success.ttl());
    assert!(Severity::Warning.ttl() > Severity::Success.ttl());
    assert!(Severity::Error.ttl() > Severity::Warning.ttl());
}

#[test]
fn each_severity_has_its_own_one_cell_mark() {
    let marks: Vec<&str> = Severity::ALL.iter().map(|s| s.mark()).collect();
    assert_eq!(
        marks,
        [marks::BULLET, marks::OK, marks::WARNING, marks::FAIL]
    );
    for mark in marks {
        assert_eq!(crate::text::width(mark), 1, "{mark}");
    }
}

#[test]
fn feedback_takes_its_severity_lifetime_unless_given_one() {
    let error = Feedback::error("copy failed");
    assert_eq!(error.severity, Severity::Error);
    assert_eq!(error.text, "copy failed");
    assert_eq!(error.ttl, Severity::Error.ttl());

    let brief = Feedback::warning("nothing to undo").with_ttl(Duration::from_millis(200));
    assert_eq!(brief.ttl, Duration::from_millis(200));

    let constructors = [
        Feedback::info("a"),
        Feedback::success("a"),
        Feedback::warning("a"),
        Feedback::error("a"),
    ];
    let severities: Vec<Severity> = constructors.iter().map(|f| f.severity).collect();
    assert_eq!(severities, Severity::ALL);
}

#[test]
fn the_status_line_span_is_the_text_in_its_severity_color() {
    let theme = Theme::default();
    for severity in Severity::ALL {
        let span = Feedback::new(severity, "renamed").span(&theme);
        assert_eq!(span.content, "renamed");
        assert_eq!(span.style, severity.style(&theme), "{severity:?}");
    }
    assert_ne!(
        Feedback::error("x").span(&theme).style,
        Feedback::success("x").span(&theme).style,
        "a failure does not look like a success"
    );
}
