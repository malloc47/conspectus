// Extracted from cross_link.rs H-HYG-011 rolling wave via #[path = "cross_link_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutNode, ForkId, ForkNode, GraphSnapshot, MuxSessionId,
    MuxSessionNode, RepoId, RepoNode, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
};

fn session(id: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("codex", "/state", id),
        harness_key: "codex".to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn claude_session(id: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("claude-code", "/state", id),
        harness_key: "claude-code".to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn session_with_activity(id: &str, cwd: Option<&str>, last_active_epoch: i64) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("codex", "/state", id),
        harness_key: "codex".to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: None,
        last_active_epoch: Some(last_active_epoch),
        session_kind: None,
    })
}

fn mux(native: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native}")),
        backend: "tmux".to_string(),
        native_id: native.to_string(),
        cwd: cwd.map(str::to_string),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
    })
}

fn mux_with_active_command(native: &str, cwd: Option<&str>, command: &str) -> GraphNode {
    let active_pane_command = command.split_whitespace().next().map(str::to_string);
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native}")),
        backend: "tmux".to_string(),
        native_id: native.to_string(),
        cwd: cwd.map(str::to_string),
        active_pane_command,
        active_pane_pid: None,
        active_pane_current_path: cwd.map(str::to_string),
        active_pane_start_command: Some(command.to_string()),
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
    })
}

fn mux_with_active_process(native: &str, cwd: Option<&str>, command: &str, pid: i64) -> GraphNode {
    let active_pane_command = command.split_whitespace().next().map(str::to_string);
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native}")),
        backend: "tmux".to_string(),
        native_id: native.to_string(),
        cwd: cwd.map(str::to_string),
        active_pane_command,
        active_pane_pid: Some(pid),
        active_pane_current_path: cwd.map(str::to_string),
        active_pane_start_command: Some(command.to_string()),
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
    })
}

#[derive(Clone, Debug, Default)]
struct FakeProcessSnapshot {
    records: Vec<ProcessRecord>,
}

impl FakeProcessSnapshot {
    fn new(records: impl IntoIterator<Item = ProcessRecord>) -> Self {
        Self {
            records: records.into_iter().collect(),
        }
    }
}

impl ProcessSnapshot for FakeProcessSnapshot {
    fn process_records(&self) -> Vec<ProcessRecord> {
        self.records.clone()
    }
}

fn process(pid: i64, parent_pid: Option<i64>, command: &str, cwd: Option<&str>) -> ProcessRecord {
    ProcessRecord {
        pid,
        parent_pid,
        command: Some(command.to_string()),
        cwd: cwd.map(str::to_string),
    }
}

fn fork_node(key: &str) -> GraphNode {
    GraphNode::Fork(ForkNode {
        id: ForkId::new(key),
        provider: "atelier".to_string(),
        provider_source_key: key.to_string(),
        name: Some(key.to_string()),
        scope: Some("workspace".to_string()),
        capabilities: Vec::new(),
    })
}

fn worktree(repo_common_dir: &str, root: &str) -> GraphNode {
    let repo = RepoId::new(repo_common_dir);
    GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo, root.to_string()),
        root: root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
    })
}

fn repo(common_dir: &str) -> GraphNode {
    GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
}

fn workspace(root: &str) -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(root),
        root: root.to_string(),
        provider: None,
        name: None,
    })
}

fn workspace_contains_repo(
    workspace_root: &str,
    common_dir: &str,
    logical_path: &str,
) -> GraphLink {
    workspace_contains_repo_paths(workspace_root, common_dir, logical_path, logical_path)
}

/// Variant that lets tests model symlinked members: `logical_path`
/// is the workspace-visible location (e.g. inside the workspace
/// composite directory) and `canonical_checkout_root` is the
/// canonical worktree root the symlink target resolves to — they
/// only differ when the member is a symlink pointing outside the
/// workspace tree.
fn workspace_contains_repo_paths(
    workspace_root: &str,
    common_dir: &str,
    logical_path: &str,
    canonical_checkout_root: &str,
) -> GraphLink {
    let source = NodeId::Workspace(WorkspaceId::new(workspace_root));
    let target = NodeId::Repo(RepoId::new(common_dir));
    let mut fields = crate::model::Metadata::new();
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(logical_path.to_string()),
    );
    fields.insert(
        "canonical_checkout_root".to_string(),
        serde_json::Value::String(canonical_checkout_root.to_string()),
    );
    GraphLink {
        id: format!("test:{source}:workspace_contains_repo:{target}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: Some("test workspace member".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn rooted_at_path(fork_key: &str, path: &str) -> GraphLink {
    let source = NodeId::Fork(ForkId::new(fork_key));
    GraphLink {
        id: format!("test:{source}:rooted_at_path:{path}"),
        source,
        target: LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: "path".to_string(),
                harness_key: None,
                native_id: None,
                state_scope: None,
                path: Some(path.to_string()),
                metadata: Default::default(),
            },
        },
        relation: RelationKind::RootedAtPath,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "atelier".to_string(),
            evidence: Some("test rooted_at_path".to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn parent_session_link(child: &str, parent: &str) -> GraphLink {
    let source = NodeId::AgentSession(AgentSessionId::new("codex", "/state", child));
    let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", parent));
    GraphLink {
        id: format!("test:{source}:parent_session:{target}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: Some("test parent".to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

#[test]
fn orphan_sessions_get_no_links() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("orphan", Some("/work/orphan"))],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert!(snapshot.candidate_links.is_empty());
}

#[test]
fn mux_only_snapshot_gets_no_links() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux("only", Some("/work/repo"))],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert!(snapshot.candidate_links.is_empty());
}

#[test]
fn exact_cwd_match_yields_strong_linked_to_mux() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/work/repo")),
            mux("one", Some("/work/repo")),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert_eq!(snapshot.candidate_links.len(), 1);
    let link = &snapshot.candidate_links[0];
    assert_eq!(link.relation, RelationKind::LinkedToMux);
    assert_eq!(link.provenance, Provenance::StrongDiscovered);
    assert_eq!(link.confidence, Confidence::High);
}

