use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, ForgePrId, ForgePrNode,
    ForkId, ForkNode, MuxSessionId, MuxSessionNode, NodePresence, PinKind, PinNode, RepoId,
    RepoNode, RuntimeProcessId, SlotStatus, WorkspaceId, WorkspaceNode,
};

fn session(key: &str, title: &str, epoch: i64) -> GraphNode {
    let mut node = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "global", key),
        "claude-code",
    );
    node.title = Some(title.into());
    node.cwd = Some("/src/app".into());
    node.last_message_preview = Some("refactor the zebra parser".into());
    node.last_active_epoch = Some(epoch);
    GraphNode::AgentSession(node)
}

fn mux(name: &str) -> GraphNode {
    let mut node = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    node.cwd = Some("/src/app".into());
    node.active_pane_command = Some("claude".into());
    GraphNode::MuxSession(node)
}

fn repo() -> GraphNode {
    let mut node = RepoNode::new(RepoId::new("/src/app/.git"));
    node.remotes = vec!["git@github.com:acme/app.git".into()];
    GraphNode::Repo(node)
}

fn checkout() -> GraphNode {
    let repo = RepoId::new("/src/app/.git");
    let mut node = CheckoutNode::new(CheckoutId::new(repo.clone(), "/src/app-wt"), "/src/app-wt");
    node.current_branch = Some(BranchId::new(repo, "refs/heads/feature/login"));
    GraphNode::Checkout(node)
}

fn pr() -> GraphNode {
    GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", "acme", "app", 42),
        provider: "github".into(),
        host: "github.com".into(),
        owner: "acme".into(),
        repo: "app".into(),
        number: 42,
        state: Some("open".into()),
        title: Some("Add login form".into()),
        url: None,
        updated_epoch: Some(500),
        is_draft: false,
        head_ref: Some("feature/login".into()),
    })
}

fn pin() -> GraphNode {
    GraphNode::Pin(PinNode::for_test(
        "daily",
        PinKind::Mux,
        None,
        SlotStatus::Planned,
        &[],
    ))
}

fn workspace() -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/src/ws"),
        root: "/src/ws".into(),
        provider: Some("jj".into()),
        name: Some("monorepo".into()),
    })
}

fn fork() -> GraphNode {
    GraphNode::Fork(ForkNode {
        id: ForkId::new("agent-deck:one"),
        provider: "agent-deck".into(),
        provider_source_key: "one".into(),
        name: Some("sidequest".into()),
        scope: None,
        capabilities: Vec::new(),
    })
}

fn index(nodes: Vec<GraphNode>, live_preview: bool) -> EntityIndex {
    let snapshot = GraphSnapshot {
        nodes,
        ..GraphSnapshot::empty()
    };
    EntityIndex::build(&snapshot, live_preview)
}

fn top(index: &EntityIndex, query: &str) -> Option<String> {
    index
        .rank(query)
        .first()
        .map(|hit| index.entities()[hit.index].label.clone())
}

#[test]
fn every_kind_is_indexed_and_runtime_processes_and_branches_are_not() {
    let process = GraphNode::RuntimeProcess(crate::model::RuntimeProcessNode {
        observation_key: "p".into(),
        id: RuntimeProcessId::new("p"),
        pid: Some(1),
        parent_pid: None,
        root_pane_pid: None,
        command: Some("claude".into()),
        cwd: None,
        harness_key: None,
        role: None,
        depth: None,
        observed_epoch: None,
    });
    let branch = GraphNode::Branch(crate::model::BranchNode {
        id: BranchId::new(RepoId::new("/src/app/.git"), "refs/heads/main"),
        refname: "refs/heads/main".into(),
        current_commit: None,
        upstream: None,
    });
    let idx = index(
        vec![
            session("s1", "Fix parser", 1),
            mux("work"),
            repo(),
            checkout(),
            pr(),
            pin(),
            workspace(),
            fork(),
            process,
            branch,
        ],
        true,
    );

    let kinds: Vec<NodeKind> = idx.entities().iter().map(|e| e.kind).collect();
    for kind in [
        NodeKind::AgentSession,
        NodeKind::MuxSession,
        NodeKind::Repo,
        NodeKind::Checkout,
        NodeKind::ForgePr,
        NodeKind::Pin,
        NodeKind::Workspace,
        NodeKind::Fork,
    ] {
        assert!(kinds.contains(&kind), "{kind:?} missing from {kinds:?}");
    }
    assert_eq!(idx.len(), 8, "{kinds:?}");
}

