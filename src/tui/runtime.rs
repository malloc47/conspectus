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

use anyhow::{Result, bail};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

use crate::discovery::tmux::{SystemTmux, TmuxRunner};
use crate::model::{GraphSnapshot, MuxSessionId};
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
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
use crate::tui::viewer::{
    LaunchPlan, PathBinaryProbe, ViewerTarget, resolve_viewer_target, viewer_disabled_reason,
};
use crate::tui::{RunConfig, View, ui};

/// Result of a completed background discovery run. The worker returns
/// only the resolved snapshot; the main thread builds the row tree from
/// the app's current view config.
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
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub fn run_static(config: RunConfig, snapshot: crate::model::GraphSnapshot) -> Result<()> {
    let mut terminal = ratatui::init();
    let _ = terminal.clear();
    let result = static_event_loop(&mut terminal, config, snapshot, None);
    ratatui::restore();
    result
}

/// Run an interactive TUI session against a fixture file on disk
/// (ADR 0069). Same code path as [`run_static`] but the `r`
/// (Refresh) accelerator re-reads the JSON from disk so the
/// operator can edit the fixture in another buffer and cycle in
/// the new state without leaving the session.
#[cfg(feature = "snapshot")]
pub fn run_from_fixture(config: RunConfig, fixture_path: std::path::PathBuf) -> Result<()> {
    use anyhow::Context as _;
    let snapshot = read_fixture(&fixture_path)
        .with_context(|| format!("load fixture `{}`", fixture_path.display()))?;
    let mut terminal = ratatui::init();
    let _ = terminal.clear();
    let result = static_event_loop(&mut terminal, config, snapshot, Some(fixture_path));
    ratatui::restore();
    result
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn read_fixture(path: &std::path::Path) -> Result<crate::model::GraphSnapshot> {
    let raw = std::fs::read_to_string(path)?;
    let snapshot: crate::model::GraphSnapshot = serde_json::from_str(&raw)?;
    Ok(resolve_snapshot(snapshot))
}

/// Block on terminal input, dispatching crossterm events to the
/// pure reducer until the app signals quit.
fn event_loop(terminal: &mut DefaultTerminal, config: RunConfig) -> Result<()> {
    let mut app = App::new(config.clone());
    // F8-013: enable last-active-view persistence. Snapshot mode and
    // `--no-resume-view` both go through a different entry point
    // (or skip this branch) so the file only ever moves under
    // genuine interactive runs.
    app.enable_view_persistence(crate::tui_state::TuiStateCache::from_env());
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
        // H-WIDG-003: refresh the toast engine's area + tick the
        // expiry timer before each render. set_area handles
        // terminal resize; tick retires any toast past its
        // duration so the next render reflects the polled state
        // the prior in-tree `is_expired()` provided.
        let size = terminal.size()?;
        app.prepare_toast_for_render(Rect::new(0, 0, size.width, size.height));
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
                Ok(snapshot) => {
                    let live_config = app.config().clone();
                    let tree = build_tree_for_view(&snapshot, &live_config);
                    let database = GraphDb::new(snapshot);
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
            } else if app.pins_overlay().is_some() {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        Some(Action::PinsOverlayKey(key))
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
                translate(event, viewport).and_then(|a| remap_for_focus(a, app.focus()))
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
                Some(Action::OpenPinCreate) => open_pin_create_action(&mut app),
                Some(Action::OpenPinRebind) => open_pin_rebind_action(&mut app),
                Some(Action::OpenPinAdopt) => open_pin_adopt_action(&mut app),
                Some(Action::LaunchPin) => launch_pin_action(terminal, &mut app, &config),
                Some(Action::OpenControls) => {
                    app.open_controls_overlay();
                    app.update(Msg::SetStatus(Some(
                        "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::ControlsOverlayKey(key)) => {
                    handle_controls_overlay_key(&mut app, &config, key)
                }
                Some(Action::OpenPins) => {
                    app.open_pins_overlay();
                    app.update(Msg::SetStatus(Some(
                        "pins: ↑/↓ move · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::PinsOverlayKey(key)) => {
                    handle_pins_overlay_key(terminal, &mut app, &config, key)
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

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_event_loop(
    terminal: &mut DefaultTerminal,
    config: RunConfig,
    initial_snapshot: crate::model::GraphSnapshot,
    fixture_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut snapshot = initial_snapshot;
    let mut app = App::new(config.clone());
    // F8-013: enable last-active-view persistence for the static
    // fixture-replay TUI mode too. Snapshot mode (ADR 0067) uses a
    // distinct entry point in `src/tui/snapshot.rs` that
    // deliberately skips this so the on-disk file stays
    // unconditionally unmoved when the snapshot tooling runs.
    app.enable_view_persistence(crate::tui_state::TuiStateCache::from_env());
    set_static_data(&mut app, &config, &snapshot)?;
    let tmux: Box<dyn TmuxRunner> = Box::new(SystemTmux::new());
    refresh_mux_preview_if_needed(&mut app, &config, tmux.as_ref(), None);
    let poll_timeout = Duration::from_millis(100);

    while !app.should_quit() {
        // H-WIDG-003: refresh the toast engine's area + tick the
        // expiry timer before each render. set_area handles
        // terminal resize; tick retires any toast past its
        // duration so the next render reflects the polled state
        // the prior in-tree `is_expired()` provided.
        let size = terminal.size()?;
        app.prepare_toast_for_render(Rect::new(0, 0, size.width, size.height));
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
                Some(Action::OpenPins) => {
                    app.open_pins_overlay();
                    app.update(Msg::SetStatus(Some(
                        "pins: ↑/↓ move · Enter pick · Esc close".to_string(),
                    )));
                }
                Some(Action::PinsOverlayKey(key)) => {
                    static_handle_pins_overlay_key(&mut app, &config, &snapshot, key)?;
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
                    if let Some(path) = fixture_path.as_deref() {
                        // ADR 0069: in fixture mode, `r` re-reads the
                        // file on disk so the operator can edit the
                        // JSON and cycle in the new state without
                        // leaving the session. Parse errors land in
                        // the status bar; the previously loaded
                        // fixture stays active.
                        match read_fixture(path) {
                            Ok(fresh) => {
                                snapshot = fresh;
                                set_static_data(&mut app, &config, &snapshot)?;
                                app.update(Msg::SetStatus(Some(format!(
                                    "fixture reloaded from {}",
                                    path.display()
                                ))));
                            }
                            Err(err) => {
                                app.update(Msg::SetStatus(Some(format!(
                                    "fixture reload failed ({}): {err}",
                                    path.display()
                                ))));
                            }
                        }
                    } else {
                        set_static_data(&mut app, &config, &snapshot)?;
                        app.update(Msg::SetStatus(Some(
                            "scenario snapshot reloaded".to_string(),
                        )));
                    }
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
                Some(Action::OpenPinCreate) => open_pin_create_action(&mut app),
                Some(Action::OpenPinRebind) => open_pin_rebind_action(&mut app),
                Some(Action::OpenPinAdopt) => open_pin_adopt_action(&mut app),
                Some(Action::LaunchPin) => {
                    app.update(Msg::SetStatus(Some(
                        "scenario TUI is static; pin launch is disabled".to_string(),
                    )));
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

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
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
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
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

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_apply_controls_action_and_refresh(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    action: crate::tui::widgets::controls::ControlsAction,
) -> Result<()> {
    app.apply_controls_action(action);
    set_static_data(app, config, snapshot)
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_apply_pins_action_and_refresh(
    app: &mut App,
    _config: &RunConfig,
    _snapshot: &crate::model::GraphSnapshot,
    action: crate::tui::widgets::pins::PinsAction,
) -> Result<()> {
    use crate::tui::widgets::pins::PinsAction;
    match action {
        PinsAction::CreatePin(_)
        | PinsAction::EditPin(_)
        | PinsAction::BindPin(_)
        | PinsAction::RemovePin(_)
        | PinsAction::LaunchPin { .. } => {
            app.update(Msg::SetStatus(Some(
                "scenario TUI keeps mutating actions disabled".to_string(),
            )));
        }
        PinsAction::PinPlaceholder(_) => {
            app.apply_pins_action(action);
        }
    }
    Ok(())
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn set_static_data(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
) -> Result<()> {
    let tree = build_tree_for_view(snapshot, app.config());
    let database = GraphDb::new(snapshot.clone());
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

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
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
    if app.pins_overlay().is_some() {
        return match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => Some(Action::PinsOverlayKey(key)),
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
    translate(event, viewport).and_then(|action| remap_for_focus(action, app.focus()))
}

/// Spawn a background thread that runs discovery and sends the resolved
/// snapshot through `tx`. Row-tree building stays on the main thread so
/// it can use the app's current view state.
fn spawn_discovery_worker(config: &RunConfig, tx: &mpsc::Sender<DiscoveryResult>) {
    let config = config.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let result = discover_and_resolve(&config);
        let _ = tx.send(result);
    });
}

/// Run discovery and resolver on the calling thread, returning the
/// resolved snapshot.
///
/// P11-007 cutover: when `conspectus serve` is reachable on the
/// socket, this short-circuits and pulls the already-resolved
/// snapshot from the daemon via `client_snapshot` — a single
/// IPC round-trip instead of a full discovery + resolve cycle.
/// `RunConfig::refresh` forces the local discovery path even
/// with a daemon up (the operator typically uses `--refresh` to
/// bypass cache effects and the daemon's TTL gates apply to its
/// own ticks, not to the caller's intent). When the daemon is
/// absent or the snapshot is malformed, falls back to the
/// pre-cutover local discovery path verbatim — preserving ADR
/// 0038's "absence of a server is not an error" guarantee for
/// the TUI surface.
///
/// P7-003 phase 4: the local-discovery branch reads the
/// persisted cache and skips re-running heavy providers whose
/// class TTL has not expired. The resolved snapshot is persisted
/// back at the end of each cycle (unless `RunConfig::no_cache` is
/// set) so a peer one-shot CLI invocation in another shell also
/// benefits from the freshest data. `RunConfig::refresh` collapses
/// the prior to empty for a forced cold scan.
pub(super) fn discover_and_resolve(config: &RunConfig) -> Result<crate::model::GraphSnapshot> {
    if !config.refresh
        && let Some(snapshot) = try_daemon_snapshot()
    {
        return Ok(snapshot);
    }
    let roots: Vec<PathBuf> = if config.scan_roots.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        config.scan_roots.clone()
    };
    // P11-011a: the on-disk warm-start prior was the previous
    // graph.sqlite. With graph.sqlite retired, the daemonless
    // cold-rebuild path runs every provider from scratch each
    // tick — same as the daemon does on first cycle. Discovery
    // is single-digit seconds at target scale (ADR 0082), and
    // the TUI's typical setup runs `conspectus serve` so the
    // daemon-snapshot short-circuit above is the common path.
    let discovery_config = crate::discovery::LocalDiscoveryConfig::from_env();
    let snapshot = crate::discovery::discover_local_warm_with(
        roots,
        discovery_config,
        crate::model::GraphSnapshot::empty(),
        &config.intervals,
    )?;
    let snapshot = resolve_snapshot(snapshot);
    if !config.no_cache {
        let path = crate::snapshot::graph_bin_path();
        if let Err(err) = crate::snapshot::write_atomic(&path, &snapshot) {
            // Avoid stderr spam during a live session. A
            // persistent write failure is still observable via
            // the disk state (graph.bin stops updating) and via
            // the next one-shot CLI invocation.
            let _ = err;
        }
    }
    Ok(snapshot)
}

/// Best-effort attempt to fetch the freshest snapshot from a
/// running `conspectus serve` daemon. Returns `Some` only when
/// the daemon responds with a structurally valid snapshot;
/// every other outcome (no daemon, snapshot_unavailable error
/// during the daemon's first cycle, transport error, malformed
/// bytes) returns `None` so the caller falls through to local
/// discovery. The fallback is silent for the same reason the
/// rest of the TUI's refresh path is silent on errors: a noisy
/// stderr per cycle would clobber the terminal during a live
/// session.
fn try_daemon_snapshot() -> Option<crate::model::GraphSnapshot> {
    use crate::server::{ClientOutcome, client_snapshot};
    let bytes = match client_snapshot() {
        ClientOutcome::Ok(bytes) => bytes,
        // Every other outcome is "fall back to local discovery."
        // The daemon may be absent, mid-first-cycle, or hung; any
        // of those means the local path is the right answer.
        _ => return None,
    };
    crate::snapshot::from_bytes(&bytes).ok()
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
    let snapshot = database.snapshot();

    let plan = match crate::rename::plan_session_rename(
        snapshot,
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
        &endpoint, &endpoint, snapshot, &loader,
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
    let Some(target) = app.pins_context().pin_target else {
        app.update(Msg::SetStatus(Some(format!(
            "pin rename: no editable pin `{pin_id}` in current selection"
        ))));
        return;
    };
    let request = crate::tui::widgets::pins::PinEditRequest {
        original_id: target.id.clone(),
        id: target.id,
        display_name: display.to_string(),
        harness: target.harness,
        cwd: target.cwd,
        mux_name: target.mux_name,
        mux_socket: target.mux_socket,
        launch_argv: target.launch_argv,
        store_path: target.store_path,
    };
    edit_pin_action(app, config, request);
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
    let Some(target) = app.pins_context().pin_target else {
        app.update(Msg::SetStatus(Some(format!(
            "pin remove: no editable pin `{pin_id}` in current selection"
        ))));
        return;
    };
    remove_pin_controls_action(
        app,
        config,
        crate::tui::widgets::pins::PinRemoveRequest {
            id: target.id,
            display_name: target.display_name,
            store_path: target.store_path,
        },
    );
}

fn pin_bind_hint_action(app: &mut App) {
    // Selection has competing PinAmbiguous candidates → open the
    // picker directly. Otherwise fall back to the existing hint
    // surface so the operator gets a single-line nudge instead of a
    // silent press.
    let options = app.pins_context().pin_bind_options;
    if let Some(state) = crate::tui::widgets::pins::PinsOverlayState::open_with_bind(options) {
        app.set_pins_overlay(state);
        app.update(Msg::SetStatus(Some(
            "pins: bind ↑/↓ pick · Enter confirm · Esc cancel".to_string(),
        )));
        return;
    }
    let diagnostics = crate::tui::actions::selected_pin_diagnostics(app);
    match crate::tui::actions::pin_bind_hint(&diagnostics) {
        Some(message) => app.update(Msg::SetStatus(Some(message))),
        None => app.update(Msg::SetStatus(Some(
            "pin bind: select a pin-bound row with a PinAmbiguous diagnostic".to_string(),
        ))),
    }
}

fn open_pin_create_action(app: &mut App) {
    let defaults = app.pins_context().pin_create_defaults;
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_create(defaults);
    app.set_pins_overlay(state);
    app.update(Msg::SetStatus(Some(
        "pins: new pin · Up/Down field · Enter create · Esc cancel".to_string(),
    )));
}

fn open_pin_rebind_action(app: &mut App) {
    let Some(target) = app.pins_context().pin_target else {
        app.update(Msg::SetStatus(Some(
            "pin rebind: select a pin row first".to_string(),
        )));
        return;
    };
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_rebind(target);
    app.set_pins_overlay(state);
    app.update(Msg::SetStatus(Some(
        "pins: rebind mux · Enter save · Esc cancel".to_string(),
    )));
}

fn open_pin_adopt_action(app: &mut App) {
    // Adopt only makes sense from a live mux row: pin defaults are
    // seeded from the mux's name, observed cwd, and current
    // attribution. Refuse on any other selection so the operator
    // doesn't have to type those fields from scratch.
    if !app.selection_is_live_mux() {
        app.update(Msg::SetStatus(Some(
            "pin adopt: select a live mux row first".to_string(),
        )));
        return;
    }
    let defaults = app.pins_context().pin_create_defaults;
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_create(defaults);
    app.set_pins_overlay(state);
    app.update(Msg::SetStatus(Some(
        "pins: adopt mux · Enter create · Esc cancel".to_string(),
    )));
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
pub(super) fn refresh(app: &mut App, _seed: &RunConfig) {
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
    let snapshot = discover_and_resolve(config)?;
    let tree = build_tree_for_view(&snapshot, config);
    let database = GraphDb::new(snapshot);
    Ok((database, tree))
}

/// Populate an `App` from a given snapshot rather than running live
/// discovery. Used by the snapshot tool's `--snapshot-fixture` path
/// (ADR 0068). The snapshot is run through `resolve_snapshot` so
/// fixtures missing resolved relationships still render correctly;
/// the resolver is idempotent for snapshots that already carry
/// them.
#[cfg(feature = "snapshot")]
pub(super) fn refresh_from_snapshot(
    app: &mut App,
    config: &RunConfig,
    snapshot: crate::model::GraphSnapshot,
) -> Result<()> {
    populate_provider_status(app, config);
    let resolved = resolve_snapshot(snapshot);
    let tree = build_tree_for_view(&resolved, config);
    let database = GraphDb::new(resolved);
    let initial_selection_hint = launch_context_row_id(&tree);
    app.update(Msg::SetData {
        snapshot: database,
        tree,
        loaded_at_epoch: current_unix_epoch().unwrap_or(0),
        initial_selection_hint,
    });
    Ok(())
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
        View::Mux => crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
            snapshot,
            home: home.as_deref(),
            now: current_unix_epoch(),
            filter: config.initial_filter.clone(),
            grouping: config.mux_grouping,
            sort: config.default_sort,
        }),
        View::Union => {
            crate::tui::rows::union::build_union_tree(crate::tui::rows::union::UnionBuildInputs {
                snapshot,
                home: home.as_deref(),
                filter: config.initial_filter.clone(),
            })
        }
        View::Prs => crate::tui::rows::prs::build_prs_tree(crate::tui::rows::prs::PrsBuildInputs {
            snapshot,
            home: home.as_deref(),
        }),
        View::Forks => {
            crate::tui::rows::forks::build_forks_tree(crate::tui::rows::forks::ForksBuildInputs {
                snapshot,
                home: home.as_deref(),
            })
        }
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
//
// `EnterDefault` ends in `Action` semantically (it names the key's
// behavior) and lives alongside `Attach`, `Resume`, `View`, etc.,
// which all read as actions implicitly. Suppress the
// `enum_variant_names` lint locally so the name doesn't have to be
// twisted to satisfy the lint.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "snapshot", allow(dead_code))]
pub(super) enum Action {
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
    /// `b` on a selected ambiguous pin-bound row. Opens the bind
    /// picker when the resolver flagged competing `PinAmbiguous`
    /// candidates; otherwise surfaces a status hint.
    PinBindHint,
    /// `N` opens the pin create form seeded from the current
    /// selection. Direct counterpart to `Pins > create` on `p`.
    OpenPinCreate,
    /// `B` opens the mux-only rebind form for the selected pin.
    /// Refuses with a status hint when no pin row is selected.
    OpenPinRebind,
    /// `A` adopts the selected live mux row as a new pin. Refuses
    /// with a status hint on any other row kind.
    OpenPinAdopt,
    /// `L` launches the selected pin via `conspectus pin launch
    /// <id>`. Sibling of `Enter` on a `RowKind::Pin`; refuses with
    /// a status hint when no pin row is selected.
    LaunchPin,
    /// Open the controls overlay (ADR 0031, F8-005) at its top
    /// section.
    OpenControls,
    /// Forward a key event into the open controls overlay.
    ControlsOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the pins overlay (ADR 0057) at the top of the action
    /// menu. Sibling of `OpenControls`; kept separate so view/filter
    /// and pin CRUD stay one-key-each on `f` and `p`.
    OpenPins,
    /// Forward a key event into the open pins overlay.
    PinsOverlayKey(ratatui::crossterm::event::KeyEvent),
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
        // PR / Fork / Repo rows: no muxable target and no viewer;
        // fall back to toggle so expandable parents still behave.
        RowKind::Pr(_) | RowKind::Fork(_) | RowKind::Repo(_) => SelectedDefault::ToggleExpand,
        // Unbound / stale-mux pin rows hand off to the launch
        // primitive (H-PIN-012) via a subprocess so the launch
        // logic stays in one place.
        RowKind::Pin(_) => SelectedDefault::LaunchPin,
    }
}

/// Dispatch a key into the open help overlay and close it on
/// HelpOutcome::Close.
pub(super) fn handle_help_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
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
pub(super) fn handle_search_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
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
pub(super) fn handle_controls_overlay_key(
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

/// Handle the pins overlay's key event and apply the resulting
/// action to the app. Mirrors [`handle_controls_overlay_key`] but
/// dispatches `PinsAction` through the pin-specific write path.
fn handle_pins_overlay_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::widgets::pins::PinsOutcome;
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle_key(&ctx, key),
        None => return,
    };
    match outcome {
        PinsOutcome::Continue => {}
        PinsOutcome::Close => {
            app.close_pins_overlay();
        }
        PinsOutcome::ApplyAndStay(action) => {
            apply_pins_action_and_refresh(terminal, app, config, action);
        }
        PinsOutcome::ApplyAndClose(action) => {
            app.close_pins_overlay();
            apply_pins_action_and_refresh(terminal, app, config, action);
        }
    }
}

/// Apply a pins action and rebuild the row tree so the change is
/// visible immediately. Mirrors [`apply_controls_action_and_refresh`]
/// but only handles the pin-specific variants; everything else routes
/// through the placeholder status hint.
///
/// `terminal` is threaded through so the `LaunchPin` variant can
/// suspend the alt screen and re-exec into the CLI's launch path —
/// every other variant only writes TOML and does not need it.
fn apply_pins_action_and_refresh(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    action: crate::tui::widgets::pins::PinsAction,
) {
    use crate::tui::widgets::pins::PinsAction;
    match action {
        PinsAction::CreatePin(request) => create_pin_action(app, config, request),
        PinsAction::EditPin(request) => edit_pin_action(app, config, request),
        PinsAction::BindPin(request) => bind_pin_action(app, config, request),
        PinsAction::RemovePin(request) => remove_pin_controls_action(app, config, request),
        PinsAction::LaunchPin { pin_id } => launch_pin_by_id(terminal, app, config, &pin_id),
        PinsAction::PinPlaceholder(_) => {
            app.apply_pins_action(action);
        }
    }
}

/// Scenario-mode counterpart to [`handle_pins_overlay_key`].
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_handle_pins_overlay_key(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    key: ratatui::crossterm::event::KeyEvent,
) -> Result<()> {
    use crate::tui::widgets::pins::PinsOutcome;
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle_key(&ctx, key),
        None => return Ok(()),
    };
    match outcome {
        PinsOutcome::Continue => {}
        PinsOutcome::Close => {
            app.close_pins_overlay();
        }
        PinsOutcome::ApplyAndStay(action) => {
            static_apply_pins_action_and_refresh(app, config, snapshot, action)?;
        }
        PinsOutcome::ApplyAndClose(action) => {
            app.close_pins_overlay();
            static_apply_pins_action_and_refresh(app, config, snapshot, action)?;
        }
    }
    Ok(())
}

/// Apply a controls action and rebuild the row tree so the change
/// is visible immediately. Side-effecting in two places (App state
/// plus discovery refresh) but kept in one helper so the call
/// sites can't accidentally apply without refreshing.
pub(super) fn apply_controls_action_and_refresh(
    app: &mut App,
    config: &RunConfig,
    action: crate::tui::widgets::controls::ControlsAction,
) {
    app.apply_controls_action(action);
    refresh(app, config);
}

fn create_pin_action(
    app: &mut App,
    config: &RunConfig,
    request: crate::tui::widgets::pins::PinCreateRequest,
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
    request: &crate::tui::widgets::pins::PinCreateRequest,
    loader: &crate::config::ConfigLoader,
) -> Result<(PinWriteOutcome, PinEntry, PinStoreKind)> {
    let cwd = std::path::PathBuf::from(&request.cwd);
    let selection = match request.store {
        crate::tui::widgets::pins::PinCreateStore::Auto
        | crate::tui::widgets::pins::PinCreateStore::Project => {
            crate::pins::select_store_for_pin(&cwd, loader)?
        }
        crate::tui::widgets::pins::PinCreateStore::User => crate::pins::user_pin_store(loader)?,
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

fn bind_pin_action(
    app: &mut App,
    config: &RunConfig,
    request: crate::tui::widgets::pins::PinBindRequest,
) {
    let Some(database) = app.graph_db() else {
        app.update(Msg::SetStatus(Some(
            "pin bind failed: no graph database available".to_string(),
        )));
        return;
    };
    let snapshot = database.snapshot();
    match write_pin_bind(&request, snapshot, &crate::config::ConfigLoader::from_env()) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "bound"
            } else {
                "unchanged"
            };
            refresh(app, config);
            app.update(Msg::SetStatus(Some(format!(
                "{verb} pin `{}` to session `{}` in {}",
                request.pin_id,
                request.session_key,
                outcome.path.display()
            ))));
        }
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("pin bind failed: {err}"))));
        }
    }
}

fn write_pin_bind(
    request: &crate::tui::widgets::pins::PinBindRequest,
    snapshot: &crate::model::GraphSnapshot,
    loader: &crate::config::ConfigLoader,
) -> Result<crate::declared::DeclaredWriteOutcome> {
    let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == request.pin_id) else {
        bail!("no pin `{}` in current graph", request.pin_id);
    };
    let target = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            crate::model::GraphNode::AgentSession(session)
                if session.id.harness_key == pin.harness
                    && session.id.session_key == request.session_key =>
            {
                Some(session.id.clone())
            }
            _ => None,
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no `{}` agent session with session_key `{}` in current graph",
                pin.harness,
                request.session_key
            )
        })?;

    let source = crate::declared::DeclaredEndpoint::AgentSession {
        harness_key: target.harness_key.clone(),
        state_scope: target.state_scope.clone(),
        session_key: target.session_key.clone(),
    };
    let target_endpoint = crate::declared::DeclaredEndpoint::MuxSession {
        native_id: pin.mux.native_id(),
    };
    let link = crate::declared::DeclaredLink {
        id: format!("pin:{}:bound", pin.id),
        relation: crate::model::RelationKind::LinkedToMux,
        state: crate::declared::DeclaredLinkState::Active,
        source: source.clone(),
        target: target_endpoint.clone(),
        reason: None,
        overridden_by: None,
        label: Some(format!("pin:{}", pin.id)),
    };
    let path =
        crate::declared::select_store_for_declaration(&source, &target_endpoint, snapshot, loader)
            .map(|selection| selection.path)
            .or_else(|| loader.user_config_path())
            .ok_or_else(|| anyhow::anyhow!("no declared-link store available for pin bind"))?;
    Ok(crate::declared::upsert_declared_link(&path, link)?)
}

fn edit_pin_action(
    app: &mut App,
    config: &RunConfig,
    request: crate::tui::widgets::pins::PinEditRequest,
) {
    match write_pin_edit(&request) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "saved"
            } else {
                "unchanged"
            };
            refresh(app, config);
            app.update(Msg::SetStatus(Some(format!(
                "{verb} pin `{}` in {}",
                request.id,
                outcome.path.display()
            ))));
        }
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("pin edit failed: {err}"))));
        }
    }
}

