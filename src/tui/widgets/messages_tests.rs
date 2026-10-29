use super::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::feedback::Severity;
use tuikit::overlay::Outcome;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn log_with(n: usize) -> MessageLog {
    let mut log = MessageLog::default();
    for i in 0..n {
        log.push(LogEntry::success(format!("entry {i}")));
    }
    log
}

#[test]
fn selection_moves_within_the_log_and_resets_detail_scroll() {
    let log = log_with(3);
    let mut state = MessagesOverlayState::new();
    state.handle_key(&log, press(KeyCode::Char('J')));
    assert_eq!(state.detail_scroll, 1);

    state.handle_key(&log, press(KeyCode::Char('j')));
    assert_eq!(state.selected, 1);
    assert_eq!(state.detail_scroll, 0);
    state.handle_key(&log, press(KeyCode::Char('G')));
    assert_eq!(state.selected, 2);
    state.handle_key(&log, press(KeyCode::Char('j')));
    assert_eq!(state.selected, 2, "clamps at the oldest entry");
    state.handle_key(&log, press(KeyCode::Char('g')));
    assert_eq!(state.selected, 0);
    state.handle_key(&log, press(KeyCode::Char('k')));
    assert_eq!(
        state.selected, 0,
        "clamps at the newest entry: a log does not wrap"
    );
    assert_eq!(
        state.selected_entry(&log).map(|e| e.summary.as_str()),
        Some("entry 2"),
        "index 0 is the newest entry"
    );
}

#[test]
fn close_keys_close() {
    let log = log_with(1);
    for code in [KeyCode::Esc, KeyCode::Char('q'), KeyCode::Char('!')] {
        let mut state = MessagesOverlayState::new();
        assert_eq!(state.handle_key(&log, press(code)), Outcome::Close);
    }
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
    let state = MessagesOverlayState::new();
    let theme = Theme::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    MessagesWidget::new(&state, &log, &theme, log.newest_first().next().unwrap().at)
        .render(area, &mut buf);

    let text: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                + "\n"
        })
        .collect();
    assert!(text.contains("Messages · 2"), "{text}");
    assert!(
        text.contains("×   0s ago  pin `w1` launch failed"),
        "{text}"
    );
    assert!(text.contains("attached tmux:w2"), "{text}");
    assert!(text.contains("exit: 1"), "{text}");
    assert!(text.contains("No conversation found"), "{text}");
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
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let state = MessagesOverlayState::new();
    let now = log.newest_first().next().unwrap().at;
    MessagesWidget::new(&state, &log, &theme, now).render(area, &mut buf);
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
    let mut state = MessagesOverlayState::new();
    state.handle_key(&log, press(KeyCode::Char('j')));
    let theme = Theme::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let now = log.newest_first().next().unwrap().at;
    MessagesWidget::new(&state, &log, &theme, now).render(area, &mut buf);
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
