// Extracted from app.rs H-HYG-011 rolling wave via #[path = "app_tests.rs"] mod tests;
use super::*;
use crate::dev_scenarios;
use crate::filter::RowFilter;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
    PinBinding, PinCandidate, PinMuxRef, Provenance, RepoId, RepoNode, WorkspaceId,
};
use crate::resolve::resolve_snapshot;
use crate::tui::SessionsGrouping;
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};

fn make_snapshot_with(sessions: &[(&str, &str, &str)]) -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    // Unique repo per session for simplicity.
    for (harness, key, cwd) in sessions {
        let repo_id = RepoId::new(*cwd);
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, cwd.to_string()),
            root: cwd.to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }));
        snap.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new(*harness, "/state", *key),
                harness.to_string(),
            )
            .with_cwd(cwd.to_string()),
        ));
    }
    resolve_snapshot(snap)
}

fn build_tree(snap: &GraphSnapshot) -> RowTree {
    build_sessions_tree(SessionsBuildInputs {
        snapshot: snap,
        grouping: SessionsGrouping::Graph,
        home: None,
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    })
}

fn seeded_app(sessions: &[(&str, &str, &str)]) -> App {
    let snap = make_snapshot_with(sessions);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn scenario_app(name: &str) -> (App, GraphSnapshot) {
    // H-SERVE-PERF-011: scenarios call discover_local_with which
    // dispatches TmuxDiscovery through the process-global tmux
    // cache. Serialize the scenario materializations via the
    // tmux cache test lock and reset before each so a parallel
    // scenario can't short-circuit us to its cached fragment.
    // The lock only matters during materialize()/snapshot();
    // once App owns the resulting GraphSnapshot the subsequent
    // test asserts don't touch the cache.
    let _serial = crate::discovery::tmux::TMUX_CACHE_TEST_LOCK.lock().unwrap();
    crate::discovery::tmux::reset_tmux_cache_for_tests();

    let world = dev_scenarios::materialize(name).expect("materialize scenario");
    let snap = world.snapshot().expect("scenario snapshot");
    let tree = world.sessions_tree().expect("scenario sessions tree");
    let mut app = App::new(world.tui_config(View::Sessions, false));
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_600,
        initial_selection_hint: None,
    });
    (app, snap)
}

fn select_session(app: &mut App, session_key: &str) -> RowId {
    let id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.session.session_key == session_key => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("visible session row");
    app.set_selection(id.clone());
    id
}

#[test]
fn pins_context_seeds_pin_create_defaults_from_selected_session() {
    let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
    select_session(&mut app, "Session One");

    let defaults = app.pins_context().pin_create_defaults;
    assert_eq!(defaults.id, "session-one");
    assert_eq!(defaults.display_name, "Session One");
    assert_eq!(defaults.harness, "codex");
    assert_eq!(defaults.cwd, "/p/project");
    assert_eq!(defaults.mux_name, "session-one");
}

#[test]
fn pin_create_defaults_cap_long_selected_session_titles() {
    let long_title =
        "Investigate the customer workspace regression with the unusually verbose summary";
    let mut app = seeded_app(&[("codex", long_title, "/p/project")]);
    select_session(&mut app, long_title);

    let defaults = app.pins_context().pin_create_defaults;
    assert!(defaults.display_name.chars().count() <= 48);
    assert_eq!(
        defaults.display_name,
        "Investigate the customer workspace regression"
    );
    assert_eq!(defaults.id, "investigate-the-customer-workspace-regression");
    assert_eq!(defaults.mux_name, defaults.id);
}

#[test]
fn pins_context_seeds_pin_create_cwd_from_selected_group() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    let group_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::Group(group) if group.primary_node.is_some() => Some(row.id.clone()),
            _ => None,
        })
        .expect("group row with primary node");
    app.set_selection(group_id);

    let defaults = app.pins_context().pin_create_defaults;
    assert_eq!(defaults.cwd, "/p/proja");
    assert_eq!(defaults.id, "");
    assert_eq!(defaults.harness, "");
}

#[test]
fn pins_context_seeds_pin_create_cwd_from_selected_mux_absolute() {
    // Regression: the MuxSession branch of `pin_create_defaults`
    // used to seed `cwd` from `MuxSessionRow.cwd_display`, which
    // tilde-shortens the path. The pin write path then rejected
    // it via `Path::is_absolute`. The form must instead carry
    // the raw absolute cwd off the mux node.
    let mut snap = snapshot_session_with_mux();
    for node in snap.nodes.iter_mut() {
        if let crate::model::GraphNode::MuxSession(mux) = node {
            mux.cwd = Some("/p/proj".to_string());
        }
    }
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snap,
        home: None,
        now: None,
        filter: crate::tui::RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let mux_row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("mux row in tree");
    app.set_selection(mux_row_id);

    let defaults = app.pins_context().pin_create_defaults;
    assert_eq!(defaults.cwd, "/p/proj");
    assert_eq!(defaults.mode, PinCreateMode::NewVariation);
    assert_eq!(defaults.mux_name, "work-2");
    assert_eq!(defaults.display_name, "work-2");
    assert!(
        std::path::Path::new(&defaults.cwd).is_absolute(),
        "pin create cwd must be absolute: {:?}",
        defaults.cwd,
    );
}

#[test]
fn pin_adopt_defaults_preserve_selected_live_mux_name() {
    let snap = snapshot_session_with_mux();
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snap,
        home: None,
        now: None,
        filter: crate::tui::RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let mux_row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("mux row in tree");
    app.set_selection(mux_row_id);

    let defaults = app.pin_adopt_defaults();
    assert_eq!(defaults.mode, PinCreateMode::AdoptSelected);
    assert_eq!(defaults.id, "work");
    assert_eq!(defaults.display_name, "work");
    assert_eq!(defaults.mux_name, "work");
}

#[test]
fn pins_context_exposes_known_live_mux_names() {
    let snap = snapshot_session_with_mux();
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    let ctx = app.pins_context();
    assert_eq!(ctx.known_mux_names, vec!["work".to_string()]);
}

#[test]
fn pins_context_does_not_offer_adopt_for_already_pinned_mux() {
    let mut snap = snapshot_session_with_mux();
    snap.pins.push(PinCandidate {
        id: "work-pin".to_string(),
        display_name: "Work".to_string(),
        harness: "claude-code".to_string(),
        cwd: "/p/proj".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "work".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/proj/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux {
            mux: crate::model::MuxSessionId::new("work"),
        }),
    });
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snap,
        home: None,
        now: None,
        filter: crate::tui::RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Repo,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut app = App::new(RunConfig {
        default_view: View::Mux,
        mux_grouping: crate::tui::MuxGrouping::Repo,
        ..RunConfig::defaults()
    });
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let mux_row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(mux) if mux.native_id == "work" => Some(row.id.clone()),
            _ => None,
        })
        .expect("pinned mux row");
    app.set_selection(mux_row_id);

    let ctx = app.pins_context();
    assert_eq!(ctx.selected_pin_id.as_deref(), Some("work-pin"));
    assert_eq!(ctx.known_pin_ids, vec!["work-pin".to_string()]);
    assert_eq!(ctx.known_pin_mux_names, vec!["work".to_string()]);
    assert_eq!(ctx.pin_adopt_defaults, None);
}

#[test]
fn pins_context_exposes_registered_and_discovered_harness_keys() {
    let snap = make_snapshot_with(&[("custom-harness", "s1", "/workspace/project")]);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    let ctx = app.pins_context();
    assert!(ctx.known_harness_keys.contains(&"codex".to_string()));
    assert!(
        ctx.known_harness_keys
            .contains(&"custom-harness".to_string())
    );
}

#[test]
fn infer_harness_for_mux_picks_first_active_linked_to_mux_source() {
    // Reuses the session+mux+LinkedToMux fixture defined below in
    // the explorer test block: one claude-code AgentSession linked
    // to a `tmux:work` MuxSession via an Active LinkedToMux
    // candidate. The inference walk should land on
    // `claude-code` as the harness seed.
    let snap = snapshot_session_with_mux();
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let mux_id = crate::model::MuxSessionId::new("work");
    assert_eq!(
        app.infer_harness_for_mux(&mux_id).as_deref(),
        Some("claude-code"),
    );
}

#[test]
fn infer_harness_for_mux_returns_none_without_attribution() {
    // Empty graph → no candidate links → no inference. Confirms the
    // method is safe to call from `pin_create_defaults` regardless
    // of selection state.
    let app = App::new(RunConfig::defaults());
    let mux_id = crate::model::MuxSessionId::new("nowhere");
    assert!(app.infer_harness_for_mux(&mux_id).is_none());
}

#[test]
fn pins_context_seeds_pin_mutation_target_from_selected_pin_row() {
    let mut snap = GraphSnapshot::empty();
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/project".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/project/.conspectus.toml".to_string(),
        binding: None,
    });
    let snap = resolve_snapshot(snap);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let pin_row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.pin_id.as_deref() == Some("ingest") => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("placeholder pin row");
    app.set_selection(pin_row_id);

    let target = app.pins_context().pin_target.expect("pin mutation target");
    assert_eq!(target.id, "ingest");
    assert_eq!(target.display_name, "Ingest");
    assert_eq!(target.harness, "codex");
    assert_eq!(target.cwd, "/p/project");
    assert_eq!(target.mux_name, "ingest");
    assert_eq!(target.mux_socket, None);
    assert_eq!(target.launch_argv, Vec::<String>::new());
    assert_eq!(target.store_path, "/p/project/.conspectus.toml");
}

fn pin_only_snapshot(binding: Option<PinBinding>) -> GraphSnapshot {
    let mut snap = GraphSnapshot::empty();
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/project".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/project/.conspectus.toml".to_string(),
        binding,
    });
    snap.sync_pin_nodes();
    snap
}

#[test]
fn placeholder_detail_target_falls_back_to_pin_when_no_view_aligned_entity() {
    let snap = pin_only_snapshot(Some(PinBinding::Unbound));
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Sessions),
        NodeId::Pin(PinId::new("ingest")),
    );
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Mux),
        NodeId::Pin(PinId::new("ingest")),
    );
}

#[test]
fn placeholder_detail_target_upgrades_to_mux_for_stale_mux_in_mux_view() {
    let mux_id = MuxSessionId::new("tmux:ingest");
    let snap = pin_only_snapshot(Some(PinBinding::StaleMux {
        mux: mux_id.clone(),
    }));
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Mux),
        NodeId::MuxSession(mux_id),
    );
    // Sessions view stays on the pin: the stale mux is not a session.
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Sessions),
        NodeId::Pin(PinId::new("ingest")),
    );
}

#[test]
fn placeholder_detail_target_upgrades_to_last_session_in_sessions_view() {
    let mut snap = pin_only_snapshot(Some(PinBinding::Unbound));
    let session_id = AgentSessionId::new("codex", "/state", "alpha");
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(session_id.clone(), "codex".to_string())
            .with_cwd("/p/project".to_string()),
    ));
    snap.diagnostics.push(crate::model::Diagnostic::PinUnbound {
        pin_id: "ingest".to_string(),
        expected_mux_native_id: "tmux:ingest".to_string(),
        last_session: Some(crate::model::PinLastSession {
            session_id: "alpha".to_string(),
            observed_epoch: 1_700_000_000,
        }),
    });
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Sessions),
        NodeId::AgentSession(session_id),
    );
    // Mux view stays on the pin: a known session is not a mux.
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Mux),
        NodeId::Pin(PinId::new("ingest")),
    );
}

