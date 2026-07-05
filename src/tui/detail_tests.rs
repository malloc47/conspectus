// Extracted from detail.rs H-HYG-011 rolling wave via #[path = "detail_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, Diagnostic, ForgePrId,
    ForgePrNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, PinBinding,
    PinCandidate, PinMuxRef, Provenance, RepoId, RepoNode, RuntimeProcessId, RuntimeProcessNode,
    RuntimeProcessRole, SourceMetadata, WorkspaceId, WorkspaceNode,
};
use crate::resolve::resolve_snapshot;
use std::path::PathBuf;

fn home() -> PathBuf {
    PathBuf::from("/home/op")
}

fn agent(harness: &str, key: &str, cwd: Option<&str>, title: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, "/state", key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: title.map(str::to_string),
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn runtime_process(observation_key: &str, pid: i64, command: &str) -> GraphNode {
    GraphNode::RuntimeProcess(RuntimeProcessNode {
        id: RuntimeProcessId::new(observation_key),
        observation_key: observation_key.to_string(),
        pid: Some(pid),
        parent_pid: None,
        root_pane_pid: Some(pid),
        command: Some(command.to_string()),
        cwd: Some("/home/op/src/x".to_string()),
        harness_key: Some("codex".to_string()),
        role: Some(RuntimeProcessRole::HumanAgent),
        depth: Some(0),
        observed_epoch: Some(1_700_000_500),
    })
}

fn process_link(id: &str, source: NodeId, target: NodeId, relation: RelationKind) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn build(snapshot: &GraphSnapshot, target: &NodeId, home: Option<&Path>) -> NodeDetail {
    build_node_detail(DetailInputs {
        snapshot,
        target,
        home,
    })
    .expect("detail exists")
}

#[test]
fn unknown_node_returns_none() {
    let snapshot = GraphSnapshot::empty();
    let phantom = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "nope"));
    assert!(
        build_node_detail(DetailInputs {
            snapshot: &snapshot,
            target: &phantom,
            home: None,
        })
        .is_none()
    );
}

#[test]
fn agent_session_with_no_mux_or_pr_shows_placeholders() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/src/x"), None));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

    let detail = build(&snapshot, &target, Some(home().as_path()));
    assert_eq!(detail.kind_label, "agent_session");
    assert_eq!(detail.title_line, "codex:abc");

    // No title row when unset.
    assert!(detail.header_fields.iter().all(|f| f.label != "title"));

    let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
    assert_eq!(labels, vec!["id", "harness", "cwd", "mux", "pr", "lineage"]);

    let by_label = |label: &str| {
        detail
            .header_fields
            .iter()
            .find(|f| f.label == label)
            .unwrap()
            .clone()
    };
    assert_eq!(by_label("id").value, "abc");
    assert_eq!(by_label("harness").value, "codex");
    assert_eq!(by_label("cwd").value, "~/src/x");
    assert!(by_label("mux").placeholder);
    assert_eq!(by_label("mux").value, "— (no attach)");
    assert!(by_label("pr").placeholder);
    assert!(by_label("lineage").placeholder);

    // ADR 0033 / Phase 6: sparse sessions collapse to just the
    // Session section. Mux / PR / Lineage sections are all
    // placeholder-only here and should be suppressed.
    let sections = detail.sections();
    assert_eq!(
        sections.iter().map(|s| s.kind).collect::<Vec<_>>(),
        vec![SectionKind::Session],
        "sparse session should hide placeholder-only sections",
    );
    let session_field_labels: Vec<&str> = sections[0].fields.iter().map(|f| f.label).collect();
    assert_eq!(session_field_labels, vec!["id", "harness", "cwd"]);
}

