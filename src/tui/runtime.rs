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
use crate::tui::app::{App, Msg, SnapshotHandle};
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
mod executor;
mod launch;
mod overlay_keys;
mod pin_store;
mod worktree_exec;

use executor::*;
use launch::*;
use overlay_keys::*;
use pin_store::*;
use worktree_exec::*;

pub(super) use overlay_keys::{
    handle_controls_overlay_key, handle_help_overlay_key, handle_messages_overlay_key,
    handle_search_overlay_key,
};

/// What a background discovery worker sends back. `warning` carries a
/// problem the worker worked around, such as a failed daemon nudge
/// that it answered with a local rebuild.
struct DiscoveryResult {
    snapshot: Result<crate::model::GraphSnapshot>,
    warning: Option<String>,
}

/// Mode-specific behavior for the shared [`run_loop`] driver
/// (ADR 0085 contract 5). Each mode owns its
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

/// Shared event loop driver. Consumes any
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
            let action = action_for_event(&app, event, viewport);
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
        // Initial discovery runs on the same background
        // worker path as timer refreshes and `r`, so the first
        // frame paints immediately with the `snapshot_handle.is_none()`
        // "Loading discovery…" placeholder while the scan runs.
        // Provider status still populates synchronously so the
        // status-bar chips render on frame one.
        populate_provider_status(app, config);
        self.pending_refresh = true;
        self.last_refresh = Instant::now();
        spawn_tracked_discovery(app, &self.result_tx);
        // `refresh_mux_preview_if_needed` is a no-op until a
        // snapshot lands (its `resolve_attach_target` guard
        // returns `NoSelection` when `snapshot_handle` is None), so we
        // skip the call here — the first post-`drain` iteration
        // in the run loop will invoke it once the worker
        // delivers.
        Ok(())
    }

    fn drain(&mut self, app: &mut App, config: &RunConfig) -> Result<()> {
        // Drain completed background discovery results without
        // blocking. Only the most recent result wins.
        while let Ok(DiscoveryResult { snapshot, warning }) = self.result_rx.try_recv() {
            self.pending_refresh = false;
            app.update(Msg::InFlightFinish(
                crate::tui::app::InFlightKind::Discovery,
            ));
            if let Some(warning) = warning {
                app.report(crate::tui::messages::LogEntry::warning(warning));
            }
            match snapshot {
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
                    let handle = SnapshotHandle::new(snapshot);
                    let initial_selection_hint = launch_context_row_id(&tree);
                    app.update(Msg::SetData {
                        snapshot: handle,
                        tree,
                        loaded_at_epoch: crate::discovery::current_epoch(),
                        initial_selection_hint,
                    });
                    let cfg_clone = app.config().clone();
                    populate_provider_status(app, &cfg_clone);
                }
                Err(err) => {
                    report_refresh_failure(app, &err);
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
                    // An explicit refresh asks for current state, not
                    // the daemon's last tick.
                    spawn_tracked_discovery_with(app, &self.result_tx, DaemonNudge::LiveClasses);
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
                handle_rename_overlay_key(terminal, app, config, tmux, key);
            }
            Some(Action::OpenWorktreeMenu) => open_worktree_menu_action(app),
            Some(Action::OpenWorktreeCloseDown) => open_worktree_close_down_action(app),
            Some(Action::WorktreeMenuKey(key)) => {
                handle_worktree_menu_key(terminal, app, config, tmux, key);
            }
            Some(Action::OpenNewMuxForm) => open_new_mux_form_action(app),
            Some(Action::NewMuxFormKey(key)) => {
                handle_new_mux_form_key(terminal, app, config, tmux, key);
            }
            Some(Action::OpenMuxMenu) => open_mux_menu_action(app),
            Some(Action::MuxMenuKey(key)) => handle_mux_menu_key(terminal, app, config, tmux, key),
            Some(Action::MuxLaunchFormKey(key)) => {
                handle_mux_launch_form_key(terminal, app, config, tmux, key);
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
                handle_pins_overlay_key(terminal, app, config, tmux, key);
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
            Some(Action::OpenMessages) => app.open_messages_overlay(),
            Some(Action::MessagesOverlayKey(key)) => handle_messages_overlay_key(app, key),
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
            Some(Action::OpenMessages) => app.open_messages_overlay(),
            Some(Action::MessagesOverlayKey(key)) => handle_messages_overlay_key(app, key),
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
            Some(
                Action::OpenRename
                | Action::RenameOverlayKey(_)
                | Action::OpenWorktreeMenu
                | Action::OpenWorktreeCloseDown
                | Action::WorktreeMenuKey(_)
                | Action::OpenNewMuxForm
                | Action::NewMuxFormKey(_)
                | Action::OpenMuxMenu
                | Action::MuxMenuKey(_)
                | Action::MuxLaunchFormKey(_),
            ) => {
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
/// [`event_loop`] and [`static_event_loop`] —
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
/// Shared between the live and static event loops. Only
/// KeyEventKind::Press events are forwarded — the
/// modal stack ignores repeat/release + non-key events like the
/// individual overlay handlers already did.
/// Resolve a terminal event to an action the way every key path
/// must: an open overlay owns the key, otherwise the keymap
/// translates it and the focus pass routes navigation keys to the
/// focused pane. The interactive loop and the `--snapshot-keys`
/// driver both go through here so they can't disagree about what a
/// key does.
pub(super) fn action_for_event(app: &App, event: Event, viewport_height: u16) -> Option<Action> {
    overlay_key_from_event(app, &event)
        .or_else(|| translate(event, viewport_height).and_then(|a| remap_for_focus(a, app.focus())))
}

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
    if app.messages_overlay().is_some() {
        return Some(Action::MessagesOverlayKey(key));
    }
    if app.rename_overlay().is_some() {
        return Some(Action::RenameOverlayKey(key));
    }
    if app.worktree_menu().is_some() {
        return Some(Action::WorktreeMenuKey(key));
    }
    if app.new_mux_form().is_some() {
        return Some(Action::NewMuxFormKey(key));
    }
    if app.mux_menu().is_some() {
        return Some(Action::MuxMenuKey(key));
    }
    if app.mux_launch_form().is_some() {
        return Some(Action::MuxLaunchFormKey(key));
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
    // Enable last-active-view persistence. Snapshot mode and
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
    // Enable last-active-view persistence for the static
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
    let mux_recency = app.mux_recency();
    let preview_wrap = app.preview_wrap();
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
        mux_recency,
        preview_wrap,
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
    let handle = SnapshotHandle::new(snapshot.clone());
    let initial_selection_hint = launch_context_row_id(&tree);
    app.update(Msg::SetData {
        snapshot: handle,
        tree,
        loaded_at_epoch: crate::discovery::current_epoch(),
        initial_selection_hint,
    });
    populate_provider_status(app, config);
    Ok(())
}

/// Spawn a background thread that runs discovery and sends the resolved
/// snapshot through `tx`. Row-tree building stays on the main thread so
/// it can use the app's current view state.
/// Whether a refresh first asks a running daemon to rescan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DaemonNudge {
    /// Take the daemon's snapshot as it is (timer refreshes).
    None,
    /// Rescan the mux and harness classes first; see
    /// [`nudge_daemon_live_classes`].
    LiveClasses,
}

fn spawn_discovery_worker(
    config: &RunConfig,
    tx: &mpsc::Sender<DiscoveryResult>,
    nudge: DaemonNudge,
) {
    let mut config = config.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let mut warning = None;
        if nudge == DaemonNudge::LiveClasses
            && let Err(message) = nudge_daemon_live_classes()
        {
            // The daemon's snapshot may be stale; rebuild locally so
            // the operator still sees current state.
            config.refresh = true;
            warning = Some(message);
        }
        let snapshot = discover_and_resolve(&config);
        let _ = tx.send(DiscoveryResult { snapshot, warning });
    });
}

/// Provider classes whose state changes while the operator works in
/// tmux: sessions appear, die, get renamed, and harnesses start new
/// transcripts. Both rescan in well under a second.
const LIVE_CLASSES: [&str; 2] = ["mux", "harness"];

/// Ask a running `conspectus serve` daemon to rescan
/// [`LIVE_CLASSES`] now. The daemon otherwise serves its last tick,
/// which after a tmux hand-off still shows the world from before it.
/// No daemon is not an error (the caller's refresh runs discovery
/// itself); a daemon that fails the refresh is.
pub(super) fn nudge_daemon_live_classes() -> Result<(), String> {
    use crate::server::{ClientOutcome, client_refresh};
    for class in LIVE_CLASSES {
        match client_refresh(Some(class)) {
            ClientOutcome::Ok(_) => {}
            ClientOutcome::NoDaemon => return Ok(()),
            ClientOutcome::DaemonError { code, message } => {
                return Err(format!(
                    "daemon {class} refresh failed ({code}): {message}; showing a local rebuild"
                ));
            }
            ClientOutcome::Transport(err) => {
                return Err(format!(
                    "daemon {class} refresh failed: {err:#}; showing a local rebuild"
                ));
            }
        }
    }
    Ok(())
}

/// Refresh after Conspectus handed the terminal to tmux or changed a
/// tmux session (attach, launch, rename): nudge the daemon, then
/// reload. Without the nudge the reload returns the daemon's
/// pre-hand-off snapshot.
pub(super) fn refresh_after_mux_handoff(app: &mut App, seed: &RunConfig) {
    match nudge_daemon_live_classes() {
        Ok(()) => refresh(app, seed),
        Err(message) => {
            let mut config = app.config().clone();
            config.refresh = true;
            refresh_with_config(app, &config);
            app.report(crate::tui::messages::LogEntry::warning(message));
        }
    }
}

/// Spawn a discovery worker + register the corresponding
/// `InFlightKind::Discovery` marker so the status bar renders a
/// spinner chip while the worker runs. The marker is
/// cleared when `drain` receives the worker's result.
fn spawn_tracked_discovery(app: &mut App, tx: &mpsc::Sender<DiscoveryResult>) {
    spawn_tracked_discovery_with(app, tx, DaemonNudge::None);
}

fn spawn_tracked_discovery_with(
    app: &mut App,
    tx: &mpsc::Sender<DiscoveryResult>,
    nudge: DaemonNudge,
) {
    spawn_discovery_worker(app.config(), tx, nudge);
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Discovery,
        label: "Discovering".to_string(),
    });
}

