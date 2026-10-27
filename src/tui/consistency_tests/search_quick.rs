//! Quick select: `/` finds a row of the current view, Tab widens it to
//! every entity, and opening a result does what its outcome word says.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::layout::{search_key, type_into_search};
use super::*;
use crate::model::{
    Confidence, Freshness, GraphLink, LinkEndpoint, PinId, Provenance, RelationKind,
};
use crate::tui::app::{Focus, OpenPlan};
use crate::tui::explorer::ExplorerRow;

fn link(id: &str, source: &NodeId, target: &NodeId, relation: RelationKind) -> GraphLink {
    let mut link = GraphLink::new(
        id,
        source.clone(),
        LinkEndpoint::Node { id: target.clone() },
        relation,
        Provenance::StrongDiscovered,
    );
    link.confidence = Confidence::High;
    link.freshness = Freshness::Fresh;
    link
}

fn pr(number: u64) -> GraphNode {
    GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", "acme", "app", number),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "acme".into(),
        repo: "app".into(),
        number,
        state: Some("open".into()),
        title: Some(format!("Change number {number}")),
        url: None,
        updated_epoch: Some(NOW - 60),
        is_draft: false,
        head_ref: None,
    })
}

fn pr_id(number: u64) -> NodeId {
    pr(number).id()
}

fn session_id() -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "s1"))
}

fn mux_id() -> NodeId {
    NodeId::MuxSession(MuxSessionId::new("tmux:work"))
}

/// A session linked to a mux, then a chain of pull requests:
/// session - mux - #7 - #8 - #9, with #99 related to nothing.
fn chain_world() -> GraphSnapshot {
    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "s1"),
        "claude-code",
    );
    session.title = Some("Fix the parser".into());
    session.cwd = Some("/home/op/src/app".into());
    session.last_active_epoch = Some(NOW - 30);
    let mut mux = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    mux.cwd = Some("/home/op/src/app".into());
    mux.backend_session_id = Some("$1".into());
    mux.created_epoch = Some(NOW - 1_000);
    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(session),
            GraphNode::MuxSession(mux),
            pr(7),
            pr(8),
            pr(9),
            pr(99),
        ],
        candidate_links: vec![
            link("l1", &session_id(), &mux_id(), RelationKind::LinkedToMux),
            link("l2", &mux_id(), &pr_id(7), RelationKind::AssociatedWith),
            link("l3", &pr_id(7), &pr_id(8), RelationKind::AssociatedWith),
            link("l4", &pr_id(8), &pr_id(9), RelationKind::AssociatedWith),
        ],
        ..GraphSnapshot::empty()
    };
    resolve_snapshot(snapshot)
}

/// The Sessions view over the chain, with the session selected.
fn app_on_session() -> App {
    let mut app = app_over(View::Sessions, &chain_world());
    select_node(&mut app, &session_id());
    app
}

fn select_node(app: &mut App, node: &NodeId) {
    let id = app
        .tree()
        .rows
        .iter()
        .find(|row| crate::tui::rows::row_node(&row.id) == Some(node))
        .unwrap_or_else(|| panic!("no row for {node}"))
        .id
        .clone();
    app.set_selection(id);
}

fn plan(app: &App, node: &NodeId) -> OpenPlan {
    let reach = app.search_reachability();
    app.open_plan(reach.as_ref(), node)
}

fn selected_node(app: &App) -> Option<NodeId> {
    app.selection()
        .and_then(|id| crate::tui::rows::row_node(id).cloned())
}

