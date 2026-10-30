use super::*;
use crate::filter::RowFilter;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId,
    RepoNode,
};
use crate::resolve::resolve_snapshot;
use crate::tui::app::{Msg, SnapshotHandle};
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
use crate::tui::{RunConfig, SessionsGrouping, View};

fn build_app(snapshot: GraphSnapshot) -> App {
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: None,
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut cfg = RunConfig::defaults();
    cfg.default_view = View::Sessions;
    let mut app = App::new(cfg);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn session_node(harness: &str, scope: &str, key: &str, cwd: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new(harness, scope, key),
            harness.to_string(),
        )
        .with_cwd(cwd.to_string()),
    )
}

/// Mirror real-world `TmuxDiscovery` shape: the node's `id`
/// is backend-prefixed (`tmux:editor`) while `native_id` is
/// the raw session name (`editor`).
fn mux_node(backend: &str, native: &str) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode::new(
        MuxSessionId::new(format!("{backend}:{native}")),
        backend.to_string(),
        native.to_string(),
    ))
}

fn linked_to_mux(
    session: &NodeId,
    mux: &NodeId,
    provenance: Provenance,
    suffix: &str,
) -> GraphLink {
    GraphLink {
        id: format!("session-mux-{suffix}"),
        source: session.clone(),
        target: LinkEndpoint::Node { id: mux.clone() },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn add_repo_and_worktree(snapshot: &mut GraphSnapshot, common_dir: &str) {
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(common_dir))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(common_dir), common_dir.to_string()),
        root: common_dir.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
}

#[test]
fn empty_app_reports_no_selection() {
    let app = App::new(RunConfig::defaults());
    assert_eq!(
        resolve_attach_target(&app),
        Err(AttachDisabled::NoSelection)
    );
}

#[test]
fn unmuxed_session_reports_unmuxed_disabled() {
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    let mut app = build_app(snapshot);
    // Step past the repo group to the session row.
    app.update(Msg::NavDown);
    assert_eq!(
        resolve_attach_target(&app),
        Err(AttachDisabled::UnmuxedSession)
    );
}

#[test]
fn group_row_with_no_ambiguous_mux_reports_unmuxed() {
    // ADR 0071: a workspace/repo/checkout row with no ambiguous
    // mux in scope falls through to the same UnmuxedSession
    // reason an unmuxed session row would. The earlier
    // UnsupportedRow result is gone because group rows now have
    // a meaningful attach semantic.
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    let app = build_app(snapshot);
    assert_eq!(
        resolve_attach_target(&app),
        Err(AttachDisabled::UnmuxedSession)
    );
}

/// Mux node id matching the prefixed form `mux_node` emits.
fn mux_node_id(backend: &str, native: &str) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(format!("{backend}:{native}")))
}

#[test]
fn muxed_session_resolves_to_preferred_mux_target() {
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = mux_node_id("tmux", "editor");
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));
    let mut app = build_app(snapshot);
    app.update(Msg::NavDown);
    let target = resolve_attach_target(&app).expect("muxed session attachable");
    assert_eq!(target.backend, "tmux");
    assert_eq!(
        target.mux.native_id, "tmux:editor",
        "graph id retains the backend prefix"
    );
    assert_eq!(
        target.native_id, "editor",
        "raw native id strips the prefix — this is what tmux attach -t takes"
    );
}

#[test]
fn current_tmux_session_is_not_attachable() {
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = mux_node_id("tmux", "editor");
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: None,
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut cfg = RunConfig::defaults();
    cfg.default_view = View::Sessions;
    cfg.current_tmux_session = Some("editor".to_string());
    let mut app = App::new(cfg);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::NavDown);

    assert_eq!(
        resolve_attach_target(&app),
        Err(AttachDisabled::CurrentTmuxSession("editor".to_string()))
    );
}

#[test]
fn ambiguous_session_attaches_to_resolver_preferred_candidate() {
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.nodes.push(mux_node("tmux", "scratch"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let editor = mux_node_id("tmux", "editor");
    let scratch = mux_node_id("tmux", "scratch");
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &editor,
        Provenance::StrongDiscovered,
        "1",
    ));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &scratch,
        Provenance::Discovered,
        "2",
    ));
    let mut app = build_app(snapshot);
    app.update(Msg::NavDown);
    let target = resolve_attach_target(&app).expect("ambiguous session attachable");
    assert_eq!(
        target.native_id, "editor",
        "preferred (StrongDiscovered) candidate wins; raw native id forwarded"
    );
}

