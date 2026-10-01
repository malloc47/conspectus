// Extracted from search.rs H-HYG-011 rolling wave via #[path = "search_tests.rs"] mod tests;
use super::*;
use crate::model::{AgentSessionId, NodeId};
use crate::tui::rows::{AgentSessionRow, MuxIndicator, Row, RowKind};
use crate::tui::search::items_from_rows;
use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn agent_row(key: &str, alias: Option<&str>) -> Row {
    let session_id = AgentSessionId::new("claude-code", "/state", key);
    let primary = NodeId::AgentSession(session_id.clone());
    Row {
        id: RowId::AgentSession(primary.clone()),
        depth: 1,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: session_id,
            short_id: key.to_string(),
            harness_label: "claude-code".to_string(),
            cwd_display: Some("~/proj".to_string()),
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: alias.map(str::to_string),
            title_disambiguates: false,
            primary_node: primary,
            pin_id: None,
        }),
    }
}

#[test]
fn typing_query_filters_matches() {
    let rows = vec![
        agent_row("a", Some("puffin")),
        agent_row("b", Some("other")),
    ];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    // Type "puf".
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    assert_eq!(state.matches().len(), 1);
    assert_eq!(state.matches()[0].id, rows[0].id);
}

#[test]
fn enter_confirms_with_cursor_match() {
    let rows = vec![agent_row("a", Some("puffin")), agent_row("b", Some("puff"))];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    // Cursor at 0; Enter confirms the first match.
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(
        outcome,
        SearchOutcome::Confirm(Box::new(rows[0].id.clone()))
    );
}

#[test]
fn arrow_down_moves_cursor_within_matches() {
    let rows = vec![agent_row("a", Some("puffin")), agent_row("b", Some("puff"))];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 1);
    // Wraps.
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 0);
}

#[test]
fn enter_on_empty_match_list_cancels() {
    let rows = vec![agent_row("a", None)];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    for c in "no-such-match".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    assert!(state.matches().is_empty());
    let outcome = state.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, SearchOutcome::Cancel);
}

#[test]
fn esc_cancels() {
    let mut state = SearchOverlayState::new();
    let outcome = state.handle_key(key(KeyCode::Esc));
    assert_eq!(outcome, SearchOutcome::Cancel);
}