#[test]
fn each_kind_is_found_by_its_documented_fields() {
    let idx = index(
        vec![
            session("abc123def", "Fix parser", 1),
            mux("work"),
            repo(),
            checkout(),
            pr(),
            pin(),
            workspace(),
            fork(),
        ],
        true,
    );

    assert_eq!(top(&idx, "Fix parser").as_deref(), Some("Fix parser"));
    assert_eq!(top(&idx, "abc123").as_deref(), Some("Fix parser"), "key");
    assert_eq!(top(&idx, "zebra").as_deref(), Some("Fix parser"), "preview");
    assert_eq!(top(&idx, "work").as_deref(), Some("work"), "mux name");
    assert_eq!(top(&idx, "claude").as_deref(), Some("Fix parser"));
    assert_eq!(top(&idx, "acme/app.git").as_deref(), Some("app"), "remote");
    assert_eq!(
        top(&idx, "feature/login").as_deref(),
        Some("app-wt"),
        "branch"
    );
    assert_eq!(top(&idx, "#42").as_deref(), Some("acme/app#42"), "number");
    assert_eq!(
        top(&idx, "login form").as_deref(),
        Some("acme/app#42"),
        "title"
    );
    assert_eq!(top(&idx, "daily").as_deref(), Some("daily"), "pin");
    assert_eq!(
        top(&idx, "monorepo").as_deref(),
        Some("monorepo"),
        "workspace"
    );
    assert_eq!(top(&idx, "jj").as_deref(), Some("monorepo"), "provider");
    assert_eq!(top(&idx, "sidequest").as_deref(), Some("sidequest"), "fork");
}

#[test]
fn message_previews_are_searchable_only_with_live_preview_on() {
    let nodes = vec![session("s1", "Fix parser", 1)];

    assert!(!index(nodes.clone(), true).rank("zebra").is_empty());
    assert!(index(nodes, false).rank("zebra").is_empty());
}

#[test]
fn an_alias_is_the_label_and_is_searchable() {
    let node = session("s1", "Fix parser", 1);
    let mut snapshot = GraphSnapshot {
        nodes: vec![node.clone()],
        ..GraphSnapshot::empty()
    };
    snapshot.aliases.insert(node.id(), "Puffin".into());

    let idx = EntityIndex::build(&snapshot, true);

    assert_eq!(idx.entities()[0].label, "Puffin");
    assert_eq!(top(&idx, "fix parser").as_deref(), Some("Puffin"), "title");
}

#[test]
fn a_label_match_outranks_a_match_in_another_field() {
    let idx = index(
        vec![session("s1", "Other", 9_000), mux("parser"), repo()],
        true,
    );

    // The newer session only mentions "parser" in its preview.
    assert_eq!(top(&idx, "parser").as_deref(), Some("parser"));
}

#[test]
fn ties_go_by_kind_and_then_recency() {
    // All three are labelled "work" and match at the same position.
    let idx = index(
        vec![
            mux("work"),
            session("old", "work", 10),
            session("new", "work", 20),
        ],
        true,
    );

    let order: Vec<(NodeKind, i64)> = idx
        .rank("work")
        .iter()
        .map(|hit| {
            let e = &idx.entities()[hit.index];
            (e.kind, e.recency)
        })
        .collect();

    assert_eq!(
        order,
        [
            (NodeKind::AgentSession, 20),
            (NodeKind::AgentSession, 10),
            (NodeKind::MuxSession, 0),
        ]
    );
}

#[test]
fn an_earlier_match_outranks_a_later_one() {
    let idx = index(vec![mux("my-work"), mux("work")], true);

    assert_eq!(top(&idx, "work").as_deref(), Some("work"));
}

#[test]
fn gone_entities_are_indexed_and_sort_after_live_ones() {
    let gone = mux("work-old");
    let mut snapshot = GraphSnapshot {
        nodes: vec![gone.clone(), mux("work-new")],
        ..GraphSnapshot::empty()
    };
    snapshot
        .node_presence
        .insert(gone.id(), NodePresence::Gone { last_seen: 77 });

    let idx = EntityIndex::build(&snapshot, true);
    let hits = idx.rank("work");

    let flags: Vec<(String, bool)> = hits
        .iter()
        .map(|hit| {
            let e = &idx.entities()[hit.index];
            (e.label.clone(), e.gone)
        })
        .collect();
    assert_eq!(
        flags,
        [
            ("work-new".to_string(), false),
            ("work-old".to_string(), true)
        ]
    );
}

#[test]
fn the_label_range_marks_the_match_and_is_absent_for_field_matches() {
    let idx = index(vec![mux("my-work")], true);

    let label_hit = &idx.rank("WORK")[0];
    assert_eq!(label_hit.label_range, Some(3..7));
    let field_hit = &idx.rank("claude")[0];
    assert_eq!(field_hit.label_range, None);
}

#[test]
fn an_empty_query_and_a_miss_match_nothing() {
    let idx = index(vec![mux("work")], true);

    assert!(idx.rank("").is_empty());
    assert!(idx.rank("   ").is_empty());
    assert!(idx.rank("nothing like it").is_empty());
}