#[test]
fn placeholder_detail_target_falls_back_to_pin_when_last_session_not_in_snapshot() {
    let mut snap = pin_only_snapshot(Some(PinBinding::Unbound));
    snap.diagnostics.push(crate::model::Diagnostic::PinUnbound {
        pin_id: "ingest".to_string(),
        expected_mux_native_id: "tmux:ingest".to_string(),
        last_session: Some(crate::model::PinLastSession {
            session_id: "ghost".to_string(),
            observed_epoch: 1_700_000_000,
        }),
    });
    assert_eq!(
        placeholder_detail_target(&snap, "ingest", View::Sessions),
        NodeId::Pin(PinId::new("ingest")),
    );
}

#[test]
fn placeholder_pin_detail_strips_candidate_link_summaries() {
    // sync_pin_nodes synthesizes a LinkedToMux candidate from the
    // pin to its expected mux; on the Pin-fallback path the
    // detail/explorer surfaces are expected to elide that
    // candidate so the right pane reads as "nothing live yet".
    let snap = pin_only_snapshot(Some(PinBinding::Unbound));
    let snap = resolve_snapshot(snap);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let pin_row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.pin_id.as_deref() == Some("ingest") => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("placeholder pin row");
    app.set_selection(pin_row_id);
    let detail = app.detail().expect("detail computed for pin placeholder");
    assert_eq!(detail.kind_label, "pin");
    assert!(
        detail.outgoing_links.is_empty(),
        "pin fallback should not list outgoing candidate links: {:?}",
        detail.outgoing_links,
    );
    assert!(
        detail.incoming_links.is_empty(),
        "pin fallback should not list incoming candidate links: {:?}",
        detail.incoming_links,
    );
    assert!(
        detail.resolved.is_empty(),
        "pin fallback should not list resolved relationships: {:?}",
        detail.resolved,
    );
    let explorer = app.explorer().expect("explorer view present");
    assert!(
        explorer.view.relationships.groups.is_empty(),
        "explorer Related zone should be empty for pin fallback",
    );
}

#[test]
fn set_data_keeps_pins_group_expanded_by_default() {
    let mut snap = GraphSnapshot::empty();
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/project".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/project/.conspectus.toml".to_string(),
        binding: None,
    });
    let snap = resolve_snapshot(snap);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    assert!(app.expanded.contains(&RowId::Synthetic("pins")));
    assert!(
        app.visible_rows()
            .iter()
            .any(|row| matches!(row.id, RowId::Pin { .. })),
        "pin child should be visible without manually expanding Pins"
    );
}

#[test]
fn selected_bound_pin_row_shows_realizing_session_detail() {
    let session_id = AgentSessionId::new("codex", "/state", "alpha");
    let mut snap = make_snapshot_with(&[("codex", "alpha", "/p/project")]);
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/project".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/project/.conspectus.toml".to_string(),
        binding: Some(crate::model::PinBinding::Bound {
            mux: crate::model::MuxSessionId::new("tmux:ingest"),
            session: session_id,
        }),
    });
    snap.sync_pin_nodes();
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.pin_id.as_deref() == Some("ingest") => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("bound pin should render as a session row");
    app.set_selection(row_id);

    let detail = app
        .detail()
        .expect("bound pin session row should resolve detail");
    assert_eq!(detail.kind_label, "agent_session");
    assert!(
        detail
            .header_fields
            .iter()
            .any(|field| { field.label == "harness" && field.value.contains("codex") })
    );
}

#[test]
fn select_pin_after_mutation_expands_pins_and_selects_session_row() {
    let session_id = AgentSessionId::new("codex", "/state", "alpha");
    let mut snap = make_snapshot_with(&[("codex", "alpha", "/p/project")]);
    snap.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/project".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/project/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Bound {
            mux: crate::model::MuxSessionId::new("tmux:ingest"),
            session: session_id.clone(),
        }),
    });
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.expanded.remove(&RowId::Synthetic("pins"));

    assert!(app.select_pin_after_mutation("ingest"));
    assert!(app.expanded.contains(&RowId::Synthetic("pins")));
    let selected = app.selection().expect("selection");
    assert!(matches!(selected, RowId::AgentSession(NodeId::AgentSession(id)) if id == &session_id));
    let first_visible_pin_row = app
        .visible_rows()
        .into_iter()
        .find(|row| row_pin_id(row) == Some("ingest"))
        .expect("visible pinned row");
    assert_eq!(&first_visible_pin_row.id, selected);
}

#[test]
fn select_pin_after_mutation_selects_mux_row_in_mux_pins_group() {
    let mut snap = snapshot_session_with_mux();
    snap.pins.push(PinCandidate {
        id: "work-pin".to_string(),
        display_name: "Work".to_string(),
        harness: "claude-code".to_string(),
        cwd: "/p/proj".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "work".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/proj/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux {
            mux: crate::model::MuxSessionId::new("work"),
        }),
    });
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snap,
        home: None,
        now: None,
        filter: crate::tui::RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Repo,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut app = App::new(RunConfig {
        default_view: View::Mux,
        mux_grouping: crate::tui::MuxGrouping::Repo,
        ..RunConfig::defaults()
    });
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.expanded.remove(&RowId::Synthetic("pins"));

    assert!(app.select_pin_after_mutation("work-pin"));
    assert!(app.expanded.contains(&RowId::Synthetic("pins")));
    assert!(matches!(
        app.selection(),
        Some(RowId::MuxSession(NodeId::MuxSession(id))) if id.native_id == "work"
    ));
    let first_visible_pin_row = app
        .visible_rows()
        .into_iter()
        .find(|row| row_pin_id(row) == Some("work-pin"))
        .expect("visible pinned mux row");
    assert_eq!(Some(&first_visible_pin_row.id), app.selection());
}

#[test]
fn empty_tree_leaves_selection_none() {
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&GraphSnapshot::empty()),
        tree: RowTree::default(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    assert!(app.selection().is_none());
    assert!(app.detail().is_none());
}

#[test]
fn set_data_default_expands_group_rows_and_selects_first_visible() {
    let app = seeded_app(&[("codex", "a", "/p/proj")]);
    let visible = app.visible_rows();
    assert!(
        !visible.is_empty(),
        "initial tree expands a visible starting context"
    );
    assert!(app.selection().is_some());
}

#[test]
fn set_data_first_load_honors_initial_selection_hint() {
    // Build a tree with two project groups and feed the second
    // group's id as the launch-context hint on first SetData.
    // The reducer should pre-select the hinted row instead of
    // the leading row.
    let snap = make_snapshot_with(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    let tree = build_tree(&snap);
    // Pick a group row whose id is *not* the first visible row.
    let first_group_id = tree
        .rows
        .iter()
        .find(|r| matches!(&r.kind, RowKind::Group(_)))
        .map(|r| r.id.clone())
        .expect("at least one group row");
    let hint = tree
        .rows
        .iter()
        .find_map(|r| match &r.kind {
            RowKind::Group(_) if r.id != first_group_id => Some(r.id.clone()),
            _ => None,
        })
        .expect("at least two group rows in the seeded tree");

    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: Some(hint.clone()),
    });
    assert_eq!(app.selection().cloned(), Some(hint));
}

#[test]
fn set_data_first_load_expands_only_launch_context_tree() {
    let snap = make_snapshot_with(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snap,
        grouping: SessionsGrouping::Graph,
        home: None,
        now: None,
        cwd: Some(std::path::Path::new("/p/projb")),
        filter: RowFilter::default(),
    });
    let hint = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::Group(group) if group.is_launch_context => Some(row.id.clone()),
            _ => None,
        })
        .expect("launch context row");

    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: Some(hint),
    });

    let visible = app.visible_rows();
    assert!(
        visible.iter().any(|row| {
            matches!(&row.kind, RowKind::AgentSession(s) if s.session.session_key == "b")
        }),
        "launch-context session should be visible"
    );
    assert!(
        !visible.iter().any(|row| {
            matches!(&row.kind, RowKind::AgentSession(s) if s.session.session_key == "a")
        }),
        "non-launch-context sessions should start collapsed"
    );
}

#[test]
fn set_data_later_refreshes_ignore_initial_selection_hint() {
    // Seed the app once so prev_selection is populated, then
    // dispatch a second SetData with a hint that points
    // elsewhere. The retained selection should win.
    let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    app.update(Msg::End); // move selection to the last row
    let kept = app.selection().cloned().expect("selection present");

    let snap = app.graph_db().unwrap().snapshot().clone();
    let tree = build_tree(&snap);
    // Pick *some* other row id as the hint.
    let hint = tree
        .rows
        .iter()
        .map(|r| r.id.clone())
        .find(|id| id != &kept)
        .expect("at least one alternate row");
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_010,
        initial_selection_hint: Some(hint),
    });

    assert_eq!(
        app.selection().cloned(),
        Some(kept),
        "later refreshes must not overwrite the operator's selection with the hint"
    );
}

#[test]
fn nav_down_and_up_walk_visible_rows() {
    let app = seeded_app(&[
        ("codex", "a", "/p/proja"),
        ("codex", "b", "/p/projb"),
        ("codex", "c", "/p/projc"),
    ]);
    let mut app = app;
    let visible = app.visible_rows_owned();
    assert!(visible.len() >= 3);

    let start = app.selection().cloned().unwrap();
    app.update(Msg::NavDown);
    let after_down = app.selection().cloned().unwrap();
    assert_ne!(start, after_down);
    app.update(Msg::NavUp);
    assert_eq!(app.selection().cloned().unwrap(), start);
}

#[test]
fn nav_down_past_duplicate_row_id_advances_to_the_following_row() {
    // Regression: the mux view emits the same agent-session
    // RowId under every candidate mux when the resolver hasn't
    // picked. Before the duplicate-RowId tiebreaker, `NavDown`
    // from the second copy snapped back to the row after the
    // first copy because `move_selection` looked up the current
    // position with `.position(...)`, which returned the first
    // occurrence. With the tiebreaker, the cursor advances to
    // the row *immediately following* the second copy as
    // expected. Use a hand-built RowTree so the test does not
    // depend on mux row-builder details.
    use crate::tui::rows::{GroupRow, Row, RowId, RowKind};

    let group_row = |id: NodeId, label: &str| Row {
        id: RowId::Group(id.clone()),
        depth: 0,
        expandable: false,
        kind: RowKind::Group(GroupRow {
            display_path: label.to_string(),
            primary_node: Some(id),
            is_launch_context: false,
        }),
    };
    let workspace = |key: &str| NodeId::Workspace(WorkspaceId::new(key));
    let dup_id = workspace("dup");
    let tree = crate::tui::rows::RowTree {
        view: crate::tui::rows::ViewLabel::Mux,
        rows: vec![
            group_row(workspace("a"), "a"),
            group_row(dup_id.clone(), "dup-first"),
            group_row(dup_id.clone(), "dup-second"),
            group_row(workspace("c"), "c"),
        ],
    };

    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&GraphSnapshot::empty()),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    // Step onto the first duplicate, then the second.
    app.update(Msg::NavDown);
    app.update(Msg::NavDown);
    assert_eq!(app.last_visible_index, Some(2));
    assert_eq!(app.selection.as_ref(), Some(&RowId::Group(dup_id)));

    // From the second duplicate, NavDown must advance to the
    // row *after* it, not snap back to the row after the first
    // copy.
    app.update(Msg::NavDown);
    assert_eq!(app.last_visible_index, Some(3));
    assert_eq!(
        app.selection.as_ref(),
        Some(&RowId::Group(workspace("c"))),
        "NavDown from the second duplicate must land on the next row, not snap to the row after the first copy",
    );
}

