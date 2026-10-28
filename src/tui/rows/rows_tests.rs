use super::*;
use std::path::PathBuf;

#[test]
fn shorten_home_replaces_exact_home_prefix() {
    let home = PathBuf::from("/home/op");
    assert_eq!(shorten_home("/home/op", Some(&home)), "~");
    assert_eq!(shorten_home("/home/op/src/x", Some(&home)), "~/src/x");
}

#[test]
fn shorten_home_keeps_non_home_paths_intact() {
    let home = PathBuf::from("/home/op");
    assert_eq!(shorten_home("/var/log", Some(&home)), "/var/log");
    // Prefix match must respect path boundaries — `/home/operator`
    // is NOT `/home/op/erator`.
    assert_eq!(
        shorten_home("/home/operator", Some(&home)),
        "/home/operator"
    );
}

#[test]
fn shorten_home_with_trailing_slash_in_home_still_matches() {
    let home = PathBuf::from("/home/op/");
    assert_eq!(shorten_home("/home/op/src", Some(&home)), "~/src");
}

#[test]
fn shorten_home_no_home_is_identity() {
    assert_eq!(shorten_home("/home/op/x", None), "/home/op/x");
}

#[test]
fn harness_label_shortens_claude_code() {
    assert_eq!(harness_label("claude-code"), "claude");
    assert_eq!(harness_label("codex"), "codex");
    assert_eq!(harness_label("opencode"), "opencode");
}

#[test]
fn format_recency_buckets_seconds_minutes_hours_days() {
    let now = Some(1_000_000);
    assert_eq!(format_recency(now, Some(999_990)), Some("10s".to_string()));
    assert_eq!(
        format_recency(now, Some(1_000_000 - 120)),
        Some("2m".to_string())
    );
    assert_eq!(
        format_recency(now, Some(1_000_000 - 3 * 3600)),
        Some("3h".to_string())
    );
    assert_eq!(
        format_recency(now, Some(1_000_000 - 5 * 86400)),
        Some("5d".to_string())
    );
}

#[test]
fn format_recency_clamps_future_timestamps_to_zero() {
    let now = Some(1_000_000);
    assert_eq!(format_recency(now, Some(1_000_500)), Some("0s".to_string()));
}

#[test]
fn format_recency_is_none_when_either_side_missing() {
    assert_eq!(format_recency(None, Some(100)), None);
    assert_eq!(format_recency(Some(100), None), None);
}

#[test]
fn recency_bucket_picks_bucket_per_age() {
    let now = Some(1_000_000);
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 60)),
        Some(RecencyBucket::Fresh),
        "1m ago is Fresh",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 5 * 60)),
        Some(RecencyBucket::Active),
        "exactly 5m ago crosses Fresh→Active",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 30 * 60)),
        Some(RecencyBucket::Active),
        "30m ago is Active",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 60 * 60)),
        Some(RecencyBucket::Recent),
        "exactly 1h ago crosses Active→Recent",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 12 * 60 * 60)),
        Some(RecencyBucket::Recent),
        "12h ago is Recent",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 24 * 60 * 60)),
        Some(RecencyBucket::Cold),
        "exactly 1d ago crosses Recent→Cold",
    );
    assert_eq!(
        recency_bucket(now, Some(1_000_000 - 7 * 24 * 60 * 60)),
        Some(RecencyBucket::Cold),
        "7d ago is Cold",
    );
}

#[test]
fn recency_bucket_is_none_when_either_side_missing() {
    assert_eq!(recency_bucket(None, Some(100)), None);
    assert_eq!(recency_bucket(Some(100), None), None);
}

fn program_test_pin(harness: &str, launch_argv: Option<&[&str]>) -> crate::model::PinNode {
    use crate::model::{FieldSource, FieldValue, PinId, PinKind, PinNode, RecipeField, SlotStatus};
    let mut effective = vec![RecipeField {
        name: "harness".to_string(),
        value: FieldValue::Text(harness.to_string()),
        source: FieldSource::Override,
    }];
    if let Some(argv) = launch_argv {
        effective.push(RecipeField {
            name: "argv".to_string(),
            value: FieldValue::List(argv.iter().map(ToString::to_string).collect()),
            source: FieldSource::Override,
        });
    }
    PinNode {
        id: PinId::new("p"),
        kind: PinKind::Mux,
        label: "p".to_string(),
        target: None,
        target_node: None,
        status: SlotStatus::Planned,
        overrides: Default::default(),
        snapshot: Default::default(),
        effective,
        order: 0,
        legacy: false,
        pinned_at: None,
    }
}

