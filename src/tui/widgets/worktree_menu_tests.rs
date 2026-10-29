use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, MuxSessionId, MuxSessionNode,
    RepoId, WorktreeMeta,
};
use crate::tui::{App, RunConfig};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::glyphs::marks;
use tuikit::testkit::keys::press;

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

fn repo_ctx() -> WorktreeMenuContext {
    WorktreeMenuContext {
        context: WorktreeContext::Repo,
        repo_root: Some("/app".to_string()),
        branch: None,
        guard_sessions: vec![],
        can_mutate: true,
        reveal_target: None,
    }
}

/// What each entry of a menu would do when picked: the action whose step
/// it opens, or the message it commits.
fn actions(menu: &ActionMenu<Msg>) -> Vec<WorktreeAction> {
    menu.entries()
        .iter()
        .map(|entry| match &entry.outcome {
            Msg::OpenWorktreeStep { action, .. } => *action,
            Msg::SelectRow(_) => WorktreeAction::RevealSessions,
            other => panic!("an entry commits {other:?}"),
        })
        .collect()
}

/// An App with the worktree menu for `ctx` open.
fn opened(ctx: &WorktreeMenuContext) -> App {
    let mut app = App::new(RunConfig::defaults());
    app.open_worktree_menu(worktree_menu(ctx).expect("actions"));
    app
}

/// One key the way the loops route it through the stack: a step a pick
/// asks for opens over the menu, and anything else the menu or a step
/// sends is returned.
fn key(app: &mut App, event: KeyEvent) -> Vec<Msg> {
    let mut sent = Vec::new();
    crate::tui::runtime::modal_key(app, event, &mut |app, msg| match msg {
        Msg::OpenWorktreeStep { .. } => {
            app.update(msg);
        }
        other => sent.push(other),
    });
    sent
}

fn keys(app: &mut App, events: &[KeyEvent]) -> Vec<Msg> {
    events.iter().flat_map(|event| key(app, *event)).collect()
}

fn down() -> KeyEvent {
    press(KeyCode::Down)
}

fn enter() -> KeyEvent {
    press(KeyCode::Enter)
}

fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

/// The open modals, bottom to top, by name.
fn stack(app: &App) -> Vec<&'static str> {
    app.modal_stack().iter().map(Modal::name).collect()
}

/// The stack drawn on an 80 by 24 screen.
fn drawn(app: &mut App) -> String {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.draw_modals(area, &mut buf);
    tuikit::testkit::buffer::text(&buf)
}

// ---- what the menu offers ---------------------------------------------------

#[test]
fn the_menu_offers_the_wired_mutations_for_a_worktree() {
    let menu = worktree_menu(&worktree_ctx(vec![], true)).expect("actions");
    assert_eq!(
        actions(&menu),
        vec![
            WorktreeAction::NewWorktree,
            WorktreeAction::MergeWorktree,
            WorktreeAction::RemoveWorktree,
            WorktreeAction::CloseDownWorktree,
        ],
    );
    assert_eq!(menu.title(), "Worktree");
    assert!(
        menu.entries().iter().all(|entry| entry.opens_step),
        "every mutation asks before it acts"
    );
}

#[test]
fn the_menu_is_none_for_a_read_only_worktree() {
    // Read-only: mutations gated, and no reveal target to offer.
    assert!(worktree_menu(&worktree_ctx(vec![], false)).is_none());
}

#[test]
fn a_verb_on_the_branch_cannot_run_without_one() {
    let mut detached = worktree_ctx(vec![], true);
    detached.branch = None;
    let menu = worktree_menu(&detached).expect("actions");
    let enabled: Vec<bool> = menu.entries().iter().map(|entry| entry.enabled).collect();
    // New and Merge need only the worktree; Remove and Close down need
    // its branch, and say so.
    assert_eq!(enabled, [true, true, false, false]);
    assert!(
        menu.entries()[2]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("no branch"))
    );
    assert!(close_down_dialog(&detached).is_none());
}

// ---- create ------------------------------------------------------------------

#[test]
fn the_create_flow_asks_for_a_branch_over_the_menu_and_commits_it() {
    let mut app = opened(&worktree_ctx(vec![], true));
    // The cursor starts on New worktree; Enter opens its prompt as a step.
    assert!(key(&mut app, enter()).is_empty());
    assert_eq!(stack(&app), ["WorktreeMenu", "Prompt"]);
    assert!(app.text_field_has_keys(), "the prompt has the keys");
    for c in "wip".chars() {
        assert!(key(&mut app, press(KeyCode::Char(c))).is_empty());
    }
    assert_eq!(
        key(&mut app, enter()),
        [Msg::CommitWorktreeCreate {
            repo_root: "/wt/feature".to_string(),
            branch: "wip".to_string(),
        }],
    );
    assert!(!app.has_modal(), "a commit closes the prompt and the menu");
}