#[test]
fn end_jumps_to_last_visible_and_home_returns_to_first() {
    let app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    let mut app = app;
    let visible = app.visible_rows_owned();
    let first = visible.first().cloned().unwrap();
    let last = visible.last().cloned().unwrap();

    app.update(Msg::End);
    assert_eq!(app.selection().cloned().unwrap(), last);
    app.update(Msg::Home);
    assert_eq!(app.selection().cloned().unwrap(), first);
}

#[test]
fn toggle_expand_collapses_and_re_expands_groups() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    // Selection starts on the first row (a group row).
    let group_id = app.selection().cloned().unwrap();
    assert!(matches!(group_id, RowId::Group(_)));
    let visible_before = app.visible_rows_owned().len();
    app.update(Msg::ToggleExpand);
    let visible_collapsed = app.visible_rows_owned().len();
    assert!(
        visible_collapsed < visible_before,
        "collapsing hides descendants"
    );
    app.update(Msg::ToggleExpand);
    let visible_again = app.visible_rows_owned().len();
    assert_eq!(
        visible_again, visible_before,
        "re-expansion restores the view"
    );
}

#[test]
fn expand_row_opens_then_no_ops_when_already_open() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    // Selection starts on the first group row.
    let group_id = app.selection().cloned().unwrap();
    assert!(matches!(group_id, RowId::Group(_)));
    // Force-collapse to mirror the user pressing `h` on an
    // already-expanded row.
    app.update(Msg::CollapseRow);
    let visible_after_collapse = app.visible_rows_owned().len();
    app.update(Msg::ExpandRow);
    let visible_after_expand = app.visible_rows_owned().len();
    assert!(
        visible_after_expand > visible_after_collapse,
        "expand reveals children"
    );
    // Repeated expand is a no-op (it doesn't re-collapse).
    app.update(Msg::ExpandRow);
    assert_eq!(app.visible_rows_owned().len(), visible_after_expand);
}

#[test]
fn collapse_row_hides_descendants_and_then_no_ops() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    let visible_before = app.visible_rows_owned().len();
    app.update(Msg::CollapseRow);
    let visible_after = app.visible_rows_owned().len();
    assert!(visible_after < visible_before, "collapse hides descendants");
    // Repeated collapse is a no-op.
    app.update(Msg::CollapseRow);
    assert_eq!(app.visible_rows_owned().len(), visible_after);
}

#[test]
fn expand_collapse_no_op_on_leaf_row() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    // Move past the group row to the leaf session row.
    app.update(Msg::NavDown);
    let leaf_id = app.selection().cloned().unwrap();
    assert!(matches!(leaf_id, RowId::AgentSession(_)));
    let visible = app.visible_rows_owned().len();
    app.update(Msg::ExpandRow);
    assert_eq!(app.visible_rows_owned().len(), visible);
    app.update(Msg::CollapseRow);
    assert_eq!(app.visible_rows_owned().len(), visible);
}

#[test]
fn cycle_focus_alternates_panels() {
    let mut app = App::new(RunConfig::defaults());
    assert_eq!(app.focus(), Focus::Left);
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Left);
}

#[test]
fn scroll_preview_clamps_at_zero() {
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::ScrollPreviewBy(1));
    app.update(Msg::ScrollPreviewBy(1));
    assert_eq!(app.preview_scroll(), 2);
    app.update(Msg::ScrollPreviewBy(-1));
    app.update(Msg::ScrollPreviewBy(-1));
    app.update(Msg::ScrollPreviewBy(-1));
    assert_eq!(app.preview_scroll(), 0);
}

#[test]
fn scroll_preview_by_advances_by_arbitrary_delta() {
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::ScrollPreviewBy(20));
    assert_eq!(app.preview_scroll(), 20);
    app.update(Msg::ScrollPreviewBy(-5));
    assert_eq!(app.preview_scroll(), 15);
    // Negative beyond zero clamps.
    app.update(Msg::ScrollPreviewBy(-1000));
    assert_eq!(app.preview_scroll(), 0);
}

#[test]
fn adjust_left_scroll_keeps_selection_above_top() {
    let mut app = App::new(RunConfig::defaults());
    // Selection at line 0 with viewport 5: offset is 0.
    assert_eq!(app.adjust_left_scroll(0, 5), 0);
    // Walk the selection down to line 10; offset advances so
    // the selection is on the bottom edge of the viewport.
    assert_eq!(app.adjust_left_scroll(10, 5), 6);
    // Walk back up to line 4 — selection moved above the top
    // edge, so the offset retreats to it.
    assert_eq!(app.adjust_left_scroll(4, 5), 4);
}

#[test]
fn adjust_left_scroll_holds_when_selection_inside_viewport() {
    let mut app = App::new(RunConfig::defaults());
    // Prime offset by scrolling to line 10 in a 5-tall viewport.
    app.adjust_left_scroll(10, 5);
    assert_eq!(app.left_scroll(), 6);
    // Move selection within [6, 10] — offset should not change.
    assert_eq!(app.adjust_left_scroll(8, 5), 6);
    assert_eq!(app.adjust_left_scroll(7, 5), 6);
    assert_eq!(app.adjust_left_scroll(10, 5), 6);
}

#[test]
fn adjust_left_scroll_with_zero_viewport_does_nothing() {
    let mut app = App::new(RunConfig::defaults());
    app.adjust_left_scroll(10, 5);
    let before = app.left_scroll();
    let returned = app.adjust_left_scroll(99, 0);
    assert_eq!(returned, before);
    assert_eq!(app.left_scroll(), before);
}

#[test]
fn adjust_explorer_scroll_keeps_cursor_in_viewport() {
    let mut app = App::new(RunConfig::defaults());
    assert_eq!(app.adjust_explorer_scroll(0, 0, 5), 0);
    assert_eq!(app.adjust_explorer_scroll(10, 10, 5), 6);
    assert_eq!(app.adjust_explorer_scroll(4, 4, 5), 4);
}

#[test]
fn adjust_explorer_scroll_holds_when_cursor_inside_viewport() {
    let mut app = App::new(RunConfig::defaults());
    app.adjust_explorer_scroll(10, 10, 5);
    assert_eq!(app.explorer_scroll(), 6);
    assert_eq!(app.adjust_explorer_scroll(8, 8, 5), 6);
    assert_eq!(app.adjust_explorer_scroll(7, 7, 5), 6);
    assert_eq!(app.adjust_explorer_scroll(10, 10, 5), 6);
}

#[test]
fn adjust_explorer_scroll_with_zero_viewport_does_nothing() {
    let mut app = App::new(RunConfig::defaults());
    app.adjust_explorer_scroll(10, 10, 5);
    let before = app.explorer_scroll();
    let returned = app.adjust_explorer_scroll(99, 99, 0);
    assert_eq!(returned, before);
    assert_eq!(app.explorer_scroll(), before);
}

#[test]
fn adjust_explorer_scroll_keeps_wrapped_cursor_line_fully_visible() {
    // Regression: when the cursor's logical line wraps to 2+
    // rendered rows, only feeding the line's start row left
    // the trailing wrap rows below the viewport bottom. The
    // span-aware API uses the cursor's last row to drive the
    // "scroll down" branch so a 2-row wrapped cursor line at
    // the bottom of the content advances the offset enough
    // for both rows to fit.
    let mut app = App::new(RunConfig::defaults());
    // Viewport 5 rows. Cursor's line starts at row 9 and
    // wraps to 2 rows (occupies 9 and 10). The offset must
    // advance to 6 so both 9 and 10 fit in [6, 10].
    assert_eq!(app.adjust_explorer_scroll(9, 10, 5), 6);
    assert_eq!(app.explorer_scroll(), 6);
    // Single-row cursor at the same row keeps the older
    // tighter behavior (offset = 5).
    let mut app = App::new(RunConfig::defaults());
    assert_eq!(app.adjust_explorer_scroll(9, 9, 5), 5);
}

#[test]
fn scenario_ambiguous_mux_session_is_leaf_after_adr_0071() {
    // ADR 0071: ambiguous mux candidates no longer expand a
    // per-session subtree; the chip stays but the row is a
    // leaf. The catalog of muxes lives on the shared-ancestor
    // group's detail pane via `ambiguous_muxes_for_group`.
    let (mut app, _) = scenario_app("ambiguous-mux");
    select_session(&mut app, "ambiguous");

    let session_row_id = app.selection().cloned().expect("session selected");
    let session_row = app
        .visible_rows()
        .iter()
        .find(|row| row.id == session_row_id)
        .cloned()
        .expect("session row visible");
    assert!(
        !session_row.expandable,
        "ambiguous session row stops being expandable after ADR 0071",
    );
    assert!(
        !app.visible_rows()
            .iter()
            .any(|row| matches!(row.kind, RowKind::AgentSessionMuxCandidate(_))),
        "no candidate child rows after ADR 0071",
    );
}

#[test]
fn scenario_refresh_when_selected_row_disappears_snaps_to_visible_row() {
    let (mut app, _) = scenario_app("exact-match");
    let old_selection = select_session(&mut app, "session-x");

    let replacement =
        dev_scenarios::materialize("orphan-session").expect("materialize replacement scenario");
    let snap = replacement.snapshot().expect("replacement snapshot");
    let tree = replacement.sessions_tree().expect("replacement tree");
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_601,
        initial_selection_hint: None,
    });

    let new_selection = app.selection().cloned().expect("fallback selection");
    assert_ne!(
        new_selection, old_selection,
        "selected exact-match row should have disappeared"
    );
    assert!(
        app.visible_rows().iter().any(|row| row.id == new_selection),
        "fallback selection should point at a visible row"
    );
}

#[test]
fn scenario_attach_target_refuses_current_tmux_session() {
    // H-SERVE-PERF-011: hold the tmux cache test lock + reset
    // for the same reason `scenario_app` does — the scenario
    // dispatches TmuxDiscovery through the process-global cache.
    let _serial = crate::discovery::tmux::TMUX_CACHE_TEST_LOCK.lock().unwrap();
    crate::discovery::tmux::reset_tmux_cache_for_tests();

    let world = dev_scenarios::materialize("exact-match").expect("materialize scenario");
    let snap = world.snapshot().expect("scenario snapshot");
    let tree = world.sessions_tree().expect("scenario sessions tree");
    let mut config = world.tui_config(View::Sessions, false);
    config.current_tmux_session = Some("editor".to_string());
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_600,
        initial_selection_hint: None,
    });
    select_session(&mut app, "session-x");

    assert_eq!(
        crate::tui::actions::resolve_attach_target(&app),
        Err(crate::tui::actions::AttachDisabled::CurrentTmuxSession(
            "editor".to_string()
        ))
    );
}

#[test]
fn set_data_retains_selection_by_row_id_when_present() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    // Move to a session row (not a group) so the retained
    // selection is clearly tied to the agent session.
    app.update(Msg::End);
    let saved = app.selection().cloned().unwrap();

    // Rebuild from the same snapshot; the row tree is
    // deterministic, so RowId equality should retain selection.
    let snap = app.graph_db().unwrap().snapshot().clone();
    let tree = build_tree(&snap);
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    assert_eq!(app.selection().cloned().unwrap(), saved);
}

#[test]
fn set_data_falls_back_to_nearest_index_when_selection_disappears() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja"), ("codex", "b", "/p/projb")]);
    app.update(Msg::End);
    let original_selection = app.selection().cloned().unwrap();

    // Build a snapshot that drops the previously-selected row.
    let snap = make_snapshot_with(&[("codex", "a", "/p/proja")]);
    let tree = build_tree(&snap);
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    let new_selection = app.selection().cloned().unwrap();
    assert_ne!(new_selection, original_selection);
    assert!(
        app.visible_rows_owned()
            .iter()
            .any(|id| id == &new_selection),
        "fallback selection must be visible"
    );
}