#[test]
fn the_outcome_word_says_what_enter_will_do() {
    let app = app_on_session();
    let words: Vec<(String, String)> = [
        ("the selected row", session_id()),
        ("one hop", mux_id()),
        ("two hops", pr_id(7)),
        ("three hops", pr_id(8)),
        ("four hops", pr_id(9)),
        ("unrelated", pr_id(99)),
    ]
    .into_iter()
    .map(|(what, node)| (what.to_string(), plan(&app, &node).word()))
    .collect();
    let expected = [
        ("the selected row", "here"),
        ("one hop", "via mux"),
        ("two hops", "via mux"),
        ("three hops", "3 hops"),
        ("four hops", "in PRs view"),
        ("unrelated", "in PRs view"),
    ];
    for ((what, word), (expected_what, expected_word)) in words.iter().zip(expected) {
        assert_eq!(
            (what.as_str(), word.as_str()),
            (expected_what, expected_word)
        );
    }
}

#[test]
fn a_row_of_the_current_view_opens_by_selecting_it_and_keeps_the_focus() {
    let mut app = app_on_session();
    assert_eq!(app.focus(), Focus::Left);

    app.update(Msg::OpenEntity(session_id()));

    assert_eq!(selected_node(&app), Some(session_id()));
    assert_eq!(app.focus(), Focus::Left);
}

#[test]
fn a_neighbor_opens_in_the_right_pane_with_the_cursor_on_its_row() {
    let mut app = app_on_session();

    app.update(Msg::OpenEntity(mux_id()));

    assert_eq!(app.focus(), Focus::Right);
    assert_eq!(app.active_view(), View::Sessions, "the left pane stays put");
    let state = app.explorer().expect("an explorer");
    assert_eq!(state.view.focused, session_id(), "no drill for one hop");
    assert!(state.breadcrumb.is_empty());
    let row = state.selected_row().expect("a cursor row");
    assert!(matches!(row, ExplorerRow::ValidatedLink { .. }), "{row:?}");
    assert_eq!(state.view.drill_target(&row), Some(mux_id()));
}

#[test]
fn a_farther_entity_drills_along_the_path_and_backspace_retraces_it() {
    let mut app = app_on_session();

    app.update(Msg::OpenEntity(pr_id(8)));

    assert_eq!(app.focus(), Focus::Right);
    let state = app.explorer().expect("an explorer");
    assert_eq!(
        state.view.focused,
        pr_id(7),
        "the last hop before the target"
    );
    assert_eq!(state.breadcrumb.len(), 2, "one hop per drill");
    let row = state.selected_row().expect("a cursor row");
    assert_eq!(state.view.drill_target(&row), Some(pr_id(8)));

    app.update(Msg::ExplorerBack);
    app.update(Msg::ExplorerBack);
    let state = app.explorer().expect("an explorer");
    assert_eq!(state.view.focused, session_id(), "back where it started");
    assert!(state.breadcrumb.is_empty());
}

#[test]
fn an_unrelated_entity_opens_in_the_view_that_lists_it() {
    let mut app = app_on_session();

    app.update(Msg::OpenEntity(pr_id(99)));

    assert_eq!(app.active_view(), View::Prs);
    assert_eq!(selected_node(&app), Some(pr_id(99)));
}

#[test]
fn an_entity_past_three_hops_opens_in_its_view_too() {
    let mut app = app_on_session();

    app.update(Msg::OpenEntity(pr_id(9)));

    assert_eq!(app.active_view(), View::Prs);
    assert_eq!(selected_node(&app), Some(pr_id(9)));
}

#[test]
fn an_entity_that_cannot_be_listed_leaves_the_operator_where_they_were() {
    let mut app = app_on_session();
    let before = app.selection().cloned();

    app.update(Msg::OpenEntity(NodeId::Pin(PinId::new("no-such-pin"))));

    assert_eq!(app.active_view(), View::Sessions);
    assert_eq!(app.selection().cloned(), before);
    let status = app.status_message().expect("a toast");
    assert!(status.contains("isn't listed"), "{status}");
}

#[test]
fn the_search_start_is_where_the_right_pane_is_focused() {
    let mut app = app_on_session();
    app.update(Msg::OpenEntity(pr_id(7)));
    // The pane is now drilled into the mux; #8 is two hops from the
    // session but one from #7, and `via` names that first hop.
    assert_eq!(plan(&app, &pr_id(7)), OpenPlan::Path(vec![pr_id(7)]));
    assert_eq!(plan(&app, &session_id()), OpenPlan::Here);
}

