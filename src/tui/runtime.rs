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
use std::process::Output;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

use crate::discovery::tmux::{MuxBackend, SystemTmux};
use crate::model::MuxSessionId;
use crate::pins::{PinEntry, PinLaunch, PinMux, PinStoreKind, PinWriteOutcome, TMUX_MUX_BACKEND};
use crate::resolve::resolve_snapshot;
#[cfg(test)]
use crate::tui::View;
use crate::tui::actions::{AttachTarget, PinLaunchTarget, resolve_attach_target};
use crate::tui::app::{App, GraphDb, Msg};
use crate::tui::effect::Effect;
use crate::tui::preview::capture_via;
use crate::tui::resume::{ResumeTarget, launch_resume, resume_disabled_reason};
use crate::tui::rows::RowId;
use crate::tui::rows::RowTree;
use crate::tui::viewer::{
    LaunchPlan, PathBinaryProbe, ViewerTarget, resolve_viewer_target, viewer_disabled_reason,
};
use crate::tui::{RunConfig, ui};

/// Result of a completed background discovery run. The worker returns
/// only the resolved snapshot; the main thread builds the row tree from
/// the app's current view config.
type DiscoveryResult = Result<crate::model::GraphSnapshot>;

/// One event the TUI runtime consumes per iteration (ADR 0085
/// contract 5, H-TUI-004). Wave 1 introduces the type but each
/// loop still gathers its own events inline; wave 3 refactors the
/// loops to consume a `next_ui_event(...)` stream once the
/// module split lands.
///
/// `Input` carries a crossterm event (key press, resize, focus,
/// mouse). `Tick` fires from the refresh timer. `Discovery`
/// carries a completed background discovery result off the
/// mpsc channel. Fixture mode's `r` re-reads happen synchronously
/// on `Input(Refresh)` in the current shape and don't need their
/// own variant.
#[allow(dead_code)]
#[derive(Debug)]
pub(super) enum UiEvent {
    Input(Event),
    Tick,
    Discovery(DiscoveryResult),
}

/// Mode-specific behavior for the shared [`run_loop`] driver
/// (ADR 0085 contract 5, H-TUI-004 wave 2). Each mode owns its
/// own async event sources (discovery worker + refresh timer
/// for live; nothing for scenario) and its own action-dispatch
/// policy (real handlers vs "disabled" status hints). The
/// driver stays generic over the mode.
trait LoopMode {
    /// Called once before the loop starts. Runs mode-specific
    /// initial data population + preview capture.
    fn init(
        &mut self,
        terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
    ) -> Result<()>;

    /// Called before input poll each iteration. Live mode drains
    /// completed discovery results off its channel and fires the
    /// refresh timer when it expires; scenario mode is a no-op.
    fn drain(&mut self, app: &mut App, config: &RunConfig) -> Result<()>;

    /// Dispatch a translated action against `app`. Live modes
    /// call real handlers (exec, tmux ops, store writes);
    /// scenario mode returns "disabled" status hints for mutating
    /// actions and reloads the fixture on `Action::Refresh`.
    fn dispatch(
        &mut self,
        terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
        action: Option<Action>,
    ) -> Result<()>;

    /// Reference to the mode's mux runner. Used by
    /// [`refresh_mux_preview_if_needed`] at the end of each
    /// iteration.
    fn tmux(&self) -> &dyn MuxBackend;
}

/// Shared event loop driver (H-TUI-004 wave 2). Consumes any
/// [`LoopMode`]; both live and scenario runs go through here.
/// Per-mode differences (discovery channel, refresh timer,
/// dispatch policy) live in the mode impl.
fn run_loop(
    terminal: &mut DefaultTerminal,
    mut app: App,
    config: RunConfig,
    mut mode: impl LoopMode,
) -> Result<()> {
    let poll_timeout = Duration::from_millis(100);
    mode.init(terminal, &mut app, &config)?;

    while !app.should_quit() {
        draw_frame(&mut app, terminal)?;
        mode.drain(&mut app, &config)?;
        if event::poll(poll_timeout)? {
            let event = event::read()?;
            let viewport = terminal.size()?.height.saturating_sub(2);
            let prev_mux_target = current_mux_target(&app);
            let action = overlay_key_from_event(&app, &event).or_else(|| {
                translate(event, viewport).and_then(|a| remap_for_focus(a, app.focus()))
            });
            mode.dispatch(terminal, &mut app, &config, action)?;
            refresh_mux_preview_if_needed(&mut app, &config, mode.tmux(), prev_mux_target);
        }
    }

    // Belt-and-suspenders shutdown persist: the view / grouping /
    // filter / sort reducer arms emit `Effect::Persist` per ADR
    // 0085 contract 2 Phase E, so this final call is redundant
    // on the happy path. Kept so any state a future non-reducer
    // path mutates just before shutdown still lands on disk.
    app.persist_state();
    Ok(())
}

