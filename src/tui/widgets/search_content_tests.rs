use super::*;
use crate::model::AgentSessionId;
use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyEventState, KeyModifiers};
use std::time::Duration;

fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn plain(code: KeyCode) -> KeyEvent {
    press(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    press(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_text(state: &mut SearchOverlayState, text: &str) {
    for c in text.chars() {
        state.handle_key(plain(KeyCode::Char(c)));
    }
}

fn contents_overlay(query: &str) -> SearchOverlayState {
    SearchOverlayState::configured(
        SearchScope::Contents,
        Grouping::Session,
        ContentKinds::default(),
        true,
        query,
    )
}

fn row(key: &str, turn: usize) -> ContentRow {
    ContentRow {
        session: AgentSessionId::new("claude-code", "/state", key),
        label: format!("session {key}"),
        harness: "claude".into(),
        hits: 3,
        turn: None,
        snippet: "we chose the quokka design for it".into(),
        highlight: Some(14..20),
        turn_index: turn,
    }
}

fn ctx<'a>(content: &'a [ContentRow]) -> SearchContext<'a> {
    SearchContext {
        items: &[],
        content,
    }
}

fn handle(
    state: &mut SearchOverlayState,
    content: &[ContentRow],
    key: KeyEvent,
) -> crate::tui::OverlayOutcome {
    use crate::tui::Overlay;
    state.handle(ctx(content), key)
}

#[test]
fn tab_switches_between_the_scopes_and_back() {
    let mut state = SearchOverlayState::new();
    assert_eq!(state.scope(), SearchScope::Rows);
    state.handle_key(plain(KeyCode::Tab));
    assert_eq!(state.scope(), SearchScope::Contents);
    state.handle_key(plain(KeyCode::Tab));
    assert_eq!(state.scope(), SearchScope::Rows);
}

#[test]
fn without_live_preview_the_contents_scope_cannot_be_chosen() {
    let mut state = SearchOverlayState::configured(
        SearchScope::Rows,
        Grouping::Session,
        ContentKinds::default(),
        false,
        "",
    );
    state.handle_key(plain(KeyCode::Tab));
    assert_eq!(state.scope(), SearchScope::Rows);
    assert!(
        state
            .notice()
            .is_some_and(|note| note.contains("live preview"))
    );
}

#[test]
fn a_remembered_contents_scope_is_ignored_when_contents_search_is_off() {
    let state = SearchOverlayState::configured(
        SearchScope::Contents,
        Grouping::Session,
        ContentKinds::default(),
        false,
        "",
    );
    assert_eq!(state.scope(), SearchScope::Rows);
}

#[test]
fn typing_waits_for_the_debounce_then_asks_for_one_scan_with_the_latest_text() {
    let mut state = contents_overlay("");
    // Asked about from before the first keystroke, so a stalled test
    // thread cannot make the first answer late.
    let before = Instant::now();
    type_text(&mut state, "quo");
    assert_eq!(state.take_due_search(before, false), None, "still resting");
    type_text(&mut state, "kka");
    // Measured from after the last keystroke, so a slow machine cannot
    // push the edit past the instant being asked about.
    let later = Instant::now() + DEBOUNCE + Duration::from_millis(5);
    let due = state.take_due_search(later, false);
    assert_eq!(
        due,
        Some(DueSearch::Preview {
            text: "quokka".into(),
            kinds: ContentKinds::default(),
            group: Grouping::Session,
        })
    );
    assert_eq!(state.take_due_search(later, false), None, "asked once");
}

#[test]
fn forcing_skips_the_debounce() {
    let mut state = contents_overlay("");
    type_text(&mut state, "x");
    assert!(matches!(
        state.take_due_search(Instant::now(), true),
        Some(DueSearch::Preview { .. })
    ));
}

#[test]
fn an_empty_query_stops_the_preview() {
    let mut state = contents_overlay("a");
    state.handle_key(plain(KeyCode::Backspace));
    assert_eq!(
        state.take_due_search(Instant::now(), true),
        Some(DueSearch::Clear)
    );
}

#[test]
fn leaving_the_contents_scope_stops_the_preview() {
    let mut state = contents_overlay("quokka");
    state.handle_key(plain(KeyCode::Tab));
    assert_eq!(state.scope(), SearchScope::Rows);
    assert_eq!(
        state.take_due_search(Instant::now(), true),
        Some(DueSearch::Clear)
    );
}

#[test]
fn grouping_and_kinds_change_inside_the_overlay_and_restart_the_scan() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('r'));
    assert_eq!(state.group(), Grouping::Message);
    state.handle_key(ctrl('t'));
    state.handle_key(ctrl('y'));
    assert!(state.kinds().tools && state.kinds().thinking);
    let due = state.take_due_search(Instant::now(), true);
    assert_eq!(
        due,
        Some(DueSearch::Preview {
            text: "quokka".into(),
            kinds: ContentKinds {
                tools: true,
                thinking: true,
                ..ContentKinds::default()
            },
            group: Grouping::Message,
        })
    );
    state.handle_key(ctrl('r'));
    assert_eq!(state.group(), Grouping::Flat, "the third mode follows");
    state.handle_key(ctrl('r'));
    assert_eq!(state.group(), Grouping::Session, "and the cycle wraps");
}

