use std::collections::BTreeSet;

use super::*;
use crate::model::{AgentSessionId, RepoId, WorkspaceId};
use crate::tui::rows::GroupRow;
use crate::tui::runtime::{resumed_app, set_static_data};
use crate::tui_state::{PersistedState, PersistedViewSlot, TuiStateCache};

fn cache_in(dir: &tempfile::TempDir) -> TuiStateCache {
    TuiStateCache::new().with_xdg_state_home(dir.path())
}

fn ws(path: &str) -> RowId {
    RowId::Group(NodeId::Workspace(WorkspaceId::new(path)))
}

fn repo(path: &str) -> RowId {
    RowId::Group(NodeId::Repo(RepoId::new(path)))
}

fn session(key: &str) -> RowId {
    RowId::AgentSession(NodeId::AgentSession(AgentSessionId::new(
        "codex", "/state", key,
    )))
}

fn key(id: &RowId) -> String {
    row_key(id).expect("a remembered kind")
}

fn row(id: RowId, depth: u8, expandable: bool) -> Row {
    Row {
        id,
        depth,
        expandable,
        kind: RowKind::Group(GroupRow::named("row", None)),
    }
}

/// Two workspaces, the first with two repos:
///
/// ```text
/// 0 /a
/// 1   /a/x
/// 2     s1
/// 3     s2
/// 4   /a/y
/// 5     s3
/// 6 /b
/// 7   s4
/// ```
fn rows() -> RowTree {
    RowTree {
        rows: vec![
            row(ws("/a"), 0, true),
            row(repo("/a/x"), 1, true),
            row(session("s1"), 2, false),
            row(session("s2"), 2, false),
            row(repo("/a/y"), 1, true),
            row(session("s3"), 2, false),
            row(ws("/b"), 0, true),
            row(session("s4"), 1, false),
        ],
        view: crate::tui::rows::ViewLabel::Sessions,
    }
}

fn load(app: &mut App, tree: RowTree) {
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&GraphSnapshot::empty()),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
}

/// The state file a run leaves when the Sessions view had `expanded`
/// open and `selected` (with the rows above it) selected.
fn seed(cache: &TuiStateCache, expanded: Option<&[RowId]>, selected: Option<(RowId, &[RowId])>) {
    let tree = PersistedTree {
        expanded: expanded.map(|ids| ids.iter().map(key).collect()),
        selected: selected.map(|(row, path)| PersistedSelection {
            row: key(&row),
            path: path.iter().map(key).collect(),
        }),
    };
    let mut state = PersistedState::default();
    state.view_states.insert(
        View::Sessions,
        PersistedViewSlot {
            tree,
            ..PersistedViewSlot::default()
        },
    );
    crate::tui_state::write_tui_state(cache, &state).expect("seed state");
}

/// A run started the way the live TUI starts one, over `rows()`.
fn restarted(cache: &TuiStateCache) -> App {
    let mut app = resumed_app(&RunConfig::defaults(), cache.clone());
    load(&mut app, rows());
    app
}

fn expanded(app: &App) -> BTreeSet<RowId> {
    app.tree_state().expanded().clone()
}

/// Whether the selected row shows, and a frame `height` rows tall would
/// have it on screen.
fn cursor_on_screen(app: &App, height: usize) -> bool {
    let rows = &app.tree().rows;
    let Some(cursor) = app.tree_state().cursor(rows) else {
        return false;
    };
    let window = app.tree_state().placed_window(rows, height);
    window.visible().contains(&cursor)
}

#[test]
fn the_expanded_rows_and_the_selected_row_come_back_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let mut first = resumed_app(&RunConfig::defaults(), cache.clone());
    load(&mut first, rows());
    assert!(
        first.tree_state().is_expanded(&repo("/a/x")),
        "the first group and its groups open by default"
    );
    first.update(Msg::SelectRow(Box::new(repo("/a/x"))));
    first.update(Msg::CollapseRow);
    first.update(Msg::SelectRow(Box::new(ws("/b"))));
    first.update(Msg::ExpandRow);
    first.update(Msg::SelectRow(Box::new(session("s4"))));
    let left = expanded(&first);
    // The run ends: the loop's last write.
    first.persist_state();

    let second = restarted(&cache);
    assert_eq!(expanded(&second), left);
    assert!(
        !second.tree_state().is_expanded(&repo("/a/x")),
        "a group collapsed by hand stays collapsed"
    );
    assert_eq!(second.selection(), Some(&session("s4")));
    assert!(cursor_on_screen(&second, 3), "the pane scrolls to it");
    assert!(
        second.explorer_reflects == Some(session("s4")),
        "the right pane follows the restored row"
    );
}

