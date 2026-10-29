use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use tuikit::keys::READER_KEYS;
use tuikit::overlay::Outcome;
use tuikit::text_edit::LineEdit;

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

fn press(viewer: &mut PaneViewer, code: KeyCode) -> Outcome<Msg> {
    viewer.handle_key(keys::press(code))
}

fn draw(viewer: &mut PaneViewer, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    viewer.draw(area, &mut buf, &crate::tui::Theme::default());
    buf
}

fn render(viewer: &mut PaneViewer, width: u16, height: u16) -> String {
    tuikit::testkit::buffer::rows(&draw(viewer, width, height))
        .iter()
        .map(|row| row.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn it_opens_on_the_matching_line_with_a_row_of_context_above() {
    let mut lines = scrollback(100);
    lines[59] = "the quokka line".into();
    let mut viewer = open(pane(), lines, 58, "quokka");
    let screen = render(&mut viewer, 80, 12);
    assert!(
        screen.contains("shell 0.0 scrollback · bash · 100 lines"),
        "{screen}"
    );
    assert!(screen.contains("   60 the quokka line"), "{screen}");
    assert!(screen.contains("find \"quokka\" 1/1"), "{screen}");
    assert!(screen.contains("a attach"), "{screen}");
    assert_eq!(
        viewer.window().offset(),
        58,
        "one line of context above the match"
    );
}

#[test]
fn a_scrollback_that_no_longer_holds_the_match_says_so() {
    let mut viewer = open(pane(), scrollback(10), 5, "quokka");
    assert_eq!(
        viewer.note().map(|note| note.text.as_str()),
        Some("the scrollback no longer holds the match")
    );
    assert!(viewer.find().current().is_none());
    render(&mut viewer, 80, 8);
    assert_eq!(
        viewer.window().offset(),
        3,
        "on the line asked for, less a line of context"
    );
}

#[test]
fn movement_keys_scroll_and_page() {
    let mut viewer = open(pane(), scrollback(100), 1, "");
    render(&mut viewer, 60, 12);
    let body = viewer.window().viewport();
    let offset = |viewer: &mut PaneViewer, key: KeyEvent| {
        viewer.handle_key(key);
        render(viewer, 60, 12);
        viewer.window().offset()
    };
    assert_eq!(offset(&mut viewer, keys::typed('j')), 1);
    assert_eq!(offset(&mut viewer, keys::typed(' ')), 1 + body);
    assert_eq!(offset(&mut viewer, keys::typed('g')), 0);
    let shift_g = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_eq!(offset(&mut viewer, shift_g), 100 - body);
    let ctrl_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(offset(&mut viewer, ctrl_u), 100 - body - body / 2);
}

#[test]
fn find_steps_through_matches_and_wraps() {
    let mut lines = scrollback(50);
    for index in [5, 20, 40] {
        lines[index] = format!("a needle at {index}");
    }
    let mut viewer = open(pane(), lines, 1, "");
    render(&mut viewer, 60, 12);
    press(&mut viewer, KeyCode::Char('/'));
    for c in "needle".chars() {
        viewer.handle_key(keys::typed(c));
    }
    press(&mut viewer, KeyCode::Enter);
    assert_eq!(viewer.find().matches(), [5, 20, 40]);
    assert_eq!(viewer.find().current(), Some(0));

    press(&mut viewer, KeyCode::Char('n'));
    press(&mut viewer, KeyCode::Char('n'));
    assert_eq!(viewer.find().current(), Some(2));
    press(&mut viewer, KeyCode::Char('n'));
    assert_eq!(viewer.find().current(), Some(0));
    assert_eq!(
        viewer.note().map(|note| note.text.as_str()),
        Some("wrapped to the first match")
    );
    viewer.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
    assert_eq!(viewer.find().current(), Some(2));
}

#[test]
fn esc_steps_back_from_help_then_find_then_the_viewer() {
    let mut lines = scrollback(20);
    lines[3] = "needle".into();
    let mut viewer = open(pane(), lines, 4, "needle");
    press(&mut viewer, KeyCode::Char('?'));
    assert!(viewer.help_is_open());

    assert_eq!(press(&mut viewer, KeyCode::Esc), Outcome::Consumed);
    assert!(!viewer.help_is_open());
    assert!(viewer.find().is_active());
    assert_eq!(press(&mut viewer, KeyCode::Esc), Outcome::Consumed);
    assert!(!viewer.find().is_active());
    assert_eq!(press(&mut viewer, KeyCode::Esc), Outcome::Close);
}

#[test]
fn q_leaves_at_once_even_with_a_find_active() {
    let mut lines = scrollback(20);
    lines[3] = "needle".into();
    let mut viewer = open(pane(), lines, 4, "needle");
    assert!(viewer.find().is_active());
    assert_eq!(press(&mut viewer, KeyCode::Char('q')), Outcome::Dismiss);
}

#[test]
fn q_and_ctrl_c_leave_from_the_help_and_ctrl_c_from_the_find_prompt() {
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    for leave in [keys::typed('q'), ctrl_c] {
        let mut viewer = open(pane(), scrollback(5), 1, "");
        press(&mut viewer, KeyCode::Char('?'));
        assert!(viewer.help_is_open());
        assert_eq!(viewer.handle_key(leave), Outcome::Dismiss);
    }
    let mut viewer = open(pane(), scrollback(5), 1, "");
    press(&mut viewer, KeyCode::Char('/'));
    assert!(viewer.find().prompt().is_some());
    assert_eq!(viewer.handle_key(ctrl_c), Outcome::Dismiss);
}

#[test]
fn a_asks_to_attach() {
    let mut viewer = open(pane(), scrollback(5), 1, "");
    assert_eq!(
        press(&mut viewer, KeyCode::Char('a')),
        Outcome::Stay(Msg::AttachPane)
    );
}

#[test]
fn the_find_prompt_owns_the_keys_while_it_is_open() {
    let mut viewer = open(pane(), scrollback(5), 1, "");
    press(&mut viewer, KeyCode::Char('/'));
    // `a` and `q` type rather than attach or leave.
    assert_eq!(press(&mut viewer, KeyCode::Char('a')), Outcome::Consumed);
    assert_eq!(press(&mut viewer, KeyCode::Char('q')), Outcome::Consumed);
    assert_eq!(viewer.find().prompt().map(LineEdit::value), Some("aq"));
    press(&mut viewer, KeyCode::Esc);
    assert!(viewer.find().prompt().is_none());
}

#[test]
fn the_help_page_is_drawn_over_the_viewer_and_scrolls_inside_itself() {
    let mut viewer = open(pane(), scrollback(5), 1, "");
    press(&mut viewer, KeyCode::Char('?'));
    let screen = render(&mut viewer, 90, 40);
    assert!(screen.contains("Attach to the pane's mux"), "{screen}");
    assert!(
        screen.contains("Wrap long lines, or cut them at the edge"),
        "{screen}"
    );
    assert!(screen.contains("Esc/? back · q close"), "{screen}");
    // The pane's own words for the pager's ends and find.
    for words in [
        "Jump to the oldest line",
        "Jump to the newest line",
        "Find in the scrollback",
    ] {
        assert!(screen.contains(words), "{words}: {screen}");
    }
    // The page fits, so a key past its end is taken back by the draw.
    press(&mut viewer, KeyCode::Char('G'));
    render(&mut viewer, 90, 40);
    let offset = viewer.help_page().map(|help| help.window().offset());
    assert_eq!(offset, Some(0));
}

#[test]
fn the_help_page_describes_every_key_the_viewer_binds() {
    let viewer = open(pane(), scrollback(5), 1, "");
    let titles: Vec<String> = viewer
        .help_sections()
        .iter()
        .map(|section| section.title.to_string())
        .collect();
    assert_eq!(titles, ["Moving", "Finding", "Pane", "Viewer"]);
    assert!(tuikit::help::undescribed(READER_KEYS).is_empty());
    assert!(tuikit::help::undescribed(PANE_KEYS).is_empty());
}

#[test]
fn the_footer_is_generated_and_names_the_attach_key() {
    let mut viewer = open(pane(), scrollback(5), 1, "");
    let screen = render(&mut viewer, 110, 8);
    let footer = screen.lines().last().unwrap_or_default().to_string();
    assert_eq!(
        footer,
        "[ 1/5 ] · wrap·on  (w) · j/k/PgDn/PgUp scroll · / find · ? help · a attach · Esc/q close"
    );
    assert_eq!(
        viewer.footer().text(),
        "j/k/PgDn/PgUp scroll · / find · ? help · a attach · Esc/q close"
    );
    // At 40 columns the attach and close keys stay.
    let narrow = render(&mut viewer, 40, 8);
    assert!(
        narrow.ends_with("[ 1/5 ] · a attach · Esc/q close"),
        "{narrow}"
    );
}

#[test]
fn long_lines_wrap_under_their_number_and_w_cuts_them_at_the_edge() {
    let mut lines = scrollback(3);
    lines[1] = "a long line ".repeat(8);
    let mut viewer = open(pane(), lines, 1, "");
    let screen = render(&mut viewer, 40, 12);
    let rows: Vec<&str> = screen.lines().collect();
    assert!(
        rows[3].starts_with("    2 a long line a long line a long"),
        "{screen}"
    );
    assert!(
        rows[4].starts_with("      line"),
        "a hanging indent: {screen}"
    );
    assert!(screen.contains("    3 line 3 of output"), "{screen}");
    assert_eq!(
        viewer.window().content(),
        5,
        "the long line takes three rows"
    );

    press(&mut viewer, KeyCode::Char('w'));
    let screen = render(&mut viewer, 40, 12);
    let rows: Vec<&str> = screen.lines().collect();
    assert_eq!(
        rows[3], "    2 a long line a long line a long li…",
        "{screen}"
    );
    assert_eq!(rows[4], "    3 line 3 of output");
    // The chip says so where the footer has room for it.
    let wide = render(&mut viewer, 100, 12);
    assert!(wide.contains("wrap·off (w)"), "{wide}");
}

#[test]
fn a_match_on_a_wrapped_line_is_scrolled_to_and_its_number_marked() {
    let mut lines = scrollback(60);
    lines[40] = format!("{} needle", "padding ".repeat(10));
    let mut viewer = open(pane(), lines, 1, "needle");
    let theme = crate::tui::Theme::default();
    let buf = draw(&mut viewer, 50, 12);
    let rows = tuikit::testkit::buffer::rows(&buf);
    // A row of context above the line's first row.
    assert!(rows[3].starts_with("   41 padding"), "{rows:?}");
    let number = tuikit::testkit::buffer::find(&buf, "41").expect("the line number");
    let style = buf[(number.x, number.y)].style();
    assert_eq!(style.fg, Some(theme.accent));
    assert!(style.add_modifier.contains(Modifier::BOLD));
    let needle = tuikit::testkit::buffer::find(&buf, "needle").expect("the match");
    let matched = buf[(needle.x, needle.y)].style();
    assert!(matched.add_modifier.contains(theme.selection_active));
}

#[test]
fn an_empty_scrollback_draws_a_banner() {
    let mut viewer = open(pane(), Vec::new(), 1, "");
    let screen = render(&mut viewer, 60, 8);
    assert!(screen.contains("the pane has no scrollback"), "{screen}");
    assert!(screen.contains("0 lines"), "{screen}");
}
