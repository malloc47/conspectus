//! The worktree action menu (`w`) and the questions its entries ask.
//!
//! The menu captures the selected node's worktree facts when it opens
//! ([`context_for_node`]) and offers the [`worktree_actions`] policy's
//! wired verbs as the framework's [`ActionMenu`]. A reveal entry commits
//! its jump at once; every other entry asks a further question, which
//! opens over the menu as a step of it ([`step`]): the branch prompt for a
//! new worktree (a [`TextPrompt`] that refuses a blank name), the merge,
//! remove, and prune confirmations, and the merge-or-discard choice of a
//! close-down (each a [`Dialog`] whose body says what the action will do
//! and which live sessions it affects). `Esc` in a step comes back to the
//! menu; a commit or `Ctrl-C` closes both. The step commits one
//! `Msg::CommitWorktree*` message, which the reducer turns into the
//! mutation effect. `X` opens the close-down choice on its own
//! ([`close_down_dialog`]), as a compound teardown (ADR 0093).

use std::path::Path;
use tuikit::action_menu::{ActionMenu, Entry};
use tuikit::dialog::{Dialog, Verb};
use tuikit::keys::ch;
use tuikit::text_prompt::{PromptResult, TextPrompt};

use crate::model::{
    CheckoutNode, GraphNode, GraphSnapshot, NodeId, RepoId, SnapshotIndex, WorktreeKind,
    path_is_ancestor_of,
};
use crate::tui::rows::RowId;
use crate::tui::worktree_actions::{WorktreeAction, WorktreeContext, worktree_actions};
use crate::tui::{Modal, Msg};

/// Actions the runtime knows how to dispatch today. `worktree_actions`
/// may return more (reveal, future stories); the menu offers only
/// these until each is wired.
const WIRED: &[WorktreeAction] = &[
    WorktreeAction::NewWorktree,
    WorktreeAction::MergeWorktree,
    WorktreeAction::RemoveWorktree,
    WorktreeAction::CloseDownWorktree,
    WorktreeAction::PruneWorktrees,
    WorktreeAction::RevealCheckout,
    WorktreeAction::RevealSessions,
];

/// The title of the menu and of the questions it asks.
const TITLE: &str = "Worktree";

/// What a confirmation names a worktree whose branch is not known.
const UNKNOWN_BRANCH: &str = "(unknown)";

/// Worktree facts captured when the menu opens, so the menu and its
/// steps need no graph afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeMenuContext {
    pub context: WorktreeContext,
    /// A path inside the repo (for `-C`). Always `Some` when a mutation
    /// action is offered.
    pub repo_root: Option<String>,
    /// The worktree's branch (short name), for remove.
    pub branch: Option<String>,
    /// Live sessions rooted in the worktree, surfaced in the remove
    /// confirm (the same guard `worktree rm` applies).
    pub guard_sessions: Vec<String>,
    pub can_mutate: bool,
    /// Row to jump to for the context's reveal action: the
    /// containing checkout (from an agent/mux) or the first session in
    /// the worktree. `None` disables the reveal action.
    pub reveal_target: Option<RowId>,
}