#[test]
fn plain_shell_active_pane_suppresses_cwd_only_mux_links() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("stale", Some("/work/repo")),
            mux_with_active_command("shell", Some("/work/repo"), "zsh"),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert!(
        snapshot
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::LinkedToMux),
        "plain shell mux should not claim stale sessions by cwd: {:#?}",
        snapshot.candidate_links
    );
}

#[test]
fn cwd_match_generated_across_harnesses_when_no_pid_match() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::AgentSession(
                AgentSessionNode::new(
                    AgentSessionId::new("opencode", "/state", "a"),
                    "opencode".to_string(),
                )
                .with_cwd("/work/repo".to_string()),
            ),
            GraphNode::AgentSession(
                AgentSessionNode::new(
                    AgentSessionId::new("codex", "/state", "b"),
                    "codex".to_string(),
                )
                .with_cwd("/work/repo".to_string()),
            ),
            mux_with_active_command("one", Some("/work/repo"), "opencode"),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert_eq!(
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref() == Some("exact_cwd_match")
            })
            .count(),
        2
    );
}

#[test]
fn session_matches_multiple_mux_sessions_with_preserved_candidates() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/work/repo")),
            mux("one", Some("/work/repo")),
            mux("two", Some("/work/repo")),
            mux("three", Some("/work/repo/sub")),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(
        mux_links.len(),
        3,
        "every plausible mux candidate is preserved"
    );

    assert_eq!(
        mux_links
            .iter()
            .filter(|link| link.provenance == Provenance::StrongDiscovered)
            .count(),
        2,
        "two exact-cwd matches"
    );
}

#[test]
fn active_pane_command_session_match_suppresses_cwd_only_mux_links() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("current", Some("/work/repo")),
            session("stale", Some("/work/repo")),
            mux_with_active_command("one", Some("/work/repo"), "codex resume current"),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    let link = mux_links[0];
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "current"))
    );
    assert_eq!(
        link.source_metadata.evidence.as_deref(),
        Some("active_pane_command_session_match")
    );
}

#[test]
fn opencode_file_activity_supersedes_stale_command_session_id() {
    let stale = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "/state", "ses_stale"),
            "opencode".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(1_000),
    );
    let current = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "/state", "ses_current"),
            "opencode".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(5_020),
    );
    let mux = GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new("tmux:one"),
            "tmux".to_string(),
            "one".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_active_pane_command("opencode".to_string())
        .with_active_pane_current_path("/work/repo".to_string())
        .with_active_pane_start_command("opencode -s ses_stale".to_string())
        .with_activity_epoch(5_030)
        .with_created_epoch(5_000),
    );
    let mut snapshot = GraphSnapshot {
        nodes: vec![stale, current, mux],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    let link = mux_links[0];
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "ses_current"))
    );
    assert_eq!(
        link.source_metadata.evidence.as_deref(),
        Some("session_file_activity_match")
    );
}

