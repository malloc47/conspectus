use std::collections::BTreeMap;

use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, ForgePrId, ForgePrNode,
    GraphLink, LinkEndpoint, MuxProbe, MuxSessionId, NodePresence, NodeProvenance, Provenance,
    RepoId, RepoNode, ResolvedRelationship, WorktreeMeta,
};

const NOW: i64 = 1_000;

fn mux(name: &str) -> MuxSessionNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    mux.cwd = Some("/home/me/src/app".into());
    mux.created_epoch = Some(900);
    mux.backend_session_id = Some("$3".into());
    mux
}

fn listed(epoch: i64) -> BTreeMap<String, MuxProbe> {
    BTreeMap::from([(
        "tmux".to_string(),
        MuxProbe {
            status: MuxProbeStatus::Listed,
            epoch,
            exited: Vec::new(),
        },
    )])
}

fn snapshot_with(nodes: Vec<GraphNode>, probe_epoch: i64) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        mux_probes: listed(probe_epoch),
        ..GraphSnapshot::empty()
    }
}

fn observe_with(snapshot: &GraphSnapshot, config: &HistoryConfig) -> Observations {
    observe(
        snapshot,
        &RecordContext {
            fresh_since: NOW,
            config,
            home: Some(Path::new("/home/me")),
        },
    )
}

#[test]
fn a_listed_mux_is_observed_with_its_incarnation_and_start_command() {
    let mut node = mux("main");
    node.active_pane_start_command = Some(r#"claude "--model" opus"#.into());
    let snapshot = snapshot_with(vec![GraphNode::MuxSession(node)], NOW + 1);
    let observed = observe_with(&snapshot, &HistoryConfig::default());

    let [obs] = observed.muxes.as_slice() else {
        panic!("one mux: {observed:?}");
    };
    assert_eq!(obs.seen_at, NOW + 1, "seen when the probe ran");
    assert_eq!(obs.key, "mux_session:tmux:main");
    let recipe = &obs.recipe;
    assert_eq!(recipe.name, "main");
    assert_eq!(
        recipe.incarnation,
        Some(MuxIncarnation {
            session_id: Some("$3".into()),
            created: Some(900),
        })
    );
    assert_eq!(
        recipe.argv.as_deref(),
        Some(&["claude".to_string(), "--model".into(), "opus".into()][..])
    );
    assert_eq!(recipe.argv_source, Some(ArgvSource::Observed));
    assert_eq!(recipe.harness.as_deref(), Some("claude-code"));
    assert_eq!(
        obs.display.context.as_deref(),
        Some("claude-code · ~/src/app")
    );
}

#[test]
fn a_probe_older_than_this_cycle_observes_nothing() {
    let snapshot = snapshot_with(vec![GraphNode::MuxSession(mux("main"))], NOW - 1);
    assert!(observe_with(&snapshot, &HistoryConfig::default()).is_empty());
}

#[test]
fn a_failed_or_empty_probe_observes_nothing() {
    for status in [
        MuxProbeStatus::Failed {
            message: "boom".into(),
        },
        MuxProbeStatus::NoServer,
        MuxProbeStatus::Unavailable,
    ] {
        let mut snapshot = snapshot_with(vec![GraphNode::MuxSession(mux("main"))], NOW);
        snapshot.mux_probes.get_mut("tmux").unwrap().status = status;
        assert!(observe_with(&snapshot, &HistoryConfig::default()).is_empty());
    }
}

#[test]
fn gone_nodes_are_never_observed() {
    let node = mux("ghost");
    let mut snapshot = snapshot_with(vec![GraphNode::MuxSession(node.clone())], NOW);
    snapshot.node_presence.insert(
        NodeId::MuxSession(node.id),
        NodePresence::Gone { last_seen: 10 },
    );
    assert!(observe_with(&snapshot, &HistoryConfig::default()).is_empty());
}

#[test]
fn an_exited_zellij_session_is_not_observed() {
    let mut node = MuxSessionNode::new(MuxSessionId::new("zellij:old"), "zellij", "old");
    node.client_attached = Some(false);
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(node)],
        ..GraphSnapshot::empty()
    };
    snapshot.mux_probes.insert(
        "zellij".into(),
        MuxProbe {
            status: MuxProbeStatus::Listed,
            epoch: NOW,
            exited: vec!["old".into()],
        },
    );
    assert!(observe_with(&snapshot, &HistoryConfig::default()).is_empty());
}

fn session(key: &str) -> AgentSessionNode {
    AgentSessionNode::new(
        AgentSessionId::new("claude-code", "global", key),
        "claude-code",
    )
}

