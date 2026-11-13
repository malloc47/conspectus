use super::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use tuikit::overlay::Overlay;
use tuikit::testkit::buffer::{rows, style_at};
use tuikit::testkit::keys::{ctrl, press};

use crate::tui::Theme;

/// The settings a test panel reads: an owned stand-in for the App, whose
/// [`Fixture::context`] lends them out as the App's does.
struct Fixture {
    view: View,
    grouping: Grouping,
    filter: RowFilter,
    sort: Sort,
    content: Option<String>,
}

impl Fixture {
    fn new(view: View) -> Self {
        Self {
            view,
            grouping: Grouping::default_for(view),
            filter: RowFilter::default(),
            sort: Sort::Hierarchy,
            content: None,
        }
    }

    fn filter(mut self, filter: RowFilter) -> Self {
        self.filter = filter;
        self
    }

    /// With a finished content search for `text` active.
    fn searching(mut self, text: &str) -> Self {
        self.content = Some(text.to_string());
        self
    }

    fn context(&self) -> ControlsContext<'_> {
        ControlsContext {
            view: self.view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
            mux_recency: crate::tui::MuxRecency::default(),
            preview_wrap: crate::tui::WrapMode::default(),
            session_preview: crate::tui::SessionPreview::default(),
            content: self.content.as_deref().map(|text| {
                (
                    text,
                    crate::tui::ContentStatus::Done(crate::content_search::Progress::default()),
                    0,
                )
            }),
            search_kinds: crate::viewer::search::ContentKinds::default(),
            content_group: crate::viewer::search::Grouping::Session,
            search_history: None,
        }
    }
}

type Panel = SettingsPanel<ControlsCursor, Msg, Fixture>;

/// The panel as `f` opens it: on the active view's row.
fn opened(fixture: &Fixture) -> Panel {
    panel(Fixture::context).at(opening_row(&fixture.context()))
}

/// The panel opened on `row`.
fn panel_at(row: ControlsCursor) -> Panel {
    panel(Fixture::context).at(row)
}

/// The panel opened on Filters > harness.
fn at_filters() -> Panel {
    panel_at(ControlsCursor::FilterHarness)
}

/// The editor `which` names, seeded from `fixture`.
fn open_editor(which: ControlsEditor, fixture: &Fixture) -> Modal {
    editor(which, &fixture.context())
}

fn key(panel: &mut Panel, fixture: &Fixture, event: KeyEvent) -> Outcome<Msg> {
    panel.handle(event, fixture)
}

fn down(panel: &mut Panel, fixture: &Fixture, times: usize) {
    for _ in 0..times {
        key(panel, fixture, press(KeyCode::Down));
    }
}

fn cursor(panel: &Panel) -> ControlsCursor {
    *panel.cursor().expect("the panel places its cursor")
}

/// The editor a row's message asks the reducer to open, as it opens it.
fn editor_for(outcome: Outcome<Msg>, fixture: &Fixture) -> Modal {
    match outcome {
        Outcome::Stay(Msg::OpenControlsEditor(which)) => editor(which, &fixture.context()),
        other => panic!("the row opened no editor: {other:?}"),
    }
}

/// One key into an editor step.
fn edit(modal: &mut Modal, event: KeyEvent) -> Outcome<Msg> {
    match modal {
        Modal::Checklist(list) => list.handle(event, &()),
        Modal::Prompt(prompt) => prompt.handle(event, &()),
        other => panic!("not a Controls editor: {}", other.name()),
    }
}

fn type_into(modal: &mut Modal, text: &str) {
    for c in text.chars() {
        edit(modal, press(KeyCode::Char(c)));
    }
}