#[test]
fn active_pane_process_match_links_direct_harness_process() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("target-session", Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("process link");
    assert_eq!(
        link.source_metadata.evidence.as_deref(),
        Some("active_pane_process_match")
    );
    assert_eq!(
        link.source_metadata
            .fields
            .get("matched_pid")
            .and_then(serde_json::Value::as_i64),
        Some(100)
    );

    let process_node = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::RuntimeProcess(process) => Some(process),
            _ => None,
        })
        .expect("runtime process node");
    assert_eq!(process_node.pid, Some(100));
    assert_eq!(process_node.root_pane_pid, Some(100));
    assert_eq!(process_node.harness_key.as_deref(), Some("codex"));
    assert_eq!(process_node.role, Some(RuntimeProcessRole::HumanAgent));

    let process_id = NodeId::RuntimeProcess(process_node.id.clone());
    assert!(snapshot.candidate_links.iter().any(|link| {
        link.relation == RelationKind::MuxContainsProcess
            && link.source == NodeId::MuxSession(MuxSessionId::new("tmux:editor"))
            && link.target_node_id() == Some(&process_id)
    }));
    assert!(snapshot.candidate_links.iter().any(|link| {
        link.relation == RelationKind::ProcessCandidatesSession
            && link.source == process_id
            && link.target_node_id()
                == Some(&NodeId::AgentSession(AgentSessionId::new(
                    "codex",
                    "/state",
                    "target-session",
                )))
    }));
}

#[test]
fn active_pane_process_match_walks_nested_shell_children() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("target-session", Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([
        process(100, None, "bash", Some("/work/repo")),
        process(101, Some(100), "zsh", Some("/work/repo")),
        process(102, Some(101), "/usr/bin/codex exec", Some("/work/repo")),
    ]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("process link");
    assert_eq!(
        link.source_metadata.evidence.as_deref(),
        Some("active_pane_process_match")
    );
    assert_eq!(
        link.source_metadata
            .fields
            .get("process_depth")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
}

#[test]
fn active_pane_process_match_ignores_claude_background_children() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            claude_session("16e486da-3fa2-425d-8e59-6838c3d97895", Some("/work/repo")),
            claude_session("7e2c3973-0b13-4c04-a39d-08100591dd23", Some("/work/repo")),
            mux_with_active_process(
                "editor",
                Some("/work/repo"),
                "claude --resume 7e2c3973-0b13-4c04-a39d-08100591dd23",
                100,
            ),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([
        process(
            100,
            None,
            "claude --resume 7e2c3973-0b13-4c04-a39d-08100591dd23",
            Some("/work/repo"),
        ),
        process(
            101,
            Some(100),
            ".claude-wrapped daemon run --origin transient --spawned-by {}",
            Some("/work/repo"),
        ),
        process(
            102,
            Some(101),
            ".claude-wrapped --bg-spare /tmp/cc-daemon/spare/foo.claim.sock",
            Some("/work/repo"),
        ),
    ]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let process_nodes: Vec<_> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::RuntimeProcess(process) => Some(process),
            _ => None,
        })
        .collect();
    assert_eq!(process_nodes.len(), 1);
    assert_eq!(process_nodes[0].pid, Some(100));
    assert_eq!(process_nodes[0].role, Some(RuntimeProcessRole::HumanAgent));

    assert!(
        snapshot.candidate_links.iter().all(|link| {
            if !matches!(
                link.relation,
                RelationKind::MuxContainsProcess
                    | RelationKind::ProcessIdentifiesSession
                    | RelationKind::ProcessCandidatesSession
            ) {
                return true;
            }
            link.source.to_string().contains("pid:100")
                || link
                    .target_node_id()
                    .is_some_and(|target| target.to_string().contains("pid:100"))
        }),
        "background daemon/spare children must not emit mux process links"
    );
}

#[test]
fn active_pane_process_match_does_not_fan_out_across_same_cwd_sessions() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("old-session", Some("/work/repo")),
            session("new-session", Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let concrete_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.source, NodeId::AgentSession(_))
        })
        .collect();
    assert!(
        concrete_links.is_empty(),
        "ambiguous cwd process evidence should not fan out: {concrete_links:#?}"
    );

    let unresolved = snapshot
        .candidate_links
        .iter()
        .find(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.source, NodeId::MuxSession(_))
        })
        .expect("unresolved process evidence");
    assert_eq!(
        unresolved.source_metadata.evidence.as_deref(),
        Some("active_pane_process_match")
    );
}

#[test]
fn active_pane_process_match_uses_process_command_session_key() {
    let target = "019e434b-9eff-7110-b2af-7c963aa8085e";
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("019e4354-26b9-7ad2-9521-4ad921cc312b", Some("/work/repo")),
            session(target, Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(
        100,
        None,
        &format!("codex resume {target}"),
        Some("/work/repo"),
    )]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.source, NodeId::AgentSession(_))
        })
        .expect("exact process command link");
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", target))
    );
    assert_eq!(
        link.source_metadata
            .fields
            .get("process_session_keys")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let process_link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::ProcessIdentifiesSession)
        .expect("process identifies session link");
    assert_eq!(
        process_link.target_node_id(),
        Some(&NodeId::AgentSession(AgentSessionId::new(
            "codex", "/state", target
        )))
    );
}

