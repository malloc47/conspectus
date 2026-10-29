use super::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use tuikit::testkit::keys::press;

fn ctx_with(view: View, grouping: Grouping, filter: &RowFilter, sort: Sort) -> ControlsContext<'_> {
    ControlsContext {
        view,
        grouping,
        filter,
        sort,
        mux_recency: crate::tui::MuxRecency::default(),
        preview_wrap: crate::tui::PreviewWrap::default(),
        session_preview: crate::tui::SessionPreview::default(),
        content: None,
        search_kinds: crate::viewer::search::ContentKinds::default(),
        content_group: crate::viewer::search::Grouping::Session,
        search_history: None,
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
        state.handle_key(&ctx, press(KeyCode::Down));
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
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
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
    state.handle_key(&ctx, press(KeyCode::Down));
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndClose(crate::tui::Msg::SwitchView(View::Mux))
    );
}

#[test]
fn space_on_grouping_applies_and_stays_open() {
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
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    // Land on the second grouping row (Graph).
    state.handle_key(&ctx, press(KeyCode::Down));
    assert_eq!(state.cursor(), ControlsCursor::Grouping(1));
    let outcome = state.handle_key(&ctx, press(KeyCode::Char(' ')));
    let expected = Grouping::values_for(View::Sessions)[1];
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetGrouping(expected))
    );
}

#[test]
fn enter_on_grouping_applies_and_closes() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    for _ in 0..=VIEW_OPTIONS.len() {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::Grouping(1));
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
    let expected = Grouping::values_for(View::Sessions)[1];
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndClose(crate::tui::Msg::SetGrouping(expected))
    );
}

#[test]
fn space_on_a_view_switches_and_stays_open() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    state.handle_key(&ctx, press(KeyCode::Down));
    let outcome = state.handle_key(&ctx, press(KeyCode::Char(' ')));
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SwitchView(View::Mux))
    );
}

#[test]
fn enter_on_a_checkbox_closes_and_leaves_it_alone() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new(&ctx);
    // Walk to the first float-muxed-sessions checkbox.
    for _ in 0..200 {
        if state.cursor() == ControlsCursor::FilterFloatMuxedSessions {
            break;
        }
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatMuxedSessions);
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::Close
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
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Continue);
    assert!(matches!(state.sub_editor(), Some(SubEditor::Harness(_))));
}

#[test]
fn behind_an_open_sub_editor_the_cursor_row_takes_the_inactive_look() {
    let filter = RowFilter::default();
    let theme = Theme::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    let row_style = |state: &mut ControlsOverlayState| {
        let widget = ControlsOverlayWidget::new(state, ctx.clone(), &theme);
        let index = widget.cursor_line_index().expect("cursor row");
        let line = widget.body_lines()[index].clone();
        assert!(line.to_string().starts_with(marks::POINTER), "{line}");
        line.spans[1].style.add_modifier
    };
    assert!(row_style(&mut state).contains(theme.selection_active));
    state.handle_key(&ctx, press(KeyCode::Enter));
    assert!(state.sub_editor().is_some());
    let behind = row_style(&mut state);
    assert!(behind.contains(theme.selection_inactive), "{behind:?}");
    assert!(!behind.contains(Modifier::REVERSED), "{behind:?}");
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
    state.handle_key(&ctx, press(KeyCode::Enter)); // open editor
    // Toggle the first harness (claude-code) on.
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
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
fn harness_sub_editor_selects_every_harness_or_none() {
    let filter = RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    };
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, press(KeyCode::Enter));
    state.handle_key(&ctx, press(KeyCode::Char('a')));
    match state.handle_key(&ctx, press(KeyCode::Enter)) {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(next)) => {
            let values = next
                .harness
                .map(|h| h.values().to_vec())
                .unwrap_or_default();
            assert_eq!(values.len(), harness_options().len(), "{values:?}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    state.handle_key(&ctx, press(KeyCode::Enter));
    state.handle_key(&ctx, press(KeyCode::Char('n')));
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(RowFilter::default())),
        "nothing checked clears the filter"
    );
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
    state.handle_key(&ctx, press(KeyCode::Enter));
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    let outcome = state.handle_key(&ctx, press(KeyCode::Esc));
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
    state.handle_key(&ctx, press(KeyCode::Down)); // → FilterMaxAge
    state.handle_key(&ctx, press(KeyCode::Enter)); // open editor
    for c in "7d".chars() {
        state.handle_key(&ctx, press(KeyCode::Char(c)));
    }
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
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
    state.handle_key(&ctx, press(KeyCode::Down));
    state.handle_key(&ctx, press(KeyCode::Enter));
    for c in "nope".chars() {
        state.handle_key(&ctx, press(KeyCode::Char(c)));
    }
    // The editor stays open, applies nothing, and its error line says
    // what is wrong until the next edit.
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
    assert_eq!(outcome, ControlsOutcome::Continue);
    let error = |state: &ControlsOverlayState| match state.sub_editor() {
        Some(SubEditor::MaxAge(prompt)) => prompt.error().map(str::to_string),
        other => panic!("the max-age editor closed: {other:?}"),
    };
    assert_eq!(
        error(&state).as_deref(),
        Some("max age: not a non-negative integer: ``")
    );
    state.handle_key(&ctx, press(KeyCode::Backspace));
    assert_eq!(error(&state), None, "an edit clears the reason");
}