/// Run discovery and resolver on the calling thread, returning the
/// resolved snapshot.
///
/// When `conspectus serve` is reachable on the
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
/// The local-discovery branch reads the
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
    // The on-disk warm-start prior was the previous
    // graph.sqlite. With graph.sqlite retired, the daemonless
    // cold-rebuild path runs every provider from scratch each
    // tick — same as the daemon does on first cycle. Discovery
    // is single-digit seconds at target scale (ADR 0082), and
    // the TUI's typical setup runs `conspectus serve` so the
    // daemon-snapshot short-circuit above is the common path.
    let discovery_config = crate::discovery::LocalDiscoveryConfig::from_env()
        .with_caches(std::sync::Arc::clone(&config.discovery_caches));
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
    // Every other outcome is "fall back to local discovery." The daemon
    // may be absent, mid-first-cycle, or hung; any of those means the
    // local path is the right answer.
    let ClientOutcome::Ok(bytes) = client_snapshot() else {
        return None;
    };
    crate::snapshot::from_bytes(&bytes).ok()
}

/// `w`: build the worktree action menu for the current selection and
/// open it, or post a status when there is nothing to offer.
pub(super) fn open_worktree_menu_action(app: &mut App) {
    use crate::tui::widgets::worktree_menu::{WorktreeMenuState, context_for_node};
    let Some(node) = app.selection().and_then(selection_node_id) else {
        app.update(Msg::SetStatus(Some(
            "worktree: nothing selected".to_string(),
        )));
        return;
    };
    let can_mutate = worktree_mutation_available();
    let ctx = if let Some(db) = app.snapshot_handle() {
        context_for_node(db.snapshot(), &node, can_mutate)
    } else {
        app.update(Msg::SetStatus(Some(
            "worktree: no graph loaded".to_string(),
        )));
        return;
    };
    if let Some(state) = WorktreeMenuState::new(ctx) {
        app.open_worktree_menu(state);
        app.update(Msg::SetStatus(Some(
            "worktree: ↑/↓ move · Enter pick · Esc close".to_string(),
        )));
    } else {
        let hint = if can_mutate {
            "worktree: no actions for this selection"
        } else {
            "worktree: read-only — install worktrunk or set `[worktree] backend`"
        };
        app.update(Msg::SetStatus(Some(hint.to_string())));
    }
}

