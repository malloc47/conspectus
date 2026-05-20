//! Terminal lifecycle and event loop.
//!
//! `ratatui::init` already installs a panic hook that restores the
//! terminal, so this module just sets up the loop and is the only
//! place in the crate that touches stdout in raw mode.
//!
//! v1 discovery wiring (P8-008 minimal slice): the runtime runs
//! `discover_local_at_roots` synchronously at startup and on
//! manual `r` refresh, feeding the resulting snapshot + row tree
//! into the reducer via [`crate::tui::Msg::SetData`]. The
//! call blocks input briefly during discovery; the full
//! background-task transport lands with the rest of P8-008.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::discovery::discover_local_at_roots;
use crate::discovery::tmux::{SystemTmux, TmuxRunner};
use crate::model::{GraphSnapshot, MuxSessionId};
use crate::resolve::resolve_snapshot;
use crate::tui::actions::{AttachTarget, attach_disabled_reason, resolve_attach_target};
use crate::tui::app::{App, Msg};
use crate::tui::preview::capture_via;
use crate::tui::rows::RowTree;
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
use crate::tui::{RunConfig, View, ui};

/// Run the TUI to completion. Restores the terminal on normal exit,
/// errors, and panics (the panic path is covered by the hook
/// `ratatui::init` installs).
pub fn run(config: RunConfig) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, config);
    ratatui::restore();
    result
}

/// Block on terminal input, dispatching crossterm events to the
/// pure reducer until the app signals quit.
fn event_loop(terminal: &mut DefaultTerminal, config: RunConfig) -> Result<()> {
    let mut app = App::new(config.clone());
    let tmux: Box<dyn TmuxRunner> = Box::new(SystemTmux::new());

    // Initial synchronous discovery. Failure here surfaces as an
    // empty tree + an error frame; the operator can still press
    // `r` to retry once the underlying issue is fixed.
    refresh(&mut app, &config);
    refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), None);

    let poll_timeout = Duration::from_millis(100);
    while !app.should_quit() {
        terminal.draw(|frame| ui::draw(&app, frame))?;

        if event::poll(poll_timeout)? {
            let event = event::read()?;
            let viewport = terminal.size()?.height.saturating_sub(2);
            let prev_mux_target = current_mux_target(&app);
            let action = translate(event, viewport).map(|a| remap_for_focus(a, app.focus()));
            match action {
                Some(Action::Msg(msg)) => app.update(*msg),
                Some(Action::Refresh) => refresh(&mut app, &config),
                Some(Action::Attach) => attach_action(terminal, &mut app, &config),
                None => {}
            }
            refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), prev_mux_target);
        }
    }

    Ok(())
}

/// If the selection has moved to a new muxed target, capture its
/// pane and stash the result in the app. Skipped when
/// `live_preview_enabled` is false (privacy flag) or when the
/// target hasn't changed (avoids re-shelling on every keystroke).
fn refresh_mux_preview_if_needed(
    app: &mut App,
    config: &RunConfig,
    runner: &dyn TmuxRunner,
    prev: Option<MuxSessionId>,
) {
    if !config.live_preview_enabled {
        return;
    }
    let Some(target) = resolve_attach_target(app).ok() else {
        return;
    };
    if prev.as_ref() == Some(&target.mux) && app.mux_preview(&target.mux).is_some() {
        return;
    }
    // Capture against the **raw** backend-native session name (e.g.
    // `editor`), not the backend-prefixed graph id (`tmux:editor`)
    // — tmux itself doesn't understand the latter.
    let content = capture_via(runner, &target.native_id);
    app.update(Msg::SetMuxPreview {
        mux: target.mux,
        content,
    });
}

/// Resolve the selection's preferred mux target's graph id, if
/// any. Used as the "did the selection change" comparison key for
/// preview-capture orchestration. Pure: works against [`App`]
/// state, no I/O.
fn current_mux_target(app: &App) -> Option<MuxSessionId> {
    resolve_attach_target(app).ok().map(|t| t.mux)
}

/// Run discovery, build the row tree, and dispatch [`Msg::SetData`].
/// Errors leave the app's last good snapshot in place; once the
/// status-bar wiring lands the failure surfaces there too.
fn refresh(app: &mut App, config: &RunConfig) {
    match discover_and_build(config) {
        Ok((snapshot, tree)) => {
            let initial_selection_hint = launch_context_row_id(&tree);
            app.update(Msg::SetData {
                snapshot: Arc::new(snapshot),
                tree,
                loaded_at_epoch: current_unix_epoch().unwrap_or(0),
                initial_selection_hint,
            });
        }
        Err(_err) => {
            // No status bar surface yet (P8-007 follow-on); silently
            // retain the previous good state.
        }
    }
}

