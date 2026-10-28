//! Terminal lifecycle and event loop.
//!
//! `term` takes the terminal over and hands it back (with a panic
//! hook that restores it), so this module just sets up the loop and is
//! the only place in the crate that touches stdout in raw mode.
//!
//! Discovery runs on a background thread per ADR 0024 (`mpsc` + no
//! async runtime). The initial load still runs synchronously so the
//! operator sees a populated tree on the first frame; subsequent
//! refreshes (manual `r` or timer-driven) dispatch through the
//! background channel and never block input.

use std::process::Output;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::Rect;

use crate::discovery::tmux::{MuxBackend, SystemTmux};
use crate::model::MuxSessionId;
use crate::resolve::resolve_snapshot;
#[cfg(test)]
use crate::tui::View;
use crate::tui::actions::AttachTarget;
use crate::tui::app::{App, Msg, SnapshotHandle};
use crate::tui::effect::Effect;
use crate::tui::keybindings::overlay as overlay_keymap;
use crate::tui::preview::{
    PreviewContent, PreviewSource, capture_via, load_offline_preview, pane_native_id,
    preview_source,
};
use crate::tui::resume::{ResumeTarget, launch_resume, resume_disabled_reason};
use crate::tui::rows::RowTree;
use crate::tui::viewer::{
    LaunchPlan, PathBinaryProbe, ViewerTarget, resolve_viewer_target, viewer_disabled_reason,
};
use crate::tui::{RunConfig, ui};

/// Result of a completed background discovery run. The worker returns
/// only the resolved snapshot; the main thread builds the row tree from
/// the app's current view config.
mod content;
mod executor;
mod launch;
mod overlay_keys;
mod term;
mod visibility;
mod worktree_exec;

use executor::*;
use launch::*;
use worktree_exec::*;

#[cfg(test)]
pub(super) use overlay_keys::modal_key_pure;
pub(super) use overlay_keys::{modal_key, open_rename_overlay};

/// What a background discovery worker sends back. `warning` carries a
/// problem the worker worked around, such as a failed daemon nudge
/// that it answered with a local rebuild.
struct DiscoveryResult {
    generation: u64,
    snapshot: Result<crate::model::GraphSnapshot>,
    warning: Option<String>,
    /// The snapshot came from a running daemon.
    from_daemon: bool,
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

    /// Reference to the mode's mux runner. Used to capture pane
    /// previews.
    fn tmux(&self) -> &dyn MuxBackend;

    /// Load a preview the selection needs (ADR 0112 §5). The default
    /// loads on the calling thread; live mode moves transcript reads
    /// to a worker.
    fn load_preview(&mut self, app: &mut App, source: PreviewSource) {
        load_preview_now(app, self.tmux(), source);
    }
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

    // The preview source the last pass resolved, and whether the last
    // key asked for a refresh.
    let mut shown: Option<PreviewSource> = None;
    let mut force_preview = false;
    while !app.should_quit() {
        // A contents query that has rested starts its scan.
        app.search_overlay_tick(Instant::now(), false);
        mode.drain(&mut app, &config)?;
        shown = refresh_preview_if_needed(&mut app, &config, &mut mode, shown, force_preview);
        draw_frame(&mut app, terminal)?;
        force_preview = false;
        if event::poll(poll_timeout)? {
            let event = event::read()?;
            if let Event::Paste(text) = &event {
                let mut dispatched = Ok(());
                type_paste(&mut app, text, &mut |app, action| {
                    if dispatched.is_ok() {
                        dispatched = mode.dispatch(terminal, app, &config, Some(action));
                    }
                });
                dispatched?;
                continue;
            }
            let action = action_for_event(&app, event);
            force_preview = matches!(action, Some(Action::Refresh | Action::HardRefresh));
            mode.dispatch(terminal, &mut app, &config, action)?;
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
    preview_tx: mpsc::Sender<(PreviewSource, PreviewContent)>,
    preview_rx: mpsc::Receiver<(PreviewSource, PreviewContent)>,
    workers: WorkerLedger,
    last_refresh: Instant,
    refresh_interval: Duration,
    forge: ForgeWorker,
    /// Inside tmux: tells when this pane comes back into view.
    visibility: Option<visibility::VisibilityWatcher>,
    /// Runs the content search scans the reducer asks for.
    content: content::ContentDriver,
}

/// The background forge fetch a TUI runs while it has no daemon to do
/// it (ADR 0136): at most one at a time, at most every [`FORGE_POLL`].
struct ForgeWorker {
    tx: mpsc::Sender<usize>,
    rx: mpsc::Receiver<usize>,
    running: bool,
    last_started: Option<Instant>,
    /// The last discovery came from local providers, not a daemon.
    daemonless: bool,
}

impl ForgeWorker {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            running: false,
            last_started: None,
            daemonless: false,
        }
    }

    /// Whether a fetch should start now.
    fn due(&self, now: Instant) -> bool {
        self.daemonless
            && !self.running
            && self
                .last_started
                .is_none_or(|at| now.duration_since(at) >= FORGE_POLL)
    }
}

/// How often a daemonless TUI looks for forge listings to fetch.
const FORGE_POLL: Duration = Duration::from_secs(10);

/// Bookkeeping for background discovery workers. Each spawn takes the
/// next generation. Results apply only when newer than the last one
/// applied, so a slow worker spawned before a tmux hand-off can't
/// overwrite the post-hand-off snapshot (ADR 0108).
#[derive(Debug, Default)]
struct WorkerLedger {
    next: u64,
    in_flight: usize,
    applied: u64,
    /// First generation spawned after the latest hand-off; its result
    /// (or any newer one) settles the hand-off's pending rows.
    settles_handoff_at: Option<u64>,
}

/// What [`WorkerLedger::finish`] decided about one worker's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkerVerdict {
    /// The result is newer than what's on screen.
    apply: bool,
    /// The result settles the pending hand-off.
    settles_handoff: bool,
}

impl WorkerLedger {
    fn start(&mut self) -> u64 {
        self.next += 1;
        self.in_flight += 1;
        self.next
    }

    fn start_for_handoff(&mut self) -> u64 {
        let generation = self.start();
        self.settles_handoff_at = Some(generation);
        generation
    }

    fn finish(&mut self, generation: u64) -> WorkerVerdict {
        self.in_flight = self.in_flight.saturating_sub(1);
        let apply = generation > self.applied;
        if apply {
            self.applied = generation;
        }
        let settles_handoff = apply && self.settles_handoff_at.is_some_and(|at| generation >= at);
        if settles_handoff {
            self.settles_handoff_at = None;
        }
        WorkerVerdict {
            apply,
            settles_handoff,
        }
    }

    fn idle(&self) -> bool {
        self.in_flight == 0
    }
}

