//! Terminal lifecycle and event loop.
//!
//! `ratatui::init` already installs a panic hook that restores the
//! terminal, so this module just sets up the loop and is the only
//! place in the crate that touches stdout in raw mode.
//!
//! Discovery runs on a background thread per ADR 0024 (`mpsc` + no
//! async runtime). The initial load still runs synchronously so the
//! operator sees a populated tree on the first frame; subsequent
//! refreshes (manual `r` or timer-driven) dispatch through the
//! background channel and never block input.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::discovery::discover_local_at_roots;
use crate::discovery::tmux::{SystemTmux, TmuxRunner};
use crate::model::MuxSessionId;
use crate::pins::{PinEntry, PinLaunch, PinMux, PinStoreKind, PinWriteOutcome, TMUX_MUX_BACKEND};
use crate::resolve::resolve_snapshot;
use crate::tui::actions::{
    AttachTarget, attach_disabled_reason, resolve_attach_target, resolve_view_session,
};
use crate::tui::app::{App, GraphDb, Msg};
use crate::tui::preview::capture_via;
use crate::tui::resume::{
    ResumeTarget, launch_resume, resolve_resume_target, resume_disabled_reason,
};
use crate::tui::rows::RowId;
use crate::tui::rows::RowTree;
use crate::tui::rows::sessions::{SessionsBuildInputsFromConn, build_sessions_tree_from_conn};
use crate::tui::viewer::{
    LaunchPlan, PathBinaryProbe, ViewerTarget, resolve_viewer_target, viewer_disabled_reason,
};
use crate::tui::{RunConfig, View, ui};

/// Result of a completed background discovery run. The worker returns
/// only the resolved snapshot; the main thread materializes SQLite and
/// builds the row tree from the app's current view config.
type DiscoveryResult = Result<crate::model::GraphSnapshot>;

/// Run the TUI to completion. Restores the terminal on normal exit,
/// errors, and panics (the panic path is covered by the hook
/// `ratatui::init` installs).
pub fn run(config: RunConfig) -> Result<()> {
    let mut terminal = ratatui::init();
    // Explicitly clear the alt screen before the first draw.
    // `EnterAlternateScreen` alone isn't enough under some
    // multiplexers — notably mosh, which doesn't fully blank the
    // alt buffer on switch — so the previous shell's content can
    // "bleed through" any cell ratatui's diff-renderer decides
    // hasn't changed from the empty initial buffer. The explicit
    // clear forces every cell to a known blank state and matches
    // the reattach path below.
    let _ = terminal.clear();
    let result = event_loop(&mut terminal, config);
    ratatui::restore();
    result
}

/// Run a static, pre-materialized graph in the TUI. This is for
/// debug-only replay scenarios: it lets developers inspect edge-case
/// worlds through the real renderer and reducer without teaching
/// production discovery about fixtures.
#[cfg(any(test, debug_assertions))]
pub fn run_static(config: RunConfig, snapshot: crate::model::GraphSnapshot) -> Result<()> {
    let mut terminal = ratatui::init();
    let _ = terminal.clear();
    let result = static_event_loop(&mut terminal, config, snapshot);
    ratatui::restore();
    result
}

/// Block on terminal input, dispatching crossterm events to the
/// pure reducer until the app signals quit.
fn event_loop(terminal: &mut DefaultTerminal, config: RunConfig) -> Result<()> {
    let mut app = App::new(config.clone());
    let tmux: Box<dyn TmuxRunner> = Box::new(SystemTmux::new());

    // Initial synchronous discovery.
    refresh(&mut app, &config);
    refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), None);

    let (result_tx, result_rx) = mpsc::channel::<DiscoveryResult>();
    let mut pending_refresh = false;
    let refresh_interval = config.refresh_interval;
    let mut last_refresh = Instant::now();
    let poll_timeout = Duration::from_millis(100);

    while !app.should_quit() {
        terminal.draw(|frame| {
            let area = frame.area();
            if app.viewer_modal().is_some() {
                let theme = app.theme().clone();
                if let Some(state) = app.viewer_modal_mut() {
                    crate::viewer::widget::draw(state, &theme, frame, area);
                }
            } else {
                ui::draw(&app, frame);
            }
        })?;

        // Drain completed background discovery results without
        // blocking. Only the most recent result wins.
        while let Ok(result) = result_rx.try_recv() {
            pending_refresh = false;
            match result {
                Ok(snapshot) => match crate::query::materialize_snapshot(&snapshot) {
                    Ok(conn) => {
                        let live_config = app.config().clone();
                        match build_tree_for_view(&conn, &live_config) {
                            Ok(tree) => {
                                let database = GraphDb::new(conn);
                                let initial_selection_hint = launch_context_row_id(&tree);
                                app.update(Msg::SetData {
                                    snapshot: database,
                                    tree,
                                    loaded_at_epoch: current_unix_epoch().unwrap_or(0),
                                    initial_selection_hint,
                                });
                                populate_provider_status(&mut app, &live_config);
                            }
                            Err(err) => {
                                app.update(Msg::SetRefreshFailure(format!(
                                    "last refresh failed; {err}"
                                )));
                            }
                        }
                    }
                    Err(err) => {
                        app.update(Msg::SetRefreshFailure(format!(
                            "last refresh failed; {err}"
                        )));
                    }
                },
                Err(err) => {
                    app.update(Msg::SetRefreshFailure(format!(
                        "last refresh failed; {err}"
                    )));
                }
            }
        }

        // Timer-driven auto-refresh. Only fires when no request is
        // in-flight and at least `refresh_interval` has elapsed.
        if !pending_refresh && last_refresh.elapsed() >= refresh_interval {
            pending_refresh = true;
            last_refresh = Instant::now();
            spawn_discovery_worker(app.config(), &result_tx);
        }

        if event::poll(poll_timeout)? {
            let event = event::read()?;
            let viewport = terminal.size()?.height.saturating_sub(2);
            let prev_mux_target = current_mux_target(&app);
            let action = if app.viewer_modal().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::ViewerOverlayKey(key))
                    }
                    _ => None,
                }
            } else if app.value_modal().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::ValueModalKey(key))
                    }
                    _ => None,
                }
            } else if app.help_overlay().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::HelpOverlayKey(key))
                    }
                    _ => None,
                }
            } else if app.search_overlay().is_some() {
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
                Some(Action::Refresh) => {
                    if !pending_refresh {
                        pending_refresh = true;
                        last_refresh = Instant::now();
                        spawn_discovery_worker(app.config(), &result_tx);
                    }
                }
                Some(Action::Attach) => attach_action(terminal, &mut app, &config),
                Some(Action::Resume) => resume_action(&mut app),
                Some(Action::View) => view_action(terminal, &mut app, &config),
                Some(Action::DefaultAction) => default_action(terminal, &mut app, &config),
                Some(Action::OpenRename) => open_rename_overlay(&mut app),
                Some(Action::RenameOverlayKey(key)) => {
                    handle_rename_overlay_key(&mut app, &config, tmux.as_ref(), key)
                }
                Some(Action::RemovePin) => remove_pin_action(&mut app, &config),
                Some(Action::PinBindHint) => pin_bind_hint_action(&mut app),
                Some(Action::OpenControls) => {
                    app.open_controls_overlay();
                    app.update(Msg::SetStatus(Some(
                        "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
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
                Some(Action::OpenHelp) => {
                    app.open_help_overlay();
                }
                Some(Action::HelpOverlayKey(key)) => {
                    handle_help_overlay_key(&mut app, key);
                }
                Some(Action::OpenValueModal) => {
                    app.open_value_modal_for_cursor();
                }
                Some(Action::ValueModalKey(key)) => {
                    handle_value_modal_key(&mut app, key);
                }
                Some(Action::ViewerOverlayKey(key)) => {
                    handle_viewer_overlay_key(&mut app, key);
                }
                Some(Action::ExplorerEnter) => explorer_enter_action(&mut app),
                Some(Action::CopySessionId) => copy_session_id_action(&mut app),
                None => {}
            }
            refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), prev_mux_target);
        }
    }

    Ok(())
}