#[test]
fn mux_program_prefers_the_current_command_then_start_command_then_pin() {
    use crate::model::{MuxSessionId, MuxSessionNode};
    let bare = MuxSessionNode::new(MuxSessionId::new("tmux:x"), "tmux", "x");
    let serve_pin = program_test_pin("conspectus", Some(&["conspectus", "serve"]));

    let current = bare.clone().with_active_pane_command("/usr/bin/npm");
    assert_eq!(
        mux_program(&current, Some(&serve_pin)).as_deref(),
        Some("npm")
    );

    let mut started = bare.clone().with_active_pane_command("  ");
    started.active_pane_start_command = Some("/nix/store/x/bin/node server.js".to_string());
    assert_eq!(
        mux_program(&started, Some(&serve_pin)).as_deref(),
        Some("node")
    );

    assert_eq!(
        mux_program(&bare, Some(&serve_pin)).as_deref(),
        Some("conspectus")
    );
    assert_eq!(mux_program(&bare, None), None);
}

#[test]
fn pin_program_falls_back_to_the_harness_default_argv() {
    assert_eq!(
        pin_program(&program_test_pin("claude-code", None)).as_deref(),
        Some("claude")
    );
    assert_eq!(
        pin_program(&program_test_pin("custom-launcher", None)).as_deref(),
        Some("custom-launcher")
    );
}

#[test]
fn mux_program_harness_sees_through_an_atelier_exec_launch() {
    use crate::model::{MuxSessionId, MuxSessionNode};
    let bare = MuxSessionNode::new(MuxSessionId::new("tmux:x"), "tmux", "x");

    // tmux reports the wrapper as the pane command; the harness is one
    // of the wrapper's arguments.
    let mut atelier = bare.clone().with_active_pane_command("atelier");
    atelier.active_pane_start_command =
        Some("atelier exec claude --dangerously-skip-permissions".to_string());
    assert_eq!(
        mux_program_harness(&atelier, None, None).as_deref(),
        Some("claude")
    );

    let mut wrapped = bare.clone().with_active_pane_command("atelier");
    wrapped.active_pane_start_command =
        Some("/home/op/bin/atelier exec -- nix develop -c -- codex --yolo".to_string());
    assert_eq!(
        mux_program_harness(&wrapped, None, None).as_deref(),
        Some("codex")
    );

    // A pane started as `atelier exec claude` that has since dropped to
    // a different foreground program is that program, not claude.
    let mut exited = atelier;
    exited.active_pane_command = Some("zsh".to_string());
    assert_eq!(mux_program_harness(&exited, None, None), None);

    // A bare harness binary still maps without any launch wrapper.
    let direct = bare
        .clone()
        .with_active_pane_command("/nix/store/x/bin/codex");
    assert_eq!(
        mux_program_harness(&direct, None, None).as_deref(),
        Some("codex")
    );

    // Non-harness programs stay unmapped.
    let npm = bare.with_active_pane_command("npm");
    assert_eq!(mux_program_harness(&npm, None, None), None);
}

#[test]
fn mux_program_harness_prefers_the_observed_process_harness() {
    use crate::model::{MuxSessionId, MuxSessionNode};
    // An interactive shell where the operator typed `atelier exec codex`:
    // neither tmux field names the harness, but the process walk does.
    let mut shell = MuxSessionNode::new(MuxSessionId::new("tmux:x"), "tmux", "x")
        .with_active_pane_command("atelier");
    shell.active_pane_start_command = Some("zsh".to_string());
    assert_eq!(mux_program_harness(&shell, None, None), None);
    assert_eq!(
        mux_program_harness(&shell, None, Some("codex")).as_deref(),
        Some("codex")
    );
    assert_eq!(
        mux_program_harness(&shell, None, Some("claude-code")).as_deref(),
        Some("claude")
    );
}

#[test]
fn mux_program_harness_falls_back_to_the_slots_argv() {
    use crate::model::{MuxSessionId, MuxSessionNode};
    let bare = MuxSessionNode::new(MuxSessionId::new("tmux:x"), "tmux", "x");
    let atelier_pin = program_test_pin(
        "claude-code",
        Some(&[
            "atelier",
            "exec",
            "claude",
            "--dangerously-skip-permissions",
        ]),
    );
    assert_eq!(
        mux_program_harness(&bare, Some(&atelier_pin), None).as_deref(),
        Some("claude")
    );
    assert_eq!(
        mux_program_harness(&bare, Some(&program_test_pin("codex", None)), None).as_deref(),
        Some("codex")
    );
    let serve_pin = program_test_pin("conspectus", Some(&["conspectus", "serve"]));
    assert_eq!(mux_program_harness(&bare, Some(&serve_pin), None), None);

    // The pin only speaks for a pane that reports nothing itself.
    let npm = bare.with_active_pane_command("npm");
    assert_eq!(mux_program_harness(&npm, Some(&atelier_pin), None), None);
}