impl LiveMode {
    /// Spawn a discovery worker and show the status-bar spinner while
    /// any worker runs.
    fn spawn_discovery(&mut self, app: &mut App, nudge: DaemonNudge, generation: u64) {
        self.last_refresh = Instant::now();
        spawn_discovery_worker(app.config(), &self.result_tx, nudge, generation);
        let label = match nudge {
            DaemonNudge::LiveAndForge => "Refreshing forges",
            DaemonNudge::None | DaemonNudge::LiveClasses => "Discovering",
        };
        app.update(Msg::InFlightStart {
            kind: crate::tui::app::InFlightKind::Discovery,
            label: label.to_string(),
        });
    }

    /// Start the background refreshes the reducer asked for: the one
    /// after returning from tmux, and the one after an action changed
    /// state. Runs after every action and every drain, since a request
    /// can arrive without a key press (the TUI's pane coming back into
    /// view).
    fn start_requested_refreshes(&mut self, app: &mut App) {
        // Returning from tmux paints the old snapshot right away; the
        // live classes rescan in the background even if another
        // worker is already running, since that one predates the
        // hand-off (ADR 0108).
        if app.take_handoff_refresh_request() {
            let generation = self.workers.start_for_handoff();
            let before = app
                .snapshot_handle()
                .map(|handle| live_mux_ids(handle.snapshot()));
            self.last_refresh = Instant::now();
            spawn_discovery_worker_settling(
                app.config(),
                &self.result_tx,
                DaemonNudge::for_request(&crate::tui::effect::RefreshRequest::LIVE),
                generation,
                before,
            );
            app.update(Msg::InFlightStart {
                kind: crate::tui::app::InFlightKind::Discovery,
                label: "Discovering".to_string(),
            });
        }
        // An action changed state the views show: reload them the way
        // `r` does, through the daemon when there is one, so nothing
        // the daemon already knows (PRs included) is lost.
        if app.take_refresh_request() {
            let generation = self.workers.start();
            let config = store_write_refresh_config(app);
            self.last_refresh = Instant::now();
            spawn_discovery_worker(
                &config,
                &self.result_tx,
                DaemonNudge::for_request(&crate::tui::effect::RefreshRequest::LIVE),
                generation,
            );
            app.update(Msg::InFlightStart {
                kind: crate::tui::app::InFlightKind::Discovery,
                label: "Discovering".to_string(),
            });
        }
    }
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
        let generation = self.workers.start();
        self.spawn_discovery(app, DaemonNudge::None, generation);
        // No preview loads yet: nothing is selected until the worker
        // delivers, and the run loop resolves the preview each pass.
        Ok(())
    }

    fn drain(&mut self, app: &mut App, config: &RunConfig) -> Result<()> {
        self.content.pump(app);
        while let Ok((source, content)) = self.preview_rx.try_recv() {
            app.update(Msg::SetPreview { source, content });
        }
        // The operator came back to this pane (inside tmux an attach
        // returns as soon as the client is switched away): whatever
        // they did in the other session is not on screen yet.
        if let Some(watcher) = &self.visibility {
            let mut returned = false;
            while watcher.rx.try_recv().is_ok() {
                returned = true;
            }
            if returned {
                app.update(Msg::ViewRegained);
            }
        }
        // Drain completed background discovery results without
        // blocking. A result older than the one on screen is dropped.
        // A finished background forge fetch: refresh to merge what it
        // brought in, unless a refresh is already running (its result
        // reads the same cache).
        while let Ok(fetched) = self.forge.rx.try_recv() {
            self.forge.running = false;
            if fetched > 0 && self.workers.idle() {
                let generation = self.workers.start();
                self.spawn_discovery(app, DaemonNudge::None, generation);
            }
        }
        while let Ok(DiscoveryResult {
            generation,
            snapshot,
            warning,
            from_daemon,
        }) = self.result_rx.try_recv()
        {
            if snapshot.is_ok() {
                self.forge.daemonless = !from_daemon;
            }
            let verdict = self.workers.finish(generation);
            if self.workers.idle() {
                app.update(Msg::InFlightFinish(
                    crate::tui::app::InFlightKind::Discovery,
                ));
            }
            if let Some(warning) = warning {
                app.report(crate::tui::messages::LogEntry::warning(warning));
            }
            if !verdict.apply {
                continue;
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
            if verdict.settles_handoff {
                app.update(Msg::HandoffSettled);
            }
        }
        let _ = config;
        // Timer-driven auto-refresh. Only fires when no request
        // is in-flight and at least `refresh_interval` has
        // elapsed.
        if self.workers.idle() && self.last_refresh.elapsed() >= self.refresh_interval {
            let generation = self.workers.start();
            self.spawn_discovery(app, DaemonNudge::None, generation);
        }
        // Without a daemon, this TUI keeps its own forge listings
        // current in the background (ADR 0136).
        if self.forge.due(Instant::now())
            && let Some(snapshot) = app
                .snapshot_handle()
                .map(|handle| handle.snapshot().clone())
        {
            self.forge.running = true;
            self.forge.last_started = Some(Instant::now());
            spawn_forge_fetch(app.config(), &self.forge.tx, snapshot);
        }
        self.start_requested_refreshes(app);
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
        if action.as_ref().is_some_and(acts_on_selected_row) && app.selected_row_awaits_handoff() {
            // The row shows pre-hand-off values: its mux or session may
            // be gone, so an action on it could fail (ADR 0132).
            app.update(Msg::SetStatus(Some(
                crate::tui::app::HANDOFF_PENDING_HINT.to_string(),
            )));
            return Ok(());
        }
        match action {
            // Key-bound messages emit effects (pinning, rebuilding,
            // launching) that only the live executor can run.
            Some(Action::Msg(msg)) => match live_executor_for(&Action::Msg(msg.clone())) {
                ExecutorKind::Live => dispatch_live(terminal, app, config, tmux, *msg),
                ExecutorKind::Pure => dispatch(app, *msg),
            },
            Some(Action::Refresh) => {
                app.update(Msg::RescanContent);
                if self.workers.idle() {
                    // An explicit refresh asks for current state, not
                    // the daemon's last tick.
                    let generation = self.workers.start();
                    self.spawn_discovery(app, DaemonNudge::LiveClasses, generation);
                }
            }
            Some(Action::HardRefresh) => {
                // Spawned even while another worker runs: the operator
                // asked for the forges now, and the newer generation
                // wins when results land.
                app.update(Msg::ForgeRefreshRequested);
                app.update(Msg::RescanContent);
                app.config().discovery_caches.clear_forge();
                let generation = self.workers.start();
                self.spawn_discovery(app, DaemonNudge::LiveAndForge, generation);
            }
            // What a modal commits can need a terminal, a mux, or a store
            // (a pick, a submit, a hit that opens the viewer).
            Some(Action::ModalKey(key)) => {
                modal_key(app, key, &mut live_send(terminal, config, tmux));
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
            Some(Action::OpenWorktreeMenu) => open_worktree_menu_action(app),
            Some(Action::OpenWorktreeCloseDown) => open_worktree_close_down_action(app),
            Some(Action::OpenNewMuxForm) => open_new_mux_form_action(app),
            Some(Action::OpenMuxMenu) => open_mux_menu_action(app),
            Some(Action::OpenControls) => {
                app.open_controls_overlay();
                app.update(Msg::SetStatus(Some(format!(
                    "controls: {}",
                    tuikit::keys::footer(overlay_keymap::CONTROLS_KEYS)
                ))));
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
                dispatch(app, Msg::ClearFilters);
                app.update(Msg::SetStatus(Some("filters cleared".to_string())));
            }
            Some(Action::OpenSearch) => {
                let hint = app.open_search_entry();
                app.update(Msg::SetStatus(Some(hint)));
            }
            Some(Action::OpenContentSearch) => {
                let hint = app.open_content_search_entry();
                app.update(Msg::SetStatus(Some(hint)));
            }
            Some(Action::OpenHelp) => {
                app.open_help_overlay();
            }
            Some(Action::OpenMessages) => app.open_messages_overlay(),
            Some(Action::OpenValueModal) => app.open_value_modal_for_cursor(),
            Some(Action::ExplorerEnter) => explorer_enter_action(app),
            Some(Action::CopySessionId) => copy_session_id_action(app),
            None => {}
        }
        self.start_requested_refreshes(app);
        Ok(())
    }

    fn tmux(&self) -> &dyn MuxBackend {
        self.tmux.as_ref()
    }

    /// Transcripts, listings, and files read from disk, which can be
    /// slow, so they load on a worker thread and land through
    /// [`LoopMode::drain`]. Pane captures stay synchronous.
    fn load_preview(&mut self, app: &mut App, source: PreviewSource) {
        if matches!(source, PreviewSource::Pane(_)) {
            load_preview_now(app, self.tmux(), source);
            return;
        }
        let tx = self.preview_tx.clone();
        let muted = app.theme().muted;
        std::thread::spawn(move || {
            // A loader panic still has to end the load, or the preview
            // would wait on it forever and `r` couldn't retry.
            let content = std::panic::catch_unwind(|| load_offline_preview(&source, muted))
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    PreviewContent::Failed("the preview reader crashed".to_string())
                });
            // The receiver is gone only when the TUI is exiting.
            let _ = tx.send((source, content));
        });
    }
}