#[cfg(any(test, debug_assertions))]
fn static_event_loop(
    terminal: &mut DefaultTerminal,
    config: RunConfig,
    snapshot: crate::model::GraphSnapshot,
) -> Result<()> {
    let mut app = App::new(config.clone());
    set_static_data(&mut app, &config, &snapshot)?;
    let tmux: Box<dyn TmuxRunner> = Box::new(SystemTmux::new());
    refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), None);
    let poll_timeout = Duration::from_millis(100);

    while !app.should_quit() {
        terminal.draw(|frame| {
            let area = frame.area();
            if app.viewer_modal().is_some() {
                let theme = app.theme().clone();
                if let Some(state) = app.viewer_modal_mut() {
                    crate::viewer::widget::draw(state, &theme, frame, area);
                }
            } else {
                ui::draw(&app, frame);
            }
        })?;
        if event::poll(poll_timeout)? {
            let event = event::read()?;
            let viewport = terminal.size()?.height.saturating_sub(2);
            let prev_mux_target = current_mux_target(&app);
            let action = static_action_for_event(&app, event, viewport);
            match action {
                Some(Action::Msg(msg)) => app.update(*msg),
                Some(Action::OpenHelp) => app.open_help_overlay(),
                Some(Action::HelpOverlayKey(key)) => handle_help_overlay_key(&mut app, key),
                Some(Action::OpenValueModal) => app.open_value_modal_for_cursor(),
                Some(Action::ValueModalKey(key)) => handle_value_modal_key(&mut app, key),
                Some(Action::ViewerOverlayKey(key)) => handle_viewer_overlay_key(&mut app, key),
                Some(Action::OpenSearch) => {
                    app.open_search_overlay();
                    app.update(Msg::SetStatus(Some(
                        "search: type to filter · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::SearchOverlayKey(key)) => handle_search_overlay_key(&mut app, key),
                Some(Action::OpenControls) => {
                    app.open_controls_overlay();
                    app.update(Msg::SetStatus(Some(
                        "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::ControlsOverlayKey(key)) => {
                    static_handle_controls_overlay_key(&mut app, &config, &snapshot, key)?;
                }
                Some(Action::SwitchView(view)) => {
                    app.apply_controls_action(
                        crate::tui::widgets::controls::ControlsAction::SwitchView(view),
                    );
                    set_static_data(&mut app, &config, &snapshot)?;
                }
                Some(Action::CycleView(delta)) => {
                    let next = cycle_view(app.config().default_view, delta);
                    app.apply_controls_action(
                        crate::tui::widgets::controls::ControlsAction::SwitchView(next),
                    );
                    set_static_data(&mut app, &config, &snapshot)?;
                }
                Some(Action::Refresh) => {
                    set_static_data(&mut app, &config, &snapshot)?;
                    app.update(Msg::SetStatus(Some(
                        "scenario snapshot reloaded".to_string(),
                    )));
                }
                Some(Action::Attach) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; attach is disabled".to_string(),
                    )));
                }
                Some(Action::Resume) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; resume is disabled".to_string(),
                    )));
                }
                Some(Action::View) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; view is disabled".to_string(),
                    )));
                }
                Some(Action::DefaultAction) => match selected_default_action(&app) {
                    SelectedDefault::ToggleExpand => app.update(Msg::ToggleExpand),
                    SelectedDefault::Attach => app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; attach is disabled".to_string(),
                    ))),
                    SelectedDefault::View => app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; view is disabled".to_string(),
                    ))),
                    SelectedDefault::LaunchPin => app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; pin launch is disabled".to_string(),
                    ))),
                },
                Some(Action::CycleGrouping(delta)) => {
                    let next = if delta >= 0 {
                        app.grouping().cycle_next()
                    } else {
                        app.grouping().cycle_prev()
                    };
                    static_apply_controls_action_and_refresh(
                        &mut app,
                        &config,
                        &snapshot,
                        crate::tui::widgets::controls::ControlsAction::SetGrouping(next),
                    )?;
                }
                Some(Action::ClearFilters) => {
                    static_apply_controls_action_and_refresh(
                        &mut app,
                        &config,
                        &snapshot,
                        crate::tui::widgets::controls::ControlsAction::SetFilter(
                            crate::filter::RowFilter::default(),
                        ),
                    )?;
                    app.update(Msg::SetStatus(Some("filters cleared".to_string())));
                }
                Some(Action::OpenRename) | Some(Action::RenameOverlayKey(_)) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI keeps mutating actions disabled".to_string(),
                    )));
                }
                Some(Action::RemovePin) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI keeps mutating actions disabled".to_string(),
                    )));
                }
                Some(Action::PinBindHint) => pin_bind_hint_action(&mut app),
                Some(Action::ExplorerEnter) => explorer_enter_action(&mut app),
                Some(Action::CopySessionId) => copy_session_id_action(&mut app),
                None => {}
            }
            refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), prev_mux_target);
        }
    }

    Ok(())
}

#[cfg(any(test, debug_assertions))]
fn static_handle_controls_overlay_key(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    key: ratatui::crossterm::event::KeyEvent,
) -> Result<()> {
    use crate::tui::widgets::controls::{ControlsContext, ControlsOutcome};
    let view = app.config().default_view;
    let grouping = app.grouping();
    let filter_snapshot = app.filter().clone();
    let sort = app.sort();
    let pin_create_defaults = app.controls_context().pin_create_defaults;
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
        pin_create_defaults,
    };
    let outcome = match app.controls_overlay_mut() {
        Some(state) => state.handle_key(&ctx, key),
        None => return Ok(()),
    };
    match outcome {
        ControlsOutcome::Continue => {}
        ControlsOutcome::Close => {
            app.close_controls_overlay();
        }
        ControlsOutcome::ApplyAndStay(action) => {
            static_apply_controls_action_and_refresh(app, config, snapshot, action)?;
        }
        ControlsOutcome::ApplyAndClose(action) => {
            app.close_controls_overlay();
            static_apply_controls_action_and_refresh(app, config, snapshot, action)?;
        }
    }
    Ok(())
}

#[cfg(any(test, debug_assertions))]
fn static_apply_controls_action_and_refresh(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    action: crate::tui::widgets::controls::ControlsAction,
) -> Result<()> {
    if matches!(
        action,
        crate::tui::widgets::controls::ControlsAction::CreatePin(_)
    ) {
        app.update(Msg::SetStatus(Some(
            "scenario TUI keeps mutating actions disabled".to_string(),
        )));
        return Ok(());
    }
    app.apply_controls_action(action);
    set_static_data(app, config, snapshot)
}

#[cfg(any(test, debug_assertions))]
fn set_static_data(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
) -> Result<()> {
    let conn = crate::query::materialize_snapshot(snapshot)?;
    let tree = build_tree_for_view(&conn, app.config())?;
    let database = GraphDb::new(conn);
    let initial_selection_hint = launch_context_row_id(&tree);
    app.update(Msg::SetData {
        snapshot: database,
        tree,
        loaded_at_epoch: current_unix_epoch().unwrap_or(0),
        initial_selection_hint,
    });
    populate_provider_status(app, config);
    Ok(())
}

#[cfg(any(test, debug_assertions))]
fn static_action_for_event(app: &App, event: Event, viewport: u16) -> Option<Action> {
    if app.viewer_modal().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                Some(Action::ViewerOverlayKey(key))
            }
            _ => None,
        };
    }
    if app.value_modal().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => Some(Action::ValueModalKey(key)),
            _ => None,
        };
    }
    if app.help_overlay().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => Some(Action::HelpOverlayKey(key)),
            _ => None,
        };
    }
    if app.search_overlay().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                Some(Action::SearchOverlayKey(key))
            }
            _ => None,
        };
    }
    if app.controls_overlay().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                Some(Action::ControlsOverlayKey(key))
            }
            _ => None,
        };
    }
    if app.rename_overlay().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                Some(Action::RenameOverlayKey(key))
            }
            _ => None,
        };
    }
    translate(event, viewport).map(|action| remap_for_focus(action, app.focus()))
}

/// Spawn a background thread that runs discovery and sends the resolved
/// snapshot through `tx`. SQLite materialization and row-tree building
/// stay on the main thread so they can use the app's current view state.
fn spawn_discovery_worker(config: &RunConfig, tx: &mpsc::Sender<DiscoveryResult>) {
    let config = config.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let result = discover_and_resolve(&config);
        let _ = tx.send(result);
    });
}

