use super::*;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn esc_closes_the_modal() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    assert_eq!(
        state.handle_key(press(KeyCode::Esc)),
        ValueModalOutcome::Close
    );
}

#[test]
fn q_closes_the_modal() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    assert_eq!(
        state.handle_key(press(KeyCode::Char('q'))),
        ValueModalOutcome::Close
    );
}

#[test]
fn o_toggles_the_modal_off() {
    // Mirrors the open key so operators can press `o` twice to
    // pop the modal back closed.
    let mut state = ValueModalState::new("cwd", "/long/path");
    assert_eq!(
        state.handle_key(press(KeyCode::Char('o'))),
        ValueModalOutcome::Close
    );
}

#[test]
fn j_and_k_scroll_one_row() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    state.handle_key(press(KeyCode::Char('j')));
    assert_eq!(state.scroll, 1);
    state.handle_key(press(KeyCode::Char('j')));
    assert_eq!(state.scroll, 2);
    state.handle_key(press(KeyCode::Char('k')));
    assert_eq!(state.scroll, 1);
    // Scrolling above zero clamps.
    state.handle_key(press(KeyCode::Char('k')));
    state.handle_key(press(KeyCode::Char('k')));
    assert_eq!(state.scroll, 0);
}

#[test]
fn page_down_and_up_jump_by_a_viewport() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    state.handle_key(press(KeyCode::PageDown));
    assert_eq!(state.scroll, 8);
    state.handle_key(press(KeyCode::PageUp));
    assert_eq!(state.scroll, 0);
}

#[test]
fn g_and_capital_g_jump_to_top_and_bottom() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    state.handle_key(press(KeyCode::Char('G')));
    assert_eq!(state.scroll, u16::MAX);
    state.handle_key(press(KeyCode::Char('g')));
    assert_eq!(state.scroll, 0);
}

#[test]
fn y_is_a_copy_key_the_modal_itself_leaves_to_the_runtime() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    assert_eq!(
        state.handle_key(press(KeyCode::Char('y'))),
        ValueModalOutcome::Continue
    );
    assert_eq!(state.copy_payload(), ("/long/path", "text".to_string()));
}

#[test]
fn a_plan_copies_its_commands_not_its_prose() {
    let state = ValueModalState::plan(
        "rebuild plan",
        "rebuild mux `a`\n  1. new session\n       $ tmux new-session -d -s a",
        "# new session\ntmux new-session -d -s a\n",
        1,
    );
    assert_eq!(
        state.copy_payload(),
        (
            "# new session\ntmux new-session -d -s a\n",
            "1 command".to_string()
        )
    );
    let many = ValueModalState::plan("p", "t", "s", 3);
    assert_eq!(many.copy_payload().1, "3 commands");
}

#[test]
fn a_plan_draws_its_commands_in_the_accent_style_and_a_footer() {
    let theme = Theme::default();
    let state = ValueModalState::plan(
        "rebuild plan",
        "rebuild mux `a`\n  1. new session\n       $ tmux new-session -d -s a",
        "tmux new-session -d -s a\n",
        1,
    );
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    ValueModalWidget::new(&state, &theme).render(area, &mut buf);
    let mut command_cell = None;
    let mut text = String::new();
    for y in 0..area.height {
        let row: String = (0..area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect();
        if let Some(x) = row.find("tmux new-session") {
            let col = row[..x].chars().count() as u16;
            command_cell = Some(buf[(col, y)].style());
        }
        text.push_str(&row);
        text.push('\n');
    }
    let style = command_cell.expect("the command is drawn");
    assert_eq!(style.fg, Some(theme.panel_focus_accent));
    assert!(text.contains("y to copy"), "{text}");
    assert!(text.contains("to scroll"), "{text}");
}

#[test]
fn ctrl_c_closes_modal() {
    let mut state = ValueModalState::new("cwd", "/long/path");
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(state.handle_key(event), ValueModalOutcome::Close);
}