fn linked(session: &str, mux: &str, adapter: &str) -> (GraphLink, ResolvedRelationship) {
    let source = NodeId::AgentSession(AgentSessionId::new("claude-code", "global", session));
    let target = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{mux}")));
    let mut link = GraphLink::new(
        format!("{adapter}:{session}"),
        source.clone(),
        LinkEndpoint::Node { id: target.clone() },
        RelationKind::LinkedToMux,
        Provenance::Discovered,
    );
    link.source_metadata.adapter = adapter.to_string();
    let rel = ResolvedRelationship {
        source,
        target,
        relation: RelationKind::LinkedToMux,
        selected_link_id: Some(link.id.clone()),
        competing_link_ids: Vec::new(),
        corroborating_link_ids: Vec::new(),
        explanation: Default::default(),
    };
    (link, rel)
}

#[test]
fn hosted_sessions_come_from_resolved_evidence_not_from_history() {
    let mut snapshot = snapshot_with(
        vec![
            GraphNode::MuxSession(mux("main")),
            GraphNode::AgentSession(session("hooked")),
            GraphNode::AgentSession(session("echo")),
        ],
        NOW,
    );
    for (link, rel) in [
        linked("hooked", "main", "hook_sidecar"),
        linked("echo", "main", HISTORY_ADAPTER),
    ] {
        snapshot.candidate_links.push(link);
        snapshot.resolved_relationships.push(rel);
    }
    let observed = observe_with(&snapshot, &HistoryConfig::default());
    let hosted = &observed.muxes[0].recipe.hosted;
    assert_eq!(hosted.len(), 1, "history's own fallback isn't evidence");
    assert_eq!(hosted[0].session_id, "hooked");
    assert_eq!(hosted[0].last_seen, NOW);
    assert_eq!(
        observed.muxes[0].recipe.harness.as_deref(),
        Some("claude-code")
    );
}

#[test]
fn ignored_and_orchestrator_owned_muxes() {
    let mut config = HistoryConfig::default();
    config
        .ignore
        .insert(HistoryKind::Mux, vec!["scratch*".into()]);
    let snapshot = snapshot_with(
        vec![
            GraphNode::MuxSession(mux("scratch-1")),
            GraphNode::MuxSession(mux("agentdeck_repo_abc")),
        ],
        NOW,
    );
    let observed = observe_with(&snapshot, &config);
    let [obs] = observed.muxes.as_slice() else {
        panic!("only the deck mux: {observed:?}");
    };
    assert_eq!(obs.recipe.name, "agentdeck_repo_abc");
    assert_eq!(obs.recipe.orchestrator.as_deref(), Some("agent-deck"));
}

fn provenance(provider: &str, epoch: i64) -> NodeProvenance {
    NodeProvenance {
        provider: provider.into(),
        freshness_epoch: Some(epoch),
    }
}

fn worktree(root: &str, branch: &str, head: &str) -> (CheckoutNode, NodeId) {
    let repo = RepoId::new("/home/me/src/app/.git");
    let id = CheckoutId::new(repo.clone(), root);
    let mut node = CheckoutNode::new(id.clone(), root).with_worktree(WorktreeMeta::linked());
    node.current_branch = Some(BranchId::new(repo, format!("refs/heads/{branch}")));
    node.head_commit = Some(head.into());
    (node, NodeId::Checkout(id))
}

#[test]
fn checkouts_and_repos_freshly_probed_by_git_are_observed() {
    let (checkout, checkout_id) = worktree("/home/me/wt/app.feature", "feature/x", "abc123");
    let (stale, stale_id) = worktree("/home/me/wt/app.stale", "stale", "def");
    let mut repo = RepoNode::new(RepoId::new("/home/me/src/app/.git"));
    repo.remotes = vec!["origin=git@github.com:me/app.git".into()];
    let primary = CheckoutNode::new(
        CheckoutId::new(repo.id.clone(), "/home/me/src/app"),
        "/home/me/src/app",
    )
    .with_worktree(WorktreeMeta::primary());
    let repo_id = NodeId::Repo(repo.id.clone());
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Checkout(checkout),
            GraphNode::Checkout(stale),
            GraphNode::Checkout(primary),
            GraphNode::Repo(repo),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot
        .node_provenance
        .insert(checkout_id, provenance("git", NOW));
    snapshot
        .node_provenance
        .insert(stale_id, provenance("git", NOW - 100));
    snapshot
        .node_provenance
        .insert(repo_id, provenance("git::cwd", NOW + 2));

    let observed = observe_with(&snapshot, &HistoryConfig::default());
    let [checkout] = observed.checkouts.as_slice() else {
        panic!("only the fresh checkout: {observed:?}");
    };
    assert_eq!(checkout.recipe.root, "/home/me/wt/app.feature");
    assert_eq!(checkout.recipe.repo, "repo:/home/me/src/app/.git");
    assert!(checkout.recipe.linked);
    assert_eq!(checkout.recipe.branch.as_deref(), Some("feature/x"));
    assert_eq!(checkout.recipe.head.as_deref(), Some("abc123"));
    assert_eq!(checkout.display.label, "app.feature");
    assert_eq!(checkout.display.context.as_deref(), Some("app · feature/x"));

    let [repo] = observed.repos.as_slice() else {
        panic!("one repo: {observed:?}");
    };
    assert_eq!(repo.seen_at, NOW + 2);
    assert_eq!(repo.recipe.remotes[0].name, "origin");
    assert_eq!(repo.recipe.remotes[0].url, "git@github.com:me/app.git");
    assert_eq!(
        repo.recipe.primary_root.as_deref(),
        Some("/home/me/src/app")
    );
    assert_eq!(repo.display.label, "app");
}