#[test]
fn detail_is_recomputed_when_selection_lands_on_a_node_row() {
    let app = seeded_app(&[("codex", "a", "/p/proja")]);
    // Selection auto-lands on the first visible row (a group);
    // groups still produce a NodeDetail because they're backed
    // by a NodeId.
    assert!(app.detail().is_some());
}

// ---- ADR 0031 / F8-003: per-view state retention ----

#[test]
fn switching_views_saves_active_state_and_loads_target_defaults() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    // Apply a sessions-only filter so we can observe it round-trip.
    app.update(Msg::SetFilter(crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    }));
    // Switch to mux view; sessions state should park in the
    // per-view map and mux loads fresh defaults.
    app.update(Msg::SwitchView(View::Mux));
    assert_eq!(app.active_view(), View::Mux);
    assert!(app.filter().is_empty(), "mux view starts unfiltered");
    assert_eq!(
        app.grouping(),
        crate::tui::Grouping::default_for(View::Mux),
        "mux view starts at its default grouping"
    );
    assert!(app.selection().is_none(), "fresh view has no selection");
}

#[test]
fn switching_back_to_prior_view_restores_filter_and_grouping() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    let original_filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    };
    app.update(Msg::SetFilter(original_filter.clone()));
    app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
        crate::tui::SessionsGrouping::Repo,
    )));
    // Switch away…
    app.update(Msg::SwitchView(View::Prs));
    // …and back.
    app.update(Msg::SwitchView(View::Sessions));
    assert_eq!(app.filter(), &original_filter);
    assert_eq!(
        app.grouping(),
        crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Repo)
    );
}

#[test]
fn switching_views_keeps_sort_global() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    app.update(Msg::SetSort(crate::tui::Sort::Recency));
    app.update(Msg::SwitchView(View::Mux));
    assert_eq!(app.sort(), crate::tui::Sort::Recency);
    app.update(Msg::SwitchView(View::Sessions));
    assert_eq!(app.sort(), crate::tui::Sort::Recency);
}

#[test]
fn flat_sessions_grouping_forces_recency_sort() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
        crate::tui::SessionsGrouping::None,
    )));
    assert_eq!(app.sort(), crate::tui::Sort::Recency);

    app.update(Msg::SetSort(crate::tui::Sort::Hierarchy));
    assert_eq!(app.sort(), crate::tui::Sort::Recency);
}

#[test]
fn returning_to_flat_sessions_grouping_restores_recency_sort() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
        crate::tui::SessionsGrouping::None,
    )));
    app.update(Msg::SwitchView(View::Mux));
    app.update(Msg::SetSort(crate::tui::Sort::Hierarchy));
    app.update(Msg::SwitchView(View::Sessions));
    assert_eq!(app.sort(), crate::tui::Sort::Recency);
}

#[test]
fn no_op_view_switch_is_idempotent() {
    let mut app = seeded_app(&[("codex", "a", "/p/proja")]);
    app.update(Msg::SetFilter(crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    }));
    let filter_before = app.filter().clone();
    let grouping_before = app.grouping();
    // SwitchView to the current view should be a no-op — not
    // a save+restore cycle that could wipe state.
    app.update(Msg::SwitchView(app.active_view()));
    assert_eq!(app.filter(), &filter_before);
    assert_eq!(app.grouping(), grouping_before);
}

// ----- T8-028: explorer navigation / drilldown / breadcrumb -----

use crate::model::{
    Confidence, LinkEndpoint, LinkState, MuxSessionNode, RelationKind, SourceMetadata,
};
use crate::tui::explorer::ExplorerRow;

fn snapshot_session_with_mux() -> GraphSnapshot {
    // Session → linked_to_mux → mux. Drives the simplest
    // drillable explorer state: one downstream group with one
    // link.
    let mut snap = GraphSnapshot::empty();
    let repo_id = RepoId::new("/p/proj");
    snap.nodes
        .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
    snap.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo_id, "/p/proj".to_string()),
        root: "/p/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", "abc"),
            "claude-code".to_string(),
        )
        .with_cwd("/p/proj".to_string())
        .with_last_active_epoch(1_700_000_000),
    ));
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(
            crate::model::MuxSessionId::new("work"),
            "tmux".to_string(),
            "work".to_string(),
        )
        .with_client_attached(true)
        .with_activity_epoch(1_700_000_000)
        .with_created_epoch(1_700_000_000),
    ));
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let mux_id = NodeId::MuxSession(crate::model::MuxSessionId::new("work"));
    snap.candidate_links.push(crate::model::GraphLink {
        id: "l1".to_string(),
        source: session_id,
        target: LinkEndpoint::Node { id: mux_id },
        relation: RelationKind::LinkedToMux,
        provenance: crate::model::Provenance::StrongDiscovered,
        confidence: crate::model::Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    resolve_snapshot(snap)
}

fn app_for_explorer() -> App {
    let snap = snapshot_session_with_mux();
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    // Land selection on the agent session row.
    let row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("agent session row in tree");
    app.set_selection(row_id);
    app
}

#[test]
fn explorer_home_and_end_snap_cursor_to_first_and_last_row() {
    // H-OBS-007: `g`/`Home` and `G`/`End` snap the explorer
    // cursor to its first / last row when the right pane has
    // focus. Reducer-level test: dispatching the messages
    // directly drives the cursor regardless of which pane has
    // focus (the focus check happens in `remap_for_focus`).
    let mut app = app_for_explorer();
    let row_count = app.explorer().expect("state").rows().len();
    assert!(row_count > 1, "fixture should expose multiple rows");

    // Walk the cursor down a couple of rows, then End it.
    app.update(Msg::ExplorerNavDown);
    app.update(Msg::ExplorerNavDown);
    app.update(Msg::ExplorerEnd);
    assert_eq!(
        app.explorer().expect("state").cursor,
        row_count - 1,
        "ExplorerEnd should snap cursor to the last row",
    );

    // Home brings it back to the top.
    app.update(Msg::ExplorerHome);
    assert_eq!(
        app.explorer().expect("state").cursor,
        0,
        "ExplorerHome should snap cursor to the first row",
    );
}

#[test]
fn explorer_state_initializes_with_cursor_on_the_first_node_field() {
    // Defaulting to the first Node field keeps the Preview zone
    // showing the live mux capture / message preview the
    // operator expects to see by default. Pressing `j` walks
    // down into the relationship rows, at which point the
    // Preview switches to neighbor + edge content.
    let app = app_for_explorer();
    let state = app.explorer().expect("explorer state present");
    let row = state.selected_row().expect("selected row");
    assert!(matches!(row, ExplorerRow::NodeField { index: 0, .. }));
}

#[test]
fn explorer_enter_on_link_drills_into_neighbor_and_pushes_breadcrumb() {
    let mut app = app_for_explorer();
    let before = app
        .explorer()
        .expect("explorer state present")
        .view
        .focused
        .clone();
    // Walk past the Node fields and onto the first link row,
    // then activate to drill.
    let target_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..target_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    let after = app.explorer().expect("explorer state after drill");
    // Focus now points at the mux.
    assert_ne!(after.view.focused, before);
    assert_eq!(after.view.kind_label, "mux_session");
    assert_eq!(after.breadcrumb.len(), 1);
    assert_eq!(after.breadcrumb[0].focused, before);
}

#[test]
fn explorer_drill_mirror_sync_keeps_left_pane_when_neighbor_has_no_row() {
    // T8-035: drilling from a session into its mux while the
    // left pane is in the sessions view should preserve the
    // prior selection because the sessions view doesn't carry
    // a MuxSession row. The hop still records the prior
    // selection so Backspace can restore it.
    let mut app = app_for_explorer();
    let pre_drill_selection = app
        .selection()
        .expect("session selection in app_for_explorer")
        .clone();
    let target_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..target_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    // Sessions view doesn't render the mux as a row, so the
    // left selection should stay put.
    assert_eq!(
        app.selection().expect("selection after drill"),
        &pre_drill_selection,
        "left selection should be preserved when the neighbor has no row",
    );
    // But the explorer should still be focused on the mux —
    // mirror sync's missing-row fallback only affects the left
    // pane.
    assert_eq!(
        app.explorer().expect("state").view.kind_label,
        "mux_session"
    );
    let hop = app
        .explorer()
        .expect("state")
        .breadcrumb
        .last()
        .expect("one hop after drill")
        .clone();
    assert_eq!(hop.left_pane_selection.as_ref(), Some(&pre_drill_selection));
}

#[test]
fn explorer_drill_mirrors_left_pane_to_neighbor_when_present_in_tree() {
    // T8-035: when the drilled neighbor *does* have a row in
    // the current view (here: drilling from one agent session
    // to a sibling agent session via `ParentSession`), the
    // left pane should move to it.
    let mut snap = snapshot_session_with_mux();
    // Add a second agent session and a parent_session link
    // session_a → session_b so the explorer's downstream group
    // exposes the sibling as a drillable neighbor.
    let session_b = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "child"),
        "claude-code".to_string(),
    )
    .with_cwd("/p/proj".to_string())
    .with_last_active_epoch(1_700_000_000);
    let parent_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "abc"));
    let child_id = NodeId::AgentSession(session_b.id.clone());
    snap.nodes.push(GraphNode::AgentSession(session_b));
    snap.candidate_links.push(crate::model::GraphLink {
        id: "sibling".to_string(),
        source: parent_id,
        target: LinkEndpoint::Node {
            id: child_id.clone(),
        },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snap = resolve_snapshot(snap);
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    // Select the parent session row. Use the underlying tree
    // rather than visible_rows because the parent may sit under
    // a not-yet-expanded group; set_selection accepts any row
    // that exists in the flat tree.
    let parent_row = app
        .tree()
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(s) if s.session.session_key == "abc" => Some(row.id.clone()),
            _ => None,
        })
        .expect("parent agent session row in tree");
    app.set_selection(parent_row);
    app.update(Msg::CycleFocus);
    // Walk to a link row whose neighbor is the child session.
    let rows = app.explorer().expect("state").rows();
    let link_idx = rows
        .iter()
        .enumerate()
        .find_map(|(idx, row)| match row {
            ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
                if app.explorer().expect("state").view.drill_target(row)
                    == Some(child_id.clone()) =>
            {
                Some(idx)
            }
            _ => None,
        })
        .expect("link row drilling into the child session");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    let post = app.selection().expect("selection after drill").clone();
    let child_row = app
        .tree()
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(s) if s.session.session_key == "child" => Some(row.id.clone()),
            _ => None,
        })
        .expect("child agent session row in tree");
    assert_eq!(
        post, child_row,
        "left pane should mirror the drilled neighbor when it has a row",
    );
    // Right pane still on the child session.
    assert_eq!(
        app.explorer().expect("state").view.focused,
        child_id,
        "explorer should still focus the drilled neighbor",
    );
}

#[test]
fn explorer_backspace_restores_left_pane_selection() {
    // T8-035: Backspace should pop the hop, restore the prior
    // left-pane selection, and refocus the explorer on the
    // pre-drill node.
    let mut app = app_for_explorer();
    let pre_drill_selection = app
        .selection()
        .expect("session selection in app_for_explorer")
        .clone();
    let target_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..target_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    // Backspace.
    app.update(Msg::ExplorerBack);
    assert_eq!(
        app.selection().expect("selection after backspace"),
        &pre_drill_selection,
        "left pane should be restored to the pre-drill row",
    );
    assert_eq!(
        app.explorer().expect("state").view.kind_label,
        "agent_session",
        "right pane should be restored to the pre-drill node",
    );
}

