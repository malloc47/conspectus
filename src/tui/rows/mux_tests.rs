use super::*;
use crate::filter::RowFilter;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutNode, Confidence, Freshness, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RelationKind,
    RepoNode, SourceMetadata,
};
use crate::resolve::resolve_snapshot;

fn mux_node(native: &str) -> GraphNode {
    mux_node_with_paths(native, Some(format!("/p/{native}")), None)
}

fn mux_node_with_activity(native: &str, activity_epoch: i64) -> GraphNode {
    let mut node = mux_node(native);
    if let GraphNode::MuxSession(mux) = &mut node {
        mux.activity_epoch = Some(activity_epoch);
    }
    node
}

fn mux_node_with_paths(
    native: &str,
    cwd: Option<String>,
    active_pane_current_path: Option<String>,
) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native}")),
        backend: "tmux".to_string(),
        native_id: native.to_string(),
        cwd,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: Some(1_700_000_050),
        created_epoch: None,
        last_attached_epoch: None,
    })
}

fn session_node(key: &str, cwd: &str) -> GraphNode {
    session_node_with_preview(key, cwd, None)
}

fn session_node_with_preview(
    key: &str,
    cwd: &str,
    last_message_preview: Option<String>,
) -> GraphNode {
    session_node_with_activity(key, cwd, last_message_preview, 1_700_000_100)
}

fn session_node_with_activity(
    key: &str,
    cwd: &str,
    last_message_preview: Option<String>,
    last_active_epoch: i64,
) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("codex", "/state", key),
        harness_key: "codex".to_string(),
        cwd: Some(cwd.to_string()),
        title: None,
        last_message_preview,
        last_active_epoch: Some(last_active_epoch),
        session_kind: None,
    })
}

#[test]
fn mux_view_emits_one_row_per_mux_with_agent_labels() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot.nodes.push(session_node_with_preview(
        "abcdef123456",
        "/p/editor",
        Some("running cargo test".to_string()),
    ));
    let session =
        crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abcdef123456"));
    let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: session,
        target: LinkEndpoint::Node { id: mux },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    assert_eq!(
        tree.rows.len(),
        1,
        "mux view should not emit agent children"
    );
    let RowKind::MuxSession(row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert_eq!(row.attached_count, 1);
    assert_eq!(row.agent_labels, vec!["codex"]);
    assert_eq!(row.recency.as_deref(), Some("1m"));
    assert_eq!(
        row.single_session_preview.as_deref(),
        Some("running cargo test"),
        "single-session mux should carry the agent's last-message preview"
    );
}

#[test]
fn mux_view_prefers_active_pane_cwd_for_display() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node_with_paths(
        "shell",
        Some("/started/here".to_string()),
        Some("/moved/there".to_string()),
    ));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let RowKind::MuxSession(row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert_eq!(row.cwd_display.as_deref(), Some("/moved/there"));
}

#[test]
fn mux_view_nests_session_rows_when_multiple_agents_link_to_one_mux() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot
        .nodes
        .push(session_node("abcdef123456", "/p/editor"));
    snapshot
        .nodes
        .push(session_node("123456abcdef", "/p/editor"));
    let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
    for key in ["abcdef123456", "123456abcdef"] {
        snapshot.candidate_links.push(GraphLink {
            id: format!("session-mux-{key}"),
            source: crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", key)),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    assert_eq!(tree.rows.len(), 3);
    assert!(tree.rows[0].expandable);
    let RowKind::MuxSession(mux_row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert!(
        mux_row.single_session_preview.is_none(),
        "multi-session mux should rely on child rows, not the inline preview"
    );
    assert_eq!(tree.rows[1].depth, 1);
    assert_eq!(tree.rows[2].depth, 1);
    assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
    assert!(matches!(tree.rows[2].kind, RowKind::AgentSession(_)));
}

#[test]
fn mux_view_drops_non_winner_linked_to_mux_candidate() {
    // The mux view's attached-agents list filters
    // through `resolved_relationships`. A `LinkedToMux`
    // candidate that the resolver did not pick (e.g. a weaker
    // cwd evidence pointing at a different mux) must not
    // surface as an attached agent under either mux.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot.nodes.push(mux_node("scratch"));
    snapshot.nodes.push(session_node("session-x", "/p/editor"));
    let session =
        crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "session-x"));
    let editor = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
    let scratch = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
    // Editor link is StrongDiscovered (winner); scratch link
    // is Discovered (loses the LinkedToMux slot).
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux-editor".to_string(),
        source: session.clone(),
        target: LinkEndpoint::Node { id: editor },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux-scratch".to_string(),
        source: session,
        target: LinkEndpoint::Node { id: scratch },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let mux_rows: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux),
            _ => None,
        })
        .collect();
    let editor_row = mux_rows
        .iter()
        .find(|mux| mux.native_id == "editor")
        .expect("editor row");
    let scratch_row = mux_rows
        .iter()
        .find(|mux| mux.native_id == "scratch")
        .expect("scratch row");
    assert_eq!(
        editor_row.attached_count, 1,
        "editor (resolver winner) keeps the attachment",
    );
    assert_eq!(
        scratch_row.attached_count, 0,
        "scratch (non-winner) must not surface a false-positive attachment",
    );
}