/// The filter an editor's answer sets, as it steps back to the panel.
fn set_filter(outcome: Outcome<Msg>) -> RowFilter {
    match outcome {
        Outcome::Back(Msg::SetFilter(filter)) => filter,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

/// The filter a row's change sets while the panel stays open.
fn applied_filter(outcome: Outcome<Msg>) -> RowFilter {
    match outcome {
        Outcome::Stay(Msg::SetFilter(filter)) => filter,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

/// The panel drawn on a `width` by `height` screen, with the panel on top
/// (`focused`) or under an editor of its own.
fn drawn(panel: &mut Panel, fixture: &Fixture, size: (u16, u16), focused: bool) -> Buffer {
    let theme = Theme::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, size.0, size.1));
    if focused {
        panel.draw(buf.area, &mut buf, fixture, &theme);
    } else {
        panel.draw_under_step(buf.area, &mut buf, fixture, &theme);
    }
    buf
}

fn marked(buf: &Buffer) -> Vec<String> {
    rows(buf)
        .into_iter()
        .filter(|row| row.contains(marks::POINTER))
        .collect()
}

#[test]
fn opens_with_active_view_selected() {
    let fixture = Fixture::new(View::Mux);
    assert_eq!(opening_row(&fixture.context()), ControlsCursor::View(1));
    let mut panel = opened(&fixture);
    let buf = drawn(&mut panel, &fixture, (100, 30), true);
    assert!(
        marked(&buf)[0].contains("Mux [2]  active"),
        "{:?}",
        rows(&buf)
    );
}

#[test]
fn arrow_keys_skip_section_headers() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    down(&mut panel, &fixture, flatten_rows(&fixture.context()).len());
    assert_eq!(
        cursor(&panel),
        ControlsCursor::View(0),
        "down wraps back to the top after one full cycle",
    );
}

#[test]
fn paging_and_end_keys_move_over_the_rows_and_stop_at_the_ends() {
    let fixture = Fixture::new(View::Sessions);
    let rows = flatten_rows(&fixture.context());
    let last = *rows.last().expect("rows");
    let mut panel = opened(&fixture);
    // A short screen, so a page is less than the panel.
    let _ = drawn(&mut panel, &fixture, (100, 20), true);
    key(&mut panel, &fixture, press(KeyCode::PageDown));
    let paged = rows.iter().position(|row| *row == cursor(&panel));
    assert!(paged.is_some_and(|index| index > 3), "{:?}", cursor(&panel));
    for end in [press(KeyCode::End), press(KeyCode::Char('G'))] {
        let mut panel = opened(&fixture);
        key(&mut panel, &fixture, end);
        assert_eq!(cursor(&panel), last, "{end:?}");
        key(&mut panel, &fixture, press(KeyCode::PageDown));
        assert_eq!(cursor(&panel), last, "PgDn stops at the last row");
    }
    for home in [press(KeyCode::Home), press(KeyCode::Char('g'))] {
        let mut panel = panel_at(last);
        key(&mut panel, &fixture, home);
        assert_eq!(cursor(&panel), ControlsCursor::View(0), "{home:?}");
        key(&mut panel, &fixture, press(KeyCode::PageUp));
        assert_eq!(cursor(&panel), ControlsCursor::View(0), "PgUp stops");
    }
}

#[test]
fn enter_on_active_view_closes_without_action() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Close);
}

#[test]
fn enter_on_different_view_switches_and_closes() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    key(&mut panel, &fixture, press(KeyCode::Down));
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Commit(Msg::SwitchView(View::Mux)));
}

#[test]
fn space_on_grouping_applies_and_stays_open() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    // Advance VIEW_OPTIONS.len() rows = past all View options into Grouping(0).
    down(&mut panel, &fixture, VIEW_OPTIONS.len());
    // Land on the second grouping row.
    key(&mut panel, &fixture, press(KeyCode::Down));
    assert_eq!(cursor(&panel), ControlsCursor::Grouping(1));
    let outcome = key(&mut panel, &fixture, press(KeyCode::Char(' ')));
    let expected = Grouping::values_for(View::Sessions)[1];
    assert_eq!(outcome, Outcome::Stay(Msg::SetGrouping(expected)));
}

#[test]
fn enter_on_grouping_applies_and_closes() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    down(&mut panel, &fixture, VIEW_OPTIONS.len() + 1);
    assert_eq!(cursor(&panel), ControlsCursor::Grouping(1));
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    let expected = Grouping::values_for(View::Sessions)[1];
    assert_eq!(outcome, Outcome::Commit(Msg::SetGrouping(expected)));
}