/// Run discovery and resolver on the calling thread, returning the
/// resolved snapshot (no SQLite materialization).
fn discover_and_resolve(config: &RunConfig) -> Result<crate::model::GraphSnapshot> {
    let roots: Vec<PathBuf> = if config.scan_roots.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        config.scan_roots.clone()
    };
    let snapshot = discover_local_at_roots(roots)?;
    Ok(resolve_snapshot(snapshot))
}

/// Resolve the current selection to a renameable row and seed the
/// rename overlay. Agent sessions use alias > harness title > empty;
/// pin rows use the pin display name.
fn open_rename_overlay(app: &mut App) {
    use crate::tui::rows::{RowId, RowKind};
    let Some(selection) = app.selection().cloned() else {
        app.update(Msg::SetStatus(Some("rename: nothing selected".to_string())));
        return;
    };
    let row = app.tree().rows.iter().find(|row| row.id == selection);
    let (title, initial) = match (selection, row.map(|row| &row.kind)) {
        (
            RowId::AgentSession(crate::model::NodeId::AgentSession(_)),
            Some(RowKind::AgentSession(session_row)),
        ) => (
            " rename session ",
            session_row.display_label().unwrap_or("").to_string(),
        ),
        (RowId::Pin { .. }, Some(RowKind::Pin(pin))) => (" rename pin ", pin.display_name.clone()),
        _ => {
            app.update(Msg::SetStatus(Some(
                "rename: select an agent session or pin row first".to_string(),
            )));
            return;
        }
    };
    let state = crate::tui::widgets::input::TextInputState::new(title, initial);
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
    let selection = app.selection().cloned();
    let session_id = match selection {
        Some(RowId::AgentSession(crate::model::NodeId::AgentSession(id))) => id,
        Some(RowId::Pin { pin_id }) => {
            commit_pin_rename(app, config, &pin_id, value);
            return;
        }
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

    let database = match app.graph_db() {
        Some(database) => database,
        None => {
            app.update(Msg::SetStatus(Some(
                "rename: no graph database available".to_string(),
            )));
            return;
        }
    };
    let snapshot = match crate::query::read_snapshot(database.conn()) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("rename failed: {err}"))));
            return;
        }
    };

    let plan = match crate::rename::plan_session_rename(
        &snapshot,
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
        &endpoint, &endpoint, &snapshot, &loader,
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
        // Default-socket rename for now; the pin-driven socket
        // propagation lands with H-PIN-017's TUI lockstep work.
        match tmux.rename_session(None, &mux_rename.mux.native_id, &mux_rename.new_name) {
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

fn commit_pin_rename(app: &mut App, config: &RunConfig, pin_id: &str, value: String) {
    let display = value.trim();
    if display.is_empty() {
        app.update(Msg::SetStatus(Some(
            "pin rename: display name cannot be empty".to_string(),
        )));
        return;
    }
    let status = current_exe_command()
        .args(["pin", "rename", pin_id, "--display", display])
        .status();
    refresh(app, config);
    let message = match status {
        Ok(s) if s.success() => format!("renamed pin `{pin_id}`"),
        Ok(s) => format!("pin rename `{pin_id}` exited with {s}"),
        Err(err) => format!("pin rename `{pin_id}` failed to spawn: {err}"),
    };
    app.update(Msg::SetStatus(Some(message)));
}

fn remove_pin_action(app: &mut App, config: &RunConfig) {
    let Some(selection) = app.selection().cloned() else {
        app.update(Msg::SetStatus(Some(
            "pin remove: nothing selected".to_string(),
        )));
        return;
    };
    let pin_id = match selection {
        RowId::Pin { pin_id } => pin_id,
        _ => {
            app.set_pending_pin_remove(None);
            app.update(Msg::SetStatus(Some(
                "pin remove: select an unbound pin row".to_string(),
            )));
            return;
        }
    };
    if app.pending_pin_remove() != Some(pin_id.as_str()) {
        app.set_pending_pin_remove(Some(pin_id.clone()));
        app.update(Msg::SetStatus(Some(format!(
            "pin remove: press Delete again to remove `{pin_id}`"
        ))));
        return;
    }
    app.set_pending_pin_remove(None);
    let status = current_exe_command().args(["pin", "rm", &pin_id]).status();
    refresh(app, config);
    let message = match status {
        Ok(s) if s.success() => format!("removed pin `{pin_id}`"),
        Ok(s) => format!("pin remove `{pin_id}` exited with {s}"),
        Err(err) => format!("pin remove `{pin_id}` failed to spawn: {err}"),
    };
    app.update(Msg::SetStatus(Some(message)));
}

fn pin_bind_hint_action(app: &mut App) {
    let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
    match crate::tui::actions::pin_bind_hint(&diagnostics) {
        Some(message) => app.update(Msg::SetStatus(Some(message))),
        None => app.update(Msg::SetStatus(Some(
            "pin bind: select a pin-bound row with a PinAmbiguous diagnostic".to_string(),
        ))),
    }
}

fn current_exe_command() -> std::process::Command {
    std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("conspectus")),
    )
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
    populate_provider_status(app, &config);
    match discover_and_build(&config) {
        Ok((database, tree)) => {
            let initial_selection_hint = launch_context_row_id(&tree);
            app.update(Msg::SetData {
                snapshot: database,
                tree,
                loaded_at_epoch: current_unix_epoch().unwrap_or(0),
                initial_selection_hint,
            });
        }
        Err(err) => {
            app.update(Msg::SetRefreshFailure(format!(
                "last refresh failed; {}",
                err
            )));
        }
    }
}