#[test]
fn explorer_backspace_restores_previous_focused_node_and_cursor() {
    let mut app = app_for_explorer();
    // Walk past the Node fields and onto the first link row.
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    let before_state = app.explorer().expect("initial").clone();
    let before_focus = before_state.view.focused.clone();
    let before_cursor_key = before_state
        .selected_row()
        .map(|row| row.key(&before_state.view));
    app.update(Msg::ExplorerActivate);
    // Move cursor on the new focused node to prove it gets
    // restored to the *original* one on Backspace.
    app.update(Msg::ExplorerNavDown);
    app.update(Msg::ExplorerBack);
    let restored = app.explorer().expect("restored explorer");
    assert_eq!(restored.view.focused, before_focus);
    assert!(restored.breadcrumb.is_empty());
    let restored_cursor = restored.selected_row().map(|row| row.key(&restored.view));
    assert_eq!(restored_cursor, before_cursor_key);
}

#[test]
fn explorer_back_with_no_breadcrumb_and_left_focus_surfaces_status_hint() {
    let mut app = app_for_explorer();
    assert_eq!(app.focus(), Focus::Left);
    app.update(Msg::ExplorerBack);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("no drill history"))
    );
    // Focus stays put when there's nothing to back out of.
    assert_eq!(app.focus(), Focus::Left);
}

#[test]
fn explorer_back_with_no_breadcrumb_and_right_focus_arms_then_shifts_focus_on_second_press() {
    // Backspace on the right pane with an empty drilldown stack
    // requires a confirmation press before backing out of the
    // pane entirely: the first press surfaces a hint, the second
    // performs the focus shift. This matches "press Backspace
    // twice to leave the right pane" UX.
    let mut app = app_for_explorer();
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    // First press: arms the shift and surfaces the hint.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.focus(), Focus::Right);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("press Backspace again")),
        "first backspace should surface the confirmation hint; got: {:?}",
        app.status_message()
    );
    // Second press: actually shifts focus, clears the hint.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.focus(), Focus::Left);
    assert!(app.status_message().is_none());
}

#[test]
fn explorer_back_armed_state_clears_on_intervening_message() {
    // The "press Backspace again" arming only survives across
    // consecutive Backspace presses. Any intervening message
    // (e.g. navigation, focus cycle) should reset it so the next
    // Backspace once again surfaces the hint instead of jumping
    // straight to the focus shift.
    let mut app = app_for_explorer();
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    app.update(Msg::ExplorerBack);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("press Backspace again"))
    );
    // Intervening navigation cancels the arming.
    app.update(Msg::ExplorerNavDown);
    // Next Backspace should re-arm, not shift focus.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.focus(), Focus::Right);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("press Backspace again"))
    );
}

#[test]
fn explorer_back_unwinds_drill_then_arms_then_shifts_focus() {
    // T8-031 follow-up: with one drilldown hop on the stack, three
    // Backspace taps now (1) pop the hop, (2) arm the focus shift
    // with a hint, and (3) shift focus to the left pane.
    let mut app = app_for_explorer();
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    let before = app.explorer().expect("state").view.focused.clone();
    app.update(Msg::ExplorerActivate);
    assert_ne!(app.explorer().expect("state").view.focused, before);
    // First backspace pops the drilldown hop; focus stays right.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.explorer().expect("state").view.focused, before);
    assert_eq!(app.focus(), Focus::Right);
    // Second backspace at the empty stack arms the focus shift.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.focus(), Focus::Right);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("press Backspace again"))
    );
    // Third backspace shifts focus to the left pane.
    app.update(Msg::ExplorerBack);
    assert_eq!(app.focus(), Focus::Left);
}

#[test]
fn edge_meta_visibility_defaults_to_run_config_value_and_toggles() {
    // T8-042: edge_meta_visible starts from `RunConfig.show_edge_meta`
    // and Msg::ToggleEdgeMeta flips it with a status hint.
    let app = App::new(RunConfig::defaults());
    assert!(
        !app.edge_meta_visible(),
        "RunConfig::defaults() should hide edge meta by default",
    );
    let mut config = RunConfig::defaults();
    config.show_edge_meta = true;
    let app_with_meta = App::new(config);
    assert!(
        app_with_meta.edge_meta_visible(),
        "config knob should set the initial state",
    );
    let mut app = app_for_explorer();
    assert!(!app.edge_meta_visible());
    app.update(Msg::ToggleEdgeMeta);
    assert!(app.edge_meta_visible());
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("edge meta visible"))
    );
    app.update(Msg::ToggleEdgeMeta);
    assert!(!app.edge_meta_visible());
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("edge meta hidden"))
    );
}

#[test]
fn explorer_toggle_full_detail_swaps_core_for_all_fields() {
    // T8-034: toggling Expanded Node Detail should swap the
    // Node-zone field rows for the per-kind `all_fields` set.
    // app_for_explorer focuses on an agent session, whose
    // all_fields is a superset of core_fields.
    let mut app = app_for_explorer();
    let core_count = app.explorer().expect("state").view.core_fields.len();
    let all_count = app.explorer().expect("state").view.all_fields.len();
    assert!(
        all_count > core_count,
        "test premise: agent_session should carry extras",
    );
    let rows_before = app.explorer().expect("state").rows();
    let node_field_count_before = rows_before
        .iter()
        .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
        .count();
    assert_eq!(node_field_count_before, core_count);

    app.update(Msg::ExplorerToggleFullDetail);
    assert!(app.explorer().expect("state").full_detail_expanded);
    let rows_after = app.explorer().expect("state").rows();
    let node_field_count_after = rows_after
        .iter()
        .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
        .count();
    assert_eq!(node_field_count_after, all_count);

    // Toggle back.
    app.update(Msg::ExplorerToggleFullDetail);
    assert!(!app.explorer().expect("state").full_detail_expanded);
    let rows_back = app.explorer().expect("state").rows();
    let node_field_count_back = rows_back
        .iter()
        .filter(|r| matches!(r, ExplorerRow::NodeField { .. }))
        .count();
    assert_eq!(node_field_count_back, core_count);
}

#[test]
fn explorer_full_detail_resets_on_drill_and_restores_on_backspace() {
    // T8-034: the toggle is per-focused-node — drilling into a
    // neighbor resets it, and Backspace restores the prior
    // node's toggle state.
    let mut app = app_for_explorer();
    // Turn on Expanded Detail on the original node.
    app.update(Msg::ExplorerToggleFullDetail);
    assert!(app.explorer().expect("state").full_detail_expanded);
    // Walk to the first link row and drill.
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    // Drilled state should default back to compact.
    assert!(!app.explorer().expect("state").full_detail_expanded);
    // Backspace should restore the prior toggle state.
    app.update(Msg::ExplorerBack);
    assert!(
        app.explorer().expect("state").full_detail_expanded,
        "Backspace should restore the prior node's Expanded Detail toggle",
    );
}

#[test]
fn explorer_toggle_group_only_acts_on_other_header() {
    // ADR 0074: toggling expansion only makes sense on the
    // `Other` zone header now that the per-relation sub-headers
    // are gone. Triggering the toggle from a validated link row
    // (or any other row kind) surfaces a status hint instead of
    // silently doing nothing.
    let mut app = app_for_explorer();
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerToggleGroup);
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("nothing to expand"))
    );
}

#[test]
fn explorer_nav_clamps_inside_flat_row_range() {
    let mut app = app_for_explorer();
    let len = app.explorer().expect("state").rows().len();
    for _ in 0..(len + 5) {
        app.update(Msg::ExplorerNavDown);
    }
    let cursor = app.explorer().expect("state").cursor;
    assert!(cursor < len.max(1));
    for _ in 0..(len + 5) {
        app.update(Msg::ExplorerNavUp);
    }
    assert_eq!(app.explorer().expect("state").cursor, 0);
}

#[test]
fn open_value_modal_when_cursor_has_a_long_value() {
    // Build an app focused on a session that carries a long
    // `last_message_preview` so the cursor walks to a row with
    // a long_value set.
    let snap = {
        let mut snap = GraphSnapshot::empty();
        let repo_id = RepoId::new("/p/proj");
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, "/p/proj".to_string()),
            root: "/p/proj".to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }));
        snap.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", "abc"),
                "claude-code".to_string(),
            )
            .with_cwd("/p/proj".to_string())
            .with_last_message_preview("a".repeat(120))
            .with_last_active_epoch(1_700_000_000),
        ));
        resolve_snapshot(snap)
    };
    let tree = build_tree(&snap);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("session row");
    app.set_selection(row_id);
    // The session's `last_message_preview` lives only in
    // `all_fields`, not `core_fields`. Without the full-node
    // toggle (T8-034) the cursor never lands on it through
    // navigation. For now we exercise the no-value branch.
    app.open_value_modal_for_cursor();
    assert!(app.value_modal().is_none());
    assert!(
        app.status_message()
            .is_some_and(|s| s.contains("no truncated"))
    );
}

#[test]
fn value_modal_close_clears_state() {
    let mut app = app_for_explorer();
    // Simulate an opened modal — exercise the close path.
    app.modal_stack.push(crate::tui::Modal::ValueModal(
        crate::tui::widgets::value_modal::ValueModalState::new("command", "long".to_string()),
    ));
    assert!(app.value_modal().is_some());
    app.close_value_modal();
    assert!(app.value_modal().is_none());
}

#[test]
fn scenario_process_cardinality_exposes_upstream_process_groups() {
    // T8-031: the process-cardinality dev scenario is the
    // canonical "messy" setup with one preferred process and one
    // candidate runner-up. With the new explorer, those should
    // both surface as Upstream groups on the agent session.
    let (mut app, _snap) = scenario_app("process-cardinality");
    let target = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("an agent session row in the scenario");
    app.set_selection(target);
    let state = app.explorer().expect("explorer for session");
    let labels: Vec<&str> = state
        .view
        .relationships
        .groups
        .iter()
        .filter(|g| g.direction == crate::tui::explorer::Direction::Upstream)
        .map(|g| g.relation.snake_case())
        .collect();
    assert!(
        labels.contains(&"process_identifies_session")
            || labels.contains(&"process_candidates_session"),
        "process-cardinality should expose process groups upstream: {labels:?}"
    );
}

#[test]
fn scenario_codex_fd_current_exposes_session_linked_groups() {
    // T8-031: codex-fd-current is the canonical "fd evidence
    // outranks stale launch command" setup. The detail explorer
    // should show the linked mux as a downstream group on the
    // agent session so an operator can drill into it manually.
    let (mut app, _snap) = scenario_app("codex-fd-current");
    let target = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("an agent session row in the scenario");
    app.set_selection(target);
    let state = app.explorer().expect("explorer for session");
    let downstream_kinds: Vec<&str> = state
        .view
        .relationships
        .groups
        .iter()
        .filter(|g| g.direction == crate::tui::explorer::Direction::Downstream)
        .map(|g| g.neighbor_kind.as_str())
        .collect();
    assert!(
        downstream_kinds.contains(&"mux_session"),
        "codex-fd-current should link the session to a mux downstream: {downstream_kinds:?}"
    );
}