fn write_pin_edit(request: &crate::tui::widgets::pins::PinEditRequest) -> Result<PinWriteOutcome> {
    let path = std::path::PathBuf::from(&request.store_path);
    preflight_pin_edit(request, &path)?;
    if request.original_id != request.id {
        crate::pins::remove_pin_entry(&path, &request.original_id)?;
    }
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
    Ok(crate::pins::upsert_pin_entry(&path, entry)?)
}

fn preflight_pin_edit(
    request: &crate::tui::widgets::pins::PinEditRequest,
    path: &std::path::Path,
) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let document = crate::pins::parse_pins_document(&text)?;
    let mut found_original = false;
    let requested_socket = request.mux_socket.as_deref().unwrap_or("default");
    for entry in document.entries() {
        if entry.id == request.original_id {
            found_original = true;
            continue;
        }
        if entry.id == request.id {
            bail!(
                "pin id `{}` already exists in {}",
                request.id,
                path.display()
            );
        }
        let entry_socket = entry.mux.socket_name.as_deref().unwrap_or("default");
        if entry.mux.backend == TMUX_MUX_BACKEND
            && entry.mux.name == request.mux_name
            && entry_socket == requested_socket
        {
            bail!(
                "mux `{}` is already used by pin `{}` in {}",
                entry.mux.native_id(),
                entry.id,
                path.display()
            );
        }
    }
    if !found_original {
        bail!(
            "pin `{}` was not found in {}",
            request.original_id,
            path.display()
        );
    }
    Ok(())
}