/// Static-mode loop state: fixture-replay + scenario TUI. Its
/// `dispatch` blocks mutating actions with a status hint and
/// reloads the fixture on `Action::Refresh`. Overlays take their keys
/// as in the live loop; what they commit runs on the pure executor.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
struct StaticMode {
    tmux: Box<dyn MuxBackend>,
    snapshot: crate::model::GraphSnapshot,
    fixture_path: Option<std::path::PathBuf>,
    /// A scenario world's root, whose recorded `gh pr view` output
    /// answers PR previews.
    recordings: Option<std::path::PathBuf>,
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
impl StaticMode {
    /// What [`LoopMode::dispatch`] does; it needs no terminal.
    fn dispatch_action(&mut self, app: &mut App, config: &RunConfig, action: Option<Action>) {
        match action {
            Some(Action::Msg(msg)) => dispatch(app, *msg),
            // A modal opened any way (`.`, a menu pick, a direct opener)
            // takes its keys as in the live TUI, so it moves and closes;
            // the pure executor refuses a pick that needs a terminal, a
            // mux, or a store.
            Some(Action::ModalKey(key)) => modal_key(app, key, &mut dispatch),
            Some(Action::OpenHelp) => app.open_help_overlay(),
            Some(Action::OpenMessages) => app.open_messages_overlay(),
            Some(Action::OpenValueModal) => app.open_value_modal_for_cursor(),
            Some(Action::OpenSearch) => {
                let hint = app.open_search_entry();
                app.update(Msg::SetStatus(Some(hint)));
            }
            Some(Action::OpenContentSearch) => {
                let hint = app.open_content_search_entry();
                app.update(Msg::SetStatus(Some(hint)));
            }
            Some(Action::OpenControls) => {
                app.open_controls_overlay();
                app.update(Msg::SetStatus(Some(format!(
                    "controls: {}",
                    tuikit::keys::footer(overlay_keymap::CONTROLS_KEYS)
                ))));
            }
            Some(Action::SwitchView(view)) => dispatch(app, Msg::SwitchView(view)),
            Some(Action::CycleView(delta)) => {
                let next = cycle_view(app.active_view(), delta);
                dispatch(app, Msg::SwitchView(next));
            }
            Some(action @ (Action::Refresh | Action::HardRefresh)) => {
                if action == Action::HardRefresh {
                    // Recorded PR previews reload; nothing asks a forge.
                    app.update(Msg::ForgeRefreshRequested);
                }
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
                            set_static_data(app, config, &self.snapshot);
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
                    set_static_data(app, config, &self.snapshot);
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
                    SelectedDefault::Scrollback => Msg::SetStatus(Some(
                        "scenario TUI is static; scrollback is disabled".to_string(),
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
                dispatch(app, Msg::ClearFilters);
                app.update(Msg::SetStatus(Some("filters cleared".to_string())));
            }
            Some(
                Action::OpenRename
                | Action::OpenWorktreeMenu
                | Action::OpenWorktreeCloseDown
                | Action::OpenNewMuxForm
                | Action::OpenMuxMenu,
            ) => {
                app.update(Msg::SetStatus(Some(
                    "scenario TUI keeps mutating actions disabled".to_string(),
                )));
            }
            Some(Action::ExplorerEnter) => explorer_enter_action(app),
            Some(Action::CopySessionId) => copy_session_id_action(app),
            None => {}
        }
    }
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
impl LoopMode for StaticMode {
    fn init(
        &mut self,
        _terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
    ) -> Result<()> {
        set_static_data(app, config, &self.snapshot);
        Ok(())
    }

    fn drain(&mut self, app: &mut App, _config: &RunConfig) -> Result<()> {
        // Scenario mode has no async event sources: no discovery
        // worker to poll and no refresh timer to arm. A requested
        // content search runs to completion here.
        content::run_blocking(app, None);
        Ok(())
    }

    fn dispatch(
        &mut self,
        _terminal: &mut DefaultTerminal,
        app: &mut App,
        config: &RunConfig,
        action: Option<Action>,
    ) -> Result<()> {
        self.dispatch_action(app, config, action);
        Ok(())
    }

    fn tmux(&self) -> &dyn MuxBackend {
        self.tmux.as_ref()
    }

    /// A scenario's recorded `gh pr view` output answers its PRs;
    /// everything else loads as usual.
    fn load_preview(&mut self, app: &mut App, source: PreviewSource) {
        if let Some(root) = &self.recordings
            && let Some(content) = recorded_preview(&source, root, app.theme().muted)
        {
            app.update(Msg::SetPreview { source, content });
            return;
        }
        load_preview_now(app, self.tmux(), source);
    }
}

/// Prepare the toast area and render one frame. Shared between
/// [`event_loop`] and [`static_event_loop`] —
/// the toast prep + viewer-or-ui draw block was byte-identical
/// in both loops.
fn draw_frame(app: &mut App, terminal: &mut DefaultTerminal) -> Result<()> {
    let size = terminal.size()?;
    app.prepare_toast_for_render(Rect::new(0, 0, size.width, size.height));
    terminal.draw(|frame| draw_screen(app, frame))?;
    Ok(())
}

/// Draw the whole screen (the panes, then every open modal bottom to
/// top, then a toast) and keep the windows the panes drew. The
/// interactive loops and `--snapshot` share it.
pub(super) fn draw_screen(app: &mut App, frame: &mut ratatui::Frame<'_>) {
    let report = ui::draw(app, frame);
    app.keep_viewports(report);
}

/// Resolve a terminal event to an action the way every key path
/// must: an open overlay owns the key, otherwise the keymap
/// translates it and the focus pass routes navigation keys to the
/// focused pane. The interactive loop and the `--snapshot-keys`
/// driver both go through here so they can't disagree about what a
/// key does. Page keys move the tree by the rows it drew in the last
/// frame.
pub(super) fn action_for_event(app: &App, event: Event) -> Option<Action> {
    overlay_key_from_event(app, &event).or_else(|| {
        translate(event, app.left_viewport()).and_then(|a| remap_for_focus(a, app.focus()))
    })
}

/// Type a bracketed paste into the text field that has the keys, one key
/// per character, so each field edits, completes, and refilters exactly
/// as it does for typing. The text is flattened to one line first
/// (`single_line_paste`), so a paste never presses Enter or Tab.
/// `dispatch` handles each key's action the way the loop handles a typed
/// key. A paste goes only into a text field (`App::text_field_has_keys`):
/// with none focused it does nothing, and typing stops if a key leaves
/// the field, so pasted characters never act as the keys of a menu, a
/// choice, or the tree.
pub(super) fn type_paste(app: &mut App, text: &str, dispatch: &mut dyn FnMut(&mut App, Action)) {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for c in crate::tui::widgets::input::single_line_paste(text).chars() {
        if !app.text_field_has_keys() {
            break;
        }
        let key = Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        let Some(action) = overlay_key_from_event(app, &key) else {
            break;
        };
        dispatch(app, action);
    }
}

/// The key press an open modal owns, as [`Action::ModalKey`] for the
/// modal stack (ADR 0085 contract 3, ADR 0161); `None` when no modal is
/// open or the event is not a press, so the caller translates it with the
/// main keymap. Repeats, releases, and other events never reach a modal.
fn overlay_key_from_event(app: &App, event: &Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press && app.has_modal() => {
            Some(Action::ModalKey(*key))
        }
        _ => None,
    }
}

/// Run the TUI to completion. Restores the terminal on normal exit,
/// errors, and panics (the panic path is covered by the hooks
/// `term::enter` installs).
pub fn run(config: RunConfig) -> Result<()> {
    let mut terminal = term::enter();
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
    term::leave();
    result
}

/// Run a static, pre-materialized graph in the TUI. This is for
/// debug-only replay scenarios: it lets developers inspect edge-case
/// worlds through the real renderer and reducer without teaching
/// production discovery about fixtures.
/// `tmux` answers pane captures; a scenario passes its recorded panes,
/// and `recordings`, its world's root, answers PR previews from the
/// `gh pr view` output it recorded.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub fn run_static(
    config: RunConfig,
    snapshot: crate::model::GraphSnapshot,
    tmux: Box<dyn MuxBackend>,
    recordings: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut terminal = term::enter();
    let _ = terminal.clear();
    let result = static_event_loop(&mut terminal, config, snapshot, None, tmux, recordings);
    term::leave();
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
    let mut terminal = term::enter();
    let _ = terminal.clear();
    let result = static_event_loop(
        &mut terminal,
        config,
        snapshot,
        Some(fixture_path),
        Box::new(SystemTmux::new()),
        None,
    );
    term::leave();
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
fn event_loop(terminal: &mut DefaultTerminal, mut config: RunConfig) -> Result<()> {
    // A live TUI's refreshes never wait on a forge: they build PRs from
    // cached listings, a background worker fetches them, and only
    // Ctrl-R fetches in its refresh (ADR 0136).
    config.forge_fetch = crate::discovery::forge::ForgeFetch::CachedOnly;
    let mut app = App::new(config.clone());
    // Enable last-active-view persistence. Snapshot mode and
    // `--no-resume-view` both go through a different entry point
    // (or skip this branch) so the file only ever moves under
    // genuine interactive runs.
    app.enable_view_persistence(crate::tui_state::TuiStateCache::from_env());
    app.restore_persisted_state();
    let (result_tx, result_rx) = mpsc::channel::<DiscoveryResult>();
    let (preview_tx, preview_rx) = mpsc::channel();
    let refresh_interval = config.refresh_interval;
    let mode = LiveMode {
        tmux: Box::new(SystemTmux::new()),
        result_tx,
        result_rx,
        preview_tx,
        preview_rx,
        workers: WorkerLedger::default(),
        last_refresh: Instant::now(),
        refresh_interval,
        forge: ForgeWorker::new(),
        visibility: visibility::VisibilityWatcher::start(),
        content: content::ContentDriver::live(),
    };
    run_loop(terminal, app, config, mode)
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn static_event_loop(
    terminal: &mut DefaultTerminal,
    config: RunConfig,
    initial_snapshot: crate::model::GraphSnapshot,
    fixture_path: Option<std::path::PathBuf>,
    tmux: Box<dyn MuxBackend>,
    recordings: Option<std::path::PathBuf>,
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
        tmux,
        snapshot: initial_snapshot,
        fixture_path,
        recordings,
    };
    run_loop(terminal, app, config, mode)
}

#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn set_static_data(app: &mut App, config: &RunConfig, snapshot: &crate::model::GraphSnapshot) {
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
}

/// Handle one terminal event the way `tui --fixture` and the scenario
/// TUI do: the loop's key routing, then their dispatch. For tests, which
/// have no terminal; `r` reloads the snapshot `app` holds.
#[cfg(test)]
pub(in crate::tui) fn press_in_fixture_mode(app: &mut App, event: Event) {
    let mut mode = StaticMode {
        tmux: Box::new(crate::discovery::tmux::FakeTmux::with_sessions("")),
        snapshot: app
            .snapshot_handle()
            .map(|handle| handle.snapshot().clone())
            .unwrap_or_default(),
        fixture_path: None,
        recordings: None,
    };
    let config = app.config().clone();
    let action = action_for_event(app, event);
    mode.dispatch_action(app, &config, action);
}

/// Whether a refresh first asks a running daemon to rescan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui) enum DaemonNudge {
    /// Take the daemon's snapshot as it is (timer refreshes).
    None,
    /// Rescan the mux and harness classes first; see
    /// [`nudge_daemon_live_classes`].
    LiveClasses,
    /// Rescan the live classes and ask the forges again (Ctrl-R,
    /// ADR 0116).
    LiveAndForge,
}

impl DaemonNudge {
    /// The nudge that carries out `request`.
    pub(in crate::tui) fn for_request(request: &crate::tui::effect::RefreshRequest) -> Self {
        if request.classes.contains(&"forge") {
            Self::LiveAndForge
        } else {
            Self::LiveClasses
        }
    }