#[test]
fn esc_in_a_text_editor_steps_back_and_ctrl_c_closes_the_panel() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, press(KeyCode::Down));
    state.handle_key(&ctx, press(KeyCode::Enter));
    state.handle_key(&ctx, press(KeyCode::Char('q')));
    assert!(state.editor_takes_text(), "q is typed, not a way out");
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Esc)),
        ControlsOutcome::Continue
    );
    assert!(state.sub_editor().is_none(), "Esc steps back to the panel");

    state.handle_key(&ctx, press(KeyCode::Enter));
    assert_eq!(
        state.handle_key(&ctx, tuikit::testkit::keys::ctrl('c')),
        ControlsOutcome::Close
    );
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
    // Step past the four predicate rows, the sessions-only float
    // checkbox, and pinned only to land on FilterClear.
    for _ in 0..6 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
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
    for _ in 0..6 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    let outcome = state.handle_key(&ctx, press(KeyCode::Enter));
    assert_eq!(
        outcome,
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::ClearFilters)
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

    let rows_prs = flatten_rows(&ctx_with(
        View::Prs,
        Grouping::default_for(View::Prs),
        &filter,
        Sort::Hierarchy,
    ));
    assert!(!rows_prs.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(!rows_prs.contains(&ControlsCursor::FilterFloatAttachedMuxes));
}

#[test]
fn space_on_float_muxed_sessions_toggles_bool() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..4 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatMuxedSessions);
    let outcome = state.handle_key(&ctx, press(KeyCode::Char(' ')));
    match outcome {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(f)) => {
            assert!(f.float_muxed_sessions_top);
            assert!(!f.float_attached_muxes_top);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn space_on_float_attached_muxes_toggles_bool() {
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
    for _ in 0..4 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatAttachedMuxes);
    let outcome = state.handle_key(&ctx, press(KeyCode::Char(' ')));
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
fn selected_last_sort_row_scrolls_into_short_controls_body() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let mut state = ControlsOverlayState {
        cursor: ControlsCursor::Sort(1),
        sub_editor: None,
    };
    let theme = Theme::default();
    // Too short for every row: the body scrolls to the cursor's row.
    let area = Rect::new(0, 0, 80, 14);
    let mut buf = Buffer::empty(area);
    ControlsOverlayWidget::new(&mut state, ctx, &theme).render(area, &mut buf);
    let rows: Vec<String> = (0..area.height)
        .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("{} Recency", marks::POINTER))),
        "the selected sort row is drawn:\n{}",
        rows.join("\n")
    );
    assert!(
        !rows.iter().any(|row| row.contains("Sessions [1]")),
        "the top of the body scrolled away:\n{}",
        rows.join("\n")
    );
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
    let outcome = state.handle_key(&ctx, press(KeyCode::Esc));
    assert_eq!(outcome, ControlsOutcome::Close);
}