#[test]
fn group_row_with_multiple_ambiguous_muxes_reports_ambiguous() {
    // ADR 0071: with two muxes ambiguously claimed by the same
    // session, `a` on the parent repo no longer has a unique
    // default pick. The renderer surfaces the count as a hint.
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot.nodes.push(mux_node("tmux", "scratch"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let checkout_id = NodeId::Checkout(crate::model::CheckoutId::new(
        RepoId::new("/p/proj"),
        "/p/proj",
    ));
    snapshot.candidate_links.push(GraphLink {
        id: "assoc".to_string(),
        source: session_id.clone(),
        target: crate::model::LinkEndpoint::Node { id: checkout_id },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: crate::model::Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: crate::model::LinkState::Active,
    });
    let editor = mux_node_id("tmux", "editor");
    let scratch = mux_node_id("tmux", "scratch");
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &editor,
        Provenance::Discovered,
        "1",
    ));
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &scratch,
        Provenance::Discovered,
        "2",
    ));
    let app = build_app(snapshot);
    // Auto-selection lands on the repo group row. Two ambiguous
    // muxes in scope → the new variant fires with the count.
    match resolve_attach_target(&app) {
        Err(AttachDisabled::AmbiguousGroupMuxes(n)) => assert_eq!(n, 2),
        other => panic!("expected AmbiguousGroupMuxes(2), got {other:?}"),
    }
}

#[test]
fn mux_view_mux_row_resolves_attach_target() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("tmux", "editor"));
    let snapshot = resolve_snapshot(snapshot);
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut cfg = RunConfig::defaults();
    cfg.default_view = View::Mux;
    let mut app = App::new(cfg);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::new(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    let target = resolve_attach_target(&app).expect("mux row attachable");
    assert_eq!(target.backend, "tmux");
    assert_eq!(target.native_id, "editor");
}

fn build_mux_view_app_with_attachments(snapshot: GraphSnapshot) -> App {
    let snapshot = resolve_snapshot(snapshot);
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut cfg = RunConfig::defaults();
    cfg.default_view = View::Mux;
    let mut app = App::new(cfg);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::new(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

#[test]
fn view_resolves_agent_session_row_directly() {
    let mut snapshot = GraphSnapshot::empty();
    add_repo_and_worktree(&mut snapshot, "/p/proj");
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    let mut app = build_app(snapshot);
    app.update(Msg::NavDown);
    let session = resolve_view_session(&app).expect("agent session is viewable");
    assert_eq!(session.harness_key, "codex");
    assert_eq!(session.session_key, "abc");
}

#[test]
fn view_resolves_mux_row_to_linked_session() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = mux_node_id("tmux", "editor");
    snapshot.candidate_links.push(linked_to_mux(
        &session_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));

    let app = build_mux_view_app_with_attachments(snapshot);
    let session = resolve_view_session(&app).expect("mux row resolves linked session");
    assert_eq!(session.harness_key, "codex");
    assert_eq!(session.session_key, "abc");
}

#[test]
fn view_on_mux_row_picks_preferred_when_multiple_sessions_linked() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("tmux", "editor"));
    snapshot
        .nodes
        .push(session_node("codex", "/state", "abc", "/p/proj"));
    snapshot
        .nodes
        .push(session_node("claude-code", "/state", "def", "/p/proj"));
    let codex_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let claude_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "def"));
    let mux_id = mux_node_id("tmux", "editor");
    // codex link is the lower-precedence Discovered; claude link
    // is StrongDiscovered and should win.
    snapshot.candidate_links.push(linked_to_mux(
        &codex_id,
        &mux_id,
        Provenance::Discovered,
        "1",
    ));
    snapshot.candidate_links.push(linked_to_mux(
        &claude_id,
        &mux_id,
        Provenance::StrongDiscovered,
        "2",
    ));

    let app = build_mux_view_app_with_attachments(snapshot);
    let session = resolve_view_session(&app).expect("preferred session resolves");
    assert_eq!(
        session.harness_key, "claude-code",
        "StrongDiscovered candidate wins over Discovered"
    );
}

#[test]
fn view_on_unattached_mux_row_reports_unsupported() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("tmux", "editor"));
    let app = build_mux_view_app_with_attachments(snapshot);
    assert_eq!(
        resolve_view_session(&app),
        Err(ViewerDisabled::UnsupportedRow)
    );
}

// ----- Pin slot status hints and attach -----