/// Collapse every group so only the top rows show.
pub(super) fn collapse_groups(app: &mut App) {
    let groups: Vec<RowId> = app
        .tree()
        .rows
        .iter()
        .filter(|row| row.expandable)
        .map(|row| row.id.clone())
        .collect();
    for id in groups.iter().rev() {
        app.set_selection(id.clone());
        app.update(Msg::CollapseRow);
    }
}

#[test]
fn current_reach_finds_and_selects_rows_under_collapsed_groups() {
    let mut app = app_over(View::Sessions, &world());
    collapse_groups(&mut app);
    let hidden = app
        .tree()
        .rows
        .iter()
        .find(|row| {
            matches!(row.kind, crate::tui::rows::RowKind::AgentSession(_))
                && !app
                    .visible_rows()
                    .iter()
                    .any(|visible| visible.id == row.id)
        })
        .expect("a session row hidden under a collapsed group")
        .clone();
    let label = items_label(&hidden);

    app.open_search_overlay();
    type_into_search(&mut app, &label);
    search_key(&mut app, KeyCode::Enter);

    assert!(app.search_overlay().is_none(), "Enter closes the overlay");
    assert_eq!(app.selection(), Some(&hidden.id));
    assert!(
        app.visible_rows().iter().any(|row| row.id == hidden.id),
        "its groups were expanded"
    );
}

fn items_label(row: &crate::tui::rows::Row) -> String {
    crate::tui::search::items_from_rows([row])
        .remove(0)
        .label
        .to_string()
}

#[test]
fn tab_then_a_name_then_enter_opens_an_entity_from_another_view() {
    let mut app = app_on_session();

    app.open_search_overlay();
    search_key(&mut app, KeyCode::Tab);
    type_into_search(&mut app, "Change number 99");
    search_key(&mut app, KeyCode::Enter);

    assert!(app.search_overlay().is_none());
    assert_eq!(app.active_view(), View::Prs);
    assert_eq!(selected_node(&app), Some(pr_id(99)));
}

#[test]
fn the_search_hint_names_the_tab_keys() {
    let mut app = app_on_session();
    let hint = app.open_search_entry();
    assert!(hint.contains("Tab current/all"), "{hint}");
    assert!(!hint.contains("search contents"), "{hint}");
}

#[test]
fn shift_tab_widens_too() {
    let mut app = app_on_session();
    app.open_search_overlay();
    crate::tui::runtime::handle_search_overlay_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    );
    assert_eq!(
        app.search_overlay()
            .map(crate::tui::widgets::search::SearchOverlayState::reach),
        Some(crate::tui::widgets::search::Reach::All)
    );
}

/// 30 muxes named `alpha-NN`, so a query matches more rows than one
/// screen of the modal shows.
fn many_muxes() -> GraphSnapshot {
    let nodes = (0..30)
        .map(|n| {
            let name = format!("alpha-{n:02}");
            let mut mux = crate::model::MuxSessionNode::new(
                crate::model::MuxSessionId::new(format!("tmux:{name}")),
                "tmux",
                name,
            );
            mux.cwd = Some("/home/op/src/alpha".into());
            GraphNode::MuxSession(mux)
        })
        .collect();
    GraphSnapshot {
        nodes,
        ..GraphSnapshot::empty()
    }
}

fn cursor(app: &App) -> usize {
    app.search_overlay().expect("the overlay is open").cursor()
}

fn ctrl(app: &mut App, c: char) {
    crate::tui::runtime::handle_search_overlay_key(
        app,
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL),
    );
}