    /// Whether the refresh must ask the mux server again instead of
    /// reusing a cached listing.
    pub(in crate::tui) fn rescans_mux(self) -> bool {
        self.classes().contains(&"mux")
    }

    /// The daemon classes to rescan before taking its snapshot.
    pub(in crate::tui) fn classes(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::LiveClasses => crate::tui::effect::RefreshRequest::LIVE.classes,
            Self::LiveAndForge => crate::tui::effect::RefreshRequest::LIVE_AND_FORGE.classes,
        }
    }
}

/// Spawn a background thread that runs discovery and sends the resolved
/// snapshot through `tx`. Row-tree building stays on the main thread so
/// it can use the app's current view state.
fn spawn_discovery_worker(
    config: &RunConfig,
    tx: &mpsc::Sender<DiscoveryResult>,
    nudge: DaemonNudge,
    generation: u64,
) {
    spawn_discovery_worker_settling(config, tx, nudge, generation, None);
}

/// [`spawn_discovery_worker`] for the refresh after a mux hand-off.
/// `before` is the live mux set the screen showed before the hand-off;
/// a result that equals it is probed again a moment later (see
/// [`SETTLE_TRIES`]), since the hand-off nearly always changes it.
fn spawn_discovery_worker_settling(
    config: &RunConfig,
    tx: &mpsc::Sender<DiscoveryResult>,
    nudge: DaemonNudge,
    generation: u64,
    before: Option<std::collections::BTreeSet<crate::model::NodeId>>,
) {
    let mut config = config.clone();
    config.forge_fetch = worker_forge_fetch(nudge, config.forge_fetch);
    let tx = tx.clone();
    std::thread::spawn(move || {
        let mut warning = None;
        let (snapshot, from_daemon) = probe_until_changed(
            before.as_ref(),
            SETTLE_TRIES,
            SETTLE_DELAY,
            std::thread::sleep,
            || {
                prepare_rescan(&config, nudge);
                if let Err(message) = nudge_daemon(nudge.classes()) {
                    // The daemon's snapshot may be stale; rebuild
                    // locally so the operator still sees current state.
                    config.refresh = true;
                    warning = Some(message);
                }
                match discover_and_resolve_sourced(&config) {
                    Ok((snapshot, from_daemon)) => (Ok(snapshot), from_daemon),
                    Err(err) => (Err(err), false),
                }
            },
        );
        let _ = tx.send(DiscoveryResult {
            generation,
            snapshot,
            warning,
            from_daemon,
        });
    });
}

