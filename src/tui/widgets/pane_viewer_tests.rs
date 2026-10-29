use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::content_search::MuxRef;
use tuikit::testkit::keys;

fn pane() -> PaneRef {
    PaneRef {
        mux: MuxRef {
            backend: "tmux".into(),
            socket: None,
            session: "shell".into(),
        },
        pane_id: "%0".into(),
        window_index: "0".into(),
        pane_index: "0".into(),
        command: Some("bash".into()),
    }
}

fn scrollback(count: usize) -> Vec<String> {
    (1..=count).map(|n| format!("line {n} of output")).collect()
}

fn press(state: &mut PaneViewerState, code: KeyCode) -> PaneViewerOutcome {
    state.handle_key(keys::press(code))
}

fn render(state: &mut PaneViewerState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let theme = Theme::default();
    terminal
        .draw(|frame| {
            let area = frame.area();
            draw(state, &theme, frame.buffer_mut(), area);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn it_opens_on_the_matching_line_with_context_above() {
    let mut lines = scrollback(100);
    lines[59] = "the quokka line".into();
    let mut state = PaneViewerState::open(pane(), lines, 58, "quokka");
    let screen = render(&mut state, 80, 12);
    assert!(screen.contains("shell 0.0 scrollback"), "{screen}");
    assert!(screen.contains("   60 the quokka line"), "{screen}");
    assert!(screen.contains("find \"quokka\" 1/1"), "{screen}");
    assert!(screen.contains("(a) attach"), "{screen}");
    assert_eq!(state.scroll, 57, "two lines of context above the match");
}

#[test]
fn a_scrollback_that_no_longer_holds_the_match_says_so() {
    let state = PaneViewerState::open(pane(), scrollback(10), 5, "quokka");
    assert_eq!(
        state.note.as_deref(),
        Some("the scrollback no longer holds the match")
    );
    assert!(state.current.is_none());
}

#[test]
fn movement_keys_scroll_and_page() {
    let mut state = PaneViewerState::open(pane(), scrollback(100), 1, "");
    render(&mut state, 60, 12);
    let body = usize::from(state.viewport);
    press(&mut state, KeyCode::Char('j'));
    assert_eq!(state.scroll, 1);
    press(&mut state, KeyCode::Char(' '));
    assert_eq!(state.scroll, 1 + body);
    press(&mut state, KeyCode::Char('g'));
    assert_eq!(state.scroll, 0);
    state.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    assert_eq!(state.scroll, 100 - body);
    state.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert_eq!(state.scroll, 100 - body - body / 2);
}

#[test]
fn find_steps_through_matches_and_wraps() {
    let mut lines = scrollback(50);
    for index in [5, 20, 40] {
        lines[index] = format!("a needle at {index}");
    }
    let mut state = PaneViewerState::open(pane(), lines, 1, "");
    render(&mut state, 60, 12);
    press(&mut state, KeyCode::Char('/'));
    for c in "needle".chars() {
        state.handle_key(keys::typed(c));
    }
    press(&mut state, KeyCode::Enter);
    assert_eq!(state.matches, vec![5, 20, 40]);
    assert_eq!(state.current, Some(0));

    press(&mut state, KeyCode::Char('n'));
    press(&mut state, KeyCode::Char('n'));
    assert_eq!(state.current, Some(2));
    press(&mut state, KeyCode::Char('n'));
    assert_eq!(state.current, Some(0));
    assert_eq!(state.note.as_deref(), Some("wrapped to the first match"));
    state.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
    assert_eq!(state.current, Some(2));
}

#[test]
fn esc_steps_back_from_help_then_find_then_the_viewer() {
    let mut lines = scrollback(20);
    lines[3] = "needle".into();
    let mut state = PaneViewerState::open(pane(), lines, 4, "needle");
    press(&mut state, KeyCode::Char('?'));
    assert!(state.show_help);

    assert_eq!(press(&mut state, KeyCode::Esc), PaneViewerOutcome::Continue);
    assert!(!state.show_help);
    assert!(state.find_is_active());
    assert_eq!(press(&mut state, KeyCode::Esc), PaneViewerOutcome::Continue);
    assert!(!state.find_is_active());
    assert_eq!(press(&mut state, KeyCode::Esc), PaneViewerOutcome::Close);
}

#[test]
fn q_leaves_at_once_even_with_a_find_active() {
    let mut lines = scrollback(20);
    lines[3] = "needle".into();
    let mut state = PaneViewerState::open(pane(), lines, 4, "needle");
    assert!(state.find_is_active());
    assert_eq!(
        press(&mut state, KeyCode::Char('q')),
        PaneViewerOutcome::Close
    );
}

#[test]
fn q_and_ctrl_c_leave_from_the_help_and_ctrl_c_from_the_find_prompt() {
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    for leave in [keys::typed('q'), ctrl_c] {
        let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
        press(&mut state, KeyCode::Char('?'));
        assert!(state.show_help);
        assert_eq!(state.handle_key(leave), PaneViewerOutcome::Close);
    }
    let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
    press(&mut state, KeyCode::Char('/'));
    assert!(state.prompt.is_some());
    assert_eq!(state.handle_key(ctrl_c), PaneViewerOutcome::Close);
}

#[test]
fn a_asks_to_attach() {
    let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
    assert_eq!(
        press(&mut state, KeyCode::Char('a')),
        PaneViewerOutcome::Attach
    );
}

#[test]
fn the_find_prompt_owns_the_keys_while_it_is_open() {
    let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
    press(&mut state, KeyCode::Char('/'));
    // `a` and `q` type rather than attach or leave.
    assert_eq!(
        press(&mut state, KeyCode::Char('a')),
        PaneViewerOutcome::Continue
    );
    assert_eq!(
        press(&mut state, KeyCode::Char('q')),
        PaneViewerOutcome::Continue
    );
    assert_eq!(state.prompt.as_ref().map(LineEdit::value), Some("aq"));
    press(&mut state, KeyCode::Esc);
    assert!(state.prompt.is_none());
}

#[test]
fn the_help_page_is_drawn_over_the_viewer_and_scrolls_inside_itself() {
    let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
    press(&mut state, KeyCode::Char('?'));
    let screen = render(&mut state, 90, 40);
    assert!(screen.contains("Attach to the pane's mux"), "{screen}");
    assert!(screen.contains("Esc/? back · q close"), "{screen}");
    // The page fits, so a key past its end is taken back by the draw.
    press(&mut state, KeyCode::Char('G'));
    render(&mut state, 90, 40);
    assert_eq!(state.help_scroll.offset(), 0);
}

#[test]
fn the_footer_names_the_attach_key() {
    let mut state = PaneViewerState::open(pane(), scrollback(5), 1, "");
    let screen = render(&mut state, 90, 8);
    assert!(screen.contains("(a) attach"), "{screen}");
    assert!(screen.contains("(q) close"), "{screen}");
}

#[test]
fn an_empty_scrollback_draws_a_banner() {
    let mut state = PaneViewerState::open(pane(), Vec::new(), 1, "");
    let screen = render(&mut state, 60, 8);
    assert!(screen.contains("the pane has no scrollback"), "{screen}");
}