#[test]
fn a_gone_row_lands_on_the_deepest_row_of_its_path_still_listed() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    for (path, lands) in [
        (vec![ws("/a"), repo("/a/x")], repo("/a/x")),
        (vec![ws("/a"), repo("/a/gone")], ws("/a")),
        (vec![ws("/gone"), repo("/a/y")], repo("/a/y")),
    ] {
        seed(&cache, Some(&[]), Some((session("gone"), &path)));
        let app = restarted(&cache);
        assert_eq!(app.selection(), Some(&lands), "path {path:?}");
        assert!(cursor_on_screen(&app, 2), "path {path:?}");
        assert!(app.status_message.is_none(), "no warning for a gone row");
        assert!(app.toasts().is_empty());
    }
}

#[test]
fn a_gone_row_with_nothing_of_its_path_left_lands_on_the_first_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[ws("/b")]),
        Some((session("gone"), &[ws("/gone"), repo("/gone/x")])),
    );
    let mut app = restarted(&cache);
    assert_eq!(app.selection(), Some(&ws("/a")));
    assert!(app.status_message.is_none() && app.toasts().is_empty());
    seed(&cache, None, Some((session("gone"), &[])));
    app = restarted(&cache);
    assert_eq!(app.selection(), Some(&ws("/a")));
}

#[test]
fn a_restored_row_inside_collapsed_groups_opens_them() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[]),
        Some((session("s3"), &[ws("/a"), repo("/a/y")])),
    );
    let app = restarted(&cache);
    assert_eq!(app.selection(), Some(&session("s3")));
    assert_eq!(
        expanded(&app),
        BTreeSet::from([ws("/a"), repo("/a/y")]),
        "only the groups above it open"
    );
    assert!(cursor_on_screen(&app, 2));
}

#[test]
fn saved_folds_that_hide_the_first_loads_row_leave_the_cursor_on_one_that_shows() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(&cache, Some(&[]), None);
    let mut app = resumed_app(&RunConfig::defaults(), cache);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&GraphSnapshot::empty()),
        tree: rows(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: Some(session("s1")),
    });
    assert!(expanded(&app).is_empty(), "every group stays collapsed");
    assert_eq!(app.selection(), Some(&ws("/a")));
}

#[test]
fn of_two_copies_of_a_row_the_one_under_its_saved_path_is_selected() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let mut twice = rows();
    // s1 is listed under both repos.
    twice.rows.insert(5, row(session("s1"), 2, false));
    seed(
        &cache,
        Some(&[ws("/a"), repo("/a/x"), repo("/a/y")]),
        Some((session("s1"), &[ws("/a"), repo("/a/y")])),
    );
    let mut app = resumed_app(&RunConfig::defaults(), cache);
    load(&mut app, twice);
    let rows = &app.tree().rows;
    let shown = app.tree_state().visible_indices(rows);
    let cursor = app.tree_state().cursor(rows).expect("a cursor");
    assert_eq!(shown[cursor], 5, "the copy under /a/y, not the first");

    // And the next write names that copy's path.
    let remembered = app.remembered_tree().selected.expect("a selection");
    assert_eq!(remembered.path, vec![key(&ws("/a")), key(&repo("/a/y"))]);
}

#[test]
fn a_gone_group_is_ignored_and_leaves_the_file_on_the_next_write() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[ws("/b"), ws("/gone"), repo("/a/gone")]),
        Some((session("s4"), &[ws("/b")])),
    );
    let app = restarted(&cache);
    assert_eq!(expanded(&app), BTreeSet::from([ws("/b")]));
    app.persist_state();
    let read = crate::tui_state::read_tui_state(&cache).expect("read");
    assert_eq!(
        read.view_states[&View::Sessions].tree.expanded,
        Some(BTreeSet::from([key(&ws("/b"))])),
        "only the groups still listed are written back"
    );
}