fn remove_pin_controls_action(
    app: &mut App,
    config: &RunConfig,
    request: crate::tui::widgets::pins::PinRemoveRequest,
) {
    match write_pin_remove(&request) {
        Ok(outcome) if outcome.changed => {
            refresh(app, config);
            app.update(Msg::SetStatus(Some(format!(
                "removed pin `{}` from {}",
                request.id,
                outcome.path.display()
            ))));
        }
        Ok(outcome) => {
            refresh(app, config);
            app.update(Msg::SetStatus(Some(format!(
                "pin `{}` was already absent from {}",
                request.id,
                outcome.path.display()
            ))));
        }
        Err(err) => {
            app.update(Msg::SetStatus(Some(format!("pin remove failed: {err}"))));
        }
    }
}

fn write_pin_remove(
    request: &crate::tui::widgets::pins::PinRemoveRequest,
) -> Result<PinWriteOutcome> {
    crate::pins::remove_pin_entry(&request.store_path, &request.id).map_err(Into::into)
}

/// Switch view and refresh. Shared between the `1`–`5` direct keys
/// and `]` / `[` cycling.
pub(super) fn apply_view_switch(app: &mut App, config: &RunConfig, view: View) {
    apply_controls_action_and_refresh(
        app,
        config,
        crate::tui::widgets::controls::ControlsAction::SwitchView(view),
    );
}

