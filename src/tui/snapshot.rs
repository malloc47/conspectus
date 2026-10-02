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
use crate::tui::app::App;
use crate::tui::runtime::{
    self, Action, cycle_view, dispatch, handle_controls_overlay_key, handle_help_overlay_key,
    handle_messages_overlay_key, handle_search_overlay_key, refresh, refresh_from_snapshot,
};
use crate::tui::{RunConfig, ui};

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
}

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
        refresh_from_snapshot(&mut app, &config, snapshot)?;
    } else if let Some(export_path) = snap.export_fixture.as_deref() {
        let snapshot = runtime::discover_and_resolve(&config)?;
        write_fixture(export_path, &snapshot)?;
        refresh_from_snapshot(&mut app, &config, snapshot)?;
    } else {
        refresh(&mut app, &config);
    }

    let keys = parse_key_script(&snap.keys)?;
    drive_keys(&mut app, &config, keys, snap.height.saturating_sub(2));

    let backend = TestBackend::new(snap.width, snap.height);
    let mut terminal = Terminal::new(backend)?;
    // Mirror the interactive runtime's pre-draw setup so the toast
    // engine knows where to render. Without this, set_area stays
    // Rect::default() and any toast queued by `--snapshot-keys`
    // never paints.
    app.prepare_toast_for_render(Rect::new(0, 0, snap.width, snap.height));
    terminal.draw(|frame| ui::draw(&mut app, frame))?;
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
fn drive_keys(app: &mut App, config: &RunConfig, keys: Vec<KeyEvent>, viewport_height: u16) {
    for key in keys {
        if let Some(action) = runtime::action_for_event(app, Event::Key(key), viewport_height) {
            apply_action(app, config, action);
        }
    }
}

/// Apply an action against the snapshot-safe action subset. Actions
/// that would spawn a process, take over the terminal, or otherwise
/// reach outside the snapshot are skipped with a stderr note so the
/// agent knows the key was a no-op rather than silently dropped.
fn apply_action(app: &mut App, config: &RunConfig, action: Action) {
    let _ = config;
    match action {
        Action::Msg(msg) => dispatch(app, *msg),
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
            dispatch(app, Msg::SetFilter(crate::filter::RowFilter::default()));
        }
        Action::OpenControls => app.open_controls_overlay(),
        Action::OpenPins => app.open_pins_overlay(),
        Action::OpenSearch => app.open_search_overlay(),
        Action::OpenHelp => app.open_help_overlay(),
        Action::OpenMessages => app.open_messages_overlay(),
        Action::MessagesOverlayKey(key) => handle_messages_overlay_key(app, key),
        Action::OpenWorktreeMenu => runtime::open_worktree_menu_action(app),
        Action::OpenWorktreeCloseDown => runtime::open_worktree_close_down_action(app),
        Action::ControlsOverlayKey(key) => handle_controls_overlay_key(app, config, key),
        Action::PinsOverlayKey(key) => handle_pins_overlay_key(app, key),
        Action::SearchOverlayKey(key) => handle_search_overlay_key(app, key),
        Action::HelpOverlayKey(key) => handle_help_overlay_key(app, key),
        Action::Refresh => runtime::refresh(app, config),
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

fn handle_pins_overlay_key(app: &mut App, key: KeyEvent) {
    use crate::tui::widgets::pins::PinsOutcome;
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle_key(&ctx, key),
        None => return,
    };
    match outcome {
        PinsOutcome::Continue => {}
        PinsOutcome::Close | PinsOutcome::ApplyAndClose(_) => {
            app.close_pins_overlay();
        }
        PinsOutcome::ApplyAndStay(_) => {
            app.update(crate::tui::app::Msg::SetStatus(Some(
                "snapshot mode skipped mutating pin action".to_string(),
            )));
        }
    }
}

fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Attach => "Attach",
        Action::Resume => "Resume",
        Action::View => "View",
        Action::DefaultAction => "DefaultAction",
        Action::OpenRename => "OpenRename",
        Action::RenameOverlayKey(_) => "RenameOverlayKey",
        Action::RemovePin => "RemovePin",
        Action::PinBindHint => "PinBindHint",
        Action::OpenPinCreate => "OpenPinCreate",
        Action::OpenPinRebind => "OpenPinRebind",
        Action::OpenPinAdopt => "OpenPinAdopt",
        Action::LaunchPin => "LaunchPin",
        Action::PinsOverlayKey(_) => "PinsOverlayKey",
        Action::OpenValueModal => "OpenValueModal",
        Action::ValueModalKey(_) => "ValueModalKey",
        Action::ViewerOverlayKey(_) => "ViewerOverlayKey",
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