#[test]
fn active_pane_process_match_uses_opencode_string_session_key() {
    let target = "ses_16d204c1dffeIkBjxxbpD9uujI";
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            opencode_session("ses_18f56c3f2ffeBjnY8BDLxIC7ru", Some("/work/repo")),
            opencode_session(target, Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "opencode", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(
        100,
        None,
        &format!("opencode -s {target}"),
        Some("/work/repo"),
    )]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.source, NodeId::AgentSession(_))
        })
        .expect("exact opencode process command link");
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("opencode", "/state", target))
    );

    let process_link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::ProcessIdentifiesSession)
        .expect("process identifies opencode session link");
    assert_eq!(
        process_link.target_node_id(),
        Some(&NodeId::AgentSession(AgentSessionId::new(
            "opencode", "/state", target
        )))
    );
}

#[test]
fn active_pane_fd_session_suppresses_conflicting_process_resume_key() {
    let fd_target = "019e7733-0be9-7720-b828-e185f9029793";
    let process_target = "019e434b-9eff-7110-b2af-7c963aa8085e";
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session(process_target, Some("/work/repo")),
            session(fd_target, Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "codex", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(
        100,
        None,
        &format!("codex resume {process_target}"),
        Some("/work/repo"),
    )]);
    let fd_path = format!("/home/me/.codex/sessions/rollout-{fd_target}.jsonl");

    infer_with_readers(
        &mut snapshot,
        |pid| (pid == 100).then(|| session_key_evidence_from_fd_paths([fd_path.as_str()])),
        Some(&processes),
    );

    let concrete_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.source, NodeId::AgentSession(_))
        })
        .collect();
    assert_eq!(concrete_links.len(), 1);
    assert_eq!(
        concrete_links[0].source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", fd_target))
    );
    assert_eq!(
        concrete_links[0].source_metadata.evidence.as_deref(),
        Some("active_pane_fd_session_match")
    );
}

#[test]
fn active_pane_fd_session_emits_runtime_process_without_process_tree() {
    let target = "019e7733-0be9-7720-b828-e185f9029793";
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session(target, Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "codex", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let fd_path = format!("/home/me/.codex/sessions/rollout-{target}.jsonl");
    let fd_paths = BTreeMap::from([(100, vec![fd_path])]);

    infer_with_fd_paths(&mut snapshot, &fd_paths);

    let process_node = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::RuntimeProcess(process) => Some(process),
            _ => None,
        })
        .expect("runtime process node");
    assert_eq!(process_node.pid, Some(100));
    assert_eq!(process_node.root_pane_pid, Some(100));
    assert_eq!(process_node.harness_key.as_deref(), Some("codex"));
    assert_eq!(process_node.depth, Some(0));

    let process_id = NodeId::RuntimeProcess(process_node.id.clone());
    assert!(snapshot.candidate_links.iter().any(|link| {
        link.relation == RelationKind::MuxContainsProcess
            && link.source == NodeId::MuxSession(MuxSessionId::new("tmux:editor"))
            && link.target_node_id() == Some(&process_id)
    }));
    assert!(snapshot.candidate_links.iter().any(|link| {
        link.relation == RelationKind::ProcessIdentifiesSession
            && link.source == process_id
            && link.target_node_id()
                == Some(&NodeId::AgentSession(AgentSessionId::new(
                    "codex", "/state", target,
                )))
            && link.source_metadata.evidence.as_deref() == Some("active_pane_fd_session_match")
    }));
}

#[test]
fn single_agent_process_collapses_activity_matches_to_one_session() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session_with_activity("older", Some("/work/repo"), 1_700_000_010),
            session_with_activity("newer", Some("/work/repo"), 1_700_000_020),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
        mux.created_epoch = Some(1_700_000_000);
    }
    let processes = FakeProcessSnapshot::new([process(100, None, "codex", Some("/work/repo"))]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let activity_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source_metadata.evidence.as_deref() == Some("session_file_activity_match")
        })
        .collect();
    assert_eq!(activity_links.len(), 1);
    assert_eq!(
        activity_links[0].source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "newer"))
    );
}

#[test]
fn multiple_agent_processes_allow_multiple_session_attribution() {
    let codex_id = "019e7733-0be9-7720-b828-e185f9029793";
    let claude_id = "c1901a9e-f3db-48e3-a46c-3f92c7c0f2d3";
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session(codex_id, Some("/work/repo")),
            GraphNode::AgentSession(
                AgentSessionNode::new(
                    AgentSessionId::new("claude-code", "/state", claude_id),
                    "claude-code".to_string(),
                )
                .with_cwd("/work/repo".to_string()),
            ),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([
        process(100, None, "bash", Some("/work/repo")),
        process(
            101,
            Some(100),
            &format!("codex resume {codex_id}"),
            Some("/work/repo"),
        ),
        process(
            102,
            Some(100),
            &format!("claude --resume {claude_id}"),
            Some("/work/repo"),
        ),
    ]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    assert_eq!(
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref() == Some("active_pane_process_match")
            })
            .count(),
        2
    );
}

#[test]
fn active_pane_process_unknown_binary_degrades_without_link() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("target-session", Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::new([process(100, None, "vim", Some("/work/repo"))]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    assert!(
        snapshot
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::LinkedToMux)
    );
}

