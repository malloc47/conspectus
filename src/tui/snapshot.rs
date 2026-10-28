//! Dev-only one-shot TUI snapshot mode (ADR 0067).
//!
//! Renders one frame of the TUI into a ratatui `TestBackend`, emits
//! the cell grid as ANSI-styled text to stdout, and exits. Used by
//! agents iterating on renderer changes so they can "see" the TUI
//! without an interactive screenshot loop.
//!
//! Pipeline:
//!
//! 1. Build an `App` exactly like the production `tui` runtime does
//!    (`App::new(config)`), then call [`crate::tui::runtime::refresh`]
//!    to populate the row tree via live discovery.
//! 2. Parse the optional `--snapshot-keys` script into a sequence of
//!    `KeyEvent`s and dispatch each one through the same `translate`
//!    function the interactive event loop uses, so key behavior
//!    matches the live UI.
//! 3. Render one frame via `TestBackend` and walk the resulting
//!    `Buffer`, emitting minimal ANSI escape sequences as styles
//!    change cell-to-cell.
//! 4. Optionally slice the buffer to a single pane
//!    ([`SnapshotPane`]) using the same layout split the renderer
//!    uses, so agents can target the left tree, the detail pane,
//!    the header bar, or the status bar without grepping.
//!
//! The dispatcher handles a deliberate subset of actions — anything
//! that would take over the terminal, spawn a process, or write to
//! disk is skipped with a one-line stderr warning. The agent's
//! keystroke is recorded as a no-op rather than silently dropped.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

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
    /// first, dated `now` when it is set.
    pub messages: Vec<String>,
}

/// The prefix fixtures write in place of the temp directory a world
/// was built in (`just regen-showcase-fixture`, `ReplayWorld`).
pub const FIXTURE_PATH_ROOT: &str = "/fixture";

/// Targets the `--snapshot-pane` flag can select. `All` returns the
/// full buffer; the other variants slice using the same layout the
/// renderer uses.
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
/// interactive `tui` runtime; the only difference is the
/// `TestBackend` and the optional key prelude.
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

    let keys = parse_key_script(&snap.keys)?;
    let rebase = snap
        .fixture_root
        .as_deref()
        .map(|root| (std::path::Path::new(FIXTURE_PATH_ROOT), root));
    // A `--contains` query scans before the first key.
    runtime::run_pending_content_scans(&mut app, rebase);
    drive_keys(
        &mut app,
        &config,
        keys,
        Rect::new(0, 0, snap.width, snap.height),
        rebase,
    )?;
    // One frame, no event loop: read the selection's preview now
    // rather than showing it loading.
    let fixture_root = snap
        .fixture_root
        .as_deref()
        .map(|root| (std::path::Path::new(FIXTURE_PATH_ROOT), root));
    runtime::load_offline_preview_now(&mut app, &config, fixture_root);

    let backend = TestBackend::new(snap.width, snap.height);
    let mut terminal = Terminal::new(backend)?;
    // Mirror the interactive runtime's pre-draw setup so the toast
    // engine knows where to render. Without this, set_area stays
    // Rect::default() and any toast queued by `--snapshot-keys`
    // never paints.
    app.prepare_toast_for_render(Rect::new(0, 0, snap.width, snap.height));
    terminal.draw(|frame| runtime::draw_screen(&mut app, frame))?;
    let buffer = terminal.backend().buffer();

    let area = Rect::new(0, 0, snap.width, snap.height);
    let region = pane_rect(area, snap.pane, app.config().narrow_layout_threshold);
    let out = buffer_to_ansi(buffer, region);

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(out.as_bytes())?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

/// Put `messages` in the Messages log as information, oldest first, so
/// the log can be drawn with entries. Reporting an entry also sets the
/// status line; that is cleared again so the frame shows what the keys
/// leave there.
fn seed_messages(app: &mut App, messages: &[String], now: Option<i64>) {
    for text in messages {
        let mut entry = crate::tui::messages::LogEntry::info(text.clone());
        if let Some(now) = now {
            entry.at_epoch = now;
        }
        app.update(Msg::Report(Box::new(entry)));
    }
    if !messages.is_empty() {
        app.update(Msg::SetStatus(None));
    }
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
) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height))?;
    for key in keys {
        app.prepare_toast_for_render(area);
        terminal.draw(|frame| runtime::draw_screen(app, frame))?;
        if let Some(action) = runtime::action_for_event(app, Event::Key(key)) {
            apply_action(app, config, action, rebase);
            // Overlay keys can start a search; finish it before the
            // next key so the frame is deterministic.
            app.search_overlay_tick(std::time::Instant::now(), true);
            runtime::run_pending_content_scans(app, rebase);
        }
    }
    Ok(())
}