/// Build the menu context for the selected `node` from the graph.
/// Pure so it can be unit-tested; the runtime calls it on `w`.
pub fn context_for_node(
    snapshot: &GraphSnapshot,
    node: &NodeId,
    can_mutate: bool,
) -> WorktreeMenuContext {
    let index = SnapshotIndex::new(snapshot);
    // A gone repo or worktree has nothing to change until it is
    // rebuilt (ADR 0140, ADR 0142).
    let can_mutate = can_mutate && index.is_live(node);
    match node {
        NodeId::Repo(repo_id) => WorktreeMenuContext {
            context: WorktreeContext::Repo,
            repo_root: primary_worktree_root(snapshot, repo_id),
            branch: None,
            guard_sessions: Vec::new(),
            can_mutate,
            reveal_target: None,
        },
        NodeId::Checkout(checkout_id) => {
            let branch = match index.node(node) {
                Some(GraphNode::Checkout(c)) => checkout_short_branch(c),
                _ => None,
            };
            WorktreeMenuContext {
                context: WorktreeContext::Worktree,
                repo_root: Some(checkout_id.root.clone()),
                branch,
                guard_sessions: live_sessions_in_worktree(snapshot, &checkout_id.root),
                can_mutate,
                // Reveal sessions: jump to the first live session inside.
                reveal_target: first_session_row_in(snapshot, &checkout_id.root),
            }
        }
        NodeId::MuxSession(_) => {
            let cwd = match index.node(node) {
                Some(GraphNode::MuxSession(m)) => {
                    m.active_pane_current_path.clone().or_else(|| m.cwd.clone())
                }
                _ => None,
            };
            let checkout = cwd
                .as_deref()
                .and_then(|c| worktree_checkout_containing(snapshot, c));
            WorktreeMenuContext {
                context: WorktreeContext::Mux,
                repo_root: checkout.map(|c| c.root.clone()),
                branch: checkout.and_then(checkout_short_branch),
                guard_sessions: cwd
                    .as_deref()
                    .map(|c| live_sessions_in_worktree(snapshot, c))
                    .unwrap_or_default(),
                can_mutate,
                // Reveal checkout: jump to the containing checkout row.
                reveal_target: checkout.map(checkout_group_row),
            }
        }
        NodeId::AgentSession(_) => {
            let cwd = match index.node(node) {
                Some(GraphNode::AgentSession(s)) => s.cwd.clone(),
                _ => None,
            };
            WorktreeMenuContext {
                context: WorktreeContext::Agent,
                repo_root: None,
                branch: None,
                guard_sessions: Vec::new(),
                can_mutate,
                reveal_target: cwd
                    .as_deref()
                    .and_then(|c| worktree_checkout_containing(snapshot, c))
                    .map(checkout_group_row),
            }
        }
        _ => WorktreeMenuContext {
            context: WorktreeContext::None,
            repo_root: None,
            branch: None,
            guard_sessions: Vec::new(),
            can_mutate,
            reveal_target: None,
        },
    }
}

/// The innermost worktree checkout whose root contains `path`. Its
/// group row is the reveal-checkout jump target, and its root and
/// branch scope the menu for a mux rooted inside it. Innermost, because
/// a worktree can sit inside another checkout's tree.
fn worktree_checkout_containing<'a>(
    snapshot: &'a GraphSnapshot,
    path: &str,
) -> Option<&'a CheckoutNode> {
    let p = Path::new(path);
    snapshot
        .live_nodes()
        .filter_map(|n| match n {
            GraphNode::Checkout(c)
                if c.worktree.is_some() && path_is_ancestor_of(Path::new(&c.root), p) =>
            {
                Some(c)
            }
            _ => None,
        })
        .max_by_key(|c| Path::new(&c.root).components().count())
}

fn checkout_group_row(checkout: &CheckoutNode) -> RowId {
    RowId::Group(NodeId::Checkout(checkout.id.clone()))
}

fn checkout_short_branch(checkout: &CheckoutNode) -> Option<String> {
    checkout
        .current_branch
        .as_ref()
        .map(|b| short_branch(&b.refname))
}

/// The first live session rooted in `worktree_path` — the
/// reveal-sessions jump target. Prefers a mux session over an agent so
/// the operator lands on the attachable row.
fn first_session_row_in(snapshot: &GraphSnapshot, worktree_path: &str) -> Option<RowId> {
    let root = Path::new(worktree_path);
    let mux = snapshot.live_nodes().find_map(|n| match n {
        GraphNode::MuxSession(m) => {
            let cwd = m.active_pane_current_path.as_deref().or(m.cwd.as_deref());
            cwd.filter(|c| path_is_ancestor_of(root, Path::new(c)))
                .map(|_| RowId::MuxSession(NodeId::MuxSession(m.id.clone())))
        }
        _ => None,
    });
    mux.or_else(|| {
        snapshot.live_nodes().find_map(|n| match n {
            GraphNode::AgentSession(s) => s
                .cwd
                .as_deref()
                .filter(|c| path_is_ancestor_of(root, Path::new(c)))
                .map(|_| RowId::AgentSession(NodeId::AgentSession(s.id.clone()))),
            _ => None,
        })
    })
}

/// The primary worktree's root for a repo (the checkout git lists
/// first), used as the `-C` path when creating from a repo node.
fn primary_worktree_root(snapshot: &GraphSnapshot, repo: &RepoId) -> Option<String> {
    let mut fallback = None;
    for node in snapshot.live_nodes() {
        if let GraphNode::Checkout(c) = node
            && &c.id.repo == repo
        {
            match c.worktree.as_ref().map(|w| w.kind) {
                Some(WorktreeKind::Primary) => return Some(c.root.clone()),
                _ => fallback.get_or_insert_with(|| c.root.clone()),
            };
        }
    }
    fallback
}