#[test]
fn mux_view_counts_same_target_evidence_as_one_attachment() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot.nodes.push(session_node("session-x", "/p/editor"));
    let session =
        crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "session-x"));
    let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));

    for id in ["session-mux-log", "session-mux-process"] {
        snapshot.candidate_links.push(GraphLink {
            id: id.to_string(),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let RowKind::MuxSession(row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert_eq!(row.attached_count, 1);
    assert_eq!(tree.rows.len(), 1);
    let candidate_counts: Vec<usize> = collect_attached_agents(&snapshot)
        .values()
        .flatten()
        .map(|agent| agent.candidate_count)
        .collect();
    assert_eq!(
        candidate_counts,
        vec![1],
        "multiple evidence links to the same mux target are corroboration, not ambiguity",
    );
}

#[test]
fn mux_view_omits_single_session_preview_when_unattached() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("solo"));

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let RowKind::MuxSession(row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert!(row.single_session_preview.is_none());
    assert_eq!(row.attached_count, 0);
}

#[test]
fn mux_view_omits_preview_when_attached_agent_has_none() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot
        .nodes
        .push(session_node("abcdef123456", "/p/editor"));
    let session =
        crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abcdef123456"));
    let mux = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: session,
        target: LinkEndpoint::Node { id: mux },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });

    let snapshot = resolve_snapshot(snapshot);
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let RowKind::MuxSession(row) = &tree.rows[0].kind else {
        panic!("expected mux row");
    };
    assert!(row.single_session_preview.is_none());
    assert_eq!(row.attached_count, 1);
}

#[test]
fn float_attached_muxes_top_lifts_attached_above_unattached() {
    // Three muxes: alpha and gamma are unattached, beta has one
    // agent session linked to it. Default row order is by
    // node_id (alpha, beta, gamma); with the bool set we expect
    // beta first and the alpha/gamma order preserved after it.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("alpha"));
    snapshot.nodes.push(mux_node("beta"));
    snapshot.nodes.push(mux_node("gamma"));
    snapshot.nodes.push(session_node("agent-1", "/p/beta"));
    let agent =
        crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", "agent-1"));
    let beta = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:beta"));
    snapshot.candidate_links.push(GraphLink {
        id: "session-beta".to_string(),
        source: agent,
        target: LinkEndpoint::Node { id: beta },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });

    let snapshot = resolve_snapshot(snapshot);

    let native_ids = |tree: &RowTree| -> Vec<String> {
        tree.rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
                _ => None,
            })
            .collect()
    };

    let baseline = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    assert_eq!(
        native_ids(&baseline),
        vec!["alpha", "beta", "gamma"],
        "baseline order is alphabetical by node_id"
    );

    let floated = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter {
            float_attached_muxes_top: true,
            ..RowFilter::default()
        },
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    assert_eq!(
        native_ids(&floated),
        vec!["beta", "alpha", "gamma"],
        "beta rises and alpha/gamma keep their relative order"
    );
}

#[test]
fn recency_sort_orders_muxes_by_latest_attached_agent_activity() {
    let now = 1_700_000_000;
    let old = now - 3 * 86_400;
    let fresh = now - 60;

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node_with_activity("alpha", old));
    snapshot.nodes.push(mux_node_with_activity("beta", fresh));
    snapshot.nodes.push(session_node_with_activity(
        "old-agent",
        "/p/alpha",
        None,
        old,
    ));
    snapshot.nodes.push(session_node_with_activity(
        "fresh-agent",
        "/p/beta",
        None,
        fresh,
    ));
    for (key, mux_native) in [("old-agent", "alpha"), ("fresh-agent", "beta")] {
        snapshot.candidate_links.push(GraphLink {
            id: format!("session-{mux_native}"),
            source: crate::model::NodeId::AgentSession(AgentSessionId::new("codex", "/state", key)),
            target: LinkEndpoint::Node {
                id: crate::model::NodeId::MuxSession(MuxSessionId::new(format!(
                    "tmux:{mux_native}"
                ))),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }

    let snapshot = resolve_snapshot(snapshot);

    let native_ids = |tree: &RowTree| -> Vec<String> {
        tree.rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
                _ => None,
            })
            .collect()
    };

    let hierarchy = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(now),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    assert_eq!(native_ids(&hierarchy), vec!["alpha", "beta"]);

    let recency = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(now),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Recency,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    assert_eq!(native_ids(&recency), vec!["beta", "alpha"]);
}