#[test]
fn active_pane_process_missing_pid_degrades_without_link() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("target-session", Some("/work/repo")),
            mux_with_active_process("editor", Some("/work/repo"), "bash", 100),
        ],
        ..GraphSnapshot::empty()
    };
    let processes = FakeProcessSnapshot::default();

    infer_with_process_snapshot(&mut snapshot, &processes);

    assert!(
        snapshot
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::LinkedToMux)
    );
}

#[test]
fn active_pane_process_preserves_unresolved_agent_evidence() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux_with_active_process(
            "editor",
            Some("/work/repo"),
            "bash",
            100,
        )],
        ..GraphSnapshot::empty()
    };
    let processes =
        FakeProcessSnapshot::new([process(101, Some(100), "codex", Some("/work/repo"))]);

    infer_with_process_snapshot(&mut snapshot, &processes);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("unresolved process evidence");
    assert!(matches!(link.source, NodeId::MuxSession(_)));
    let LinkEndpoint::Unresolved { evidence } = &link.target else {
        panic!("expected unresolved target");
    };
    assert_eq!(evidence.node_type, "agent_session");
    assert_eq!(evidence.harness_key.as_deref(), Some("codex"));
    assert_eq!(evidence.path.as_deref(), Some("/work/repo"));
}

#[test]
fn session_file_activity_match_links_recent_same_harness_session() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session_with_activity("recent", Some("/work/repo"), 1_700_000_050),
            session_with_activity("stale", Some("/work/repo"), 1_699_000_000),
            mux_with_active_command("one", Some("/work/repo"), "codex"),
        ],
        ..GraphSnapshot::empty()
    };
    if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
        mux.created_epoch = Some(1_700_000_000);
    }

    infer_without_process_tree(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    assert_eq!(
        mux_links[0].source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "recent"))
    );
    assert_eq!(
        mux_links[0].source_metadata.evidence.as_deref(),
        Some("session_file_activity_match")
    );
}

#[test]
fn stale_session_file_activity_does_not_emit_activity_match() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session_with_activity("stale", Some("/work/repo"), 1_699_000_000),
            mux_with_active_command("one", Some("/work/repo"), "codex"),
        ],
        ..GraphSnapshot::empty()
    };
    if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[1] {
        mux.created_epoch = Some(1_700_000_000);
    }

    infer_without_process_tree(&mut snapshot);

    assert!(
        snapshot.candidate_links.iter().all(|link| {
            link.source_metadata.evidence.as_deref() != Some("session_file_activity_match")
        }),
        "stale session activity should not become activity evidence: {:#?}",
        snapshot.candidate_links
    );
}

#[test]
fn ambiguous_same_cwd_activity_matches_remain_candidates() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session_with_activity("one", Some("/work/repo"), 1_700_000_010),
            session_with_activity("two", Some("/work/repo"), 1_700_000_020),
            mux_with_active_command("one", Some("/work/repo"), "codex"),
        ],
        ..GraphSnapshot::empty()
    };
    if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
        mux.created_epoch = Some(1_700_000_000);
    }

    infer_without_process_tree(&mut snapshot);

    assert_eq!(
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source_metadata.evidence.as_deref()
                        == Some("session_file_activity_match")
            })
            .count(),
        2
    );
}

#[test]
fn active_pane_resume_target_prefers_lineage_child_when_more_recent() {
    let parent = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "parent"),
            "codex".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(1_000),
    );
    let child = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "child"),
            "codex".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(2_000),
    );
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            parent,
            child,
            mux_with_active_command("one", Some("/work/repo"), "codex resume parent"),
        ],
        candidate_links: vec![parent_session_link("child", "parent")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    assert_eq!(
        mux_links[0].source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "child"))
    );
}

#[test]
fn active_pane_argv_match_prefers_parent_when_more_recent_than_child() {
    let parent = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "parent"),
            "codex".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(2_000),
    );
    let child = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "child"),
            "codex".to_string(),
        )
        .with_cwd("/work/repo".to_string())
        .with_last_active_epoch(1_000),
    );
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            parent,
            child,
            mux_with_active_command("one", Some("/work/repo"), "codex -s parent"),
        ],
        candidate_links: vec![parent_session_link("child", "parent")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    assert_eq!(
        mux_links[0].source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "parent"))
    );
}

#[test]
fn generic_uuid_like_session_keys_extracts_uuid_shaped_tokens() {
    let values = crate::discovery::harness::generic_uuid_like_session_keys(
        "/home/me/.codex/sessions/2026/05/19/rollout-2026-05-19T23-00-48-019e4354-26b9-7ad2-9521-4ad921cc312b.jsonl",
    );

    assert_eq!(
        values,
        BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()])
    );
}