/// Find the `RowId` of the group row marked as the launch-context
/// (per [`crate::tui::rows::GroupRow::is_launch_context`]). Used as
/// the `initial_selection_hint` for [`Msg::SetData`] so the
/// operator's cwd-matching project is pre-selected on first load.
fn launch_context_row_id(tree: &RowTree) -> Option<crate::tui::rows::RowId> {
    use crate::tui::rows::RowKind;
    tree.rows.iter().find_map(|row| match &row.kind {
        RowKind::Group(g) if g.is_launch_context => Some(row.id.clone()),
        _ => None,
    })
}

fn discover_and_build(config: &RunConfig) -> Result<(GraphSnapshot, RowTree)> {
    let snapshot = if config.scan_roots.is_empty() {
        let cwd = std::env::current_dir()?;
        discover_local_at_roots([cwd])?
    } else {
        discover_local_at_roots(config.scan_roots.clone())?
    };
    let snapshot = resolve_snapshot(snapshot);

    let tree = build_tree_for_view(&snapshot, config);
    Ok((snapshot, tree))
}

fn build_tree_for_view(snapshot: &GraphSnapshot, config: &RunConfig) -> RowTree {
    let home = home_dir();
    match config.default_view {
        View::Sessions => build_sessions_tree(SessionsBuildInputs {
            snapshot,
            grouping: config.sessions_grouping,
            home: home.as_deref(),
            now: current_unix_epoch(),
            cwd: config.cwd.as_deref(),
        }),
        // Mux / union / prs / forks builders land in the remaining
        // P8-004 commits; until then those views show an empty
        // placeholder. The renderer already labels the active view
        // in the header so the operator sees what's loaded.
        View::Mux | View::Union | View::Prs | View::Forks => RowTree::default(),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn current_unix_epoch() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

/// The runtime's outer action: either a [`Msg`] for the pure
/// reducer or a side-effecting operation the reducer can't perform
/// (running discovery, exec'ing into a mux client). `Msg` is
/// boxed because `Msg::SetData` carries a `RowTree` that pushes
/// the enum past clippy's `large_enum_variant` threshold, even
/// though `Action::Msg` only ever carries the small navigation
/// variants in practice.
#[derive(Debug, Clone, PartialEq)]
enum Action {
    Msg(Box<Msg>),
    Refresh,
    Attach,
}

/// Handle the `a` key. On success, suspend the TUI, spawn
/// `tmux attach-session` and wait for it to exit, then re-enter
/// the alt screen so the operator returns to the TUI ready to
/// pick another row. On disabled, set a status-bar message and
/// stay in the TUI without touching the terminal.
fn attach_action(terminal: &mut DefaultTerminal, app: &mut App, config: &RunConfig) {
    match resolve_attach_target(app) {
        Ok(target) => {
            let outcome = run_tmux_attach(terminal, &target);
            // Always restore the row tree state — sessions may have
            // come and gone during the attach.
            refresh(app, config);
            // Surface a status line that reflects what happened so
            // the operator isn't guessing if anything ran.
            let message = match outcome {
                AttachOutcome::Detached => format!("attached/detached: {}", target_short(&target)),
                AttachOutcome::Failed(reason) => format!("attach failed: {reason}"),
            };
            app.update(Msg::SetStatus(Some(message)));
        }
        Err(reason) => {
            app.update(Msg::SetStatus(Some(attach_disabled_reason(&reason))));
        }
    }
}

/// Outcome of a single attach attempt. Errors carry a
/// human-readable reason for the status bar.
#[derive(Debug)]
enum AttachOutcome {
    /// `tmux attach-session` ran and exited (operator detached,
    /// pane was closed, tmux returned successfully, etc.).
    Detached,
    /// We never got to `tmux` cleanly, or `tmux` exited non-zero
    /// with a message worth surfacing.
    Failed(String),
}

/// Leave the alt screen + raw mode, spawn `tmux attach-session
/// -t <native_id>` inheriting the parent terminal, wait for it to
/// exit, then re-enter the alt screen. The caller's
/// `DefaultTerminal` is replaced in place so the resumed event
/// loop draws into the fresh terminal.
fn run_tmux_attach(terminal: &mut DefaultTerminal, target: &AttachTarget) -> AttachOutcome {
    // Suspend the ratatui terminal so tmux owns the real screen
    // for the duration of the attach.
    ratatui::restore();

    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", &target.native_id])
        .status();

    // Re-enter the alt screen + raw mode and swap the terminal in
    // place. Failure to re-init is fatal for the TUI, but we
    // attempted restore() first so the shell stays usable.
    *terminal = ratatui::init();
    // Forget any cached frame state — the parent screen was
    // overwritten by tmux, and a clean clear avoids ghost cells
    // from the suspended buffer.
    let _ = terminal.clear();

    match status {
        Ok(s) if s.success() => AttachOutcome::Detached,
        Ok(s) => AttachOutcome::Failed(format!("tmux exited with {s}")),
        Err(err) => AttachOutcome::Failed(format!("could not launch tmux: {err}")),
    }
}

fn target_short(target: &AttachTarget) -> String {
    format!("{}:{}", target.backend, target.native_id)
}

/// Remap navigation keys to preview scroll when focus is on the
/// right panel. Keeps the j/k muscle memory consistent — they
/// always drive the focused pane. Uppercase J/K continue to scroll
/// the preview regardless of focus, so operators with the left
/// panel focused can still poke the preview without switching
/// panes.
fn remap_for_focus(action: Action, focus: crate::tui::app::Focus) -> Action {
    use crate::tui::app::Focus;
    if focus != Focus::Right {
        return action;
    }
    match action {
        Action::Msg(boxed) => Action::Msg(Box::new(match *boxed {
            Msg::NavDown => Msg::ScrollPreviewBy(1),
            Msg::NavUp => Msg::ScrollPreviewBy(-1),
            Msg::PageDown(viewport) => Msg::ScrollPreviewBy(i32::from(viewport.max(1))),
            Msg::PageUp(viewport) => Msg::ScrollPreviewBy(-i32::from(viewport.max(1))),
            other => other,
        })),
        other => other,
    }
}

/// Map crossterm events to [`Action`]s. Returns `None` for events
/// the v1 shell ignores. Pulled out so tests don't need a terminal.
/// Focus-aware remapping (j/k driving the focused pane) happens in
/// a separate pass via [`remap_for_focus`] so the keymap stays
/// pure with respect to terminal state.
///
/// `viewport_height` is the rendered height of the row tree in
/// rows, used to size PageUp/PageDown jumps. Pass 1 if unknown.
fn translate(event: Event, viewport_height: u16) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Action::Msg(Box::new(Msg::Quit))),
            (_, KeyCode::Char('q')) => Some(Action::Msg(Box::new(Msg::Quit))),
            (m, KeyCode::Char('r')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::Refresh),
            (m, KeyCode::Char('a')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::Attach),
            (_, KeyCode::Char('j')) | (_, KeyCode::Down) => {
                Some(Action::Msg(Box::new(Msg::NavDown)))
            }
            (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Some(Action::Msg(Box::new(Msg::NavUp))),
            (_, KeyCode::PageDown) => Some(Action::Msg(Box::new(Msg::PageDown(viewport_height)))),
            (_, KeyCode::PageUp) => Some(Action::Msg(Box::new(Msg::PageUp(viewport_height)))),
            (_, KeyCode::Home) | (_, KeyCode::Char('g')) => Some(Action::Msg(Box::new(Msg::Home))),
            (_, KeyCode::End) | (_, KeyCode::Char('G')) => Some(Action::Msg(Box::new(Msg::End))),
            (_, KeyCode::Enter) => Some(Action::Msg(Box::new(Msg::ToggleExpand))),
            (_, KeyCode::Tab) => Some(Action::Msg(Box::new(Msg::CycleFocus))),
            (_, KeyCode::Char('J')) => Some(Action::Msg(Box::new(Msg::ScrollPreviewBy(1)))),
            (_, KeyCode::Char('K')) => Some(Action::Msg(Box::new(Msg::ScrollPreviewBy(-1)))),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn press(code: KeyCode, mods: KeyModifiers) -> Event {
        let mut key = KeyEvent::new(code, mods);
        key.kind = KeyEventKind::Press;
        Event::Key(key)
    }

    fn msg(action: Option<Action>) -> Option<Msg> {
        // Action::Msg now boxes its payload (large enum variant);
        // unwrap for the test assertions.
        match action {
            Some(Action::Msg(msg)) => Some(*msg),
            _ => None,
        }
    }

    #[test]
    fn translate_q_quits() {
        assert_eq!(
            msg(translate(press(KeyCode::Char('q'), KeyModifiers::NONE), 24)),
            Some(Msg::Quit)
        );
    }

    #[test]
    fn translate_ctrl_c_quits() {
        assert_eq!(
            msg(translate(
                press(KeyCode::Char('c'), KeyModifiers::CONTROL),
                24
            )),
            Some(Msg::Quit)
        );
    }

    #[test]
    fn translate_r_requests_refresh() {
        assert_eq!(
            translate(press(KeyCode::Char('r'), KeyModifiers::NONE), 24),
            Some(Action::Refresh)
        );
    }

    #[test]
    fn translate_a_requests_attach() {
        assert_eq!(
            translate(press(KeyCode::Char('a'), KeyModifiers::NONE), 24),
            Some(Action::Attach)
        );
    }

    #[test]
    fn translate_ignores_control_a_and_control_r() {
        assert_eq!(
            translate(press(KeyCode::Char('a'), KeyModifiers::CONTROL), 24),
            None
        );
        assert_eq!(
            translate(press(KeyCode::Char('r'), KeyModifiers::CONTROL), 24),
            None
        );
    }

    #[test]
    fn translate_maps_navigation_keys() {
        assert_eq!(
            msg(translate(press(KeyCode::Char('j'), KeyModifiers::NONE), 24)),
            Some(Msg::NavDown)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('k'), KeyModifiers::NONE), 24)),
            Some(Msg::NavUp)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Down, KeyModifiers::NONE), 24)),
            Some(Msg::NavDown)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Up, KeyModifiers::NONE), 24)),
            Some(Msg::NavUp)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Enter, KeyModifiers::NONE), 24)),
            Some(Msg::ToggleExpand)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Tab, KeyModifiers::NONE), 24)),
            Some(Msg::CycleFocus)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('g'), KeyModifiers::NONE), 24)),
            Some(Msg::Home)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('G'), KeyModifiers::NONE), 24)),
            Some(Msg::End)
        );
        assert_eq!(
            msg(translate(press(KeyCode::PageDown, KeyModifiers::NONE), 20)),
            Some(Msg::PageDown(20))
        );
        assert_eq!(
            msg(translate(press(KeyCode::PageUp, KeyModifiers::NONE), 20)),
            Some(Msg::PageUp(20))
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('J'), KeyModifiers::NONE), 24)),
            Some(Msg::ScrollPreviewBy(1))
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('K'), KeyModifiers::NONE), 24)),
            Some(Msg::ScrollPreviewBy(-1))
        );
    }

    #[test]
    fn remap_for_focus_left_is_identity() {
        use crate::tui::app::Focus;
        let action = Action::Msg(Box::new(Msg::NavDown));
        assert_eq!(remap_for_focus(action.clone(), Focus::Left), action);
        let action = Action::Msg(Box::new(Msg::PageDown(20)));
        assert_eq!(remap_for_focus(action.clone(), Focus::Left), action);
    }

    #[test]
    fn remap_for_focus_right_swaps_nav_for_preview_scroll() {
        use crate::tui::app::Focus;
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::NavDown)), Focus::Right),
            Action::Msg(Box::new(Msg::ScrollPreviewBy(1)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::NavUp)), Focus::Right),
            Action::Msg(Box::new(Msg::ScrollPreviewBy(-1)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageDown(20))), Focus::Right),
            Action::Msg(Box::new(Msg::ScrollPreviewBy(20)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageUp(20))), Focus::Right),
            Action::Msg(Box::new(Msg::ScrollPreviewBy(-20)))
        );
    }

    #[test]
    fn remap_for_focus_right_leaves_non_nav_actions_alone() {
        use crate::tui::app::Focus;
        // Tab / Enter / quit should not be remapped.
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
            Action::Msg(Box::new(Msg::CycleFocus))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::ToggleExpand)), Focus::Right),
            Action::Msg(Box::new(Msg::ToggleExpand))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::Quit)), Focus::Right),
            Action::Msg(Box::new(Msg::Quit))
        );
        assert_eq!(
            remap_for_focus(Action::Refresh, Focus::Right),
            Action::Refresh
        );
    }

    #[test]
    fn translate_ignores_unbound_keys() {
        assert_eq!(
            translate(press(KeyCode::Char('z'), KeyModifiers::NONE), 24),
            None
        );
        assert_eq!(
            translate(press(KeyCode::Char('a'), KeyModifiers::CONTROL), 24),
            None
        );
    }

    #[test]
    fn translate_ignores_release_kind_keys() {
        let mut key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert_eq!(translate(Event::Key(key), 24), None);
    }
}
