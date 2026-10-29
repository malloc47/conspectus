use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::overlay::Overlay;

use super::*;
use crate::tui::theme::Theme;
use tuikit::testkit::keys;

fn item(label: &str, detail: &str, value: &str) -> PickerItem {
    PickerItem::new(label, value.to_string()).detail(detail)
}

fn state() -> Picker {
    retarget(
        "demo".into(),
        vec![
            item("alpha", "running", "mux_session:tmux:alpha"),
            item("beta", "gone · claude", "h_beta"),
            item("alphabet", "running", "mux_session:tmux:alphabet"),
        ],
    )
}

fn press(state: &mut Picker, code: KeyCode) -> Outcome<Msg> {
    state.handle(keys::press(code), &())
}

fn type_text(state: &mut Picker, text: &str) {
    for c in text.chars() {
        press(state, KeyCode::Char(c));
    }
}

fn labels(state: &Picker) -> Vec<&str> {
    state.listed().map(|item| item.label.as_str()).collect()
}

fn draw(state: &mut Picker, theme: &Theme, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    state.draw(area, &mut buf, theme);
    buf
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
    let buf = draw(&mut state, &theme, area);
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

#[test]
fn the_query_line_draws_its_caret_and_windows_by_cells() {
    use tuikit::testkit::{buffer, golden};
    let theme = Theme::default();
    let mut state = state();
    type_text(&mut state, "東京 alpha");
    press(&mut state, KeyCode::Home);
    press(&mut state, KeyCode::Right);
    let area = Rect::new(0, 0, 60, 12);
    let buf = draw(&mut state, &theme, area);
    let at = buffer::find(&buf, "/東").expect("the query line");
    let caret = &buf[(at.x + 3, at.y)];
    assert_eq!(caret.symbol(), "京", "the caret is on the second character");
    let cue = theme.field_cursor.into_style();
    assert_eq!((Some(caret.fg), Some(caret.bg)), (cue.fg, cue.bg));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/tui/widgets/goldens/picker-query.golden");
    let row = Rect::new(at.x, at.y, 16, 1);
    golden::assert_golden_text(&path, &golden::styled_region(&buf, row), |path, text| {
        std::fs::write(path, text)
    });
}

#[test]
fn the_retarget_picker_names_its_pin_and_says_when_there_is_nothing() {
    let theme = Theme::default();
    let mut empty = retarget("demo".into(), Vec::new());
    assert_eq!(empty.title(), "Retarget demo");
    let buf = draw(&mut empty, &theme, Rect::new(0, 0, 80, 20));
    let text = tuikit::testkit::buffer::rows(&buf).join("\n");
    assert!(text.contains("Retarget demo  (0/0)"), "{text}");
    assert!(text.contains(NOTHING_TO_RETARGET), "{text}");
}

#[test]
fn the_picker_draws_the_palette_footer() {
    let theme = Theme::default();
    let mut state = state();
    let buf = draw(&mut state, &theme, Rect::new(0, 0, 80, 20));
    let text = tuikit::testkit::buffer::rows(&buf).join("\n");
    assert!(
        text.contains(&tuikit::keys::footer(tuikit::keys::PALETTE_KEYS)),
        "{text}"
    );
}

#[test]
fn the_search_history_picker_starts_on_the_viewed_search_and_shows_it() {
    let items = vec![
        item("\"older\"", "yesterday", "0"),
        item("\"newer\"", "today", "1"),
    ];
    let mut picker = search_history(items.clone(), Some("1"));
    assert_eq!(picker.title(), "Searches");
    assert_eq!(
        picker.current().map(|i| i.label.as_str()),
        Some("\"newer\"")
    );
    assert_eq!(
        press(&mut picker, KeyCode::Enter),
        Outcome::Commit(Msg::ShowSearchHistory(1))
    );
    let unviewed = search_history(items, None);
    assert_eq!(unviewed.cursor(), 0);
}