#[test]
fn command_session_evidence_ignores_ordinary_command_tokens() {
    let evidence = command_session_evidence("opencode run --flag project-name");

    assert!(
        evidence.session_keys.is_empty(),
        "ordinary argv tokens are not session keys"
    );
    assert_eq!(evidence.harnesses, BTreeSet::from(["opencode".to_string()]));
}

#[test]
fn fd_paths_extract_codex_and_claude_session_keys() {
    let evidence = session_key_evidence_from_fd_paths([
        "/home/me/.codex/sessions/2026/05/19/rollout-2026-05-19T23-00-48-019e4354-26b9-7ad2-9521-4ad921cc312b.jsonl",
        "/home/me/.claude/tasks/e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6/.lock",
        "/home/me/.local/share/opencode/session/ses_16d204c1dffeIkBjxxbpD9uujI/state.json",
        "/home/me/.config/other/11111111-2222-3333-4444-555555555555",
    ]);

    assert_eq!(
        evidence.session_keys,
        BTreeSet::from([
            "019e4354-26b9-7ad2-9521-4ad921cc312b".to_string(),
            "e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string(),
            "ses_16d204c1dffeIkBjxxbpD9uujI".to_string(),
        ])
    );
    assert_eq!(
        evidence.harnesses,
        BTreeSet::from([
            "claude-code".to_string(),
            "codex".to_string(),
            "opencode".to_string()
        ])
    );
}

#[test]
fn active_pane_evidence_prefers_single_fd_session_over_command() {
    let mux = match mux_with_active_command(
        "one",
        Some("/work/repo"),
        "codex resume 019e3b8b-e512-7532-a1f2-7e88fcace046",
    ) {
        GraphNode::MuxSession(mut mux) => {
            mux.active_pane_pid = None;
            mux
        }
        _ => unreachable!(),
    };
    let fd = SessionKeyEvidence {
        session_keys: BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()]),
        harnesses: BTreeSet::from(["codex".to_string()]),
    };
    let command = command_session_evidence(mux.active_pane_start_command.as_deref().unwrap());
    let evidence =
        active_pane_evidence_from_sources(fd, command, active_pane_harnesses(&mux)).unwrap();

    assert_eq!(evidence.link_evidence, "active_pane_fd_session_match");
    assert_eq!(
        evidence.session_keys,
        BTreeSet::from(["019e4354-26b9-7ad2-9521-4ad921cc312b".to_string()])
    );
}

#[test]
fn active_pane_evidence_uses_fd_command_intersection() {
    let fd = SessionKeyEvidence {
        session_keys: BTreeSet::from([
            "e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string(),
            "c1901a9e-f3db-48e3-a46c-3f92c7c0f2d3".to_string(),
        ]),
        harnesses: BTreeSet::from(["claude-code".to_string()]),
    };
    let command = command_session_evidence("claude --resume e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6");
    let evidence = active_pane_evidence_from_sources(fd, command, BTreeSet::new()).unwrap();

    assert_eq!(
        evidence.link_evidence,
        "active_pane_fd_command_session_match"
    );
    assert_eq!(
        evidence.session_keys,
        BTreeSet::from(["e7a0ba3e-68a9-4ae1-bebc-c174e78de1e6".to_string()])
    );
}

#[test]
fn fork_root_match_emits_associated_with_candidate() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/work/fork-alpha/inside")),
            fork_node("atelier:alpha"),
        ],
        candidate_links: vec![rooted_at_path("atelier:alpha", "/work/fork-alpha")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .collect();
    assert_eq!(associated.len(), 1);
    let link = associated[0];
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"))
    );
    assert_eq!(
        link.target_node_id(),
        Some(&NodeId::Fork(ForkId::new("atelier:alpha")))
    );
    assert_eq!(
        link.source_metadata
            .fields
            .get("fork_root")
            .and_then(serde_json::Value::as_str),
        Some("/work/fork-alpha")
    );
}

#[test]
fn session_inside_worktree_emits_checkout_association_candidate() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/work/repo/crates/core")),
            worktree("/work/repo/.git", "/work/repo"),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .collect();
    assert_eq!(associated.len(), 1);
    let link = associated[0];
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new("codex", "/state", "a"))
    );
    assert_eq!(
        link.target_node_id(),
        Some(&NodeId::Checkout(CheckoutId::new(
            RepoId::new("/work/repo/.git"),
            "/work/repo"
        )))
    );
    assert_eq!(
        link.source_metadata
            .fields
            .get("checkout_root")
            .and_then(serde_json::Value::as_str),
        Some("/work/repo")
    );
}