fn pin_snapshot(status: crate::model::SlotStatus, target: Option<NodeId>) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Pin(crate::model::PinNode::for_test(
            "ingest",
            crate::model::PinKind::Mux,
            target,
            status,
            &[("name", "ingest")],
        )));
    snapshot
}

#[test]
fn pin_status_hint_is_silent_for_a_live_slot() {
    let snapshot = pin_snapshot(crate::model::SlotStatus::Live, None);
    assert_eq!(pin_status_hint(&snapshot, "ingest"), None);
}

#[test]
fn pin_status_hint_points_a_gone_slot_at_rebuild() {
    let snapshot = pin_snapshot(crate::model::SlotStatus::Gone, None);
    let hint = pin_status_hint(&snapshot, "ingest").expect("status hint produced");
    assert!(hint.contains("is stopped"), "hint: {hint}");
    assert!(hint.contains("rebuild ingest"), "hint: {hint}");
}

#[test]
fn pin_status_hint_names_each_unhealthy_status() {
    use crate::model::SlotStatus;
    for (status, needle) in [
        (SlotStatus::Unknown, "couldn't check"),
        (SlotStatus::Missing, "target not found"),
        (SlotStatus::Planned, "isn't built yet"),
    ] {
        let snapshot = pin_snapshot(status, None);
        let hint = pin_status_hint(&snapshot, "ingest").expect("status hint produced");
        assert!(hint.contains(needle), "{status:?}: {hint}");
    }
    assert_eq!(
        pin_status_hint(&pin_snapshot(crate::model::SlotStatus::Gone, None), "other"),
        None
    );
}

#[test]
fn pin_live_mux_follows_a_live_slot_to_its_mux() {
    let mux = mux_node_id("tmux", "ingest");
    let mut snapshot = pin_snapshot(crate::model::SlotStatus::Live, Some(mux.clone()));
    assert_eq!(pin_live_mux(&snapshot, "ingest"), None, "mux not in graph");
    snapshot.nodes.push(mux_node("tmux", "ingest"));
    assert_eq!(
        pin_live_mux(&snapshot, "ingest"),
        Some(MuxSessionId::new("tmux:ingest"))
    );
    let gone = pin_snapshot(crate::model::SlotStatus::Gone, Some(mux));
    assert_eq!(pin_live_mux(&gone, "ingest"), None);
}

#[test]
fn attach_target_label_is_backend_and_native_name() {
    let target = AttachTarget {
        mux: MuxSessionId::new("zellij:notes"),
        backend: "zellij".to_string(),
        native_id: "notes".to_string(),
    };
    assert_eq!(target.label(), "zellij:notes");
}

#[test]
fn attach_refuses_a_mux_that_is_only_in_history() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("tmux", "editor"));
    let snapshot = resolve_snapshot(snapshot);
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    // The same row, now that history says the mux is gone.
    let mut gone = snapshot;
    gone.node_presence.insert(
        NodeId::MuxSession(MuxSessionId::new("tmux:editor")),
        crate::model::NodePresence::Gone { last_seen: 1 },
    );
    let mut cfg = RunConfig::defaults();
    cfg.default_view = View::Mux;
    let mut app = App::new(cfg);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::new(gone),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    assert_eq!(
        resolve_attach_target(&app),
        Err(AttachDisabled::MuxGone("editor".to_string()))
    );
    assert_eq!(
        attach_disabled_reason(&AttachDisabled::MuxGone("editor".into())),
        "attach: `editor` isn't running · `conspectus rebuild editor` brings it back"
    );
}