#[test]
fn q_dismisses_the_overlay_except_in_an_editor_that_takes_text() {
    let filter = RowFilter::default();
    let ctx = ctx_with(
        View::Sessions,
        Grouping::default_for(View::Sessions),
        &filter,
        Sort::Hierarchy,
    );
    let q = press(KeyCode::Char('q'));
    // The list, and the harness checklist behind it: nothing is typed.
    let mut state = ControlsOverlayState::new(&ctx);
    assert_eq!(state.handle_key(&ctx, q), ControlsOutcome::Close);
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, press(KeyCode::Enter));
    assert!(matches!(state.sub_editor(), Some(SubEditor::Harness(_))));
    assert_eq!(
        state.handle_key(&ctx, q),
        ControlsOutcome::Close,
        "q dismisses the whole overlay, not just the checklist"
    );
    // The max-age editor takes text: q types; Ctrl-C still dismisses.
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    state.handle_key(&ctx, press(KeyCode::Down));
    state.handle_key(&ctx, press(KeyCode::Enter));
    assert!(state.editor_takes_text());
    assert_eq!(state.handle_key(&ctx, q), ControlsOutcome::Continue);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(state.handle_key(&ctx, ctrl_c), ControlsOutcome::Close);
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

#[test]
fn preview_wrap_section_lists_every_mode_and_applies_the_pick() {
    let filter = RowFilter::default();
    for view in VIEW_OPTIONS {
        let ctx = ctx_with(
            *view,
            Grouping::default_for(*view),
            &filter,
            Sort::Hierarchy,
        );
        let rows = flatten_rows(&ctx);
        let wrap_rows = rows
            .iter()
            .filter(|row| matches!(row, ControlsCursor::PreviewWrap(_)))
            .count();
        assert_eq!(wrap_rows, crate::tui::PreviewWrap::ALL.len(), "{view:?}");

        let mut state = ControlsOverlayState {
            cursor: ControlsCursor::PreviewWrap(2),
            sub_editor: None,
        };
        assert_eq!(
            state.handle_key(&ctx, press(KeyCode::Char(' '))),
            ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetPreviewWrap(
                crate::tui::PreviewWrap::ALL[2]
            ))
        );
    }
}

#[test]
fn session_preview_section_closes_the_overlay_and_applies_the_pick() {
    let filter = RowFilter::default();
    for view in VIEW_OPTIONS {
        let ctx = ctx_with(
            *view,
            Grouping::default_for(*view),
            &filter,
            Sort::Hierarchy,
        );
        let rows = flatten_rows(&ctx);
        let choice_rows = rows
            .iter()
            .filter(|row| matches!(row, ControlsCursor::SessionPreview(_)))
            .count();
        assert_eq!(
            choice_rows,
            crate::tui::SessionPreview::ALL.len(),
            "{view:?}"
        );
        assert!(
            matches!(rows.last(), Some(ControlsCursor::SessionPreview(_))),
            "the section closes the overlay: {view:?}"
        );

        let mut state = ControlsOverlayState {
            cursor: ControlsCursor::SessionPreview(1),
            sub_editor: None,
        };
        assert_eq!(
            state.handle_key(&ctx, press(KeyCode::Char(' '))),
            ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetSessionPreview(
                crate::tui::SessionPreview::Pane
            ))
        );
    }
}

#[test]
fn cursor_line_index_points_at_the_cursored_row_in_every_section() {
    let filter = RowFilter::default();
    let theme = Theme::default();
    for view in VIEW_OPTIONS {
        let ctx = ctx_with(
            *view,
            Grouping::default_for(*view),
            &filter,
            Sort::Hierarchy,
        );
        for row in flatten_rows(&ctx) {
            let mut state = ControlsOverlayState {
                cursor: row,
                sub_editor: None,
            };
            let widget = ControlsOverlayWidget::new(&mut state, ctx.clone(), &theme);
            let lines = widget.body_lines();
            let index = widget.cursor_line_index().expect("cursor row is rendered");
            assert!(
                lines[index].to_string().starts_with(marks::POINTER),
                "{view:?} {row:?}: {}",
                lines[index]
            );
            for span in &lines[index].spans {
                assert!(
                    span.style.add_modifier.contains(theme.selection_active),
                    "{view:?} {row:?}: the whole row is highlighted: {span:?}"
                );
            }
        }
    }
}

