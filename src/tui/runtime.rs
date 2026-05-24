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
            // Open overlays own key input while up. The controls
            // overlay takes precedence over the bare keymap; the
            // rename overlay does the same. Only one is open at a
            // time in v1.
            let action = if app.search_overlay().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::SearchOverlayKey(key))
                    }
                    _ => None,
                }
            } else if app.controls_overlay().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::ControlsOverlayKey(key))
                    }
                    _ => None,
                }
            } else if app.rename_overlay().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::RenameOverlayKey(key))
                    }
                    _ => None,
                }
            } else {
                translate(event, viewport).map(|a| remap_for_focus(a, app.focus()))
            };
            match action {
                Some(Action::Msg(msg)) => app.update(*msg),
                Some(Action::Refresh) => refresh(&mut app, &config),
                Some(Action::Attach) => attach_action(terminal, &mut app, &config),
                Some(Action::OpenRename) => open_rename_overlay(&mut app),
                Some(Action::RenameOverlayKey(key)) => {
                    handle_rename_overlay_key(&mut app, &config, tmux.as_ref(), key)
                }
                Some(Action::OpenControls) => {
                    app.open_controls_overlay();
                    app.update(Msg::SetStatus(Some(
                        "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::OpenControlsAtFilters) => {
                    app.open_controls_overlay_at_filters();
                    app.update(Msg::SetStatus(Some(
                        "controls: editing filters · Esc closes".to_string(),
                    )));
                }
                Some(Action::ControlsOverlayKey(key)) => {
                    handle_controls_overlay_key(&mut app, &config, key)
                }
                Some(Action::SwitchView(view)) => apply_view_switch(&mut app, &config, view),
                Some(Action::CycleView(delta)) => {
                    let next = cycle_view(app.config().default_view, delta);
                    apply_view_switch(&mut app, &config, next);
                }
                Some(Action::CycleGrouping(delta)) => {
                    let next = if delta >= 0 {
                        app.grouping().cycle_next()
                    } else {
                        app.grouping().cycle_prev()
                    };
                    apply_controls_action_and_refresh(
                        &mut app,
                        &config,
                        crate::tui::widgets::controls::ControlsAction::SetGrouping(next),
                    );
                }
                Some(Action::ClearFilters) => {
                    apply_controls_action_and_refresh(
                        &mut app,
                        &config,
                        crate::tui::widgets::controls::ControlsAction::SetFilter(
                            crate::filter::RowFilter::default(),
                        ),
                    );
                    app.update(Msg::SetStatus(Some("filters cleared".to_string())));
                }
                Some(Action::OpenSearch) => {
                    app.open_search_overlay();
                    app.update(Msg::SetStatus(Some(
                        "search: type to filter · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::SearchOverlayKey(key)) => {
                    handle_search_overlay_key(&mut app, key);
                }
                None => {}
            }
            refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), prev_mux_target);
        }
    }

    Ok(())
}

/// Resolve the current selection to an agent session and seed the
/// rename overlay with the strongest display label available
/// (alias > harness title > empty). No-op for non-session
/// selections; a status-bar message explains why.
fn open_rename_overlay(app: &mut App) {
    use crate::tui::rows::{RowId, RowKind};
    let Some(selection) = app.selection().cloned() else {
        app.update(Msg::SetStatus(Some("rename: nothing selected".to_string())));
        return;
    };
    if !matches!(
        selection,
        RowId::AgentSession(crate::model::NodeId::AgentSession(_))
    ) {
        app.update(Msg::SetStatus(Some(
            "rename: select an agent session row first".to_string(),
        )));
        return;
    }
    let initial = app
        .tree()
        .rows
        .iter()
        .find(|row| row.id == selection)
        .and_then(|row| match &row.kind {
            RowKind::AgentSession(session_row) => {
                Some(session_row.display_label().unwrap_or("").to_string())
            }
            _ => None,
        })
        .unwrap_or_default();
    let state = crate::tui::widgets::input::TextInputState::new(" rename session ", initial);
    app.open_rename_overlay(state);
    app.update(Msg::SetStatus(Some(
        "rename: Enter confirm · Esc cancel".to_string(),
    )));
}

/// Forward `key` to the open rename overlay, then act on the
/// resulting outcome. Confirm runs the lockstep plan (alias write +
/// optional tmux rename) and refreshes; Cancel just closes the
/// overlay.
fn handle_rename_overlay_key(
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn TmuxRunner,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::widgets::input::InputOutcome;
    let Some(state) = app.rename_overlay_mut() else {
        return;
    };
    let outcome = state.handle_key(key);
    match outcome {
        InputOutcome::Continue => {}
        InputOutcome::Cancel => {
            app.close_rename_overlay();
            app.update(Msg::SetStatus(Some("rename: cancelled".to_string())));
        }
        InputOutcome::Confirm(value) => {
            app.close_rename_overlay();
            commit_rename(app, config, tmux, value);
        }
    }
}

fn commit_rename(app: &mut App, config: &RunConfig, tmux: &dyn TmuxRunner, value: String) {
    use crate::tui::rows::RowId;
    let session_id = match app.selection().cloned() {
        Some(RowId::AgentSession(crate::model::NodeId::AgentSession(id))) => id,
        _ => {
            app.update(Msg::SetStatus(Some(
                "rename: lost selection before commit".to_string(),
            )));
            return;
        }
    };
    let trimmed = value.trim().to_string();
    let new_display_name = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.clone())
    };

    let snapshot = match app.snapshot() {
        Some(snap) => snap,
        None => {
            app.update(Msg::SetStatus(Some(
                "rename: no snapshot available".to_string(),
            )));
            return;
        }
    };

    let plan = match crate::rename::plan_session_rename(
        snapshot.as_ref(),
        &session_id,
        new_display_name.clone(),
        false,
    ) {
        Ok(plan) => plan,
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("rename failed: {err}"))));
            return;
        }
    };

    let endpoint = crate::declared::declared_endpoint_from_node_id(
        &crate::model::NodeId::AgentSession(session_id.clone()),
    );
    let loader = crate::config::ConfigLoader::from_env();
    let store_path = match crate::declared::select_store_for_declaration(
        &endpoint,
        &endpoint,
        snapshot.as_ref(),
        &loader,
    ) {
        Some(selection) => selection.path,
        None => match loader.user_config_path() {
            Some(path) => path,
            None => {
                app.update(Msg::SetStatus(Some(
                    "rename failed: no alias store available".to_string(),
                )));
                return;
            }
        },
    };

    let alias_outcome = match &plan.agent_alias_write.display_name {
        Some(name) => crate::aliases::upsert_alias_entry(
            &store_path,
            crate::aliases::AliasEntry {
                node: endpoint.clone(),
                display_name: name.clone(),
                reason: None,
            },
        )
        .map(|_| format!("renamed: {name}"))
        .map_err(|err| err.to_string()),
        None => crate::aliases::remove_alias_entry(&store_path, &endpoint)
            .map(|_| "rename: cleared alias".to_string())
            .map_err(|err| err.to_string()),
    };

    let alias_status = match alias_outcome {
        Ok(message) => message,
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("rename failed: {err}"))));
            return;
        }
    };

    if let Some(mux_rename) = &plan.mux_native_rename {
        match tmux.rename_session(&mux_rename.mux.native_id, &mux_rename.new_name) {
            Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => {}
            Ok(other) => {
                app.update(Msg::SetStatus(Some(format!(
                    "alias updated, tmux rename failed: {other:?}"
                ))));
                refresh(app, config);
                return;
            }
            Err(err) => {
                app.update(Msg::SetStatus(Some(format!(
                    "alias updated, tmux rename errored: {err}"
                ))));
                refresh(app, config);
                return;
            }
        }
    }

    let advisory = live_session_advisory(app, &session_id);
    refresh(app, config);
    let final_status = match advisory {
        Some(suffix) => format!("{alias_status} · {suffix}"),
        None => alias_status,
    };
    app.update(Msg::SetStatus(Some(final_status)));
}

