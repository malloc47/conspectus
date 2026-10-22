//! Every consumer that acts on, counts, or links to a node skips or
//! refuses a gone one (ADR 0140's live-assumption audit).

use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot,
    MuxSessionId, MuxSessionNode, NodeId, NodePresence, RelationKind, RepoId, WorktreeMeta,
};

const NOW: i64 = 1_790_000_000;

fn mux(name: &str, cwd: &str) -> MuxSessionNode {
    let mut mux = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    mux.cwd = Some(cwd.into());
    mux
}

/// A live mux and a ghost with the same cwd, plus a gone worktree.
fn world() -> GraphSnapshot {
    let ghost = mux("ghost", "/wt/feature");
    let gone_checkout = CheckoutNode::new(
        CheckoutId::new(RepoId::new("/src/app/.git"), "/wt/gone"),
        "/wt/gone",
    )
    .with_worktree(WorktreeMeta::linked());
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::MuxSession(mux("live", "/wt/feature")),
            GraphNode::MuxSession(ghost.clone()),
            GraphNode::Checkout(gone_checkout.clone()),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.node_presence.insert(
        NodeId::MuxSession(ghost.id),
        NodePresence::Gone {
            last_seen: NOW - 86_400,
        },
    );
    snapshot.node_presence.insert(
        NodeId::Checkout(gone_checkout.id),
        NodePresence::Gone {
            last_seen: NOW - 86_400,
        },
    );
    snapshot
}

#[test]
fn tables_list_only_muxes_that_exist() {
    let snapshot = world();
    let options = crate::output::render::RenderOptions::default();
    let rows = crate::output::mux::build_mux_rows_from_snapshot(&snapshot, &["mux"], &options);
    let text = format!("{rows:?}");
    assert!(text.contains("live") && !text.contains("ghost"), "{text}");
    let rows = crate::output::union::build_union_rows_from_snapshot(&snapshot, &["mux"], &options);
    assert!(!format!("{rows:?}").contains("ghost"));
}

#[test]
fn a_gone_mux_cannot_be_renamed() {
    let snapshot = world();
    let err =
        crate::rename::plan_mux_rename(&snapshot, &MuxSessionId::new("tmux:ghost"), "new".into())
            .unwrap_err();
    assert_eq!(err, crate::rename::MuxRenamePlanError::MuxGone);
    assert!(
        crate::rename::plan_mux_rename(&snapshot, &MuxSessionId::new("tmux:live"), "new".into())
            .is_ok()
    );
}

#[test]
fn close_down_never_tears_down_a_gone_mux() {
    let snapshot = world();
    let targets =
        crate::discovery::worktree::close_down::live_mux_teardown_targets(&snapshot, "/wt/feature");
    let names: Vec<_> = targets.iter().map(|t| t.native_id.as_str()).collect();
    assert_eq!(names, ["live"]);
}

#[test]
fn a_hook_record_naming_a_gone_mux_links_nothing_to_it() {
    let mut snapshot = world();
    snapshot
        .nodes
        .push(GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("claude-code", "global", "s1"),
            "claude-code",
        )));
    let record = crate::hook::HookRecord {
        schema_version: 1,
        harness_key: "claude-code".into(),
        session_key: "s1".into(),
        cwd: None,
        pid: None,
        ppid: None,
        tmux: Some(crate::hook::HookTmuxRecord {
            session_name: Some("ghost".into()),
            native_id: Some("ghost".into()),
            pane_id: None,
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".into()),
        observed_epoch: NOW,
        harness_version: None,
        background_harness: false,
    };
    crate::discovery::hook_sidecar::apply_hook_records(&mut snapshot, vec![record], NOW);
    let ghost = NodeId::MuxSession(MuxSessionId::new("tmux:ghost"));
    assert!(
        snapshot.candidate_links.iter().all(|link| {
            link.relation != RelationKind::LinkedToMux
                || link.target != crate::model::LinkEndpoint::Node { id: ghost.clone() }
        }),
        "{:?}",
        snapshot.candidate_links
    );
}

#[test]
fn the_worktree_menu_offers_no_mutation_on_a_gone_worktree() {
    let snapshot = world();
    let gone = NodeId::Checkout(CheckoutId::new(RepoId::new("/src/app/.git"), "/wt/gone"));
    let context = crate::tui::widgets::worktree_menu::context_for_node(&snapshot, &gone, true);
    assert!(!context.can_mutate);
}

#[test]
fn node_show_says_a_node_is_gone() {
    let snapshot = world();
    let text = crate::output::node_show::render_node_show(
        &snapshot,
        &NodeId::MuxSession(MuxSessionId::new("tmux:ghost")),
        false,
    );
    assert!(text.contains("presence: gone (last seen"), "{text}");
    let live = crate::output::node_show::render_node_show(
        &snapshot,
        &NodeId::MuxSession(MuxSessionId::new("tmux:live")),
        false,
    );
    assert!(!live.contains("presence:"));
}

#[test]
fn graph_exports_draw_gone_nodes_dashed() {
    let snapshot = world();
    let dot =
        crate::output::dot::render_graph_dot(&snapshot, crate::output::dot::DotOptions::default())
            .unwrap();
    let ghost_line = dot
        .lines()
        .find(|line| line.contains("ghost") && line.contains("shape="))
        .expect("ghost node line");
    assert!(ghost_line.contains("dashed") && ghost_line.contains("tooltip=\"gone\""));
    let live_line = dot
        .lines()
        .find(|line| line.contains("tmux:live") && line.contains("shape="))
        .expect("live node line");
    assert!(!live_line.contains("tooltip=\"gone\""));
}

#[test]
fn live_nodes_skip_only_what_isnt_live() {
    let snapshot = world();
    assert_eq!(snapshot.live_nodes().count(), 1);
    assert!(snapshot.is_live(&NodeId::MuxSession(MuxSessionId::new("tmux:live"))));
    assert!(!snapshot.is_live(&NodeId::MuxSession(MuxSessionId::new("tmux:ghost"))));
}