/// Live agent/mux sessions rooted in `worktree_path` (the
/// `worktree rm` guard, reused for the remove confirm).
fn live_sessions_in_worktree(snapshot: &GraphSnapshot, worktree_path: &str) -> Vec<String> {
    let root = Path::new(worktree_path);
    let mut out = Vec::new();
    for node in snapshot.live_nodes() {
        match node {
            GraphNode::AgentSession(s) => {
                if let Some(cwd) = &s.cwd
                    && path_is_ancestor_of(root, Path::new(cwd))
                {
                    out.push(format!("{} (agent)", s.harness_key));
                }
            }
            GraphNode::MuxSession(m) => {
                let cwd = m.active_pane_current_path.as_deref().or(m.cwd.as_deref());
                if let Some(cwd) = cwd
                    && path_is_ancestor_of(root, Path::new(cwd))
                {
                    out.push(format!("{} (mux)", m.native_id));
                }
            }
            _ => {}
        }
    }
    out
}

fn short_branch(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string()
}

/// Whether `action` is a read-only reveal/navigate jump,
/// which needs a resolved `reveal_target` to be offerable.
fn is_reveal(action: WorktreeAction) -> bool {
    matches!(
        action,
        WorktreeAction::RevealCheckout | WorktreeAction::RevealSessions
    )
}

/// Whether `action` acts on the worktree's branch, so it cannot run on a
/// worktree with none checked out.
fn needs_branch(action: WorktreeAction) -> bool {
    matches!(
        action,
        WorktreeAction::RemoveWorktree | WorktreeAction::CloseDownWorktree
    )
}

/// The menu for a selection: the wired subset of the policy, without the
/// mutations when there is no `repo_root` to target and without the
/// reveals that have no jump target. `None` when nothing is offerable, so
/// the runtime can post a status instead of opening an empty menu.
pub fn worktree_menu(ctx: &WorktreeMenuContext) -> Option<ActionMenu<Msg>> {
    let has_repo = ctx.repo_root.is_some();
    let entries: Vec<Entry<Msg>> = worktree_actions(ctx.context, ctx.can_mutate)
        .into_iter()
        .filter(|a| WIRED.contains(a))
        .filter(|a| !a.is_mutation() || has_repo)
        .filter_map(|action| entry(action, ctx))
        .collect();
    (!entries.is_empty()).then(|| ActionMenu::new(TITLE, entries))
}

/// The menu's entry for `action`: a reveal commits its jump, and every
/// other action opens its question as a step over the menu. A verb on
/// the branch is shown but cannot run when no branch is checked out.
fn entry(action: WorktreeAction, ctx: &WorktreeMenuContext) -> Option<Entry<Msg>> {
    if is_reveal(action) {
        let row = ctx.reveal_target.clone()?;
        return Some(Entry::new(action.label(), Msg::SelectRow(Box::new(row))));
    }
    let open = Msg::OpenWorktreeStep {
        action,
        context: Box::new(ctx.clone()),
    };
    let entry = Entry::new(action.label(), open).opens_step();
    Some(if needs_branch(action) && ctx.branch.is_none() {
        entry
            .detail("no branch is checked out in this worktree")
            .disabled()
    } else {
        entry
    })
}

/// The question `action` asks over the menu, built from the facts in
/// `ctx`: the branch prompt for a new worktree, the merge, remove, and
/// prune confirmations, and the close-down choice. `None` for a reveal,
/// which asks nothing, and for an action whose facts are missing.
pub fn step(action: WorktreeAction, ctx: &WorktreeMenuContext) -> Option<Modal> {
    let dialog = match action {
        WorktreeAction::NewWorktree => {
            return ctx
                .repo_root
                .clone()
                .map(|root| Modal::Prompt(branch_prompt(root)));
        }
        WorktreeAction::MergeWorktree => merge_dialog(ctx)?,
        WorktreeAction::RemoveWorktree => remove_dialog(ctx)?,
        WorktreeAction::CloseDownWorktree => close_down_dialog(ctx)?,
        WorktreeAction::PruneWorktrees => prune_dialog(ctx)?,
        WorktreeAction::RevealCheckout | WorktreeAction::RevealSessions => return None,
    };
    Some(Modal::Dialog(Box::new(dialog)))
}