#[test]
fn enter_opens_the_result_under_the_cursor_at_its_turn() {
    use crate::tui::{Msg, OverlayOutcome};
    let mut state = contents_overlay("quokka");
    let rows = [row("a", 3), row("b", 9)];
    handle(&mut state, &rows, plain(KeyCode::Down));
    let outcome = handle(&mut state, &rows, plain(KeyCode::Enter));
    let OverlayOutcome::Commit(msg) = outcome else {
        panic!("{outcome:?}");
    };
    let Msg::OpenContentHit {
        session,
        text,
        turn_index,
    } = *msg
    else {
        panic!("not an open");
    };
    assert_eq!(session.session_key, "b");
    assert_eq!(text, "quokka");
    assert_eq!(turn_index, 9);
}

#[test]
fn enter_with_no_results_closes() {
    let mut state = contents_overlay("quokka");
    assert!(matches!(
        handle(&mut state, &[], plain(KeyCode::Enter)),
        crate::tui::OverlayOutcome::Close
    ));
}

#[test]
fn the_cursor_wraps_over_the_contents_results() {
    let mut state = contents_overlay("q");
    let rows = [row("a", 0), row("b", 0), row("c", 0)];
    handle(&mut state, &rows, plain(KeyCode::Up));
    assert_eq!(state.cursor(), 2);
    handle(&mut state, &rows, plain(KeyCode::Down));
    assert_eq!(state.cursor(), 0);
}

#[test]
fn the_promote_keys_keep_the_query_as_the_filter_or_open_the_view() {
    use crate::tui::{Msg, OverlayOutcome};
    for (key, show) in [(ctrl('f'), false), (ctrl('o'), true)] {
        let mut state = contents_overlay("quokka");
        let outcome = handle(&mut state, &[], key);
        let OverlayOutcome::Commit(msg) = outcome else {
            panic!("{outcome:?}");
        };
        let Msg::ApplyContentSearch {
            text, show_results, ..
        } = *msg
        else {
            panic!("not an apply");
        };
        assert_eq!(text, "quokka");
        assert_eq!(show_results, show);
    }
}

#[test]
fn promoting_an_empty_query_does_nothing() {
    let mut state = contents_overlay("  ");
    assert!(matches!(
        handle(&mut state, &[], ctrl('f')),
        crate::tui::OverlayOutcome::Consumed
    ));
}

#[test]
fn escape_closes_in_either_scope() {
    for scope in [SearchScope::Rows, SearchScope::Contents] {
        let mut state = SearchOverlayState::configured(
            scope,
            Grouping::Session,
            ContentKinds::default(),
            true,
            "",
        );
        assert!(matches!(
            handle(&mut state, &[], plain(KeyCode::Esc)),
            crate::tui::OverlayOutcome::Close
        ));
    }
}