#[test]
fn an_expanded_group_that_is_gone_by_the_write_is_left_out() {
    let mut app = App::new(RunConfig::defaults());
    load(&mut app, rows());
    assert!(app.tree_state().is_expanded(&repo("/a/x")));
    let without: Vec<Row> = rows()
        .rows
        .into_iter()
        .filter(|row| ![repo("/a/x"), session("s1"), session("s2")].contains(&row.id))
        .collect();
    load(
        &mut app,
        RowTree {
            rows: without,
            view: crate::tui::rows::ViewLabel::Sessions,
        },
    );
    assert!(
        app.tree_state().is_expanded(&repo("/a/x")),
        "the run keeps the fold, in case the group comes back"
    );
    let written = app.remembered_tree().expanded.expect("a list");
    assert!(!written.contains(&key(&repo("/a/x"))), "{written:?}");
}

#[test]
fn a_file_from_an_earlier_build_opens_on_the_default_folds_and_first_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let path = cache.path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"schema_version": 1, "last_view": "sessions",
            "view_states": {"sessions": {"grouping": {"Sessions": "graph"}}}}"#,
    )
    .unwrap();
    let restored = restarted(&cache);
    let mut fresh = App::new(RunConfig::defaults());
    load(&mut fresh, rows());
    assert_eq!(expanded(&restored), expanded(&fresh));
    assert_eq!(restored.selection(), fresh.selection());
    assert_eq!(restored.selection(), Some(&ws("/a")));
}

#[test]
fn a_malformed_or_unknown_row_key_is_skipped_and_the_rest_apply() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let path = cache.path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let b = key(&ws("/b"));
    std::fs::write(
        &path,
        format!(
            r#"{{"schema_version": 1, "row_keys": 1,
                "view_states": {{"sessions": {{
                  "expanded": [{b:?}, 3, "widget:unknown", "group:workspace"],
                  "selected": {{"row": "not a key", "path": [{b:?}, false]}}
                }}}}}}"#
        ),
    )
    .unwrap();
    let app = restarted(&cache);
    assert_eq!(expanded(&app), BTreeSet::from([ws("/b")]));
    assert_eq!(
        app.selection(),
        Some(&ws("/b")),
        "a key that names no row falls back along its path"
    );
}

#[test]
fn no_resume_view_opens_on_the_defaults_and_keeps_other_views_saved_rows() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let saved = PersistedTree {
        expanded: Some(BTreeSet::from([key(&ws("/b"))])),
        selected: Some(PersistedSelection {
            row: key(&session("s4")),
            path: vec![key(&ws("/b"))],
        }),
    };
    let mut state = PersistedState::default();
    for view in [View::Sessions, View::Mux] {
        state.view_states.insert(
            view,
            PersistedViewSlot {
                tree: saved.clone(),
                ..PersistedViewSlot::default()
            },
        );
    }
    crate::tui_state::write_tui_state(&cache, &state).unwrap();

    let mut config = RunConfig::defaults();
    config.resume_view = false;
    let mut app = resumed_app(&config, cache.clone());
    load(&mut app, rows());
    let mut fresh = App::new(RunConfig::defaults());
    load(&mut fresh, rows());
    assert_eq!(expanded(&app), expanded(&fresh));
    assert_eq!(app.selection(), fresh.selection());

    app.persist_state();
    let read = crate::tui_state::read_tui_state(&cache).unwrap();
    assert_eq!(
        read.view_states[&View::Mux].tree,
        saved,
        "a view the run never showed keeps what it had"
    );
    assert_eq!(
        read.view_states[&View::Sessions].tree,
        app.remembered_tree(),
        "the view it showed is written as this run left it"
    );
}

#[test]
fn moving_and_folding_write_nothing_until_the_run_writes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let mut app = restarted(&cache);
    for msg in [
        Msg::NavDown,
        Msg::NavDown,
        Msg::ToggleExpand,
        Msg::ExpandRow,
        Msg::CollapseRow,
        Msg::End,
        Msg::Home,
        Msg::PageDown,
        Msg::SelectRow(Box::new(session("s3"))),
    ] {
        let effects = app.update(msg);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, crate::tui::Effect::Persist)),
            "{effects:?}"
        );
    }
    assert!(
        !cache.path().unwrap().exists(),
        "nothing is written per key"
    );
    app.persist_state();
    let read = crate::tui_state::read_tui_state(&cache).unwrap();
    assert_eq!(
        read.view_states[&View::Sessions]
            .tree
            .selected
            .as_ref()
            .map(|selected| selected.row.clone()),
        Some(key(&session("s3")))
    );
}