#[test]
fn a_blank_branch_is_refused_in_the_prompt_until_a_name_is_typed() {
    let mut app = opened(&worktree_ctx(vec![], true));
    key(&mut app, enter());
    keys(
        &mut app,
        &[press(KeyCode::Char(' ')), press(KeyCode::Char(' '))],
    );
    // Refused without a message to the host: the prompt says why itself.
    assert!(key(&mut app, enter()).is_empty());
    assert_eq!(
        app.top_modal_name(),
        Some("Prompt"),
        "the prompt stays open"
    );
    let frame = drawn(&mut app);
    assert!(
        frame.contains("worktree: branch name is required"),
        "the error line says why:\n{frame}"
    );
    keys(
        &mut app,
        &[press(KeyCode::Char('w')), press(KeyCode::Char('i'))],
    );
    let frame = drawn(&mut app);
    assert!(!frame.contains("required"), "typing clears it:\n{frame}");
    assert_eq!(
        key(&mut app, enter()),
        [Msg::CommitWorktreeCreate {
            repo_root: "/wt/feature".to_string(),
            branch: "wi".to_string(),
        }],
        "the name is trimmed"
    );
}

// ---- merge, remove, prune ----------------------------------------------------

#[test]
fn the_merge_flow_confirms_and_merges_into_the_default_branch() {
    let mut app = opened(&worktree_ctx(vec!["codex (agent)".to_string()], true));
    keys(&mut app, &[down(), enter()]);
    assert_eq!(stack(&app), ["WorktreeMenu", "Dialog"]);
    let frame = drawn(&mut app);
    assert!(
        frame.contains("Merge `feature` back and remove its worktree?"),
        "{frame}"
    );
    assert!(frame.contains("hosts 1 live session(s):"), "{frame}");
    assert!(frame.contains("  - codex (agent)"), "{frame}");
    assert_eq!(
        key(&mut app, press(KeyCode::Char('y'))),
        [Msg::CommitWorktreeMerge {
            worktree_root: "/wt/feature".to_string(),
            target: None,
        }],
    );
    assert!(!app.has_modal());
}

#[test]
fn the_remove_flow_forces_past_the_live_sessions_it_lists() {
    let mut app = opened(&worktree_ctx(vec!["codex (agent)".to_string()], true));
    keys(&mut app, &[down(), down(), enter()]);
    assert_eq!(stack(&app), ["WorktreeMenu", "Dialog"]);
    let frame = drawn(&mut app);
    assert!(frame.contains("Remove worktree for `feature`?"), "{frame}");
    assert!(frame.contains("Enter/y confirm · Esc/n cancel"), "{frame}");
    // Confirming past the surfaced guard list is the force.
    assert_eq!(
        key(&mut app, press(KeyCode::Char('y'))),
        [Msg::CommitWorktreeRemove {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            force: true,
        }],
    );
}

#[test]
fn a_remove_with_no_live_sessions_does_not_force_and_lists_nothing() {
    let mut app = opened(&worktree_ctx(vec![], true));
    keys(&mut app, &[down(), down(), enter()]);
    let frame = drawn(&mut app);
    assert!(!frame.contains(marks::WARNING), "no list to show:\n{frame}");
    assert_eq!(
        key(&mut app, enter()),
        [Msg::CommitWorktreeRemove {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            force: false,
        }],
    );
}

#[test]
fn the_prune_flow_confirms_for_the_repo() {
    let mut app = opened(&repo_ctx());
    assert_eq!(
        actions(app.worktree_menu().expect("the menu")),
        [WorktreeAction::NewWorktree, WorktreeAction::PruneWorktrees],
    );
    keys(&mut app, &[down(), enter()]);
    assert_eq!(stack(&app), ["WorktreeMenu", "Dialog"]);
    assert!(drawn(&mut app).contains("Prune merged worktrees?"));
    assert_eq!(
        key(&mut app, press(KeyCode::Char('y'))),
        [Msg::CommitWorktreePrune {
            repo_root: "/app".to_string(),
        }],
    );
}

// ---- close down --------------------------------------------------------------

