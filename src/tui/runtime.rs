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
            match translate(event, viewport) {
                Some(Action::Msg(msg)) => app.update(msg),
                Some(Action::Refresh) => refresh(&mut app, &config),
                Some(Action::Attach) => attach_action(&mut app),
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
    let Some(current) = current_mux_target(app) else {
        return;
    };
    if prev.as_ref() == Some(&current) && app.mux_preview(&current).is_some() {
        return;
    }
    let content = capture_via(runner, &current.native_id);
    app.update(Msg::SetMuxPreview {
        mux: current,
        content,
    });
}

/// Resolve the selection's preferred mux target, if any. Reuses
/// the attach resolver so the row glyph, the attach action, and
/// the preview source agree on what "this row's mux" is.
fn current_mux_target(app: &App) -> Option<MuxSessionId> {
    resolve_attach_target(app).ok().map(|t| t.mux)
}

/// Run discovery, build the row tree, and dispatch [`Msg::SetData`].
/// Errors leave the app's last good snapshot in place; once the
/// status-bar wiring lands the failure surfaces there too.
fn refresh(app: &mut App, config: &RunConfig) {
    match discover_and_build(config) {
        Ok((snapshot, tree)) => {
            app.update(Msg::SetData {
                snapshot: Arc::new(snapshot),
                tree,
                loaded_at_epoch: current_unix_epoch().unwrap_or(0),
            });
        }
        Err(_err) => {
            // No status bar surface yet (P8-007 follow-on); silently
            // retain the previous good state.
        }
    }
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
/// (running discovery, exec'ing into a mux client).
#[derive(Debug, Clone, PartialEq)]
enum Action {
    Msg(Msg),
    Refresh,
    Attach,
}

/// Handle the `a` key. On success, exec into `tmux attach-session`
/// — the process becomes the tmux client and never returns. On
/// disabled, set a status-bar message and stay in the TUI.
fn attach_action(app: &mut App) {
    match resolve_attach_target(app) {
        Ok(target) => exec_tmux_attach(&target),
        Err(reason) => {
            app.update(Msg::SetStatus(Some(attach_disabled_reason(&reason))));
        }
    }
}

/// Restore the terminal and exec into the mux client. Diverges on
/// success (the kernel replaces this process); exits with code 2
/// on failure, after restoring the terminal so the shell prompt
/// returns cleanly.
fn exec_tmux_attach(target: &AttachTarget) -> ! {
    ratatui::restore();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let native = target.mux.native_id.clone();
        let err = std::process::Command::new("tmux")
            .args(["attach-session", "-t", &native])
            .exec();
        eprintln!("conspectus: failed to exec tmux: {err}");
        std::process::exit(2);
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        eprintln!("conspectus: tmux attach is only supported on Unix");
        std::process::exit(2);
    }
}

/// Map crossterm events to [`Action`]s. Returns `None` for events
/// the v1 shell ignores. Pulled out so tests don't need a terminal.
///
/// `viewport_height` is the rendered height of the row tree in
/// rows, used to size PageUp/PageDown jumps. Pass 1 if unknown.
fn translate(event: Event, viewport_height: u16) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Action::Msg(Msg::Quit)),
            (_, KeyCode::Char('q')) => Some(Action::Msg(Msg::Quit)),
            (m, KeyCode::Char('r')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::Refresh),
            (m, KeyCode::Char('a')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::Attach),
            (_, KeyCode::Char('j')) | (_, KeyCode::Down) => Some(Action::Msg(Msg::NavDown)),
            (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Some(Action::Msg(Msg::NavUp)),
            (_, KeyCode::PageDown) => Some(Action::Msg(Msg::PageDown(viewport_height))),
            (_, KeyCode::PageUp) => Some(Action::Msg(Msg::PageUp(viewport_height))),
            (_, KeyCode::Home) | (_, KeyCode::Char('g')) => Some(Action::Msg(Msg::Home)),
            (_, KeyCode::End) | (_, KeyCode::Char('G')) => Some(Action::Msg(Msg::End)),
            (_, KeyCode::Enter) => Some(Action::Msg(Msg::ToggleExpand)),
            (_, KeyCode::Tab) => Some(Action::Msg(Msg::CycleFocus)),
            (_, KeyCode::Char('J')) => Some(Action::Msg(Msg::ScrollPreviewDown)),
            (_, KeyCode::Char('K')) => Some(Action::Msg(Msg::ScrollPreviewUp)),
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
        match action {
            Some(Action::Msg(msg)) => Some(msg),
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
            Some(Msg::ScrollPreviewDown)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('K'), KeyModifiers::NONE), 24)),
            Some(Msg::ScrollPreviewUp)
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