#[test]
fn the_saved_tree_applies_once_and_a_later_refresh_keeps_the_operators_moves() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(&cache, Some(&[ws("/b")]), Some((ws("/b"), &[])));
    let mut app = restarted(&cache);
    assert_eq!(app.selection(), Some(&ws("/b")));
    app.update(Msg::NavDown);
    app.update(Msg::SelectRow(Box::new(repo("/a/y"))));
    app.update(Msg::ExpandRow);
    load(&mut app, rows());
    assert_eq!(app.selection(), Some(&repo("/a/y")));
    assert!(app.tree_state().is_expanded(&repo("/a/y")));
}

#[test]
fn the_saved_tree_waits_for_a_view_that_lists_no_rows_yet() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[ws("/b")]),
        Some((session("s4"), &[ws("/b")])),
    );
    let mut app = resumed_app(&RunConfig::defaults(), cache);
    load(&mut app, RowTree::default());
    assert_eq!(app.selection(), None);
    load(&mut app, rows());
    assert_eq!(app.selection(), Some(&session("s4")));
}

#[test]
fn a_regroup_before_the_rows_arrive_drops_the_saved_tree() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[ws("/b")]),
        Some((session("s4"), &[ws("/b")])),
    );
    let mut app = resumed_app(&RunConfig::defaults(), cache);
    app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
        crate::tui::SessionsGrouping::Workspace,
    )));
    load(&mut app, rows());
    assert_eq!(app.selection(), Some(&ws("/a")), "the view's top");
}

#[test]
fn only_the_live_entry_point_and_an_enabled_cache_reopen_saved_rows() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    seed(
        &cache,
        Some(&[ws("/b")]),
        Some((session("s4"), &[ws("/b")])),
    );
    // Snapshot mode builds its app with no cache: nothing is read.
    let mut app = App::new(RunConfig::defaults());
    app.restore_persisted_state();
    load(&mut app, rows());
    assert_eq!(app.selection(), Some(&ws("/a")));
}

// ---- over a real world, every view ---------------------------------------

/// The views that remember their rows.
const REMEMBERING: [View; 5] = [
    View::Sessions,
    View::Mux,
    View::Prs,
    View::Pinned,
    View::History,
];

/// The showcase world the README screenshots show, which lists rows in
/// every view.
fn world() -> GraphSnapshot {
    crate::tui::runtime::read_fixture(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/showcase.json"
    )))
    .expect("the showcase fixture")
}

fn live(cache: &TuiStateCache, config: &RunConfig, snapshot: &GraphSnapshot) -> App {
    let mut app = resumed_app(config, cache.clone());
    set_static_data(&mut app, config, snapshot);
    app
}

/// Leave each view with its first group closed (or its first closed
/// group open) and its last row selected; returns what each view showed.
fn visit_every_view(app: &mut App) -> BTreeMap<View, (BTreeSet<RowId>, Option<RowId>)> {
    let mut left = BTreeMap::new();
    for view in REMEMBERING {
        app.update(Msg::SwitchView(view));
        let rows = app.tree().rows.clone();
        if let Some(group) = rows.iter().find(|row| row.expandable) {
            app.update(Msg::SelectRow(Box::new(group.id.clone())));
            app.update(Msg::ToggleExpand);
        }
        if let Some(last) = rows.last() {
            app.update(Msg::SelectRow(Box::new(last.id.clone())));
        }
        let listed: BTreeSet<RowId> = rows.iter().map(|row| row.id.clone()).collect();
        let folds = app
            .tree_state()
            .expanded()
            .iter()
            .filter(|id| listed.contains(id))
            .cloned()
            .collect();
        left.insert(view, (folds, app.selection().cloned()));
    }
    left
}

