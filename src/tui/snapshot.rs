//! Dev-only one-shot TUI snapshot mode (ADR 0067).
//!
//! Renders one frame of the TUI off screen, emits the cell grid as
//! ANSI-styled text to stdout, and exits. Used by agents iterating on
//! renderer changes so they can "see" the TUI without an interactive
//! screenshot loop.
//!
//! Pipeline:
//!
//! 1. Build an `App` exactly like the production `tui` runtime does
//!    (`App::new(config)`), then call `runtime::refresh`
//!    to populate the row tree via live discovery.
//! 2. Parse the optional `--snapshot-keys` script with the framework's
//!    key-script notation (`tuikit::testkit::keys`) and dispatch each key
//!    through the same `translate` function the interactive event loop
//!    uses, so key behavior matches the live UI. A key an open modal owns
//!    goes through the modal stack, and whatever the modal sends goes
//!    through the reducer, so every overlay key does what it does live.
//! 3. Render one frame with the framework's one-frame driver
//!    (`tuikit::testkit::render`) and write it as ANSI text
//!    (`tuikit::testkit::ansi`), styles kept.
//! 4. Optionally slice the buffer to a single pane
//!    ([`SnapshotPane`]) through the layout the renderer draws with,
//!    so agents can target the left tree, the detail pane, the header
//!    bar, or the status bar without grepping.
//!
//! The dispatcher handles a deliberate subset of actions, and the
//! effects the reducer emits run on a snapshot-safe executor: anything
//! that would take over the terminal, spawn a process, or write to disk
//! is skipped with a one-line stderr note naming it. The agent's
//! keystroke is recorded as a no-op rather than silently dropped.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::layout::Rect;
use tuikit::feedback::{Feedback, Severity};
use tuikit::testkit::render::FrameDriver;
use tuikit::testkit::{ansi, keys};

use crate::model::GraphSnapshot;
use crate::tui::Msg;
use crate::tui::RunConfig;
use crate::tui::app::App;
use crate::tui::runtime::{self, Action, cycle_view, dispatch, refresh, refresh_from_snapshot};

/// Tunables for one snapshot pass. Filled from `--snapshot-*` flags
/// on the `tui` subcommand.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub width: u16,
    pub height: u16,
    /// Vim-style key script: literal characters pass through;
    /// `<Name>` bracketed names map to non-printable keys; `<C-x>`
    /// / `<A-x>` add modifiers.
    pub keys: String,
    /// Which region of the rendered frame to emit.
    pub pane: SnapshotPane,
    /// Read the input `GraphSnapshot` from this JSON file instead
    /// of running live discovery (ADR 0068). Resolves the loaded
    /// snapshot through `resolve_snapshot` so hand-crafted fixtures
    /// missing `resolved_relationships` still render.
    pub fixture: Option<PathBuf>,
    /// After the input snapshot is produced (live discovery or
    /// fixture load) and resolved, serialize it to this JSON file
    /// (ADR 0068). Renders proceed normally afterward.
    pub export_fixture: Option<PathBuf>,
    /// Directory holding the files a fixture's [`FIXTURE_PATH_ROOT`]
    /// paths name, so previews read real transcripts, listings, and
    /// git state (ADR 0113).
    pub fixture_root: Option<PathBuf>,
    /// Unix time to render ages against instead of the clock.
    pub now: Option<i64>,
    /// Entries put in the Messages log before the keys run, oldest
    /// first, dated `now` when it is set: `[SEVERITY:]TEXT`, a success
    /// unless it names a severity.
    pub messages: Vec<String>,
    /// A toast posted after the keys run, so the frame shows it:
    /// `[SEVERITY:]TEXT`, a success unless it names a severity.
    pub toast: Option<String>,
    /// A status-line message set after the keys run: `[SEVERITY:]TEXT`,
    /// a note (info) unless it names a severity.
    pub status: Option<String>,
}