#[test]
fn repo_display_path_for_non_bare_repos_ignores_probed_paths() {
    // Whichever checkout a probe reached first, the canonical clone is
    // the directory holding `.git` (ADR 0034).
    let repo = crate::model::RepoId::new("/src/drw/developer/.git");
    let mut node = crate::model::RepoNode::new(repo.clone());
    node.source_paths
        .push("/src/workspaces/documentation/developer".to_string());

    assert_eq!(repo_display_path(&repo, Some(&node)), "/src/drw/developer");
    assert_eq!(repo_display_path(&repo, None), "/src/drw/developer");
}

#[test]
fn repo_display_path_for_bare_repos_uses_a_probed_checkout() {
    let repo = crate::model::RepoId::new("/srv/git/foo.git");
    let mut node = crate::model::RepoNode::new(repo.clone());
    node.source_paths
        .push("/home/op/.agent-deck/multi-repo-worktrees/abc/foo".to_string());
    node.source_paths.push("/home/op/src/foo".to_string());

    assert_eq!(repo_display_path(&repo, Some(&node)), "/home/op/src/foo");
    assert_eq!(repo_display_path(&repo, None), "/srv/git/foo.git");
}

fn group_scope_link(
    id: &str,
    source: &NodeId,
    target: &NodeId,
    relation: crate::model::RelationKind,
) -> crate::model::GraphLink {
    crate::model::GraphLink {
        id: id.to_string(),
        source: source.clone(),
        target: crate::model::LinkEndpoint::Node { id: target.clone() },
        relation,
        provenance: crate::model::Provenance::Discovered,
        confidence: crate::model::Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: crate::model::LinkState::Active,
    }
}

fn group_scope_session(key: &str) -> (NodeId, crate::model::GraphNode) {
    let id = crate::model::AgentSessionId::new("claude-code", "/state", key);
    let node = crate::model::GraphNode::AgentSession(crate::model::AgentSessionNode::new(
        id.clone(),
        "claude-code".to_string(),
    ));
    (NodeId::AgentSession(id), node)
}

fn group_scope_mux(native: &str) -> (NodeId, crate::model::GraphNode) {
    let id = MuxSessionId::new(native);
    let node = crate::model::GraphNode::MuxSession(crate::model::MuxSessionNode::new(
        id.clone(),
        "tmux".to_string(),
        native.to_string(),
    ));
    (NodeId::MuxSession(id), node)
}

#[test]
fn ambiguous_muxes_for_group_aggregates_workspace_scoped_sessions() {
    // When several sessions in one workspace are each ambiguously
    // linked to two muxes, the workspace lists those muxes once, not
    // once per session.
    use crate::model::{GraphNode, GraphSnapshot, RelationKind, WorkspaceId, WorkspaceNode};
    let workspace_id = WorkspaceId::new("/home/op/ws");
    let workspace = NodeId::Workspace(workspace_id.clone());
    let (session_a, node_a) = group_scope_session("a");
    let (session_b, node_b) = group_scope_session("b");
    let (mux_x, node_x) = group_scope_mux("editor");
    let (mux_y, node_y) = group_scope_mux("scratch");

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
        root: "/home/op/ws".to_string(),
        provider: Some("atelier".to_string()),
        name: Some("ws".to_string()),
    }));
    snapshot.nodes.extend([node_a, node_b, node_x, node_y]);
    let mut next_link = 0_usize;
    for session in [&session_a, &session_b] {
        next_link += 1;
        snapshot.candidate_links.push(group_scope_link(
            &format!("test-{next_link}"),
            session,
            &workspace,
            RelationKind::AssociatedWith,
        ));
        for mux in [&mux_x, &mux_y] {
            next_link += 1;
            snapshot.candidate_links.push(group_scope_link(
                &format!("test-{next_link}"),
                session,
                mux,
                RelationKind::LinkedToMux,
            ));
        }
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);

    let muxes = ambiguous_muxes_for_group(&snapshot, &workspace);
    assert_eq!(
        muxes.len(),
        2,
        "two distinct muxes surface once on the workspace, not per session: {muxes:?}",
    );
    assert!(muxes.contains(&mux_x));
    assert!(muxes.contains(&mux_y));
}