#[test]
fn repo_grouping_buckets_muxes_under_repo_headers() {
    let mut snapshot = GraphSnapshot::empty();
    // Two repos, three muxes total: two under /p/foo, one under
    // /p/bar. A fourth mux has no resolvable cwd and lands in
    // the Ungrouped synthetic bucket.
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
            "/p/foo/.git",
        ))));
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(crate::model::RepoId::new(
            "/p/bar/.git",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: crate::model::CheckoutId::new(
            crate::model::RepoId::new("/p/foo/.git"),
            "/p/foo".to_string(),
        ),
        root: "/p/foo".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: crate::model::CheckoutId::new(
            crate::model::RepoId::new("/p/bar/.git"),
            "/p/bar".to_string(),
        ),
        root: "/p/bar".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snapshot.nodes.push(mux_node_with_paths(
        "foo-a",
        Some("/p/foo".to_string()),
        None,
    ));
    snapshot.nodes.push(mux_node_with_paths(
        "foo-b",
        Some("/p/foo/sub".to_string()),
        None,
    ));
    snapshot.nodes.push(mux_node_with_paths(
        "bar-a",
        Some("/p/bar".to_string()),
        None,
    ));
    snapshot.nodes.push(mux_node_with_paths(
        "orphan",
        Some("/elsewhere".to_string()),
        None,
    ));

    let snapshot = resolve_snapshot(snapshot);

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Repo,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let row_summary: Vec<(u8, String)> = tree
        .rows
        .iter()
        .map(|row| {
            let label = match &row.kind {
                RowKind::MuxSession(mux) => mux.native_id.clone(),
                RowKind::Group(g) => g.display_path.clone(),
                other => format!("{other:?}"),
            };
            (row.depth, label)
        })
        .collect();

    // Bucket order is repo display path asc, then Ungrouped last.
    // Expected shape:
    //   depth 0: /p/bar
    //     depth 1: bar-a
    //   depth 0: /p/foo
    //     depth 1: foo-a
    //     depth 1: foo-b
    //   depth 0: Ungrouped
    //     depth 1: orphan
    assert_eq!(row_summary[0], (0, "/p/bar".to_string()));
    assert_eq!(row_summary[1], (1, "bar-a".to_string()));
    assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
    assert_eq!(row_summary[3], (1, "foo-a".to_string()));
    assert_eq!(row_summary[4], (1, "foo-b".to_string()));
    assert_eq!(row_summary[5], (0, "Ungrouped".to_string()));
    assert_eq!(row_summary[6], (1, "orphan".to_string()));
    assert_eq!(row_summary.len(), 7, "no extra rows: {row_summary:?}");
}

#[test]
fn mux_view_paints_pin_id_on_bound_mux_rows() {
    // A pin bound to `tmux:editor` should leave its `pin_id` on
    // the mux row so the renderer can paint the bound-pin glyph
    // (regardless of grouping). Other muxes stay None. We set
    // `binding` directly rather than running the resolver
    // because the resolver test fixtures and production
    // discovery encode `MuxSessionNode.native_id` differently
    // (resolver-test fixtures prefix `tmux:`, production
    // discovery emits the bare name); the mux-view code path
    // we're testing only cares about the value already stored
    // on the pin.
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("editor"));
    snapshot.nodes.push(mux_node("scratch"));
    snapshot.pins.push(PinCandidate {
        id: "code".to_string(),
        display_name: "Code Review".to_string(),
        harness: "claude-code".to_string(),
        cwd: "/p/work".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "editor".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/work/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Bound {
            mux: MuxSessionId::new("tmux:editor"),
            session: AgentSessionId::new("claude-code", "/state", "session"),
        }),
    });

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let mut pin_ids: Vec<(String, Option<String>)> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some((mux.native_id.clone(), mux.pin_id.clone())),
            _ => None,
        })
        .collect();
    pin_ids.sort();
    assert_eq!(
        pin_ids,
        vec![
            ("editor".to_string(), Some("code".to_string())),
            ("scratch".to_string(), None),
        ],
    );
}

#[test]
fn mux_view_flat_grouping_floats_pinned_mux_rows_to_top() {
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("newer"));
    snapshot.nodes.push(mux_node("older"));
    snapshot.pins.push(PinCandidate {
        id: "old-pin".to_string(),
        display_name: "Old Pin".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/older".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "older".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/older/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux {
            mux: MuxSessionId::new("tmux:older"),
        }),
    });

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let muxes: Vec<_> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux),
            _ => None,
        })
        .collect();
    assert_eq!(muxes.len(), 2, "{:#?}", tree.rows);
    assert_eq!(muxes[0].native_id, "older");
    assert_eq!(muxes[0].pin_id.as_deref(), Some("old-pin"));
    assert_eq!(muxes[1].native_id, "newer");
}

