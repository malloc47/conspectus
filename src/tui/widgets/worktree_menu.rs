//! Worktree action menu overlay.
//!
//! The `w` menu: a self-contained (`Ctx = ()`) modal that captures the
//! selected node's worktree facts at open time and drives a small
//! state machine — a list of [`WorktreeAction`]s, then either a
//! branch-name text input (create) or a confirm (remove). It reuses
//! the shared [`TextInputState`] for the branch prompt and emits a
//! single committed [`Msg`] that the runtime turns into a mutation
//! effect.
//!
//! The wired subset (create / merge / remove / close-down) is offered
//! today; the rest of the [`worktree_actions`] policy (reveal, and the
//! later lock / prune stories) is filtered out here until each is
//! wired. Close-down (`X` or the menu entry) opens a merge/discard
//! choice that commits a compound teardown (ADR 0093).

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

use std::path::Path;

use crate::model::{GraphNode, GraphSnapshot, NodeId, RepoId, WorktreeKind, path_is_ancestor_of};
use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::rows::RowId;
use crate::tui::theme::Theme;
use crate::tui::widgets::input::{InputOutcome, TextInputState, TextInputWidget};
use crate::tui::widgets::popup_frame;
use crate::tui::worktree_actions::{WorktreeAction, WorktreeContext, worktree_actions};

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