/// Populate `App::provider_status` from the run config and (in a
/// follow-up) from discovery-level diagnostics. Today the env-var
/// toggles are the only source; tmux/forge availability is probed
/// during discovery and surfaced later.
fn populate_provider_status(app: &mut App, _config: &RunConfig) {
    use std::env;
    let mut status = crate::tui::app::ProviderStatus::default();
    if env::var("CONSPECTUS_DISABLE_TMUX").is_ok_and(|v| !v.is_empty()) {
        status.tmux_disabled = true;
    }
    if env::var("CONSPECTUS_DISABLE_FORGE").is_ok_and(|v| !v.is_empty()) {
        status.forge_disabled = true;
    }
    app.update(Msg::SetProviderStatus(status));
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

fn discover_and_build(config: &RunConfig) -> Result<(GraphDb, RowTree)> {
    let snapshot = if config.scan_roots.is_empty() {
        let cwd = std::env::current_dir()?;
        discover_local_at_roots([cwd])?
    } else {
        discover_local_at_roots(config.scan_roots.clone())?
    };
    let snapshot = resolve_snapshot(snapshot);
    let database = GraphDb::new(crate::query::materialize_snapshot(&snapshot)?);

    let tree = build_tree_for_view(database.conn(), config)?;
    Ok((database, tree))
}

fn build_tree_for_view(conn: &rusqlite::Connection, config: &RunConfig) -> Result<RowTree> {
    let home = home_dir();
    let tree = match config.default_view {
        View::Sessions => build_sessions_tree_from_conn(SessionsBuildInputsFromConn {
            conn,
            grouping: config.sessions_grouping,
            home: home.as_deref(),
            now: current_unix_epoch(),
            cwd: config.cwd.as_deref(),
            filter: config.initial_filter.clone(),
        })?,
        View::Mux => crate::tui::rows::mux::build_mux_tree_from_conn(
            crate::tui::rows::mux::MuxBuildInputsFromConn {
                conn,
                home: home.as_deref(),
                now: current_unix_epoch(),
                filter: config.initial_filter.clone(),
                grouping: config.mux_grouping,
            },
        )?,
        View::Union => crate::tui::rows::union::build_union_tree_from_conn(
            crate::tui::rows::union::UnionBuildInputsFromConn {
                conn,
                home: home.as_deref(),
                now: current_unix_epoch(),
                filter: config.initial_filter.clone(),
            },
        )?,
        View::Prs => crate::tui::rows::prs::build_prs_tree_from_conn(
            crate::tui::rows::prs::PrsBuildInputsFromConn {
                conn,
                home: home.as_deref(),
                now: current_unix_epoch(),
                filter: config.initial_filter.clone(),
            },
        )?,
        View::Forks => crate::tui::rows::forks::build_forks_tree_from_conn(
            crate::tui::rows::forks::ForksBuildInputsFromConn {
                conn,
                home: home.as_deref(),
                now: current_unix_epoch(),
                filter: config.initial_filter.clone(),
            },
        )?,
    };
    Ok(tree)
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
//
// `EnterDefault` ends in `Action` semantically (it names the key's
// behavior) and lives alongside `Attach`, `Resume`, `View`, etc.,
// which all read as actions implicitly. Suppress the
// `enum_variant_names` lint locally so the name doesn't have to be
// twisted to satisfy the lint.
#[allow(clippy::enum_variant_names)]
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
    /// `Delete` on an unbound/stale pin row. First press arms a
    /// confirmation; second press shells out to `conspectus pin rm`.
    RemovePin,
    /// `b` on a selected ambiguous pin-bound row. H-PIN-018 routes
    /// to a bind escape-hatch hint; the structured picker lands in
    /// H-PIN-024.
    PinBindHint,
    /// Open the controls overlay (ADR 0031, F8-005) at its top
    /// section.
    OpenControls,
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
    /// Open the `?` help overlay (F8-011).
    OpenHelp,
    /// Forward a key event into the open help overlay.
    HelpOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the `o` full-value modal (T8-030) on the active explorer
    /// cursor row, when the row has a truncated value.
    OpenValueModal,
    /// Forward a key event into the open full-value modal.
    ValueModalKey(ratatui::crossterm::event::KeyEvent),
    /// Resume the selected un-muxed agent session in a new terminal
    /// (launches the harness binary in the background).
    Resume,
    /// Open the native full-screen transcript viewer modal for
    /// the selected agent session (H-VIEWER-NATIVE-008, ADR 0052).
    /// The widget renders inside the existing terminal — no alt-
    /// screen swap, no child process. Falls through to the
    /// escape-hatch external launch when the harness has no
    /// native parser (kept for `aider` etc.).
    View,
    /// Forward a key event into the open viewer modal.
    ViewerOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// `Enter` on the left pane (T8-043). The dispatcher resolves
    /// the selected row's default action: attach a mux row, view an
    /// un-muxed session, or expand/collapse a group row. Right-pane
    /// focus is remapped to [`Action::ExplorerEnter`] before this
    /// variant ever reaches the dispatcher.
    DefaultAction,
    /// `Enter` on the right pane (T8-040 / T8-043). When the
    /// explorer cursor is on a Node-zone field row with a copyable
    /// value, the runtime writes the value to the clipboard via OSC
    /// 52 (ADR 0056) and posts a toast. Otherwise the dispatcher
    /// falls through to [`Msg::ExplorerActivate`] (group expand,
    /// link drill).
    ExplorerEnter,
    /// `i` on any row (T8-040). When the selected row is an agent
    /// session or mux session, copies the full id to the clipboard
    /// (ADR 0056) and posts a toast. No-op with a status hint
    /// otherwise.
    CopySessionId,
}

/// Resolved default action for the left-pane cursor (T8-043). Pure
/// over [`App`] state so it can be reused by the live and static
/// event loops and snapshot-tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedDefault {
    /// Group row — `Enter` expands/collapses.
    ToggleExpand,
    /// Mux row or attachable agent session — `Enter` attaches.
    Attach,
    /// Un-muxed agent session — `Enter` opens the transcript viewer.
    View,
    /// Unbound / stale-mux pin row — `Enter` shells out to
    /// `conspectus pin launch <id>` per ADR 0057.
    LaunchPin,
}

fn selected_default_action(app: &App) -> SelectedDefault {
    use crate::tui::rows::{MuxIndicator, RowKind};
    let Some(selection) = app.selection() else {
        return SelectedDefault::ToggleExpand;
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return SelectedDefault::ToggleExpand;
    };
    match &row.kind {
        RowKind::Group(_) => SelectedDefault::ToggleExpand,
        RowKind::MuxSession(_) | RowKind::AgentSessionMuxCandidate(_) => SelectedDefault::Attach,
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Unmuxed => SelectedDefault::View,
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => SelectedDefault::Attach,
        },
        // PR / Fork rows: no muxable target and no viewer; fall back
        // to toggle so expandable parents still behave.
        RowKind::Pr(_) | RowKind::Fork(_) => SelectedDefault::ToggleExpand,
        // Unbound / stale-mux pin rows hand off to the launch
        // primitive (H-PIN-012) via a subprocess so the launch
        // logic stays in one place.
        RowKind::Pin(_) => SelectedDefault::LaunchPin,
    }
}

/// Dispatch a key into the open help overlay and close it on
/// HelpOutcome::Close.
fn handle_help_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::widgets::help::HelpOutcome;
    let outcome = match app.help_overlay_mut() {
        Some(state) => state.handle_key(key),
        None => return,
    };
    if let HelpOutcome::Close = outcome {
        app.close_help_overlay();
    }
}

/// Dispatch a key into the open full-value modal and close it on
/// ValueModalOutcome::Close.
fn handle_value_modal_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::widgets::value_modal::ValueModalOutcome;
    let outcome = match app.value_modal_mut() {
        Some(state) => state.handle_key(key),
        None => return,
    };
    if let ValueModalOutcome::Close = outcome {
        app.close_value_modal();
    }
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
    let pin_create_defaults = app.controls_context().pin_create_defaults;
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
        pin_create_defaults,
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
    match action {
        crate::tui::widgets::controls::ControlsAction::CreatePin(request) => {
            create_pin_action(app, config, request);
        }
        other => {
            app.apply_controls_action(other);
            refresh(app, config);
        }
    }
}

fn create_pin_action(
    app: &mut App,
    config: &RunConfig,
    request: crate::tui::widgets::controls::PinCreateRequest,
) {
    match write_pin_create(&request, &crate::config::ConfigLoader::from_env()) {
        Ok((outcome, entry, store_kind)) => {
            let verb = if outcome.changed {
                if outcome.entry_count == 1 {
                    "wrote"
                } else {
                    "updated"
                }
            } else {
                "unchanged"
            };
            refresh(app, config);
            app.update(Msg::SetStatus(Some(format!(
                "{verb} pin `{}` in {} ({})",
                entry.id,
                outcome.path.display(),
                pin_store_label(store_kind)
            ))));
        }
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("pin create failed: {err}"))));
        }
    }
}

fn write_pin_create(
    request: &crate::tui::widgets::controls::PinCreateRequest,
    loader: &crate::config::ConfigLoader,
) -> Result<(PinWriteOutcome, PinEntry, PinStoreKind)> {
    let cwd = std::path::PathBuf::from(&request.cwd);
    let selection = match request.store {
        crate::tui::widgets::controls::PinCreateStore::Auto
        | crate::tui::widgets::controls::PinCreateStore::Project => {
            crate::pins::select_store_for_pin(&cwd, loader)?
        }
        crate::tui::widgets::controls::PinCreateStore::User => crate::pins::user_pin_store(loader)?,
    };
    let entry = PinEntry {
        id: request.id.clone(),
        display_name: request.display_name.clone(),
        harness: request.harness.clone(),
        cwd: request.cwd.clone(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: request.mux_name.clone(),
            socket_name: request.mux_socket.clone(),
        },
        launch: if request.launch_argv.is_empty() {
            None
        } else {
            Some(PinLaunch {
                argv: request.launch_argv.clone(),
            })
        },
        reason: None,
    };
    let outcome = crate::pins::upsert_pin_entry(&selection.path, entry.clone())?;
    Ok((outcome, entry, selection.kind))
}

fn pin_store_label(kind: PinStoreKind) -> &'static str {
    match kind {
        PinStoreKind::Project => "project",
        PinStoreKind::User => "user",
    }
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

/// Dispatch `Enter` on the left pane (T8-043) to the selected
/// row's default action. Group rows expand/collapse; mux rows and
/// muxed agent sessions attach; un-muxed agent sessions open the
/// transcript viewer. Right-pane focus is handled in
/// [`remap_for_focus`] before this is reached.
fn default_action(terminal: &mut DefaultTerminal, app: &mut App, config: &RunConfig) {
    match selected_default_action(app) {
        SelectedDefault::ToggleExpand => app.update(Msg::ToggleExpand),
        SelectedDefault::Attach => attach_action(terminal, app, config),
        SelectedDefault::View => view_action(terminal, app, config),
        SelectedDefault::LaunchPin => launch_pin_action(terminal, app, config),
    }
}

