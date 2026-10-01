use super::*;
use crate::tui::{Overlay, OverlayOutcome};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn log_with(n: usize) -> MessageLog {
    let mut log = MessageLog::default();
    for i in 0..n {
        log.push(LogEntry::info(format!("entry {i}")));
    }
    log
}

#[test]
fn selection_moves_within_the_log_and_resets_detail_scroll() {
    let log = log_with(3);
    let mut state = MessagesOverlayState::new();
    state.handle(&log, press(KeyCode::Char('J')));
    assert_eq!(state.detail_scroll, 1);

    state.handle(&log, press(KeyCode::Char('j')));
    assert_eq!(state.selected, 1);
    assert_eq!(state.detail_scroll, 0);
    state.handle(&log, press(KeyCode::Char('G')));
    assert_eq!(state.selected, 2);
    state.handle(&log, press(KeyCode::Char('j')));
    assert_eq!(state.selected, 2, "clamps at the oldest entry");
    state.handle(&log, press(KeyCode::Char('g')));
    assert_eq!(state.selected, 0);
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
        assert_eq!(state.handle(&log, press(code)), OverlayOutcome::Close);
    }
}

#[test]
fn renders_list_and_selected_detail() {
    let mut log = MessageLog::default();
    log.push(LogEntry::info("attached tmux:w2"));
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

    MessagesWidget::new(
        &state,
        &log,
        &theme,
        log.newest_first().next().unwrap().at_epoch,
    )
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
        text.contains("✗   0s ago  pin `w1` launch failed"),
        "{text}"
    );
    assert!(text.contains("attached tmux:w2"), "{text}");
    assert!(text.contains("exit: 1"), "{text}");
    assert!(text.contains("No conversation found"), "{text}");
}