#[test]
fn mux_view_emits_pins_group_at_top_under_repo_grouping() {
    // Repo grouping introduces header rows. The Pins group
    // should sit above every repo bucket so the operator sees
    // pinned work first.
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Repo(RepoNode {
        id: crate::model::RepoId::new("/p/foo/.git"),
        common_dir: "/p/foo/.git".to_string(),
        source_paths: vec!["/p/foo".to_string()],
        remotes: vec![],
    }));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: crate::model::CheckoutId::new(
            crate::model::RepoId::new("/p/foo/.git"),
            "/p/foo".to_string(),
        ),
        root: "/p/foo".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snapshot.nodes.push(mux_node_with_paths(
        "foo-a",
        Some("/p/foo".to_string()),
        None,
    ));
    snapshot.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest Pin".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/foo".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/foo/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Unbound),
    });

    let snapshot = resolve_snapshot(snapshot);

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Repo,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let row_summary: Vec<(u8, String)> = tree
        .rows
        .iter()
        .map(|row| {
            let label = match &row.kind {
                RowKind::MuxSession(mux) => format!("mux:{}", mux.native_id),
                RowKind::Group(g) => g.display_path.clone(),
                other => format!("{other:?}"),
            };
            (row.depth, label)
        })
        .collect();
    // Pins group first, then the repo bucket. The unbound pin
    // also appears in its natural repo location as a placeholder
    // mux row, mirroring bound pins appearing both in Pins and
    // in place.
    assert_eq!(row_summary[0], (0, "Pins".to_string()));
    assert_eq!(row_summary[1], (1, "mux:ingest".to_string()));
    assert_eq!(row_summary[2], (0, "/p/foo".to_string()));
    assert_eq!(row_summary[3], (1, "mux:ingest".to_string()));
    assert_eq!(row_summary[4], (1, "mux:foo-a".to_string()));
}

#[test]
fn mux_view_repo_grouping_renders_bound_pins_as_mux_rows() {
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Repo(RepoNode {
        id: crate::model::RepoId::new("/p/foo/.git"),
        common_dir: "/p/foo/.git".to_string(),
        source_paths: vec!["/p/foo".to_string()],
        remotes: vec![],
    }));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: crate::model::CheckoutId::new(
            crate::model::RepoId::new("/p/foo/.git"),
            "/p/foo".to_string(),
        ),
        root: "/p/foo".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    }));
    snapshot.nodes.push(mux_node_with_paths(
        "foo-a",
        Some("/p/foo".to_string()),
        None,
    ));
    snapshot.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "Ingest Pin".to_string(),
        harness: "codex".to_string(),
        cwd: "/p/foo".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "foo-a".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/foo/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux {
            mux: MuxSessionId::new("tmux:foo-a"),
        }),
    });

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Repo,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let pins_group_idx = tree
        .rows
        .iter()
        .position(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"))
        .expect("Pins group present");
    let pins_children: Vec<_> = tree
        .rows
        .iter()
        .skip(pins_group_idx + 1)
        .take_while(|row| row.depth > 0)
        .collect();
    assert_eq!(pins_children.len(), 1, "{:#?}", tree.rows);
    match &pins_children[0].kind {
        RowKind::MuxSession(mux) => {
            assert_eq!(mux.native_id, "foo-a");
            assert_eq!(mux.pin_id.as_deref(), Some("ingest"));
        }
        other => panic!("expected pinned mux row, got {other:?}"),
    }
}

#[test]
fn mux_view_flat_grouping_renders_unbound_pin_as_placeholder_mux_row() {
    // Flat groupings float pinned mux entities directly rather
    // than inserting a synthetic Pins group header. Unbound pins
    // still get a mux-shaped placeholder row so the operator can
    // launch them from the mux view before tmux exists.
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(PinCandidate {
        id: "code".to_string(),
        display_name: "Pin".to_string(),
        harness: "claude-code".to_string(),
        cwd: "/p".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "editor".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Unbound),
    });

    let snapshot = resolve_snapshot(snapshot);

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let has_pins_group = tree
        .rows
        .iter()
        .any(|row| matches!(&row.id, RowId::Synthetic(tag) if *tag == "pins"));
    assert!(
        !has_pins_group,
        "session/host groupings should not emit a Pins group: {:#?}",
        tree.rows,
    );
    let mux = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(mux) if mux.pin_id.as_deref() == Some("code") => Some(mux),
            _ => None,
        })
        .expect("placeholder mux row");
    assert_eq!(mux.native_id, "editor");
    assert_eq!(mux.agent_labels, vec!["claude".to_string()]);
    assert_eq!(mux.single_session_preview.as_deref(), Some("/p"));
    assert!(matches!(&mux.primary_node, NodeId::Pin(pin) if pin.id == "code"));
}

