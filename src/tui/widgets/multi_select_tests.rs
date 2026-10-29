use super::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::testkit::keys::{chord, press};

#[test]
fn cursor_starts_at_zero_and_wraps() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    assert_eq!(state.cursor(), 0);
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.cursor(), 2);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.cursor(), 0, "down wraps");
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.cursor(), 2, "up wraps");
}

#[test]
fn vim_keys_move_cursor() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    state.handle_key(press(KeyCode::Char('j')));
    assert_eq!(state.cursor(), 1);
    state.handle_key(press(KeyCode::Char('k')));
    assert_eq!(state.cursor(), 0);
}

#[test]
fn space_toggles_item_under_cursor() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.selected_indices(), vec![0]);
    state.handle_key(press(KeyCode::Char(' ')));
    assert!(state.selected_indices().is_empty());
}

#[test]
fn enter_confirms_with_current_selection() {
    let mut state = MultiSelectState::new("harness", 3, &[1]);
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![1]));
}

#[test]
fn esc_cancels_without_confirming() {
    let mut state = MultiSelectState::new("harness", 3, &[1]);
    let outcome = state.handle_key(press(KeyCode::Esc));
    assert_eq!(outcome, MultiSelectOutcome::Cancel);
    assert_eq!(state.selected_indices(), vec![1]);
}

#[test]
fn ctrl_c_cancels() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    let outcome = state.handle_key(chord(KeyModifiers::CONTROL, KeyCode::Char('c')));
    assert_eq!(outcome, MultiSelectOutcome::Cancel);
}

#[test]
fn initial_selection_round_trips() {
    let state = MultiSelectState::new("harness", 4, &[0, 2]);
    assert!(state.is_selected(0));
    assert!(!state.is_selected(1));
    assert!(state.is_selected(2));
    assert!(!state.is_selected(3));
    assert_eq!(state.selected_indices(), vec![0, 2]);
}

#[test]
fn out_of_range_initial_indices_are_ignored() {
    let state = MultiSelectState::new("harness", 2, &[1, 5, 99]);
    assert_eq!(state.selected_indices(), vec![1]);
}

#[test]
fn empty_list_handles_keys_without_panicking() {
    let mut state = MultiSelectState::new("harness", 0, &[]);
    state.handle_key(press(KeyCode::Down));
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Char(' ')));
    assert_eq!(state.cursor(), 0);
    let outcome = state.handle_key(press(KeyCode::Enter));
    assert_eq!(outcome, MultiSelectOutcome::Confirm(vec![]));
}

#[test]
fn unrecognized_keys_are_ignored() {
    let mut state = MultiSelectState::new("harness", 3, &[]);
    let outcome = state.handle_key(press(KeyCode::Tab));
    assert_eq!(outcome, MultiSelectOutcome::Continue);
    assert_eq!(state.cursor(), 0);
}

#[test]
fn selected_last_row_scrolls_into_short_multi_select_body() {
    let items: Vec<String> = (0..10).map(|n| format!("item {n}")).collect();
    let mut state = MultiSelectState::new("pick", items.len(), &[]);
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.cursor(), 9);
    // A terminal 10 rows tall leaves the modal 6 item rows.
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    MultiSelectWidget::new(&state, &items, &Theme::default()).render(area, &mut buf);
    let text: String = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    assert!(text.contains("item 9"), "the cursor's row is drawn: {text}");
    assert!(
        text.contains("item 4"),
        "the rows above it fill the body: {text}"
    );
    assert!(!text.contains("item 3"), "{text}");
}

#[test]
fn the_cursor_row_shows_the_marker_and_the_highlight_and_checks_keep_their_mark() {
    let items = ["alpha", "beta", "gamma"];
    let mut state = MultiSelectState::new("pick", items.len(), &[2]);
    state.handle_key(press(KeyCode::Down));
    let theme = Theme::default();
    let area = Rect::new(0, 0, 40, 10);
    let mut buf = Buffer::empty(area);
    MultiSelectWidget::new(&state, &items, &theme).render(area, &mut buf);
    let row_of = |label: &str| {
        (0..area.height)
            .find_map(|y| {
                let text: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
                text.find(label).map(|byte| {
                    let x = text[..byte].chars().count();
                    (u16::try_from(x).unwrap_or(0), y, text)
                })
            })
            .unwrap_or_else(|| panic!("{label} is drawn"))
    };
    let (x, y, text) = row_of("beta");
    let lead: String = (x - 4..x).map(|x| buf[(x, y)].symbol()).collect();
    assert_eq!(
        lead,
        format!("{} {} ", marks::POINTER, marks::BULLET),
        "{text}"
    );
    for x in x - 4..x + 4 {
        assert!(
            buf[(x, y)].modifier.contains(theme.selection_active),
            "cell {x}: {text}"
        );
    }
    let (x, y, _) = row_of("gamma");
    let lead: String = (x - 4..x).map(|x| buf[(x, y)].symbol()).collect();
    assert_eq!(
        lead,
        format!("  {} ", marks::OK),
        "a checked row off the cursor"
    );
    assert_eq!(buf[(x, y)].fg, theme.accent);
    assert!(!buf[(x, y)].modifier.contains(Modifier::REVERSED));
}
