use super::*;
use crate::model::{
    AgentSessionNode, CheckoutRecipe, HostedSession, MuxIncarnation, MuxProbe, MuxProbeStatus,
    MuxRecipe, RecordDisplay, RecordMeta, RecordedRemote, RepoRecipe,
};

const NOW: i64 = 5_000;

fn meta(id: &str, last_seen: i64) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: String::new(),
        first_seen: 100,
        last_seen,
        origin: Default::default(),
        display: RecordDisplay::default(),
        rebuilt_from: None,
    }
}

fn mux_record(id: &str, name: &str, session_id: &str, hosted: &[&str]) -> MuxRecord {
    MuxRecord {
        meta: meta(id, 4_000),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some("/src/app".into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some(session_id.into()),
                created: Some(1_000),
            }),
            argv: Some(vec!["claude".into(), "--flag".into()]),
            hosted: hosted
                .iter()
                .enumerate()
                .map(|(i, id)| HostedSession {
                    harness: "claude-code".into(),
                    session_id: (*id).into(),
                    first_seen: 2_000,
                    last_seen: 3_000 - i as i64,
                })
                .collect(),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn probe(status: MuxProbeStatus) -> MuxProbe {
    MuxProbe {
        status,
        epoch: NOW,
        exited: Vec::new(),
    }
}

fn session(key: &str, active: Option<i64>) -> GraphNode {
    let mut node = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "global", key),
        "claude-code",
    );
    node.last_active_epoch = active;
    GraphNode::AgentSession(node)
}

fn live_mux(name: &str, session_id: &str, created: i64) -> GraphNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    mux.backend_session_id = Some(session_id.into());
    mux.created_epoch = Some(created);
    GraphNode::MuxSession(mux)
}

fn history(muxes: Vec<MuxRecord>) -> HistorySnapshot {
    HistorySnapshot {
        muxes,
        ..Default::default()
    }
}

fn ghost<'a>(snapshot: &'a GraphSnapshot, name: &str) -> &'a MuxSessionNode {
    snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::MuxSession(mux) if mux.native_id == name => Some(mux),
            _ => None,
        })
        .expect("ghost node")
}

fn links(snapshot: &GraphSnapshot, relation: RelationKind) -> Vec<&GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == relation)
        .collect()
}

#[test]
fn a_mux_a_successful_probe_left_out_is_a_gone_node_linked_to_its_last_session() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("s-last", Some(3_000)),
            session("s-older", Some(2_000)),
        ],
        mux_probes: BTreeMap::from([("tmux".into(), probe(MuxProbeStatus::NoServer))]),
        ..GraphSnapshot::empty()
    };
    let mut history = history(vec![mux_record(
        "h_1",
        "main",
        "$1",
        &["s-last", "s-older"],
    )]);
    project_into(&mut snapshot, &mut history, NOW);

    let node = ghost(&snapshot, "main");
    assert_eq!(node.created_epoch, Some(1_000));
    assert_eq!(node.backend_session_id.as_deref(), Some("$1"));
    let id = NodeId::MuxSession(node.id.clone());
    assert_eq!(
        snapshot.node_presence.get(&id),
        Some(&NodePresence::Gone { last_seen: 4_000 })
    );
    assert_eq!(
        history.presence_of("h_1"),
        Some(NodePresence::Gone { last_seen: 4_000 })
    );
    assert_eq!(snapshot.node_provenance[&id].provider, HISTORY_ADAPTER);

    let last = links(&snapshot, RelationKind::MuxLastHosted);
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].source, id);
    assert_eq!(
        last[0].target,
        LinkEndpoint::Node {
            id: NodeId::AgentSession(AgentSessionId::new("claude-code", "global", "s-last"))
        }
    );
    assert!(
        links(&snapshot, RelationKind::LinkedToMux).is_empty(),
        "a ghost has no live-only relations"
    );
}

#[test]
fn a_failed_missing_or_unprobed_server_leaves_its_muxes_unknown() {
    for probes in [
        BTreeMap::from([(
            "tmux".to_string(),
            probe(MuxProbeStatus::Failed {
                message: "boom".into(),
            }),
        )]),
        BTreeMap::from([("tmux".to_string(), probe(MuxProbeStatus::Unavailable))]),
        BTreeMap::new(),
    ] {
        let mut snapshot = GraphSnapshot {
            mux_probes: probes,
            ..GraphSnapshot::empty()
        };
        let mut history = history(vec![mux_record("h_1", "main", "$1", &[])]);
        project_into(&mut snapshot, &mut history, NOW);
        assert_eq!(
            history.presence_of("h_1"),
            Some(NodePresence::Unknown { last_seen: 4_000 })
        );
    }
}