/// The prefix fixtures write in place of the temp directory a world
/// was built in (`just regen-showcase-fixture`, `ReplayWorld`).
pub const FIXTURE_PATH_ROOT: &str = "/fixture";

/// Targets the `--snapshot-pane` flag can select. `All` returns the
/// full buffer; the other variants slice it through the layout the
/// renderer draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPane {
    All,
    Header,
    Left,
    Right,
    Status,
}

/// Render one snapshot frame and write the ANSI-encoded result to
/// stdout. Mirrors the discovery + reducer pipeline of the
/// interactive `tui` runtime; the only difference is the off-screen
/// terminal and the optional key prelude.
pub fn run(config: RunConfig, snap: SnapshotConfig) -> Result<()> {
    if snap.width == 0 || snap.height == 0 {
        return Err(anyhow!(
            "snapshot dimensions must be positive (got {}x{})",
            snap.width,
            snap.height
        ));
    }

    if let Some(now) = snap.now {
        crate::tui::rows::freeze_clock(now);
    }
    let mut app = App::new(config.clone());

    // Resolve the input snapshot. Fixture path bypasses live
    // discovery (ADR 0068); without a fixture, we run the same
    // discover-and-resolve pipeline the production runtime uses,
    // returning the snapshot so `--snapshot-export-fixture` can
    // capture it.
    if let Some(fixture_path) = snap.fixture.as_deref() {
        let snapshot = load_fixture(fixture_path)?;
        if let Some(export_path) = snap.export_fixture.as_deref() {
            write_fixture(export_path, &snapshot)?;
        }
        refresh_from_snapshot(&mut app, &config, snapshot);
    } else if let Some(export_path) = snap.export_fixture.as_deref() {
        let snapshot = runtime::discover_and_resolve(&config)?;
        write_fixture(export_path, &snapshot)?;
        refresh_from_snapshot(&mut app, &config, snapshot);
    } else {
        refresh(&mut app, &config);
    }

    seed_messages(&mut app, &snap.messages, snap.now);

    let keys = keys::parse(&snap.keys).map_err(|error| anyhow!(error))?;
    let rebase = snap
        .fixture_root
        .as_deref()
        .map(|root| (std::path::Path::new(FIXTURE_PATH_ROOT), root));
    let mut notes = Notes::new();
    // A `--contains` query scans before the first key.
    runtime::run_pending_content_scans(&mut app, rebase);
    drive_keys(
        &mut app,
        &config,
        keys,
        Rect::new(0, 0, snap.width, snap.height),
        rebase,
        &mut notes,
    );
    for note in &notes {
        eprintln!("conspectus: {note}");
    }
    show_feedback(&mut app, &snap);
    // One frame, no event loop: read the selection's preview now
    // rather than showing it loading.
    runtime::load_offline_preview_now(&mut app, &config, rebase);

    // A fresh driver, so the frame holds only the cells this draw sent.
    let mut driver = FrameDriver::new(snap.width, snap.height);
    // Mirror the interactive runtime's pre-draw step, which retires a
    // toast whose time is up.
    app.prepare_toast_for_render();
    let buffer = driver.draw(|frame| runtime::draw_screen(&mut app, frame));

    let area = Rect::new(0, 0, snap.width, snap.height);
    let region = pane_rect(area, snap.pane, app.config().narrow_layout_threshold);
    let out = ansi::to_ansi(buffer, region);

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(out.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

/// Put `messages` in the Messages log, oldest first, so the log can be
/// drawn with entries. Each is `[SEVERITY:]TEXT`, a success when it
/// names no severity. Reporting an entry also sets the status line;
/// that is cleared again so the frame shows what the keys leave there.
fn seed_messages(app: &mut App, messages: &[String], now: Option<i64>) {
    use crate::tui::messages::{LogEntry, LogEntryExt};
    for value in messages {
        let feedback = parse_feedback(value, Severity::Success);
        let mut entry = LogEntry::of(feedback.severity, feedback.text);
        if let Some(now) = now {
            entry.at = now;
        }
        app.update(Msg::Report(Box::new(entry)));
    }
    if !messages.is_empty() {
        app.update(Msg::SetStatus(None));
    }
}

/// Post the toast and set the status line the flags ask for, after the
/// keys (which clear the status line as they move) have run. A toast
/// that names no severity is a success, a status message a note.
fn show_feedback(app: &mut App, snap: &SnapshotConfig) {
    if let Some(value) = &snap.status {
        app.update(Msg::SetStatus(Some(parse_feedback(value, Severity::Info))));
    }
    if let Some(value) = &snap.toast {
        app.post_toast(parse_feedback(value, Severity::Success));
    }
}

/// A `[SEVERITY:]TEXT` flag value: a leading `info:`, `success:`,
/// `warning:`, or `error:` names the severity; without one the text has
/// `default`.
fn parse_feedback(value: &str, default: Severity) -> Feedback {
    let named = [
        ("info:", Severity::Info),
        ("success:", Severity::Success),
        ("warning:", Severity::Warning),
        ("error:", Severity::Error),
    ];
    named
        .into_iter()
        .find_map(|(prefix, severity)| {
            value
                .strip_prefix(prefix)
                .map(|text| Feedback::new(severity, text))
        })
        .unwrap_or_else(|| Feedback::new(default, value))
}

/// Read and deserialize a `GraphSnapshot` from a JSON fixture.
/// Errors carry the path in the context so the agent can see which
/// file failed at a glance.
fn load_fixture(path: &std::path::Path) -> Result<GraphSnapshot> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read snapshot fixture `{}`", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("parse snapshot fixture `{}`", path.display()))
}

/// Serialize a `GraphSnapshot` to a JSON fixture file. Pretty-
/// printed for hand-editing and diff-friendliness, matching the
/// shape `output::render_graph_json` emits.
fn write_fixture(path: &std::path::Path, snapshot: &GraphSnapshot) -> Result<()> {
    let json = serde_json::to_string_pretty(snapshot)
        .with_context(|| format!("serialize snapshot fixture `{}`", path.display()))?;
    fs::write(path, json)
        .with_context(|| format!("write snapshot fixture `{}`", path.display()))?;
    Ok(())
}

/// What the snapshot driver skipped or could not do, one line each, for
/// stderr.
pub(in crate::tui) type Notes = Vec<String>;

/// Dispatch the key prelude one key at a time, resolving each through
/// the same overlay / keymap / focus pipeline as the interactive loop.
/// Like that loop, a frame of `area` is drawn before each key, so a key
/// that acts on what was drawn (a page key moving by the rows a pane or
/// modal showed) does the same here.
fn drive_keys(
    app: &mut App,
    config: &RunConfig,
    keys: Vec<KeyEvent>,
    area: Rect,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
    notes: &mut Notes,
) {
    let mut driver = FrameDriver::new(area.width, area.height);
    driver.drive(
        app,
        keys,
        |app, frame| {
            app.prepare_toast_for_render();
            runtime::draw_screen(app, frame);
        },
        |app, key| {
            if let Some(action) = runtime::action_for_event(app, Event::Key(key)) {
                apply_action(app, config, action, rebase, notes);
                // Overlay keys can start a search; finish it before the
                // next key so the frame is deterministic.
                app.search_overlay_tick(std::time::Instant::now(), true);
                runtime::run_pending_content_scans(app, rebase);
            }
        },
    );
}

/// Open the transcript viewer for the selected session, reading the
/// transcript from the scenario world when a fixture root rebases the
/// paths. A session with nothing to show leaves a note.
fn open_viewer(
    app: &mut App,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
    notes: &mut Notes,
) {
    let effects = app.update(Msg::ViewSelected);
    run_effects_opening_viewers(app, effects, rebase, notes);
}

/// Run effects the snapshot-safe way: a request to view a session opens
/// the viewer from the scenario world, and a request to read a pane's
/// scrollback opens the pane viewer on the capture the world recorded.
/// What would reach outside the snapshot ([`reaches_outside`]) is skipped
/// with a note naming it; everything else goes through the pure executor.
fn run_effects_opening_viewers(
    app: &mut App,
    effects: Vec<crate::tui::effect::Effect>,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
    notes: &mut Notes,
) {
    use crate::tui::effect::{Effect, ExecSpec};
    for effect in effects {
        if let Effect::Copy { done, .. } = effect {
            skip_copy(app, done, notes);
            continue;
        }
        if let Some(what) = reaches_outside(&effect) {
            notes.push(format!("snapshot mode skipped {what}"));
            continue;
        }
        if let Effect::Exec(ExecSpec::PaneScrollback { pane, line, text }) = &effect {
            match rebase {
                Some((_, root)) => runtime::open_recorded_pane(app, root, pane, *line, text),
                None => notes.push(
                    "snapshot mode reads pane scrollback only from a fixture root".to_string(),
                ),
            }
            continue;
        }
        let Effect::Exec(ExecSpec::ViewSession(mut id, open)) = effect else {
            runtime::execute_effects(app, vec![effect]);
            continue;
        };
        if let Some((from, to)) = rebase
            && let Ok(rest) = std::path::Path::new(&id.state_scope).strip_prefix(from)
        {
            id.state_scope = to.join(rest).display().to_string();
        }
        match crate::tui::viewer_bridge::build_viewer_at(&id, open.as_ref()) {
            Some(viewer) => app.open_viewer_modal(viewer),
            None => notes.push("snapshot mode found no transcript to view".to_string()),
        }
    }
}

/// `msg` through the reducer, its effects run on the pure executor as the
/// live loop runs a key's, except a copy, which [`skip_copy`] skips.
fn dispatch_msg(app: &mut App, msg: Msg, notes: &mut Notes) {
    for effect in app.update(msg) {
        match effect {
            crate::tui::effect::Effect::Copy { done, .. } => skip_copy(app, done, notes),
            other => runtime::execute_effects(app, vec![other]),
        }
    }
}

/// Skip a copy, whose OSC 52 escape would land in the frame, with a note,
/// and still show the toast that says what it copied, as the live
/// executor does after a write that went out.
fn skip_copy(app: &mut App, done: String, notes: &mut Notes) {
    notes.push("snapshot mode skipped a copy".to_string());
    app.post_toast(Feedback::success(done));
}

/// What `effect` would do outside the snapshot, if it would: a copy (its
/// OSC 52 escape would land in the frame), a hand-off to another program,
/// a mux command, or a store write. `None` for the effects the snapshot
/// runs itself (a viewer, a pane's recorded scrollback) or on the pure
/// executor (a toast, a refresh, the TUI's own state).
fn reaches_outside(effect: &crate::tui::effect::Effect) -> Option<&'static str> {
    use crate::tui::effect::{Effect, ExecSpec};
    match effect {
        Effect::Copy { .. } => Some("a copy"),
        Effect::Exec(ExecSpec::AttachMux(_)) => Some("an attach"),
        Effect::Exec(ExecSpec::Resume(_)) => Some("a resume"),
        Effect::Exec(ExecSpec::MuxNew { .. } | ExecSpec::MuxLaunch { .. }) => Some("a launch"),
        Effect::Exec(ExecSpec::Rebuild { .. } | ExecSpec::ShowRebuildPlan { .. }) => {
            Some("a rebuild")
        }
        Effect::RunMux(_) => Some("a mux command"),
        Effect::WriteStore(_) => Some("a store write"),
        Effect::Exec(ExecSpec::ViewSession(..) | ExecSpec::PaneScrollback { .. })
        | Effect::SpawnRefresh { .. }
        | Effect::Toast(_)
        | Effect::Persist
        | Effect::Quit => None,
    }
}