/// The prompt that asks for a new worktree's branch in `repo_root`: a
/// blank name is refused with the reason, and a name commits the create.
fn branch_prompt(repo_root: String) -> TextPrompt<Msg> {
    TextPrompt::new(
        "New worktree — branch",
        "",
        move |text: &str| match text.trim() {
            "" => PromptResult::Reject("worktree: branch name is required".into()),
            branch => PromptResult::Commit(Msg::CommitWorktreeCreate {
                repo_root: repo_root.clone(),
                branch: branch.to_string(),
            }),
        },
    )
}

/// `dialog` with the live sessions in `ctx` listed under `heading`, which
/// says what the action does to them; nothing is added when there are
/// none.
fn with_sessions(dialog: Dialog<Msg>, heading: String, ctx: &WorktreeMenuContext) -> Dialog<Msg> {
    dialog.warning(heading, ctx.guard_sessions.iter().cloned())
}

/// The remove confirmation. Confirming past live sessions it lists is
/// the force `worktree rm` would otherwise ask for.
fn remove_dialog(ctx: &WorktreeMenuContext) -> Option<Dialog<Msg>> {
    let (repo_root, branch) = (ctx.repo_root.clone()?, ctx.branch.clone()?);
    let body = format!("Remove worktree for `{branch}`?");
    let remove = Msg::CommitWorktreeRemove {
        repo_root,
        branch,
        force: !ctx.guard_sessions.is_empty(),
    };
    let count = ctx.guard_sessions.len();
    let dialog = Dialog::confirm(TITLE, remove).body([body]);
    Some(with_sessions(
        dialog,
        format!("hosts {count} live session(s):"),
        ctx,
    ))
}

/// The merge-back confirmation. The menu merges into the repo's default
/// branch; an explicit target is a CLI concern.
fn merge_dialog(ctx: &WorktreeMenuContext) -> Option<Dialog<Msg>> {
    let worktree_root = ctx.repo_root.clone()?;
    let branch = ctx.branch.as_deref().unwrap_or(UNKNOWN_BRANCH);
    let merge = Msg::CommitWorktreeMerge {
        worktree_root,
        target: None,
    };
    let count = ctx.guard_sessions.len();
    let dialog = Dialog::confirm(TITLE, merge).body([
        format!("Merge `{branch}` back and remove its worktree?"),
        "(squash + rebase + fast-forward the default branch)".to_string(),
    ]);
    Some(with_sessions(
        dialog,
        format!("hosts {count} live session(s):"),
        ctx,
    ))
}

/// The close-down choice: `m` lands the branch, `d` discards it, and
/// either then tears the stream down (ADR 0093). `None` when close-down
/// does not apply to the selection: no mutation backend, nothing to
/// target, or no branch to close.
pub fn close_down_dialog(ctx: &WorktreeMenuContext) -> Option<Dialog<Msg>> {
    if !ctx.can_mutate {
        return None;
    }
    let (repo_root, branch) = (ctx.repo_root.clone()?, ctx.branch.clone()?);
    let close_down = |discard| Msg::CommitWorktreeCloseDown {
        repo_root: repo_root.clone(),
        branch: branch.clone(),
        discard,
    };
    let count = ctx.guard_sessions.len();
    let dialog = Dialog::choice(
        TITLE,
        vec![
            Verb::new(ch('m'), "merge & close", close_down(false)),
            Verb::new(ch('d'), "discard & close", close_down(true)),
        ],
    )
    .body([
        format!("Close down stream `{branch}`"),
        "Ends its sessions, removes the worktree, forgets its history records.".to_string(),
    ]);
    Some(with_sessions(
        dialog,
        format!("ends {count} live session(s) (transcripts preserved):"),
        ctx,
    ))
}

/// The prune confirmation for the repo at `repo_root`.
fn prune_dialog(ctx: &WorktreeMenuContext) -> Option<Dialog<Msg>> {
    let repo_root = ctx.repo_root.clone()?;
    Some(
        Dialog::confirm(TITLE, Msg::CommitWorktreePrune { repo_root }).body([
            "Prune merged worktrees?",
            "Removes every worktree already merged into the default branch \
             (worktrunk skips ones younger than its min-age).",
        ]),
    )
}

#[cfg(test)]
#[path = "worktree_menu_tests.rs"]
mod tests;