/// Open the transcript viewer for the selected session, reading the
/// transcript from the scenario world when a fixture root rebases the
/// paths. A session with nothing to show leaves a note on stderr.
fn open_viewer(app: &mut App, rebase: Option<(&std::path::Path, &std::path::Path)>) {
    let effects = app.update(Msg::ViewSelected);
    run_effects_opening_viewers(app, effects, rebase);
}

/// Run effects the snapshot-safe way: a request to view a session opens
/// the viewer from the scenario world; everything else goes through the
/// pure executor.
fn run_effects_opening_viewers(
    app: &mut App,
    effects: Vec<crate::tui::effect::Effect>,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
) {
    use crate::tui::effect::{Effect, ExecSpec};
    for effect in effects {
        // OSC 52 would land in the frame output.
        if matches!(effect, Effect::Copy { .. }) {
            eprintln!("conspectus: snapshot mode skipped a copy");
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
        match crate::tui::viewer_bridge::build_viewer_state_at(&id, open.as_ref()) {
            Some(state) => app.open_viewer_modal(state),
            None => eprintln!("conspectus: snapshot mode found no transcript to view"),
        }
    }
}

/// Apply an action against the snapshot-safe action subset. Actions
/// that would spawn a process, take over the terminal, or otherwise
/// reach outside the snapshot are skipped with a stderr note so the
/// agent knows the key was a no-op rather than silently dropped.
fn apply_action(
    app: &mut App,
    config: &RunConfig,
    action: Action,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
) {
    match action {
        Action::Msg(msg) => dispatch(app, *msg),
        Action::ModalKey(key) => modal_key(app, key, rebase),
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
        Action::OpenControls => app.open_controls_overlay(),
        Action::OpenSearch => {
            app.open_search_entry();
        }
        Action::OpenContentSearch => {
            app.open_content_search_entry();
        }
        Action::OpenHelp => app.open_help_overlay(),
        Action::OpenMessages => app.open_messages_overlay(),
        Action::OpenWorktreeMenu => runtime::open_worktree_menu_action(app),
        Action::OpenWorktreeCloseDown => runtime::open_worktree_close_down_action(app),
        Action::OpenMuxMenu => runtime::open_mux_menu_action(app),
        Action::OpenValueModal => app.open_value_modal_for_cursor(),
        // The forms and the rename prompt open as in the live loop.
        Action::OpenNewMuxForm => runtime::open_new_mux_form_action(app),
        Action::OpenRename => runtime::open_rename_overlay(app),
        Action::View => open_viewer(app, rebase),
        Action::Refresh => runtime::refresh(app, config),
        Action::HardRefresh => {
            app.update(crate::tui::Msg::ForgeRefreshRequested);
            app.config().discovery_caches.clear_forge();
            runtime::refresh(app, config);
        }
        // The clipboard write is skipped: OSC 52 would land in the
        // frame output. The toast still renders.
        Action::ExplorerEnter => runtime::explorer_enter(app, |_| {}),
        other => {
            eprintln!(
                "conspectus: snapshot mode skipped unsupported action: {}",
                action_label(&other)
            );
        }
    }
}

/// A key for the modal on top, routed as in the live loop: every modal
/// moves, scrolls, edits, and closes the same way. What it sends goes
/// through [`snapshot_send`], which keeps the snapshot from reaching
/// outside itself.
fn modal_key(app: &mut App, key: KeyEvent, rebase: Option<(&std::path::Path, &std::path::Path)>) {
    let from = app.top_modal_name();
    runtime::modal_key(app, key, &mut |app, msg| {
        snapshot_send(app, from, msg, rebase);
    });
}

/// What the snapshot driver does with a message the modal `from` sent.
///
/// Help, Controls, Messages, the search overlay, and the transcript
/// viewer only move the TUI (a setting, a row, a search, a scroll), so
/// what they send runs, a request to view a session opening the viewer
/// from the scenario world and a copy skipped. Everything else (the
/// menus, prompts, forms, checklists, the picker, the value modal, the
/// pane viewer) acts when it picks or submits, so only what reaches
/// nothing outside the snapshot runs: a notice it shows while it stays
/// open, a copy (skipped), and a pick that only opens another modal. The
/// rest is skipped with a note.
fn snapshot_send(
    app: &mut App,
    from: Option<&str>,
    msg: Msg,
    rebase: Option<(&std::path::Path, &std::path::Path)>,
) {
    let moves_only = matches!(
        from,
        Some("Help" | "Controls" | "Messages" | "Search" | "Viewer")
    );
    let harmless = matches!(msg, Msg::Toast(_) | Msg::Copy { .. }) || only_opens_a_modal(&msg);
    if moves_only || harmless {
        let effects = app.update(msg);
        run_effects_opening_viewers(app, effects, rebase);
    } else {
        eprintln!("conspectus: snapshot mode skipped a pick or submit");
    }
}

/// Whether `msg` does nothing but open a modal: one of the forms, the
/// rename prompt, the picker, or the Clean up checklist.
fn only_opens_a_modal(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::OpenNewMuxForm
            | Msg::OpenMuxLaunchForm
            | Msg::OpenSlotEdit(_)
            | Msg::OpenSlotRename(_)
            | Msg::OpenSlotRetarget(_)
            | Msg::OpenCleanUp
    )
}

fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Attach => "Attach",
        Action::Resume => "Resume",
        Action::DefaultAction => "DefaultAction",
        Action::OpenValueModal => "OpenValueModal",
        _ => "<other>",
    }
}

/// Compute the rect for a snapshot pane using the same layout the
/// renderer applies in `ui::draw`. Kept in sync with the constraints
/// in `ui::draw` and `ui::draw_body`; the renderer's constants are
/// pulled in via `pub(super)` so the two never drift.
fn pane_rect(area: Rect, pane: SnapshotPane, narrow_layout_threshold: u16) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
    let header = layout[0];
    let body = layout[1];
    let status = layout[2];
    match pane {
        SnapshotPane::All => area,
        SnapshotPane::Header => header,
        SnapshotPane::Status => status,
        SnapshotPane::Left | SnapshotPane::Right => {
            let direction = if body.width < narrow_layout_threshold {
                Direction::Vertical
            } else {
                Direction::Horizontal
            };
            let split = Layout::default()
                .direction(direction)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(body);
            if matches!(pane, SnapshotPane::Left) {
                split[0]
            } else {
                split[1]
            }
        }
    }
}

/// Walk the buffer cells row by row and emit minimal ANSI escape
/// sequences as styles change. A reset is emitted at the end of
/// each row so background fills don't bleed into the next line in
/// the operator's terminal.
fn buffer_to_ansi(buffer: &Buffer, region: Rect) -> String {
    let mut out = String::new();
    let area = buffer.area();
    for y in region.y..region.y.saturating_add(region.height) {
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        let mut current_style: Option<Style> = None;
        for x in region.x..region.x.saturating_add(region.width) {
            if x >= area.x.saturating_add(area.width) {
                break;
            }
            let cell = &buffer[(x, y)];
            let style = cell.style();
            if current_style.as_ref() != Some(&style) {
                out.push_str(&ansi_style_seq(style));
                current_style = Some(style);
            }
            out.push_str(cell.symbol());
        }
        out.push_str("\x1b[0m");
        if y + 1 < region.y.saturating_add(region.height) {
            out.push('\n');
        }
    }
    out
}

fn ansi_style_seq(style: Style) -> String {
    let mut out = String::from("\x1b[0m");
    if let Some(fg) = style.fg
        && let Some(seq) = color_to_seq(fg, false)
    {
        out.push_str(&seq);
    }
    if let Some(bg) = style.bg
        && let Some(seq) = color_to_seq(bg, true)
    {
        out.push_str(&seq);
    }
    let mods = style.add_modifier;
    if mods.contains(Modifier::BOLD) {
        out.push_str("\x1b[1m");
    }
    if mods.contains(Modifier::DIM) {
        out.push_str("\x1b[2m");
    }
    if mods.contains(Modifier::ITALIC) {
        out.push_str("\x1b[3m");
    }
    if mods.contains(Modifier::UNDERLINED) {
        out.push_str("\x1b[4m");
    }
    if mods.contains(Modifier::REVERSED) {
        out.push_str("\x1b[7m");
    }
    out
}