#[test]
fn the_close_down_choice_merges_or_discards() {
    for (choice, discard) in [('m', false), ('d', true)] {
        let mut app = opened(&worktree_ctx(vec![], true));
        keys(&mut app, &[down(), down(), down(), enter()]);
        assert_eq!(stack(&app), ["WorktreeMenu", "Dialog"]);
        let frame = drawn(&mut app);
        assert!(
            frame.contains("m merge & close · d discard & close · Esc/n cancel"),
            "{frame}"
        );
        // A choice has no default: Enter and y do nothing.
        assert!(keys(&mut app, &[enter(), press(KeyCode::Char('y'))]).is_empty());
        assert_eq!(app.top_modal_name(), Some("Dialog"));
        assert_eq!(
            key(&mut app, press(KeyCode::Char(choice))),
            [Msg::CommitWorktreeCloseDown {
                repo_root: "/wt/feature".to_string(),
                branch: "feature".to_string(),
                discard,
            }],
        );
        assert!(!app.has_modal());
    }
}

#[test]
fn the_close_down_hotkey_opens_the_choice_on_its_own() {
    // `X` opens the choice with no menu under it, so Esc closes it.
    let choice = close_down_dialog(&worktree_ctx(vec!["codex (agent)".to_string()], true))
        .expect("close-down applies");
    let mut app = App::new(RunConfig::defaults());
    app.open_modal(Modal::Dialog(Box::new(choice.clone())));
    assert!(key(&mut app, press(KeyCode::Esc)).is_empty());
    assert!(!app.has_modal(), "nothing to step back to");
    app.open_modal(Modal::Dialog(Box::new(choice)));
    let frame = drawn(&mut app);
    assert!(
        frame.contains("ends 1 live session(s) (transcripts preserved):"),
        "{frame}"
    );
    assert_eq!(
        key(&mut app, press(KeyCode::Char('d'))),
        [Msg::CommitWorktreeCloseDown {
            repo_root: "/wt/feature".to_string(),
            branch: "feature".to_string(),
            discard: true,
        }],
    );
}

#[test]
fn the_close_down_hotkey_does_not_apply_without_a_branch_or_backend() {
    let mut no_branch = worktree_ctx(vec![], true);
    no_branch.branch = None;
    assert!(close_down_dialog(&no_branch).is_none());
    assert!(close_down_dialog(&worktree_ctx(vec![], false)).is_none());
    assert!(
        close_down_dialog(&repo_ctx()).is_none(),
        "a repo has no branch"
    );
}

// ---- stepping back and dismissing ---------------------------------------------

#[test]
fn esc_steps_back_to_the_menu_and_ctrl_c_and_q_dismiss_the_whole_flow() {
    // House rule 4: Esc steps back to the list, which keeps its cursor;
    // Ctrl-C, and q where nothing is typed, close the step and the menu.
    // The steps behind each entry: the branch prompt (typed), the merge
    // and remove confirmations, and the close-down choice.
    for (downs, typed) in [(0, true), (1, false), (2, false), (3, false)] {
        let open = || {
            let mut app = opened(&worktree_ctx(vec![], true));
            for _ in 0..downs {
                key(&mut app, down());
            }
            key(&mut app, enter());
            assert_eq!(app.modal_stack().len(), 2, "step {downs} is open");
            app
        };
        let mut app = open();
        assert!(key(&mut app, ctrl_c()).is_empty());
        assert!(!app.has_modal(), "Ctrl-C on step {downs}");

        let mut app = open();
        key(&mut app, press(KeyCode::Char('q')));
        let left = if typed { 2 } else { 0 };
        assert_eq!(app.modal_stack().len(), left, "q on step {downs}");

        let mut app = open();
        assert!(key(&mut app, press(KeyCode::Esc)).is_empty());
        assert_eq!(stack(&app), ["WorktreeMenu"], "Esc steps back from {downs}");
        let menu = app.worktree_menu().expect("the menu has the keys");
        assert_eq!(menu.cursor(), downs, "the cursor is where it was");
        key(&mut app, press(KeyCode::Char('q')));
        assert!(!app.has_modal(), "q closes the menu");
    }
}

#[test]
fn n_steps_back_from_every_dialog() {
    for downs in [1, 2, 3] {
        let mut app = opened(&worktree_ctx(vec![], true));
        for _ in 0..downs {
            key(&mut app, down());
        }
        key(&mut app, enter());
        assert!(key(&mut app, press(KeyCode::Char('n'))).is_empty());
        assert_eq!(stack(&app), ["WorktreeMenu"], "n on step {downs}");
    }
}

