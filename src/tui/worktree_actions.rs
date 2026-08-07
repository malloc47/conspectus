//! Worktree action model (H-WT-004b).
//!
//! The `w` worktree action menu is context-sensitive: which actions it
//! offers depends on what the operator has selected (a repo, a worktree
//! checkout, an agent session, or a mux) and whether a mutation backend
//! is available. This module is the pure decision core — it maps a
//! [`WorktreeContext`] + capability to the ordered list of applicable
//! [`WorktreeAction`]s. The overlay widget, keybindings, and effect
//! wiring consume it; keeping the policy here makes it unit-testable
//! and the single place new actions (merge / close-down / lock / prune
//! from later H-WT stories) slot into.

/// What the operator currently has selected, from the worktree
/// interaction point of view. Derived from the selected node's kind.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorktreeContext {
    /// A git repo node (the origin — creation entry point).
    Repo,
    /// A checkout node that is a worktree (carries worktree metadata).
    Worktree,
    /// An agent session (runs inside a worktree).
    Agent,
    /// A mux session (rooted in a worktree).
    Mux,
    /// Nothing worktree-relevant is selected.
    None,
}

/// A worktree action the menu can offer. Only the H-WT-004b subset
/// (create / remove / reveal) is modeled today; merge, close-down,
/// new-stream, lock/unlock, and prune land with their own stories and
/// extend this enum + [`worktree_actions`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorktreeAction {
    /// Create a new worktree + branch (mutation).
    NewWorktree,
    /// Merge the worktree's branch back and tear it down (mutation).
    MergeWorktree,
    /// Remove the selected worktree (mutation, guarded).
    RemoveWorktree,
    /// Close down a whole stream of work: land or discard the branch,
    /// terminate its sessions, remove the worktree, drop its pins
    /// (compound mutation; ADR 0093).
    CloseDownWorktree,
    /// Prune worktrees already merged into the repo's default branch
    /// (repo-level mutation; H-WT-008).
    PruneWorktrees,
    /// Select the checkout/worktree the selected agent or mux is in.
    RevealCheckout,
    /// List the sessions rooted in the selected worktree.
    RevealSessions,
}

impl WorktreeAction {
    /// Whether this action mutates worktree state (and therefore
    /// requires a mutation-capable backend). Reveal actions are
    /// read-only graph navigation.
    pub fn is_mutation(self) -> bool {
        matches!(
            self,
            Self::NewWorktree
                | Self::MergeWorktree
                | Self::RemoveWorktree
                | Self::CloseDownWorktree
                | Self::PruneWorktrees
        )
    }

    /// Menu label.
    pub fn label(self) -> &'static str {
        match self {
            Self::NewWorktree => "New worktree…",
            Self::MergeWorktree => "Merge back & close",
            Self::RemoveWorktree => "Remove worktree",
            Self::CloseDownWorktree => "Close down stream…",
            Self::PruneWorktrees => "Prune merged worktrees…",
            Self::RevealCheckout => "Reveal checkout",
            Self::RevealSessions => "Reveal sessions",
        }
    }
}

/// The ordered actions the menu offers for `context`, given whether a
/// mutation-capable backend is available. Mutation actions are omitted
/// entirely when `can_mutate` is false (read-only host) so the menu
/// never dangles an action that would immediately error; read-only
/// reveal actions are always offered.
pub fn worktree_actions(context: WorktreeContext, can_mutate: bool) -> Vec<WorktreeAction> {
    use WorktreeAction::*;
    let mut actions = Vec::new();
    match context {
        WorktreeContext::Repo => {
            if can_mutate {
                actions.push(NewWorktree);
                actions.push(PruneWorktrees);
            }
        }
        WorktreeContext::Worktree => {
            if can_mutate {
                actions.push(NewWorktree); // sibling
                actions.push(MergeWorktree);
                actions.push(RemoveWorktree);
                actions.push(CloseDownWorktree);
            }
            actions.push(RevealSessions);
        }
        WorktreeContext::Agent => {
            actions.push(RevealCheckout);
        }
        WorktreeContext::Mux => {
            if can_mutate {
                actions.push(NewWorktree); // parallel
                actions.push(MergeWorktree);
                actions.push(RemoveWorktree);
                actions.push(CloseDownWorktree);
            }
            actions.push(RevealCheckout);
        }
        WorktreeContext::None => {}
    }
    actions
}

#[cfg(test)]
#[path = "worktree_actions_tests.rs"]
mod tests;