#[test]
fn session_inside_workspace_member_emits_workspace_and_checkout_candidates() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/workspace/repo/crates/core")),
            workspace("/workspace"),
            repo("/workspace/repo/.git"),
            worktree("/workspace/repo/.git", "/workspace/repo"),
        ],
        candidate_links: vec![workspace_contains_repo(
            "/workspace",
            "/workspace/repo/.git",
            "/workspace/repo",
        )],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .collect();
    assert_eq!(associated.len(), 2);
    assert!(associated.iter().any(|link| {
        link.target_node_id() == Some(&NodeId::Workspace(WorkspaceId::new("/workspace")))
    }));
    assert!(associated.iter().any(|link| {
        link.target_node_id()
            == Some(&NodeId::Checkout(CheckoutId::new(
                RepoId::new("/workspace/repo/.git"),
                "/workspace/repo",
            )))
    }));
}

#[test]
fn symlinked_workspace_member_does_not_associate_session_at_canonical_path() {
    // H-WS-001 follow-up: an atelier/agent-deck workspace whose
    // member is a symlink should associate sessions running
    // *inside the workspace tree* (logical_path), not sessions
    // running at the symlink target's canonical location. The
    // canonical_checkout_root is recorded on the membership link
    // for downstream lookups but must not drive the cross-link
    // AssociatedWith Workspace inference — otherwise every
    // session in `~/src/conspectus` gets nested under every
    // workspace that lists conspectus as a member.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            // Session is at the canonical checkout, not inside
            // the workspace's composite tree.
            session("a", Some("/home/op/src/conspectus/crates/core")),
            workspace("/home/op/atelier-ws"),
            repo("/home/op/src/conspectus/.git"),
            worktree("/home/op/src/conspectus/.git", "/home/op/src/conspectus"),
        ],
        candidate_links: vec![workspace_contains_repo_paths(
            "/home/op/atelier-ws",
            "/home/op/src/conspectus/.git",
            // Workspace-visible location (the symlink itself).
            "/home/op/atelier-ws/conspectus",
            // Symlink target — the canonical checkout root.
            "/home/op/src/conspectus",
        )],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated_to_workspace: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .filter(|link| matches!(link.target_node_id(), Some(NodeId::Workspace(_))))
        .collect();
    assert!(
        associated_to_workspace.is_empty(),
        "session at canonical checkout path must not gain an AssociatedWith \
             Workspace edge through the canonical_checkout_root index entry; \
             got {associated_to_workspace:#?}",
    );

    // The session still associates with its checkout — that path
    // is unaffected by the workspace inference.
    assert!(
        snapshot.candidate_links.iter().any(|link| {
            link.relation == RelationKind::AssociatedWith
                && matches!(link.target_node_id(), Some(NodeId::Checkout(_)))
        }),
        "session-to-checkout association should still fire",
    );
}

#[test]
fn session_at_workspace_root_associates_with_workspace() {
    // H-WS-001 follow-up: agent-deck launches the harness with
    // cwd = the multi-repo-worktrees `<id>` directory itself, not
    // inside a specific member subdir. Indexing only member
    // `logical_path` would miss this — the session sits above
    // any member path, so no match would fire. The fix indexes
    // the workspace's own root alongside member paths so a
    // session at the workspace root attributes to the workspace.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/home/op/.agent-deck/multi-repo-worktrees/abc")),
            workspace("/home/op/.agent-deck/multi-repo-worktrees/abc"),
            repo("/home/op/src/conspectus/.git"),
        ],
        candidate_links: vec![workspace_contains_repo_paths(
            "/home/op/.agent-deck/multi-repo-worktrees/abc",
            "/home/op/src/conspectus/.git",
            "/home/op/.agent-deck/multi-repo-worktrees/abc/conspectus",
            "/home/op/src/conspectus",
        )],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated_to_workspace: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .filter(|link| matches!(link.target_node_id(), Some(NodeId::Workspace(_))))
        .collect();
    assert_eq!(
        associated_to_workspace.len(),
        1,
        "session at workspace root should associate with the workspace: got {associated_to_workspace:#?}",
    );
}

#[test]
fn session_in_member_subdir_still_picks_deepest_member_path() {
    // The workspace-root index must not regress the existing
    // "deepest match wins" behavior — a session inside a specific
    // member subdir attributes via that member, not via the
    // workspace root.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session(
                "a",
                Some("/home/op/.agent-deck/multi-repo-worktrees/abc/conspectus/src"),
            ),
            workspace("/home/op/.agent-deck/multi-repo-worktrees/abc"),
            repo("/home/op/src/conspectus/.git"),
        ],
        candidate_links: vec![workspace_contains_repo_paths(
            "/home/op/.agent-deck/multi-repo-worktrees/abc",
            "/home/op/src/conspectus/.git",
            "/home/op/.agent-deck/multi-repo-worktrees/abc/conspectus",
            "/home/op/src/conspectus",
        )],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated_to_workspace: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .filter(|link| matches!(link.target_node_id(), Some(NodeId::Workspace(_))))
        .collect();
    assert_eq!(associated_to_workspace.len(), 1);
    let member_root = associated_to_workspace[0]
        .source_metadata
        .fields
        .get("workspace_member_root")
        .and_then(|v| v.as_str());
    assert_eq!(
        member_root,
        Some("/home/op/.agent-deck/multi-repo-worktrees/abc/conspectus"),
        "deepest-match should prefer the member's logical_path over the workspace root",
    );
}