/// Apply an action against the snapshot-safe action subset. Actions
/// that would spawn a process, take over the terminal, or otherwise
/// reach outside the snapshot are skipped with a note so the agent
/// knows the key was a no-op rather than silently dropped.
fn apply_action(
    app: &mut App,
    config: &RunConfig,
    action: Action,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
    notes: &mut Notes,
) {
    match action {
        Action::Msg(msg) => dispatch_msg(app, *msg, notes),
        Action::ModalKey(key) => modal_key(app, key, rebase, notes),
        Action::SwitchView(view) => dispatch(app, Msg::SwitchView(view)),
        Action::CycleView(delta) => {
            let next = cycle_view(app.active_view(), delta);
            dispatch(app, Msg::SwitchView(next));
        }
        Action::CycleGrouping(delta) => {
            let next = if delta >= 0 {
                app.grouping().cycle_next()
            } else {
                app.grouping().cycle_prev()
            };
            dispatch(app, Msg::SetGrouping(next));
        }
        Action::ClearFilters => {
            dispatch(app, Msg::ClearFilters);
        }
        Action::OpenControls => runtime::open_controls(app),
        Action::OpenSearch => app.open_search_entry(),
        Action::OpenContentSearch => app.open_content_search_entry(),
        Action::OpenHelp => app.open_help_overlay(),
        Action::OpenMessages => app.open_messages_overlay(),
        Action::OpenWorktreeMenu => runtime::open_worktree_menu_action(app),
        Action::OpenWorktreeCloseDown => runtime::open_worktree_close_down_action(app),
        Action::OpenMuxMenu => runtime::open_mux_menu_action(app),
        // The forms and the rename prompt open as in the live loop.
        Action::OpenNewMuxForm => runtime::open_new_mux_form_action(app),
        Action::OpenRename => runtime::open_rename_overlay(app),
        Action::View => open_viewer(app, rebase, notes),
        Action::Refresh => runtime::refresh(app, config),
        Action::HardRefresh => {
            app.update(crate::tui::Msg::ForgeRefreshRequested);
            app.config().discovery_caches.clear_forge();
            runtime::refresh(app, config);
        }
        other => notes.push(format!(
            "snapshot mode skipped unsupported action: {}",
            action_label(&other)
        )),
    }
}