#[test]
fn ignored_checkouts_are_not_observed() {
    let (checkout, id) = worktree("/tmp/scratch/app", "x", "a");
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Checkout(checkout)],
        ..GraphSnapshot::empty()
    };
    snapshot.node_provenance.insert(id, provenance("git", NOW));
    let mut config = HistoryConfig::default();
    config
        .ignore
        .insert(HistoryKind::Checkout, vec!["/tmp/*".into()]);
    assert!(observe_with(&snapshot, &config).is_empty());
}

#[test]
fn a_freshly_listed_pr_is_observed_with_its_snapshot_fields() {
    let id = ForgePrId::new("github", "github.com", "me", "app", 7);
    let pr = ForgePrNode {
        id: id.clone(),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "me".into(),
        repo: "app".into(),
        number: 7,
        state: Some("open".into()),
        title: Some("Add history".into()),
        url: Some("https://github.com/me/app/pull/7".into()),
        updated_epoch: None,
        is_draft: true,
        head_ref: Some("feature/x".into()),
    };
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::ForgePr(pr)],
        ..GraphSnapshot::empty()
    };
    snapshot
        .node_provenance
        .insert(NodeId::ForgePr(id.clone()), provenance("github", NOW));
    let observed = observe_with(&snapshot, &HistoryConfig::default());
    let [pr] = observed.prs.as_slice() else {
        panic!("one pr: {observed:?}");
    };
    assert_eq!(pr.recipe.number, 7);
    assert!(pr.recipe.draft);
    assert_eq!(pr.recipe.title.as_deref(), Some("Add history"));
    assert_eq!(pr.display.label, "#7");
    assert_eq!(pr.display.context.as_deref(), Some("me/app · Add history"));

    // A PR history projected back into the graph is not a listing.
    snapshot
        .node_provenance
        .insert(NodeId::ForgePr(id), provenance(HISTORY_ADAPTER, NOW));
    assert!(
        observe_with(&snapshot, &HistoryConfig::default())
            .prs
            .is_empty()
    );
}

#[test]
fn a_launched_mux_is_recorded_with_its_argv_before_any_probe() {
    let dir = tempfile::tempdir().unwrap();
    let writer = crate::history::HistoryWriter {
        store: crate::history::HistoryStore::at(dir.path()),
        config: HistoryConfig::default(),
        home: None,
    };
    let argv = vec![
        "claude".to_string(),
        "--dangerously-skip-permissions".into(),
    ];
    let id = writer
        .record_launched_mux(&crate::history::LaunchedMux {
            backend: "tmux",
            socket: Some("work"),
            name: "main",
            cwd: "/src/app",
            argv: &argv,
            harness: Some("claude-code"),
        })
        .unwrap()
        .expect("recorded");
    let load = writer.store.load();
    let record = load.history.mux(&id).unwrap();
    assert_eq!(record.meta.origin, RecordOrigin::Launched);
    assert_eq!(record.meta.key, "mux_session:tmux:work:main");
    assert_eq!(record.recipe.socket.as_deref(), Some("work"));
    assert_eq!(record.recipe.argv.as_ref(), Some(&argv));
    assert_eq!(record.recipe.argv_source, Some(ArgvSource::Conspectus));

    // The first probe that lists it lands on the same record.
    let mut node = mux("main");
    node.id = MuxSessionId::new("tmux:work:main");
    node.native_id = "work:main".into();
    node.socket = Some("work".into());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(node)],
        ..GraphSnapshot::empty()
    };
    snapshot.mux_probes.insert(
        "tmux:work".into(),
        MuxProbe {
            status: MuxProbeStatus::Listed,
            epoch: crate::discovery::current_epoch(),
            exited: Vec::new(),
        },
    );
    writer.record(&mut snapshot, 0).unwrap();
    assert_eq!(snapshot.history.muxes.len(), 1);
    let record = &snapshot.history.muxes[0];
    assert_eq!(record.meta.id, id);
    assert_eq!(record.recipe.argv_source, Some(ArgvSource::Conspectus));
    assert!(record.recipe.incarnation.as_ref().unwrap().is_complete());
}