/// Append an informational advisory when the rename target is a
/// live session per ADR 0029's live-session safety rule. "Live"
/// here means the row's mux indicator was `Attached` or `Ambiguous`
/// at the moment of commit — both of which require at least one
/// active mux candidate, which in turn carries hook-sidecar or
/// pane-process freshness signal.
fn live_session_advisory(
    app: &App,
    session_id: &crate::model::AgentSessionId,
) -> Option<&'static str> {
    use crate::tui::rows::{MuxIndicator, RowKind};
    let row = app.tree().rows.iter().find(|row| {
        matches!(
            &row.kind,
            RowKind::AgentSession(s) if s.session == *session_id
        )
    })?;
    let RowKind::AgentSession(session_row) = &row.kind else {
        return None;
    };
    match session_row.mux_state {
        MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => {
            Some("live session: alias overlays harness title until session ends")
        }
        MuxIndicator::Unmuxed => None,
    }
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
///
/// Reads the *live* config from `app` so view / grouping / filter
/// changes applied via the controls overlay take effect on the
/// next rebuild. The runtime's startup `config` is the seed but is
/// no longer the source of truth after the first user action.
fn refresh(app: &mut App, _seed: &RunConfig) {
    let config = app.config().clone();
    match discover_and_build(&config) {
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
            filter: config.initial_filter.clone(),
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
    /// Open the rename overlay for the current selection. The
    /// runtime resolves the AgentSession id and seeds the input
    /// buffer with the current alias, harness title, or an empty
    /// string per ADR 0030.
    OpenRename,
    /// Forward a key event into the open rename overlay.
    RenameOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the controls overlay (ADR 0031, F8-005) at its top
    /// section.
    OpenControls,
    /// Open the controls overlay positioned at the Filters section
    /// (the `f` accelerator).
    OpenControlsAtFilters,
    /// Forward a key event into the open controls overlay.
    ControlsOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Switch to a specific view (1–5 accelerators).
    SwitchView(View),
    /// Cycle to the next (delta > 0) or previous (delta < 0) view
    /// (the `]` / `[` accelerators).
    CycleView(i32),
    /// Cycle grouping for the active view forward or back. Bound
    /// to `Ctrl-G` because the End binding owns plain `G`.
    CycleGrouping(i32),
    /// Clear every active filter for the visible view (`F`).
    ClearFilters,
    /// Open the `/` search overlay (T8-017).
    OpenSearch,
    /// Forward a key event into the open search overlay.
    SearchOverlayKey(ratatui::crossterm::event::KeyEvent),
}

/// Dispatch a key into the open search overlay, refresh its match
/// list from the visible row tree using the configured backend,
/// and act on its outcome (Confirm picks a row, Cancel closes).
fn handle_search_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::search::{SubstringBackend, items_from_rows};
    use crate::tui::widgets::search::SearchOutcome;
    // The backend choice lives behind the SearchBackend trait so a
    // future swap (e.g. to a fuzzy matcher) needs only an
    // implementation change, not a runtime change. The substring
    // backend is the v1 default per ADR 0024's "prefer hand-rolled
    // first" stance.
    let backend = SubstringBackend;
    let visible: Vec<_> = app.visible_rows().into_iter().cloned().collect();
    let items = items_from_rows(&visible);
    let outcome = match app.search_overlay_mut() {
        Some(state) => {
            let outcome = state.handle_key(key);
            state.refresh_matches(&backend, &items);
            outcome
        }
        None => return,
    };
    match outcome {
        SearchOutcome::Continue => {}
        SearchOutcome::Cancel => {
            app.close_search_overlay();
        }
        SearchOutcome::Confirm(id) => {
            app.close_search_overlay();
            app.set_selection(*id);
        }
    }
}

