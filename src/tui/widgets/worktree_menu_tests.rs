use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, MuxSessionId, MuxSessionNode,
    RepoId, WorktreeMeta,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn worktree_ctx(guard: Vec<String>, can_mutate: bool) -> WorktreeMenuContext {
    WorktreeMenuContext {
        context: WorktreeContext::Worktree,
        repo_root: Some("/wt/feature".to_string()),
        branch: Some("feature".to_string()),
        guard_sessions: guard,
        can_mutate,
        reveal_target: None,
    }
}

#[test]
fn new_offers_wired_mutations_for_a_worktree() {
    let state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("actions");
    assert_eq!(
        state.actions,
        vec![
            WorktreeAction::NewWorktree,
            WorktreeAction::MergeWorktree,
            WorktreeAction::RemoveWorktree,
            WorktreeAction::CloseDownWorktree,
        ],
    );
}

#[test]
fn close_down_choice_merges_or_discards() {
    // Navigate to CloseDownWorktree (index 3) and open the choice.
    let mut state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("ok");
    for _ in 0..3 {
        state.handle((), key(KeyCode::Down));
    }
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    // `m` lands the branch (discard = false).
    assert_eq!(
        state.handle((), char_key('m')),
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeCloseDown {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            discard: false,
        })),
    );
}

#[test]
fn close_down_hotkey_opens_choice_and_discards() {
    // `new_close_down` skips the action list and opens the choice.
    let mut state =
        WorktreeMenuState::new_close_down(worktree_ctx(vec!["codex (agent)".to_string()], true))
            .expect("close-down applicable");
    // `d` discards the branch (discard = true).
    assert_eq!(
        state.handle((), char_key('d')),
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeCloseDown {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            discard: true,
        })),
    );
}

#[test]
fn close_down_hotkey_none_without_branch_or_backend() {
    // No branch → not applicable.
    let mut no_branch = worktree_ctx(vec![], true);
    no_branch.branch = None;
    assert!(WorktreeMenuState::new_close_down(no_branch).is_none());
    // Read-only → not applicable.
    assert!(WorktreeMenuState::new_close_down(worktree_ctx(vec![], false)).is_none());
}

#[test]
fn new_is_none_for_read_only_worktree() {
    // Read-only: mutations gated, reveal not yet wired → nothing to show.
    assert!(WorktreeMenuState::new(worktree_ctx(vec![], false)).is_none());
}

#[test]
fn create_flow_commits_worktree_create() {
    let mut state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("actions");
    // Cursor starts on NewWorktree; Enter opens the branch input.
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    // Type a branch name, then confirm.
    for c in "wip".chars() {
        assert_eq!(state.handle((), char_key(c)), OverlayOutcome::Consumed);
    }
    let outcome = state.handle((), key(KeyCode::Enter));
    assert_eq!(
        outcome,
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeCreate {
            repo_root: "/wt/feature".to_string(),
            branch: "wip".to_string(),
        })),
    );
}

#[test]
fn remove_flow_forces_when_guarded() {
    let mut state =
        WorktreeMenuState::new(worktree_ctx(vec!["codex (agent)".to_string()], true)).expect("ok");
    // Move past MergeWorktree to RemoveWorktree and open the confirm.
    assert_eq!(
        state.handle((), key(KeyCode::Down)),
        OverlayOutcome::Consumed
    );
    assert_eq!(
        state.handle((), key(KeyCode::Down)),
        OverlayOutcome::Consumed
    );
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    // Confirm → force is set because a live session was surfaced.
    assert_eq!(
        state.handle((), char_key('y')),
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeRemove {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            force: true,
        })),
    );
}

#[test]
fn remove_without_live_sessions_does_not_force() {
    let mut state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("ok");
    state.handle((), key(KeyCode::Down));
    state.handle((), key(KeyCode::Down));
    state.handle((), key(KeyCode::Enter));
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeRemove {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            force: false,
        })),
    );
}

#[test]
fn confirm_remove_can_be_cancelled_back_to_list() {
    let mut state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("ok");
    state.handle((), key(KeyCode::Down));
    state.handle((), key(KeyCode::Enter)); // into ConfirmRemove
    assert_eq!(
        state.handle((), key(KeyCode::Esc)),
        OverlayOutcome::Consumed
    );
    // Back in list — Esc now closes.
    assert_eq!(state.handle((), key(KeyCode::Esc)), OverlayOutcome::Close);
}

// ---- context_for_node ----

fn linked_checkout(repo: &str, path: &str, branch: &str) -> GraphNode {
    let repo_id = RepoId::new(repo);
    let mut c = CheckoutNode::new(CheckoutId::new(repo_id.clone(), path), path)
        .with_worktree(WorktreeMeta::linked());
    c.current_branch = Some(crate::model::BranchId::new(repo_id, branch));
    GraphNode::Checkout(c)
}