#[test]
fn a_renamed_live_mux_matches_its_record_by_incarnation_and_holds_its_last_session() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            live_mux("renamed", "$1", 1_000),
            session("s-last", Some(3_000)),
        ],
        mux_probes: BTreeMap::from([("tmux".into(), probe(MuxProbeStatus::Listed))]),
        ..GraphSnapshot::empty()
    };
    let mut history = history(vec![mux_record("h_1", "main", "$1", &["s-last"])]);
    project_into(&mut snapshot, &mut history, NOW);

    assert!(
        history.presence.is_empty(),
        "live records have no presence entry"
    );
    assert!(snapshot.node_presence.is_empty());
    assert_eq!(snapshot.nodes.len(), 2, "no ghost for a renamed mux");
    let fallback = links(&snapshot, RelationKind::LinkedToMux);
    assert_eq!(fallback.len(), 1);
    assert_eq!(fallback[0].provenance, Provenance::Cached);
    assert_eq!(fallback[0].freshness, Freshness::Stale);
    assert_eq!(
        fallback[0].target,
        LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new("tmux:renamed"))
        }
    );
}

#[test]
fn a_recreated_mux_does_not_inherit_a_session_that_went_quiet_before_it() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            live_mux("main", "$9", 3_500),
            session("s-last", Some(3_000)),
        ],
        mux_probes: BTreeMap::from([("tmux".into(), probe(MuxProbeStatus::Listed))]),
        ..GraphSnapshot::empty()
    };
    let mut history = history(vec![mux_record("h_1", "main", "$1", &["s-last"])]);
    project_into(&mut snapshot, &mut history, NOW);
    assert!(history.presence.is_empty(), "matched by name, so live");
    assert!(links(&snapshot, RelationKind::LinkedToMux).is_empty());
}

#[test]
fn the_fallback_holds_an_attribution_only_until_current_evidence_disagrees() {
    let base = GraphSnapshot {
        nodes: vec![
            live_mux("main", "$1", 1_000),
            live_mux("other", "$2", 1_000),
            session("s-last", Some(3_000)),
        ],
        mux_probes: BTreeMap::from([("tmux".into(), probe(MuxProbeStatus::Listed))]),
        ..GraphSnapshot::empty()
    };
    let session_id = NodeId::AgentSession(AgentSessionId::new("claude-code", "global", "s-last"));
    let resolved_target = |snapshot: &GraphSnapshot| {
        snapshot
            .resolved_relationships
            .iter()
            .find(|rel| rel.relation == RelationKind::LinkedToMux && rel.source == session_id)
            .filter(|rel| rel.selected_link_id.is_some())
            .map(|rel| rel.target.clone())
    };

    let mut quiet = base.clone();
    let mut records = history(vec![mux_record("h_1", "main", "$1", &["s-last"])]);
    project_into(&mut quiet, &mut records, NOW);
    let quiet = crate::resolve::resolve_snapshot(quiet);
    assert_eq!(
        resolved_target(&quiet),
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:main")))
    );

    let mut drifted = base;
    let mut hook = GraphLink::new(
        "hook:s-last",
        session_id.clone(),
        LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new("tmux:other")),
        },
        RelationKind::LinkedToMux,
        Provenance::StrongDiscovered,
    );
    hook.source_metadata.adapter = "hook_sidecar".into();
    drifted.candidate_links.push(hook);
    let mut records = history(vec![mux_record("h_1", "main", "$1", &["s-last"])]);
    project_into(&mut drifted, &mut records, NOW);
    let drifted = crate::resolve::resolve_snapshot(drifted);
    assert_eq!(
        resolved_target(&drifted),
        Some(NodeId::MuxSession(MuxSessionId::new("tmux:other")))
    );
}

fn checkout_record(id: &str, root: &Path, common_dir: &str) -> CheckoutRecord {
    CheckoutRecord {
        meta: meta(id, 4_000),
        recipe: CheckoutRecipe {
            repo: format!("repo:{common_dir}"),
            root: root.display().to_string(),
            linked: true,
            branch: Some("feature/x".into()),
            head: Some("abc123".into()),
            upstream: None,
            extra: Default::default(),
        },
        extra: Default::default(),
    }
}

#[test]
fn a_removed_worktree_is_gone_with_its_branch_and_head_and_files_under_its_repo() {
    let temp = tempfile::tempdir().unwrap();
    let common_dir = temp.path().join("app/.git");
    std::fs::create_dir_all(&common_dir).unwrap();
    let repo_id = RepoId::new(common_dir.display().to_string());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Repo(RepoNode::new(repo_id.clone()))],
        ..GraphSnapshot::empty()
    };
    let removed = temp.path().join("wt-removed");
    let mut history = HistorySnapshot {
        checkouts: vec![checkout_record(
            "h_c",
            &removed,
            &common_dir.display().to_string(),
        )],
        ..Default::default()
    };
    project_into(&mut snapshot, &mut history, NOW);

    let checkout = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::Checkout(checkout) => Some(checkout),
            _ => None,
        })
        .expect("gone checkout node");
    assert_eq!(checkout.head_commit.as_deref(), Some("abc123"));
    assert_eq!(
        checkout.current_branch.as_ref().unwrap().refname,
        "refs/heads/feature/x"
    );
    assert_eq!(
        history.presence_of("h_c"),
        Some(NodePresence::Gone { last_seen: 4_000 })
    );
    let belongs = links(&snapshot, RelationKind::BelongsToRepo);
    assert_eq!(belongs.len(), 1);
    assert_eq!(
        belongs[0].target,
        LinkEndpoint::Node {
            id: NodeId::Repo(repo_id)
        }
    );
}