/// Handle the controls overlay's key event and apply the resulting
/// action to the app, refreshing the row tree when needed.
fn handle_controls_overlay_key(
    app: &mut App,
    config: &RunConfig,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::widgets::controls::{ControlsContext, ControlsOutcome};
    // Snapshot the live state into owned copies so the immutable
    // borrow on `app` ends before we re-borrow it mutably to
    // dispatch the key into the overlay.
    let view = app.config().default_view;
    let grouping = app.grouping();
    let filter_snapshot = app.filter().clone();
    let sort = app.sort();
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
    };
    let outcome = match app.controls_overlay_mut() {
        Some(state) => state.handle_key(&ctx, key),
        None => return,
    };
    match outcome {
        ControlsOutcome::Continue => {}
        ControlsOutcome::Close => {
            app.close_controls_overlay();
        }
        ControlsOutcome::ApplyAndStay(action) => {
            apply_controls_action_and_refresh(app, config, action);
        }
        ControlsOutcome::ApplyAndClose(action) => {
            app.close_controls_overlay();
            apply_controls_action_and_refresh(app, config, action);
        }
    }
}

/// Apply a controls action and rebuild the row tree so the change
/// is visible immediately. Side-effecting in two places (App state
/// plus discovery refresh) but kept in one helper so the call
/// sites can't accidentally apply without refreshing.
fn apply_controls_action_and_refresh(
    app: &mut App,
    config: &RunConfig,
    action: crate::tui::widgets::controls::ControlsAction,
) {
    app.apply_controls_action(action);
    refresh(app, config);
}

/// Switch view and refresh. Shared between the `1`–`5` direct keys
/// and `]` / `[` cycling.
fn apply_view_switch(app: &mut App, config: &RunConfig, view: View) {
    apply_controls_action_and_refresh(
        app,
        config,
        crate::tui::widgets::controls::ControlsAction::SwitchView(view),
    );
}