fn mux_node_with_epochs(
    native: &str,
    activity: i64,
    created: i64,
    last_attached: i64,
) -> GraphNode {
    let mut node = mux_node(native);
    if let GraphNode::MuxSession(mux) = &mut node {
        mux.activity_epoch = Some(activity);
        mux.created_epoch = Some(created);
        mux.last_attached_epoch = Some(last_attached);
    }
    node
}

fn mux_recency_order(snapshot: &GraphSnapshot, basis: crate::tui::MuxRecency) -> Vec<String> {
    let tree = build_mux_tree(MuxBuildInputs {
        snapshot,
        home: None,
        now: Some(1_700_001_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Recency,
        mux_recency: basis,
    });
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux.native_id.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn mux_recency_basis_reorders_rows_by_chosen_signal() {
    // Three muxes whose recency signals rank them differently:
    //   activity:      a > b > c
    //   created:       c > b > a
    //   last_attached: b > a > c
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(mux_node_with_epochs("a", 300, 100, 200));
    snapshot
        .nodes
        .push(mux_node_with_epochs("b", 200, 150, 300));
    snapshot
        .nodes
        .push(mux_node_with_epochs("c", 100, 200, 100));
    let snapshot = resolve_snapshot(snapshot);

    assert_eq!(
        mux_recency_order(&snapshot, crate::tui::MuxRecency::Activity),
        vec!["a", "b", "c"],
    );
    assert_eq!(
        mux_recency_order(&snapshot, crate::tui::MuxRecency::Created),
        vec!["c", "b", "a"],
    );
    assert_eq!(
        mux_recency_order(&snapshot, crate::tui::MuxRecency::LastAttached),
        vec!["b", "a", "c"],
    );
}

#[test]
fn mux_recency_missing_signal_sorts_to_the_bottom() {
    // `b` has no created_epoch; under the Created basis it sorts last
    // (None < Some), deterministically, without panicking.
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node_with_epochs("a", 10, 100, 10));
    snapshot.nodes.push(mux_node_with_epochs("c", 10, 50, 10));
    // `b` keeps the default created_epoch: None from mux_node().
    snapshot.nodes.push(mux_node("b"));
    let snapshot = resolve_snapshot(snapshot);

    let order = mux_recency_order(&snapshot, crate::tui::MuxRecency::Created);
    assert_eq!(order.last().map(String::as_str), Some("b"));
}

#[test]
fn stale_pin_mux_without_a_reported_command_shows_the_pin_program() {
    use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(mux_node("serve"));
    snapshot.pins.push(PinCandidate {
        id: "serve".to_string(),
        display_name: "serve".to_string(),
        harness: "conspectus".to_string(),
        cwd: "/p/work".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "serve".to_string(),
            socket_name: None,
        },
        launch_argv: Some(vec!["conspectus".to_string(), "serve".to_string()]),
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/p/work/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux {
            mux: MuxSessionId::new("tmux:serve"),
        }),
    });

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let row = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(mux) if mux.native_id == "serve" => Some(mux),
            _ => None,
        })
        .expect("live mux row");
    assert!(row.agent_labels.is_empty());
    assert_eq!(row.program.as_deref(), Some("conspectus"));
    assert_eq!(row.pin_id.as_deref(), Some("serve"));
}