/// Handle `Enter` on a pin row (ADR 0057 / H-PIN-017). Suspends
/// the TUI, re-execs into `conspectus pin launch <id>` as a
/// subprocess so the launch logic stays in `cli::PinLaunchArgs`
/// without re-implementing it across the runtime, waits for the
/// nested process to exit (typically when the operator detaches
/// from tmux), then re-enters the alt screen and refreshes the
/// row tree.
fn launch_pin_action(terminal: &mut DefaultTerminal, app: &mut App, config: &RunConfig) {
    let Some(selection) = app.selection() else {
        app.update(Msg::SetStatus(Some("launch: nothing selected".to_string())));
        return;
    };
    let pin_id = match selection {
        crate::tui::rows::RowId::Pin { pin_id } => pin_id.clone(),
        _ => {
            app.update(Msg::SetStatus(Some(
                "launch: select an unbound pin row".to_string(),
            )));
            return;
        }
    };

    ratatui::restore();
    let status = std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("conspectus")),
    )
    .args(["pin", "launch", &pin_id])
    .status();
    *terminal = ratatui::init();
    let _ = terminal.clear();

    refresh(app, config);
    let message = match status {
        Ok(s) if s.success() => format!("pin `{pin_id}` launch finished"),
        Ok(s) => format!("pin `{pin_id}` launch exited with {s}"),
        Err(err) => format!("pin `{pin_id}` launch failed to spawn: {err}"),
    };
    app.update(Msg::SetStatus(Some(message)));
}

/// Right-pane Enter (T8-040 / T8-043). When the explorer cursor is on
/// a Node-zone field row with a copyable value, write it to the
/// clipboard via OSC 52 (ADR 0056) and post a toast. Otherwise
/// dispatch the normal `Msg::ExplorerActivate` so group expansion and
/// link drill still work.
fn explorer_enter_action(app: &mut App) {
    if let Some((label, value)) = app.explorer_copy_target() {
        copy_and_toast(app, format!("copied: {label}"), value);
        return;
    }
    app.update(Msg::ExplorerActivate);
}

/// `i` keybinding (T8-040). Copies the selected agent or mux session's
/// full id to the clipboard, posting a toast. Surfaces a status hint
/// when the selection is something else (a group row, a PR, …).
fn copy_session_id_action(app: &mut App) {
    match app.selected_session_id() {
        Some((label, value)) => copy_and_toast(app, label, value),
        None => app.update(Msg::SetStatus(Some(
            "i: select an agent or mux session row to copy its id".to_string(),
        ))),
    }
}

