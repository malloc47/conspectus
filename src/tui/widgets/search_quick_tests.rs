use super::rows::*;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, ForgePrId, ForgePrNode,
    GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeKind, NodePresence, PinKind,
    PinNode, RepoId, RepoNode, SlotStatus,
};
use crate::tui::search::items_from_rows;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::text::Span;
use tuikit::testkit::keys::{chord, press};

fn type_text(state: &mut SearchOverlayState, text: &str) {
    for c in text.chars() {
        state.handle_key(press(KeyCode::Char(c)));
    }
}

fn world() -> GraphSnapshot {
    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "abc123"),
        "claude-code",
    );
    session.title = Some("Quokka parser rewrite".into());
    session.cwd = Some("/home/op/src/quokka".into());
    session.last_active_epoch = Some(100);

    let mut mux = MuxSessionNode::new(MuxSessionId::new("tmux:quokka-dev"), "tmux", "quokka-dev");
    mux.cwd = Some("/home/op/src/quokka".into());
    let gone = MuxSessionNode::new(MuxSessionId::new("tmux:quokka-old"), "tmux", "quokka-old");

    let repo_id = RepoId::new("/home/op/src/quokka/.git");
    let mut repo = RepoNode::new(repo_id.clone());
    repo.remotes = vec!["git@github.com:acme/quokka.git".into()];
    let mut checkout = CheckoutNode::new(
        CheckoutId::new(repo_id.clone(), "/home/op/src/quokka-wt"),
        "/home/op/src/quokka-wt",
    );
    checkout.current_branch = Some(BranchId::new(repo_id, "refs/heads/quokka-login"));
    let pr = ForgePrNode {
        id: ForgePrId::new("github", "github.com", "acme", "quokka", 7),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "acme".into(),
        repo: "quokka".into(),
        number: 7,
        state: Some("open".into()),
        title: Some("Add the quokka login form".into()),
        url: None,
        updated_epoch: Some(50),
        is_draft: false,
        head_ref: Some("quokka-login".into()),
    };
    let pin = PinNode::for_test("quokka-daily", PinKind::Mux, None, SlotStatus::Planned, &[]);

    let gone_id = GraphNode::MuxSession(gone.clone()).id();
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(session),
            GraphNode::MuxSession(mux),
            GraphNode::MuxSession(gone),
            GraphNode::Repo(repo),
            GraphNode::Checkout(checkout),
            GraphNode::ForgePr(pr),
            GraphNode::Pin(pin),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot
        .node_presence
        .insert(gone_id, NodePresence::Gone { last_seen: 10 });
    snapshot
}

/// Quick select on the wider reach with `query` typed.
fn wide_state(query: &str) -> SearchOverlayState {
    let mut state = SearchOverlayState::new();
    state.set_entities(EntityIndex::build(&world(), true));
    state.handle_key(press(KeyCode::Tab));
    type_text(&mut state, query);
    state.refresh_matches(&[]);
    state
}

/// What the host would say for each kind, so frames show every word.
fn words(node: &NodeId) -> String {
    if node.to_string().contains("quokka-old") {
        return "in History view".into();
    }
    match NodeKind::from(node) {
        NodeKind::AgentSession => "here".into(),
        NodeKind::MuxSession => "via repo".into(),
        NodeKind::Pin => "3 hops".into(),
        NodeKind::ForgePr => "in PRs view".into(),
        _ => "in History view".into(),
    }
}

