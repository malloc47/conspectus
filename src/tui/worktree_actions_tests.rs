use super::*;

#[test]
fn repo_offers_create_only_when_mutable() {
    assert_eq!(
        worktree_actions(WorktreeContext::Repo, true),
        vec![WorktreeAction::NewWorktree],
    );
    assert!(worktree_actions(WorktreeContext::Repo, false).is_empty());
}

#[test]
fn worktree_offers_create_remove_and_reveal_sessions() {
    let actions = worktree_actions(WorktreeContext::Worktree, true);
    assert_eq!(
        actions,
        vec![
            WorktreeAction::NewWorktree,
            WorktreeAction::RemoveWorktree,
            WorktreeAction::RevealSessions,
        ],
    );
    // Read-only host still gets the reveal action, no mutations.
    assert_eq!(
        worktree_actions(WorktreeContext::Worktree, false),
        vec![WorktreeAction::RevealSessions],
    );
}

#[test]
fn agent_offers_reveal_checkout_regardless_of_backend() {
    for can_mutate in [true, false] {
        assert_eq!(
            worktree_actions(WorktreeContext::Agent, can_mutate),
            vec![WorktreeAction::RevealCheckout],
        );
    }
}

#[test]
fn mux_offers_mutations_and_reveal() {
    let actions = worktree_actions(WorktreeContext::Mux, true);
    assert!(actions.contains(&WorktreeAction::NewWorktree));
    assert!(actions.contains(&WorktreeAction::RemoveWorktree));
    assert!(actions.contains(&WorktreeAction::RevealCheckout));

    // Read-only: only the reveal action.
    assert_eq!(
        worktree_actions(WorktreeContext::Mux, false),
        vec![WorktreeAction::RevealCheckout],
    );
}

#[test]
fn none_context_offers_nothing() {
    assert!(worktree_actions(WorktreeContext::None, true).is_empty());
}

#[test]
fn mutation_flag_matches_action_set() {
    assert!(WorktreeAction::NewWorktree.is_mutation());
    assert!(WorktreeAction::RemoveWorktree.is_mutation());
    assert!(!WorktreeAction::RevealCheckout.is_mutation());
    assert!(!WorktreeAction::RevealSessions.is_mutation());
}