/// The `X` hot key: open the worktree menu straight into the close-down
/// merge/discard choice for the selected node.
pub(super) fn open_worktree_close_down_action(app: &mut App) {
    use crate::tui::widgets::worktree_menu::{WorktreeMenuState, context_for_node};
    let Some(node) = app.selection().and_then(selection_node_id) else {
        app.update(Msg::SetStatus(Some(
            "worktree: nothing selected".to_string(),
        )));
        return;
    };
    let can_mutate = worktree_mutation_available();
    let ctx = if let Some(db) = app.snapshot_handle() {
        context_for_node(db.snapshot(), &node, can_mutate)
    } else {
        app.update(Msg::SetStatus(Some(
            "worktree: no graph loaded".to_string(),
        )));
        return;
    };
    if let Some(state) = WorktreeMenuState::new_close_down(ctx) {
        app.open_worktree_menu(state);
        app.update(Msg::SetStatus(Some(
            "close down: m merge · d discard · Esc cancel".to_string(),
        )));
    } else {
        let hint = if can_mutate {
            "close down: select a worktree, mux, or checkout with a branch"
        } else {
            "worktree: read-only — install worktrunk or set `[worktree] backend`"
        };
        app.update(Msg::SetStatus(Some(hint.to_string())));
    }
}

/// Extract the underlying node id from a selected row, for node-context
/// actions like the worktree menu.
fn selection_node_id(row: &crate::tui::rows::RowId) -> Option<crate::model::NodeId> {
    use crate::tui::rows::RowId;
    match row {
        RowId::Group(id)
        | RowId::AgentSession(id)
        | RowId::MuxSession(id)
        | RowId::Pr(id)
        | RowId::Fork(id) => Some(id.clone()),
        _ => None,
    }
}