/// What a refresh does before it asks anyone for a snapshot. The
/// refresh exists to see what the action just did, so a mux listing
/// cached a few seconds ago is not good enough, here or in a daemonless
/// rebuild.
pub(in crate::tui) fn prepare_rescan(config: &RunConfig, nudge: DaemonNudge) {
    if nudge.rescans_mux() {
        config.discovery_caches.clear_mux();
    }
}

/// Run `probe` and, while its snapshot's live mux set still equals
/// `before` (nothing changed across a hand-off, which is almost never
/// true), wait `delay` and probe again, at most `tries` more times.
/// Returns the last probe's result.
fn probe_until_changed<T>(
    before: Option<&std::collections::BTreeSet<crate::model::NodeId>>,
    tries: u32,
    delay: Duration,
    mut sleep: impl FnMut(Duration),
    mut probe: impl FnMut() -> (Result<crate::model::GraphSnapshot>, T),
) -> (Result<crate::model::GraphSnapshot>, T) {
    let mut retried = 0;
    loop {
        let (snapshot, extra) = probe();
        let unchanged = match (before, &snapshot) {
            (Some(before), Ok(snapshot)) => live_mux_ids(snapshot) == *before,
            _ => false,
        };
        if unchanged && retried < tries {
            retried += 1;
            sleep(delay);
            continue;
        }
        return (snapshot, extra);
    }
}