/// Step the view enum forward (delta > 0) or back (delta < 0),
/// wrapping. Used by the `]` / `[` accelerator pair.
pub(super) fn cycle_view(view: View, delta: i32) -> View {
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
    launch_pin_by_id(terminal, app, config, &pin_id);
}

/// Shared body of `pin launch <id>`: suspend the TUI, re-exec into
/// the CLI, and refresh on return. Used by both the row-level
/// `Enter` / `L` dispatch and the Pins modal's `launch` entry so
/// the modal flow doesn't reinvent the terminal hand-off.
fn launch_pin_by_id(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    pin_id: &str,
) {
    ratatui::restore();
    let status = std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("conspectus")),
    )
    .args(["pin", "launch", pin_id])
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
fn remap_for_focus(action: Action, focus: crate::tui::app::Focus) -> Option<Action> {
    use crate::tui::app::Focus;
    if focus != Focus::Right {
        return Some(action);
    }
    match action {
        Action::Msg(boxed) => match *boxed {
            // T8-028: j/k drive the explorer cursor when the right
            // pane has focus, replacing the prior raw preview-scroll
            // remap. Uppercase J/K still scroll the preview.
            Msg::NavDown => Some(Action::Msg(Box::new(Msg::ExplorerNavDown))),
            Msg::NavUp => Some(Action::Msg(Box::new(Msg::ExplorerNavUp))),
            Msg::PageDown(_) => Some(Action::Msg(Box::new(Msg::ExplorerNavDown))),
            Msg::PageUp(_) => Some(Action::Msg(Box::new(Msg::ExplorerNavUp))),
            // `e` toggles group expansion; on a non-header row the
            // reducer surfaces a status hint.
            Msg::ToggleLinkedDetails => Some(Action::Msg(Box::new(Msg::ExplorerToggleGroup))),
            // `h`/`l`/`←`/`→` are left-tree expand/collapse keys.
            // When the right pane has focus, drop them so they
            // don't reach across panes and mutate the tree the
            // operator is no longer driving. The explorer has no
            // analogous binding in v1; `Enter` drills, `Backspace`
            // pops a hop.
            Msg::ExpandRow | Msg::CollapseRow => None,
            // H-OBS-007: `g`/`Home` and `G`/`End` should snap the
            // explorer cursor to its first / last row on right-
            // pane focus, the right-pane-equivalent of how those
            // keys jump the left-tree selection. `Tab` /
            // `Msg::CycleFocus` is intentionally left alone — that
            // key is the focus toggle itself and stays useful
            // regardless of which pane currently has focus.
            Msg::Home => Some(Action::Msg(Box::new(Msg::ExplorerHome))),
            Msg::End => Some(Action::Msg(Box::new(Msg::ExplorerEnd))),
            other => Some(Action::Msg(Box::new(other))),
        },
        // T8-040 / T8-043: Enter on the explorer cursor either
        // copies a Node-zone field value (T8-040) or expands a
        // group header / drills into a link row (T8-043). The
        // dispatcher inspects the cursor row at action time, so
        // remap to [`Action::ExplorerEnter`] and let the main
        // loop branch with App state in hand. Left-pane Enter
        // (DefaultAction) is dispatched against the selected
        // row's kind separately.
        Action::DefaultAction => Some(Action::ExplorerEnter),
        // T8-034: `F` toggles the Expanded Node Detail view when the
        // right pane is focused. The same key still clears filters
        // when the left tree has focus (ADR 0031).
        Action::ClearFilters => Some(Action::Msg(Box::new(Msg::ExplorerToggleFullDetail))),
        other => Some(other),
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
pub(super) fn translate(event: Event, viewport_height: u16) -> Option<Action> {
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
            // ADR 0057 direct pin shortcuts. `p` opens the discoverable
            // menu (added in the modal split); these capitals reach
            // each action without a menu pick.
            (KeyModifiers::SHIFT, KeyCode::Char('N'))
            | (KeyModifiers::NONE, KeyCode::Char('N')) => Some(Action::OpenPinCreate),
            (KeyModifiers::SHIFT, KeyCode::Char('B'))
            | (KeyModifiers::NONE, KeyCode::Char('B')) => Some(Action::OpenPinRebind),
            (KeyModifiers::SHIFT, KeyCode::Char('A'))
            | (KeyModifiers::NONE, KeyCode::Char('A')) => Some(Action::OpenPinAdopt),
            (KeyModifiers::SHIFT, KeyCode::Char('L'))
            | (KeyModifiers::NONE, KeyCode::Char('L')) => Some(Action::LaunchPin),
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
            // ADR 0057 §TUI: `p` opens the dedicated pins management
            // modal. Sibling of `f` (view/filter controls); pin CRUD
            // also has direct shortcuts so the modal is the
            // discoverable surface rather than a required step.
            (m, KeyCode::Char('p')) if !m.contains(KeyModifiers::CONTROL) => Some(Action::OpenPins),
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
    use crate::tui::widgets::pins::{
        PinBindRequest, PinCreateRequest, PinCreateStore, PinEditRequest, PinRemoveRequest,
    };
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
    fn translate_p_opens_pins_overlay() {
        assert_eq!(
            translate(press(KeyCode::Char('p'), KeyModifiers::NONE), 24),
            Some(Action::OpenPins)
        );
    }

    #[test]
    fn translate_capital_n_requests_open_pin_create() {
        assert_eq!(
            translate(press(KeyCode::Char('N'), KeyModifiers::SHIFT), 24),
            Some(Action::OpenPinCreate)
        );
        assert_eq!(
            translate(press(KeyCode::Char('N'), KeyModifiers::NONE), 24),
            Some(Action::OpenPinCreate)
        );
    }

    #[test]
    fn translate_capital_b_requests_open_pin_rebind() {
        assert_eq!(
            translate(press(KeyCode::Char('B'), KeyModifiers::SHIFT), 24),
            Some(Action::OpenPinRebind)
        );
        // Lowercase b still routes to the bind picker / hint.
        assert_eq!(
            translate(press(KeyCode::Char('b'), KeyModifiers::NONE), 24),
            Some(Action::PinBindHint)
        );
    }

    #[test]
    fn translate_capital_l_requests_pin_launch() {
        assert_eq!(
            translate(press(KeyCode::Char('L'), KeyModifiers::SHIFT), 24),
            Some(Action::LaunchPin)
        );
        assert_eq!(
            translate(press(KeyCode::Char('L'), KeyModifiers::NONE), 24),
            Some(Action::LaunchPin)
        );
    }

    #[test]
    fn translate_capital_a_requests_open_pin_adopt() {
        assert_eq!(
            translate(press(KeyCode::Char('A'), KeyModifiers::SHIFT), 24),
            Some(Action::OpenPinAdopt)
        );
        // Lowercase a still attaches.
        assert_eq!(
            translate(press(KeyCode::Char('a'), KeyModifiers::NONE), 24),
            Some(Action::Attach)
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
    fn write_pin_edit_updates_id_display_mux_and_launch() {
        let project = tempfile::TempDir::new().expect("project");
        let path = project.path().join(".conspectus.toml");
        let entry = PinEntry {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux: PinMux {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch: None,
            reason: None,
        };
        crate::pins::upsert_pin_entry(&path, entry).expect("seed pin");

        let outcome = write_pin_edit(&PinEditRequest {
            original_id: "ingest".to_string(),
            id: "daily-ingest".to_string(),
            display_name: "Daily Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux_name: "daily".to_string(),
            mux_socket: Some("scratch".to_string()),
            launch_argv: vec!["codex".to_string(), "--resume".to_string()],
            store_path: path.display().to_string(),
        })
        .expect("edit pin");

        assert!(outcome.changed);
        let written = fs::read_to_string(&path).expect("config");
        assert!(!written.contains(r#"id = "ingest""#));
        assert!(written.contains(r#"id = "daily-ingest""#));
        assert!(written.contains(r#"display_name = "Daily Ingest""#));
        assert!(written.contains(r#"name = "daily""#));
        assert!(written.contains(r#"socket_name = "scratch""#));
        assert!(written.contains(r#""--resume""#));
    }

    #[test]
    fn write_pin_edit_rejects_duplicate_id_without_mutating() {
        let project = tempfile::TempDir::new().expect("project");
        let path = project.path().join(".conspectus.toml");
        for (id, mux) in [("ingest", "ingest"), ("scratch", "scratch")] {
            crate::pins::upsert_pin_entry(
                &path,
                PinEntry {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    harness: "codex".to_string(),
                    cwd: project.path().display().to_string(),
                    mux: PinMux {
                        backend: TMUX_MUX_BACKEND.to_string(),
                        name: mux.to_string(),
                        socket_name: None,
                    },
                    launch: None,
                    reason: None,
                },
            )
            .expect("seed pin");
        }
        let before = fs::read_to_string(&path).expect("before");

        let err = write_pin_edit(&PinEditRequest {
            original_id: "ingest".to_string(),
            id: "scratch".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux_name: "ingest".to_string(),
            mux_socket: None,
            launch_argv: Vec::new(),
            store_path: path.display().to_string(),
        })
        .expect_err("duplicate id");
        assert!(err.to_string().contains("already exists"));
        assert_eq!(fs::read_to_string(&path).expect("after"), before);
    }

    #[test]
    fn write_pin_edit_rejects_duplicate_mux_without_mutating() {
        let project = tempfile::TempDir::new().expect("project");
        let path = project.path().join(".conspectus.toml");
        for (id, mux) in [("ingest", "ingest"), ("scratch", "scratch")] {
            crate::pins::upsert_pin_entry(
                &path,
                PinEntry {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    harness: "codex".to_string(),
                    cwd: project.path().display().to_string(),
                    mux: PinMux {
                        backend: TMUX_MUX_BACKEND.to_string(),
                        name: mux.to_string(),
                        socket_name: None,
                    },
                    launch: None,
                    reason: None,
                },
            )
            .expect("seed pin");
        }
        let before = fs::read_to_string(&path).expect("before");

        let err = write_pin_edit(&PinEditRequest {
            original_id: "ingest".to_string(),
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux_name: "scratch".to_string(),
            mux_socket: None,
            launch_argv: Vec::new(),
            store_path: path.display().to_string(),
        })
        .expect_err("duplicate mux");
        assert!(err.to_string().contains("already used"));
        assert_eq!(fs::read_to_string(&path).expect("after"), before);
    }

    #[test]
    fn write_pin_bind_writes_declared_override() {
        let temp = tempfile::TempDir::new().expect("temp");
        let loader = crate::config::ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(temp.path().join(".config"));
        let session_id = crate::model::AgentSessionId::new("codex", "/state", "session-a");
        let mut snapshot = crate::model::GraphSnapshot::empty();
        snapshot.nodes.push(crate::model::GraphNode::AgentSession(
            crate::model::AgentSessionNode {
                id: session_id,
                harness_key: "codex".to_string(),
                cwd: Some("/workspace".to_string()),
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            },
        ));
        snapshot.pins.push(crate::model::PinCandidate {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/workspace".to_string(),
            mux: crate::model::PinMuxRef {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: crate::model::Provenance::LocalPin,
            store_path: "/workspace/.conspectus.toml".to_string(),
            binding: None,
        });

        let outcome = write_pin_bind(
            &PinBindRequest {
                pin_id: "ingest".to_string(),
                session_key: "session-a".to_string(),
            },
            &snapshot,
            &loader,
        )
        .expect("bind pin");

        assert!(outcome.changed);
        let written = fs::read_to_string(outcome.path).expect("declared config");
        assert!(written.contains(r#"id = "pin:ingest:bound""#));
        assert!(written.contains(r#"label = "pin:ingest""#));
        assert!(written.contains(r#"session_key = "session-a""#));
        assert!(written.contains(r#"native_id = "tmux:ingest""#));
    }

    #[test]
    fn write_pin_remove_removes_from_explicit_store_path() {
        let project = tempfile::TempDir::new().expect("project");
        let path = project.path().join(".conspectus.toml");
        let entry = PinEntry {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            harness: "codex".to_string(),
            cwd: project.path().display().to_string(),
            mux: PinMux {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch: None,
            reason: None,
        };
        crate::pins::upsert_pin_entry(&path, entry).expect("seed pin");
        assert!(path.exists());

        let outcome = write_pin_remove(&PinRemoveRequest {
            id: "ingest".to_string(),
            display_name: "Ingest".to_string(),
            store_path: path.display().to_string(),
        })
        .expect("remove pin");
        assert!(outcome.changed);
        assert_eq!(outcome.path, path);
        assert!(!outcome.path.exists());
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
        assert_eq!(cycle_view(View::Prs, 1), View::Forks);
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
        assert_eq!(remap_for_focus(action.clone(), Focus::Left), Some(action));
        let action = Action::Msg(Box::new(Msg::PageDown(20)));
        assert_eq!(remap_for_focus(action.clone(), Focus::Left), Some(action));
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
            Some(Action::Msg(Box::new(Msg::ExplorerNavDown)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::NavUp)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerNavUp)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageDown(20))), Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerNavDown)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::PageUp(20))), Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerNavUp)))
        );
    }

    #[test]
    fn remap_for_focus_right_routes_enter_and_e_to_the_explorer() {
        use crate::tui::app::Focus;
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::CycleFocus)))
        );
        // Locked decision 8: Enter is the universal "do the obvious
        // thing" key on the explorer cursor. T8-040 made the
        // dispatch App-aware (Node-zone fields copy; other rows
        // drill/expand), so the remap produces the new
        // [`Action::ExplorerEnter`] variant that the main loop
        // resolves against [`App::explorer_copy_target`].
        assert_eq!(
            remap_for_focus(Action::DefaultAction, Focus::Right),
            Some(Action::ExplorerEnter)
        );
        // Left-pane DefaultAction is left untouched here so the main
        // loop can resolve it against the selected row.
        assert_eq!(
            remap_for_focus(Action::DefaultAction, Focus::Left),
            Some(Action::DefaultAction)
        );
        // `e` is the explicit expand/collapse accelerator.
        assert_eq!(
            remap_for_focus(
                Action::Msg(Box::new(Msg::ToggleLinkedDetails)),
                Focus::Right
            ),
            Some(Action::Msg(Box::new(Msg::ExplorerToggleGroup)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::Quit)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::Quit)))
        );
        assert_eq!(
            remap_for_focus(Action::Refresh, Focus::Right),
            Some(Action::Refresh)
        );
        // T8-034: F clears filters on the left tree but toggles the
        // Expanded Node Detail view on the right pane.
        assert_eq!(
            remap_for_focus(Action::ClearFilters, Focus::Left),
            Some(Action::ClearFilters)
        );
        assert_eq!(
            remap_for_focus(Action::ClearFilters, Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerToggleFullDetail)))
        );
    }

    #[test]
    fn remap_for_focus_right_suppresses_left_tree_expand_collapse_keys() {
        // Regression: `h` / `l` / `←` / `→` are left-tree
        // expand/collapse keys. When the right pane is focused they
        // used to leak through and mutate the tree the operator
        // wasn't driving. The focus remap drops them.
        use crate::tui::app::Focus;
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::ExpandRow)), Focus::Right),
            None
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CollapseRow)), Focus::Right),
            None
        );
        // Left focus still routes them through unchanged.
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::ExpandRow)), Focus::Left),
            Some(Action::Msg(Box::new(Msg::ExpandRow)))
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CollapseRow)), Focus::Left),
            Some(Action::Msg(Box::new(Msg::CollapseRow)))
        );
    }

    #[test]
    fn remap_for_focus_right_routes_home_and_end_into_the_explorer() {
        // H-OBS-007 (paired with the h/l/Left/Right suppression
        // above): `g`/`Home` and `G`/`End` snap the explorer
        // cursor to its first / last row on right-pane focus
        // instead of bleeding into the left tree's Home/End
        // jumps. `Tab`/`CycleFocus` is intentionally left alone —
        // it is the focus toggle and must stay useful regardless
        // of which pane has focus.
        use crate::tui::app::Focus;
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::Home)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerHome))),
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::End)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::ExplorerEnd))),
        );
        // Left focus still drives the left tree.
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::Home)), Focus::Left),
            Some(Action::Msg(Box::new(Msg::Home))),
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::End)), Focus::Left),
            Some(Action::Msg(Box::new(Msg::End))),
        );
        // Tab cycles focus on either side — never remapped.
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Right),
            Some(Action::Msg(Box::new(Msg::CycleFocus))),
        );
        assert_eq!(
            remap_for_focus(Action::Msg(Box::new(Msg::CycleFocus)), Focus::Left),
            Some(Action::Msg(Box::new(Msg::CycleFocus))),
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
            let tree =
                crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
                    snapshot: &snapshot,
                    home: None,
                    now: None,
                    filter: RowFilter::default(),
                    grouping: crate::tui::MuxGrouping::Session,
                    sort: crate::tui::Sort::Hierarchy,
                });
            let mut cfg = RunConfig::defaults();
            cfg.default_view = View::Mux;
            let mut app = App::new(cfg);
            app.update(Msg::SetData {
                snapshot: GraphDb::new(snapshot),
                tree,
                loaded_at_epoch: 1_700_000_000,
                initial_selection_hint: None,
            });
            assert_eq!(selected_default_action(&app), SelectedDefault::Attach);
        }

        #[test]
        fn unmuxed_session_resolves_to_view_regardless_of_viewer_support() {
            // T8-043: the dispatcher routes every un-muxed agent
            // session through `SelectedDefault::View`. Harnesses
            // without a registered viewer (or whose viewer binary is
            // missing from `$PATH`) still resolve to `View` here —
            // the runtime's `view_action` is what surfaces the
            // `viewer_disabled_reason` status message after the
            // operator presses Enter. Keeping the dispatcher
            // uniform keeps the keymap consistent across harnesses
            // and lets the fallback message stay actionable.
            let mut snapshot = GraphSnapshot::empty();
            add_repo_and_worktree(&mut snapshot, "/p/proj");
            // `aider` has no viewer registered in v1 (see
            // `H-TRANSCRIPT-007` deferred), so this is the
            // unsupported-viewer surface for the dispatcher.
            snapshot
                .nodes
                .push(session_node("aider", "/state", "abc", "/p/proj"));
            let mut app = build_sessions_app(snapshot);
            app.update(Msg::NavDown);
            assert_eq!(selected_default_action(&app), SelectedDefault::View);
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
