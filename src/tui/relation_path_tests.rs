use super::*;
use crate::model::{
    AgentSessionId, BranchId, CheckoutId, MuxSessionId, RepoId, ResolvedRelationship,
};

fn repo(name: &str) -> NodeId {
    NodeId::Repo(RepoId::new(format!("/src/{name}/.git")))
}

fn mux(name: &str) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(format!("tmux:{name}")))
}

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "global", key))
}

fn checkout(repo_name: &str, root: &str) -> NodeId {
    NodeId::Checkout(CheckoutId::new(
        RepoId::new(format!("/src/{repo_name}/.git")),
        root,
    ))
}

fn edge(source: &NodeId, target: &NodeId, relation: RelationKind) -> ResolvedRelationship {
    ResolvedRelationship {
        source: source.clone(),
        target: target.clone(),
        relation,
        selected_link_id: Some(format!("{source}->{target}")),
        competing_link_ids: Vec::new(),
        corroborating_link_ids: Vec::new(),
        explanation: None,
    }
}

fn world(edges: Vec<ResolvedRelationship>) -> GraphSnapshot {
    GraphSnapshot {
        resolved_relationships: edges,
        ..GraphSnapshot::empty()
    }
}

#[test]
fn a_neighbor_is_one_hop_away_whichever_way_the_edge_points() {
    let (s, m) = (session("s"), mux("work"));
    let snapshot = world(vec![edge(&s, &m, RelationKind::LinkedToMux)]);

    let from_session = Reachability::from(&snapshot, &s, MAX_HOPS);
    let from_mux = Reachability::from(&snapshot, &m, MAX_HOPS);

    assert_eq!(from_session.path_to(&m), Some([m.clone()].as_slice()));
    assert_eq!(from_mux.path_to(&s), Some([s.clone()].as_slice()));
}

#[test]
fn a_path_lists_every_hop_after_the_start() {
    let (s, m, r) = (session("s"), mux("work"), repo("app"));
    let snapshot = world(vec![
        edge(&s, &m, RelationKind::LinkedToMux),
        edge(&m, &r, RelationKind::RootedIn),
    ]);

    let reach = Reachability::from(&snapshot, &s, MAX_HOPS);

    assert_eq!(reach.path_to(&r), Some([m, r.clone()].as_slice()));
    assert_eq!(reach.path_to(&s), None, "the start is not a destination");
    assert_eq!(reach.start(), Some(&s));
}

#[test]
fn nothing_beyond_the_hop_limit_is_reachable() {
    let nodes: Vec<NodeId> = (0..5).map(|i| session(&format!("s{i}"))).collect();
    let edges = nodes
        .windows(2)
        .map(|pair| edge(&pair[0], &pair[1], RelationKind::ParentSession))
        .collect();
    let snapshot = world(edges);

    let reach = Reachability::from(&snapshot, &nodes[0], 3);

    assert_eq!(reach.path_to(&nodes[3]).map(<[NodeId]>::len), Some(3));
    assert_eq!(reach.path_to(&nodes[4]), None, "four hops is too far");
}

#[test]
fn only_resolver_winners_count() {
    let (s, m) = (session("s"), mux("work"));
    let mut loser = edge(&s, &m, RelationKind::LinkedToMux);
    loser.selected_link_id = None;
    let snapshot = world(vec![loser]);

    assert_eq!(
        Reachability::from(&snapshot, &s, MAX_HOPS).path_to(&m),
        None,
        "an ambiguous slot has no answer to follow"
    );
}

#[test]
fn runtime_process_relations_never_form_a_path() {
    let (s, m) = (session("s"), mux("work"));
    let process = NodeId::RuntimeProcess(crate::model::RuntimeProcessId::new("proc:1"));
    let snapshot = world(vec![
        edge(&m, &process, RelationKind::MuxContainsProcess),
        edge(&process, &s, RelationKind::ProcessIdentifiesSession),
    ]);

    let reach = Reachability::from(&snapshot, &m, MAX_HOPS);

    assert_eq!(reach.path_to(&s), None);
    assert_eq!(reach.path_to(&process), None);
}

#[test]
fn a_shorter_path_beats_a_longer_one() {
    let (a, b, c) = (session("a"), mux("b"), repo("c"));
    let snapshot = world(vec![
        edge(&a, &b, RelationKind::LinkedToMux),
        edge(&b, &c, RelationKind::RootedIn),
        edge(&a, &c, RelationKind::AssociatedWith),
    ]);

    let reach = Reachability::from(&snapshot, &a, MAX_HOPS);

    assert_eq!(reach.path_to(&c), Some([c.clone()].as_slice()));
}

#[test]
fn equal_paths_prefer_a_structural_first_hop() {
    // Two routes of two hops to `target`; the one through `loose` starts
    // with a looser relation even though its id sorts first.
    let start = session("start");
    let loose = checkout("aaa", "/src/aaa");
    let firm = mux("zzz");
    let target = repo("target");
    let snapshot = world(vec![
        edge(&start, &loose, RelationKind::AssociatedWith),
        edge(&loose, &target, RelationKind::BelongsToRepo),
        edge(&start, &firm, RelationKind::LinkedToMux),
        edge(&firm, &target, RelationKind::RootedIn),
    ]);

    let reach = Reachability::from(&snapshot, &start, MAX_HOPS);

    assert_eq!(
        reach.path_to(&target),
        Some([firm, target.clone()].as_slice())
    );
}

#[test]
fn remaining_ties_take_the_path_whose_ids_sort_first() {
    let start = session("start");
    let first = mux("alpha");
    let second = mux("beta");
    let target = repo("target");
    let snapshot = world(vec![
        edge(&start, &second, RelationKind::LinkedToMux),
        edge(&second, &target, RelationKind::RootedIn),
        edge(&start, &first, RelationKind::LinkedToMux),
        edge(&first, &target, RelationKind::RootedIn),
    ]);

    let reach = Reachability::from(&snapshot, &start, MAX_HOPS);

    assert_eq!(
        reach.path_to(&target),
        Some([first, target.clone()].as_slice())
    );
}

#[test]
fn a_structural_relation_anywhere_between_two_nodes_makes_the_first_hop_structural() {
    let start = session("start");
    let hop = mux("hop");
    let other = checkout("x", "/src/x");
    let target = NodeId::Branch(BranchId::new(RepoId::new("/src/x/.git"), "refs/heads/main"));
    let snapshot = world(vec![
        // Two relations between the same pair: one loose, one structural.
        edge(&start, &hop, RelationKind::AssociatedWith),
        edge(&start, &hop, RelationKind::LinkedToMux),
        edge(&hop, &target, RelationKind::AssociatedBranch),
        edge(&start, &other, RelationKind::AssociatedWith),
        edge(&other, &target, RelationKind::CheckedOutBranch),
    ]);

    let reach = Reachability::from(&snapshot, &start, MAX_HOPS);

    assert_eq!(
        reach.path_to(&target),
        Some([hop, target.clone()].as_slice())
    );
}

#[test]
fn a_start_with_no_edges_reaches_nothing() {
    let snapshot = world(Vec::new());
    let s = session("alone");

    let reach = Reachability::from(&snapshot, &s, MAX_HOPS);

    assert_eq!(reach.path_to(&mux("work")), None);
}