/// The muxes `snapshot` shows as running.
fn live_mux_ids(
    snapshot: &crate::model::GraphSnapshot,
) -> std::collections::BTreeSet<crate::model::NodeId> {
    snapshot
        .live_nodes()
        .filter_map(|node| match node {
            crate::model::GraphNode::MuxSession(mux) => {
                Some(crate::model::NodeId::MuxSession(mux.id.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The forge fetch policy for a refresh worker. Ctrl-R is the operator
/// asking for the forges now, so its local rebuild fetches; every other
/// refresh keeps the live TUI's cached-only policy (ADR 0136).
fn worker_forge_fetch(
    nudge: DaemonNudge,
    base: crate::discovery::forge::ForgeFetch,
) -> crate::discovery::forge::ForgeFetch {
    match nudge {
        DaemonNudge::LiveAndForge => crate::discovery::forge::ForgeFetch::Fetch,
        DaemonNudge::None | DaemonNudge::LiveClasses => base,
    }
}

/// Fetch the forge listings `snapshot`'s repos need into `config`'s
/// caches on a background thread, and report how many remotes it asked
/// (ADR 0136).
fn spawn_forge_fetch(
    config: &RunConfig,
    tx: &mpsc::Sender<usize>,
    snapshot: crate::model::GraphSnapshot,
) {
    let discovery_config = crate::discovery::LocalDiscoveryConfig::from_env()
        .with_caches(std::sync::Arc::clone(&config.discovery_caches))
        .with_forge_concurrency(config.forge_concurrency);
    let intervals = config.intervals.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let fetched = crate::discovery::prefetch_forges(&snapshot, discovery_config, &intervals);
        let _ = tx.send(fetched);
    });
}

/// How many times a refresh after a mux hand-off re-probes when the
/// mux set came back exactly as it was before the hand-off, and how
/// long it waits between tries. A session that ended, or a server that
/// was killed, can still answer a listing for a moment; both probes
/// rescan in well under a second.
const SETTLE_TRIES: u32 = 2;
const SETTLE_DELAY: Duration = Duration::from_millis(150);

/// Ask a running `conspectus serve` daemon to rescan
/// the live classes now. The daemon otherwise serves its last tick,
/// which after a tmux hand-off still shows the world from before it.
/// No daemon is not an error (the caller's refresh runs discovery
/// itself); a daemon that fails the refresh is.
pub(super) fn nudge_daemon_live_classes() -> Result<(), String> {
    nudge_daemon(crate::tui::effect::RefreshRequest::LIVE.classes)
}

/// Ask a running daemon to rescan `classes` now, as
/// [`nudge_daemon_live_classes`] does for the live ones.
fn nudge_daemon(classes: &[&str]) -> Result<(), String> {
    use crate::server::{ClientOutcome, client_refresh};
    for &class in classes {
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

/// What a nudged reload produced, ready to apply on the UI thread.
struct HandoffRefresh {
    snapshot: Result<crate::model::GraphSnapshot>,
    /// Why the daemon nudge failed; the snapshot is then a local
    /// rebuild.
    warning: Option<String>,
}

/// Nudge the daemon, then load a snapshot. Touches no UI state, so it
/// can run on a worker thread.
fn fetch_after_mux_handoff(mut config: RunConfig) -> HandoffRefresh {
    prepare_rescan(&config, DaemonNudge::LiveClasses);
    let warning = nudge_daemon_live_classes().err();
    if warning.is_some() {
        config.refresh = true;
    }
    HandoffRefresh {
        snapshot: discover_and_resolve(&config),
        warning,
    }
}

fn apply_handoff_refresh(app: &mut App, fetched: HandoffRefresh) {
    let config = app.config().clone();
    apply_discovery(app, &config, fetched.snapshot);
    if let Some(message) = fetched.warning {
        app.report(crate::tui::messages::LogEntry::warning(message));
    }
}

/// Pause between redraws while [`run_with_spinner`] waits on its
/// worker; well under one spinner frame.
const SPINNER_REDRAW: Duration = Duration::from_millis(50);

/// Run `work` on a worker thread while `draw` keeps repainting `app`
/// with a status-bar spinner chip labelled `label`. Used for the
/// launch steps that run before an attach, where the UI thread would
/// otherwise sit on a frozen or blank screen (ADR 0127).
pub(super) fn run_with_spinner<T: Send + 'static>(
    app: &mut App,
    label: String,
    mut draw: impl FnMut(&mut App),
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Launch,
        label,
    });
    let worker = std::thread::spawn(work);
    loop {
        draw(app);
        if worker.is_finished() {
            break;
        }
        std::thread::sleep(SPINNER_REDRAW);
    }
    app.update(Msg::InFlightFinish(crate::tui::app::InFlightKind::Launch));
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// [`run_with_spinner`] drawing to the live terminal.
pub(super) fn run_with_terminal_spinner<T: Send + 'static>(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    label: String,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    run_with_spinner(
        app,
        label,
        |app| {
            let _ = draw_frame(app, terminal);
        },
        work,
    )
}

/// The refresh a launch runs when it ends without an attach: the
/// same nudge and reload as [`refresh_after_mux_handoff`], behind a
/// spinner. A launch that attaches refreshes in the background when
/// the attach returns instead (ADR 0133).
pub(super) fn refresh_after_launch_with_spinner(terminal: &mut DefaultTerminal, app: &mut App) {
    let config = app.config().clone();
    let fetched = run_with_terminal_spinner(terminal, app, "Refreshing".to_string(), move || {
        fetch_after_mux_handoff(config)
    });
    apply_handoff_refresh(app, fetched);
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
    discover_and_resolve_sourced(config).map(|(snapshot, _)| snapshot)
}

/// [`discover_and_resolve`], also reporting whether the snapshot came
/// from a running daemon.
fn discover_and_resolve_sourced(config: &RunConfig) -> Result<(crate::model::GraphSnapshot, bool)> {
    if !config.refresh
        && let Some(snapshot) = try_daemon_snapshot()
    {
        return Ok((snapshot, true));
    }
    let roots = config.scan_roots.effective();
    // The on-disk warm-start prior was the previous
    // graph.sqlite. With graph.sqlite retired, the daemonless
    // cold-rebuild path runs every provider from scratch each
    // tick — same as the daemon does on first cycle. Discovery
    // is single-digit seconds at target scale (ADR 0082), and
    // the TUI's typical setup runs `conspectus serve` so the
    // daemon-snapshot short-circuit above is the common path.
    let discovery_config = crate::discovery::LocalDiscoveryConfig::from_env()
        .with_caches(std::sync::Arc::clone(&config.discovery_caches))
        .with_forge_fetch(config.forge_fetch)
        .with_forge_concurrency(config.forge_concurrency);
    let fresh_since = crate::discovery::current_epoch();
    let snapshot = crate::discovery::discover_local_warm_with(
        roots,
        discovery_config,
        crate::model::GraphSnapshot::empty(),
        &config.intervals,
    )?;
    let mut snapshot = resolve_snapshot(snapshot);
    if config.record_history
        && let Some(writer) = crate::history::HistoryWriter::from_env(config.history.clone())
    {
        // Like the `graph.bin` write below, a failure stays quiet in a
        // live session; the next one-shot command reports it.
        let _ = writer.record(&mut snapshot, fresh_since);
    }
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
    Ok((snapshot, false))
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
    let Some(node) = crate::tui::actions::action_subject(app).node else {
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
    let Some(node) = crate::tui::actions::action_subject(app).node else {
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

/// Whether a worktree mutation backend is available (config selection +
/// `wt` on PATH). Governs whether the menu offers mutating actions.
fn worktree_mutation_available() -> bool {
    let selection = crate::config::load_from_cwd().config.worktree.backend;
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
    let candidates = known_cwd_candidates(app, Some(&seeded_cwd));
    app.open_new_mux_form(
        NewMuxFormState::new(seeded_name, seeded_cwd).with_cwd_candidates(candidates),
    );
    app.update(Msg::SetStatus(Some(
        "mux: ↑/↓ move · Tab complete/next · Enter next/commit · Esc cancel".to_string(),
    )));
}

/// Public alias for [`derive_new_mux_cwd`] so the reducer's
/// `Msg::OpenNewMuxForm` / `Msg::OpenMuxLaunchForm` arms can seed
/// the cwd from the selected row (ADR 0096).
pub(crate) fn derive_mux_form_cwd(app: &App) -> Option<String> {
    derive_new_mux_cwd(app)
}

/// Directories the cwd fields offer to complete: the selected row's,
/// then those of live agent sessions, muxes, processes, checkouts, repos,
/// and workspaces in the graph, and the cwds of pins.
pub(crate) fn known_cwd_candidates(
    app: &App,
    selected: Option<&str>,
) -> Vec<crate::tui::widgets::path_omnibox::PathCandidate> {
    use crate::model::GraphNode;
    use crate::tui::widgets::path_omnibox::PathCandidate;
    let mut candidates = Vec::new();
    let mut push = |path: &str, source: &str, rank: i32| {
        let path = path.trim();
        if !path.is_empty() {
            candidates.push(PathCandidate::new(path, source, rank));
        }
    };
    if let Some(selected) = selected {
        push(selected, "selected", 1_000);
    }
    if let Some(handle) = app.snapshot_handle() {
        for node in handle.snapshot().live_nodes() {
            match node {
                GraphNode::AgentSession(node) => {
                    if let Some(cwd) = node.cwd.as_deref() {
                        push(cwd, "agent", 450);
                    }
                }
                GraphNode::MuxSession(node) => {
                    if let Some(cwd) = node.cwd.as_deref() {
                        push(cwd, "mux", 440);
                    }
                    if let Some(cwd) = node.active_pane_current_path.as_deref() {
                        push(cwd, "mux", 430);
                    }
                }
                GraphNode::RuntimeProcess(node) => {
                    if let Some(cwd) = node.cwd.as_deref() {
                        push(cwd, "process", 400);
                    }
                }
                GraphNode::Checkout(node) => push(&node.root, "checkout", 350),
                GraphNode::Repo(node) => {
                    let root = node
                        .common_dir
                        .strip_suffix("/.git")
                        .unwrap_or(&node.common_dir);
                    push(root, "repo", 320);
                }
                GraphNode::Workspace(node) => push(&node.root, "workspace", 300),
                GraphNode::Pin(node) => {
                    if let Some(cwd) = node.text("cwd") {
                        push(cwd, "pin", 500);
                    }
                }
                GraphNode::Branch(_) | GraphNode::Fork(_) | GraphNode::ForgePr(_) => {}
            }
        }
    }
    candidates
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
        .live_nodes()
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
    // What the operator is looking at decides where the new mux
    // starts: the entity in the right pane while it has focus.
    if let Some(node) = crate::tui::actions::action_subject(app).node
        && let Some(cwd) = cwd_for_node(app.snapshot_handle()?.snapshot(), &node)
    {
        return Some(cwd);
    }
    let selection = app.selection()?.clone();
    let rows = app.visible_rows();
    let row = rows.iter().find(|row| row.id == selection)?;
    let snapshot = app.snapshot_handle()?.snapshot();
    match &row.kind {
        RowKind::Group(group) => group
            .primary_node
            .as_ref()
            .and_then(|node| cwd_for_node(snapshot, node)),
        RowKind::MuxSession(mux) => snapshot.live_nodes().find_map(|n| match n {
            GraphNode::MuxSession(m) if m.id == mux.mux => {
                m.active_pane_current_path.clone().or_else(|| m.cwd.clone())
            }
            _ => None,
        }),
        RowKind::AgentSession(session) => snapshot.live_nodes().find_map(|n| match n {
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
    // A gone node's directory may no longer exist, so it can't seed a
    // new mux's working directory.
    snapshot.live_nodes().find_map(|n| match (node, n) {
        (NodeId::Checkout(id), GraphNode::Checkout(c)) if &c.id == id => Some(c.root.clone()),
        // The path the repo's header shows, so a mux opened from it
        // starts there rather than in whichever checkout was probed first.
        (NodeId::Repo(id), GraphNode::Repo(r)) if &r.id == id => {
            Some(crate::tui::rows::repo_display_path(id, Some(r)))
        }
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

/// Decide whether the selection's preview needs loading (ADR 0112 §5):
/// it does when nothing is cached for its source, when the selection
/// just moved onto it, when its content went stale, or on `r`. Returns
/// `None` for a transcript or pane under `--no-live-preview`, which
/// hides them, or while a load for the source is in flight. Pure: reads
/// only [`App`] and [`RunConfig`] state.
fn plan_preview_load(
    app: &App,
    config: &RunConfig,
    shown: Option<&PreviewSource>,
    force: bool,
) -> Option<PreviewSource> {
    let source = preview_source(app)?;
    if source.is_live() && !config.live_preview_enabled {
        return None;
    }
    if app.previews().is_loading(&source) {
        return None;
    }
    let entry = app.previews().get(&source);
    let fresh = entry.is_some_and(|entry| !entry.stale);
    if source.is_forge() {
        // A forge read is a network call: reuse a loaded PR across
        // selections and `r` until it ages past the forge interval or
        // a hard refresh marks it stale (ADR 0116). A failed read
        // retries the next time the PR is selected.
        let reusable = fresh
            && entry.is_some_and(|entry| {
                matches!(entry.content, PreviewContent::Lines(_))
                    && entry
                        .captured_at
                        .is_some_and(|at| at.elapsed() < config.intervals.forge)
            });
        let reload = !reusable && (!fresh || force || shown != Some(&source));
        return reload.then_some(source);
    }
    if fresh && !force && shown == Some(&source) {
        return None;
    }
    Some(source)
}

/// Load the selection's preview when [`plan_preview_load`] says it
/// needs one. Returns the source the selection now shows, for the next
/// pass to compare against.
fn refresh_preview_if_needed(
    app: &mut App,
    config: &RunConfig,
    mode: &mut impl LoopMode,
    shown: Option<PreviewSource>,
    force: bool,
) -> Option<PreviewSource> {
    if let Some(source) = plan_preview_load(app, config, shown.as_ref(), force) {
        app.update(Msg::PreviewLoadStarted(source.clone()));
        mode.load_preview(app, source);
    }
    preview_source(app)
}

/// A preview a materialized scenario recorded in place of a live
/// source: a PR's `gh pr view` output. `None` for anything else.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
fn recorded_preview(
    source: &PreviewSource,
    root: &std::path::Path,
    muted: ratatui::style::Modifier,
) -> Option<PreviewContent> {
    let PreviewSource::PullRequest(pr) = source else {
        return None;
    };
    crate::dev_scenarios::recorded_pr_view(root, &pr.owner, &pr.repo, pr.number)
        .map(|json| crate::tui::preview::forge::pr_preview_from_json(&json, muted))
}

/// Load one preview on the calling thread. A pane capture goes through
/// the executor's mux op so the reducer stays the only writer.
fn load_preview_now(app: &mut App, tmux: &dyn MuxBackend, source: PreviewSource) {
    match source {
        PreviewSource::Pane(mux) => match pane_native_id(app, &mux) {
            Some(native_id) => execute_mux_op(
                app,
                tmux,
                crate::tui::effect::MuxOp::CapturePreview { mux, native_id },
            ),
            None => {
                app.update(Msg::SetPreview {
                    source: PreviewSource::Pane(mux),
                    content: PreviewContent::NoTarget,
                });
            }
        },
        source => {
            if let Some(content) = load_offline_preview(&source, app.theme().muted) {
                app.update(Msg::SetPreview { source, content });
            }
        }
    }
}

/// Load the selection's preview on the calling thread, for snapshot
/// mode, which renders one frame with no event loop to wait on. Pane
/// captures stay unloaded there, as before. With `rebase`, paths under
/// its first directory are read from its second (ADR 0113); the
/// preview is still stored under the source the renderer resolves.
#[cfg(feature = "snapshot")]
pub(super) fn load_offline_preview_now(
    app: &mut App,
    config: &RunConfig,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
) {
    let Some(source) = plan_preview_load(app, config, None, true) else {
        return;
    };
    if let Some((_, root)) = rebase
        && let Some(content) = recorded_preview(&source, root, app.theme().muted)
    {
        app.update(Msg::SetPreview { source, content });
        return;
    }
    if let (PreviewSource::Pane(mux), Some((_, root))) = (&source, rebase) {
        // A materialized scenario records its panes instead of running
        // tmux.
        let capture = pane_native_id(app, mux)
            .and_then(|native_id| crate::dev_scenarios::recorded_pane(root, &native_id));
        if let Some(text) = capture {
            app.update(Msg::SetPreview {
                source,
                content: PreviewContent::text(text),
            });
        }
        return;
    }
    let read = match rebase {
        Some((from, to)) => source.rebased(from, to),
        None => source.clone(),
    };
    if let Some(content) = load_offline_preview(&read, app.theme().muted) {
        app.update(Msg::SetPreview { source, content });
    }
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
///
/// What happens around each effect comes from
/// [`crate::tui::effect::lifecycle`]: an effect that changes state the
/// views show ends with a refresh, unless a hand-off refresh is already
/// on its way (an attach, a launch, a rebuild of a known mux).
pub(super) fn execute_effects_live(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    effects: Vec<Effect>,
) {
    for effect in effects {
        let policy = crate::tui::effect::lifecycle(&effect);
        match effect {
            Effect::Exec(crate::tui::effect::ExecSpec::PaneScrollback { pane, line, text }) => {
                open_pane_scrollback(app, tmux, &pane, line, &text);
            }
            Effect::Exec(spec) => execute_exec_spec(terminal, app, config, spec, policy),
            Effect::RunMux(op) => execute_mux_op(app, tmux, op),
            Effect::WriteStore(op) => execute_store_op(app, tmux, op),
            other => execute_pure_effect(app, other),
        }
        if wants_refresh_request(&policy, app) {
            app.request_refresh();
        }
    }
}

/// Whether an effect with `policy` still needs a refresh asked for: it
/// changes what the views show, and no hand-off refresh (an attach, or
/// a rebuild of a known mux) is already on its way.
pub(in crate::tui) fn wants_refresh_request(
    policy: &crate::tui::effect::Lifecycle,
    app: &App,
) -> bool {
    policy.refreshes && !app.handoff_refresh_pending()
}

/// Which executor runs the effects a key-bound action emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tui) enum ExecutorKind {
    /// Handles every effect.
    Live,
    /// Handles only the effects that need no terminal; anything else is
    /// reported as unavailable.
    Pure,
}

/// The executor the live loop runs `action`'s message through.
pub(in crate::tui) fn live_executor_for(action: &Action) -> ExecutorKind {
    match action {
        Action::Msg(_) => ExecutorKind::Live,
        _ => ExecutorKind::Pure,
    }
}

/// Whether the pure executor can run `effect`; the rest need a live
/// terminal, mux backend, or store.
pub(in crate::tui) fn pure_executor_runs(effect: &Effect) -> bool {
    match effect {
        Effect::Exec(_) | Effect::RunMux(_) | Effect::WriteStore(_) => false,
        Effect::SpawnRefresh { .. }
        | Effect::Toast(_)
        | Effect::Copy { .. }
        | Effect::Persist
        | Effect::Quit => true,
    }
}

/// Run any content search scan the app asked for to completion on this
/// thread. Snapshot mode calls it between keys so a search is finished
/// before the next one.
#[cfg(feature = "snapshot")]
pub(super) fn run_pending_content_scans(
    app: &mut App,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
) {
    // A materialized scenario answers pane searches from the captures it
    // recorded instead of running tmux.
    let panes: Option<std::sync::Arc<dyn MuxBackend>> = rebase.map(|(_, root)| {
        std::sync::Arc::new(crate::dev_scenarios::recorded_pane_backend(root))
            as std::sync::Arc<dyn MuxBackend>
    });
    content::run_blocking_with(app, rebase, panes.as_ref());
}

/// Open the pane viewer on a pane of the scenario world rooted at `root`,
/// reading the scrollback it recorded instead of running tmux, as snapshot
/// mode does for a pane search hit.
#[cfg(feature = "snapshot")]
pub(super) fn open_recorded_pane(
    app: &mut App,
    root: &std::path::Path,
    pane: &crate::content_search::PaneRef,
    line: usize,
    text: &str,
) {
    let panes = crate::dev_scenarios::recorded_pane_backend(root);
    open_pane_scrollback(app, &panes, pane, line, text);
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

/// [`dispatch_live`] as the `send` callback an overlay's key handler
/// takes for what the overlay commits.
fn live_send<'a>(
    terminal: &'a mut DefaultTerminal,
    config: &'a RunConfig,
    tmux: &'a dyn MuxBackend,
) -> impl FnMut(&mut App, Msg) + 'a {
    move |app, msg| dispatch_live(terminal, app, config, tmux, msg)
}

/// The configuration a refresh after a state-changing action runs
/// with: the one `r` uses. After the live classes are nudged it takes
/// the daemon's snapshot, which still has every PR it fetched; forcing
/// a local rebuild instead would read only this process's forge cache
/// and empty the PR list.
pub(in crate::tui) fn store_write_refresh_config(app: &App) -> RunConfig {
    let mut config = app.config().clone();
    config.refresh = false;
    config
}

fn refresh_with_config(app: &mut App, config: &RunConfig) {
    apply_discovery(app, config, discover_and_resolve(config));
}

fn apply_discovery(
    app: &mut App,
    config: &RunConfig,
    discovered: Result<crate::model::GraphSnapshot>,
) {
    match discovered {
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
    populate_provider_status(app, config);
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
    // Forge discovery reports why it read nothing (ADR 0115); the
    // header's forge chip shows the reason.
    if let Some(handle) = app.snapshot_handle() {
        let reason = forge_unavailable_reason(handle.snapshot());
        status.forge_available = Some(reason.is_none());
        status.forge_reason = reason;
    }
    app.update(Msg::SetProviderStatus(status));
}

/// Why forge discovery read nothing, from the snapshot's diagnostics:
/// the first `github` provider-unavailable reason.
fn forge_unavailable_reason(snapshot: &crate::model::GraphSnapshot) -> Option<String> {
    snapshot
        .diagnostics
        .iter()
        .find_map(|diagnostic| match diagnostic {
            crate::model::Diagnostic::ProviderUnavailable { provider, reason }
                if provider == crate::discovery::providers::GITHUB =>
            {
                Some(reason.clone())
            }
            _ => None,
        })
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
) {
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
    populate_provider_status(app, config);
}

pub(super) use crate::tui::keymap::{Action, SelectedDefault, selected_default_action};

pub(super) use crate::tui::keymap::cycle_view;

/// Dispatch `Enter` on the left pane to the selected
/// row's default action. Group rows expand/collapse; mux rows and
/// muxed agent sessions attach; un-muxed agent sessions open the
/// transcript viewer. Right-pane focus is handled in
/// [`remap_for_focus`] before this is reached.
/// Actions that read the selected row's live mux or session and then
/// change something (rename, worktree, pin edits). The attach, resume,
/// and launch actions are guarded in the reducer, so `Enter` can still
/// fold a row that awaits a refresh. Read-only actions stay available.
fn acts_on_selected_row(action: &Action) -> bool {
    matches!(
        action,
        Action::OpenRename | Action::OpenWorktreeMenu | Action::OpenWorktreeCloseDown
    )
}

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
        SelectedDefault::Scrollback => {
            dispatch_live(terminal, app, config, tmux, Msg::OpenSelectedPane);
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