// ---- the list ----------------------------------------------------------------

#[test]
fn the_action_list_wraps_at_both_ends() {
    let mut app = opened(&worktree_ctx(vec![], true));
    key(&mut app, press(KeyCode::Up));
    assert_eq!(
        app.worktree_menu().map(ActionMenu::cursor),
        Some(3),
        "up from the first"
    );
    key(&mut app, down());
    assert_eq!(
        app.worktree_menu().map(ActionMenu::cursor),
        Some(0),
        "down from the last"
    );
}

#[test]
fn the_cursor_action_shows_the_marker_and_the_highlight() {
    let theme = crate::tui::theme::Theme::default();
    let mut app = opened(&worktree_ctx(vec![], true));
    key(&mut app, down());
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    app.draw_modals(area, &mut buf);
    let marked: Vec<(u16, u16)> = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|(x, y)| buf[(*x, *y)].symbol() == marks::POINTER)
        .collect();
    assert_eq!(marked.len(), 1, "one marked row");
    let (x, y) = marked[0];
    let row: String = (x..area.width).map(|x| buf[(x, y)].symbol()).collect();
    assert!(row.contains(WorktreeAction::MergeWorktree.label()), "{row}");
    assert!(buf[(x + 2, y)].modifier.contains(theme.selection_active));
    assert!(
        !buf[(x + 2, y + 1)]
            .modifier
            .contains(theme.selection_active)
    );
}

#[test]
fn a_step_shows_over_the_menu_it_was_asked_from() {
    let mut app = opened(&worktree_ctx(vec![], true));
    key(&mut app, enter());
    let frame = drawn(&mut app);
    assert!(
        frame.contains("New worktree — branch"),
        "the prompt:\n{frame}"
    );
    assert!(
        frame.contains("↑/↓ move · Enter pick · Esc close"),
        "the menu, around the prompt:\n{frame}"
    );
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
    let target = RowId::MuxSession(NodeId::MuxSession(MuxSessionId::new("tmux:feat")));
    assert_eq!(ctx.reveal_target, Some(target.clone()));
    // With a target, the read-only reveal action is offered even on a
    // read-only host.
    let menu = worktree_menu(&WorktreeMenuContext {
        can_mutate: false,
        ..ctx
    })
    .expect("reveal offered");
    assert_eq!(
        menu.entries()
            .iter()
            .map(|entry| entry.outcome.clone())
            .collect::<Vec<_>>(),
        [Msg::SelectRow(Box::new(target))],
    );
}

#[test]
fn the_reveal_action_commits_a_selection_jump() {
    let mut ctx = worktree_ctx(vec![], true);
    let target = RowId::MuxSession(NodeId::MuxSession(MuxSessionId::new("tmux:feat")));
    ctx.reveal_target = Some(target.clone());
    let mut app = opened(&ctx);
    // RevealSessions is the last offered action for a worktree.
    key(&mut app, press(KeyCode::Up));
    assert_eq!(key(&mut app, enter()), [Msg::SelectRow(Box::new(target))],);
    assert!(!app.has_modal(), "a jump asks nothing and closes the menu");
}

#[test]
fn a_reveal_is_dropped_without_a_target() {
    // A worktree with no live sessions has no reveal target, so the
    // reveal action isn't offered.
    let menu = worktree_menu(&worktree_ctx(vec![], true)).expect("actions");
    assert!(
        !menu
            .entries()
            .iter()
            .any(|entry| matches!(entry.outcome, Msg::SelectRow(_)))
    );
}

#[test]
fn containing_checkout_is_the_innermost_worktree() {
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/outer",
        "refs/heads/outer",
    ));
    snap.nodes.push(linked_checkout(
        "/app/.git",
        "/wt/outer/inner",
        "refs/heads/inner",
    ));
    snap.canonicalize();

    let inner = worktree_checkout_containing(&snap, "/wt/outer/inner/src")
        .expect("a checkout contains the path");
    assert_eq!(inner.root, "/wt/outer/inner");
    assert_eq!(checkout_short_branch(inner).as_deref(), Some("inner"));

    let outer = worktree_checkout_containing(&snap, "/wt/outer/docs").expect("outer contains it");
    assert_eq!(outer.root, "/wt/outer");
    assert!(worktree_checkout_containing(&snap, "/elsewhere").is_none());
}

#[test]
fn every_choice_verb_is_reachable() {
    let choice = close_down_dialog(&worktree_ctx(vec![], true)).expect("applies");
    assert!(choice.shadowed_verbs().is_empty());
}