/// Whether a worktree mutation backend is available (config selection +
/// `wt` on PATH). Governs whether the menu offers mutating actions.
fn worktree_mutation_available() -> bool {
    let selection = std::env::current_dir()
        .ok()
        .map(|cwd| {
            crate::config::ConfigLoader::from_env()
                .load_from(&cwd)
                .config
                .worktree
                .backend
        })
        .unwrap_or_default();
    crate::discovery::worktree::resolve_mutation_backend(
        selection,
        crate::discovery::worktree::worktrunk_available(),
    )
    .ok()
    .flatten()
    .is_some()
}

/// Open the bare-tmux new-session form (`n`), seeding cwd from the
/// selected row's cwd when it resolves to a repo, checkout, mux, or
/// agent-session row; otherwise fall back to `$HOME`. Name defaults
/// to a unique-per-live-mux slug derived from the selected row when
/// possible, else empty (operator types it in).
pub(super) fn open_new_mux_form_action(app: &mut App) {
    use crate::tui::widgets::new_mux::NewMuxFormState;
    let seeded_cwd = derive_new_mux_cwd(app).unwrap_or_else(default_new_mux_cwd);
    let seeded_name = derive_new_mux_name(app);
    app.open_new_mux_form(NewMuxFormState::new(seeded_name, seeded_cwd));
    app.update(Msg::SetStatus(Some(
        "mux: Tab switch field · Enter next/commit · Esc cancel".to_string(),
    )));
}