/// Step the cursor down 8 and back up 8, with every key the table binds
/// for it, and expect it to follow each press.
fn walk_the_cursor(app: &mut App) {
    for expected in 1..=8 {
        search_key(app, KeyCode::Down);
        assert_eq!(cursor(app), expected, "Down #{expected}");
    }
    for expected in (0..8).rev() {
        search_key(app, KeyCode::Up);
        assert_eq!(cursor(app), expected, "Up to {expected}");
    }
    for expected in 1..=5 {
        ctrl(app, 'n');
        assert_eq!(cursor(app), expected, "Ctrl-N #{expected}");
    }
    for expected in (0..5).rev() {
        ctrl(app, 'p');
        assert_eq!(cursor(app), expected, "Ctrl-P to {expected}");
    }
}

#[test]
fn the_cursor_follows_every_arrow_key_in_current_scope() {
    let mut app = app_over(View::Mux, &many_muxes());
    app.open_search_overlay();
    type_into_search(&mut app, "alpha");
    walk_the_cursor(&mut app);
}

#[test]
fn the_cursor_follows_every_arrow_key_in_all_scope() {
    let mut app = app_over(View::Mux, &many_muxes());
    app.open_search_overlay();
    search_key(&mut app, KeyCode::Tab);
    type_into_search(&mut app, "alpha");
    walk_the_cursor(&mut app);
}

#[test]
fn the_cursor_survives_typing_after_moving() {
    let mut app = app_over(View::Mux, &many_muxes());
    app.open_search_overlay();
    type_into_search(&mut app, "alpha");
    for _ in 0..4 {
        search_key(&mut app, KeyCode::Down);
    }
    assert_eq!(cursor(&app), 4);
    // A longer query keeps the 30 matches, so the cursor stays put.
    type_into_search(&mut app, "-");
    assert_eq!(
        cursor(&app),
        4,
        "typing a matching character keeps the cursor"
    );
}

#[test]
fn the_highlighted_row_scrolls_into_view_past_the_first_screen() {
    for reach_all in [false, true] {
        let mut app = app_over(View::Mux, &many_muxes());
        app.open_search_overlay();
        if reach_all {
            search_key(&mut app, KeyCode::Tab);
        }
        type_into_search(&mut app, "alpha");
        for _ in 0..25 {
            search_key(&mut app, KeyCode::Down);
        }
        assert_eq!(cursor(&app), 25);
        let shown = frame(&mut app, 120, 24);
        let marked = shown
            .lines()
            .find(|line| line.contains("│> "))
            .unwrap_or_else(|| panic!("a marked row in:\n{shown}"));
        assert!(
            marked.contains("alpha-25"),
            "the marker sits on the cursor's row (all={reach_all}): {marked}"
        );
    }
}

/// Quick select on the current view finds and selects any row the view
/// lists, whatever its kind.
fn quick_select_picks_every_row_of(view: View) {
    let mut app = app_listing_all(view);
    let rows: Vec<_> = app
        .tree()
        .rows
        .iter()
        .filter(|row| !matches!(row.kind, crate::tui::rows::RowKind::Group(_)))
        .map(|row| row.id.clone())
        .collect();
    assert!(!rows.is_empty(), "the {view:?} world lists rows");
    for id in rows {
        let row = app.tree().rows.iter().find(|row| row.id == id).unwrap();
        let label = items_label(row);
        app.open_search_overlay();
        type_into_search(&mut app, &label);
        search_key(&mut app, KeyCode::Enter);
        assert!(app.search_overlay().is_none(), "Enter closes the overlay");
        assert!(
            app.selection().is_some_and(|sel| {
                let picked = app.tree().rows.iter().find(|r| r.id == *sel).unwrap();
                items_label(picked) == label
            }),
            "{view:?}: `{label}` is selected"
        );
    }
}

#[test]
fn quick_select_picks_rows_of_the_pinned_view() {
    quick_select_picks_every_row_of(View::Pinned);
}

#[test]
fn quick_select_picks_rows_of_the_history_view() {
    quick_select_picks_every_row_of(View::History);
}