#[test]
fn agent_session_bound_to_ambiguous_pin_shows_bind_hint() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", "alpha", Some("/home/op/src/x"), None));
    let chosen = AgentSessionId::new("codex", "/state", "alpha");
    let competing = AgentSessionId::new("codex", "/state", "beta");
    snapshot.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/home/op/src/x".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/home/op/src/x/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Bound {
            mux: MuxSessionId::new("tmux:ingest"),
            session: chosen.clone(),
        }),
    });
    snapshot.diagnostics.push(Diagnostic::PinAmbiguous {
        pin_id: "ingest".to_string(),
        chosen: chosen.clone(),
        competing: vec![competing],
    });

    let detail = build(
        &snapshot,
        &NodeId::AgentSession(chosen),
        Some(home().as_path()),
    );
    let pin = detail
        .header_fields
        .iter()
        .find(|field| field.label == "pin")
        .expect("pin diagnostic field");

    assert!(pin.value.contains("ingest ambiguous"));
    assert!(pin.value.contains("chosen codex:alpha"));
    assert!(pin.value.contains("competing codex:beta"));
    assert_eq!(pin.annotation, Some("b to bind"));
}

#[test]
fn agent_session_bound_to_healthy_pin_shows_pin_summary() {
    let mut snapshot = GraphSnapshot::empty();
    let session = AgentSessionId::new("codex", "/state", "alpha");
    snapshot
        .nodes
        .push(agent("codex", "alpha", Some("/home/op/src/x"), None));
    snapshot.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/home/op/src/x".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/home/op/src/x/.conspectus.toml".to_string(),
        binding: Some(PinBinding::Bound {
            mux: MuxSessionId::new("tmux:ingest"),
            session: session.clone(),
        }),
    });

    let detail = build(
        &snapshot,
        &NodeId::AgentSession(session),
        Some(home().as_path()),
    );
    let pin = detail
        .header_fields
        .iter()
        .find(|field| field.label == "pin")
        .expect("pin summary field");

    assert!(pin.value.contains("ingest"));
    assert!(pin.value.contains("bound"));
    assert!(pin.value.contains(".conspectus.toml"));
}

#[test]
fn mux_targeted_by_stale_pin_shows_pin_summary() {
    let mut snapshot = GraphSnapshot::empty();
    let mux = MuxSessionId::new("tmux:ingest");
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: mux.clone(),
        backend: "tmux".to_string(),
        native_id: "ingest".to_string(),
        cwd: Some("/home/op/src/x".to_string()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.pins.push(PinCandidate {
        id: "ingest".to_string(),
        display_name: "ingest".to_string(),
        harness: "codex".to_string(),
        cwd: "/home/op/src/x".to_string(),
        mux: PinMuxRef {
            backend: "tmux".to_string(),
            name: "ingest".to_string(),
            socket_name: None,
        },
        launch_argv: None,
        reason: None,
        provenance: Provenance::LocalPin,
        store_path: "/home/op/src/x/.conspectus.toml".to_string(),
        binding: Some(PinBinding::StaleMux { mux: mux.clone() }),
    });

    let detail = build(&snapshot, &NodeId::MuxSession(mux), Some(home().as_path()));
    let pin = detail
        .header_fields
        .iter()
        .find(|field| field.label == "pin")
        .expect("pin summary field");

    assert!(pin.value.contains("ingest"));
    assert!(pin.value.contains("stale-mux"));
}

#[test]
fn sections_emit_mux_section_when_ambiguous_mux_carries_warning() {
    // A session with ≥2 mux candidates has a `⚠` annotation on
    // its mux field; even though the value reads as a
    // placeholder-ish "— (N candidates)" it is *not* a blank
    // placeholder per ADR 0033's suppression rule and the Mux
    // section stays visible.
    use crate::model::{Freshness, GraphLink, RelationKind};
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/src/x"), None));
    let mux_a = MuxSessionId::new("editor");
    let mux_b = MuxSessionId::new("scratch");
    for native in ["editor", "scratch"] {
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
    }
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    for (idx, mux) in [mux_a, mux_b].into_iter().enumerate() {
        snapshot.candidate_links.push(GraphLink {
            id: format!("link-{idx}"),
            source: session_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux),
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
    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    let kinds: Vec<SectionKind> = detail.sections().iter().map(|s| s.kind).collect();
    assert!(
        kinds.contains(&SectionKind::Mux),
        "ambiguous-mux session keeps the Mux section: got {kinds:?}",
    );
}

#[test]
fn agent_session_with_title_inserts_title_row() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent(
        "opencode",
        "abc",
        Some("/home/op/src/x"),
        Some("Phase 8 mockup"),
    ));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "abc"));
    let detail = build(&snapshot, &target, Some(home().as_path()));
    let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
    assert_eq!(
        labels,
        vec!["id", "harness", "cwd", "title", "mux", "pr", "lineage"]
    );
    let title = detail
        .header_fields
        .iter()
        .find(|f| f.label == "title")
        .unwrap();
    assert_eq!(title.value, "Phase 8 mockup");
    assert!(!title.placeholder);
}