#[test]
fn scenario_ambiguous_mux_exposes_two_candidate_muxes() {
    // T8-031: ambiguous-mux carries two plausible tmux sessions
    // for one agent. The explorer should surface both as
    // selectable rows in a single downstream group so operators
    // can drill into either candidate from the detail pane.
    let (mut app, _snap) = scenario_app("ambiguous-mux");
    let target = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(_) => Some(row.id.clone()),
            _ => None,
        })
        .expect("an agent session row in the scenario");
    app.set_selection(target);
    let state = app.explorer().expect("explorer for session");
    let total_mux_links: usize = state
        .view
        .relationships
        .groups
        .iter()
        .filter(|g| {
            g.direction == crate::tui::explorer::Direction::Downstream
                && g.neighbor_kind == "mux_session"
        })
        .map(|g| g.link_count())
        .sum();
    assert!(
        total_mux_links >= 2,
        "ambiguous-mux should surface two mux candidates in downstream groups; got {total_mux_links}"
    );
}

#[test]
fn explorer_state_resets_when_left_tree_selection_changes() {
    let mut app = app_for_explorer();
    let initial_focused = app.explorer().expect("state").view.focused.clone();
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate); // drill into mux
    let drilled_focused = app.explorer().expect("state").view.focused.clone();
    assert_ne!(initial_focused, drilled_focused);
    // Selecting a different row in the left tree should reset
    // the explorer to that new node — the right pane is the
    // detail surface for whatever the left pane points at.
    let snap = snapshot_session_with_mux();
    let other_session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "second"),
        "claude-code".to_string(),
    )
    .with_cwd("/p/proj".to_string())
    .with_last_active_epoch(1_700_000_000);
    let mut snap = snap;
    snap.nodes.push(GraphNode::AgentSession(other_session));
    let snap = resolve_snapshot(snap);
    let tree = build_tree(&snap);
    app.update(Msg::SetData {
        snapshot: GraphDb::from_snapshot(&snap),
        tree,
        loaded_at_epoch: 1_700_000_100,
        initial_selection_hint: None,
    });
    // Re-select the original session — explorer follows the
    // selection.
    let row_id = app
        .visible_rows()
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::AgentSession(session) if session.session.session_key == "abc" => {
                Some(row.id.clone())
            }
            _ => None,
        })
        .expect("first session row");
    app.set_selection(row_id);
    let after = app.explorer().expect("explorer after reselect");
    assert_eq!(after.view.focused, initial_focused);
    assert!(after.breadcrumb.is_empty());
}

// ------------------------------------------------------------------
// T8-040: Enter-to-copy on Node-zone field rows + `i` for full id.
// ------------------------------------------------------------------

#[test]
fn explorer_copy_target_returns_value_on_node_field_row() {
    let app = app_for_explorer();
    // Cursor defaults to the first Node-zone field row, which is
    // the focused agent session's `id` field.
    let (label, value) = app
        .explorer_copy_target()
        .expect("Node-zone field rows have a copy target");
    assert!(!label.is_empty(), "label must caption the toast");
    assert!(!value.is_empty(), "value must be the clipboard payload");
}

#[test]
fn explorer_copy_target_is_none_when_cursor_walks_onto_link_row() {
    // T8-040: Enter on link rows still drills; the copy seam must
    // refuse so the runtime falls through to ExplorerActivate.
    let mut app = app_for_explorer();
    let link_idx = app
        .explorer()
        .expect("explorer state present")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    assert!(app.explorer_copy_target().is_none());
}

#[test]
fn selected_session_id_returns_full_id_for_agent_session_row() {
    let app = app_for_explorer();
    let (label, value) = app
        .selected_session_id()
        .expect("agent session selection has a full id");
    assert_eq!(label, "copied: agent_session id");
    // Display form is `agent_session:<harness>:<scope>:<key>`
    // per NodeId / AgentSessionId — full id, not a short form.
    assert!(
        value.starts_with("agent_session:"),
        "expected full id prefix, got {value:?}",
    );
}

#[test]
fn selected_session_id_is_none_on_non_session_selection() {
    let mut app = app_for_explorer();
    // Drop the selection so we cover the "nothing selected" arm
    // (which is the same as a non-session selection from the
    // runtime's perspective).
    app.selection = None;
    assert!(app.selected_session_id().is_none());
}

#[test]
fn post_toast_supersedes_prior_toast() {
    // H-WIDG-003 contract: posting a new toast drains any prior
    // queued toast so the newer feedback is the one rendered.
    // Under the upstream engine the queue length stays at 1
    // after a second post even though the engine itself supports
    // queueing — `engine_dismiss_all` runs before each show.
    let mut app = app_for_explorer();
    assert!(!app.toast().has_toast());
    app.post_toast("copied: cwd");
    assert!(app.toast().has_toast());
    assert_eq!(app.toast().queue_len(), 1);
    app.post_toast("copied: id");
    assert_eq!(
        app.toast().queue_len(),
        1,
        "newer toast must drain the queue"
    );
    assert_eq!(app.toast().current_message(), Some("copied: id"));
}

#[test]
fn switch_view_msg_persists_through_enabled_cache() {
    // F8-013: when persistence is enabled, every view switch must
    // funnel through `crate::tui_state::write_tui_state`. Phase E
    // moves the persist call out of `switch_to_view` and into the
    // `Msg::SwitchView` reducer arm (via `Effect::Persist`), so we
    // pin the seam by dispatching the Msg and running the effects
    // executor.
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
    let cache_for_assert = cache.clone();

    let mut app = App::new(RunConfig::defaults());
    app.enable_view_persistence(cache);
    let effects = app.update(Msg::SwitchView(View::Mux));
    crate::tui::runtime::execute_effects(&mut app, effects);

    assert_eq!(
        crate::tui_state::read_last_view(&cache_for_assert),
        Some(View::Mux),
        "SwitchView Msg must emit Persist and land the state on disk",
    );
}

#[test]
fn restore_persisted_state_applies_state_and_mirrors_config() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
    let mut persisted = crate::tui_state::PersistedState {
        last_view: Some(View::Sessions),
        sort: Some(crate::tui::Sort::Recency),
        mux_recency: None,
        view_states: BTreeMap::new(),
    };
    let filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    };
    persisted.view_states.insert(
        View::Sessions,
        crate::tui_state::PersistedViewSlot {
            filter: filter.clone(),
            grouping: Some(crate::tui::Grouping::Sessions(
                crate::tui::SessionsGrouping::Repo,
            )),
        },
    );
    crate::tui_state::write_tui_state(&cache, &persisted).expect("seed state");

    let mut app = App::new(RunConfig::defaults());
    app.enable_view_persistence(cache);
    app.restore_persisted_state();

    assert_eq!(app.sort(), crate::tui::Sort::Recency);
    assert_eq!(app.filter(), &filter);
    assert_eq!(
        app.grouping(),
        crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Repo)
    );
}

#[test]
fn restore_persisted_state_preserves_explicit_cli_overrides() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());
    let persisted_filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    };
    let mut persisted = crate::tui_state::PersistedState {
        last_view: Some(View::Sessions),
        sort: Some(crate::tui::Sort::Recency),
        mux_recency: None,
        view_states: BTreeMap::new(),
    };
    persisted.view_states.insert(
        View::Sessions,
        crate::tui_state::PersistedViewSlot {
            filter: persisted_filter,
            grouping: Some(crate::tui::Grouping::Sessions(
                crate::tui::SessionsGrouping::Repo,
            )),
        },
    );
    crate::tui_state::write_tui_state(&cache, &persisted).expect("seed state");

    let cli_filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
        ..crate::filter::RowFilter::default()
    };
    let mut config = RunConfig::defaults();
    config.default_sort = crate::tui::Sort::Hierarchy;
    config.initial_filter = cli_filter.clone();
    config.sessions_grouping = crate::tui::SessionsGrouping::Workspace;
    config.explicit_sort = true;
    config.explicit_filter = true;
    config.explicit_grouping = true;

    let mut app = App::new(config);
    app.enable_view_persistence(cache);
    app.restore_persisted_state();

    assert_eq!(app.sort(), crate::tui::Sort::Hierarchy);
    assert_eq!(app.filter(), &cli_filter);
    assert_eq!(
        app.grouping(),
        crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Workspace)
    );
}

#[test]
fn switch_view_msg_without_persistence_does_not_write() {
    // Negative pin: when persistence is disabled (the default,
    // matching snapshot mode and `--no-resume-view`), the on-disk
    // file must not appear even after the effect executor runs.
    // Guards against accidental unconditional writes leaking into
    // snapshot tests. Phase E routes the persist through the
    // reducer + executor, so we drive the flow the same way.
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::tui_state::TuiStateCache::default().with_xdg_state_home(dir.path());

    let mut app = App::new(RunConfig::defaults());
    // Deliberately skip `enable_view_persistence`.
    let effects = app.update(Msg::SwitchView(View::Prs));
    crate::tui::runtime::execute_effects(&mut app, effects);

    assert!(
        crate::tui_state::read_last_view(&cache).is_none(),
        "view switch without enabled persistence must not write",
    );
}

// ADR 0085 contract 3 (H-TUI-003 phase 1): the modal stack is
// the sole open-overlay tracker for migrated overlays. These
// tests pin the Help overlay's push/pop shape so a future
// migration wave can trust the same pattern.
mod modal_stack {
    use super::*;
    use crate::tui::Modal;

