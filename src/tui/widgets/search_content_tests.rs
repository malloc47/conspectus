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
        SearchMode::Content,
        Grouping::Session,
        ContentKinds::default(),
        true,
        query,
    )
}

fn row(key: &str, turn: usize) -> ContentRow {
    ContentRow {
        target: ContentTarget::Session {
            session: AgentSessionId::new("claude-code", "/state", key),
            turn_index: turn,
        },
        label: format!("session {key}"),
        chip: "claude".into(),
        hits: 3,
        turn: None,
        age: "1h".into(),
        snippet: "we chose the quokka design for it".into(),
        highlight: Some(14..20),
    }
}

fn pane_row(line: usize) -> ContentRow {
    ContentRow {
        target: ContentTarget::Pane {
            pane: crate::content_search::PaneRef {
                mux: crate::content_search::MuxRef {
                    backend: "tmux".into(),
                    socket: None,
                    session: "work".into(),
                },
                pane_id: "%3".into(),
                window_index: "0".into(),
                pane_index: "1".into(),
                command: Some("zsh".into()),
            },
            line,
        },
        label: "work 0.1".into(),
        chip: "zsh".into(),
        hits: 2,
        turn: None,
        age: "2m".into(),
        snippet: "error: the quokka crashed".into(),
        highlight: Some(11..17),
    }
}

fn ctx<'a>(content: &'a [ContentRow]) -> SearchContext<'a> {
    SearchContext {
        items: &[],
        content,
        entities: None,
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
fn tab_switches_between_current_and_all_and_asks_for_a_new_scan() {
    let mut state = contents_overlay("quokka");
    assert_eq!(
        state.reach(),
        Reach::Current,
        "it opens on the current view"
    );
    state.handle_key(plain(KeyCode::Tab));
    assert_eq!(state.reach(), Reach::All);
    assert_eq!(state.mode(), SearchMode::Content);
    assert_eq!(state.query(), "quokka", "Tab is not typed into the query");
    let due = state.take_due_search(Instant::now(), true);
    assert!(
        matches!(
            due,
            Some(DueSearch::Preview {
                reach: Reach::All,
                ..
            })
        ),
        "{due:?}"
    );
    state.handle_key(press(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(state.reach(), Reach::Current, "Shift-Tab goes back");
}

#[test]
fn an_overlay_can_open_on_the_wider_reach_without_searching() {
    let mut state = contents_overlay("quokka").with_reach(Reach::All);
    assert_eq!(state.reach(), Reach::All);
    assert_eq!(
        state.take_due_search(Instant::now(), true),
        None,
        "opening is not an edit"
    );
}

#[test]
fn a_remembered_contents_scope_is_ignored_when_contents_search_is_off() {
    let state = SearchOverlayState::configured(
        SearchMode::Content,
        Grouping::Session,
        ContentKinds::default(),
        false,
        "",
    );
    assert_eq!(state.mode(), SearchMode::Quick);
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
            reach: Reach::Current,
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
fn the_options_pane_changes_what_is_searched_and_restarts_the_scan() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('o'));
    assert_eq!(state.options_cursor(), Some(0));
    state.handle_key(plain(KeyCode::Char(' ')));
    state.handle_key(plain(KeyCode::Down));
    state.handle_key(plain(KeyCode::Enter));
    state.handle_key(plain(KeyCode::Down));
    state.handle_key(plain(KeyCode::Char(' ')));
    assert!(state.kinds().tools && state.kinds().thinking);
    assert!(!state.panes());
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
            group: Grouping::Session,
            reach: Reach::Current,
        })
    );
    // Esc goes back to the results, not out of the modal.
    assert!(matches!(
        state.handle_key(plain(KeyCode::Esc)),
        SearchOutcome::Continue
    ));
    assert_eq!(state.options_cursor(), None);
    assert!(matches!(
        state.handle_key(plain(KeyCode::Esc)),
        SearchOutcome::Cancel
    ));
}

#[test]
fn the_granularity_key_lists_the_same_matches_another_way_without_a_new_scan() {
    let mut state = contents_overlay("quokka");
    assert!(matches!(
        state.handle_key(ctrl('l')),
        SearchOutcome::Regroup(Grouping::Flat)
    ));
    assert_eq!(state.group(), Grouping::Flat);
    assert_eq!(
        state.take_due_search(Instant::now(), true),
        None,
        "no scan is owed"
    );
    assert!(matches!(
        state.handle_key(ctrl('l')),
        SearchOutcome::Regroup(Grouping::Session)
    ));
}

