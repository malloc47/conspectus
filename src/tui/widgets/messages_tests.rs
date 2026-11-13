use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::feedback::Severity;
use tuikit::glyphs::marks;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::testkit::keys::{ctrl, press};

use super::*;
use crate::tui::Theme;

/// The overlay over `log` itself, aged against the newest entry's time.
fn list_over(log: &MessageLog) -> LogList<Report, Msg, MessageLog> {
    let now = log.newest_first().next().map_or(0, |entry| entry.at);
    over(|log: &MessageLog| log, move |_| now)
}

fn log_with(n: usize) -> MessageLog {
    let mut log = MessageLog::default();
    for i in 0..n {
        log.push(LogEntry::success(format!("entry {i}")));
    }
    log
}

fn draw(list: &mut LogList<Report, Msg, MessageLog>, log: &MessageLog, theme: &Theme) -> Buffer {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    list.draw(area, &mut buf, log, theme);
    buf
}

fn text_of(buf: &Buffer) -> String {
    tuikit::testkit::buffer::text(buf)
}

#[test]
fn selection_moves_within_the_log_and_resets_detail_scroll() {
    let mut log = log_with(2);
    let detail: Vec<String> = (1..=60).map(|n| format!("line {n}")).collect();
    log.push(LogEntry::error("launch failed").with_detail(detail.join("\n")));
    let theme = Theme::default();
    let mut list = list_over(&log);
    draw(&mut list, &log, &theme);
    list.handle(press(KeyCode::Char('J')), &log);
    draw(&mut list, &log, &theme);
    assert_eq!(list.detail_window().offset(), 1);

    list.handle(press(KeyCode::Char('j')), &log);
    draw(&mut list, &log, &theme);
    assert_eq!(list.selected(), 1);
    assert_eq!(list.detail_window().offset(), 0);
    list.handle(press(KeyCode::Char('G')), &log);
    assert_eq!(list.selected(), 2);
    list.handle(press(KeyCode::Char('j')), &log);
    assert_eq!(list.selected(), 2, "clamps at the oldest entry");
    list.handle(press(KeyCode::Char('g')), &log);
    assert_eq!(list.selected(), 0);
    list.handle(press(KeyCode::Char('k')), &log);
    assert_eq!(
        list.selected(),
        0,
        "clamps at the newest entry: a log does not wrap"
    );
    assert_eq!(
        list.selected_entry(&log).map(|e| e.summary.as_str()),
        Some("launch failed"),
        "index 0 is the newest entry"
    );
}

#[test]
fn esc_and_bang_close_and_q_and_ctrl_c_dismiss() {
    let log = log_with(1);
    for code in [KeyCode::Esc, KeyCode::Char('!')] {
        assert_eq!(list_over(&log).handle(press(code), &log), Outcome::Close);
    }
    assert_eq!(
        list_over(&log).handle(press(KeyCode::Char('q')), &log),
        Outcome::Dismiss
    );
    assert_eq!(list_over(&log).handle(ctrl('c'), &log), Outcome::Dismiss);
}

#[test]
fn y_asks_the_runtime_to_copy_the_selected_record() {
    let log = log_with(2);
    let mut list = list_over(&log);
    list.handle(press(KeyCode::Char('j')), &log);
    let entry = list.selected_entry(&log).expect("an entry");
    assert_eq!(
        list.handle(press(KeyCode::Char('y')), &log),
        Outcome::Stay(Msg::Copy {
            text: entry.full_text(),
            done: "copied message".into(),
        })
    );
}

#[test]
fn renders_list_and_selected_detail() {
    let mut log = MessageLog::default();
    log.push(LogEntry::success("attached tmux:w2"));
    log.push(LogEntry::error("pin `w1` launch failed").with_command(
        crate::tui::messages::CommandRecord {
            argv: vec!["conspectus".into(), "pin".into()],
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "No conversation found".into(),
        },
    ));
    let theme = Theme::default();
    let text = text_of(&draw(&mut list_over(&log), &log, &theme));
    assert!(text.contains("Messages · 2"), "{text}");
    assert!(
        text.contains("×   0s ago  pin `w1` launch failed"),
        "{text}"
    );
    assert!(text.contains("attached tmux:w2"), "{text}");
    assert!(text.contains("exit: 1"), "{text}");
    assert!(text.contains("No conversation found"), "{text}");
    assert!(
        text.contains("j/k select · J/K scroll detail · y copy · Esc close"),
        "{text}"
    );
}

#[test]
fn an_empty_log_says_what_will_be_listed() {
    let log = MessageLog::default();
    let theme = Theme::default();
    let text = text_of(&draw(&mut list_over(&log), &log, &theme));
    assert!(text.contains("Messages · 0"), "{text}");
    assert!(
        text.contains("No messages yet. Launches, attaches, renames and"),
        "{text}"
    );
    assert!(
        text.contains("failures from this session are listed here."),
        "{text}"
    );
}

#[test]
fn each_severity_takes_its_own_theme_color() {
    use crate::tui::theme::OverrideValue::Text;
    let mut theme = Theme::default();
    let problems = theme.apply_overrides([("info", Text("blue")), ("success", Text("magenta"))]);
    assert_eq!(problems, []);
    let mut log = MessageLog::default();
    for severity in Severity::ALL {
        log.push(LogEntry::of(severity, format!("{severity:?} entry")));
    }
    let buf = draw(&mut list_over(&log), &log, &theme);
    let area = buf.area;
    for severity in Severity::ALL {
        let (x, y) = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .find(|(x, y)| buf[(*x, *y)].symbol() == severity.mark())
            .unwrap_or_else(|| panic!("{severity:?}: its mark is drawn"));
        let row: String = (x..area.width).map(|x| buf[(x, y)].symbol()).collect();
        assert!(row.contains(&format!("{severity:?} entry")), "{row}");
        assert_eq!(buf[(x, y)].fg, severity.color(&theme), "{severity:?}");
    }
    let default = Theme::default();
    assert_eq!(
        Severity::Success.color(&default),
        Severity::Info.color(&default),
        "by default a success is the green an info entry always was"
    );
}

#[test]
fn the_selected_entry_shows_the_marker_and_the_highlight() {
    let log = log_with(3);
    let mut list = list_over(&log);
    list.handle(press(KeyCode::Char('j')), &log);
    let theme = Theme::default();
    let buf = draw(&mut list, &log, &theme);
    let area = buf.area;
    let marked: Vec<(u16, u16)> = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|(x, y)| buf[(*x, *y)].symbol() == marks::POINTER)
        .collect();
    assert_eq!(marked.len(), 1, "one marked entry");
    let (x, y) = marked[0];
    let row: String = (x..area.width).map(|x| buf[(x, y)].symbol()).collect();
    assert!(row.contains("entry 1"), "{row}");
    assert!(buf[(x, y)].modifier.contains(theme.selection_active));
    // The other entries keep their text a gutter in, unmarked.
    let other: String = (x..x + 2).map(|x| buf[(x, y - 1)].symbol()).collect();
    assert_eq!(other, "  ");
}