#[test]
fn space_on_a_view_switches_and_stays_open() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    key(&mut panel, &fixture, press(KeyCode::Down));
    let outcome = key(&mut panel, &fixture, press(KeyCode::Char(' ')));
    assert_eq!(outcome, Outcome::Stay(Msg::SwitchView(View::Mux)));
}

#[test]
fn enter_on_a_checkbox_closes_and_leaves_it_alone() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = panel_at(ControlsCursor::FilterFloatMuxedSessions);
    assert_eq!(
        key(&mut panel, &fixture, press(KeyCode::Enter)),
        Outcome::Close
    );
}

#[test]
fn enter_on_harness_opens_multi_select_sub_editor() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Stay(Msg::OpenControlsEditor(ControlsEditor::Harness))
    );
    assert!(matches!(editor_for(outcome, &fixture), Modal::Checklist(_)));
}

#[test]
fn behind_an_open_sub_editor_the_cursor_row_takes_the_inactive_look() {
    let theme = Theme::default();
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    let style = |buf: &Buffer| {
        assert!(marked(buf)[0].contains("harness"), "{:?}", rows(buf));
        style_at(buf, "harness ")
            .expect("the cursor row")
            .add_modifier
    };
    let on_top = drawn(&mut panel, &fixture, (100, 40), true);
    assert!(style(&on_top).contains(theme.selection_active));
    let behind = style(&drawn(&mut panel, &fixture, (100, 40), false));
    assert!(behind.contains(theme.selection_inactive), "{behind:?}");
    assert!(!behind.contains(Modifier::REVERSED), "{behind:?}");
}

#[test]
fn harness_sub_editor_confirm_emits_set_filter() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    let mut editor = editor_for(key(&mut panel, &fixture, press(KeyCode::Enter)), &fixture);
    // Toggle the first harness (claude-code) on.
    edit(&mut editor, press(KeyCode::Char(' ')));
    let filter = set_filter(edit(&mut editor, press(KeyCode::Enter)));
    let values = filter
        .harness
        .as_ref()
        .map(|h| h.values().to_vec())
        .unwrap_or_default();
    assert_eq!(values, vec!["claude-code".to_string()]);
}

#[test]
fn harness_sub_editor_selects_every_harness_or_none() {
    let fixture = Fixture::new(View::Sessions).filter(RowFilter {
        harness: Some(HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    });
    let mut editor = open_editor(ControlsEditor::Harness, &fixture);
    edit(&mut editor, press(KeyCode::Char('a')));
    let all = set_filter(edit(&mut editor, press(KeyCode::Enter)));
    let values = all.harness.map(|h| h.values().to_vec()).unwrap_or_default();
    assert_eq!(values.len(), harness_options().len(), "{values:?}");
    let mut editor = open_editor(ControlsEditor::Harness, &fixture);
    edit(&mut editor, press(KeyCode::Char('n')));
    assert_eq!(
        edit(&mut editor, press(KeyCode::Enter)),
        Outcome::Back(Msg::SetFilter(RowFilter::default())),
        "nothing checked clears the filter"
    );
}

#[test]
fn harness_sub_editor_cancel_keeps_filter() {
    let fixture = Fixture::new(View::Sessions);
    let mut editor = open_editor(ControlsEditor::Harness, &fixture);
    edit(&mut editor, press(KeyCode::Char(' ')));
    assert_eq!(
        edit(&mut editor, press(KeyCode::Esc)),
        Outcome::Close,
        "Esc steps back to the panel and applies nothing"
    );
}

#[test]
fn max_age_sub_editor_parses_typed_value() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    key(&mut panel, &fixture, press(KeyCode::Down)); // → FilterMaxAge
    let mut editor = editor_for(key(&mut panel, &fixture, press(KeyCode::Enter)), &fixture);
    type_into(&mut editor, "7d");
    let filter = set_filter(edit(&mut editor, press(KeyCode::Enter)));
    assert_eq!(
        filter.max_age,
        Some(std::time::Duration::from_secs(7 * 86_400))
    );
}