#[test]
fn ambiguous_muxes_for_group_is_empty_when_no_session_in_scope_is_ambiguous() {
    use crate::model::{GraphNode, GraphSnapshot, RelationKind, WorkspaceId, WorkspaceNode};
    let workspace_id = WorkspaceId::new("/home/op/ws");
    let workspace = NodeId::Workspace(workspace_id.clone());
    let (session, session_node) = group_scope_session("a");
    let (mux, mux_node) = group_scope_mux("only");

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
        root: "/home/op/ws".to_string(),
        provider: None,
        name: None,
    }));
    snapshot.nodes.extend([session_node, mux_node]);
    snapshot.candidate_links.push(group_scope_link(
        "assoc",
        &session,
        &workspace,
        RelationKind::AssociatedWith,
    ));
    snapshot.candidate_links.push(group_scope_link(
        "mux",
        &session,
        &mux,
        RelationKind::LinkedToMux,
    ));
    let snapshot = crate::resolve::resolve_snapshot(snapshot);

    assert!(
        ambiguous_muxes_for_group(&snapshot, &workspace).is_empty(),
        "one mux per session is not ambiguous",
    );
}

#[test]
fn ambiguous_muxes_for_group_scopes_checkout_and_repo_to_unworkspaced_sessions() {
    // A session associated with a workspace belongs to the workspace,
    // so its muxes do not also surface on the checkout or repo it
    // works in. A session with no workspace belongs to its checkout,
    // and through it to the repo.
    use crate::model::{
        CheckoutId, CheckoutNode, GraphNode, GraphSnapshot, RelationKind, RepoId, RepoNode,
        WorkspaceId, WorkspaceNode,
    };
    let repo_id = RepoId::new("/home/op/src/proj");
    let checkout_id = CheckoutId::new(repo_id.clone(), "/home/op/src/proj");
    let workspace_id = WorkspaceId::new("/home/op/ws");
    let repo = NodeId::Repo(repo_id.clone());
    let checkout = NodeId::Checkout(checkout_id.clone());
    let workspace = NodeId::Workspace(workspace_id.clone());
    let (loose, loose_node) = group_scope_session("loose");
    let (owned, owned_node) = group_scope_session("owned");
    let muxes = [
        group_scope_mux("editor"),
        group_scope_mux("scratch"),
        group_scope_mux("build"),
        group_scope_mux("deploy"),
    ];

    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::Repo(RepoNode::new(repo_id)));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: checkout_id,
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: workspace_id,
        root: "/home/op/ws".to_string(),
        provider: None,
        name: None,
    }));
    snapshot.nodes.extend([loose_node, owned_node]);
    snapshot
        .nodes
        .extend(muxes.iter().map(|(_, node)| node.clone()));
    let links = [
        (&loose, &checkout, RelationKind::AssociatedWith),
        (&owned, &checkout, RelationKind::AssociatedWith),
        (&owned, &workspace, RelationKind::AssociatedWith),
        (&loose, &muxes[0].0, RelationKind::LinkedToMux),
        (&loose, &muxes[1].0, RelationKind::LinkedToMux),
        (&owned, &muxes[2].0, RelationKind::LinkedToMux),
        (&owned, &muxes[3].0, RelationKind::LinkedToMux),
    ];
    for (index, (source, target, relation)) in links.into_iter().enumerate() {
        snapshot.candidate_links.push(group_scope_link(
            &format!("link-{index}"),
            source,
            target,
            relation,
        ));
    }

    // The answer is sorted, whichever order the links came in.
    let mut loose_muxes = vec![muxes[0].0.clone(), muxes[1].0.clone()];
    loose_muxes.sort();
    let mut owned_muxes = vec![muxes[2].0.clone(), muxes[3].0.clone()];
    owned_muxes.sort();
    assert_eq!(ambiguous_muxes_for_group(&snapshot, &checkout), loose_muxes);
    assert_eq!(ambiguous_muxes_for_group(&snapshot, &repo), loose_muxes);
    assert_eq!(
        ambiguous_muxes_for_group(&snapshot, &workspace),
        owned_muxes
    );
    assert!(
        ambiguous_muxes_for_group(&snapshot, &muxes[0].0).is_empty(),
        "only workspace, checkout, and repo groups scope sessions"
    );
}