/// Shared copy-side-effect: write `value` to the clipboard via OSC 52
/// and post the `label` as a toast. Clipboard errors are silenced —
/// OSC 52 is fire-and-forget per ADR 0056, and the toast is the
/// operator-facing signal.
fn copy_and_toast(app: &mut App, label: String, value: String) {
    let _ = crate::tui::clipboard::copy_to_clipboard(&value);
    app.post_toast(label);
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

/// Handle the resume action: resolve the selected agent session's
/// resume command, launch it in the background, and surface the
/// outcome as a status-bar message.
fn resume_action(app: &mut App) {
    let Some(selection) = app.selection().cloned() else {
        app.update(Msg::SetStatus(Some("resume: nothing selected".to_string())));
        return;
    };
    let session_id = match &selection {
        RowId::AgentSession(crate::model::NodeId::AgentSession(id)) => id.clone(),
        _ => {
            app.update(Msg::SetStatus(Some(
                "resume: select an agent session".to_string(),
            )));
            return;
        }
    };
    let target = resolve_resume_target(&session_id);
    match &target {
        ResumeTarget::Launch { label, .. } => {
            if launch_resume(&target) {
                app.update(Msg::SetStatus(Some(format!("resumed: {label}"))));
            } else {
                app.update(Msg::SetStatus(Some("resume: failed to launch".to_string())));
            }
        }
        _ => {
            app.update(Msg::SetStatus(Some(resume_disabled_reason(&target))));
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

/// Handle the `T` key (H-VIEWER-NATIVE-008, ADR 0052). Resolve the
/// selected agent session through `viewer_bridge::build_viewer_state`
/// and open the native full-screen modal. Falls through to the
/// escape-hatch external launch (`H-TRANSCRIPT-012`) when the
/// harness has no native parser registered (currently: `aider`).
/// On disabled, set a status-bar message and stay in the TUI.
fn view_action(terminal: &mut DefaultTerminal, app: &mut App, config: &RunConfig) {
    // Resolve the session through the actions module so mux rows
    // open the viewer for their preferred linked session (T8-043
    // companion: `v` on a mux row is the inverse of `a` on a
    // session). Agent-session rows pass through unchanged.
    let session_id = match resolve_view_session(app) {
        Ok(id) => id,
        Err(reason) => {
            app.update(Msg::SetStatus(Some(viewer_disabled_reason(&reason))));
            return;
        }
    };

    // Native viewer is the default per ADR 0052.
    if let Some(state) = crate::tui::viewer_bridge::build_viewer_state(&session_id) {
        let label = format!("{}:{}", session_id.harness_key, session_id.session_key);
        app.open_viewer_modal(state);
        app.update(Msg::SetStatus(Some(format!("viewing {label}"))));
        return;
    }

    // Fallback: harness has no native parser. Honor the escape-hatch
    // external launcher (`claude-history` only, for now). Used by
    // `aider` and any other harness we add to the graph before its
    // viewer parser lands.
    let target = resolve_viewer_target(&session_id, &PathBinaryProbe);
    match target {
        ViewerTarget::Launch(plan) => {
            let outcome = run_viewer_launch(terminal, &plan);
            refresh(app, config);
            let message = match outcome {
                ViewerOutcome::Exited => format!("viewed: {}", plan.label),
                ViewerOutcome::Failed(reason) => format!("view failed: {reason}"),
            };
            app.update(Msg::SetStatus(Some(message)));
        }
        ViewerTarget::Disabled(reason) => {
            app.update(Msg::SetStatus(Some(viewer_disabled_reason(&reason))));
        }
    }
}

/// Translate a key event into a [`crate::viewer::input::ViewerMsg`],
/// run it through the pure reducer, and put the new state back on
/// `app` — unless the reducer's effect was `Close`, in which case
/// dismiss the modal. Keys that don't map are dropped silently
/// (the modal owns every keystroke while open).
fn handle_viewer_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::viewer::input::{ViewerEffect, ViewerMsg, reduce};
    let Some(state) = app.take_viewer_modal() else {
        return;
    };
    let msg =
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(ViewerMsg::Close),
            (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => Some(ViewerMsg::Close),
            (_, KeyCode::Char('j')) | (_, KeyCode::Down) => Some(ViewerMsg::ScrollDown),
            (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Some(ViewerMsg::ScrollUp),
            (_, KeyCode::PageDown) | (_, KeyCode::Char(' ')) => Some(ViewerMsg::PageDown),
            (_, KeyCode::PageUp) => Some(ViewerMsg::PageUp),
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => Some(ViewerMsg::HalfPageDown),
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => Some(ViewerMsg::HalfPageUp),
            (_, KeyCode::Char('g')) | (_, KeyCode::Home) => Some(ViewerMsg::JumpToStart),
            (KeyModifiers::SHIFT, KeyCode::Char('G'))
            | (KeyModifiers::NONE, KeyCode::Char('G'))
            | (_, KeyCode::End) => Some(ViewerMsg::JumpToEnd),
            (_, KeyCode::Char('t')) => Some(ViewerMsg::CycleToolDetail),
            (KeyModifiers::SHIFT, KeyCode::Char('T'))
            | (KeyModifiers::NONE, KeyCode::Char('T')) => Some(ViewerMsg::ToggleThinking),
            (KeyModifiers::SHIFT, KeyCode::Char('I'))
            | (KeyModifiers::NONE, KeyCode::Char('I')) => Some(ViewerMsg::ToggleAborted),
            (_, KeyCode::Char('?')) => Some(ViewerMsg::ToggleHelp),
            _ => None,
        };
    let Some(msg) = msg else {
        app.open_viewer_modal(state);
        return;
    };
    let (next, effect) = reduce(state, msg);
    match effect {
        ViewerEffect::Close => {
            app.close_viewer_modal();
            app.update(Msg::SetStatus(Some("viewer closed".to_string())));
        }
        ViewerEffect::None => {
            app.open_viewer_modal(next);
        }
    }
}

/// Outcome of a single viewer launch attempt. Errors carry a
/// human-readable reason for the status bar, parallel to
/// [`AttachOutcome`].
#[derive(Debug)]
enum ViewerOutcome {
    /// The viewer ran and exited (operator closed it, viewer
    /// returned successfully, etc.).
    Exited,
    /// We never got to the viewer cleanly, or it exited non-zero
    /// with a message worth surfacing.
    Failed(String),
}

/// Leave the alt screen + raw mode, spawn the viewer inheriting the
/// parent terminal, wait for it to exit, then re-enter the alt
/// screen. Mirrors `run_tmux_attach` with two viewer-specific
/// tweaks:
///   1. Explicit `Clear(All)` + cursor-to-origin after restoring the
///      terminal. Plain `ratatui::restore()` is enough for local
///      terminals but on mosh / nested muxers the LeaveAlternateScreen
///      sequence can be coalesced with the child's first writes,
///      leaving the viewer's output overlaid on the dropped TUI
///      buffer. The clear forces a clean canvas.
///   2. On non-zero exit, hold for Enter before re-entering the alt
///      screen so the operator can read whatever the viewer
///      printed to stderr (e.g. `recall`'s "Session not found")
///      instead of having it wiped by the re-render.
fn run_viewer_launch(terminal: &mut DefaultTerminal, plan: &LaunchPlan) -> ViewerOutcome {
    use ratatui::crossterm::{
        cursor::MoveTo,
        execute,
        terminal::{Clear, ClearType},
    };

    ratatui::restore();
    let _ = execute!(std::io::stdout(), Clear(ClearType::All), MoveTo(0, 0));

    let status = std::process::Command::new(&plan.program)
        .args(&plan.args)
        .status();

    let outcome = match status {
        Ok(s) if s.success() => ViewerOutcome::Exited,
        Ok(s) => ViewerOutcome::Failed(format!("{} exited with {s}", plan.program)),
        Err(err) => ViewerOutcome::Failed(format!("could not launch {}: {err}", plan.program)),
    };

    if matches!(outcome, ViewerOutcome::Failed(_)) {
        eprintln!("\n[viewer exited non-zero — press Enter to return to conspectus]");
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
    }

    *terminal = ratatui::init();
    let _ = terminal.clear();
    outcome
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
            // T8-028: j/k drive the explorer cursor when the right
            // pane has focus, replacing the prior raw preview-scroll
            // remap. Uppercase J/K still scroll the preview.
            Msg::NavDown => Msg::ExplorerNavDown,
            Msg::NavUp => Msg::ExplorerNavUp,
            Msg::PageDown(_) => Msg::ExplorerNavDown,
            Msg::PageUp(_) => Msg::ExplorerNavUp,
            // `e` toggles group expansion; on a non-header row the
            // reducer surfaces a status hint.
            Msg::ToggleLinkedDetails => Msg::ExplorerToggleGroup,
            other => other,
        })),
        // T8-040 / T8-043: Enter on the explorer cursor either
        // copies a Node-zone field value (T8-040) or expands a
        // group header / drills into a link row (T8-043). The
        // dispatcher inspects the cursor row at action time, so
        // remap to [`Action::ExplorerEnter`] and let the main
        // loop branch with App state in hand. Left-pane Enter
        // (DefaultAction) is dispatched against the selected
        // row's kind separately.
        Action::DefaultAction => Action::ExplorerEnter,
        // T8-034: `F` toggles the Expanded Node Detail view when the
        // right pane is focused. The same key still clears filters
        // when the left tree has focus (ADR 0031).
        Action::ClearFilters => Action::Msg(Box::new(Msg::ExplorerToggleFullDetail)),
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
            (KeyModifiers::SHIFT, KeyCode::Char('S'))
            | (KeyModifiers::NONE, KeyCode::Char('S')) => Some(Action::Resume),
            (KeyModifiers::SHIFT, KeyCode::Char('R'))
            | (KeyModifiers::NONE, KeyCode::Char('R')) => Some(Action::OpenRename),
            (_, KeyCode::Delete) => Some(Action::RemovePin),
            (m, KeyCode::Char('b')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::PinBindHint)
            }
            // ADR 0031 / F8-005 accelerator surface (reshuffled
            // alongside H-VIEWER-NATIVE-008 to give the more
            // discoverable `v` to the session viewer):
            //   `v` opens the session transcript viewer (was `T`).
            //   `f` opens the controls overlay (was `v`).
            //   `F` clears every active filter (unchanged).
            //   `1`–`5` switch view; `]`/`[` cycle views;
            //   `Ctrl-G` cycles grouping.
            // The pre-existing `f` → "jump to Filters section"
            // shortcut was retired; the controls overlay places
            // the cursor at the top and the operator navigates
            // from there.
            (m, KeyCode::Char('v')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::View),
            (m, KeyCode::Char('f')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenControls)
            }
            (KeyModifiers::SHIFT, KeyCode::Char('F'))
            | (KeyModifiers::NONE, KeyCode::Char('F')) => Some(Action::ClearFilters),
            // T8-042: `E` toggles the explorer's edge-meta visibility
            // (provenance · confidence · state on link rows).
            // Focus-agnostic: the meta visibility is a global UI
            // preference that applies to the right pane regardless
            // of which pane currently has focus.
            (KeyModifiers::SHIFT, KeyCode::Char('E'))
            | (KeyModifiers::NONE, KeyCode::Char('E')) => {
                Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
            }
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
            (m, KeyCode::Char('?')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::OpenHelp),
            (_, KeyCode::Char('j')) | (_, KeyCode::Down) => {
                Some(Action::Msg(Box::new(Msg::NavDown)))
            }
            (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Some(Action::Msg(Box::new(Msg::NavUp))),
            // Vi-style tree expand/collapse on the left pane. `l` /
            // `→` open the selected row's children, `h` / `←`
            // collapse them. Enter is still the default-action key
            // (T8-043); these bindings give the operator an explicit
            // expand/collapse path now that Enter no longer plays
            // that role for every row kind.
            (m, KeyCode::Char('l')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::Msg(Box::new(Msg::ExpandRow)))
            }
            (m, KeyCode::Char('h')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::Msg(Box::new(Msg::CollapseRow)))
            }
            (_, KeyCode::Right) => Some(Action::Msg(Box::new(Msg::ExpandRow))),
            (_, KeyCode::Left) => Some(Action::Msg(Box::new(Msg::CollapseRow))),
            (_, KeyCode::PageDown) => Some(Action::Msg(Box::new(Msg::PageDown(viewport_height)))),
            (_, KeyCode::PageUp) => Some(Action::Msg(Box::new(Msg::PageUp(viewport_height)))),
            (_, KeyCode::Home) | (_, KeyCode::Char('g')) => Some(Action::Msg(Box::new(Msg::Home))),
            (_, KeyCode::End) | (_, KeyCode::Char('G')) => Some(Action::Msg(Box::new(Msg::End))),
            // T8-043: `Enter` resolves to the selected row's default
            // action when the left pane has focus (attach mux rows,
            // view un-muxed sessions, expand/collapse groups). Right
            // pane focus is remapped to `ExplorerActivate` in
            // `remap_for_focus`.
            (_, KeyCode::Enter) => Some(Action::DefaultAction),
            // Backspace on the explorer pops a drilldown hop. The
            // reducer no-ops on left focus / empty stack and surfaces
            // a status hint when appropriate.
            (_, KeyCode::Backspace) => Some(Action::Msg(Box::new(Msg::ExplorerBack))),
            (m, KeyCode::Char('e')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::Msg(Box::new(Msg::ToggleLinkedDetails)))
            }
            // T8-030: `o` opens the full-value modal on the cursor
            // row. The reducer no-ops gracefully if the cursor isn't
            // on a row with a truncated value.
            (m, KeyCode::Char('o')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenValueModal)
            }
            // T8-040: `i` copies the selected agent or mux session's
            // full id to the clipboard via OSC 52 (ADR 0056) and
            // posts a toast. The runtime branches on selection kind;
            // a non-session row surfaces a status hint instead.
            (m, KeyCode::Char('i')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::CopySessionId)
            }
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
    use crate::tui::widgets::controls::{PinCreateRequest, PinCreateStore};
    use ratatui::crossterm::event::KeyEvent;
    use std::fs;

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
    fn translate_delete_requests_pin_remove() {
        assert_eq!(
            translate(press(KeyCode::Delete, KeyModifiers::NONE), 24),
            Some(Action::RemovePin)
        );
    }

    #[test]
    fn translate_b_requests_pin_bind_hint() {
        assert_eq!(
            translate(press(KeyCode::Char('b'), KeyModifiers::NONE), 24),
            Some(Action::PinBindHint)
        );
    }

    #[test]
    fn translate_i_requests_copy_session_id() {
        // T8-040: `i` resolves the selected agent or mux session's
        // full id and routes it through the OSC 52 clipboard
        // primitive (ADR 0056) at the main-loop boundary.
        assert_eq!(
            translate(press(KeyCode::Char('i'), KeyModifiers::NONE), 24),
            Some(Action::CopySessionId)
        );
        // Ctrl-I is the terminal alias for Tab; the binding must not
        // claim it.
        assert_ne!(
            translate(press(KeyCode::Char('i'), KeyModifiers::CONTROL), 24),
            Some(Action::CopySessionId)
        );
    }

    #[test]
    fn translate_v_opens_viewer() {
        assert_eq!(
            translate(press(KeyCode::Char('v'), KeyModifiers::NONE), 24),
            Some(Action::View)
        );
    }

    #[test]
    fn translate_f_opens_controls_overlay() {
        assert_eq!(
            translate(press(KeyCode::Char('f'), KeyModifiers::NONE), 24),
            Some(Action::OpenControls)
        );
    }

    #[test]
    fn translate_upper_t_no_longer_bound_to_view() {
        // Post H-VIEWER-NATIVE-008 reshuffle: `v` owns View;
        // `T` is unbound and falls through to None.
        assert_eq!(
            translate(press(KeyCode::Char('T'), KeyModifiers::NONE), 24),
            None,
        );
        assert_eq!(
            translate(press(KeyCode::Char('T'), KeyModifiers::SHIFT), 24),
            None,
        );
    }

    #[test]
    fn static_action_routes_keys_to_controls_overlay() {
        let mut app = App::new(RunConfig::defaults());
        app.open_controls_overlay();

        assert_eq!(
            static_action_for_event(&app, press(KeyCode::Down, KeyModifiers::NONE), 24),
            Some(Action::ControlsOverlayKey(KeyEvent::new(
                KeyCode::Down,
                KeyModifiers::NONE
            )))
        );
        assert_eq!(
            static_action_for_event(&app, press(KeyCode::Char('q'), KeyModifiers::NONE), 24),
            Some(Action::ControlsOverlayKey(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE
            )))
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
    fn write_pin_create_writes_project_store() {
        let home = tempfile::TempDir::new().expect("home");
        let project = tempfile::TempDir::new().expect("project");
        let loader = crate::config::ConfigLoader::new()
            .with_home(home.path())
            .with_xdg_config_home(home.path().join(".config"));
        let request = PinCreateRequest {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux_name: "ingest-mux".to_string(),
            mux_socket: Some("scratch".to_string()),
            launch_argv: vec!["codex".to_string(), "--resume".to_string()],
            store: PinCreateStore::Project,
        };

        let (outcome, entry, kind) = write_pin_create(&request, &loader).expect("write pin");
        assert!(outcome.changed);
        assert_eq!(kind, PinStoreKind::Project);
        assert_eq!(entry.id, "ingest");
        assert_eq!(entry.mux.native_id(), "tmux:scratch:ingest-mux");
        assert_eq!(outcome.path, project.path().join(".conspectus.toml"));

        let written = fs::read_to_string(&outcome.path).expect("project config");
        assert!(written.contains("[pins]"));
        assert!(written.contains(r#"id = "ingest""#));
        assert!(written.contains(r#"display_name = "Ingest""#));
        assert!(written.contains(r#"harness = "codex""#));
        assert!(written.contains(r#"name = "ingest-mux""#));
        assert!(written.contains(r#"socket_name = "scratch""#));
        assert!(written.contains("argv = ["));
        assert!(written.contains(r#""codex""#));
        assert!(written.contains(r#""--resume""#));
    }

    #[test]
    fn write_pin_create_user_store_uses_user_config_path() {
        let home = tempfile::TempDir::new().expect("home");
        let xdg = home.path().join(".config");
        let project = tempfile::TempDir::new().expect("project");
        let loader = crate::config::ConfigLoader::new()
            .with_home(home.path())
            .with_xdg_config_home(&xdg);
        let request = PinCreateRequest {
            id: "scratch".to_string(),
            display_name: "Scratch".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux_name: "scratch".to_string(),
            mux_socket: None,
            launch_argv: Vec::new(),
            store: PinCreateStore::User,
        };

        let (outcome, _, kind) = write_pin_create(&request, &loader).expect("write user pin");
        assert_eq!(kind, PinStoreKind::User);
        assert_eq!(outcome.path, xdg.join(crate::config::USER_CONFIG_RELATIVE));
        assert!(outcome.path.exists());
        assert!(!project.path().join(".conspectus.toml").exists());
    }

    #[test]
    fn translate_shift_e_toggles_edge_meta() {
        // T8-042: `E` toggles the explorer's edge-meta visibility.
        // Focus-agnostic — the binding is global so the operator
        // can flip it without first tabbing into the right pane.
        assert_eq!(
            translate(press(KeyCode::Char('E'), KeyModifiers::SHIFT), 24),
            Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
        );
        assert_eq!(
            translate(press(KeyCode::Char('E'), KeyModifiers::NONE), 24),
            Some(Action::Msg(Box::new(Msg::ToggleEdgeMeta)))
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
        // T8-043: Enter no longer maps to a Msg directly. It is
        // resolved against the selected row's kind by the dispatcher
        // at the call site (and remapped to ExplorerActivate when
        // the right pane has focus).
        assert_eq!(
            translate(press(KeyCode::Enter, KeyModifiers::NONE), 24),
            Some(Action::DefaultAction)
        );
        // Vi-style tree fold keys: `l` / `→` expand, `h` / `←`
        // collapse the selected left-tree row.
        assert_eq!(
            msg(translate(press(KeyCode::Char('l'), KeyModifiers::NONE), 24)),
            Some(Msg::ExpandRow)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Right, KeyModifiers::NONE), 24)),
            Some(Msg::ExpandRow)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Char('h'), KeyModifiers::NONE), 24)),
            Some(Msg::CollapseRow)
        );
        assert_eq!(
            msg(translate(press(KeyCode::Left, KeyModifiers::NONE), 24)),
            Some(Msg::CollapseRow)
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
    fn remap_for_focus_right_routes_nav_keys_into_the_explorer() {
        use crate::tui::app::Focus;
        // T8-028: with the right pane focused, j/k and PageUp/Down
        // move the explorer cursor instead of scrolling the preview.
        // J/K (uppercase) keep their preview-scroll role via the
        // standalone bindings in `translate`.
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::NavDown)), Focus::Right),
            Action::Msg(Box::new(Msg::ExplorerNavDown))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::NavUp)), Focus::Right),
            Action::Msg(Box::new(Msg::ExplorerNavUp))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageDown(20))), Focus::Right),
            Action::Msg(Box::new(Msg::ExplorerNavDown))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageUp(20))), Focus::Right),
            Action::Msg(Box::new(Msg::ExplorerNavUp))
        );
    }

    #[test]
    fn remap_for_focus_right_routes_enter_and_e_to_the_explorer() {
        use crate::tui::app::Focus;
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
            Action::Msg(Box::new(Msg::CycleFocus))
        );
        // Locked decision 8: Enter is the universal "do the obvious
        // thing" key on the explorer cursor. T8-040 made the
        // dispatch App-aware (Node-zone fields copy; other rows
        // drill/expand), so the remap produces the new
        // [`Action::ExplorerEnter`] variant that the main loop
        // resolves against [`App::explorer_copy_target`].
        assert_eq!(
            remap_for_focus(Action::DefaultAction, Focus::Right),
            Action::ExplorerEnter
        );
        // Left-pane DefaultAction is left untouched here so the main
        // loop can resolve it against the selected row.
        assert_eq!(
            remap_for_focus(Action::DefaultAction, Focus::Left),
            Action::DefaultAction
        );
        // `e` is the explicit expand/collapse accelerator.
        assert_eq!(
            remap_for_focus(
                Action::Msg(Box::new(Msg::ToggleLinkedDetails)),
                Focus::Right
            ),
            Action::Msg(Box::new(Msg::ExplorerToggleGroup))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::Quit)), Focus::Right),
            Action::Msg(Box::new(Msg::Quit))
        );
        assert_eq!(
            remap_for_focus(Action::Refresh, Focus::Right),
            Action::Refresh
        );
        // T8-034: F clears filters on the left tree but toggles the
        // Expanded Node Detail view on the right pane.
        assert_eq!(
            remap_for_focus(Action::ClearFilters, Focus::Left),
            Action::ClearFilters
        );
        assert_eq!(
            remap_for_focus(Action::ClearFilters, Focus::Right),
            Action::Msg(Box::new(Msg::ExplorerToggleFullDetail))
        );
    }

    mod selected_default_action_tests {
        use super::*;
        use crate::filter::RowFilter;
        use crate::model::{
            AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, GraphLink,
            GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
            NodeId, Provenance, RelationKind, RepoId, RepoNode,
        };
        use crate::resolve::resolve_snapshot;
        use crate::tui::SessionsGrouping;
        use crate::tui::app::{GraphDb, Msg};
        use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};

        fn session_node(harness: &str, scope: &str, key: &str, cwd: &str) -> GraphNode {
            GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(harness, scope, key),
                harness_key: harness.to_string(),
                cwd: Some(cwd.to_string()),
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            })
        }

        fn mux_node(backend: &str, native: &str) -> GraphNode {
            GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new(format!("{backend}:{native}")),
                backend: backend.to_string(),
                native_id: native.to_string(),
                cwd: None,
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                client_attached: None,
                activity_epoch: None,
                created_epoch: None,
            })
        }

        fn linked_to_mux(session: &NodeId, mux: &NodeId, suffix: &str) -> GraphLink {
            GraphLink {
                id: format!("session-mux-{suffix}"),
                source: session.clone(),
                target: LinkEndpoint::Node { id: mux.clone() },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: crate::model::Freshness::Fresh,
                source_metadata: crate::model::SourceMetadata::default(),
                state: LinkState::Active,
            }
        }

        fn add_repo_and_worktree(snapshot: &mut GraphSnapshot, common_dir: &str) {
            snapshot
                .nodes
                .push(GraphNode::Repo(RepoNode::new(RepoId::new(common_dir))));
            snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(RepoId::new(common_dir), common_dir.to_string()),
                root: common_dir.to_string(),
                git_dir: None,
                current_branch: None,
            }));
        }

        fn build_sessions_app(snapshot: GraphSnapshot) -> App {
            let snapshot = resolve_snapshot(snapshot);
            let tree = build_sessions_tree(SessionsBuildInputs {
                snapshot: &snapshot,
                grouping: SessionsGrouping::Graph,
                home: None,
                now: None,
                cwd: None,
                filter: RowFilter::default(),
            });
            let mut cfg = RunConfig::defaults();
            cfg.default_view = View::Sessions;
            let mut app = App::new(cfg);
            app.update(Msg::SetData {
                snapshot: GraphDb::from_snapshot(&snapshot),
                tree,
                loaded_at_epoch: 1_700_000_000,
                initial_selection_hint: None,
            });
            app
        }

        #[test]
        fn empty_selection_falls_back_to_toggle_expand() {
            let app = App::new(RunConfig::defaults());
            assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
        }

        #[test]
        fn group_row_resolves_to_toggle_expand() {
            let mut snapshot = GraphSnapshot::empty();
            add_repo_and_worktree(&mut snapshot, "/p/proj");
            snapshot
                .nodes
                .push(session_node("codex", "/state", "abc", "/p/proj"));
            // Auto-selection lands on the repo group row.
            let app = build_sessions_app(snapshot);
            assert_eq!(selected_default_action(&app), SelectedDefault::ToggleExpand);
        }

        #[test]
        fn unmuxed_session_resolves_to_view() {
            let mut snapshot = GraphSnapshot::empty();
            add_repo_and_worktree(&mut snapshot, "/p/proj");
            snapshot
                .nodes
                .push(session_node("codex", "/state", "abc", "/p/proj"));
            let mut app = build_sessions_app(snapshot);
            // Step past the group row to the session row.
            app.update(Msg::NavDown);
            assert_eq!(selected_default_action(&app), SelectedDefault::View);
        }

        #[test]
        fn muxed_session_resolves_to_attach() {
            let mut snapshot = GraphSnapshot::empty();
            add_repo_and_worktree(&mut snapshot, "/p/proj");
            snapshot
                .nodes
                .push(session_node("codex", "/state", "abc", "/p/proj"));
            snapshot.nodes.push(mux_node("tmux", "editor"));
            let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
            let mux_id = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
            snapshot
                .candidate_links
                .push(linked_to_mux(&session_id, &mux_id, "1"));
            let mut app = build_sessions_app(snapshot);
            app.update(Msg::NavDown);
            assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
        }

        #[test]
        fn mux_candidate_child_resolves_to_attach() {
            let mut snapshot = GraphSnapshot::empty();
            add_repo_and_worktree(&mut snapshot, "/p/proj");
            snapshot
                .nodes
                .push(session_node("codex", "/state", "abc", "/p/proj"));
            snapshot.nodes.push(mux_node("tmux", "editor"));
            snapshot.nodes.push(mux_node("tmux", "scratch"));
            let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
            let editor = NodeId::MuxSession(MuxSessionId::new("tmux:editor"));
            let scratch = NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
            snapshot
                .candidate_links
                .push(linked_to_mux(&session_id, &editor, "1"));
            snapshot
                .candidate_links
                .push(linked_to_mux(&session_id, &scratch, "2"));
            let mut app = build_sessions_app(snapshot);
            // Group → ambiguous session → expand → candidate child.
            app.update(Msg::NavDown);
            app.update(Msg::ToggleExpand);
            app.update(Msg::NavDown);
            assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
        }

        #[test]
        fn mux_view_mux_row_resolves_to_attach() {
            let mut snapshot = GraphSnapshot::empty();
            snapshot.nodes.push(mux_node("tmux", "editor"));
            let snapshot = resolve_snapshot(snapshot);
            let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize snapshot");
            let tree = crate::tui::rows::mux::build_mux_tree_from_conn(
                crate::tui::rows::mux::MuxBuildInputsFromConn {
                    conn: &conn,
                    home: None,
                    now: None,
                    filter: RowFilter::default(),
                    grouping: crate::tui::MuxGrouping::Session,
                },
            )
            .expect("build mux tree");
            let mut cfg = RunConfig::defaults();
            cfg.default_view = View::Mux;
            let mut app = App::new(cfg);
            app.update(Msg::SetData {
                snapshot: GraphDb::new(conn),
                tree,
                loaded_at_epoch: 1_700_000_000,
                initial_selection_hint: None,
            });
            assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
        }

        #[test]
        fn unbound_pin_row_resolves_to_launch_pin() {
            use crate::model::{PinBinding, PinCandidate, PinMuxRef, Provenance};

            let mut snapshot = GraphSnapshot::empty();
            snapshot.pins.push(PinCandidate {
                id: "ingest".to_string(),
                display_name: "ingest".to_string(),
                harness: "codex".to_string(),
                cwd: "/p/proj".to_string(),
                mux: PinMuxRef {
                    backend: "tmux".to_string(),
                    name: "ingest".to_string(),
                    socket_name: None,
                },
                launch_argv: None,
                reason: None,
                provenance: Provenance::LocalPin,
                store_path: "/tmp/.conspectus.toml".to_string(),
                binding: Some(PinBinding::Unbound),
            });
            // Auto-selection lands on the first row of the synthetic
            // "Pins" group (the group header). Walk the tree to find
            // the actual pin row id and select it so the dispatch
            // exercises `RowKind::Pin` instead of the group header.
            let mut app = build_sessions_app(snapshot);
            let pin_row_id = app
                .tree()
                .rows
                .iter()
                .find(|r| matches!(&r.id, crate::tui::rows::RowId::Pin { .. }))
                .map(|r| r.id.clone())
                .expect("pin row emitted");
            app.set_selection(pin_row_id);
            assert_eq!(selected_default_action(&app), SelectedDefault::LaunchPin);
        }
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
