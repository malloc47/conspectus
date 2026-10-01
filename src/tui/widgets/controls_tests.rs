use super::*;
use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn ctx_with(view: View, grouping: Grouping, filter: &RowFilter, sort: Sort) -> ControlsContext<'_> {
    ControlsContext {
        view,
        grouping,
        filter,
        sort,
        mux_recency: crate::tui::MuxRecency::default(),
    }
}

#[test]
fn opens_with_active_view_selected() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Mux,
        Grouping::default_for(View::Mux),
        &filter,
        Sort::Hierarchy,
    );
    let state = ControlsOverlayState::new(&ctx);
    assert_eq!(state.cursor(), ControlsCursor::View(1));
}

#[test]
fn arrow_keys_skip_section_headers() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    // Five views + 5 sessions groupings + 5 filter rows
    // (3 predicates + 1 sessions-only float checkbox + clear) +
    // 2 sort rows = 17 actionable rows on Sessions.
    for _ in 0..flatten_rows(&ctx).len() {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(
        state.cursor(),
        ControlsCursor::View(0),
        "down wraps back to the top after one full cycle",
    );
}

#[test]
fn enter_on_active_view_closes_without_action() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Close);
}

#[test]
fn enter_on_different_view_switches_and_closes() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    state.handle_key(&ctx, key(KeyCode::Down));
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndClose(crate::tui::Msg::SwitchView(View::Mux))
    );
}

#[test]
fn enter_on_grouping_applies_and_stays_open() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    // Advance VIEW_OPTIONS.len() rows = past all View options into Grouping(0).
    for _ in 0..VIEW_OPTIONS.len() {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    // Land on the second grouping row (Graph).
    state.handle_key(&ctx, key(KeyCode::Down));
    assert_eq!(state.cursor(), ControlsCursor::Grouping(1));
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    let expected = Grouping::values_for(View::Sessions)[1];
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetGrouping(expected))
    );
}

#[test]
fn enter_on_harness_opens_multi_select_sub_editor() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    assert_eq!(state.cursor(), ControlsCursor::FilterHarness);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(SubEditor::Harness(_))));
}

#[test]
fn harness_sub_editor_confirm_emits_set_filter() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, key(KeyCode::Enter)); // open editor
    // Toggle the first harness (claude-code) on.
    state.handle_key(&ctx, key(KeyCode::Char(' ')));
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    match outcome {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(f)) => {
            let values = f
                .harness
                .as_ref()
                .map(|h| h.values().to_vec())
                .unwrap_or_default();
            assert_eq!(values, vec!["claude-code".to_string()]);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    // Editor closed after commit.
    assert!(state.sub_editor().is_none());
}

#[test]
fn harness_sub_editor_cancel_keeps_filter() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, key(KeyCode::Enter));
    state.handle_key(&ctx, key(KeyCode::Char(' ')));
    let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
    assert_eq!(outcome, ControlsOutcome::Continue);
    assert!(state.sub_editor().is_none());
}

#[test]
fn max_age_sub_editor_parses_typed_value() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, key(KeyCode::Down)); // → FilterMaxAge
    state.handle_key(&ctx, key(KeyCode::Enter)); // open editor
    for c in "7d".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(c)));
    }
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    match outcome {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(f)) => {
            assert_eq!(f.max_age, Some(std::time::Duration::from_secs(7 * 86_400)));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn max_age_invalid_value_keeps_editor_open() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, key(KeyCode::Down));
    state.handle_key(&ctx, key(KeyCode::Enter));
    for c in "nope".chars() {
        state.handle_key(&ctx, key(KeyCode::Char(c)));
    }
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(SubEditor::MaxAge(_))));
}

#[test]
fn clear_all_with_empty_filter_is_silent_noop() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    // Step past the three predicate rows and the sessions-only
    // float checkbox to land on FilterClear.
    for _ in 0..4 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Continue);
}

#[test]
fn clear_all_with_active_filter_emits_empty_filter() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["claude-code"])),
        ..RowFilter::default()
    };
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..4 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(RowFilter::default()))
    );
}