    #[test]
    fn open_help_pushes_modal_help_onto_stack() {
        let mut app = App::new(RunConfig::defaults());
        assert!(app.modal_stack().is_empty());
        app.open_help_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Help(_))));
        assert!(app.help_overlay().is_some());
    }

    #[test]
    fn close_help_pops_when_top_is_help() {
        let mut app = App::new(RunConfig::defaults());
        app.open_help_overlay();
        app.close_help_overlay();
        assert!(app.modal_stack().is_empty());
        assert!(app.help_overlay().is_none());
    }

    #[test]
    fn close_help_when_stack_empty_is_a_no_op() {
        let mut app = App::new(RunConfig::defaults());
        app.close_help_overlay();
        assert!(app.modal_stack().is_empty());
    }

    #[test]
    fn help_overlay_esc_key_returns_close_via_overlay_trait() {
        use crate::tui::widgets::help::HelpOverlayState;
        use crate::tui::{Overlay, OverlayOutcome};
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = HelpOverlayState::new();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(state.handle((), key), OverlayOutcome::Close);
    }

    #[test]
    fn help_overlay_scroll_key_returns_consumed_via_overlay_trait() {
        use crate::tui::widgets::help::HelpOverlayState;
        use crate::tui::{Overlay, OverlayOutcome};
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = HelpOverlayState::new();
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(state.handle((), key), OverlayOutcome::Consumed);
    }

    // H-TUI-003 wave 3: controls overlay lives on the same
    // modal stack. Doesn't implement the Overlay trait yet
    // (needs live ControlsContext), but push/pop invariants
    // match Help's.
    #[test]
    fn open_controls_pushes_modal_controls_onto_stack() {
        let mut app = App::new(RunConfig::defaults());
        assert!(app.modal_stack().is_empty());
        app.open_controls_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(
            app.modal_stack().first(),
            Some(Modal::Controls(_))
        ));
        assert!(app.controls_overlay().is_some());
    }

    #[test]
    fn close_controls_pops_when_top_is_controls() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.close_controls_overlay();
        assert!(app.modal_stack().is_empty());
        assert!(app.controls_overlay().is_none());
    }

    #[test]
    fn controls_and_help_stack_together_with_correct_top() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_help_overlay();
        assert_eq!(app.modal_stack().len(), 2);
        assert!(app.help_overlay().is_some());
        // controls_overlay() peeks the top — Help is on top,
        // not Controls, so this must return None even though
        // Modal::Controls is somewhere in the stack.
        assert!(app.controls_overlay().is_none());
    }

    #[test]
    fn close_help_leaves_controls_on_stack() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_help_overlay();
        app.close_help_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(app.controls_overlay().is_some());
        assert!(app.help_overlay().is_none());
    }

    #[test]
    fn close_controls_is_no_op_when_help_is_on_top() {
        // Guards against a naive "pop the top" close impl —
        // close_controls_overlay must only pop when the top
        // variant matches, otherwise the operator could Esc
        // Help and accidentally close Controls too.
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_help_overlay();
        app.close_controls_overlay();
        assert_eq!(app.modal_stack().len(), 2);
        assert!(app.help_overlay().is_some());
    }

    // H-TUI-003 wave 4: pins overlay on the same stack.
    #[test]
    fn open_pins_pushes_modal_pins_onto_stack() {
        let mut app = App::new(RunConfig::defaults());
        app.open_pins_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Pins(_))));
        assert!(app.pins_overlay().is_some());
    }

    #[test]
    fn set_pins_overlay_pushes_preconfigured_state() {
        use crate::tui::widgets::pins::PinsOverlayState;
        let mut app = App::new(RunConfig::defaults());
        app.set_pins_overlay(PinsOverlayState::new());
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Pins(_))));
    }

    #[test]
    fn close_pins_pops_when_top_is_pins() {
        let mut app = App::new(RunConfig::defaults());
        app.open_pins_overlay();
        app.close_pins_overlay();
        assert!(app.modal_stack().is_empty());
        assert!(app.pins_overlay().is_none());
    }

    #[test]
    fn triple_stack_orders_correctly_and_pops_lifo() {
        // Controls at bottom, Pins in middle, Help on top.
        // Peek accessors report the exact top variant only;
        // popping in reverse order reveals each in turn.
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_pins_overlay();
        app.open_help_overlay();
        assert_eq!(app.modal_stack().len(), 3);
        assert!(app.help_overlay().is_some());
        assert!(app.pins_overlay().is_none());
        assert!(app.controls_overlay().is_none());

        app.close_help_overlay();
        assert_eq!(app.modal_stack().len(), 2);
        assert!(app.help_overlay().is_none());
        assert!(app.pins_overlay().is_some());
        assert!(app.controls_overlay().is_none());

        app.close_pins_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(app.pins_overlay().is_none());
        assert!(app.controls_overlay().is_some());

        app.close_controls_overlay();
        assert!(app.modal_stack().is_empty());
    }

    // H-TUI-003 wave 5: rename, search, value_modal on the
    // same stack. ValueModal implements the Overlay trait
    // (its Continue/Close outcomes map cleanly to
    // Consumed/Close); rename and search stay with their
    // specialized dispatchers.
    #[test]
    fn open_rename_pushes_modal_rename_onto_stack() {
        use crate::tui::widgets::input::TextInputState;
        let mut app = App::new(RunConfig::defaults());
        app.open_rename_overlay(TextInputState::new("rename", ""));
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Rename(_))));
        assert!(app.rename_overlay().is_some());
    }

    #[test]
    fn close_rename_pops_when_top_is_rename() {
        use crate::tui::widgets::input::TextInputState;
        let mut app = App::new(RunConfig::defaults());
        app.open_rename_overlay(TextInputState::new("rename", ""));
        app.close_rename_overlay();
        assert!(app.modal_stack().is_empty());
        assert!(app.rename_overlay().is_none());
    }

    #[test]
    fn open_search_pushes_modal_search_onto_stack() {
        let mut app = App::new(RunConfig::defaults());
        app.open_search_overlay();
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Search(_))));
        assert!(app.search_overlay().is_some());
    }

    #[test]
    fn close_search_pops_when_top_is_search() {
        let mut app = App::new(RunConfig::defaults());
        app.open_search_overlay();
        app.close_search_overlay();
        assert!(app.modal_stack().is_empty());
        assert!(app.search_overlay().is_none());
    }

    #[test]
    fn value_modal_esc_key_returns_close_via_overlay_trait() {
        use crate::tui::widgets::value_modal::ValueModalState;
        use crate::tui::{Overlay, OverlayOutcome};
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ValueModalState::new("cwd", "/very/long/path".to_string());
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(state.handle((), key), OverlayOutcome::Close);
    }

    #[test]
    fn value_modal_scroll_key_returns_consumed_via_overlay_trait() {
        use crate::tui::widgets::value_modal::ValueModalState;
        use crate::tui::{Overlay, OverlayOutcome};
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ValueModalState::new("cwd", "/very/long/path".to_string());
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(state.handle((), key), OverlayOutcome::Consumed);
    }

    #[test]
    fn five_wide_stack_orders_and_pops_lifo() {
        // Bottom to top: Controls, Pins, Search, Rename, Help.
        // (ValueModal covered separately since it exercises
        // the Overlay trait path.) Peek accessors report the
        // exact top variant only; pop reveals each in turn.
        use crate::tui::widgets::input::TextInputState;
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();
        app.open_pins_overlay();
        app.open_search_overlay();
        app.open_rename_overlay(TextInputState::new("rename", ""));
        app.open_help_overlay();
        assert_eq!(app.modal_stack().len(), 5);
        assert!(app.help_overlay().is_some());
        assert!(app.rename_overlay().is_none());

        app.close_help_overlay();
        assert!(app.rename_overlay().is_some());

        app.close_rename_overlay();
        assert!(app.search_overlay().is_some());

        app.close_search_overlay();
        assert!(app.pins_overlay().is_some());

        app.close_pins_overlay();
        assert!(app.controls_overlay().is_some());

        app.close_controls_overlay();
        assert!(app.modal_stack().is_empty());
    }

    // H-TUI-003 wave 7: viewer_modal migrates as a nested
    // reducer entry. Msg::Viewer(ViewerMsg) is the App-level
    // wrapper; the reducer arm pops, delegates to
    // viewer::input::reduce, and pushes back or leaves
    // popped based on the returned ViewerEffect.
    #[test]
    fn open_viewer_pushes_modal_viewer_onto_stack() {
        use crate::viewer::model::TranscriptDocument;
        use crate::viewer::state::ViewerState;
        let mut app = App::new(RunConfig::defaults());
        app.open_viewer_modal(ViewerState::new(TranscriptDocument::default()));
        assert_eq!(app.modal_stack().len(), 1);
        assert!(matches!(app.modal_stack().first(), Some(Modal::Viewer(_))));
        assert!(app.viewer_modal().is_some());
    }

    #[test]
    fn viewer_msg_close_pops_stack_and_sets_status() {
        use crate::viewer::input::ViewerMsg;
        use crate::viewer::model::TranscriptDocument;
        use crate::viewer::state::ViewerState;
        let mut app = App::new(RunConfig::defaults());
        app.open_viewer_modal(ViewerState::new(TranscriptDocument::default()));
        let effects = app.update(Msg::Viewer(ViewerMsg::Close));
        assert!(effects.is_empty());
        assert!(app.modal_stack().is_empty());
        assert_eq!(app.status_message(), Some("viewer closed"));
    }

    #[test]
    fn viewer_msg_scroll_leaves_modal_on_top() {
        use crate::viewer::input::ViewerMsg;
        use crate::viewer::model::TranscriptDocument;
        use crate::viewer::state::ViewerState;
        let mut app = App::new(RunConfig::defaults());
        app.open_viewer_modal(ViewerState::new(TranscriptDocument::default()));
        let effects = app.update(Msg::Viewer(ViewerMsg::ScrollDown));
        assert!(effects.is_empty());
        assert!(app.viewer_modal().is_some());
    }

    #[test]
    fn viewer_msg_without_open_viewer_is_a_no_op() {
        use crate::viewer::input::ViewerMsg;
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::Viewer(ViewerMsg::Close));
        assert!(effects.is_empty());
        assert!(app.modal_stack().is_empty());
    }

    #[test]
    fn viewer_msg_with_help_on_top_leaves_viewer_underneath_untouched() {
        use crate::viewer::input::ViewerMsg;
        use crate::viewer::model::TranscriptDocument;
        use crate::viewer::state::ViewerState;
        let mut app = App::new(RunConfig::defaults());
        app.open_viewer_modal(ViewerState::new(TranscriptDocument::default()));
        app.open_help_overlay();
        // Msg::Viewer must only fire when Viewer is on top.
        // Help is on top now, so the reducer no-ops.
        let effects = app.update(Msg::Viewer(ViewerMsg::Close));
        assert!(effects.is_empty());
        assert_eq!(app.modal_stack().len(), 2);
        assert!(app.help_overlay().is_some());
    }
}

// ADR 0085 contract 2: the reducer emits Effects as data. These
// tests pin the initial catalog — Msg::Quit yields Effect::Quit,
// navigation Msgs yield none — so a future PR that accidentally
// stops emitting an effect fails a fast unit test rather than a
// slow integration test.
mod reducer_effects {
    use super::*;
    use crate::tui::Effect;