#[test]
fn agent_session_detail_uses_full_native_session_id() {
    let long_id = "019eced4-4fb0-70d1-b8f4-72de7c469e62";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", long_id, Some("/home/op/src/x"), None));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", long_id));
    let detail = build(&snapshot, &target, Some(home().as_path()));
    let session = detail
        .header_fields
        .iter()
        .find(|f| f.label == "id")
        .expect("id field");

    assert_eq!(session.value, long_id);
    assert!(
        !session.value.contains('…'),
        "right pane session id should not be truncated: {}",
        session.value
    );
}

#[test]
fn mux_session_detail_uses_full_native_session_name() {
    let long_native_id = "agentdeck_worktrunk-multi-repo_d459b661-extra-long-copyable-session-name";
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(long_native_id),
        backend: "tmux".into(),
        native_id: long_native_id.into(),
        cwd: Some("/home/op/src/worktrunk".into()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::MuxSession(MuxSessionId::new(long_native_id));
    let detail = build(&snapshot, &target, Some(home().as_path()));
    let mux = detail
        .header_fields
        .iter()
        .find(|f| f.label == "name")
        .expect("name field");

    assert_eq!(mux.value, long_native_id);
    assert!(
        !mux.value.contains('…'),
        "right pane mux name should not be truncated: {}",
        mux.value
    );
}

#[test]
fn alias_replaces_title_in_detail_header_per_adr_0029() {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(agent(
        "opencode",
        "abc",
        Some("/home/op/src/x"),
        Some("harness title that should be hidden"),
    ));
    let session_id = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "abc"));
    snapshot
        .aliases
        .insert(session_id.clone(), "ingest-refactor".to_string());
    let snapshot = resolve_snapshot(snapshot);
    let detail = build(&snapshot, &session_id, Some(home().as_path()));

    let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
    assert_eq!(
        labels,
        vec!["id", "harness", "cwd", "alias", "mux", "pr", "lineage"],
        "alias row replaces title row when both would be present"
    );
    let alias = detail
        .header_fields
        .iter()
        .find(|f| f.label == "alias")
        .expect("alias header present");
    assert_eq!(alias.value, "ingest-refactor");
}