#[test]
fn pr_branch_rows_only_appear_on_the_prs_view() {
    let filter = RowFilter::default();
    for view in [
        View::Sessions,
        View::Mux,
        View::Pinned,
        View::History,
        View::Search,
    ] {
        let rows = flatten_rows(&ctx_with(
            view,
            Grouping::default_for(view),
            &filter,
            Sort::Hierarchy,
        ));
        assert!(!rows.contains(&ControlsCursor::FilterPrBranch));
        assert!(!rows.contains(&ControlsCursor::FilterFloatTrackedPrs));
    }
    let rows = flatten_rows(&ctx_with(
        View::Prs,
        Grouping::default_for(View::Prs),
        &filter,
        Sort::Hierarchy,
    ));
    let at = |cursor| rows.iter().position(|row| *row == cursor).expect("row");
    assert_eq!(
        at(ControlsCursor::FilterPrBranch),
        at(ControlsCursor::FilterPrState) + 1
    );
    assert_eq!(
        at(ControlsCursor::FilterFloatTrackedPrs),
        at(ControlsCursor::FilterPrBranch) + 1
    );
    assert_eq!(
        at(ControlsCursor::FilterPinnedOnly),
        at(ControlsCursor::FilterFloatTrackedPrs) + 1
    );
    assert_eq!(
        at(ControlsCursor::FilterClear),
        at(ControlsCursor::FilterPinnedOnly) + 1
    );
}

fn prs_ctx(filter: &RowFilter) -> ControlsContext<'_> {
    ctx_with(
        View::Prs,
        Grouping::default_for(View::Prs),
        filter,
        Sort::Hierarchy,
    )
}

#[test]
fn space_on_local_branch_row_cycles_any_tracked_untracked() {
    use crate::filter::PrBranchFilter;

    let mut filter = RowFilter::default();
    let mut seen = Vec::new();
    for _ in 0..3 {
        let ctx = prs_ctx(&filter);
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        for _ in 0..5 {
            state.handle_key(&ctx, press(KeyCode::Down));
        }
        assert_eq!(state.cursor(), ControlsCursor::FilterPrBranch);
        match state.handle_key(&ctx, press(KeyCode::Char(' '))) {
            ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(next)) => {
                seen.push(next.pr_branch);
                filter = next;
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    assert_eq!(
        seen,
        [
            Some(PrBranchFilter::Tracked),
            Some(PrBranchFilter::Untracked),
            None
        ]
    );
}

#[test]
fn space_on_float_tracked_prs_toggles_bool() {
    let filter = RowFilter::default();
    let ctx = prs_ctx(&filter);
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..6 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterFloatTrackedPrs);
    match state.handle_key(&ctx, press(KeyCode::Char(' '))) {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(next)) => {
            assert!(next.float_tracked_prs_top);
            assert_eq!(next.pr_branch, None);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn clear_all_resets_the_pr_settings() {
    let filter = RowFilter {
        pr_branch: Some(crate::filter::PrBranchFilter::Untracked),
        float_tracked_prs_top: true,
        ..RowFilter::default()
    };
    let ctx = prs_ctx(&filter);
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..8 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::ClearFilters)
    );
}

#[test]
fn pr_state_row_only_appears_on_the_prs_view() {
    let filter = RowFilter::default();
    for view in [
        View::Sessions,
        View::Mux,
        View::Pinned,
        View::History,
        View::Search,
    ] {
        let rows = flatten_rows(&ctx_with(
            view,
            Grouping::default_for(view),
            &filter,
            Sort::Hierarchy,
        ));
        assert!(!rows.contains(&ControlsCursor::FilterPrState));
    }
    assert!(flatten_rows(&prs_ctx(&filter)).contains(&ControlsCursor::FilterPrState));
}

#[test]
fn pr_state_sub_editor_commits_the_chosen_states() {
    use crate::filter::{PrStateFilter, PrStateKey};

    let filter = RowFilter::default();
    let ctx = prs_ctx(&filter);
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..4 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterPrState);
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::Continue
    );
    assert!(matches!(state.sub_editor(), Some(SubEditor::PrState(_))));

    // Options are open, draft, merged, closed: pick open and merged.
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    state.handle_key(&ctx, press(KeyCode::Down));
    state.handle_key(&ctx, press(KeyCode::Down));
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    match state.handle_key(&ctx, press(KeyCode::Enter)) {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(next)) => assert_eq!(
            next.pr_state,
            Some(PrStateFilter::from_values([
                PrStateKey::Open,
                PrStateKey::Merged
            ]))
        ),
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(state.sub_editor().is_none());
}