/// A key for the modal on top, routed as in the live loop: every modal
/// moves, scrolls, edits, picks, submits, and closes the same way. What
/// it sends goes through the reducer, as in every loop, and the effects
/// that follow through [`run_effects_opening_viewers`], which keeps the
/// snapshot from reaching outside itself.
fn modal_key(
    app: &mut App,
    key: KeyEvent,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
    notes: &mut Notes,
) {
    runtime::modal_key(app, key, &mut |app, msg| {
        let effects = app.update(msg);
        run_effects_opening_viewers(app, effects, rebase, notes);
    });
}

/// One key for the modal on top, as `--snapshot-keys` drives it with no
/// fixture world, for tests elsewhere in the TUI: what the snapshot
/// skipped, one note each.
#[cfg(test)]
pub(in crate::tui) fn press_modal_key(app: &mut App, key: KeyEvent) -> Notes {
    let mut notes = Notes::new();
    modal_key(app, key, None, &mut notes);
    notes
}

fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Attach => "Attach",
        Action::Resume => "Resume",
        Action::DefaultAction => "DefaultAction",
        _ => "<other>",
    }
}

/// The rect of a snapshot pane: the region of the screen the renderer
/// draws that part in, from the screen's one layout (`ui::screen_areas`),
/// so the snapshot cannot drift from the draw.
fn pane_rect(area: Rect, pane: SnapshotPane, narrow_layout_threshold: u16) -> Rect {
    let areas = crate::tui::ui::screen_areas(area, narrow_layout_threshold);
    match pane {
        SnapshotPane::All => area,
        SnapshotPane::Header => areas.header,
        SnapshotPane::Status => areas.status,
        SnapshotPane::Left => areas.left,
        SnapshotPane::Right => areas.right,
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