#[test]
fn every_view_reopens_with_its_own_rows_through_the_live_entry_point() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let config = RunConfig::defaults();
    let snapshot = world();
    let mut first = live(&cache, &config, &snapshot);
    let left = visit_every_view(&mut first);
    assert!(
        left.values()
            .filter(|(_, selected)| selected.is_some())
            .count()
            >= 3,
        "the world lists rows in most views: {left:?}"
    );
    first.persist_state();

    let mut config = config;
    config.default_view = crate::tui_state::read_last_view(&cache).expect("a last view");
    let mut second = live(&cache, &config, &snapshot);
    for view in REMEMBERING.iter().rev().copied() {
        second.update(Msg::SwitchView(view));
        let listed: BTreeSet<RowId> = second
            .tree()
            .rows
            .iter()
            .map(|row| row.id.clone())
            .collect();
        let folds: BTreeSet<RowId> = second
            .tree_state()
            .expanded()
            .iter()
            .filter(|id| listed.contains(id))
            .cloned()
            .collect();
        let (left_folds, left_selected) = &left[&view];
        assert_eq!(&folds, left_folds, "{view:?}");
        assert_eq!(second.selection(), left_selected.as_ref(), "{view:?}");
    }

    // Switching views during the run keeps each view's rows too.
    second.update(Msg::SwitchView(View::Mux));
    let mux = second.selection().cloned();
    second.update(Msg::SwitchView(View::Sessions));
    second.update(Msg::SwitchView(View::Mux));
    assert_eq!(second.selection().cloned(), mux);
}

#[test]
fn a_jump_into_a_view_lands_on_its_target_not_the_saved_row() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let snapshot = world();
    let mut config = RunConfig::defaults();
    config.default_view = View::Pinned;
    let mut first = live(&cache, &config, &snapshot);
    // A view that lists two entities it is the natural view of.
    let (view, entities) = [View::Sessions, View::Mux, View::Prs]
        .into_iter()
        .find_map(|view| {
            first.update(Msg::SwitchView(view));
            let entities: Vec<(RowId, NodeId)> = first
                .tree()
                .rows
                .iter()
                .filter(|row| !matches!(row.kind, RowKind::Group(_)))
                .filter_map(|row| {
                    let node = super::super::tree::row_primary_node(row)?;
                    (crate::tui::actions::natural_view(&snapshot, node) == Some(view))
                        .then(|| (row.id.clone(), node.clone()))
                })
                .collect();
            (entities.len() >= 2).then_some((view, entities))
        })
        .expect("a view listing two entities");
    first.update(Msg::SelectRow(Box::new(entities[0].0.clone())));
    first.update(Msg::SwitchView(View::Pinned));
    first.persist_state();

    let mut second = live(&cache, &config, &snapshot);
    assert_eq!(second.active_view(), View::Pinned);
    second
        .reveal_node(&entities[1].1, None)
        .expect("its view lists it");
    assert_eq!(second.active_view(), view);
    assert_eq!(second.selection(), Some(&entities[1].0));

    // Without the jump, the view opens on the saved row.
    let mut third = live(&cache, &config, &snapshot);
    third.update(Msg::SwitchView(view));
    assert_eq!(third.selection(), Some(&entities[0].0));
}

/// The state file `tests/cli_tui_state_invariants.rs` seeds before a
/// snapshot, which must not reopen it.
const SNAPSHOT_SEED: &str = r#"{
  "schema_version": 1,
  "row_keys": 1,
  "view_states": {
    "sessions": {
      "expanded": ["group:repo:/fixture/repos/project/.git"],
      "selected": {
        "row": "session:agent_session:claude-code:/fixture/harness/claude:showcase-claude",
        "path": ["group:repo:/fixture/repos/project/.git"]
      }
    }
  }
}"#;

#[test]
fn the_state_the_snapshot_check_seeds_reopens_in_a_live_run() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache_in(&dir);
    let path = cache.path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, SNAPSHOT_SEED).unwrap();
    let project = RowId::Group(NodeId::Repo(RepoId::new("/fixture/repos/project/.git")));
    let app = live(&cache, &RunConfig::defaults(), &world());
    assert_eq!(expanded(&app), BTreeSet::from([project]));
    assert_eq!(
        app.selection().and_then(row_key).as_deref(),
        Some("session:agent_session:claude-code:/fixture/harness/claude:showcase-claude")
    );
}