#[test]
fn float_muxed_checkbox_only_appears_on_sessions_view() {
    let filter = RowFilter::default();
    let rows_sessions = flatten_rows(&ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    ));
    assert!(rows_sessions.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(!rows_sessions.contains(&ControlsCursor::FilterFloatAttachedMuxes));

    let rows_mux = flatten_rows(&ctx_with(
        View::Mux,
        Grouping::default_for(View::Mux),
        &filter,
        Sort::Hierarchy,
    ));
    assert!(!rows_mux.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(rows_mux.contains(&ControlsCursor::FilterFloatAttachedMuxes));

    for view in [View::Union, View::Prs, View::Forks] {
        let rows = flatten_rows(&ctx_with(
            view,
            Grouping::default_for(view),
            &filter,
            Sort::Hierarchy,
        ));
        assert!(!rows.contains(&ControlsCursor::FilterFloatMuxedSessions));
        assert!(!rows.contains(&ControlsCursor::FilterFloatAttachedMuxes));
    }
}

#[test]
fn enter_on_float_muxed_sessions_toggles_bool() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..3 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatMuxedSessions);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    match outcome {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(f)) => {
            assert!(f.float_muxed_sessions_top);
            assert!(!f.float_attached_muxes_top);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn enter_on_float_attached_muxes_toggles_bool() {
    let filter = RowFilter {
        float_attached_muxes_top: true,
        ..RowFilter::default()
    };
    let ctx = ctx_with(
        View::Mux,
        Grouping::default_for(View::Mux),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..3 {
        state.handle_key(&ctx, key(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatAttachedMuxes);
    let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
    match outcome {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(f)) => {
            assert!(!f.float_attached_muxes_top, "Enter toggles bool off");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn ordering_bool_keeps_filter_non_empty_for_clear_all() {
    let filter = RowFilter {
        float_muxed_sessions_top: true,
        ..RowFilter::default()
    };
    assert!(!filter.is_empty());
    assert!(!filter.has_narrowing_predicates());
}

#[test]
fn max_content_line_count_tracks_rendered_controls_body() {
    let filter = RowFilter::default();
    let theme = Theme::default();
    let max_rendered = VIEW_OPTIONS
        .iter()
        .map(|view| {
            let ctx = ctx_with(
                *view,
                Grouping::default_for(*view),
                &filter,
                Sort::Hierarchy,
            );
            let state = ControlsOverlayState::new(&ctx);
            let widget = ControlsOverlayWidget::new(&state, ctx, &theme);
            widget.body_lines().len() + 2
        })
        .max()
        .unwrap();
    assert_eq!(max_controls_content_lines(), max_rendered);
}

#[test]
fn selected_last_sort_row_scrolls_into_short_controls_body() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let state = ControlsOverlayState {
        cursor: ControlsCursor::Sort(1),
        sub_editor: None,
    };
    let theme = Theme::default();
    let widget = ControlsOverlayWidget::new(&state, ctx, &theme);
    let mut lines = widget.body_lines();
    lines.push(Line::default());
    lines.push(Line::from("↑/↓ move · Enter pick · Esc close"));
    let cursor_line = widget.cursor_line_index().unwrap();
    let inner_height = 10;
    let offset = scroll_offset_for_cursor(Some(cursor_line), inner_height, lines.len());
    assert!(offset > 0, "short controls body should scroll");
    assert!(cursor_line >= offset as usize);
    assert!(cursor_line < offset as usize + inner_height);
}

#[test]
fn esc_at_top_level_closes_overlay() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
    assert_eq!(outcome, ControlsOutcome::Close);
}

#[test]
fn format_and_parse_round_trip_for_common_durations() {
    let cases = [
        (std::time::Duration::from_secs(7 * 86_400), "7d"),
        (std::time::Duration::from_secs(2 * 3_600), "2h"),
        (std::time::Duration::from_secs(15 * 60), "15m"),
        (std::time::Duration::from_secs(45), "45s"),
    ];
    for (dur, expected) in cases {
        assert_eq!(format_duration_for_input(dur), expected);
        assert_eq!(parse_max_age(expected), Ok(dur));
    }
}