/// A workspace with a session ambiguous about two muxes, and a repo
/// with two checkouts, the first with a session ambiguous about two
/// other muxes, as the Sessions view groups them: the workspace, the
/// repo, and each checkout get a group row.
fn ambiguous_groups_app() -> (App, [NodeId; 3]) {
    use crate::model::{WorkspaceId, WorkspaceNode};
    let repo_id = RepoId::new("/p/proj/.git");
    let workspace = NodeId::Workspace(WorkspaceId::new("/w/ws"));
    let repo = NodeId::Repo(repo_id.clone());
    let checkout = NodeId::Checkout(CheckoutId::new(repo_id.clone(), "/p/proj"));
    let other_checkout = NodeId::Checkout(CheckoutId::new(repo_id.clone(), "/p/proj-wt"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/w/ws"),
        root: "/w/ws".to_string(),
        provider: None,
        name: None,
    }));
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
    for root in ["/p/proj", "/p/proj-wt"] {
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id.clone(), root.to_string()),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
        }));
    }
    let session = |key: &str| NodeId::AgentSession(AgentSessionId::new("codex", "/state", key));
    let associated = |id: &str, from: &NodeId, to: &NodeId| GraphLink {
        id: id.to_string(),
        source: from.clone(),
        target: LinkEndpoint::Node { id: to.clone() },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    };
    for (key, cwd, group, muxes) in [
        ("owned", "/w/ws", &workspace, &["build", "deploy"][..]),
        ("loose", "/p/proj", &checkout, &["editor", "scratch"][..]),
        ("quiet", "/p/proj-wt", &other_checkout, &[][..]),
    ] {
        snapshot
            .nodes
            .push(session_node("codex", "/state", key, cwd));
        snapshot
            .candidate_links
            .push(associated(&format!("assoc-{key}"), &session(key), group));
        for mux in muxes {
            snapshot.nodes.push(mux_node("tmux", mux));
            snapshot.candidate_links.push(linked_to_mux(
                &session(key),
                &mux_node_id("tmux", mux),
                Provenance::Discovered,
                &format!("{key}-{mux}"),
            ));
        }
    }
    (build_app(snapshot), [workspace, repo, checkout])
}

#[test]
fn a_on_a_group_with_several_ambiguous_muxes_sends_the_operator_to_its_sessions() {
    let (mut app, groups) = ambiguous_groups_app();
    for group in groups {
        let snapshot = app.snapshot_handle().expect("data").snapshot().clone();
        let muxes = crate::tui::rows::ambiguous_muxes_for_group(&snapshot, &group);
        assert_eq!(muxes.len(), 2, "{group:?}: {muxes:?}");
        let rows = app.tree().rows.clone();
        let at = rows
            .iter()
            .position(|row| row.id == RowId::Group(group.clone()))
            .unwrap_or_else(|| panic!("the Sessions view lists {group:?}"));
        app.set_selection(rows[at].id.clone());

        // `a` cannot pick one, and says where they are.
        let said = app
            .update(Msg::AttachSelected)
            .into_iter()
            .find_map(|effect| match effect {
                crate::tui::Effect::Toast(feedback) => Some(feedback.text),
                _ => None,
            });
        assert_eq!(
            said.as_deref(),
            Some("attach: 2 ambiguous muxes here — select a session to see its candidates"),
            "{group:?}"
        );

        // The group's sessions, the rows under it, list every one of
        // them in the right pane, and `a` there attaches the one the
        // cursor is on.
        let mut listed = Vec::new();
        for row in rows[at + 1..]
            .iter()
            .take_while(|row| row.depth > rows[at].depth)
            .filter(|row| matches!(row.kind, crate::tui::rows::RowKind::AgentSession(_)))
        {
            app.set_selection(row.id.clone());
            listed.extend(candidates_attached_from_the_right_pane(&mut app, &muxes));
        }
        for mux in &muxes {
            assert!(listed.contains(mux), "{group:?}: {mux:?} not in {listed:?}");
        }
    }
}

/// With the selected row's right pane focused and its `Other` zone open,
/// the rows that lead to one of `muxes`, each checked to be what `a`
/// attaches with the cursor on it. Gives the left pane its focus back.
fn candidates_attached_from_the_right_pane(app: &mut App, muxes: &[NodeId]) -> Vec<NodeId> {
    use crate::tui::app::Focus;
    use crate::tui::explorer::ExplorerRow;
    use tuikit::explorer::ExplorerKey;
    let put_cursor_on = |app: &mut App, index: usize| {
        app.update(Msg::Explorer(ExplorerKey::First));
        for _ in 0..index {
            app.update(Msg::Explorer(ExplorerKey::Down));
        }
    };
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Right);
    if let Some(header) = app
        .explorer_rows()
        .iter()
        .position(|row| matches!(row, ExplorerRow::OtherHeader { expanded: false }))
    {
        put_cursor_on(app, header);
        app.update(Msg::Explorer(ExplorerKey::Activate));
    }
    let mut attached = Vec::new();
    for (index, row) in app.explorer_rows().iter().enumerate() {
        let target = app
            .explorer()
            .and_then(|explorer| explorer.content().drill_target(row));
        let Some(target) = target.filter(|target| muxes.contains(target)) else {
            continue;
        };
        put_cursor_on(app, index);
        let attach = resolve_attach_target(app).expect("a attaches the candidate");
        assert_eq!(NodeId::MuxSession(attach.mux), target);
        attached.push(target);
    }
    app.update(Msg::CycleFocus);
    assert_eq!(app.focus(), Focus::Left);
    attached
}