#[test]
fn agentless_mux_running_atelier_exec_carries_the_harness_from_its_process_tree() {
    use crate::model::{RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole};
    let process = |key: &str, harness: &str, depth: i64| RuntimeProcessNode {
        id: RuntimeProcessId::new(key),
        observation_key: key.to_string(),
        pid: None,
        parent_pid: None,
        root_pane_pid: None,
        command: None,
        cwd: None,
        harness_key: Some(harness.to_string()),
        role: Some(RuntimeProcessRole::HumanAgent),
        depth: Some(depth),
        observed_epoch: None,
    };
    let contains = |key: &str| GraphLink {
        id: format!("contains-{key}"),
        source: NodeId::MuxSession(MuxSessionId::new("tmux:ingest")),
        target: LinkEndpoint::Node {
            id: NodeId::RuntimeProcess(RuntimeProcessId::new(key)),
        },
        relation: RelationKind::MuxContainsProcess,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    };

    let mut snapshot = GraphSnapshot::empty();
    let mut ingest = mux_node("ingest");
    if let GraphNode::MuxSession(mux) = &mut ingest {
        mux.active_pane_command = Some("atelier".to_string());
        mux.active_pane_start_command = Some("zsh".to_string());
    }
    snapshot.nodes.push(ingest);
    // `atelier exec` spawned codex, which spawned a claude subagent; the
    // shallower codex process is the pane's harness.
    snapshot
        .nodes
        .push(GraphNode::RuntimeProcess(process("sub", "claude-code", 3)));
    snapshot
        .nodes
        .push(GraphNode::RuntimeProcess(process("main", "codex", 2)));
    snapshot.candidate_links.push(contains("sub"));
    snapshot.candidate_links.push(contains("main"));
    let snapshot = resolve_snapshot(snapshot);

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_700_000_000),
        filter: RowFilter::default(),
        grouping: MuxGrouping::Session,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let row = tree
        .rows
        .iter()
        .find_map(|row| match &row.kind {
            RowKind::MuxSession(mux) if mux.native_id == "ingest" => Some(mux),
            _ => None,
        })
        .expect("ingest mux row");
    assert!(row.agent_labels.is_empty());
    assert_eq!(row.program.as_deref(), Some("atelier"));
    assert_eq!(row.program_harness.as_deref(), Some("codex"));
}

fn workspace_node(root: &str, name: &str) -> GraphNode {
    GraphNode::Workspace(crate::model::WorkspaceNode {
        id: crate::model::WorkspaceId::new(root.to_string()),
        root: root.to_string(),
        provider: Some("atelier".to_string()),
        name: Some(name.to_string()),
    })
}

fn membership_link(workspace_root: &str, member: &str, common_dir: &str) -> GraphLink {
    let mut source_metadata = SourceMetadata::default();
    source_metadata.fields.insert(
        crate::model::source_field::LOGICAL_PATH.to_string(),
        serde_json::Value::String(format!("{workspace_root}/{member}")),
    );
    GraphLink {
        id: format!("member:{workspace_root}:{member}"),
        source: crate::model::NodeId::Workspace(crate::model::WorkspaceId::new(
            workspace_root.to_string(),
        )),
        target: LinkEndpoint::Node {
            id: crate::model::NodeId::Repo(crate::model::RepoId::new(common_dir)),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata,
        state: LinkState::Active,
    }
}

fn repo_with_checkouts(common_dir: &str, roots: &[&str]) -> Vec<GraphNode> {
    let repo_id = crate::model::RepoId::new(common_dir);
    let mut repo = RepoNode::new(repo_id.clone());
    repo.source_paths.push(roots[0].to_string());
    let mut nodes = vec![GraphNode::Repo(repo)];
    for root in roots {
        nodes.push(GraphNode::Checkout(CheckoutNode {
            id: crate::model::CheckoutId::new(repo_id.clone(), root.to_string()),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
        }));
    }
    nodes
}

fn mux_at(native: &str, cwd: &str) -> GraphNode {
    mux_node_with_paths(native, Some(cwd.to_string()), None)
}

/// Run the cross-link pass and the resolver, as discovery does, so the
/// builder sees real `rooted_in` slots.
fn linked_and_resolved(mut snapshot: GraphSnapshot) -> GraphSnapshot {
    crate::discovery::cross_link::infer_without_process_tree(&mut snapshot);
    resolve_snapshot(snapshot)
}

fn build_with(snapshot: &GraphSnapshot, grouping: MuxGrouping) -> RowTree {
    build_mux_tree(MuxBuildInputs {
        snapshot,
        home: None,
        now: Some(1_700_000_160),
        filter: RowFilter::default(),
        grouping,
        sort: Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    })
}

fn row_labels(tree: &RowTree) -> Vec<(u8, String)> {
    tree.rows
        .iter()
        .map(|row| {
            let label = match &row.kind {
                RowKind::MuxSession(mux) => mux.native_id.clone(),
                RowKind::Group(g) => g.display_path.clone(),
                other => format!("{other:?}"),
            };
            (row.depth, label)
        })
        .collect()
}

/// An Atelier-style `sysadmin` workspace with `config` and
/// `work-config` members whose canonical clones live under `/src`.
fn sysadmin_snapshot(muxes: Vec<GraphNode>) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(workspace_node("/ws/sysadmin", "sysadmin"));
    snapshot.nodes.extend(repo_with_checkouts(
        "/src/config/.git",
        &["/src/config", "/ws/sysadmin/config"],
    ));
    snapshot.nodes.extend(repo_with_checkouts(
        "/src/work-config/.git",
        &["/src/work-config", "/ws/sysadmin/work-config"],
    ));
    snapshot.candidate_links.push(membership_link(
        "/ws/sysadmin",
        "config",
        "/src/config/.git",
    ));
    snapshot.candidate_links.push(membership_link(
        "/ws/sysadmin",
        "work-config",
        "/src/work-config/.git",
    ));
    snapshot.nodes.extend(muxes);
    snapshot
}