#[test]
fn a_path_under_a_missing_parent_is_unknown_and_an_existing_one_is_live() {
    let temp = tempfile::tempdir().unwrap();
    let unmounted = temp.path().join("mnt/drive/wt");
    let present = temp.path().join("present");
    std::fs::create_dir_all(&present).unwrap();
    let mut snapshot = GraphSnapshot::empty();
    let mut history = HistorySnapshot {
        checkouts: vec![
            checkout_record("h_unmounted", &unmounted, "/r/.git"),
            checkout_record("h_present", &present, "/r/.git"),
        ],
        ..Default::default()
    };
    project_into(&mut snapshot, &mut history, NOW);
    assert_eq!(
        history.presence_of("h_unmounted"),
        Some(NodePresence::Unknown { last_seen: 4_000 })
    );
    assert_eq!(
        history.presence_of("h_present"),
        None,
        "a scan that didn't reach it never makes it gone"
    );
    assert_eq!(
        snapshot.nodes.len(),
        1,
        "only the unknown one becomes a node"
    );
}

#[test]
fn a_deleted_repo_becomes_a_gone_repo_node_with_its_remotes() {
    let temp = tempfile::tempdir().unwrap();
    let common_dir = temp.path().join("old/.git");
    std::fs::create_dir_all(temp.path().join("old")).unwrap();
    let mut snapshot = GraphSnapshot::empty();
    let mut history = HistorySnapshot {
        repos: vec![RepoRecord {
            meta: meta("h_r", 4_000),
            recipe: RepoRecipe {
                common_dir: common_dir.display().to_string(),
                remotes: vec![RecordedRemote {
                    name: "origin".into(),
                    url: "git@github.com:me/old.git".into(),
                }],
                primary_root: Some(temp.path().join("old").display().to_string()),
                default_branch: None,
                extra: Default::default(),
            },
            extra: Default::default(),
        }],
        ..Default::default()
    };
    project_into(&mut snapshot, &mut history, NOW);
    let GraphNode::Repo(repo) = &snapshot.nodes[0] else {
        panic!("repo node")
    };
    assert_eq!(
        repo.remotes,
        vec!["origin=git@github.com:me/old.git".to_string()]
    );
    assert!(snapshot.node_presence.values().all(|p| p.is_gone()));
}

#[test]
fn a_rebuilt_checkout_links_to_the_one_it_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let new_root = temp.path().join("new");
    std::fs::create_dir_all(&new_root).unwrap();
    let old_root = temp.path().join("old");
    let new_id = CheckoutId::new(RepoId::new("/r/.git"), new_root.display().to_string());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Checkout(CheckoutNode::new(
            new_id.clone(),
            new_root.display().to_string(),
        ))],
        ..GraphSnapshot::empty()
    };
    let mut rebuilt = checkout_record("h_new", &new_root, "/r/.git");
    rebuilt.meta.rebuilt_from = Some("h_old".into());
    let mut history = HistorySnapshot {
        checkouts: vec![checkout_record("h_old", &old_root, "/r/.git"), rebuilt],
        ..Default::default()
    };
    project_into(&mut snapshot, &mut history, NOW);
    let rebuilt = links(&snapshot, RelationKind::RebuiltFrom);
    assert_eq!(rebuilt.len(), 1);
    assert_eq!(rebuilt[0].source, NodeId::Checkout(new_id));
}

#[test]
fn an_exited_zellij_session_is_gone_in_place() {
    let node = MuxSessionNode::new(MuxSessionId::new("zellij:old"), "zellij", "old");
    let id = NodeId::MuxSession(node.id.clone());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(node)],
        mux_probes: BTreeMap::from([(
            "zellij".into(),
            MuxProbe {
                status: MuxProbeStatus::Listed,
                epoch: NOW,
                exited: vec!["old".into()],
            },
        )]),
        ..GraphSnapshot::empty()
    };
    let mut record = mux_record("h_z", "old", "", &[]);
    record.recipe.backend = "zellij".into();
    record.recipe.incarnation = None;
    let mut history = history(vec![record]);
    project_into(&mut snapshot, &mut history, NOW);
    assert_eq!(snapshot.nodes.len(), 1, "no duplicate node");
    assert!(snapshot.node_presence[&id].is_gone());
    assert!(history.presence_of("h_z").unwrap().is_gone());
}

#[test]
fn read_problems_become_config_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("muxes.json"), "not json").unwrap();
    let load = crate::history::HistoryStore::at(temp.path()).load();
    let mut snapshot = GraphSnapshot::empty();
    project(&mut snapshot, load, NOW);
    assert!(matches!(
        snapshot.diagnostics.as_slice(),
        [Diagnostic::Config { .. }]
    ));
}