    #[test]
    fn quit_msg_emits_quit_effect_and_sets_should_quit() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::Quit);
        assert_eq!(effects, vec![Effect::Quit]);
        assert!(app.should_quit());
    }

    #[test]
    fn nav_msgs_emit_no_effects() {
        let mut app = App::new(RunConfig::defaults());
        assert!(app.update(Msg::NavDown).is_empty());
        assert!(app.update(Msg::NavUp).is_empty());
        assert!(app.update(Msg::Home).is_empty());
        assert!(app.update(Msg::End).is_empty());
        assert!(app.update(Msg::CycleFocus).is_empty());
    }

    #[test]
    fn set_status_emits_no_effects() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::SetStatus(Some("hi".to_string())));
        assert!(effects.is_empty());
    }

    // ADR 0085 contract 2: terminal-suspending exec resolves in
    // the reducer and comes back as `Effect::Exec(...)`. The
    // executor is the only code that touches `tmux` or
    // `std::process`, and never gets involved here — these tests
    // assert the (state', effects) shape without a terminal in
    // the loop.
    #[test]
    fn attach_selected_with_no_selection_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::AttachSelected);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::Toast(reason) => assert!(
                reason.starts_with("attach: "),
                "expected attach disabled reason, got {reason:?}",
            ),
            other => panic!("expected Toast, got {other:?}"),
        }
    }

    #[test]
    fn resume_selected_with_no_selection_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::ResumeSelected);
        assert_eq!(
            effects,
            vec![Effect::Toast("resume: nothing selected".to_string())]
        );
    }

    #[test]
    fn resume_selected_on_supported_harness_emits_exec() {
        use crate::tui::effect::ExecSpec;
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::ResumeSelected);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::Exec(ExecSpec::Resume(crate::tui::resume::ResumeTarget::Launch {
                label,
                ..
            })) => {
                assert_eq!(label, "Session One");
            }
            other => panic!("expected Exec(Resume(Launch)), got {other:?}"),
        }
    }

    #[test]
    fn resume_selected_on_unsupported_harness_emits_toast() {
        let mut app = seeded_app(&[("aider", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::ResumeSelected);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::Toast(reason) => assert!(
                reason.contains("aider"),
                "expected aider-unsupported reason, got {reason:?}",
            ),
            other => panic!("expected Toast, got {other:?}"),
        }
    }

    #[test]
    fn view_selected_with_no_selection_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::ViewSelected);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::Toast(_)));
    }

    #[test]
    fn view_selected_on_agent_session_emits_exec() {
        use crate::tui::effect::ExecSpec;
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::ViewSelected);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::Exec(ExecSpec::ViewSession(session_id)) => {
                assert_eq!(session_id.harness_key, "codex");
            }
            other => panic!("expected Exec(ViewSession), got {other:?}"),
        }
    }

    #[test]
    fn launch_selected_pin_with_no_selection_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::LaunchSelectedPin);
        assert_eq!(
            effects,
            vec![Effect::Toast("launch: nothing selected".to_string())]
        );
    }

    #[test]
    fn launch_selected_pin_on_non_pin_row_emits_toast() {
        // A plain agent-session row with no pin_id — nothing to
        // launch. The reducer surfaces the operator-facing hint
        // instead of firing Exec.
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::LaunchSelectedPin);
        assert_eq!(
            effects,
            vec![Effect::Toast(
                "launch: select a pin or placeholder row".to_string(),
            )]
        );
    }

    #[test]
    fn pin_remove_msg_emits_write_store_effect() {
        use crate::tui::effect::StoreOp;
        let mut app = App::new(RunConfig::defaults());
        let request = crate::tui::widgets::pins::PinRemoveRequest {
            id: "work".to_string(),
            display_name: "work".to_string(),
            store_path: "/tmp/.conspectus.toml".to_string(),
        };
        let effects = app.update(Msg::PinRemove(request.clone()));
        assert_eq!(
            effects,
            vec![Effect::WriteStore(StoreOp::PinRemove(request))]
        );
    }

    #[test]
    fn pin_bind_without_snapshot_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let request = crate::tui::widgets::pins::PinBindRequest {
            pin_id: "work".to_string(),
            session_key: "abc".to_string(),
        };
        let effects = app.update(Msg::PinBind(request));
        assert_eq!(
            effects,
            vec![Effect::Toast(
                "pin bind failed: no graph database available".to_string()
            )]
        );
    }

    #[test]
    fn pin_bind_with_snapshot_emits_write_store_effect() {
        use crate::tui::effect::StoreOp;
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        let request = crate::tui::widgets::pins::PinBindRequest {
            pin_id: "work".to_string(),
            session_key: "abc".to_string(),
        };
        let effects = app.update(Msg::PinBind(request.clone()));
        assert_eq!(effects, vec![Effect::WriteStore(StoreOp::PinBind(request))]);
    }

    #[test]
    fn pin_create_msg_emits_write_store_effect() {
        use crate::tui::effect::StoreOp;
        let mut app = App::new(RunConfig::defaults());
        let request = crate::tui::widgets::pins::PinCreateRequest {
            id: "work".to_string(),
            display_name: "work".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux_name: "work".to_string(),
            mux_socket: None,
            adopt_source_mux_name: None,
            launch_argv: Vec::new(),
            store: crate::tui::widgets::pins::PinCreateStore::Auto,
        };
        let effects = app.update(Msg::PinCreate(request.clone()));
        assert_eq!(
            effects,
            vec![Effect::WriteStore(StoreOp::PinCreate(request))]
        );
    }

    #[test]
    fn pin_edit_msg_emits_write_store_effect() {
        use crate::tui::effect::StoreOp;
        let mut app = App::new(RunConfig::defaults());
        let request = crate::tui::widgets::pins::PinEditRequest {
            original_id: "work".to_string(),
            id: "work-2".to_string(),
            display_name: "Work Two".to_string(),
            harness: "codex".to_string(),
            cwd: "/p/project".to_string(),
            mux_name: "work-2".to_string(),
            mux_socket: None,
            launch_argv: Vec::new(),
            store_path: "/tmp/.conspectus.toml".to_string(),
        };
        let effects = app.update(Msg::PinEdit(request.clone()));
        assert_eq!(effects, vec![Effect::WriteStore(StoreOp::PinEdit(request))]);
    }

    #[test]
    fn commit_rename_with_agent_session_selection_emits_alias_rename() {
        use crate::tui::effect::StoreOp;
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::CommitRename("New Alias".to_string()));
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::WriteStore(StoreOp::CommitAliasRename {
                session_id,
                new_display_name,
            }) => {
                assert_eq!(session_id.harness_key, "codex");
                assert_eq!(new_display_name.as_deref(), Some("New Alias"));
            }
            other => panic!("expected CommitAliasRename, got {other:?}"),
        }
    }

    #[test]
    fn commit_rename_with_empty_value_on_agent_session_clears_alias() {
        use crate::tui::effect::StoreOp;
        let mut app = seeded_app(&[("codex", "Session One", "/p/project")]);
        select_session(&mut app, "Session One");
        let effects = app.update(Msg::CommitRename("   ".to_string()));
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::WriteStore(StoreOp::CommitAliasRename {
                new_display_name, ..
            }) => {
                assert!(new_display_name.is_none());
            }
            other => panic!("expected CommitAliasRename, got {other:?}"),
        }
    }

    #[test]
    fn commit_rename_without_selection_emits_toast() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::CommitRename("value".to_string()));
        assert_eq!(
            effects,
            vec![Effect::Toast(
                "rename: lost selection before commit".to_string()
            )]
        );
    }

    fn app_with_mux_selection(mux_name: &str) -> (App, crate::model::MuxSessionId) {
        let mut snapshot = crate::model::GraphSnapshot::empty();
        let mux_id = crate::model::MuxSessionId::new(mux_name);
        snapshot.nodes.push(crate::model::GraphNode::MuxSession(
            crate::model::MuxSessionNode::new(
                mux_id.clone(),
                "tmux".to_string(),
                mux_name.to_string(),
            ),
        ));
        let resolved = crate::resolve::resolve_snapshot(snapshot);
        let mut app = App::new(RunConfig::defaults());
        // Build the tree in the mux view so a mux row appears.
        app.update(Msg::SwitchView(View::Mux));
        let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
            &resolved, &app,
        ));
        app.update(Msg::SetData {
            snapshot: crate::tui::app::GraphDb::new(resolved),
            tree,
            loaded_at_epoch: 0,
            initial_selection_hint: None,
        });
        app.set_selection(crate::tui::rows::RowId::MuxSession(
            crate::model::NodeId::MuxSession(mux_id.clone()),
        ));
        (app, mux_id)
    }

    #[test]
    fn commit_rename_with_mux_selection_emits_commit_mux_rename() {
        use crate::tui::effect::StoreOp;
        let (mut app, mux_id) = app_with_mux_selection("editor");
        let effects = app.update(Msg::CommitRename("workshop".to_string()));
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::WriteStore(StoreOp::CommitMuxRename {
                mux_id: m,
                new_name,
            }) => {
                assert_eq!(m, &mux_id);
                assert_eq!(new_name, "workshop");
            }
            other => panic!("expected CommitMuxRename, got {other:?}"),
        }
    }

    #[test]
    fn commit_rename_with_empty_value_on_mux_emits_toast() {
        let (mut app, _mux_id) = app_with_mux_selection("editor");
        let effects = app.update(Msg::CommitRename("   ".to_string()));
        assert_eq!(
            effects,
            vec![Effect::Toast(
                "mux rename: name cannot be empty".to_string()
            )]
        );
    }

    // ADR 0085 contracts 1 + 4 (H-TUI-002 Phase F): projection
    // changes now flow through the reducer as first-class Msgs
    // instead of the runtime's `ControlsAction` bridge. The
    // arms mutate App state and re-derive the row tree from the
    // held snapshot in one shot; no effects are emitted (the
    // tree swap happens inline).
    // Phase E (ADR 0085 contract 2): view / grouping / filter /
    // sort switches emit `Effect::Persist` so the F8-013 sidecar
    // stays in sync mid-session. Pre-Phase-E these arms emitted
    // no effects and relied on the runtime shutdown fallback for
    // persistence — see `runtime.rs`'s shutdown persist for the
    // pre-Phase-E path that Phase E retires from the happy path.

    #[test]
    fn switch_view_msg_updates_active_view_and_emits_persist() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::SwitchView(View::Mux));
        assert_eq!(app.active_view(), View::Mux);
        assert_eq!(effects, vec![Effect::Persist]);
    }

    #[test]
    fn switch_view_msg_to_current_view_is_noop_and_emits_no_effect() {
        let mut app = App::new(RunConfig::defaults());
        let before = app.active_view();
        let effects = app.update(Msg::SwitchView(before));
        assert_eq!(app.active_view(), before);
        assert!(effects.is_empty(), "same-view switch is a no-op");
    }

    #[test]
    fn set_grouping_msg_updates_grouping_and_emits_persist() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
            crate::tui::SessionsGrouping::Workspace,
        )));
        assert_eq!(
            app.grouping(),
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::Workspace)
        );
        assert_eq!(effects, vec![Effect::Persist]);
    }

    #[test]
    fn set_filter_msg_updates_filter_and_emits_persist() {
        let mut app = App::new(RunConfig::defaults());
        let filter = crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
            ..crate::filter::RowFilter::default()
        };
        let effects = app.update(Msg::SetFilter(filter.clone()));
        assert_eq!(app.filter(), &filter);
        assert_eq!(effects, vec![Effect::Persist]);
    }

    #[test]
    fn set_sort_msg_updates_sort_and_emits_persist() {
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::SetSort(crate::tui::Sort::Recency));
        assert_eq!(app.sort(), crate::tui::Sort::Recency);
        assert_eq!(effects, vec![Effect::Persist]);
    }

    // H-WIDG-007: in-flight-ops substrate. The reducer stores one
    // op per `InFlightKind`; start with the same kind twice replaces
    // the prior record; finish removes the matching kind.

    #[test]
    fn in_flight_start_adds_op_and_emits_no_effect() {
        use crate::tui::app::InFlightKind;
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::InFlightStart {
            kind: InFlightKind::Discovery,
            label: "Discovering".to_string(),
        });
        assert_eq!(app.in_flight_ops().len(), 1);
        assert_eq!(app.in_flight_ops()[0].kind, InFlightKind::Discovery);
        assert_eq!(app.in_flight_ops()[0].label, "Discovering");
        assert!(effects.is_empty());
    }

    #[test]
    fn in_flight_start_same_kind_replaces_prior_record() {
        use crate::tui::app::InFlightKind;
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::InFlightStart {
            kind: InFlightKind::Discovery,
            label: "First".to_string(),
        });
        app.update(Msg::InFlightStart {
            kind: InFlightKind::Discovery,
            label: "Second".to_string(),
        });
        assert_eq!(app.in_flight_ops().len(), 1);
        assert_eq!(app.in_flight_ops()[0].label, "Second");
    }

    #[test]
    fn in_flight_finish_removes_matching_kind_and_emits_no_effect() {
        use crate::tui::app::InFlightKind;
        let mut app = App::new(RunConfig::defaults());
        app.update(Msg::InFlightStart {
            kind: InFlightKind::Discovery,
            label: "Discovering".to_string(),
        });
        let effects = app.update(Msg::InFlightFinish(InFlightKind::Discovery));
        assert!(app.in_flight_ops().is_empty());
        assert!(effects.is_empty());
    }

    #[test]
    fn in_flight_finish_unknown_kind_is_noop() {
        use crate::tui::app::InFlightKind;
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::InFlightFinish(InFlightKind::Discovery));
        assert!(app.in_flight_ops().is_empty());
        assert!(effects.is_empty());
    }

    #[test]
    fn launch_pin_by_id_without_snapshot_emits_exec_with_no_attach_target() {
        use crate::tui::effect::ExecSpec;
        let mut app = App::new(RunConfig::defaults());
        let effects = app.update(Msg::LaunchPinById("work".to_string()));
        assert_eq!(
            effects,
            vec![Effect::Exec(ExecSpec::LaunchPin {
                pin_id: "work".to_string(),
                attach_target: None,
            })]
        );
    }
}