#[test]
fn nested_worktree_association_chooses_deepest_checkout() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("a", Some("/work/repo/nested/src")),
            worktree("/work/repo/.git", "/work/repo"),
            worktree("/work/repo/nested/.git", "/work/repo/nested"),
        ],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let associated: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::AssociatedWith)
        .collect();
    assert_eq!(associated.len(), 1);
    assert_eq!(
        associated[0].target_node_id(),
        Some(&NodeId::Checkout(CheckoutId::new(
            RepoId::new("/work/repo/nested/.git"),
            "/work/repo/nested"
        )))
    );
}

#[test]
fn session_cwd_outside_fork_root_does_not_associate() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("a", Some("/elsewhere")), fork_node("atelier:alpha")],
        candidate_links: vec![rooted_at_path("atelier:alpha", "/work/fork-alpha")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    assert!(
        snapshot
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::AssociatedWith)
    );
}

#[test]
fn unresolved_lineage_links_are_preserved() {
    let lineage = GraphLink {
        id: "atelier:lineage".to_string(),
        source: NodeId::Fork(ForkId::new("atelier:alpha")),
        target: LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some("codex".to_string()),
                native_id: Some("missing".to_string()),
                state_scope: None,
                path: Some("/work/fork-alpha".to_string()),
                metadata: Default::default(),
            },
        },
        relation: RelationKind::ChildSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "atelier".to_string(),
            evidence: Some("test lineage".to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    };
    let mut snapshot = GraphSnapshot {
        nodes: vec![fork_node("atelier:alpha")],
        candidate_links: vec![lineage.clone()],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    // `infer` now stamps any link missing `source_metadata.freshness_epoch`
    // (P7-002), so equality on the raw `lineage` fixture is no longer the
    // right invariant; check existence by id instead.
    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|link| link.id == lineage.id)
    );
}

// --- Subagent mux suppression tests ---

fn opencode_session(id: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("opencode", "/state", id),
        harness_key: "opencode".to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn subagent_session(id: &str, cwd: Option<&str>, parent_id: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("opencode", "/state", id),
        harness_key: "opencode".to_string(),
        cwd: cwd.map(str::to_string),
        title: Some(format!("(@general subagent) task from {parent_id}")),
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: Some(SessionKind::Subagent),
    })
}

fn subagent_parent_link(child_key: &str, parent_key: &str) -> GraphLink {
    let child = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", child_key));
    let parent = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", parent_key));
    GraphLink {
        id: format!("opencode:lineage:{child_key}:parent_session:{parent_key}"),
        source: child,
        target: LinkEndpoint::Node { id: parent },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "opencode".to_string(),
            evidence: Some("opencode session.parent_id unknown".to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

#[test]
fn subagent_mux_link_overridden_when_parent_matches_same_mux() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            opencode_session("parent", Some("/work/repo")),
            subagent_session("child", Some("/work/repo"), "parent"),
            mux("one", Some("/work/repo")),
        ],
        candidate_links: vec![subagent_parent_link("child", "parent")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let child_mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source
                    == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "child"))
        })
        .collect();

    assert_eq!(child_mux_links.len(), 1);
    assert!(
        matches!(child_mux_links[0].state, LinkState::Overridden { .. }),
        "subagent mux link should be overridden when parent matches same mux"
    );
}

#[test]
fn subagent_mux_link_stays_active_when_parent_does_not_match_mux() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            opencode_session("parent", Some("/other")),
            subagent_session("child", Some("/work/repo"), "parent"),
            mux("one", Some("/work/repo")),
        ],
        // ParentSession link exists, but parent has no LinkedToMux — so
        // the subagent's link should remain active.
        candidate_links: vec![subagent_parent_link("child", "parent")],
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let child_mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source
                    == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "child"))
        })
        .collect();

    assert_eq!(child_mux_links.len(), 1);
    assert!(
        matches!(child_mux_links[0].state, LinkState::Active),
        "subagent mux link should stay active when parent doesn't match the same mux"
    );
}

#[test]
fn orphan_subagent_mux_link_stays_active() {
    // Subagent with no discovered parent should have normal mux linking.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            subagent_session("orphan", Some("/work/repo"), "missing-parent"),
            mux("one", Some("/work/repo")),
        ],
        // No ParentSession link (parent not discovered).
        ..GraphSnapshot::empty()
    };

    infer(&mut snapshot);

    let orphan_mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source
                    == NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "orphan"))
        })
        .collect();

    assert_eq!(orphan_mux_links.len(), 1);
    assert!(
        matches!(orphan_mux_links[0].state, LinkState::Active),
        "orphan subagent mux link should stay active"
    );
}