fn render_with(
    state: &SearchOverlayState,
    items: &[SearchItem<'_>],
    width: u16,
    height: u16,
) -> String {
    let theme = Theme::default();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let mut state = state.clone();
    SearchOverlayWidget::new(&mut state, items, &theme)
        .with_outcomes(&words)
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

/// The reach strip as the palette's title draws it.
fn reach_segments(reach: Reach, theme: &Theme) -> Vec<Span<'static>> {
    let segments: Vec<(&str, bool)> = REACHES
        .iter()
        .enumerate()
        .map(|(choice, word)| (*word, choice == reach.choice()))
        .collect();
    tuikit::chip::SegmentedTitle::new(&segments).spans(theme)
}

fn current_items() -> Vec<crate::tui::rows::Row> {
    use crate::tui::rows::{AgentSessionRow, MuxIndicator, Row, RowId, RowKind};
    [
        ("api1", "quokka api rewrite", "~/src/quokka"),
        ("dev2", "quokka dev server", "~/src/quokka"),
        ("else", "something else", "~/src/other"),
    ]
    .into_iter()
    .map(|(key, alias, cwd)| {
        let session = AgentSessionId::new("claude-code", "/state", key);
        let primary = NodeId::AgentSession(session.clone());
        Row {
            id: RowId::AgentSession(primary.clone()),
            depth: 1,
            expandable: false,
            kind: RowKind::AgentSession(AgentSessionRow {
                session,
                short_id: key.to_string(),
                harness_label: "claude-code".to_string(),
                cwd_display: Some(cwd.to_string()),
                project_display: None,
                recency: None,
                activity_epoch: None,
                mux_state: MuxIndicator::Unmuxed,
                preview: Some(format!("talking about {alias}")),
                title: None,
                alias: Some(alias.to_string()),
                title_disambiguates: false,
                mux_host: None,
                primary_node: primary,
                pin_id: None,
                content_hit: None,
            }),
        }
    })
    .collect()
}

#[test]
fn quick_select_opens_on_the_current_reach() {
    let state = SearchOverlayState::new();
    assert_eq!(state.mode(), SearchMode::Quick);
    assert_eq!(state.reach(), Reach::Current);
}

#[test]
fn tab_and_shift_tab_both_switch_the_reach() {
    let mut state = SearchOverlayState::new();
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.reach(), Reach::All);
    state.handle_key(press(KeyCode::Tab));
    assert_eq!(state.reach(), Reach::Current);
    state.handle_key(chord(KeyModifiers::SHIFT, KeyCode::BackTab));
    assert_eq!(state.reach(), Reach::All);
    state.handle_key(chord(KeyModifiers::SHIFT, KeyCode::BackTab));
    assert_eq!(state.reach(), Reach::Current);
}

#[test]
fn widening_keeps_the_query_and_goes_back_to_the_top() {
    let mut state = SearchOverlayState::new();
    state.set_entities(EntityIndex::build(&world(), true));
    type_text(&mut state, "quokka");
    state.handle_key(press(KeyCode::Tab));
    state.refresh_matches(&[]);
    assert_eq!(state.query(), "quokka");
    assert_eq!(state.cursor(), 0);
    assert!(
        state.entity_matches().len() >= 5,
        "{:?}",
        state.entity_matches()
    );
}

#[test]
fn the_host_is_asked_for_the_index_only_on_the_first_widening() {
    let mut state = SearchOverlayState::new();
    assert!(state.widens_with(press(KeyCode::Tab)));
    assert!(state.widens_with(chord(KeyModifiers::SHIFT, KeyCode::BackTab)));
    assert!(!state.widens_with(press(KeyCode::Char('a'))));
    state.set_entities(EntityIndex::build(&world(), true));
    assert!(!state.widens_with(press(KeyCode::Tab)), "already built");

    let content = SearchOverlayState::configured(
        SearchMode::Content,
        Grouping::Session,
        ContentKinds::default(),
        true,
        "",
    );
    assert!(
        !content.widens_with(press(KeyCode::Tab)),
        "not in content mode"
    );
}

#[test]
fn enter_on_a_wider_result_opens_that_entity() {
    let mut state = wide_state("quokka-dev");
    let outcome = state.handle_key(press(KeyCode::Enter));
    let expected = NodeId::MuxSession(MuxSessionId::new("tmux:quokka-dev"));
    assert_eq!(outcome, SearchOutcome::OpenEntity(Box::new(expected)));
}

#[test]
fn enter_with_no_wider_result_closes() {
    let mut state = wide_state("zzz-nothing");
    assert_eq!(
        state.handle_key(press(KeyCode::Enter)),
        SearchOutcome::Cancel
    );
}

#[test]
fn the_cursor_moves_among_the_wider_results() {
    let mut state = wide_state("quokka");
    let n = state.entity_matches().len();
    assert!(n >= 3);
    state.handle_key(press(KeyCode::Down));
    assert_eq!(state.cursor(), 1);
    state.handle_key(press(KeyCode::Up));
    state.handle_key(press(KeyCode::Up));
    assert_eq!(state.cursor(), n - 1, "wraps");
}

#[test]
fn the_overlay_commits_an_open_entity_message() {
    use tuikit::overlay::Outcome;
    let mut state = wide_state("quokka-dev");
    let outcome = state.handle_in(
        SearchContext {
            items: &[],
            content: &[],
            entities: None,
            listing: None,
        },
        press(KeyCode::Enter),
    );
    let Outcome::Commit(msg) = outcome else {
        panic!("expected a commit, got {outcome:?}");
    };
    assert!(matches!(msg, crate::tui::Msg::OpenEntity(_)), "{msg:?}");
}

#[test]
fn the_wider_reach_ranks_everything_while_the_current_one_ranks_rows() {
    let rows = current_items();
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    state.set_entities(EntityIndex::build(&world(), true));
    type_text(&mut state, "quokka");
    state.refresh_matches(&items);
    assert_eq!(state.matches().len(), 2, "the two rows that say quokka");
    assert!(state.entity_matches().is_empty());

    state.handle_key(press(KeyCode::Tab));
    state.refresh_matches(&items);
    assert!(state.matches().is_empty());
    assert!(state.entity_matches().len() > 2);
}