fn color_to_seq(color: Color, bg: bool) -> Option<String> {
    let base = if bg { 48 } else { 38 };
    Some(match color {
        Color::Reset => return None,
        Color::Black => format!("\x1b[{}m", if bg { 40 } else { 30 }),
        Color::Red => format!("\x1b[{}m", if bg { 41 } else { 31 }),
        Color::Green => format!("\x1b[{}m", if bg { 42 } else { 32 }),
        Color::Yellow => format!("\x1b[{}m", if bg { 43 } else { 33 }),
        Color::Blue => format!("\x1b[{}m", if bg { 44 } else { 34 }),
        Color::Magenta => format!("\x1b[{}m", if bg { 45 } else { 35 }),
        Color::Cyan => format!("\x1b[{}m", if bg { 46 } else { 36 }),
        Color::Gray => format!("\x1b[{}m", if bg { 47 } else { 37 }),
        Color::DarkGray => format!("\x1b[{}m", if bg { 100 } else { 90 }),
        Color::LightRed => format!("\x1b[{}m", if bg { 101 } else { 91 }),
        Color::LightGreen => format!("\x1b[{}m", if bg { 102 } else { 92 }),
        Color::LightYellow => format!("\x1b[{}m", if bg { 103 } else { 93 }),
        Color::LightBlue => format!("\x1b[{}m", if bg { 104 } else { 94 }),
        Color::LightMagenta => format!("\x1b[{}m", if bg { 105 } else { 95 }),
        Color::LightCyan => format!("\x1b[{}m", if bg { 106 } else { 96 }),
        Color::White => format!("\x1b[{}m", if bg { 107 } else { 97 }),
        Color::Rgb(r, g, b) => format!("\x1b[{base};2;{r};{g};{b}m"),
        Color::Indexed(i) => format!("\x1b[{base};5;{i}m"),
    })
}

/// Parse a `--snapshot-keys` script into a list of `KeyEvent`s.
///
/// Literal characters pass through as-is (`v` → `KeyCode::Char('v')`).
/// Uppercase literals carry an implicit `SHIFT` modifier so the
/// translate layer sees them the same way a real keyboard does.
/// `<Name>` bracketed sequences map non-printables and modifier
/// combinations; supported names:
///
/// - Arrows: `<Up> <Down> <Left> <Right>`
/// - Enter / Esc / Tab / BackTab / Space / Backspace / Delete /
///   Insert / Home / End / PageUp / PageDown
/// - Function keys: `<F1>` .. `<F12>`
/// - Single literal in brackets (e.g. `<v>`)
/// - Modifier prefixes: `<C-x>` (Control), `<A-x>` (Alt), `<S-Tab>`
///   (Shift); modifiers nest, so `<C-A-Del>` is legal.
fn parse_key_script(raw: &str) -> Result<Vec<KeyEvent>> {
    let mut out = Vec::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut name = String::new();
            let mut closed = false;
            for nc in chars.by_ref() {
                if nc == '>' {
                    closed = true;
                    break;
                }
                name.push(nc);
            }
            if !closed {
                return Err(anyhow!("unterminated `<...>` in key script: missing `>`"));
            }
            out.push(parse_named_key(&name)?);
        } else {
            out.push(literal_to_key(ch));
        }
    }
    Ok(out)
}

fn literal_to_key(ch: char) -> KeyEvent {
    let mods = if ch.is_ascii_uppercase() {
        KeyModifiers::SHIFT
    } else {
        KeyModifiers::NONE
    };
    KeyEvent::new(KeyCode::Char(ch), mods)
}

fn parse_named_key(name: &str) -> Result<KeyEvent> {
    if let Some(rest) = name.strip_prefix("C-") {
        let inner = parse_named_key(rest)?;
        return Ok(KeyEvent::new(
            inner.code,
            inner.modifiers | KeyModifiers::CONTROL,
        ));
    }
    if let Some(rest) = name.strip_prefix("A-") {
        let inner = parse_named_key(rest)?;
        return Ok(KeyEvent::new(
            inner.code,
            inner.modifiers | KeyModifiers::ALT,
        ));
    }
    if let Some(rest) = name.strip_prefix("S-") {
        let inner = parse_named_key(rest)?;
        return Ok(KeyEvent::new(
            inner.code,
            inner.modifiers | KeyModifiers::SHIFT,
        ));
    }
    let code = match name {
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "BackTab" => KeyCode::BackTab,
        "Space" => KeyCode::Char(' '),
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        s if s.starts_with('F') && s.len() > 1 => {
            let n: u8 = s[1..]
                .parse()
                .map_err(|_| anyhow!("unknown function key `<{name}>`"))?;
            KeyCode::F(n)
        }
        s if s.chars().count() == 1 => {
            let ch = s.chars().next().unwrap();
            return Ok(literal_to_key(ch));
        }
        _ => return Err(anyhow!("unknown key name `<{name}>`")),
    };
    Ok(KeyEvent::new(code, KeyModifiers::NONE))
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