/// Live-mode loop state: owns the mux runner, the background
/// discovery channel, and the refresh timer. Its `dispatch`
/// routes every action to the real handler (exec, tmux ops,
/// store writes).
struct LiveMode {
    tmux: Box<dyn MuxBackend>,
    result_tx: mpsc::Sender<DiscoveryResult>,
    result_rx: mpsc::Receiver<DiscoveryResult>,
    pending_refresh: bool,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl LoopMode for LiveMode {
    fn init(
        &mut self,
        _terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
    ) -> Result<()> {
        // T8-007: initial discovery runs on the same background
        // worker path as timer refreshes and `r`, so the first
        // frame paints immediately with the `graph_db.is_none()`
        // "Loading discovery…" placeholder while the scan runs.
        // Provider status still populates synchronously so the
        // status-bar chips render on frame one.
        populate_provider_status(app, config);
        self.pending_refresh = true;
        self.last_refresh = Instant::now();
        spawn_tracked_discovery(app, &self.result_tx);
        // `refresh_mux_preview_if_needed` is a no-op until a
        // snapshot lands (its `resolve_attach_target` guard
        // returns `NoSelection` when `graph_db` is None), so we
        // skip the call here — the first post-`drain` iteration
        // in the run loop will invoke it once the worker
        // delivers.
        Ok(())
    }

    fn drain(&mut self, app: &mut App, config: &RunConfig) -> Result<()> {
        // Drain completed background discovery results without
        // blocking. Only the most recent result wins.
        while let Ok(result) = self.result_rx.try_recv() {
            self.pending_refresh = false;
            app.update(Msg::InFlightFinish(
                crate::tui::app::InFlightKind::Discovery,
            ));
            match result {
                Ok(snapshot) => {
                    // Build the tree against `App`'s current
                    // projection state (ADR 0085 contract 4). If
                    // the operator changed view / grouping /
                    // filter after the worker was spawned, the
                    // fresh snapshot lands in the projection the
                    // operator is actually looking at instead of
                    // the stale config snapshot the worker
                    // captured.
                    let tree = crate::tui::rows::build_tree_for_view(
                        crate::tui::rows::TreeInputs::from_app(&snapshot, app),
                    );
                    let database = GraphDb::new(snapshot);
                    let initial_selection_hint = launch_context_row_id(&tree);
                    app.update(Msg::SetData {
                        snapshot: database,
                        tree,
                        loaded_at_epoch: current_unix_epoch().unwrap_or(0),
                        initial_selection_hint,
                    });
                    let cfg_clone = app.config().clone();
                    populate_provider_status(app, &cfg_clone);
                }
                Err(err) => {
                    app.update(Msg::SetRefreshFailure(format!(
                        "last refresh failed; {err}"
                    )));
                }
            }
        }
        let _ = config;
        // Timer-driven auto-refresh. Only fires when no request
        // is in-flight and at least `refresh_interval` has
        // elapsed.
        if !self.pending_refresh && self.last_refresh.elapsed() >= self.refresh_interval {
            self.pending_refresh = true;
            self.last_refresh = Instant::now();
            spawn_tracked_discovery(app, &self.result_tx);
        }
        Ok(())
    }

    fn dispatch(
        &mut self,
        terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
        action: Option<Action>,
    ) -> Result<()> {
        let tmux = self.tmux.as_ref();
        match action {
            Some(Action::Msg(msg)) => dispatch(app, *msg),
            Some(Action::Refresh) => {
                if !self.pending_refresh {
                    self.pending_refresh = true;
                    self.last_refresh = Instant::now();
                    spawn_tracked_discovery(app, &self.result_tx);
                }
            }
            Some(Action::Attach) => {
                dispatch_live(terminal, app, config, tmux, Msg::AttachSelected);
            }
            Some(Action::Resume) => {
                dispatch_live(terminal, app, config, tmux, Msg::ResumeSelected);
            }
            Some(Action::View) => {
                dispatch_live(terminal, app, config, tmux, Msg::ViewSelected);
            }
            Some(Action::DefaultAction) => default_action(terminal, app, config, tmux),
            Some(Action::OpenRename) => open_rename_overlay(app),
            Some(Action::RenameOverlayKey(key)) => {
                handle_rename_overlay_key(terminal, app, config, tmux, key)
            }
            Some(Action::RemovePin) => remove_pin_action(terminal, app, config, tmux),
            Some(Action::PinBindHint) => pin_bind_hint_action(app),
            Some(Action::OpenPinCreate) => open_pin_create_action(app),
            Some(Action::OpenPinRebind) => open_pin_rebind_action(app),
            Some(Action::OpenPinAdopt) => open_pin_adopt_action(app),
            Some(Action::LaunchPin) => {
                dispatch_live(terminal, app, config, tmux, Msg::LaunchSelectedPin);
            }
            Some(Action::OpenControls) => {
                app.open_controls_overlay();
                app.update(Msg::SetStatus(Some(
                    "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::ControlsOverlayKey(key)) => handle_controls_overlay_key(app, config, key),
            Some(Action::OpenPins) => {
                app.open_pins_overlay();
                app.update(Msg::SetStatus(Some(
                    "pins: ↑/↓ move · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::PinsOverlayKey(key)) => {
                handle_pins_overlay_key(terminal, app, config, tmux, key)
            }
            Some(Action::SwitchView(view)) => dispatch(app, Msg::SwitchView(view)),
            Some(Action::CycleView(delta)) => {
                let next = cycle_view(app.active_view(), delta);
                dispatch(app, Msg::SwitchView(next));
            }
            Some(Action::CycleGrouping(delta)) => {
                let next = if delta >= 0 {
                    app.grouping().cycle_next()
                } else {
                    app.grouping().cycle_prev()
                };
                dispatch(app, Msg::SetGrouping(next));
            }
            Some(Action::ClearFilters) => {
                dispatch(app, Msg::SetFilter(crate::filter::RowFilter::default()));
                app.update(Msg::SetStatus(Some("filters cleared".to_string())));
            }
            Some(Action::OpenSearch) => {
                app.open_search_overlay();
                app.update(Msg::SetStatus(Some(
                    "search: type to filter · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::SearchOverlayKey(key)) => handle_search_overlay_key(app, key),
            Some(Action::OpenHelp) => {
                app.open_help_overlay();
            }
            Some(Action::HelpOverlayKey(key)) => handle_help_overlay_key(app, key),
            Some(Action::OpenValueModal) => app.open_value_modal_for_cursor(),
            Some(Action::ValueModalKey(key)) => handle_value_modal_key(app, key),
            Some(Action::ViewerOverlayKey(key)) => handle_viewer_overlay_key(app, key),
            Some(Action::ExplorerEnter) => explorer_enter_action(app),
            Some(Action::CopySessionId) => copy_session_id_action(app),
            None => {}
        }
        Ok(())
    }

    fn tmux(&self) -> &dyn MuxBackend {
        self.tmux.as_ref()
    }
}

/// Static-mode loop state: fixture-replay + scenario TUI. Its
/// `dispatch` blocks mutating actions with a status hint and
/// reloads the fixture on `Action::Refresh`.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
struct StaticMode {
    tmux: Box<dyn MuxBackend>,
    snapshot: crate::model::GraphSnapshot,
    fixture_path: Option<std::path::PathBuf>,
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
impl LoopMode for StaticMode {
    fn init(
        &mut self,
        _terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
    ) -> Result<()> {
        set_static_data(app, config, &self.snapshot)?;
        refresh_mux_preview_if_needed(app, config, self.tmux.as_ref(), None);
        Ok(())
    }

    fn drain(&mut self, _app: &mut App, _config: &RunConfig) -> Result<()> {
        // Scenario mode has no async event sources: no discovery
        // worker to poll and no refresh timer to arm.
        Ok(())
    }

    fn dispatch(
        &mut self,
        _terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
        action: Option<Action>,
    ) -> Result<()> {
        match action {
            Some(Action::Msg(msg)) => dispatch(app, *msg),
            Some(Action::OpenHelp) => app.open_help_overlay(),
            Some(Action::HelpOverlayKey(key)) => handle_help_overlay_key(app, key),
            Some(Action::OpenValueModal) => app.open_value_modal_for_cursor(),
            Some(Action::ValueModalKey(key)) => handle_value_modal_key(app, key),
            Some(Action::ViewerOverlayKey(key)) => handle_viewer_overlay_key(app, key),
            Some(Action::OpenSearch) => {
                app.open_search_overlay();
                app.update(Msg::SetStatus(Some(
                    "search: type to filter · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::SearchOverlayKey(key)) => handle_search_overlay_key(app, key),
            Some(Action::OpenControls) => {
                app.open_controls_overlay();
                app.update(Msg::SetStatus(Some(
                    "controls: ↑/↓ move · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::ControlsOverlayKey(key)) => {
                static_handle_controls_overlay_key(app, config, &self.snapshot, key)?;
            }
            Some(Action::OpenPins) => {
                app.open_pins_overlay();
                app.update(Msg::SetStatus(Some(
                    "pins: ↑/↓ move · Enter pick · Esc close".to_string(),
                )));
            }
            Some(Action::PinsOverlayKey(key)) => {
                static_handle_pins_overlay_key(app, config, &self.snapshot, key)?;
            }
            Some(Action::SwitchView(view)) => dispatch(app, Msg::SwitchView(view)),
            Some(Action::CycleView(delta)) => {
                let next = cycle_view(app.active_view(), delta);
                dispatch(app, Msg::SwitchView(next));
            }
            Some(Action::Refresh) => {
                if let Some(path) = self.fixture_path.as_deref() {
                    // ADR 0069: in fixture mode, `r` re-reads the
                    // file on disk so the operator can edit the
                    // JSON and cycle in the new state without
                    // leaving the session. Parse errors land in
                    // the status bar; the previously loaded
                    // fixture stays active.
                    match read_fixture(path) {
                        Ok(fresh) => {
                            self.snapshot = fresh;
                            set_static_data(app, config, &self.snapshot)?;
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
                    set_static_data(app, config, &self.snapshot)?;
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
            Some(Action::DefaultAction) => {
                let msg = match selected_default_action(app) {
                    SelectedDefault::ToggleExpand => Msg::ToggleExpand,
                    SelectedDefault::Attach => Msg::SetStatus(Some(
                        "scenario TUI is static; attach is disabled".to_string(),
                    )),
                    SelectedDefault::View => {
                        Msg::SetStatus(Some("scenario TUI is static; view is disabled".to_string()))
                    }
                    SelectedDefault::LaunchPin => Msg::SetStatus(Some(
                        "scenario TUI is static; pin launch is disabled".to_string(),
                    )),
                };
                dispatch(app, msg);
            }
            Some(Action::CycleGrouping(delta)) => {
                let next = if delta >= 0 {
                    app.grouping().cycle_next()
                } else {
                    app.grouping().cycle_prev()
                };
                dispatch(app, Msg::SetGrouping(next));
            }
            Some(Action::ClearFilters) => {
                dispatch(app, Msg::SetFilter(crate::filter::RowFilter::default()));
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
            Some(Action::PinBindHint) => pin_bind_hint_action(app),
            Some(Action::OpenPinCreate) => open_pin_create_action(app),
            Some(Action::OpenPinRebind) => open_pin_rebind_action(app),
            Some(Action::OpenPinAdopt) => open_pin_adopt_action(app),
            Some(Action::LaunchPin) => {
                app.update(Msg::SetStatus(Some(
                    "scenario TUI is static; pin launch is disabled".to_string(),
                )));
            }
            Some(Action::ExplorerEnter) => explorer_enter_action(app),
            Some(Action::CopySessionId) => copy_session_id_action(app),
            None => {}
        }
        Ok(())
    }

    fn tmux(&self) -> &dyn MuxBackend {
        self.tmux.as_ref()
    }
}

/// Prepare the toast area and render one frame. Shared between
/// [`event_loop`] and [`static_event_loop`] (H-TUI-004 wave 1) —
/// the toast prep + viewer-or-ui draw block was byte-identical
/// in both loops.
fn draw_frame(app: &mut App, terminal: &mut DefaultTerminal) -> Result<()> {
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
            ui::draw(app, frame);
        }
    })?;
    Ok(())
}

/// Route a crossterm event through the modal stack (ADR 0085
/// contract 3). Returns `Some(...OverlayKey(key))` when an overlay
/// owns the input, `None` when the caller should fall through to
/// its per-mode action translation.
///
/// Shared between the live and static event loops (H-TUI-004
/// wave 1). Only KeyEventKind::Press events are forwarded — the
/// modal stack ignores repeat/release + non-key events like the
/// individual overlay handlers already did.
fn overlay_key_from_event(app: &App, event: &Event) -> Option<Action> {
    let key = match event {
        Event::Key(k) if k.kind == KeyEventKind::Press => *k,
        _ => return None,
    };
    if app.viewer_modal().is_some() {
        return Some(Action::ViewerOverlayKey(key));
    }
    if app.value_modal().is_some() {
        return Some(Action::ValueModalKey(key));
    }
    if app.rename_overlay().is_some() {
        return Some(Action::RenameOverlayKey(key));
    }
    if app.controls_overlay().is_some() {
        return Some(Action::ControlsOverlayKey(key));
    }
    if app.pins_overlay().is_some() {
        return Some(Action::PinsOverlayKey(key));
    }
    if app.search_overlay().is_some() {
        return Some(Action::SearchOverlayKey(key));
    }
    if app.help_overlay().is_some() {
        return Some(Action::HelpOverlayKey(key));
    }
    None
}

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
    app.restore_persisted_state();
    let (result_tx, result_rx) = mpsc::channel::<DiscoveryResult>();
    let refresh_interval = config.refresh_interval;
    let mode = LiveMode {
        tmux: Box::new(SystemTmux::new()),
        result_tx,
        result_rx,
        pending_refresh: false,
        last_refresh: Instant::now(),
        refresh_interval,
    };
    run_loop(terminal, app, config, mode)
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_event_loop(
    terminal: &mut DefaultTerminal,
    config: RunConfig,
    initial_snapshot: crate::model::GraphSnapshot,
    fixture_path: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut app = App::new(config.clone());
    // F8-013: enable last-active-view persistence for the static
    // fixture-replay TUI mode too. Snapshot mode (ADR 0067) uses a
    // distinct entry point in `src/tui/snapshot.rs` that
    // deliberately skips this so the on-disk file stays
    // unconditionally unmoved when the snapshot tooling runs.
    app.enable_view_persistence(crate::tui_state::TuiStateCache::from_env());
    app.restore_persisted_state();
    let mode = StaticMode {
        tmux: Box::new(SystemTmux::new()),
        snapshot: initial_snapshot,
        fixture_path,
    };
    run_loop(terminal, app, config, mode)
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_handle_controls_overlay_key(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    key: ratatui::crossterm::event::KeyEvent,
) -> Result<()> {
    use crate::tui::widgets::controls::ControlsContext;
    use crate::tui::{Overlay, OverlayOutcome};
    let view = app.active_view();
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
        Some(state) => state.handle(&ctx, key),
        None => return Ok(()),
    };
    let _ = (config, snapshot);
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_controls_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_controls_overlay();
            dispatch(app, *msg);
        }
    }
    Ok(())
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_apply_pins_msg(app: &mut App, msg: Msg) -> Result<()> {
    // Scenario TUI keeps every mutating pin action off. Anything
    // that would write TOML or spawn a subprocess gets replaced
    // with a status hint; harmless status/placeholder Msgs pass
    // through to the reducer.
    match msg {
        Msg::PinCreate(_)
        | Msg::PinEdit(_)
        | Msg::PinBind(_)
        | Msg::PinRemove(_)
        | Msg::LaunchPinById(_) => {
            app.update(Msg::SetStatus(Some(
                "scenario TUI keeps mutating actions disabled".to_string(),
            )));
        }
        other => {
            app.update(other);
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
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        snapshot, app,
    ));
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

/// Spawn a discovery worker + register the corresponding
/// `InFlightKind::Discovery` marker so the status bar renders a
/// spinner chip while the worker runs (H-WIDG-007). The marker is
/// cleared when `drain` receives the worker's result.
fn spawn_tracked_discovery(app: &mut App, tx: &mpsc::Sender<DiscoveryResult>) {
    spawn_discovery_worker(app.config(), tx);
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Discovery,
        label: "Discovering".to_string(),
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
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.rename_overlay_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_rename_overlay();
            app.update(Msg::SetStatus(Some("rename: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_rename_overlay();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

fn remove_pin_action(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
) {
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
    dispatch_live(
        terminal,
        app,
        config,
        tmux,
        Msg::PinRemove(crate::tui::widgets::pins::PinRemoveRequest {
            id: target.id,
            display_name: target.display_name,
            store_path: target.store_path,
        }),
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
    let status = match crate::tui::actions::pin_bind_hint(&diagnostics) {
        Some(message) => message,
        None => "pin bind: select a pin-bound row with a PinAmbiguous diagnostic".to_string(),
    };
    dispatch(app, Msg::SetStatus(Some(status)));
}

fn open_pin_create_action(app: &mut App) {
    let ctx = app.pins_context();
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_create_context(&ctx);
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
    let ctx = app.pins_context();
    if ctx.pin_adopt_defaults.is_none() {
        app.update(Msg::SetStatus(Some(
            "pin adopt: select a live mux row first".to_string(),
        )));
        return;
    }
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_adopt_context(&ctx);
    app.set_pins_overlay(state);
    app.update(Msg::SetStatus(Some(
        "pins: new pin · adopt enabled · Enter create · Esc cancel".to_string(),
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

/// Plan a preview-capture effect for the current selection.
/// Returns `None` when live preview is disabled, when the target is
/// unchanged and already cached, or when the selection has no mux
/// target. Pure: reads only [`App`] and [`RunConfig`] state.
fn plan_mux_preview_capture(
    app: &App,
    config: &RunConfig,
    prev: Option<MuxSessionId>,
) -> Option<Effect> {
    if !config.live_preview_enabled {
        return None;
    }
    let target = resolve_attach_target(app).ok()?;
    if prev.as_ref() == Some(&target.mux) && app.mux_preview(&target.mux).is_some() {
        return None;
    }
    Some(Effect::RunMux(crate::tui::effect::MuxOp::CapturePreview {
        mux: target.mux,
        native_id: target.native_id,
    }))
}

/// Refresh the mux preview if the selection has moved to a new
/// target. Reducer-adjacent orchestration: pure [`plan_mux_preview_capture`]
/// decides whether a capture is needed; the executor performs it.
fn refresh_mux_preview_if_needed(
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    prev: Option<MuxSessionId>,
) {
    let Some(effect) = plan_mux_preview_capture(app, config, prev) else {
        return;
    };
    execute_mux_op(
        app,
        tmux,
        match effect {
            Effect::RunMux(op) => op,
            _ => unreachable!("plan_mux_preview_capture only emits RunMux"),
        },
    );
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
    refresh_with_config(app, &config);
}

/// Run one pure (no-terminal) effect. Shared between
/// [`execute_effects`] and [`execute_effects_live`] so the two
/// executors stay in lockstep for the non-`Exec` variants.
fn execute_pure_effect(app: &mut App, effect: Effect) {
    match effect {
        Effect::Quit => {
            // `Msg::Quit` already set `App::should_quit`; the event
            // loop reads that flag each iteration. The effect
            // signal is redundant with the state field today and
            // will become the sole quit signal once every quit
            // path routes through here.
        }
        Effect::Toast(label) => app.post_toast(label),
        Effect::Persist => app.persist_state(),
        Effect::SpawnRefresh { force_local } => {
            if force_local {
                let mut cfg = app.config().clone();
                cfg.refresh = true;
                refresh_with_config(app, &cfg);
            } else {
                let cfg = app.config().clone();
                refresh(app, &cfg);
            }
        }
        Effect::Exec(_) => {
            // Exec effects need a live terminal; the pure executor
            // silently drops them. Static and snapshot modes gate
            // their translators upstream so `AttachSelected` /
            // `ResumeSelected` never fire there — this branch is a
            // safety net rather than a code path exercised in
            // practice.
        }
        Effect::RunMux(_) => {
            // Mux ops need a live `MuxBackend`; the pure executor
            // silently drops them. Same rationale as `Effect::Exec`
            // above — no static-mode path emits `RunMux` today, so
            // this branch is a safety net.
        }
        Effect::WriteStore(_) => {
            // Some `WriteStore` variants (`PinCreate`,
            // `CommitAliasRename`) chain a tmux rename through the
            // executor, so all store writes flow through
            // `execute_effects_live` where the `MuxBackend`
            // reference lives. The pure executor drops them the
            // same way it drops `RunMux` / `Exec`.
        }
    }
}

/// Dispatch a [`StoreOp`] against the on-disk TOML store. The only
/// place in the TUI where a reducer-emitted store write touches
/// `.conspectus.toml` (ADR 0085 contract 2 Phase D). Handles the
/// full post-write flow: refresh so the row tree reflects the
/// change, then post a status message summarizing the outcome.
fn execute_store_op(app: &mut App, tmux: &dyn MuxBackend, op: crate::tui::effect::StoreOp) {
    use crate::tui::effect::StoreOp;
    match op {
        StoreOp::PinCreate(request) => execute_pin_create(app, tmux, request),
        StoreOp::PinEdit(request) => execute_pin_edit(app, request),
        StoreOp::PinRemove(request) => execute_pin_remove(app, request),
        StoreOp::PinBind(request) => execute_pin_bind(app, request),
        StoreOp::CommitAliasRename {
            session_id,
            new_display_name,
        } => execute_commit_alias_rename(app, tmux, session_id, new_display_name),
    }
}

fn execute_pin_remove(app: &mut App, request: crate::tui::widgets::pins::PinRemoveRequest) {
    match write_pin_remove(&request) {
        Ok(outcome) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let message = if outcome.changed {
                format!(
                    "removed pin `{}` from {}",
                    request.id,
                    outcome.path.display()
                )
            } else {
                format!(
                    "pin `{}` was already absent from {}",
                    request.id,
                    outcome.path.display()
                )
            };
            let _ = app.update(Msg::SetStatus(Some(message)));
        }
        Err(err) => {
            let _ = app.update(Msg::SetStatus(Some(format!("pin remove failed: {err}"))));
        }
    }
}

fn execute_pin_bind(app: &mut App, request: crate::tui::widgets::pins::PinBindRequest) {
    let Some(database) = app.graph_db() else {
        // Reducer already gated on this — the branch is a safety
        // net for direct executor callers.
        let _ = app.update(Msg::SetStatus(Some(
            "pin bind failed: no graph database available".to_string(),
        )));
        return;
    };
    let snapshot = database.snapshot().clone();
    match write_pin_bind(
        &request,
        &snapshot,
        &crate::config::ConfigLoader::from_env(),
    ) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "bound"
            } else {
                "unchanged"
            };
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let message = format!(
                "{verb} pin `{}` to session `{}` in {}",
                request.pin_id,
                request.session_key,
                outcome.path.display()
            );
            let _ = app.update(Msg::SetStatus(Some(message)));
        }
        Err(err) => {
            let _ = app.update(Msg::SetStatus(Some(format!("pin bind failed: {err}"))));
        }
    }
}

fn execute_pin_create(
    app: &mut App,
    tmux: &dyn MuxBackend,
    request: crate::tui::widgets::pins::PinCreateRequest,
) {
    match write_pin_create(&request, &crate::config::ConfigLoader::from_env()) {
        Ok((outcome, entry, store_kind)) => {
            let is_adopt = request.adopt_source_mux_name.is_some();
            let verb = if outcome.changed {
                if outcome.entry_count == 1 {
                    "wrote"
                } else {
                    "updated"
                }
            } else {
                "unchanged"
            };
            let rename_status = apply_pin_adopt_mux_rename(tmux, &request);
            let pin_id = entry.id.clone();
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let selected = app.select_pin_after_mutation(&pin_id);
            app.post_toast(pin_create_success_toast(is_adopt, &pin_id));
            let mut message = format!(
                "{verb} pin `{}` in {} ({})",
                entry.id,
                outcome.path.display(),
                pin_store_label(store_kind)
            );
            if let Some(rename_status) = rename_status {
                message.push_str("; ");
                message.push_str(&rename_status);
            }
            if !selected {
                message.push_str("; no visible row matched the new pin");
            }
            let _ = app.update(Msg::SetStatus(Some(message)));
        }
        Err(err) => {
            let _ = app.update(Msg::SetStatus(Some(format!("pin create failed: {err}"))));
        }
    }
}

fn execute_pin_edit(app: &mut App, request: crate::tui::widgets::pins::PinEditRequest) {
    match write_pin_edit(&request) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "saved"
            } else {
                "unchanged"
            };
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let _ = app.update(Msg::SetStatus(Some(format!(
                "{verb} pin `{}` in {}",
                request.id,
                outcome.path.display()
            ))));
        }
        Err(err) => {
            let _ = app.update(Msg::SetStatus(Some(format!("pin edit failed: {err}"))));
        }
    }
}

/// Executor branch for `StoreOp::CommitAliasRename`. Mirrors the
/// old `commit_rename` runtime helper: plan the rename against the
/// held snapshot, write the alias entry (or remove it when the
/// operator cleared the field), then chain a tmux rename when the
/// plan carries a native mux rename. Uses `app.config()` for the
/// post-write refresh so a stale config from an earlier snapshot
/// doesn't leak through.
fn execute_commit_alias_rename(
    app: &mut App,
    tmux: &dyn MuxBackend,
    session_id: crate::model::AgentSessionId,
    new_display_name: Option<String>,
) {
    let Some(database) = app.graph_db() else {
        let _ = app.update(Msg::SetStatus(Some(
            "rename: no graph database available".to_string(),
        )));
        return;
    };
    let snapshot = database.snapshot().clone();

    let plan =
        match crate::rename::plan_session_rename(&snapshot, &session_id, new_display_name, false) {
            Ok(plan) => plan,
            Err(err) => {
                let _ = app.update(Msg::SetStatus(Some(format!("rename failed: {err}"))));
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
                let _ = app.update(Msg::SetStatus(Some(
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
                node: endpoint,
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
            let _ = app.update(Msg::SetStatus(Some(format!("rename failed: {err}"))));
            return;
        }
    };

    if let Some(mux_rename) = &plan.mux_native_rename {
        // Default-socket rename for now; pin-driven socket
        // propagation lands with H-PIN-017's TUI lockstep work.
        let config = app.config().clone();
        match tmux.rename_session(None, &mux_rename.mux.native_id, &mux_rename.new_name) {
            Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => {}
            Ok(other) => {
                let _ = app.update(Msg::SetStatus(Some(format!(
                    "alias updated, tmux rename failed: {other:?}"
                ))));
                refresh(app, &config);
                return;
            }
            Err(err) => {
                let _ = app.update(Msg::SetStatus(Some(format!(
                    "alias updated, tmux rename errored: {err}"
                ))));
                refresh(app, &config);
                return;
            }
        }
    }

    let advisory = live_session_advisory(app, &session_id);
    let config = app.config().clone();
    refresh(app, &config);
    let final_status = match advisory {
        Some(suffix) => format!("{alias_status} · {suffix}"),
        None => alias_status,
    };
    let _ = app.update(Msg::SetStatus(Some(final_status)));
}

/// Pure executor (ADR 0085 contract 2). Runs the effects a reducer
/// can produce without a live terminal — used by tests, snapshot
/// mode, and the scenario TUI. `Effect::Exec` is silently dropped;
/// callers that need to run execs use [`execute_effects_live`].
pub(super) fn execute_effects(app: &mut App, effects: Vec<Effect>) {
    for effect in effects {
        execute_pure_effect(app, effect);
    }
}

/// Live-loop executor. Handles every [`Effect`] variant, including
/// `Effect::Exec` (which requires `&mut DefaultTerminal` and the
/// process launcher) and `Effect::RunMux` (which requires a live
/// [`MuxBackend`] reference). This is the sole code in the TUI
/// that touches `&mut Terminal`, `std::process`, or `MuxBackend`
/// for reducer-emitted effects.
pub(super) fn execute_effects_live(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    effects: Vec<Effect>,
) {
    for effect in effects {
        match effect {
            Effect::Exec(spec) => execute_exec_spec(terminal, app, config, spec),
            Effect::RunMux(op) => execute_mux_op(app, tmux, op),
            Effect::WriteStore(op) => execute_store_op(app, tmux, op),
            other => execute_pure_effect(app, other),
        }
    }
}

/// Dispatch a [`MuxOp`] against the executor's `MuxBackend`. The
/// only place in the TUI where a reducer-emitted mux op talks to
/// tmux (ADR 0085 contract 2 Phase C).
fn execute_mux_op(app: &mut App, tmux: &dyn MuxBackend, op: crate::tui::effect::MuxOp) {
    use crate::tui::effect::MuxOp;
    match op {
        MuxOp::CapturePreview { mux, native_id } => {
            // Capture against the **raw** backend-native session
            // name (e.g. `editor`), not the backend-prefixed graph
            // id (`tmux:editor`) — tmux itself doesn't understand
            // the latter.
            let content = capture_via(tmux, &native_id);
            let _ = app.update(Msg::SetMuxPreview { mux, content });
        }
    }
}

/// Dispatch an [`ExecSpec`] against the live terminal. Every branch
/// owns the terminal handoff, waits for the child to exit,
/// re-enters the alt screen if needed, schedules a follow-up
/// refresh, and posts a status message summarizing the outcome.
fn execute_exec_spec(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    spec: crate::tui::effect::ExecSpec,
) {
    use crate::tui::effect::ExecSpec;
    match spec {
        ExecSpec::AttachMux(target) => {
            let outcome = run_tmux_attach(terminal, &target);
            refresh(app, config);
            let message = match outcome {
                AttachOutcome::Detached => format!("attached/detached: {}", target_short(&target)),
                AttachOutcome::Failed(reason) => format!("attach failed: {reason}"),
            };
            let _ = app.update(Msg::SetStatus(Some(message)));
        }
        ExecSpec::Resume(target) => {
            let message = match &target {
                ResumeTarget::Launch { label, .. } => {
                    if launch_resume(&target) {
                        format!("resumed: {label}")
                    } else {
                        "resume: failed to launch".to_string()
                    }
                }
                other => resume_disabled_reason(other),
            };
            let _ = app.update(Msg::SetStatus(Some(message)));
        }
        ExecSpec::ViewSession(session_id) => {
            execute_view_session(terminal, app, config, session_id);
        }
        ExecSpec::LaunchPin {
            pin_id,
            attach_target,
        } => {
            execute_launch_pin(terminal, app, config, &pin_id, attach_target.as_ref());
        }
    }
}

/// Executor branch for `ExecSpec::ViewSession`. Tries the native
/// viewer first (ADR 0052) — its filesystem read + parser call
/// keep the reducer pure. Falls through to the external-launch
/// escape hatch for harnesses without a native parser (currently
/// `aider`).
fn execute_view_session(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    session_id: crate::model::AgentSessionId,
) {
    if let Some(state) = crate::tui::viewer_bridge::build_viewer_state(&session_id) {
        let label = format!("{}:{}", session_id.harness_key, session_id.session_key);
        app.open_viewer_modal(state);
        let _ = app.update(Msg::SetStatus(Some(format!("viewing {label}"))));
        return;
    }
    let target = resolve_viewer_target(&session_id, &PathBinaryProbe);
    let message = match target {
        ViewerTarget::Launch(plan) => {
            let outcome = run_viewer_launch(terminal, &plan);
            refresh(app, config);
            match outcome {
                ViewerOutcome::Exited => format!("viewed: {}", plan.label),
                ViewerOutcome::Failed(reason) => format!("view failed: {reason}"),
            }
        }
        ViewerTarget::Disabled(reason) => viewer_disabled_reason(&reason),
    };
    let _ = app.update(Msg::SetStatus(Some(message)));
}

/// Bridge helper: dispatch a `Msg` through the reducer and run any
/// returned effects through the pure executor. Call sites that don't
/// have a live terminal (tests, scenario TUI) use this; the live
/// loop uses [`dispatch_live`] so exec effects have a place to run.
pub(super) fn dispatch(app: &mut App, msg: Msg) {
    let effects = app.update(msg);
    execute_effects(app, effects);
}

/// Live-loop dispatch: reduce + execute against the live terminal
/// and mux backend. Only used from the live event loop and
/// executor-adjacent helpers — pure contexts (tests, snapshot mode)
/// call [`dispatch`] instead.
pub(super) fn dispatch_live(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    msg: Msg,
) {
    let effects = app.update(msg);
    execute_effects_live(terminal, app, config, tmux, effects);
}

/// Refresh immediately after a pin mutation.
///
/// Pin writes update `.conspectus.toml` synchronously, but the
/// regular refresh path may short-circuit through a running daemon.
/// Force the local discovery branch here so the row tree reflects
/// the just-written pin store without waiting for the daemon's next
/// tick.
fn refresh_after_pin_mutation(app: &mut App, _seed: &RunConfig) {
    let config = pin_mutation_refresh_config(app);
    refresh_with_config(app, &config);
}

fn pin_mutation_refresh_config(app: &App) -> RunConfig {
    let mut config = app.config().clone();
    config.refresh = true;
    config
}

fn refresh_with_config(app: &mut App, config: &RunConfig) {
    populate_provider_status(app, config);
    match discover_and_resolve(config) {
        Ok(snapshot) => {
            // Build against `App`'s projection state (ADR 0085
            // contract 4). This is a synchronous refresh so no App
            // mutations landed between resolve and build, but
            // reading from App keeps the shape symmetric with the
            // async worker path above.
            let tree = crate::tui::rows::build_tree_for_view(
                crate::tui::rows::TreeInputs::from_app(&snapshot, app),
            );
            let database = GraphDb::new(snapshot);
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
                "last refresh failed; {err}"
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
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &resolved, app,
    ));
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

// Tree derivation moved to `tui::rows::mod.rs` in H-TUI-002 Phase F
// so `App::update`'s projection-change reducer arms can call it
// without depending on runtime.

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
// Action / SelectedDefault / selected_default_action /
// pin_placeholder_row moved to `tui::keymap` in H-TUI-004 wave 3.
pub(super) use crate::tui::keymap::{Action, SelectedDefault, selected_default_action};

/// Dispatch a key into the open help overlay via the shared
/// [`crate::tui::Overlay`] contract (ADR 0085 contract 3). Close /
/// Commit outcomes pop the stack; Consumed leaves the overlay
/// open.
pub(super) fn handle_help_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.help_overlay_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_help_overlay(),
        OverlayOutcome::Commit(msg) => {
            app.close_help_overlay();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Dispatch a key into the open full-value modal via the shared
/// [`crate::tui::Overlay`] contract (ADR 0085 contract 3). Close /
/// Commit outcomes pop the stack; Consumed leaves the modal open.
fn handle_value_modal_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.value_modal_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_value_modal(),
        OverlayOutcome::Commit(msg) => {
            app.close_value_modal();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Dispatch a key into the open search overlay, refresh its match
/// list from the visible row tree using the configured backend,
/// and act on its outcome (Confirm picks a row, Cancel closes).
pub(super) fn handle_search_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::search::{SubstringBackend, items_from_rows};
    use crate::tui::widgets::search::SearchContext;
    use crate::tui::{Overlay, OverlayOutcome};
    // The backend choice lives behind the SearchBackend trait so a
    // future swap (e.g. to a fuzzy matcher) needs only an
    // implementation change, not a runtime change. The substring
    // backend is the v1 default per ADR 0024's "prefer hand-rolled
    // first" stance.
    let backend = SubstringBackend;
    let visible: Vec<_> = app.visible_rows().into_iter().cloned().collect();
    let items = items_from_rows(&visible);
    let ctx = SearchContext {
        items: &items,
        backend: &backend,
    };
    let outcome = match app.search_overlay_mut() {
        Some(state) => state.handle(ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_search_overlay();
        }
        OverlayOutcome::Commit(msg) => {
            app.close_search_overlay();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Handle the controls overlay's key event and apply the resulting
/// action to the app, re-deriving the row tree when needed.
pub(super) fn handle_controls_overlay_key(
    app: &mut App,
    _config: &RunConfig,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::widgets::controls::ControlsContext;
    use crate::tui::{Overlay, OverlayOutcome};
    // Snapshot the live state into owned copies so the immutable
    // borrow on `app` ends before we re-borrow it mutably to
    // dispatch the key into the overlay.
    let view = app.active_view();
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
        Some(state) => state.handle(&ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_controls_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_controls_overlay();
            dispatch(app, *msg);
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
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle(&ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_pins_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_pins_overlay();
            dispatch_live(terminal, app, config, tmux, *msg);
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
    use crate::tui::{Overlay, OverlayOutcome};
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle(&ctx, key),
        None => return Ok(()),
    };
    let _ = (config, snapshot);
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_pins_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            static_apply_pins_msg(app, *msg)?;
        }
        OverlayOutcome::Commit(msg) => {
            app.close_pins_overlay();
            static_apply_pins_msg(app, *msg)?;
        }
    }
    Ok(())
}

fn pin_create_success_toast(is_adopt: bool, pin_id: &str) -> String {
    if is_adopt {
        format!("pin adopted; mux already running: `{pin_id}`")
    } else {
        format!("pin created, not started: `{pin_id}`")
    }
}

fn apply_pin_adopt_mux_rename(
    tmux: &dyn MuxBackend,
    request: &crate::tui::widgets::pins::PinCreateRequest,
) -> Option<String> {
    let source = request.adopt_source_mux_name.as_deref()?;
    if source == request.mux_name {
        return Some(format!("adopted existing mux `{source}`"));
    }
    match tmux.rename_session(request.mux_socket.as_deref(), source, &request.mux_name) {
        Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => Some(format!(
            "renamed adopted mux `{source}` to `{}`",
            request.mux_name
        )),
        Ok(other) => Some(format!(
            "pin written, but adopted mux rename `{source}` -> `{}` failed: {other:?}",
            request.mux_name
        )),
        Err(err) => Some(format!(
            "pin written, but adopted mux rename `{source}` -> `{}` errored: {err}",
            request.mux_name
        )),
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
        session_key: target.session_key,
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

fn write_pin_remove(
    request: &crate::tui::widgets::pins::PinRemoveRequest,
) -> Result<PinWriteOutcome> {
    crate::pins::remove_pin_entry(&request.store_path, &request.id).map_err(Into::into)
}

// cycle_view moved to `tui::keymap` in H-TUI-004 wave 3.
pub(super) use crate::tui::keymap::cycle_view;

/// Dispatch `Enter` on the left pane (T8-043) to the selected
/// row's default action. Group rows expand/collapse; mux rows and
/// muxed agent sessions attach; un-muxed agent sessions open the
/// transcript viewer. Right-pane focus is handled in
/// [`remap_for_focus`] before this is reached.
fn default_action(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
) {
    match selected_default_action(app) {
        SelectedDefault::ToggleExpand => dispatch(app, Msg::ToggleExpand),
        SelectedDefault::Attach => dispatch_live(terminal, app, config, tmux, Msg::AttachSelected),
        SelectedDefault::View => dispatch_live(terminal, app, config, tmux, Msg::ViewSelected),
        SelectedDefault::LaunchPin => {
            dispatch_live(terminal, app, config, tmux, Msg::LaunchSelectedPin);
        }
    }
}

/// Handle `Enter` on a pin row (ADR 0057 / H-PIN-017). Suspends
/// the TUI, re-execs into `conspectus pin launch <id>` as a
/// subprocess so the launch logic stays in `cli::PinLaunchArgs`
/// without re-implementing it across the runtime, waits for the
/// nested process to exit (typically when the operator detaches
/// from tmux), then re-enters the alt screen and refreshes the
/// row tree.
/// Look up the pin's `cwd` from the loaded snapshot so
/// [`execute_launch_pin`] can pass it as `--scan-root` on the
/// subprocess. Returns `None` when no snapshot is loaded yet or
/// the pin id isn't present — the subprocess still runs, it just
/// falls back to its inherited-CWD discovery (matching pre-fix
/// behavior on the honest miss).
fn resolve_pin_scan_root(app: &App, pin_id: &str) -> Option<String> {
    app.graph_db()
        .and_then(|db| db.snapshot().pins.iter().find(|p| p.id == pin_id).cloned())
        .map(|p| p.cwd)
}

/// Build the argv the TUI passes when re-execing into
/// `conspectus pin launch`. When `scan_root` is present it becomes
/// a `--scan-root <path>` pair so the subprocess discovers pins
/// from the pin's project root instead of the TUI's inherited
/// CWD. Split out from [`execute_launch_pin`] so the argv shape
/// is unit-testable without running the actual subprocess.
fn pin_launch_argv(pin_id: &str, scan_root: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "pin".to_string(),
        "launch".to_string(),
        pin_id.to_string(),
        "--no-attach".to_string(),
    ];
    if let Some(root) = scan_root {
        args.push("--scan-root".to_string());
        args.push(root.to_string());
    }
    args
}

/// Executor branch for `ExecSpec::LaunchPin`: suspend the alt
/// screen, re-exec into `conspectus pin launch <id> --no-attach`,
/// refresh so the row tree reflects the just-created session, then
/// attach to the resulting tmux session when the pin has an
/// attachable target. Also called directly from the Pins overlay's
/// `PinsAction::LaunchPin` path (which has the pin id in hand from
/// the modal and bypasses the reducer until Phase D lands).
fn execute_launch_pin(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    pin_id: &str,
    target: Option<&PinLaunchTarget>,
) {
    // Pass the pin's own `cwd` as `--scan-root` on the subprocess
    // so `conspectus pin launch` discovers from the pin's project
    // root, not the TUI's inherited CWD. Without this, launching a
    // pin from a TUI started in a directory that isn't the pin's
    // project root fails with "no pin `<id>` in any discovered
    // store" — the subprocess re-discovers using its own CWD, and
    // project-scoped pin stores (`.conspectus.toml`) don't live
    // outside their project. User-scoped pins are unaffected
    // either way; passing the extra scan-root is safe there too.
    let pin_scan_root = resolve_pin_scan_root(app, pin_id);
    let args = pin_launch_argv(pin_id, pin_scan_root.as_deref());
    ratatui::restore();
    let output = std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("conspectus")),
    )
    .args(&args)
    .output();
    *terminal = ratatui::init();
    let _ = terminal.clear();

    refresh(app, config);
    let launch = summarize_pin_launch_output(pin_id, output);
    app.update(Msg::SetStatus(Some(launch.message.clone())));
    if !launch.success {
        app.post_toast(launch.message);
        return;
    }

    let Some(target) = target else {
        app.update(Msg::SetStatus(Some(format!(
            "{}; attach target unavailable after refresh",
            launch.message
        ))));
        return;
    };

    if let Some(reason) = tmux_session_unavailable(target) {
        let message = format!(
            "pin `{pin_id}` launched but tmux session `{}` is not attachable: {reason}",
            target.mux_name
        );
        app.update(Msg::SetStatus(Some(message.clone())));
        app.post_toast(message);
        return;
    }

    let attach_target = AttachTarget {
        mux: MuxSessionId::new(format!("tmux:{}", target.mux_name)),
        backend: "tmux".to_string(),
        native_id: target.mux_name.clone(),
    };
    let outcome =
        run_tmux_attach_with_socket(terminal, &attach_target, target.mux_socket.as_deref());
    refresh(app, config);
    let message = match outcome {
        AttachOutcome::Detached => format!("pin `{pin_id}` launch attached/detached"),
        AttachOutcome::Failed(reason) => format!("pin `{pin_id}` launch attach failed: {reason}"),
    };
    app.update(Msg::SetStatus(Some(message)));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PinLaunchSummary {
    success: bool,
    message: String,
}

fn summarize_pin_launch_output(pin_id: &str, output: std::io::Result<Output>) -> PinLaunchSummary {
    match output {
        Ok(output) if output.status.success() => {
            let detail = command_output_excerpt(&output);
            PinLaunchSummary {
                success: true,
                message: if detail.is_empty() {
                    format!("pin `{pin_id}` launched")
                } else {
                    format!("pin `{pin_id}` launched: {detail}")
                },
            }
        }
        Ok(output) => {
            let detail = command_output_excerpt(&output);
            PinLaunchSummary {
                success: false,
                message: if detail.is_empty() {
                    format!("pin `{pin_id}` launch exited with {}", output.status)
                } else {
                    format!("pin `{pin_id}` launch failed: {detail}")
                },
            }
        }
        Err(err) => PinLaunchSummary {
            success: false,
            message: format!("pin `{pin_id}` launch failed to spawn: {err}"),
        },
    }
}

fn command_output_excerpt(output: &Output) -> String {
    let mut lines = String::new();
    lines.push_str(&String::from_utf8_lossy(&output.stderr));
    if lines.trim().is_empty() {
        lines.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    lines
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(180)
        .collect()
}

fn tmux_session_unavailable(target: &PinLaunchTarget) -> Option<String> {
    let mut command = std::process::Command::new("tmux");
    if let Some(socket) = target
        .mux_socket
        .as_deref()
        .filter(|socket| *socket != "default")
    {
        command.args(["-L", socket]);
    }
    let output = command
        .args(["has-session", "-t", target.mux_name.as_str()])
        .output();
    match output {
        Ok(output) if output.status.success() => None,
        Ok(output) => {
            let detail = command_output_excerpt(&output);
            Some(if detail.is_empty() {
                format!("tmux has-session exited with {}", output.status)
            } else {
                detail
            })
        }
        Err(err) => Some(format!("could not run tmux has-session: {err}")),
    }
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
        None => {
            app.update(Msg::SetStatus(Some(
                "i: select an agent or mux session row to copy its id".to_string(),
            )));
        }
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
    run_tmux_attach_with_socket(terminal, target, None)
}

fn run_tmux_attach_with_socket(
    terminal: &mut DefaultTerminal,
    target: &AttachTarget,
    socket_name: Option<&str>,
) -> AttachOutcome {
    // Suspend the ratatui terminal so tmux owns the real screen
    // for the duration of the attach.
    ratatui::restore();

    let nested = std::env::var_os("TMUX").is_some();
    let subcommand = if nested {
        "switch-client"
    } else {
        "attach-session"
    };
    let mut command = std::process::Command::new("tmux");
    if let Some(socket) = socket_name.filter(|socket| *socket != "default") {
        command.args(["-L", socket]);
    }
    let status = command.args([subcommand, "-t", &target.native_id]).status();

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
        Ok(s) => AttachOutcome::Failed(format!("tmux {subcommand} exited with {s}")),
        Err(err) => AttachOutcome::Failed(format!("could not launch tmux: {err}")),
    }
}

fn target_short(target: &AttachTarget) -> String {
    format!("{}:{}", target.backend, target.native_id)
}

/// Translate a key event into a [`crate::viewer::input::ViewerMsg`],
/// run it through the pure reducer, and put the new state back on
/// `app` — unless the reducer's effect was `Close`, in which case
/// dismiss the modal. Keys that don't map are dropped silently
/// (the modal owns every keystroke while open).
fn handle_viewer_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::viewer::input::ViewerMsg;
    if app.viewer_modal().is_none() {
        return;
    }
    let vmsg =
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
    let Some(vmsg) = vmsg else {
        return;
    };
    // Nested-reducer dispatch: the App reducer's Msg::Viewer arm
    // pops the top viewer state, runs it through
    // viewer::input::reduce, and pushes the new state back
    // (or leaves it popped on ViewerEffect::Close).
    dispatch(app, Msg::Viewer(vmsg));
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

// translate and remap_for_focus moved to `tui::keymap` in H-TUI-004 wave 3.
pub(super) use crate::tui::keymap::{remap_for_focus, translate};

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