/// Step the view enum forward (delta > 0) or back (delta < 0),
/// wrapping. Used by the `]` / `[` accelerator pair.
fn cycle_view(view: View, delta: i32) -> View {
    use crate::tui::widgets::controls::VIEW_OPTIONS;
    let idx = VIEW_OPTIONS.iter().position(|v| *v == view).unwrap_or(0) as i32;
    let len = VIEW_OPTIONS.len() as i32;
    let next = ((idx + delta) % len + len) % len;
    VIEW_OPTIONS[next as usize]
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
            (KeyModifiers::SHIFT, KeyCode::Char('R'))
            | (KeyModifiers::NONE, KeyCode::Char('R')) => Some(Action::OpenRename),
            // ADR 0031 / F8-005 accelerator surface. `v` opens the
            // controls overlay; `1`–`5` switch view directly;
            // `]`/`[` cycle views; `f` jumps into the controls
            // overlay's Filters section; `F` clears every active
            // filter; `Ctrl-G` cycles grouping (plain `G` is the
            // existing End binding).
            (m, KeyCode::Char('v')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenControls)
            }
            (m, KeyCode::Char('f')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenControlsAtFilters)
            }
            (KeyModifiers::SHIFT, KeyCode::Char('F'))
            | (KeyModifiers::NONE, KeyCode::Char('F')) => Some(Action::ClearFilters),
            (KeyModifiers::CONTROL, KeyCode::Char('g')) => Some(Action::CycleGrouping(1)),
            (m, KeyCode::Char(']')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::CycleView(1))
            }
            (m, KeyCode::Char('[')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::CycleView(-1))
            }
            (m, KeyCode::Char('1')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::SwitchView(View::Sessions))
            }
            (m, KeyCode::Char('2')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::SwitchView(View::Mux))
            }
            (m, KeyCode::Char('3')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::SwitchView(View::Union))
            }
            (m, KeyCode::Char('4')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::SwitchView(View::Prs))
            }
            (m, KeyCode::Char('5')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::SwitchView(View::Forks))
            }
            (m, KeyCode::Char('/')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenSearch)
            }
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
    fn translate_shift_r_opens_rename_overlay() {
        assert_eq!(
            translate(press(KeyCode::Char('R'), KeyModifiers::SHIFT), 24),
            Some(Action::OpenRename)
        );
        assert_eq!(
            translate(press(KeyCode::Char('R'), KeyModifiers::NONE), 24),
            Some(Action::OpenRename)
        );
    }

    #[test]
    fn translate_lowercase_r_still_refreshes() {
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
    fn translate_v_opens_controls_overlay() {
        assert_eq!(
            translate(press(KeyCode::Char('v'), KeyModifiers::NONE), 24),
            Some(Action::OpenControls)
        );
    }

    #[test]
    fn translate_f_opens_controls_at_filters() {
        assert_eq!(
            translate(press(KeyCode::Char('f'), KeyModifiers::NONE), 24),
            Some(Action::OpenControlsAtFilters)
        );
    }

    #[test]
    fn translate_shift_f_clears_filters() {
        assert_eq!(
            translate(press(KeyCode::Char('F'), KeyModifiers::SHIFT), 24),
            Some(Action::ClearFilters)
        );
        assert_eq!(
            translate(press(KeyCode::Char('F'), KeyModifiers::NONE), 24),
            Some(Action::ClearFilters)
        );
    }

    #[test]
    fn translate_ctrl_g_cycles_grouping() {
        assert_eq!(
            translate(press(KeyCode::Char('g'), KeyModifiers::CONTROL), 24),
            Some(Action::CycleGrouping(1))
        );
    }

    #[test]
    fn translate_digits_switch_views_directly() {
        let cases = [
            ('1', View::Sessions),
            ('2', View::Mux),
            ('3', View::Union),
            ('4', View::Prs),
            ('5', View::Forks),
        ];
        for (ch, view) in cases {
            assert_eq!(
                translate(press(KeyCode::Char(ch), KeyModifiers::NONE), 24),
                Some(Action::SwitchView(view)),
                "char {ch}"
            );
        }
    }

    #[test]
    fn translate_brackets_cycle_views() {
        assert_eq!(
            translate(press(KeyCode::Char(']'), KeyModifiers::NONE), 24),
            Some(Action::CycleView(1))
        );
        assert_eq!(
            translate(press(KeyCode::Char('['), KeyModifiers::NONE), 24),
            Some(Action::CycleView(-1))
        );
    }

    #[test]
    fn cycle_view_wraps_in_both_directions() {
        assert_eq!(cycle_view(View::Sessions, -1), View::Forks);
        assert_eq!(cycle_view(View::Forks, 1), View::Sessions);
        assert_eq!(cycle_view(View::Mux, 1), View::Union);
        assert_eq!(cycle_view(View::Union, -1), View::Mux);
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