/// Worktree facts captured when the menu opens, so the overlay stays
/// context-free (`Ctx = ()`) afterwards.
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
            let branch = snapshot.nodes.iter().find_map(|n| match n {
                GraphNode::Checkout(c) if &c.id == checkout_id => {
                    c.current_branch.as_ref().map(|b| short_branch(&b.refname))
                }
                _ => None,
            });
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
            let cwd = snapshot.nodes.iter().find_map(|n| match n {
                GraphNode::MuxSession(m) if &NodeId::MuxSession(m.id.clone()) == node => {
                    m.active_pane_current_path.clone().or_else(|| m.cwd.clone())
                }
                _ => None,
            });
            let checkout = cwd
                .as_deref()
                .and_then(|c| checkout_containing(snapshot, c));
            WorktreeMenuContext {
                context: WorktreeContext::Mux,
                repo_root: checkout.as_ref().map(|(root, _)| root.clone()),
                branch: checkout.and_then(|(_, b)| b),
                guard_sessions: cwd
                    .as_deref()
                    .map(|c| live_sessions_in_worktree(snapshot, c))
                    .unwrap_or_default(),
                can_mutate,
                // Reveal checkout: jump to the containing checkout row.
                reveal_target: cwd
                    .as_deref()
                    .and_then(|c| checkout_row_containing(snapshot, c)),
            }
        }
        NodeId::AgentSession(_) => {
            let cwd = snapshot.nodes.iter().find_map(|n| match n {
                GraphNode::AgentSession(s) if &NodeId::AgentSession(s.id.clone()) == node => {
                    s.cwd.clone()
                }
                _ => None,
            });
            WorktreeMenuContext {
                context: WorktreeContext::Agent,
                repo_root: None,
                branch: None,
                guard_sessions: Vec::new(),
                can_mutate,
                reveal_target: cwd
                    .as_deref()
                    .and_then(|c| checkout_row_containing(snapshot, c)),
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

/// The `Group` row of the worktree checkout containing `path` — the
/// reveal-checkout jump target.
fn checkout_row_containing(snapshot: &GraphSnapshot, path: &str) -> Option<RowId> {
    let p = Path::new(path);
    snapshot.nodes.iter().find_map(|n| match n {
        GraphNode::Checkout(c)
            if c.worktree.is_some() && path_is_ancestor_of(Path::new(&c.root), p) =>
        {
            Some(RowId::Group(NodeId::Checkout(c.id.clone())))
        }
        _ => None,
    })
}

/// The first live session rooted in `worktree_path` — the
/// reveal-sessions jump target. Prefers a mux session over an agent so
/// the operator lands on the attachable row.
fn first_session_row_in(snapshot: &GraphSnapshot, worktree_path: &str) -> Option<RowId> {
    let root = Path::new(worktree_path);
    let mux = snapshot.nodes.iter().find_map(|n| match n {
        GraphNode::MuxSession(m) => {
            let cwd = m.active_pane_current_path.as_deref().or(m.cwd.as_deref());
            cwd.filter(|c| path_is_ancestor_of(root, Path::new(c)))
                .map(|_| RowId::MuxSession(NodeId::MuxSession(m.id.clone())))
        }
        _ => None,
    });
    mux.or_else(|| {
        snapshot.nodes.iter().find_map(|n| match n {
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
    for node in &snapshot.nodes {
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

/// The worktree checkout containing `path` and its short branch.
fn checkout_containing(snapshot: &GraphSnapshot, path: &str) -> Option<(String, Option<String>)> {
    let p = Path::new(path);
    snapshot.nodes.iter().find_map(|n| match n {
        GraphNode::Checkout(c)
            if c.worktree.is_some() && path_is_ancestor_of(Path::new(&c.root), p) =>
        {
            Some((
                c.root.clone(),
                c.current_branch.as_ref().map(|b| short_branch(&b.refname)),
            ))
        }
        _ => None,
    })
}

/// Live agent/mux sessions rooted in `worktree_path` (the
/// `worktree rm` guard, reused for the remove confirm).
fn live_sessions_in_worktree(snapshot: &GraphSnapshot, worktree_path: &str) -> Vec<String> {
    let root = Path::new(worktree_path);
    let mut out = Vec::new();
    for node in &snapshot.nodes {
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

#[derive(Debug, Clone)]
enum Mode {
    List,
    BranchInput(TextInputState),
    ConfirmRemove,
    ConfirmMerge,
    /// Close-down: pick merge (land) vs discard, then commit.
    CloseDownChoice,
    ConfirmPrune,
}

/// Overlay state: the captured context, the offered actions, a cursor,
/// and the current sub-mode.
#[derive(Debug, Clone)]
pub struct WorktreeMenuState {
    ctx: WorktreeMenuContext,
    actions: Vec<WorktreeAction>,
    cursor: usize,
    mode: Mode,
}

impl WorktreeMenuState {
    /// Build the menu for a selection. Offers the wired subset of the
    /// policy, and drops mutation actions when there's no `repo_root`
    /// to target. Returns `None` when nothing is offerable, so the
    /// runtime can post a status instead of opening an empty modal.
    pub fn new(ctx: WorktreeMenuContext) -> Option<Self> {
        let has_repo = ctx.repo_root.is_some();
        let has_reveal = ctx.reveal_target.is_some();
        let actions: Vec<WorktreeAction> = worktree_actions(ctx.context, ctx.can_mutate)
            .into_iter()
            .filter(|a| WIRED.contains(a))
            .filter(|a| !a.is_mutation() || has_repo)
            // Drop reveal actions with no resolved jump target.
            .filter(|a| !is_reveal(*a) || has_reveal)
            .collect();
        if actions.is_empty() {
            return None;
        }
        Some(Self {
            ctx,
            actions,
            cursor: 0,
            mode: Mode::List,
        })
    }

    /// Open the menu straight into the close-down merge/discard choice
    /// (the `X` hot key), skipping the action list. Returns `None` when
    /// close-down isn't applicable to the selection (no worktree branch
    /// to close, or no mutation backend).
    pub fn new_close_down(ctx: WorktreeMenuContext) -> Option<Self> {
        if !ctx.can_mutate || ctx.repo_root.is_none() || ctx.branch.is_none() {
            return None;
        }
        Some(Self {
            actions: vec![WorktreeAction::CloseDownWorktree],
            cursor: 0,
            mode: Mode::CloseDownChoice,
            ctx,
        })
    }

    pub fn context(&self) -> &WorktreeMenuContext {
        &self.ctx
    }

    fn selected(&self) -> WorktreeAction {
        self.actions[self.cursor.min(self.actions.len() - 1)]
    }

    fn activate(&mut self) -> OverlayOutcome {
        match self.selected() {
            WorktreeAction::NewWorktree => {
                self.mode = Mode::BranchInput(TextInputState::new("New worktree — branch", ""));
                OverlayOutcome::Consumed
            }
            WorktreeAction::MergeWorktree => {
                self.mode = Mode::ConfirmMerge;
                OverlayOutcome::Consumed
            }
            WorktreeAction::RemoveWorktree => {
                self.mode = Mode::ConfirmRemove;
                OverlayOutcome::Consumed
            }
            WorktreeAction::CloseDownWorktree => {
                self.mode = Mode::CloseDownChoice;
                OverlayOutcome::Consumed
            }
            WorktreeAction::PruneWorktrees => {
                self.mode = Mode::ConfirmPrune;
                OverlayOutcome::Consumed
            }
            WorktreeAction::RevealCheckout | WorktreeAction::RevealSessions => {
                match self.ctx.reveal_target.clone() {
                    Some(row) => OverlayOutcome::Commit(Box::new(Msg::SelectRow(Box::new(row)))),
                    None => OverlayOutcome::Close,
                }
            }
        }
    }

    fn handle_list(&mut self, key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Esc => OverlayOutcome::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                OverlayOutcome::Consumed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(self.actions.len().saturating_sub(1));
                OverlayOutcome::Consumed
            }
            KeyCode::Enter => self.activate(),
            _ => OverlayOutcome::Consumed,
        }
    }

    fn handle_branch_input(&mut self, key: KeyEvent) -> OverlayOutcome {
        let Mode::BranchInput(input) = &mut self.mode else {
            return OverlayOutcome::Consumed;
        };
        match input.handle_key(key) {
            InputOutcome::Continue => OverlayOutcome::Consumed,
            InputOutcome::Cancel => {
                self.mode = Mode::List;
                OverlayOutcome::Consumed
            }
            InputOutcome::Confirm(branch) => {
                let branch = branch.trim().to_string();
                if branch.is_empty() {
                    // Keep the input open for a real value.
                    return OverlayOutcome::Consumed;
                }
                let Some(repo_root) = self.ctx.repo_root.clone() else {
                    return OverlayOutcome::Close;
                };
                OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeCreate { repo_root, branch }))
            }
        }
    }

    fn handle_confirm_remove(&mut self, key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                let (Some(repo_root), Some(branch)) =
                    (self.ctx.repo_root.clone(), self.ctx.branch.clone())
                else {
                    return OverlayOutcome::Close;
                };
                OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeRemove {
                    repo_root,
                    branch,
                    // Confirming past the surfaced guard list is the force.
                    force: !self.ctx.guard_sessions.is_empty(),
                }))
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                self.mode = Mode::List;
                OverlayOutcome::Consumed
            }
            _ => OverlayOutcome::Consumed,
        }
    }

    fn handle_confirm_merge(&mut self, key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                let Some(worktree_root) = self.ctx.repo_root.clone() else {
                    return OverlayOutcome::Close;
                };
                // Menu merges into the repo's default branch; explicit
                // targets are a CLI concern.
                OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeMerge {
                    worktree_root,
                    target: None,
                }))
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                self.mode = Mode::List;
                OverlayOutcome::Consumed
            }
            _ => OverlayOutcome::Consumed,
        }
    }

    fn handle_confirm_prune(&mut self, key: KeyEvent) -> OverlayOutcome {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                let Some(repo_root) = self.ctx.repo_root.clone() else {
                    return OverlayOutcome::Close;
                };
                OverlayOutcome::Commit(Box::new(Msg::CommitWorktreePrune { repo_root }))
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                self.mode = Mode::List;
                OverlayOutcome::Consumed
            }
            _ => OverlayOutcome::Consumed,
        }
    }

    fn handle_close_down(&mut self, key: KeyEvent) -> OverlayOutcome {
        // `m` lands the branch, `d` discards it; both then tear the
        // stream down (ADR 0093). Esc/q backs out.
        let discard = match key.code {
            KeyCode::Char('m') => false,
            KeyCode::Char('d') => true,
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::List;
                return OverlayOutcome::Consumed;
            }
            _ => return OverlayOutcome::Consumed,
        };
        let (Some(repo_root), Some(branch)) = (self.ctx.repo_root.clone(), self.ctx.branch.clone())
        else {
            return OverlayOutcome::Close;
        };
        OverlayOutcome::Commit(Box::new(Msg::CommitWorktreeCloseDown {
            repo_root,
            branch,
            discard,
        }))
    }
}