#[test]
fn workspace_grouping_gathers_a_workspaces_muxes_and_keeps_repo_buckets_for_the_rest() {
    let snapshot = linked_and_resolved(sysadmin_snapshot(vec![
        mux_at("sys-root", "/ws/sysadmin"),
        mux_at("sys-config", "/ws/sysadmin/config/nix"),
        mux_at("sys-work", "/ws/sysadmin/work-config"),
        mux_at("canonical", "/src/config"),
        mux_at("orphan", "/elsewhere"),
    ]));

    let tree = build_with(&snapshot, MuxGrouping::Workspace);

    // One header for the workspace whichever member each mux is in,
    // the canonical clone keeps its repo bucket (`config` sorts before
    // `sysadmin`), and the rest go to Ungrouped.
    assert_eq!(
        row_labels(&tree),
        vec![
            (0, "/src/config".to_string()),
            (1, "canonical".to_string()),
            (0, "sysadmin  config+work-config  (atelier)".to_string()),
            (1, "sys-config".to_string()),
            (1, "sys-root".to_string()),
            (1, "sys-work".to_string()),
            (0, "Ungrouped".to_string()),
            (1, "orphan".to_string()),
        ]
    );
    assert_eq!(
        tree.rows[2].id,
        RowId::Group(crate::model::NodeId::Workspace(
            crate::model::WorkspaceId::new("/ws/sysadmin".to_string())
        ))
    );
}

#[test]
fn workspace_grouping_without_workspaces_matches_repo_grouping() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .extend(repo_with_checkouts("/p/foo/.git", &["/p/foo"]));
    snapshot.nodes.push(mux_at("foo", "/p/foo/src"));
    snapshot.nodes.push(mux_at("orphan", "/elsewhere"));
    let snapshot = linked_and_resolved(snapshot);

    let workspace = build_with(&snapshot, MuxGrouping::Workspace);
    let repo = build_with(&snapshot, MuxGrouping::Repo);

    assert_eq!(row_labels(&workspace), row_labels(&repo));
    assert_eq!(
        row_labels(&workspace),
        vec![
            (0, "/p/foo".to_string()),
            (1, "foo".to_string()),
            (0, "Ungrouped".to_string()),
            (1, "orphan".to_string()),
        ]
    );
}

#[test]
fn workspace_and_repo_headers_interleave_by_name_and_nest_innermost() {
    let mut snapshot = GraphSnapshot::empty();
    // `beta` has the lexically earlier root but the later label.
    snapshot.nodes.push(workspace_node("/a/beta", "beta"));
    snapshot.nodes.push(workspace_node("/z/alpha", "alpha"));
    snapshot
        .nodes
        .push(workspace_node("/z/alpha/inner", "inner"));
    snapshot
        .candidate_links
        .push(membership_link("/a/beta", "repo", "/src/beta-repo/.git"));
    snapshot
        .candidate_links
        .push(membership_link("/z/alpha", "inner", "/src/inner-repo/.git"));
    snapshot.candidate_links.push(membership_link(
        "/z/alpha/inner",
        "repo",
        "/src/nested-repo/.git",
    ));
    snapshot
        .nodes
        .extend(repo_with_checkouts("/0/first/.git", &["/0/first"]));
    snapshot.nodes.push(mux_at("in-beta", "/a/beta/repo"));
    snapshot.nodes.push(mux_at("in-alpha", "/z/alpha"));
    snapshot
        .nodes
        .push(mux_at("in-inner", "/z/alpha/inner/repo/src"));
    snapshot.nodes.push(mux_at("in-repo", "/0/first"));
    let snapshot = linked_and_resolved(snapshot);

    let tree = build_with(&snapshot, MuxGrouping::Workspace);

    // Names, not roots or header kinds, set the order: the `first`
    // repo lands between the `beta` and `inner` workspaces.
    assert_eq!(
        row_labels(&tree),
        vec![
            (0, "alpha  inner  (atelier)".to_string()),
            (1, "in-alpha".to_string()),
            (0, "beta  repo  (atelier)".to_string()),
            (1, "in-beta".to_string()),
            (0, "/0/first".to_string()),
            (1, "in-repo".to_string()),
            (0, "inner  repo  (atelier)".to_string()),
            (1, "in-inner".to_string()),
        ]
    );
}