/// Public alias for [`derive_new_mux_cwd`] so the reducer's
/// `Msg::OpenNewMuxForm` / `Msg::OpenMuxLaunchForm` arms can seed
/// the cwd from the selected row (ADR 0096).
pub(crate) fn derive_mux_form_cwd(app: &App) -> Option<String> {
    derive_new_mux_cwd(app)
}

/// Known harness keys from the registered adapters. Used by the
/// mux-launch form's `known_harness_keys` autocomplete + cycling.
pub(crate) fn known_harness_keys() -> Vec<String> {
    let mut keys: Vec<String> = crate::discovery::harness::registered_adapters()
        .map(|a| a.harness_key().to_string())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Known live tmux session names from the current graph snapshot.
/// Used by the mux-launch form's name-collision check. Strips the
/// `<backend>:` prefix so the compared value matches what the operator
/// types (tmux uses `tmux:<socket>:<name>` on non-default sockets and
/// `tmux:<name>` on the default socket per ADR 0057).
pub(crate) fn known_live_mux_names(app: &App) -> Vec<String> {
    use crate::model::GraphNode;
    let Some(db) = app.snapshot_handle() else {
        return Vec::new();
    };
    let mut names: Vec<String> = db
        .snapshot()
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::MuxSession(m) => Some(strip_tmux_prefix(&m.native_id)),
            _ => None,
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

fn strip_tmux_prefix(native_id: &str) -> String {
    match native_id.strip_prefix("tmux:") {
        Some(rest) => match rest.split_once(':') {
            Some((_socket, name)) => name.to_string(),
            None => rest.to_string(),
        },
        None => native_id.to_string(),
    }
}

/// Derive a default mux name for the mux-launch form. Today this
/// mirrors [`derive_new_mux_name`] (empty — operator picks); a
/// future refinement (slugify repo basename etc.) has one place to
/// land.
pub(crate) fn derive_mux_launch_name(app: &App) -> Option<String> {
    let name = derive_new_mux_name(app);
    if name.is_empty() { None } else { Some(name) }
}

/// Read the selected row's cwd (repo / checkout / mux / agent
/// session). Returns `None` when nothing is selected or the row has
/// no cwd we can inherit.
fn derive_new_mux_cwd(app: &App) -> Option<String> {
    use crate::model::GraphNode;
    use crate::tui::rows::RowKind;
    let selection = app.selection()?.clone();
    let rows = app.visible_rows();
    let row = rows.iter().find(|row| row.id == selection)?;
    let snapshot = app.snapshot_handle()?.snapshot();
    match &row.kind {
        RowKind::Group(group) => group
            .primary_node
            .as_ref()
            .and_then(|node| cwd_for_node(snapshot, node)),
        RowKind::Repo(r) => cwd_for_node(snapshot, &r.primary_node),
        RowKind::MuxSession(mux) => snapshot.nodes.iter().find_map(|n| match n {
            GraphNode::MuxSession(m) if m.id == mux.mux => {
                m.active_pane_current_path.clone().or_else(|| m.cwd.clone())
            }
            _ => None,
        }),
        RowKind::AgentSession(session) => snapshot.nodes.iter().find_map(|n| match n {
            GraphNode::AgentSession(s) if s.id == session.session => s.cwd.clone(),
            _ => None,
        }),
        _ => None,
    }
}

fn cwd_for_node(
    snapshot: &crate::model::GraphSnapshot,
    node: &crate::model::NodeId,
) -> Option<String> {
    use crate::model::{GraphNode, NodeId};
    snapshot.nodes.iter().find_map(|n| match (node, n) {
        (NodeId::Checkout(id), GraphNode::Checkout(c)) if &c.id == id => Some(c.root.clone()),
        (NodeId::Repo(id), GraphNode::Repo(r)) if &r.id == id => r.source_paths.first().cloned(),
        (NodeId::MuxSession(id), GraphNode::MuxSession(m)) if &m.id == id => {
            m.active_pane_current_path.clone().or_else(|| m.cwd.clone())
        }
        (NodeId::AgentSession(id), GraphNode::AgentSession(s)) if &s.id == id => s.cwd.clone(),
        _ => None,
    })
}

fn default_new_mux_cwd() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

/// Derive a default session-name seed from the selected row. Today
/// this is intentionally empty — the operator picks a name that fits
/// their workflow, and tmux forbids some of the punctuation we might
/// derive from a path anyway. Kept as a helper so a future refinement
/// (e.g. slugify the repo basename) has one place to land.
fn derive_new_mux_name(_app: &App) -> String {
    String::new()
}

/// Open the Mux action menu (`m`). Context-free — the menu itself
/// derives per-entry defaults when its selection commits.
pub(super) fn open_mux_menu_action(app: &mut App) {
    use crate::tui::widgets::mux_menu::MuxMenuState;
    app.open_mux_menu(MuxMenuState::new());
    app.update(Msg::SetStatus(Some(
        "mux: ↑/↓ move · Enter pick · Esc close".to_string(),
    )));
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
    let RowId::Pin { pin_id } = selection else {
        app.set_pending_pin_remove(None);
        app.update(Msg::SetStatus(Some(
            "pin remove: select an unbound pin row".to_string(),
        )));
        return;
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
    // `N` is the new-stream entry (ADR 0094): open the create form with
    // the worktree toggle pre-enabled. Plain pin creation stays one
    // keystroke away via the `p` pins menu.
    let ctx = app.pins_context();
    let state = crate::tui::widgets::pins::PinsOverlayState::open_with_new_stream_context(&ctx);
    app.set_pins_overlay(state);
    app.update(Msg::SetStatus(Some(
        "pins: new stream · Space toggles worktree · Enter create · Esc cancel".to_string(),
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
            let handle = SnapshotHandle::new(snapshot);
            let initial_selection_hint = launch_context_row_id(&tree);
            app.update(Msg::SetData {
                snapshot: handle,
                tree,
                loaded_at_epoch: crate::discovery::current_epoch(),
                initial_selection_hint,
            });
        }
        Err(err) => report_refresh_failure(app, &err),
    }
}

/// Mark the snapshot stale and log why (ADR 0105). The previous good
/// snapshot stays on screen.
fn report_refresh_failure(app: &mut App, err: &anyhow::Error) {
    app.update(Msg::SetRefreshFailure(format!(
        "last refresh failed; {err}"
    )));
    app.report(crate::tui::messages::LogEntry::error(format!(
        "refresh failed: {err:#}"
    )));
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
    let handle = SnapshotHandle::new(resolved);
    let initial_selection_hint = launch_context_row_id(&tree);
    app.update(Msg::SetData {
        snapshot: handle,
        tree,
        loaded_at_epoch: crate::discovery::current_epoch(),
        initial_selection_hint,
    });
    Ok(())
}

pub(super) use crate::tui::keymap::{Action, SelectedDefault, selected_default_action};

pub(super) use crate::tui::keymap::cycle_view;

/// Dispatch `Enter` on the left pane to the selected
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

/// Right-pane Enter. When the explorer cursor is on
/// a Node-zone field row with a copyable value, write it to the
/// clipboard via OSC 52 (ADR 0056) and post a toast. Otherwise
/// dispatch the normal `Msg::ExplorerActivate` so group expansion and
/// link drill still work.
fn explorer_enter_action(app: &mut App) {
    explorer_enter(app, |value| {
        let _ = crate::tui::clipboard::copy_to_clipboard(value);
    });
}

/// [`explorer_enter_action`] with the clipboard write supplied by the
/// caller, so the snapshot driver can run the same branch without
/// emitting OSC 52 into its frame output.
pub(super) fn explorer_enter(app: &mut App, write_clipboard: impl FnOnce(&str)) {
    if let Some((label, value)) = app.explorer_copy_target() {
        write_clipboard(&value);
        app.post_toast(format!("copied: {label}"));
        return;
    }
    app.update(Msg::ExplorerActivate);
}

/// `i` keybinding. Copies the selected agent or mux session's
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

pub(super) use crate::tui::keymap::{remap_for_focus, translate};

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