#[test]
fn context_for_checkout_carries_root_branch_and_guard() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/feature",
        "refs/heads/feature",
    ));
    snap.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(AgentSessionId::new("codex", "/s", "s1"), "codex")
            .with_cwd("/wt/feature/x".to_string()),
    ));
    let id = NodeId::Checkout(CheckoutId::new(RepoId::new("/app/.git"), "/wt/feature"));

    let ctx = context_for_node(&snap, &id, true);
    assert_eq!(ctx.context, WorktreeContext::Worktree);
    assert_eq!(ctx.repo_root.as_deref(), Some("/wt/feature"));
    assert_eq!(ctx.branch.as_deref(), Some("feature"));
    assert_eq!(ctx.guard_sessions.len(), 1);
}

#[test]
fn context_for_repo_uses_primary_worktree_root() {
    let mut snap = GraphSnapshot::empty();
    let repo_id = RepoId::new("/app/.git");
    let mut primary = CheckoutNode::new(CheckoutId::new(repo_id.clone(), "/app"), "/app")
        .with_worktree(WorktreeMeta::primary());
    primary.current_branch = Some(crate::model::BranchId::new(
        repo_id.clone(),
        "refs/heads/main",
    ));
    snap.nodes.push(GraphNode::Checkout(primary));
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/feature",
        "refs/heads/feature",
    ));

    let ctx = context_for_node(&snap, &NodeId::Repo(repo_id), true);
    assert_eq!(ctx.context, WorktreeContext::Repo);
    assert_eq!(ctx.repo_root.as_deref(), Some("/app"), "primary worktree");
}

#[test]
fn context_for_mux_resolves_its_rooted_worktree() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/feature",
        "refs/heads/feature",
    ));
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(MuxSessionId::new("tmux:feat"), "tmux", "feat")
            .with_active_pane_current_path("/wt/feature".to_string()),
    ));
    let id = NodeId::MuxSession(MuxSessionId::new("tmux:feat"));

    let ctx = context_for_node(&snap, &id, true);
    assert_eq!(ctx.context, WorktreeContext::Mux);
    assert_eq!(ctx.repo_root.as_deref(), Some("/wt/feature"));
    assert_eq!(ctx.branch.as_deref(), Some("feature"));
    // Reveal-checkout target is the containing checkout's group row.
    assert_eq!(
        ctx.reveal_target,
        Some(RowId::Group(NodeId::Checkout(CheckoutId::new(
            RepoId::new("/app/.git"),
            "/wt/feature"
        ))))
    );
}

#[test]
fn repo_prune_flow_commits_prune() {
    let ctx = WorktreeMenuContext {
        context: WorktreeContext::Repo,
        repo_root: Some("/app".to_string()),
        branch: None,
        guard_sessions: vec![],
        can_mutate: true,
        reveal_target: None,
    };
    let mut state = WorktreeMenuState::new(ctx).expect("actions");
    assert_eq!(
        state.actions,
        vec![WorktreeAction::NewWorktree, WorktreeAction::PruneWorktrees],
    );
    // Navigate to prune, confirm.
    state.handle((), key(KeyCode::Down));
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Consumed
    );
    assert_eq!(
        state.handle((), char_key('y')),
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreePrune {
            repo_root: "/app".to_string(),
        })),
    );
}

// ---- reveal / navigate ----

#[test]
fn context_for_worktree_reveal_targets_first_session() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/feature",
        "refs/heads/feature",
    ));
    snap.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(MuxSessionId::new("tmux:feat"), "tmux", "feat")
            .with_active_pane_current_path("/wt/feature/src".to_string()),
    ));
    let id = NodeId::Checkout(CheckoutId::new(RepoId::new("/app/.git"), "/wt/feature"));

    let ctx = context_for_node(&snap, &id, true);
    assert_eq!(
        ctx.reveal_target,
        Some(RowId::MuxSession(NodeId::MuxSession(MuxSessionId::new(
            "tmux:feat"
        ))))
    );
    // With a target, the read-only reveal action is offered even on a
    // read-only host.
    let state = WorktreeMenuState::new(WorktreeMenuContext {
        can_mutate: false,
        ..ctx
    })
    .expect("reveal offered");
    assert!(state.actions.contains(&WorktreeAction::RevealSessions));
}

#[test]
fn reveal_action_commits_a_selection_jump() {
    let mut ctx = worktree_ctx(vec![], true);
    let target = RowId::MuxSession(NodeId::MuxSession(MuxSessionId::new("tmux:feat")));
    ctx.reveal_target = Some(target.clone());
    let mut state = WorktreeMenuState::new(ctx).expect("actions");
    // RevealSessions is the last offered action for a worktree.
    while state.selected() != WorktreeAction::RevealSessions {
        state.handle((), key(KeyCode::Down));
    }
    assert_eq!(
        state.handle((), key(KeyCode::Enter)),
        OverlayOutcome::Commit(Box::new(Msg::SelectRow(Box::new(target)))),
    );
}

#[test]
fn reveal_dropped_without_a_target() {
    // A worktree with no live sessions has no reveal target, so the
    // reveal action isn't offered.
    let state = WorktreeMenuState::new(worktree_ctx(vec![], true)).expect("actions");
    assert!(!state.actions.contains(&WorktreeAction::RevealSessions));
}