#[test]
fn ctrl_n_p_navigate_results() {
    let rows = vec![agent_row("a", Some("puff")), agent_row("b", Some("puffin"))];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    let ctrl_n = KeyEvent {
        code: KeyCode::Char('n'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    state.handle_key(ctrl_n);
    assert_eq!(state.cursor(), 1);
    let ctrl_p = KeyEvent {
        code: KeyCode::Char('p'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    state.handle_key(ctrl_p);
    assert_eq!(state.cursor(), 0);
}

#[test]
fn match_line_includes_snippet_with_matched_bytes_highlighted() {
    // Agent row whose alias is short but whose preview contains
    // the actual match — the rendered line should expose the
    // matching preview snippet alongside the alias.
    let row = agent_row("nice", Some("nice"));
    let mut row_with_preview = row;
    if let crate::tui::rows::RowKind::AgentSession(s) = &mut row_with_preview.kind {
        s.preview = Some("lots of stuff and then puffin shows up here".to_string());
    }
    let items = items_from_rows(std::slice::from_ref(&row_with_preview));
    let mut state = SearchOverlayState::new();
    for c in "puffin".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    let m = &state.matches()[0];
    let theme = Theme::default();
    let line = build_match_line(
        items[0].label.to_string(),
        Some(&items[0]),
        m,
        true,
        40,
        &theme,
    );
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(rendered.contains("nice"), "label rendered: {rendered}");
    assert!(rendered.contains("puffin"), "snippet rendered: {rendered}");
    // The matched portion should land in its own span so the
    // renderer can style it.
    let has_match_span = line.spans.iter().any(|s| s.content.as_ref() == "puffin");
    assert!(has_match_span, "match span missing in: {:?}", line.spans);
}

#[test]
fn match_line_skips_snippet_when_label_equals_haystack() {
    // Group rows have label == haystack — a snippet would just
    // duplicate the label, so we omit it.
    use crate::tui::rows::{GroupRow, Row, RowKind};
    let row = Row {
        id: RowId::Synthetic("g"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "puffin/dir".to_string(),
            primary_node: None,
            is_launch_context: false,
        }),
    };
    let items = items_from_rows(std::slice::from_ref(&row));
    let mut state = SearchOverlayState::new();
    for c in "puffin".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    let m = &state.matches()[0];
    let theme = Theme::default();
    let line = build_match_line(
        items[0].label.to_string(),
        Some(&items[0]),
        m,
        false,
        40,
        &theme,
    );
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // No `· ` separator means no duplicate snippet.
    assert!(!rendered.contains("  · "), "rendered: {rendered}");
    // Label still appears.
    assert!(rendered.contains("puffin/dir"));
}

#[test]
fn search_glyph_span_uses_kind_color_for_graph_rows() {
    // H-UI-002 slice: each result row carries a kind glyph in
    // its NodeKind color (ADR 0073). An AgentSession row picks
    // up the AgentSession glyph + `theme.node_agent_session`
    // color; a MuxSession row picks up the mux glyph + color.
    use crate::model::MuxSessionId;
    let theme = Theme::default();

    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let session_span = search_kind_glyph_span(&RowId::AgentSession(session_id), &theme, false);
    let session_glyph = NodeKind::AgentSession.default_glyph();
    assert!(
        session_span.content.starts_with(session_glyph),
        "session glyph span content: {:?}",
        session_span.content,
    );
    assert_eq!(session_span.style.fg, Some(theme.node_agent_session));

    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    let mux_span = search_kind_glyph_span(&RowId::MuxSession(mux_id), &theme, false);
    let mux_glyph = NodeKind::MuxSession.default_glyph();
    assert!(mux_span.content.starts_with(mux_glyph));
    assert_eq!(mux_span.style.fg, Some(theme.node_mux_session));
}

#[test]
fn search_glyph_span_falls_back_to_two_spaces_for_kindless_rows() {
    // Pin ids are graph-backed and carry the pin glyph.
    let theme = Theme::default();
    let pin_span = search_kind_glyph_span(
        &RowId::Pin {
            pin_id: "ingest".to_string(),
        },
        &theme,
        false,
    );
    assert_eq!(pin_span.content, "◉ ");
    assert_eq!(pin_span.style.fg, Some(theme.node_mux_session));

    // Synthetic ids aren't graph nodes — they get two blank
    // cells so the label column lines up with glyph-bearing rows.
    let synthetic_span = search_kind_glyph_span(&RowId::Synthetic("ungrouped"), &theme, false);
    assert_eq!(synthetic_span.content, "  ");
    assert_eq!(synthetic_span.style.fg, None);
}

#[test]
fn match_line_includes_kind_glyph_before_label() {
    // End-to-end on `build_match_line`: the rendered line
    // should carry the kind glyph between the cursor prefix
    // and the label so operators scan by symbol.
    let row = agent_row("abcdef", Some("puffin"));
    let items = items_from_rows(std::slice::from_ref(&row));
    let mut state = SearchOverlayState::new();
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    let m = &state.matches()[0];
    let theme = Theme::default();
    let line = build_match_line(
        items[0].label.to_string(),
        Some(&items[0]),
        m,
        false,
        40,
        &theme,
    );
    let agent_glyph = NodeKind::AgentSession.default_glyph();
    let glyph_span = line
        .spans
        .iter()
        .find(|s| s.content == format!("{agent_glyph} "))
        .expect("kind glyph span present in match line");
    assert_eq!(glyph_span.style.fg, Some(theme.node_agent_session));
}

#[test]
fn refresh_matches_resets_cursor_when_truncated() {
    let rows = vec![agent_row("a", Some("puff")), agent_row("b", Some("puffin"))];
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    for c in "puf".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    state.handle_key(key(KeyCode::Down));
    assert_eq!(state.cursor(), 1);
    // Narrow the query so only one match survives — cursor
    // shouldn't dangle past the new end.
    for c in "fin".chars() {
        state.handle_key(key(KeyCode::Char(c)));
        state.refresh_matches(&items);
    }
    assert_eq!(state.matches().len(), 1);
    assert_eq!(state.cursor(), 0);
}