#[test]
fn ambiguous_mux_annotates_with_warning_and_count() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
    let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/src/x"), None));
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("scratch"),
        backend: "tmux".into(),
        native_id: "scratch".into(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.candidate_links.push(GraphLink {
        id: "mux-1".into(),
        source: session_id.clone(),
        target: LinkEndpoint::Node { id: editor },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    snapshot.candidate_links.push(GraphLink {
        id: "mux-2".into(),
        source: session_id.clone(),
        target: LinkEndpoint::Node { id: scratch },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    let mux = detail
        .header_fields
        .iter()
        .find(|f| f.label == "mux")
        .unwrap();
    assert!(mux.value.contains("tmux:editor"), "value={}", mux.value);
    assert!(mux.value.contains("2 candidates"), "value={}", mux.value);
    assert_eq!(mux.annotation, Some("⚠"));
}

#[test]
fn agent_session_pr_field_walks_worktree_branch_pr_chain() {
    // session at cwd /home/op/src/x → worktree → branch main →
    // PR octo/repo#7 (open).
    let cwd = "/home/op/src/x";
    let repo_id = RepoId::new(cwd);
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo_id.clone(), cwd.to_string()),
        root: cwd.to_string(),
        git_dir: None,
        current_branch: None,
    }));
    let branch_id = crate::model::BranchId::new(repo_id.clone(), "refs/heads/main");
    snapshot
        .nodes
        .push(GraphNode::Branch(crate::model::BranchNode {
            id: branch_id.clone(),
            refname: "refs/heads/main".to_string(),
            current_commit: None,
            upstream: None,
        }));
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
        id: pr_id.clone(),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "octo".into(),
        repo: "repo".into(),
        number: 7,
        state: Some("open".into()),
        url: None,
        updated_epoch: None,
        is_draft: false,
    }));
    snapshot.nodes.push(agent("codex", "abc", Some(cwd), None));

    // Worktree → Branch (CheckedOutBranch)
    snapshot.candidate_links.push(GraphLink {
        id: "wt-branch".into(),
        source: NodeId::Checkout(CheckoutId::new(repo_id, cwd.to_string())),
        target: LinkEndpoint::Node {
            id: NodeId::Branch(branch_id.clone()),
        },
        relation: RelationKind::CheckedOutBranch,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    // Branch → PR (BranchHasForgePr)
    snapshot.candidate_links.push(GraphLink {
        id: "branch-pr".into(),
        source: NodeId::Branch(branch_id),
        target: LinkEndpoint::Node {
            id: NodeId::ForgePr(pr_id),
        },
        relation: RelationKind::BranchHasForgePr,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

    let detail = build(&snapshot, &target, Some(home().as_path()));
    let pr = detail
        .header_fields
        .iter()
        .find(|f| f.label == "pr")
        .unwrap();
    assert_eq!(pr.value, "octo/repo#7 (open)");
    assert!(!pr.placeholder);
}

#[test]
fn mux_session_detail_counts_attached_agents() {
    let mux_id = MuxSessionId::new("editor");
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: mux_id.clone(),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: Some("/home/op/src/x".into()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot
        .nodes
        .push(agent("codex", "a", Some("/home/op/src/x"), None));
    snapshot
        .nodes
        .push(agent("codex", "b", Some("/home/op/src/x"), None));
    for (i, session_key) in ["a", "b"].iter().enumerate() {
        snapshot.candidate_links.push(GraphLink {
            id: format!("attached-{i}"),
            source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", *session_key)),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id.clone()),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    let snapshot = resolve_snapshot(snapshot);
    let target = NodeId::MuxSession(mux_id);
    let detail = build(&snapshot, &target, Some(home().as_path()));
    assert_eq!(detail.kind_label, "mux_session");
    assert_eq!(detail.title_line, "tmux:editor");
    let attached = detail
        .header_fields
        .iter()
        .find(|f| f.label == "attached")
        .unwrap();
    assert_eq!(attached.value, "2");
    let sections = detail.sections();
    assert_eq!(
        sections.iter().map(|s| s.kind).collect::<Vec<_>>(),
        vec![SectionKind::Mux, SectionKind::Session],
        "mux details should lead with mux fields, then attached sessions"
    );
    let mux_field_labels: Vec<&str> = sections[0].fields.iter().map(|f| f.label).collect();
    assert_eq!(mux_field_labels, vec!["name", "backend", "cwd", "attached"]);
    let session_field_labels: Vec<&str> = sections[1].fields.iter().map(|f| f.label).collect();
    assert_eq!(session_field_labels, vec!["session", "session"]);
    let session_values: Vec<&str> = sections[1]
        .fields
        .iter()
        .map(|f| f.value.as_str())
        .collect();
    assert_eq!(session_values, vec!["codex:a", "codex:b"]);
    let first_session = sections[1].fields.first().unwrap();
    assert_eq!(
        first_session
            .expanded_fields
            .iter()
            .map(|field| field.label)
            .collect::<Vec<_>>(),
        vec!["id", "harness", "cwd", "mux", "pr", "lineage"],
        "attached session row should carry one-level details for expansion"
    );
    let cwd = detail
        .header_fields
        .iter()
        .find(|f| f.label == "cwd")
        .unwrap();
    assert_eq!(cwd.value, "~/src/x");
}

#[test]
fn process_links_surface_on_agent_and_mux_details() {
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let process_id = NodeId::RuntimeProcess(RuntimeProcessId::new("tmux:editor:pid:4242"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: Some("/home/op/src/x".into()),
        active_pane_command: None,
        active_pane_pid: Some(4242),
        active_pane_current_path: Some("/home/op/src/x".into()),
        active_pane_start_command: Some("codex".into()),
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/src/x"), None));
    snapshot
        .nodes
        .push(runtime_process("tmux:editor:pid:4242", 4242, "codex"));
    snapshot.candidate_links.push(process_link(
        "mux-process",
        mux_id.clone(),
        process_id.clone(),
        RelationKind::MuxContainsProcess,
    ));
    snapshot.candidate_links.push(process_link(
        "process-session",
        process_id.clone(),
        session_id.clone(),
        RelationKind::ProcessIdentifiesSession,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let session_detail = build(&snapshot, &session_id, Some(home().as_path()));
    let session_process = session_detail
        .header_fields
        .iter()
        .find(|field| field.label == "process")
        .expect("session process field");
    assert_eq!(session_process.value, "pid 4242: codex");
    assert_eq!(session_process.target, Some(process_id.clone()));

    let mux_detail = build(&snapshot, &mux_id, Some(home().as_path()));
    let mux_process = mux_detail
        .sections()
        .into_iter()
        .find(|section| section.kind == SectionKind::Process)
        .and_then(|section| section.fields.into_iter().next())
        .expect("mux process field");
    assert_eq!(mux_process.value, "pid 4242: codex");
    assert_eq!(mux_process.target, Some(process_id));
}

#[test]
fn runtime_process_detail_links_back_to_mux_and_session() {
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let process_id = NodeId::RuntimeProcess(RuntimeProcessId::new("tmux:editor:pid:4242"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.nodes.push(agent("codex", "abc", None, None));
    snapshot
        .nodes
        .push(runtime_process("tmux:editor:pid:4242", 4242, "codex"));
    snapshot.candidate_links.push(process_link(
        "mux-process",
        mux_id.clone(),
        process_id.clone(),
        RelationKind::MuxContainsProcess,
    ));
    snapshot.candidate_links.push(process_link(
        "process-session",
        process_id.clone(),
        session_id.clone(),
        RelationKind::ProcessCandidatesSession,
    ));
    let snapshot = resolve_snapshot(snapshot);

    let detail = build(&snapshot, &process_id, Some(home().as_path()));
    let sections = detail.sections();
    assert_eq!(
        sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![SectionKind::Process, SectionKind::Mux, SectionKind::Session]
    );
    let mux = detail
        .header_fields
        .iter()
        .find(|field| field.label == "mux")
        .expect("mux field");
    assert_eq!(mux.value, "tmux:editor");
    assert_eq!(mux.target, Some(mux_id));
    let session = detail
        .header_fields
        .iter()
        .find(|field| field.label == "session")
        .expect("session field");
    assert_eq!(session.value, "codex:abc");
    assert_eq!(session.target, Some(session_id));
    assert_eq!(session.annotation, Some("⚠"));
}

#[test]
fn agent_session_mux_row_carries_mux_detail_for_expansion() {
    let mux_id = MuxSessionId::new("editor");
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: mux_id.clone(),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: Some("/home/op/src/x".into()),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/src/x"), None));
    snapshot.candidate_links.push(GraphLink {
        id: "attached".into(),
        source: session_id.clone(),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_id),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);
    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    let mux = detail
        .header_fields
        .iter()
        .find(|field| field.label == "mux")
        .expect("mux field");

    assert_eq!(
        mux.target,
        Some(NodeId::MuxSession(MuxSessionId::new("editor")))
    );
    assert_eq!(
        mux.expanded_fields
            .iter()
            .map(|field| field.label)
            .collect::<Vec<_>>(),
        vec!["name", "backend", "cwd", "attached", "session"]
    );
}

#[test]
fn outgoing_and_incoming_link_summaries_populate_from_snapshot() {
    let mut snapshot = GraphSnapshot::empty();
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    snapshot
        .nodes
        .push(agent("codex", "abc", Some("/home/op/x"), None));
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.candidate_links.push(GraphLink {
        id: "link-1".into(),
        source: session_id.clone(),
        target: LinkEndpoint::Node { id: mux_id.clone() },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    let session_detail = build(&snapshot, &session_id, None);
    assert_eq!(session_detail.outgoing_links.len(), 1);
    assert!(session_detail.incoming_links.is_empty());

    let mux_detail = build(&snapshot, &mux_id, None);
    assert!(mux_detail.outgoing_links.is_empty());
    assert_eq!(mux_detail.incoming_links.len(), 1);
}

fn workspace_node(root: &str, provider: Option<&str>, name: Option<&str>) -> GraphNode {
    GraphNode::Workspace(crate::model::WorkspaceNode {
        id: crate::model::WorkspaceId::new(root),
        root: root.to_string(),
        provider: provider.map(str::to_string),
        name: name.map(str::to_string),
    })
}

fn repo_graph_node(common_dir: &str) -> GraphNode {
    GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
}

fn workspace_contains_repo_link(
    link_id: &str,
    workspace_root: &str,
    repo_common_dir: &str,
    logical_path: &str,
) -> GraphLink {
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(logical_path.to_string()),
    );
    GraphLink {
        id: link_id.to_string(),
        source: NodeId::Workspace(crate::model::WorkspaceId::new(workspace_root)),
        target: LinkEndpoint::Node {
            id: NodeId::Repo(RepoId::new(repo_common_dir)),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: None,
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn session_associated_with_workspace(session_id: NodeId, workspace_root: &str) -> GraphLink {
    GraphLink {
        id: format!("assoc-{workspace_root}"),
        source: session_id,
        target: LinkEndpoint::Node {
            id: NodeId::Workspace(crate::model::WorkspaceId::new(workspace_root)),
        },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn workspace_detail_lists_member_repos_as_linked_targets() {
    let workspace_root = "/work/atelier";
    let repo_a = "/work/atelier/atelier/.git";
    let repo_b = "/work/atelier/conspectus/.git";
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
            repo_graph_node(repo_a),
            repo_graph_node(repo_b),
        ],
        candidate_links: vec![
            workspace_contains_repo_link(
                "ws-atelier",
                workspace_root,
                repo_a,
                "/work/atelier/atelier",
            ),
            workspace_contains_repo_link(
                "ws-conspectus",
                workspace_root,
                repo_b,
                "/work/atelier/conspectus",
            ),
        ],
        ..GraphSnapshot::empty()
    });

    let target = NodeId::Workspace(crate::model::WorkspaceId::new(workspace_root));
    let detail = build(&snapshot, &target, Some(home().as_path()));

    let members: Vec<&HeaderField> = detail
        .header_fields
        .iter()
        .filter(|f| f.label == "member")
        .collect();
    assert_eq!(members.len(), 2, "two member fields expected");
    let displays: Vec<&str> = members.iter().map(|f| f.value.as_str()).collect();
    assert!(displays.contains(&"atelier"));
    assert!(displays.contains(&"conspectus"));
    for member in &members {
        assert!(
            matches!(member.target, Some(NodeId::Repo(_))),
            "member targets a Repo NodeId"
        );
    }
}

#[test]
fn workspace_detail_omits_member_rows_when_no_members_resolved() {
    let workspace_root = "/work/empty";
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![workspace_node(workspace_root, None, None)],
        ..GraphSnapshot::empty()
    });

    let target = NodeId::Workspace(crate::model::WorkspaceId::new(workspace_root));
    let detail = build(&snapshot, &target, Some(home().as_path()));

    assert!(
        detail.header_fields.iter().all(|f| f.label != "member"),
        "no member fields expected when the workspace has no resolved repos"
    );
}

#[test]
fn session_detail_lists_associated_workspaces_as_linked_targets() {
    let workspace_root = "/work/atelier";
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "alpha"));
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent(
                "codex",
                "alpha",
                Some("/work/atelier/conspectus/crates/core"),
                None,
            ),
            workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
        ],
        candidate_links: vec![session_associated_with_workspace(
            session_id.clone(),
            workspace_root,
        )],
        ..GraphSnapshot::empty()
    });

    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    let workspace_field = detail
        .header_fields
        .iter()
        .find(|f| f.label == "workspace")
        .expect("workspace field present");
    assert_eq!(workspace_field.value, "atelier-ws");
    assert_eq!(
        workspace_field.target,
        Some(NodeId::Workspace(crate::model::WorkspaceId::new(
            workspace_root
        )))
    );
}

#[test]
fn session_detail_emits_one_workspace_row_per_association() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "alpha"));
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent("codex", "alpha", Some("/work/a/x"), None),
            workspace_node("/work/a", None, Some("alpha-ws")),
            workspace_node("/work/b", None, Some("beta-ws")),
        ],
        candidate_links: vec![
            session_associated_with_workspace(session_id.clone(), "/work/a"),
            session_associated_with_workspace(session_id.clone(), "/work/b"),
        ],
        ..GraphSnapshot::empty()
    });

    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    let rows: Vec<&HeaderField> = detail
        .header_fields
        .iter()
        .filter(|f| f.label == "workspace")
        .collect();
    assert_eq!(rows.len(), 2);
    let displays: Vec<&str> = rows.iter().map(|f| f.value.as_str()).collect();
    assert!(displays.contains(&"alpha-ws"));
    assert!(displays.contains(&"beta-ws"));
}

#[test]
fn session_detail_omits_workspace_row_when_no_association() {
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "alpha"));
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![agent("codex", "alpha", Some("/work/x"), None)],
        ..GraphSnapshot::empty()
    });

    let detail = build(&snapshot, &session_id, Some(home().as_path()));
    assert!(
        detail.header_fields.iter().all(|f| f.label != "workspace"),
        "no workspace field expected when the session has no workspace association"
    );
}