#[test]
fn the_modal_follows_the_sizing_rule() {
    // The palette policy: width 85% of W within 60..=200, height 80% of H
    // within 12..=50, and never past the terminal.
    for (terminal, expected) in [
        ((60, 20), (60, 16)),
        ((80, 24), (68, 19)),
        ((120, 40), (102, 32)),
        ((160, 40), (136, 32)),
        ((220, 50), (187, 40)),
        ((300, 100), (200, 50)),
        ((50, 10), (50, 10)),
        ((220, 14), (187, 12)),
    ] {
        let rect = SEARCH_SIZE.rect(Rect::new(0, 0, terminal.0, terminal.1), 0);
        assert_eq!((rect.width, rect.height), expected, "{terminal:?}");
    }
}

#[test]
fn the_modal_is_centered_and_fits_every_terminal() {
    for width in [20_u16, 60, 80, 120, 160, 220, 400] {
        for height in [6_u16, 20, 40, 80] {
            let area = Rect::new(0, 0, width, height);
            let modal = SEARCH_SIZE.rect(area, 0);
            assert!(
                modal.right() <= width && modal.bottom() <= height,
                "{area:?} {modal:?}"
            );
            // An odd cell left over goes before the modal.
            assert_eq!(modal.x, (width - modal.width).div_ceil(2));
            assert_eq!(modal.y, (height - modal.height).div_ceil(2));
        }
    }
}

/// The split the entity rows had before their column schema: the
/// outcome word a third of the row (at most 16), the label 45% of what the
/// cursor mark, the glyph, the word, and the gap before it leave (16 to
/// 48, and no more than that), and the context the rest unless it gets
/// fewer than 10 cells, when the label takes them.
fn split_by_hand(inner: usize) -> EntityColumns {
    let outcome = OUTCOME_WIDTH.min(inner / 3);
    let body = inner.saturating_sub(4 + outcome + 2);
    let label = (body * 45 / 100).clamp(16, 48).min(body);
    let context = body.saturating_sub(label + 2);
    if context < 10 {
        EntityColumns {
            label: body,
            context: 0,
            outcome,
        }
    } else {
        EntityColumns {
            label,
            context,
            outcome,
        }
    }
}

#[test]
fn entity_columns_always_fit_and_drop_the_context_before_the_outcome() {
    for inner in [30_usize, 40, 56, 78, 118, 158, 218] {
        let columns = entity_widths(inner);
        let context_gap = if columns.context > 0 { 2 } else { 0 };
        let used = 4 + columns.label + context_gap + columns.context + 2 + columns.outcome;
        assert!(used <= inner, "{inner}: {columns:?}");
        assert!(columns.outcome > 0, "{inner}: the outcome word stays");
    }
    assert_eq!(entity_widths(40).context, 0, "narrow rows lose the context");
    assert!(entity_widths(158).context > 40, "wide rows show it");
    assert_eq!(entity_widths(158).outcome, OUTCOME_WIDTH);
}

#[test]
fn the_entity_schema_splits_a_row_as_the_hand_split_did_at_every_width() {
    for inner in 8..=260 {
        assert_eq!(entity_widths(inner), split_by_hand(inner), "{inner} cells");
    }
}

#[test]
fn the_title_names_both_reaches_and_marks_the_chosen_one() {
    let theme = Theme::default();
    let words = |reach| {
        reach_segments(reach, &theme)
            .iter()
            .map(|span| span.content.to_string())
            .collect::<String>()
    };
    assert_eq!(words(Reach::Current), "[ Current ] All");
    assert_eq!(words(Reach::All), "Current [ All ]");
    assert_eq!(
        words(Reach::Current).len(),
        words(Reach::All).len(),
        "no layout shift"
    );
}

