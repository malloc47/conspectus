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

// ----- H-PIN-RESUME-005: pin status hint branches on last_session -----

#[test]
fn pin_status_hint_unbound_without_last_session_advertises_launch() {
    let diagnostic = PinDiagnosticView::Unbound {
        pin_id: "ingest".to_string(),
        expected_mux_native_id: "tmux:ingest".to_string(),
        last_session: None,
    };
    let hint = pin_status_hint(&[diagnostic]).expect("status hint produced");
    assert!(hint.contains("Enter launch"), "hint: {hint}");
    assert!(!hint.contains("resume"), "hint: {hint}");
}

#[test]
fn pin_status_hint_unbound_with_last_session_advertises_resume() {
    let diagnostic = PinDiagnosticView::Unbound {
        pin_id: "ingest".to_string(),
        expected_mux_native_id: "tmux:ingest".to_string(),
        last_session: Some(PinLastSession {
            session_id: "session-a".to_string(),
            observed_epoch: 1,
        }),
    };
    let hint = pin_status_hint(&[diagnostic]).expect("status hint produced");
    assert!(hint.contains("Enter resume"), "hint: {hint}");
    assert!(hint.contains("session-a"), "hint: {hint}");
}