#[test]
fn workspace_member_links_expand_inline_under_linked_details() {
    // attach_linked_details should walk the workspace's `member`
    // fields and inline the Repo node's own header fields. This
    // is the "navigate workspace → repo as naturally as the rest
    // of the graph" expectation made explicit.
    let workspace_root = "/work/atelier";
    let repo_a = "/work/atelier/atelier/.git";
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent("codex", "alpha", Some("/work/atelier/atelier/src"), None),
            workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
            repo_graph_node(repo_a),
            repo_graph_node("/work/atelier/conspectus/.git"),
        ],
        candidate_links: vec![
            session_associated_with_workspace(
                NodeId::AgentSession(AgentSessionId::new("codex", "/state", "alpha")),
                workspace_root,
            ),
            workspace_contains_repo_link(
                "ws-atelier",
                workspace_root,
                repo_a,
                "/work/atelier/atelier",
            ),
            workspace_contains_repo_link(
                "ws-conspectus",
                workspace_root,
                "/work/atelier/conspectus/.git",
                "/work/atelier/conspectus",
            ),
        ],
        ..GraphSnapshot::empty()
    });

    let target = NodeId::Workspace(crate::model::WorkspaceId::new(workspace_root));
    let detail = build(&snapshot, &target, Some(home().as_path()));
    let atelier_member = detail
        .header_fields
        .iter()
        .find(|f| f.label == "member" && f.value == "atelier")
        .expect("atelier member field");
    assert_eq!(atelier_member.expanded_kind_label, Some("repo"));
    assert!(
        !atelier_member.expanded_fields.is_empty(),
        "linked details should inline the repo's fields"
    );
}