#[test]
fn max_age_invalid_value_keeps_editor_open() {
    let fixture = Fixture::new(View::Sessions);
    let mut editor = open_editor(ControlsEditor::MaxAge, &fixture);
    type_into(&mut editor, "nope");
    // The editor stays open, applies nothing, and its error line says
    // what is wrong until the next edit.
    assert_eq!(edit(&mut editor, press(KeyCode::Enter)), Outcome::Consumed);
    let error = |editor: &Modal| match editor {
        Modal::Prompt(prompt) => prompt.error().map(str::to_string),
        other => panic!("not the max-age editor: {}", other.name()),
    };
    assert_eq!(
        error(&editor).as_deref(),
        Some("max age: not a non-negative integer: ``")
    );
    edit(&mut editor, press(KeyCode::Backspace));
    assert_eq!(error(&editor), None, "an edit clears the reason");
}

#[test]
fn esc_in_a_text_editor_steps_back_and_ctrl_c_closes_the_panel() {
    let fixture = Fixture::new(View::Sessions);
    let mut editor = open_editor(ControlsEditor::MaxAge, &fixture);
    assert_eq!(
        edit(&mut editor, press(KeyCode::Char('q'))),
        Outcome::Consumed,
        "q is typed, not a way out"
    );
    assert!(Overlay::<Msg, ()>::takes_text(&editor_prompt(&editor)));
    assert_eq!(
        edit(&mut editor, press(KeyCode::Esc)),
        Outcome::Close,
        "Esc steps back to the panel"
    );
    assert_eq!(
        edit(&mut editor, ctrl('c')),
        Outcome::Dismiss,
        "Ctrl-C dismisses the editor and the panel under it"
    );
}

fn editor_prompt(editor: &Modal) -> tuikit::text_prompt::TextPrompt<Msg> {
    match editor {
        Modal::Prompt(prompt) => prompt.clone(),
        other => panic!("not a prompt: {}", other.name()),
    }
}

#[test]
fn clear_all_with_empty_filter_is_silent_noop() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    // Step past the four predicate rows, the sessions-only float
    // checkbox, and pinned only to land on FilterClear.
    down(&mut panel, &fixture, 6);
    assert_eq!(cursor(&panel), ControlsCursor::FilterClear);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Consumed);
}

#[test]
fn clear_all_with_active_filter_emits_empty_filter() {
    let fixture = Fixture::new(View::Sessions).filter(RowFilter {
        harness: Some(HarnessFilter::from_values(["claude-code"])),
        ..RowFilter::default()
    });
    let mut panel = at_filters();
    down(&mut panel, &fixture, 6);
    assert_eq!(cursor(&panel), ControlsCursor::FilterClear);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(outcome, Outcome::Stay(Msg::ClearFilters));
}

#[test]
fn float_muxed_checkbox_only_appears_on_sessions_view() {
    let rows = |view| flatten_rows(&Fixture::new(view).context());
    let rows_sessions = rows(View::Sessions);
    assert!(rows_sessions.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(!rows_sessions.contains(&ControlsCursor::FilterFloatAttachedMuxes));

    let rows_mux = rows(View::Mux);
    assert!(!rows_mux.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(rows_mux.contains(&ControlsCursor::FilterFloatAttachedMuxes));

    let rows_prs = rows(View::Prs);
    assert!(!rows_prs.contains(&ControlsCursor::FilterFloatMuxedSessions));
    assert!(!rows_prs.contains(&ControlsCursor::FilterFloatAttachedMuxes));
}

#[test]
fn space_on_float_muxed_sessions_toggles_bool() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = at_filters();
    down(&mut panel, &fixture, 4);
    assert_eq!(cursor(&panel), ControlsCursor::FilterFloatMuxedSessions);
    let filter = applied_filter(key(&mut panel, &fixture, press(KeyCode::Char(' '))));
    assert!(filter.float_muxed_sessions_top);
    assert!(!filter.float_attached_muxes_top);
}

#[test]
fn space_on_float_attached_muxes_toggles_bool() {
    let fixture = Fixture::new(View::Mux).filter(RowFilter {
        float_attached_muxes_top: true,
        ..RowFilter::default()
    });
    let mut panel = at_filters();
    down(&mut panel, &fixture, 4);
    assert_eq!(cursor(&panel), ControlsCursor::FilterFloatAttachedMuxes);
    let filter = applied_filter(key(&mut panel, &fixture, press(KeyCode::Char(' '))));
    assert!(!filter.float_attached_muxes_top, "Space toggles bool off");
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
    let fixture = Fixture::new(View::Sessions);
    let mut panel = panel_at(ControlsCursor::Sort(1));
    // Too short for every row: the body scrolls to the cursor's row.
    let buf = drawn(&mut panel, &fixture, (80, 14), true);
    let rows = rows(&buf);
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
    let fixture = Fixture::new(View::Sessions);
    let mut panel = opened(&fixture);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Esc));
    assert_eq!(outcome, Outcome::Close);
}