#[test]
fn workspace_grouping_header_order_ignores_case_and_breaks_name_ties_by_path() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(workspace_node("/w/zeta", "Zeta"));
    snapshot.nodes.push(workspace_node("/w/docs-ws", "docs"));
    snapshot
        .candidate_links
        .push(membership_link("/w/zeta", "repo", "/src/zeta-repo/.git"));
    snapshot
        .candidate_links
        .push(membership_link("/w/docs-ws", "repo", "/src/docs-repo/.git"));
    for root in ["/r/alpha", "/r/Delta", "/other/docs"] {
        snapshot
            .nodes
            .extend(repo_with_checkouts(&format!("{root}/.git"), &[root]));
    }
    snapshot.nodes.push(mux_at("z", "/w/zeta/repo"));
    snapshot.nodes.push(mux_at("d-ws", "/w/docs-ws"));
    snapshot.nodes.push(mux_at("a", "/r/alpha"));
    snapshot.nodes.push(mux_at("delta", "/r/Delta"));
    snapshot.nodes.push(mux_at("d-repo", "/other/docs"));
    let snapshot = linked_and_resolved(snapshot);

    let tree = build_with(&snapshot, MuxGrouping::Workspace);
    let headers: Vec<String> = row_labels(&tree)
        .into_iter()
        .filter(|(depth, _)| *depth == 0)
        .map(|(_, label)| label)
        .collect();

    // `Delta` sorts as `delta`, `Zeta` as `zeta`; the two `docs`
    // headers tie on name and fall back to path, so `/other/docs`
    // precedes `/w/docs-ws`.
    assert_eq!(
        headers,
        vec![
            "/r/alpha".to_string(),
            "/r/Delta".to_string(),
            "/other/docs".to_string(),
            "docs  repo  (atelier)".to_string(),
            "Zeta  repo  (atelier)".to_string(),
        ]
    );
}

#[test]
fn workspace_grouping_places_unbound_pin_placeholders_by_cwd() {
    use crate::model::{PinCandidate, PinMuxRef};
    let mut snapshot = sysadmin_snapshot(vec![mux_at("sys-root", "/ws/sysadmin")]);
    snapshot.pins.push(PinCandidate {
        id: "nix".to_string(),
        display_name: "Nix".to_string(),
        harness: "claude-code".to_string(),
        cwd: "/ws/sysadmin/config".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "nix-pin".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/ws/sysadmin/.conspectus.toml".to_string(),
        binding: Some(crate::model::PinBinding::Unbound),
    });
    let snapshot = linked_and_resolved(snapshot);

    let tree = build_with(&snapshot, MuxGrouping::Workspace);

    // The placeholder shows in Pins and, by its cwd, under the
    // workspace, ahead of the live mux because pinned rows float.
    assert_eq!(
        row_labels(&tree),
        vec![
            (0, "Pins".to_string()),
            (1, "nix-pin".to_string()),
            (0, "sysadmin  config+work-config  (atelier)".to_string()),
            (1, "nix-pin".to_string()),
            (1, "sys-root".to_string()),
        ]
    );
}

/// One git probe's worth of graph: the repo node carrying the probed
/// checkout as its only source path, plus that checkout.
fn probe_fragment(common_dir: &str, checkout_root: &str) -> crate::discovery::GraphFragment {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .extend(repo_with_checkouts(common_dir, &[checkout_root]));
    crate::discovery::GraphFragment::from(snapshot)
}

#[test]
fn repo_buckets_do_not_depend_on_which_checkout_was_probed_first() {
    // Discovery keeps the first probe's repo node, so the developer
    // repo carries either its canonical clone or its linked worktree
    // as its only source path depending on fragment order. The bucket
    // label and its position must come out the same either way.
    let rows_for = |linked_first: bool| {
        let linked = probe_fragment("/src/drw/developer/.git", "/src/workspaces/developer");
        let canonical = probe_fragment("/src/drw/developer/.git", "/src/drw/developer");
        let (first, second) = if linked_first {
            (linked, canonical)
        } else {
            (canonical, linked)
        };
        let mut muxes = GraphSnapshot::empty();
        muxes.nodes.push(mux_at("dev", "/src/workspaces/developer"));
        muxes.nodes.push(mux_at("csp", "/src/oss/conspectus"));
        let snapshot = crate::discovery::merge_fragments([
            first,
            second,
            probe_fragment("/src/oss/conspectus/.git", "/src/oss/conspectus"),
            crate::discovery::GraphFragment::from(muxes),
        ]);
        row_labels(&build_with(&resolve_snapshot(snapshot), MuxGrouping::Repo))
    };

    let expected = vec![
        (0, "/src/drw/developer".to_string()),
        (1, "dev".to_string()),
        (0, "/src/oss/conspectus".to_string()),
        (1, "csp".to_string()),
    ];
    assert_eq!(rows_for(true), expected);
    assert_eq!(rows_for(false), expected);
}