#[test]
fn ambiguous_mux_section_aggregates_workspace_scoped_sessions() {
    // ADR 0071: when multiple A-class sessions in the same
    // workspace are each ambiguously linked to ≥2 muxes, the
    // workspace detail's `AmbiguousMux` section surfaces those
    // muxes once at the shared parent, not duplicated per
    // session.
    let workspace_id = WorkspaceId::new("/home/op/ws");
    let workspace = NodeId::Workspace(workspace_id.clone());
    let session_a = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "a"));
    let session_b = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "b"));
    let mux_x = NodeId::MuxSession(MuxSessionId::new("editor"));
    let mux_y = NodeId::MuxSession(MuxSessionId::new("scratch"));

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
        root: "/home/op/ws".to_string(),
        provider: Some("atelier".to_string()),
        name: Some("ws".to_string()),
    }));
    for key in ["a", "b"] {
        snapshot
            .nodes
            .push(agent("claude-code", key, Some("/home/op/ws"), None));
    }
    for native in ["editor", "scratch"] {
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native),
            backend: "tmux".to_string(),
            native_id: native.to_string(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
    }
    let mut next_link = 0_usize;
    let mut link = |source: &NodeId, target: &NodeId, relation: RelationKind| -> GraphLink {
        next_link += 1;
        GraphLink {
            id: format!("test-{next_link}"),
            source: source.clone(),
            target: LinkEndpoint::Node { id: target.clone() },
            relation,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    };
    for session in [&session_a, &session_b] {
        snapshot
            .candidate_links
            .push(link(session, &workspace, RelationKind::AssociatedWith));
        for mux in [&mux_x, &mux_y] {
            snapshot
                .candidate_links
                .push(link(session, mux, RelationKind::LinkedToMux));
        }
    }
    let snapshot = resolve_snapshot(snapshot);

    let muxes = ambiguous_muxes_for_group(&snapshot, &workspace);
    assert_eq!(
        muxes.len(),
        2,
        "two distinct muxes surface once on the workspace, not per session: {muxes:?}",
    );
    assert!(muxes.contains(&mux_x));
    assert!(muxes.contains(&mux_y));

    let detail = build(&snapshot, &workspace, Some(home().as_path()));
    let amb_section = detail
        .sections()
        .into_iter()
        .find(|s| s.kind == SectionKind::AmbiguousMux)
        .expect("AmbiguousMux section emitted");
    assert_eq!(amb_section.fields.len(), 2);
    let labels: Vec<&str> = amb_section
        .fields
        .iter()
        .map(|f| f.value.as_str())
        .collect();
    assert!(labels.iter().any(|l| l.contains("editor")));
    assert!(labels.iter().any(|l| l.contains("scratch")));
}