#[test]
fn q_dismisses_the_overlay_except_in_an_editor_that_takes_text() {
    let fixture = Fixture::new(View::Sessions);
    let q = press(KeyCode::Char('q'));
    // The list, and the harness checklist over it: nothing is typed.
    let mut panel = opened(&fixture);
    assert_eq!(key(&mut panel, &fixture, q), Outcome::Dismiss);
    let mut harness = open_editor(ControlsEditor::Harness, &fixture);
    assert_eq!(
        edit(&mut harness, q),
        Outcome::Dismiss,
        "q dismisses the whole overlay, not just the checklist"
    );
    // The max-age editor takes text: q types; Ctrl-C still dismisses.
    let mut max_age = open_editor(ControlsEditor::MaxAge, &fixture);
    assert_eq!(edit(&mut max_age, q), Outcome::Consumed);
    assert_eq!(edit(&mut max_age, ctrl('c')), Outcome::Dismiss);
    assert_eq!(key(&mut panel, &fixture, ctrl('c')), Outcome::Dismiss);
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
    for view in VIEW_OPTIONS {
        let fixture = Fixture::new(*view);
        let rows = flatten_rows(&fixture.context());
        let wrap_rows = rows
            .iter()
            .filter(|row| matches!(row, ControlsCursor::PreviewWrap(_)))
            .count();
        assert_eq!(wrap_rows, crate::tui::WrapMode::ALL.len(), "{view:?}");

        let mut panel = panel_at(ControlsCursor::PreviewWrap(2));
        assert_eq!(
            key(&mut panel, &fixture, press(KeyCode::Char(' '))),
            Outcome::Stay(Msg::SetPreviewWrap(crate::tui::WrapMode::ALL[2]))
        );
    }
}

#[test]
fn session_preview_section_closes_the_overlay_and_applies_the_pick() {
    for view in VIEW_OPTIONS {
        let fixture = Fixture::new(*view);
        let rows = flatten_rows(&fixture.context());
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

        let mut panel = panel_at(ControlsCursor::SessionPreview(1));
        assert_eq!(
            key(&mut panel, &fixture, press(KeyCode::Char(' '))),
            Outcome::Stay(Msg::SetSessionPreview(crate::tui::SessionPreview::Pane))
        );
    }
}

#[test]
fn the_cursor_row_is_marked_and_highlighted_in_every_section() {
    let theme = Theme::default();
    for view in VIEW_OPTIONS {
        let fixture = Fixture::new(*view);
        for row in flatten_rows(&fixture.context()) {
            let mut panel = panel_at(row);
            let buf = drawn(&mut panel, &fixture, (100, 60), true);
            let marked = marked(&buf);
            assert_eq!(marked.len(), 1, "{view:?} {row:?}: {:?}", rows(&buf));
            let pointer = style_at(&buf, marks::POINTER).expect("the pointer");
            assert!(
                pointer.add_modifier.contains(theme.selection_active),
                "{view:?} {row:?}: the row is highlighted from its gutter"
            );
        }
    }
}