#[test]
fn the_wider_frame_shows_the_outcome_word_whole_at_every_size() {
    let state = wide_state("quokka");
    for (width, height) in [(60, 20), (80, 24), (120, 36), (160, 40), (220, 50)] {
        let text = render_with(&state, &[], width, height);
        let modal = SEARCH_SIZE.rect(Rect::new(0, 0, width, height), 0);
        // An odd margin puts the extra cell before the frame, so find it.
        let left = text
            .lines()
            .find_map(|line| line.chars().position(|c| c == '┌'))
            .expect("the frame");
        for line in text.lines().skip(usize::from(modal.y) + 2) {
            let inner: String = line
                .chars()
                .skip(left + 1)
                .take(usize::from(modal.width) - 2)
                .collect();
            if let Some(word) = [
                "here",
                "via repo",
                "3 hops",
                "in PRs view",
                "in History view",
            ]
            .iter()
            .find(|word| inner.trim_end().ends_with(*word))
            {
                assert!(
                    inner.starts_with(&format!("{} ", tuikit::glyphs::marks::POINTER))
                        || inner.starts_with("  "),
                    "{width}x{height} modal {modal:?}: {inner:?}\n{text}"
                );
                assert!(
                    inner.trim_end().len() < inner.len()
                        || inner.chars().count() == usize::from(modal.width) - 2,
                    "{word} runs into the frame at {width}x{height}: {inner:?}"
                );
            }
        }
        assert!(text.contains("Current [ All ]"), "{width}x{height}\n{text}");
        assert!(
            text.contains("Tab search current/all"),
            "{width}x{height}\n{text}"
        );
        let results = text.matches("via repo").count()
            + text.matches("3 hops").count()
            + text.matches("in PRs view").count()
            + text.matches("in History view").count()
            + text.matches("here").count();
        assert!(
            results >= 3,
            "{width}x{height}: outcome words missing\n{text}"
        );
    }
}

#[test]
fn a_gone_entity_is_listed_with_its_history_word() {
    let text = render_with(&wide_state("quokka-old"), &[], 160, 30);
    assert!(text.contains("quokka-old"), "{text}");
    assert!(text.contains("in History view"), "{text}");
}

#[test]
fn snapshot_current_at_80_columns() {
    let rows = current_items();
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    type_text(&mut state, "quokka");
    state.refresh_matches(&items);
    insta::assert_snapshot!(
        "quick_select_current_80",
        render_with(&state, &items, 80, 24)
    );
}

#[test]
fn snapshot_current_at_160_columns() {
    let rows = current_items();
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    type_text(&mut state, "quokka");
    state.refresh_matches(&items);
    insta::assert_snapshot!(
        "quick_select_current_160",
        render_with(&state, &items, 160, 30)
    );
}

#[test]
fn snapshot_current_at_220_columns() {
    let rows = current_items();
    let items = items_from_rows(&rows);
    let mut state = SearchOverlayState::new();
    type_text(&mut state, "quokka");
    state.refresh_matches(&items);
    insta::assert_snapshot!(
        "quick_select_current_220",
        render_with(&state, &items, 220, 30)
    );
}

#[test]
fn snapshot_all_at_80_columns() {
    insta::assert_snapshot!(
        "quick_select_all_80",
        render_with(&wide_state("quokka"), &[], 80, 24)
    );
}

#[test]
fn snapshot_all_at_160_columns() {
    insta::assert_snapshot!(
        "quick_select_all_160",
        render_with(&wide_state("quokka"), &[], 160, 30)
    );
}

#[test]
fn snapshot_all_at_220_columns() {
    insta::assert_snapshot!(
        "quick_select_all_220",
        render_with(&wide_state("quokka"), &[], 220, 30)
    );
}

#[test]
fn the_query_line_draws_its_caret_and_windows_by_cells() {
    use tuikit::testkit::{buffer, golden};
    let theme = Theme::default();
    let mut state = SearchOverlayState::new();
    type_text(&mut state, "東京 quokka");
    state.handle_key(press(KeyCode::Home));
    state.handle_key(press(KeyCode::Right));
    let rows = current_items();
    let items = items_from_rows(&rows);
    state.refresh_matches(&items);
    let area = Rect::new(0, 0, 60, 16);
    let mut buf = Buffer::empty(area);
    SearchOverlayWidget::new(&mut state, &items, &theme).render(area, &mut buf);
    let at = buffer::find(&buf, "/東").expect("the query line");
    let caret = &buf[(at.x + 3, at.y)];
    assert_eq!(caret.symbol(), "京", "the caret is on the second character");
    let cue = theme.field_cursor.into_style();
    assert_eq!((Some(caret.fg), Some(caret.bg)), (cue.fg, cue.bg));
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/tui/widgets/goldens/search-query.golden");
    let row = Rect::new(at.x, at.y, 16, 1);
    golden::assert_golden_text(&path, &golden::styled_region(&buf, row), |path, text| {
        std::fs::write(path, text)
    });
}

#[test]
fn a_long_query_scrolls_to_keep_the_caret_in_view() {
    let mut state = SearchOverlayState::new();
    let long = "a query far longer than the line it is typed into";
    type_text(&mut state, long);
    let screen = render_with(&state, &[], 40, 12);
    let query = screen
        .lines()
        .find(|line| line.contains("│/"))
        .expect("the query line");
    assert!(query.contains("is typed into"), "{query}");
    assert!(!query.contains("a query far"), "{query}");
}
