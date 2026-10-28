use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::*;

fn item(label: &str, detail: &str, value: &str) -> PickerItem {
    PickerItem {
        label: label.into(),
        detail: detail.into(),
        value: value.into(),
    }
}

fn state() -> PickerState {
    PickerState::new(
        "Retarget demo",
        PickerPurpose::Retarget { pin: "demo".into() },
        vec![
            item("alpha", "running", "mux_session:tmux:alpha"),
            item("beta", "gone · claude", "h_beta"),
            item("alphabet", "running", "mux_session:tmux:alphabet"),
        ],
    )
}

fn press(state: &mut PickerState, code: KeyCode) -> Outcome<Msg> {
    state.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(state: &mut PickerState, text: &str) {
    for c in text.chars() {
        press(state, KeyCode::Char(c));
    }
}

fn labels(state: &PickerState) -> Vec<&str> {
    state
        .visible_items()
        .into_iter()
        .map(|item| item.label.as_str())
        .collect()
}

#[test]
fn typing_narrows_the_list_to_items_matching_every_word() {
    let mut state = state();
    assert_eq!(labels(&state), ["alpha", "beta", "alphabet"]);
    type_text(&mut state, "alph");
    assert_eq!(labels(&state), ["alpha", "alphabet"]);
    type_text(&mut state, "abe");
    assert_eq!(labels(&state), ["alphabet"]);
}

#[test]
fn the_detail_is_searchable_too() {
    let mut state = state();
    type_text(&mut state, "gone");
    assert_eq!(labels(&state), ["beta"]);
}

#[test]
fn the_cursor_moves_with_the_arrows_and_wraps() {
    let mut state = state();
    press(&mut state, KeyCode::Down);
    assert_eq!(state.current().map(|i| i.label.as_str()), Some("beta"));
    press(&mut state, KeyCode::Up);
    press(&mut state, KeyCode::Up);
    assert_eq!(state.current().map(|i| i.label.as_str()), Some("alphabet"));
}

#[test]
fn enter_commits_the_item_under_the_cursor() {
    let mut state = state();
    press(&mut state, KeyCode::Down);
    assert_eq!(
        press(&mut state, KeyCode::Enter),
        Outcome::Commit(Msg::CommitRetarget {
            pin: "demo".into(),
            target: "h_beta".into(),
        })
    );
}

#[test]
fn enter_on_an_empty_filter_just_closes() {
    let mut state = state();
    type_text(&mut state, "zzz");
    assert!(labels(&state).is_empty());
    assert_eq!(press(&mut state, KeyCode::Enter), Outcome::Close);
}

#[test]
fn escape_closes_without_committing() {
    let mut state = state();
    assert_eq!(press(&mut state, KeyCode::Esc), Outcome::Close);
}

#[test]
fn narrowing_keeps_the_cursor_on_the_list() {
    let mut state = state();
    press(&mut state, KeyCode::Down);
    press(&mut state, KeyCode::Down);
    type_text(&mut state, "beta");
    assert_eq!(state.cursor(), 0);
    assert_eq!(state.current().map(|i| i.label.as_str()), Some("beta"));
}

#[test]
fn the_cursor_row_shows_the_marker_and_the_highlight() {
    let theme = Theme::default();
    let mut state = state();
    press(&mut state, KeyCode::Down);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    PickerWidget::new(&state, &theme).render(area, &mut buf);
    let marked: Vec<(u16, u16)> = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|(x, y)| buf[(*x, *y)].symbol() == tuikit::glyphs::marks::POINTER)
        .collect();
    assert_eq!(marked.len(), 1, "one marked row");
    let (x, y) = marked[0];
    let label: String = (x + 2..x + 6).map(|x| buf[(x, y)].symbol()).collect();
    assert_eq!(label, "beta");
    for x in x..x + 12 {
        assert!(
            buf[(x, y)].modifier.contains(theme.selection_active),
            "cell {x}"
        );
    }
    let above: String = (x..x + 7).map(|x| buf[(x, y - 1)].symbol()).collect();
    assert_eq!(above, "  alpha", "other rows keep the label a gutter in");
}