#[test]
fn ambiguous_mux_section_suppressed_when_no_session_in_scope_is_ambiguous() {
    let workspace_id = WorkspaceId::new("/home/op/ws");
    let workspace = NodeId::Workspace(workspace_id.clone());
    let session = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"));
    let mux = NodeId::MuxSession(MuxSessionId::new("only"));

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
        root: "/home/op/ws".to_string(),
        provider: None,
        name: None,
    }));
    snapshot
        .nodes
        .push(agent("codex", "a", Some("/home/op/ws"), None));
    snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("only"),
        backend: "tmux".to_string(),
        native_id: "only".to_string(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
    }));
    snapshot.candidate_links.push(GraphLink {
        id: "assoc".to_string(),
        source: session.clone(),
        target: LinkEndpoint::Node {
            id: workspace.clone(),
        },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    snapshot.candidate_links.push(GraphLink {
        id: "mux".to_string(),
        source: session,
        target: LinkEndpoint::Node { id: mux },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    assert!(
        ambiguous_muxes_for_group(&snapshot, &workspace).is_empty(),
        "single mux per session is not ambiguous",
    );
    let detail = build(&snapshot, &workspace, Some(home().as_path()));
    assert!(
        detail
            .sections()
            .iter()
            .all(|s| s.kind != SectionKind::AmbiguousMux),
        "section is suppressed when scope has no ambiguity",
    );
}