#[test]
fn pr_state_sub_editor_opens_with_the_current_states_selected_and_clears_when_empty() {
    use crate::filter::{PrStateFilter, PrStateKey};

    let filter = RowFilter {
        pr_state: Some(PrStateFilter::from_values([PrStateKey::Open])),
        ..RowFilter::default()
    };
    let ctx = prs_ctx(&filter);
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..4 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    state.handle_key(&ctx, press(KeyCode::Enter));
    // Un-pick the preselected open state and confirm: no constraint.
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    match state.handle_key(&ctx, press(KeyCode::Enter)) {
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(next)) => {
            assert_eq!(next.pr_state, None);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

fn content_ctx<'a>(filter: &'a RowFilter, text: Option<&'a str>) -> ControlsContext<'a> {
    ControlsContext {
        content: text.map(|text| {
            (
                text,
                crate::tui::ContentStatus::Done(crate::content_search::Progress::default()),
                0,
            )
        }),
        ..ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            filter,
            Sort::Hierarchy,
        )
    }
}

fn on_content_row(ctx: &ControlsContext<'_>) -> ControlsOverlayState {
    let mut state = ControlsOverlayState::new_at_filters(ctx);
    for _ in 0..3 {
        state.handle_key(ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterContent);
    state
}

#[test]
fn the_content_row_opens_an_editor_and_starts_a_search_on_enter() {
    let filter = RowFilter::default();
    let ctx = content_ctx(&filter, None);
    let mut state = on_content_row(&ctx);
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::Continue,
        "Enter opens the editor"
    );
    assert!(state.sub_editor.is_some());
    for ch in " needle ".chars() {
        state.handle_key(&ctx, press(KeyCode::Char(ch)));
    }
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::StartContentSearch {
            text: "needle".into(),
            kinds: crate::viewer::search::ContentKinds::default(),
        })
    );
    assert!(!state.sub_editor.is_some());
}

#[test]
fn space_opens_the_content_editor_too() {
    let filter = RowFilter::default();
    let ctx = content_ctx(&filter, None);
    let mut state = on_content_row(&ctx);
    state.handle_key(&ctx, press(KeyCode::Char(' ')));
    assert!(state.sub_editor.is_some());
}

#[test]
fn the_content_editor_starts_with_the_active_query_and_empty_text_clears_it() {
    let filter = RowFilter::default();
    let ctx = content_ctx(&filter, Some("needle"));
    let mut state = on_content_row(&ctx);
    state.handle_key(&ctx, press(KeyCode::Enter));
    for _ in 0.."needle".len() {
        state.handle_key(&ctx, press(KeyCode::Backspace));
    }
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::ClearContentSearch)
    );
}

#[test]
fn escape_in_the_content_editor_keeps_the_search_as_it_was() {
    let filter = RowFilter::default();
    let ctx = content_ctx(&filter, Some("needle"));
    let mut state = on_content_row(&ctx);
    state.handle_key(&ctx, press(KeyCode::Enter));
    state.handle_key(&ctx, press(KeyCode::Char('x')));
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Esc)),
        ControlsOutcome::Continue
    );
    assert!(!state.sub_editor.is_some());
}

#[test]
fn clear_all_has_something_to_clear_when_only_a_search_is_active() {
    let filter = RowFilter::default();
    let ctx = content_ctx(&filter, Some("needle"));
    let mut state = ControlsOverlayState::new_at_filters(&ctx);
    for _ in 0..6 {
        state.handle_key(&ctx, press(KeyCode::Down));
    }
    assert_eq!(state.cursor(), ControlsCursor::FilterClear);
    assert_eq!(
        state.handle_key(&ctx, press(KeyCode::Enter)),
        ControlsOutcome::ApplyAndStay(crate::tui::Msg::ClearFilters)
    );
}