#[test]
fn every_kind_of_row_reads_as_it_did() {
    let fixture = Fixture::new(View::Prs).filter(RowFilter {
        float_tracked_prs_top: true,
        ..RowFilter::default()
    });
    let mut panel = opened(&fixture);
    let buf = drawn(&mut panel, &fixture, (100, 60), true);
    let drawn = rows(&buf).join("\n");
    for expected in [
        "│View ",
        &format!("│{} PRs [3]  active ", marks::POINTER),
        "│  Sessions [1] ",
        "│Grouping (PRs) ",
        "│  contents     Enter searches  [ Edit ] ",
        "│  options      Enter opens  [ Edit ] ",
        "│Filters (PRs) ",
        &format!("│  harness      {}  [ Edit ] ", marks::NO_VALUE),
        &format!("│  PR state     {}  [ Edit ] ", marks::NO_VALUE),
        "│  local branch any  [ Cycle ] ",
        "│  [x] Float PRs with a local branch to top ",
        "│  [ ] Pinned only ",
        "│  Clear all filters ",
        "│Sort ",
        "│  Hierarchy  active ",
        "│Session preview ",
        "changes apply as you make them",
    ] {
        assert!(drawn.contains(expected), "`{expected}` in\n{drawn}");
    }
    let heading = style_at(&buf, "Filters (PRs)").expect("a heading");
    assert!(heading.add_modifier.contains(Modifier::BOLD));
    let checked = style_at(&buf, "[x] Float").expect("a checked box");
    assert!(checked.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn pr_branch_rows_only_appear_on_the_prs_view() {
    for view in [
        View::Sessions,
        View::Mux,
        View::Pinned,
        View::History,
        View::Search,
    ] {
        let rows = flatten_rows(&Fixture::new(view).context());
        assert!(!rows.contains(&ControlsCursor::FilterPrBranch));
        assert!(!rows.contains(&ControlsCursor::FilterFloatTrackedPrs));
    }
    let rows = flatten_rows(&Fixture::new(View::Prs).context());
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

#[test]
fn space_on_local_branch_row_cycles_any_tracked_untracked() {
    use crate::filter::PrBranchFilter;

    let mut filter = RowFilter::default();
    let mut seen = Vec::new();
    for _ in 0..3 {
        let fixture = Fixture::new(View::Prs).filter(filter);
        let mut panel = at_filters();
        down(&mut panel, &fixture, 5);
        assert_eq!(cursor(&panel), ControlsCursor::FilterPrBranch);
        let next = applied_filter(key(&mut panel, &fixture, press(KeyCode::Char(' '))));
        seen.push(next.pr_branch);
        filter = next;
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
    let fixture = Fixture::new(View::Prs);
    let mut panel = at_filters();
    down(&mut panel, &fixture, 6);
    assert_eq!(cursor(&panel), ControlsCursor::FilterFloatTrackedPrs);
    let next = applied_filter(key(&mut panel, &fixture, press(KeyCode::Char(' '))));
    assert!(next.float_tracked_prs_top);
    assert_eq!(next.pr_branch, None);
}

#[test]
fn clear_all_resets_the_pr_settings() {
    let fixture = Fixture::new(View::Prs).filter(RowFilter {
        pr_branch: Some(crate::filter::PrBranchFilter::Untracked),
        float_tracked_prs_top: true,
        ..RowFilter::default()
    });
    let mut panel = at_filters();
    down(&mut panel, &fixture, 8);
    assert_eq!(cursor(&panel), ControlsCursor::FilterClear);
    assert_eq!(
        key(&mut panel, &fixture, press(KeyCode::Enter)),
        Outcome::Stay(Msg::ClearFilters)
    );
}

#[test]
fn pr_state_row_only_appears_on_the_prs_view() {
    for view in [
        View::Sessions,
        View::Mux,
        View::Pinned,
        View::History,
        View::Search,
    ] {
        let rows = flatten_rows(&Fixture::new(view).context());
        assert!(!rows.contains(&ControlsCursor::FilterPrState));
    }
    assert!(
        flatten_rows(&Fixture::new(View::Prs).context()).contains(&ControlsCursor::FilterPrState)
    );
}

#[test]
fn pr_state_sub_editor_commits_the_chosen_states() {
    use crate::filter::{PrStateFilter, PrStateKey};

    let fixture = Fixture::new(View::Prs);
    let mut panel = at_filters();
    down(&mut panel, &fixture, 4);
    assert_eq!(cursor(&panel), ControlsCursor::FilterPrState);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Stay(Msg::OpenControlsEditor(ControlsEditor::PrState))
    );
    let mut editor = editor_for(outcome, &fixture);
    assert!(matches!(editor, Modal::Checklist(_)));

    // Options are open, draft, merged, closed: pick open and merged.
    edit(&mut editor, press(KeyCode::Char(' ')));
    edit(&mut editor, press(KeyCode::Down));
    edit(&mut editor, press(KeyCode::Down));
    edit(&mut editor, press(KeyCode::Char(' ')));
    let next = set_filter(edit(&mut editor, press(KeyCode::Enter)));
    assert_eq!(
        next.pr_state,
        Some(PrStateFilter::from_values([
            PrStateKey::Open,
            PrStateKey::Merged
        ]))
    );
}

#[test]
fn pr_state_sub_editor_opens_with_the_current_states_selected_and_clears_when_empty() {
    use crate::filter::{PrStateFilter, PrStateKey};

    let fixture = Fixture::new(View::Prs).filter(RowFilter {
        pr_state: Some(PrStateFilter::from_values([PrStateKey::Open])),
        ..RowFilter::default()
    });
    let mut editor = open_editor(ControlsEditor::PrState, &fixture);
    // Un-pick the preselected open state and confirm: no constraint.
    edit(&mut editor, press(KeyCode::Char(' ')));
    let next = set_filter(edit(&mut editor, press(KeyCode::Enter)));
    assert_eq!(next.pr_state, None);
}

fn on_content_row(fixture: &Fixture) -> Panel {
    let mut panel = at_filters();
    down(&mut panel, fixture, 3);
    assert_eq!(cursor(&panel), ControlsCursor::FilterContent);
    panel
}

#[test]
fn the_content_row_opens_an_editor_and_starts_a_search_on_enter() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = on_content_row(&fixture);
    let outcome = key(&mut panel, &fixture, press(KeyCode::Enter));
    assert_eq!(
        outcome,
        Outcome::Stay(Msg::OpenControlsEditor(ControlsEditor::Content)),
        "Enter opens the editor"
    );
    let mut editor = editor_for(outcome, &fixture);
    type_into(&mut editor, " needle ");
    assert_eq!(
        edit(&mut editor, press(KeyCode::Enter)),
        Outcome::Back(Msg::StartContentSearch {
            text: "needle".into(),
            kinds: crate::viewer::search::ContentKinds::default(),
        })
    );
}

#[test]
fn space_opens_the_content_editor_too() {
    let fixture = Fixture::new(View::Sessions);
    let mut panel = on_content_row(&fixture);
    assert_eq!(
        key(&mut panel, &fixture, press(KeyCode::Char(' '))),
        Outcome::Stay(Msg::OpenControlsEditor(ControlsEditor::Content))
    );
}

#[test]
fn the_content_editor_starts_with_the_active_query_and_empty_text_clears_it() {
    let fixture = Fixture::new(View::Sessions).searching("needle");
    let mut editor = open_editor(ControlsEditor::Content, &fixture);
    for _ in 0.."needle".len() {
        edit(&mut editor, press(KeyCode::Backspace));
    }
    assert_eq!(
        edit(&mut editor, press(KeyCode::Enter)),
        Outcome::Back(Msg::ClearContentSearch)
    );
}

#[test]
fn escape_in_the_content_editor_keeps_the_search_as_it_was() {
    let fixture = Fixture::new(View::Sessions).searching("needle");
    let mut editor = open_editor(ControlsEditor::Content, &fixture);
    edit(&mut editor, press(KeyCode::Char('x')));
    assert_eq!(edit(&mut editor, press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn clear_all_has_something_to_clear_when_only_a_search_is_active() {
    let fixture = Fixture::new(View::Sessions).searching("needle");
    let mut panel = at_filters();
    down(&mut panel, &fixture, 6);
    assert_eq!(cursor(&panel), ControlsCursor::FilterClear);
    assert_eq!(
        key(&mut panel, &fixture, press(KeyCode::Enter)),
        Outcome::Stay(Msg::ClearFilters)
    );
}