impl Overlay for WorktreeMenuState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        match self.mode {
            Mode::List => self.handle_list(key),
            Mode::BranchInput(_) => self.handle_branch_input(key),
            Mode::ConfirmRemove => self.handle_confirm_remove(key),
            Mode::ConfirmMerge => self.handle_confirm_merge(key),
            Mode::CloseDownChoice => self.handle_close_down(key),
            Mode::ConfirmPrune => self.handle_confirm_prune(key),
        }
    }
}

/// Renders the worktree menu (list / branch input / confirm) in a
/// centered popup.
pub struct WorktreeMenuWidget<'a> {
    state: &'a WorktreeMenuState,
    theme: &'a Theme,
}

impl<'a> WorktreeMenuWidget<'a> {
    pub fn new(state: &'a WorktreeMenuState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for WorktreeMenuWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Branch input renders through the shared text-input widget.
        if let Mode::BranchInput(input) = &self.state.mode {
            TextInputWidget::new(input)
                .theme(self.theme)
                .render(area, buf);
            return;
        }

        let lines: Vec<Line> = match &self.state.mode {
            Mode::List => self
                .state
                .actions
                .iter()
                .enumerate()
                .map(|(i, action)| {
                    let marker = if i == self.state.cursor { "▸ " } else { "  " };
                    let style = if i == self.state.cursor {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::styled(format!("{marker}{}", action.label()), style)
                })
                .collect(),
            Mode::ConfirmRemove => {
                let branch = self.state.ctx.branch.as_deref().unwrap_or("(unknown)");
                let mut lines = vec![
                    Line::from(format!("Remove worktree for `{branch}`?")),
                    Line::from(""),
                ];
                if !self.state.ctx.guard_sessions.is_empty() {
                    lines.push(Line::styled(
                        format!(
                            "⚠ hosts {} live session(s):",
                            self.state.ctx.guard_sessions.len()
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for s in &self.state.ctx.guard_sessions {
                        lines.push(Line::from(format!("  - {s}")));
                    }
                    lines.push(Line::from(""));
                }
                lines.push(Line::from("Enter/y to confirm · Esc/n to cancel"));
                lines
            }
            Mode::ConfirmMerge => {
                let branch = self.state.ctx.branch.as_deref().unwrap_or("(unknown)");
                let mut lines = vec![
                    Line::from(format!("Merge `{branch}` back and remove its worktree?")),
                    Line::from("(squash + rebase + fast-forward the default branch)"),
                    Line::from(""),
                ];
                if !self.state.ctx.guard_sessions.is_empty() {
                    lines.push(Line::styled(
                        format!(
                            "⚠ hosts {} live session(s):",
                            self.state.ctx.guard_sessions.len()
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for s in &self.state.ctx.guard_sessions {
                        lines.push(Line::from(format!("  - {s}")));
                    }
                    lines.push(Line::from(""));
                }
                lines.push(Line::from("Enter/y to confirm · Esc/n to cancel"));
                lines
            }
            Mode::CloseDownChoice => {
                let branch = self.state.ctx.branch.as_deref().unwrap_or("(unknown)");
                let mut lines = vec![
                    Line::styled(
                        format!("Close down stream `{branch}`"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Line::from("Ends its sessions, removes the worktree, drops its pins."),
                    Line::from(""),
                ];
                if !self.state.ctx.guard_sessions.is_empty() {
                    lines.push(Line::styled(
                        format!(
                            "⚠ ends {} live session(s) (transcripts preserved):",
                            self.state.ctx.guard_sessions.len()
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for s in &self.state.ctx.guard_sessions {
                        lines.push(Line::from(format!("  - {s}")));
                    }
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(
                    "m = merge & close · d = discard & close · Esc cancel",
                ));
                lines
            }
            Mode::ConfirmPrune => {
                vec![
                    Line::styled(
                        "Prune merged worktrees?",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Line::from("Removes every worktree already merged into the default"),
                    Line::from("branch (worktrunk skips ones younger than its min-age)."),
                    Line::from(""),
                    Line::from("Enter/y to confirm · Esc/n to cancel"),
                ]
            }
            Mode::BranchInput(_) => unreachable!("handled above"),
        };

        let height = (lines.len() as u16).saturating_add(2).clamp(5, area.height);
        let rect = popup_frame::centered_rect(area, 56.min(area.width), height);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Worktree ")
            .border_style(Style::default().fg(self.theme.panel_focus_accent));
        let inner = block.inner(rect);
        block.render(rect, buf);
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }
}

#[cfg(test)]
#[path = "worktree_menu_tests.rs"]
mod tests;