#[test]
fn the_rows_scope_does_not_bind_the_contents_keys() {
    // Ctrl-F and Ctrl-R are text-editing keys in the rows scope; the
    // contents keys apply only after Tab.
    let mut state = SearchOverlayState::new();
    state.handle_key(ctrl('r'));
    state.handle_key(ctrl('t'));
    assert_eq!(state.group(), Grouping::Session);
    assert!(!state.kinds().tools);
    assert_eq!(state.scope(), SearchScope::Rows);
    assert!(matches!(
        handle(&mut state, &[], ctrl('f')),
        crate::tui::OverlayOutcome::Consumed
    ));
}

fn render(state: &SearchOverlayState, view: &ContentView, width: u16, height: u16) -> String {
    let theme = Theme::default();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    SearchOverlayWidget::new(state, &[], &theme)
        .with_content(view)
        .render(area, &mut buf);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn view(rows: Vec<ContentRow>, done: bool) -> ContentView {
    ContentView {
        rows,
        scanned: 34,
        total: if done { 34 } else { 77 },
        skipped: 2,
        done,
        unavailable: None,
    }
}

#[test]
fn the_contents_scope_shows_progress_results_and_the_generated_key_hints() {
    let state = contents_overlay("quokka");
    let text = render(
        &state,
        &view(vec![row("aaaa1111", 3), row("bbbb2222", 5)], false),
        100,
        24,
    );
    assert!(text.contains("search contents (2 results)"), "{text}");
    assert!(text.contains("/quokka"), "{text}");
    assert!(
        text.contains("34/77 sessions") && text.contains("2 skipped"),
        "{text}"
    );
    assert!(
        text.contains("3 hits") && text.contains("session aaaa1111"),
        "{text}"
    );
    assert!(text.contains("Tab search rows"), "{text}");
    assert!(text.contains("Ctrl-F filter lists"), "{text}");
    assert!(text.contains("Ctrl-O open Search view"), "{text}");
    insta::assert_snapshot!("search_overlay_contents_by_session", text);
}

#[test]
fn the_per_message_grouping_lists_one_row_per_turn() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('r'));
    let mut first = row("aaaa1111", 3);
    first.turn = Some("turn 4 · message".into());
    let mut second = row("aaaa1111", 7);
    second.turn = Some("turn 8 · tool output".into());
    let text = render(&state, &view(vec![first, second], true), 100, 24);
    assert!(text.contains("by message"), "{text}");
    assert!(
        text.contains("turn 4 · message") && text.contains("turn 8 · tool output"),
        "{text}"
    );
    assert!(text.contains("34 sessions searched"), "{text}");
    insta::assert_snapshot!("search_overlay_contents_by_message", text);
}

#[test]
fn the_flat_grouping_names_the_session_of_every_message() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('r'));
    state.handle_key(ctrl('r'));
    let mut first = row("aaaa1111", 3);
    first.turn = Some("turn 4 · message".into());
    let mut second = row("bbbb2222", 1);
    second.turn = Some("turn 2 · message".into());
    let text = render(&state, &view(vec![first, second], true), 100, 24);
    assert!(text.contains("by message, flat"), "{text}");
    assert!(
        text.contains("session aaaa1111") && text.contains("session bbbb2222"),
        "{text}"
    );
    insta::assert_snapshot!("search_overlay_contents_flat", text);
}

#[test]
fn the_empty_states_say_what_to_do() {
    let state = contents_overlay("");
    let text = render(&state, &ContentView::default(), 80, 20);
    assert!(text.contains("type to search what was said"), "{text}");
    let state = contents_overlay("zzz");
    let text = render(&state, &view(Vec::new(), true), 80, 20);
    assert!(text.contains("no sessions match"), "{text}");
    let text = render(&state, &view(Vec::new(), false), 80, 20);
    assert!(text.contains("searching"), "{text}");
    let unavailable = ContentView {
        unavailable: Some("Content search is off while live preview is disabled"),
        ..ContentView::default()
    };
    let text = render(&state, &unavailable, 80, 20);
    assert!(text.contains("live preview is disabled"), "{text}");
}

#[test]
fn nothing_is_cut_off_at_narrow_widths() {
    let state = contents_overlay("quokka");
    for width in [40, 60, 80] {
        let text = render(&state, &view(vec![row("aaaa1111", 3)], false), width, 20);
        for line in text.lines() {
            assert!(line.chars().count() <= width as usize, "{width}: {line}");
        }
    }
}