#[test]
fn the_retired_result_and_kind_keys_edit_the_query_like_any_other() {
    let mut state = contents_overlay("quokka");
    for c in ['r', 't', 'y'] {
        state.handle_key(ctrl(c));
    }
    assert_eq!(state.group(), Grouping::Session);
    assert!(!state.kinds().tools && !state.kinds().thinking);
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
fn ctrl_f_keeps_the_query_as_the_filter_and_ctrl_s_keeps_it_in_the_search_view() {
    use crate::tui::{Msg, OverlayOutcome};
    let mut state = contents_overlay("quokka");
    let OverlayOutcome::Commit(msg) = handle(&mut state, &[], ctrl('f')) else {
        panic!("Ctrl-F does not commit");
    };
    let Msg::ApplyContentSearch {
        text,
        show_results,
        reach,
        ..
    } = *msg
    else {
        panic!("not an apply");
    };
    assert_eq!(
        (text.as_str(), show_results, reach),
        ("quokka", false, Reach::Current)
    );

    let mut state = contents_overlay("quokka").with_reach(Reach::All);
    let OverlayOutcome::Commit(msg) = handle(&mut state, &[], ctrl('s')) else {
        panic!("Ctrl-S does not commit");
    };
    let Msg::KeepContentSearch { text, reach, .. } = *msg else {
        panic!("not a keep");
    };
    assert_eq!((text.as_str(), reach), ("quokka", Reach::All));
}

#[test]
fn ctrl_o_no_longer_opens_the_search_view() {
    let mut state = contents_overlay("quokka");
    assert!(matches!(
        handle(&mut state, &[], ctrl('o')),
        crate::tui::OverlayOutcome::Consumed
    ));
}

#[test]
fn enter_on_a_pane_row_opens_its_scrollback_after_revealing_the_mux() {
    use crate::tui::{Msg, OverlayOutcome};
    let mut state = contents_overlay("quokka");
    let rows = [row("a", 3), pane_row(120)];
    handle(&mut state, &rows, plain(KeyCode::Down));
    let OverlayOutcome::Commit(msg) = handle(&mut state, &rows, plain(KeyCode::Enter)) else {
        panic!("Enter does not commit");
    };
    let Msg::OpenPaneHit {
        pane,
        line,
        text,
        reveal,
    } = *msg
    else {
        panic!("not a pane hit");
    };
    assert_eq!(pane.pane_id, "%3");
    assert_eq!((line, text.as_str(), reveal), (120, "quokka", true));
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
    for scope in [SearchMode::Quick, SearchMode::Content] {
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
    // Ctrl-F is a text-editing key in quick select; the contents keys
    // apply only in content mode.
    let mut state = SearchOverlayState::new();
    state.handle_key(ctrl('l'));
    state.handle_key(ctrl('o'));
    assert_eq!(state.group(), Grouping::Session);
    assert!(!state.kinds().tools);
    assert_eq!(state.mode(), SearchMode::Quick);
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
        with_panes: false,
        empty_scope: false,
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
    assert!(text.contains("search contents"), "{text}");
    assert!(text.contains("[ Current ] All"), "{text}");
    assert!(text.contains("[ Sessions ] Messages"), "{text}");
    assert!(text.contains("2 results"), "{text}");
    assert!(text.contains("/quokka"), "{text}");
    assert!(
        text.contains("34/77 sessions") && text.contains("2 skipped"),
        "{text}"
    );
    assert!(
        text.contains("3 hits") && text.contains("session aaaa1111"),
        "{text}"
    );
    assert!(text.contains("Tab current/all"), "{text}");
    assert!(text.contains("Ctrl-F filter lists"), "{text}");
    assert!(text.contains("Ctrl-S Search view"), "{text}");
    assert!(text.contains("Ctrl-L sessions/messages"), "{text}");
    assert!(text.contains("Ctrl-O options"), "{text}");
    for retired in ["Ctrl-R", "Ctrl-T", "Ctrl-Y", "thinking ·"] {
        assert!(!text.contains(retired), "{retired}: {text}");
    }
    insta::assert_snapshot!("search_overlay_contents_by_session", text);
}

#[test]
fn the_messages_granularity_names_the_session_of_every_message() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('l'));
    let mut first = row("aaaa1111", 3);
    first.turn = Some("turn 4 · message".into());
    let mut second = row("bbbb2222", 1);
    second.turn = Some("turn 2 · message".into());
    let text = render(&state, &view(vec![first, second], true), 100, 24);
    assert!(text.contains("Sessions [ Messages ]"), "{text}");
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

#[test]
fn a_pane_row_carries_the_source_word_the_command_and_the_line() {
    let state = contents_overlay("quokka");
    let mut pane = pane_row(120);
    pane.turn = Some("line 120".into());
    let text = render(&state, &view(vec![row("aaaa1111", 3), pane], true), 160, 24);
    let session_line = text
        .lines()
        .find(|l| l.contains("session aaaa1111"))
        .unwrap();
    assert!(session_line.contains("session  claude"), "{session_line}");
    let pane_line = text.lines().find(|l| l.contains("work 0.1")).unwrap();
    assert!(
        pane_line.contains("pane    zsh") && pane_line.contains("line 120"),
        "{pane_line}"
    );
    assert!(
        pane_line.contains("error: the quokka crashed"),
        "{pane_line}"
    );
}

#[test]
fn the_columns_give_way_to_the_snippet_in_the_documented_order() {
    // Wide: every cell. Narrower: the chip goes first, then the source
    // word, then the age; the label shrinks last. The snippet keeps at
    // least 20 cells whenever any cell can give way.
    let wide = content_columns(200);
    assert!(wide.source > 0 && wide.chip > 0 && wide.age > 0);
    assert_eq!(wide.label, 32, "the label stops growing");
    let mid = content_columns(83);
    assert_eq!((mid.chip, mid.source > 0, mid.age > 0), (0, true, true));
    let narrow = content_columns(60);
    assert_eq!((narrow.chip, narrow.source), (0, 0));
    let tiny = content_columns(40);
    assert_eq!((tiny.chip, tiny.source, tiny.age), (0, 0, 0));
    for inner in [40, 60, 83, 120, 200] {
        let columns = content_columns(inner);
        assert!(
            columns.snippet() >= MIN_SNIPPET || columns.label <= 8,
            "{inner}: {columns:?}"
        );
    }
}

#[test]
fn the_tab_identifier_shows_the_chosen_reach() {
    let mut state = contents_overlay("quokka");
    let text = render(&state, &view(vec![row("aaaa1111", 3)], true), 100, 20);
    assert!(text.contains("[ Current ] All"), "{text}");
    state.handle_key(plain(KeyCode::Tab));
    let text = render(&state, &view(vec![row("aaaa1111", 3)], true), 100, 20);
    assert!(text.contains("Current [ All ]"), "{text}");
}

#[test]
fn an_empty_current_context_says_so_and_points_at_tab() {
    let state = contents_overlay("quokka");
    let empty = ContentView {
        empty_scope: true,
        done: true,
        ..ContentView::default()
    };
    let text = render(&state, &empty, 100, 20);
    assert!(text.contains("lists no sessions or muxes"), "{text}");
    assert!(text.contains("Tab"), "{text}");
}

#[test]
fn a_search_that_read_panes_counts_them_and_says_no_panes_match() {
    let state = contents_overlay("quokka");
    let mut searched = view(vec![row("aaaa1111", 3)], true);
    searched.with_panes = true;
    let text = render(&state, &searched, 120, 20);
    assert!(text.contains("34 sessions and panes searched"), "{text}");
    let mut none = view(Vec::new(), true);
    none.with_panes = true;
    let text = render(&state, &none, 120, 20);
    assert!(text.contains("no sessions or panes match"), "{text}");
}

#[test]
fn the_modal_scales_to_a_wide_terminal_without_clipping() {
    let state = contents_overlay("quokka");
    let rows = vec![row("aaaa1111", 3), pane_row(40)];
    for width in [60u16, 100, 160, 220, 260] {
        let text = render(&state, &view(rows.clone(), true), width, 30);
        let widest = text
            .lines()
            .map(|l| l.trim().chars().count())
            .max()
            .unwrap_or(0);
        let expected = usize::from(modal_size(width, 30).0);
        assert!(
            (expected..=expected + 1).contains(&widest),
            "{width}: the modal is {widest} cells wide, the rule gives {expected}"
        );
        for line in text.lines() {
            assert!(
                line.chars().count() <= usize::from(width),
                "{width}: {line}"
            );
        }
    }
}

fn header(width: u16) -> String {
    let state = contents_overlay("quokka");
    let text = render(&state, &view(vec![row("aaaa1111", 3)], true), width, 24);
    assert!(
        text.contains("[ Current ] All  [ Sessions ] Messages"),
        "{width}: {text}"
    );
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_header_shows_both_choices_at_every_width() {
    insta::assert_snapshot!("search_overlay_contents_header_80", header(80));
    insta::assert_snapshot!("search_overlay_contents_header_160", header(160));
    insta::assert_snapshot!("search_overlay_contents_header_220", header(220));
}

#[test]
fn the_options_pane_lists_what_a_search_looks_in() {
    let mut state = contents_overlay("quokka");
    state.handle_key(ctrl('o'));
    state.handle_key(plain(KeyCode::Char(' ')));
    let text = render(&state, &view(vec![row("aaaa1111", 3)], true), 100, 14);
    assert!(text.contains("[x] Tool calls and output"), "{text}");
    assert!(text.contains("[ ] Thinking"), "{text}");
    assert!(text.contains("[x] Pane scrollback"), "{text}");
    assert!(text.contains("Esc back"), "{text}");
    assert!(!text.contains("session aaaa1111"), "the results give way");
    insta::assert_snapshot!("search_overlay_options_pane", text);
}
