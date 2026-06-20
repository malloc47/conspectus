//! `?` help overlay (F8-011).
//!
//! Minimal reference card for the current keymap. Render-only — no
//! mutation, no sub-editors — so the state struct is empty today and
//! exists mainly so the runtime can use the same open/close pattern
//! it uses for the controls, rename, and search overlays. As more
//! capabilities accumulate this can grow tabs or scrolling without
//! changing its boundary with the runtime.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::tui::Theme;
use crate::tui::icons::{NodeKind, node_kind_style};

/// Pure state for the help overlay. Carries vertical scroll
/// position so long keymaps stay reachable on short terminals.
#[derive(Debug, Clone, Default)]
pub struct HelpOverlayState {
    pub scroll: u16,
}

impl HelpOverlayState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dispatch a crossterm key event. Esc / Ctrl-C / `q` close the
    /// overlay; j/k/PgDn/PgUp/g/G scroll the keymap if it overflows
    /// the modal; everything else is swallowed so navigation keys
    /// don't accidentally affect the row tree behind it.
    pub fn handle_key(&mut self, event: KeyEvent) -> HelpOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return HelpOutcome::Close;
        }
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => HelpOutcome::Close,
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_by(1);
                HelpOutcome::Continue
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_by(-1);
                HelpOutcome::Continue
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_by(8);
                HelpOutcome::Continue
            }
            KeyCode::PageUp => {
                self.scroll_by(-8);
                HelpOutcome::Continue
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.scroll = 0;
                HelpOutcome::Continue
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.scroll = u16::MAX;
                HelpOutcome::Continue
            }
            _ => HelpOutcome::Continue,
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        let current = i32::from(self.scroll);
        let next = current.saturating_add(delta).max(0);
        self.scroll = u16::try_from(next.min(i32::from(u16::MAX))).unwrap_or(u16::MAX);
    }
}

/// What the host should do after passing a key event through the
/// help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOutcome {
    Continue,
    Close,
}

/// Centered modal that renders the keymap reference. Lays out two
/// columns of `key · action` pairs grouped into sections so the
/// operator can scan for the action they want. Supports vertical
/// scrolling when the keymap overflows the modal.
pub struct HelpOverlayWidget<'a> {
    state: &'a HelpOverlayState,
    theme: &'a Theme,
}

impl<'a> HelpOverlayWidget<'a> {
    pub fn new(state: &'a HelpOverlayState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for HelpOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: the centered-bordered-modal shell — clear,
        // border, title — is owned by `tui_popup::Popup` via
        // `crate::tui::widgets::popup_frame::themed_popup`. The body
        // wrapper reports the same cap dimensions
        // `centered_modal_rect` computed before, so the popup's
        // auto-sizing reproduces the in-tree rect.
        let modal = centered_modal_rect(area);
        let body = HelpBody {
            state: self.state,
            theme: self.theme,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let popup =
            crate::tui::widgets::popup_frame::themed_popup(body, line![" Help "], self.theme);
        popup.render(area, buf);
    }
}

/// Body wrapper for `tui_popup::Popup`. Reports the inner width /
/// height the in-tree `centered_modal_rect` cap produces so the
/// popup's auto-sizing reproduces the legacy rect. `Widget::render`
/// delegates to the same `Paragraph` the prior in-tree render built.
struct HelpBody<'a> {
    state: &'a HelpOverlayState,
    theme: &'a Theme,
    inner_width: usize,
    inner_height: usize,
}

impl KnownSize for HelpBody<'_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for HelpBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let para = Paragraph::new(body_lines(self.theme)).scroll((self.state.scroll, 0));
        para.render(area, buf);
    }
}

/// Build the static keymap. The first column is the key, the second
/// is a one-line description. Sections are bold; bindings are plain
/// text.
fn body_lines(theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let bind = |lines: &mut Vec<Line<'static>>, key: &str, desc: &str| {
        lines.push(line![
            span!(Style::default().fg(theme.panel_focus_accent); "  {key:<14}"),
            desc.to_string(),
        ]);
    };

    section(&mut lines, "Discoverable controls (ADR 0031)");
    bind(
        &mut lines,
        "f",
        "Open the controls overlay (view / grouping / filters / sort)",
    );
    bind(
        &mut lines,
        "p",
        "Open the pins overlay (create / rename / remove / bind / rebind / adopt)",
    );
    bind(&mut lines, "?", "This help");
    blank(&mut lines);

    section(&mut lines, "Actions");
    bind(
        &mut lines,
        "Enter",
        "Default action on the selected row (T8-043): attach mux/muxed sessions, view un-muxed sessions, expand groups",
    );
    bind(&mut lines, "a", "Attach to the selected mux");
    bind(
        &mut lines,
        "i",
        "Copy the selected agent or mux session's full id to the clipboard",
    );
    bind(
        &mut lines,
        "R",
        "Rename the selected agent session or pin's display name",
    );
    bind(
        &mut lines,
        "v",
        "Open the selected session's transcript (or the mux row's linked session); q/Esc close, j/k or PgDn/PgUp scroll, g/G start/end, t cycle tool detail, T thinking",
    );
    bind(&mut lines, "r", "Refresh discovery now");
    bind(
        &mut lines,
        "S",
        "Resume the selected un-muxed agent session in a new terminal",
    );
    bind(&mut lines, "q / Ctrl-C", "Quit");
    blank(&mut lines);

    section(&mut lines, "Pins (ADR 0057)");
    bind(
        &mut lines,
        "p",
        "Open the pins overlay (menu listing every action)",
    );
    bind(
        &mut lines,
        "N",
        "New pin — opens the create form seeded from the current selection",
    );
    bind(
        &mut lines,
        "L",
        "Launch the selected pin (same code path as Enter on an unbound pin row)",
    );
    bind(
        &mut lines,
        "R",
        "Rename the selected pin's display name (same key as session rename)",
    );
    bind(
        &mut lines,
        "B",
        "Rebind the selected pin's mux target (mux name + optional socket)",
    );
    bind(
        &mut lines,
        "b",
        "Bind picker for the selected PinAmbiguous row (status hint otherwise)",
    );
    bind(
        &mut lines,
        "A",
        "Adopt the selected live mux row as a new pin",
    );
    bind(
        &mut lines,
        "Delete",
        "Remove the selected pin (two-press confirmation)",
    );
    blank(&mut lines);

    section(&mut lines, "View switching");
    bind(
        &mut lines,
        "1 – 5",
        "Switch directly to view N (sessions, mux, union, prs, forks)",
    );
    bind(&mut lines, "] / [", "Cycle to next / previous view");
    blank(&mut lines);

    section(&mut lines, "Filters & grouping");
    bind(
        &mut lines,
        "F",
        "Clear all active filters for the visible view",
    );
    bind(
        &mut lines,
        "Ctrl-G",
        "Cycle grouping forward for the active view",
    );
    blank(&mut lines);

    section(&mut lines, "Search");
    bind(
        &mut lines,
        "/",
        "Open the search overlay (ranks within the active filter set)",
    );
    blank(&mut lines);

    section(&mut lines, "Navigation");
    bind(&mut lines, "j / k / ↓ / ↑", "Move selection down / up");
    bind(
        &mut lines,
        "l / → / h / ←",
        "Expand / collapse the selected left-tree row (vi-style fold)",
    );
    bind(&mut lines, "PgDn / PgUp", "Page through the row tree");
    bind(&mut lines, "g / G", "First / last row");
    bind(
        &mut lines,
        "Enter",
        "Left tree: row-kind default action (attach / view / expand); right pane: drill or expand a group",
    );
    bind(
        &mut lines,
        "Tab",
        "Cycle focus between left tree and right panel",
    );
    bind(&mut lines, "J / K", "Scroll the right-panel preview");
    blank(&mut lines);

    section(&mut lines, "Detail-pane graph explorer (right focus)");
    bind(
        &mut lines,
        "j / k",
        "Move the explorer cursor between Node fields and relationship rows",
    );
    bind(
        &mut lines,
        "Enter",
        "Copy the value on a Node-zone field row · drill on a link row · expand on a group header",
    );
    bind(
        &mut lines,
        "e",
        "Toggle expand/collapse on a multi-link group header",
    );
    bind(
        &mut lines,
        "Backspace",
        "Back out of the most recent drilldown hop · once the stack is empty, press twice to return focus to the left pane",
    );
    bind(
        &mut lines,
        "F",
        "Toggle Expanded Node Detail (every per-kind field) on the focused node",
    );
    bind(
        &mut lines,
        "E",
        "Toggle edge meta (provenance · confidence · state) on link rows",
    );
    bind(
        &mut lines,
        "o",
        "Open the full untruncated value for the cursor row in a modal",
    );
    blank(&mut lines);

    section(&mut lines, "Node kind icons (ADR 0073)");
    push_icon_legend(&mut lines, theme);
    blank(&mut lines);

    lines.push(line![
        span!(theme.placeholder; "Press Esc, q, or ? to close.")
    ]);
    lines
}

/// Built-in legend mapping each `NodeKind` glyph to its
/// human-readable name. Operators learn the symbol vocabulary by
/// pressing `?` instead of reading the docs. Mirrors ADR 0073's
/// canonical display order (`NodeKind::ALL`) so the icon column
/// down-reads the same sequence the row tree and detail pane use.
fn push_icon_legend(lines: &mut Vec<Line<'static>>, theme: &Theme) {
    for kind in NodeKind::ALL {
        let style = node_kind_style(kind, theme);
        // ForgePr's slate color is `Color::Reset`; mirror the
        // dodge used in `kind_chip_span` / the breadcrumb /
        // search-result renderers — fall back to `theme.pr_open`
        // since the legend doesn't carry PR state.
        let color = if matches!(kind, NodeKind::ForgePr) {
            theme.pr_open
        } else {
            style.color
        };
        lines.push(line![
            "  ",
            span!(Style::default().fg(color); "{} ", style.glyph),
            span!(Modifier::BOLD; "{:<14}", node_kind_display_name(kind)),
            node_kind_help_blurb(kind).to_string(),
        ]);
    }
}

/// Operator-facing display name for a `NodeKind`. Distinct from
/// `theme_key` (the config-loader handle) and `snake_case` (the
/// stable string tag in non-TUI outputs) so the legend reads
/// naturally — `Agent session`, not `agent_session`.
fn node_kind_display_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Workspace => "Workspace",
        NodeKind::Repo => "Repo",
        NodeKind::Checkout => "Checkout",
        NodeKind::AgentSession => "Agent session",
        NodeKind::MuxSession => "Mux session",
        NodeKind::RuntimeProcess => "Runtime process",
        NodeKind::Branch => "Branch",
        NodeKind::Fork => "Fork",
        NodeKind::ForgePr => "Forge PR",
    }
}

/// One-line context for each kind so the legend is self-explanatory
/// without forcing the operator to cross-reference the design docs.
fn node_kind_help_blurb(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Workspace => "logical bundle of repos (atelier, agent-deck)",
        NodeKind::Repo => "discovered git repository",
        NodeKind::Checkout => "working-tree checkout of a repo",
        NodeKind::AgentSession => "harness session (claude, codex, opencode, …)",
        NodeKind::MuxSession => "tmux / mux backend session",
        NodeKind::RuntimeProcess => "live process attached to a mux pane",
        NodeKind::Branch => "git branch reference",
        NodeKind::Fork => "atelier fork (worktree-backed branch family)",
        NodeKind::ForgePr => "forge pull request (GitHub, …)",
    }
}

fn section(lines: &mut Vec<Line<'static>>, title: &str) {
    lines.push(line![span!(Modifier::BOLD; "{title}")]);
}

fn blank(lines: &mut Vec<Line<'static>>) {
    lines.push(line![""]);
}

/// Centered modal sized to roughly two thirds of the terminal,
/// capped so it stays readable on wide screens.
pub fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(40);
    let max_height = area.height.saturating_sub(2);
    let desired = 28;
    let height = (desired as u16).clamp(10, max_height.max(10));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn help_body_includes_node_kind_icon_legend() {
        // H-UI-002 slice: pressing `?` should surface a built-in
        // legend for the ADR 0073 glyph slate so operators learn
        // the symbol vocabulary without cross-referencing the
        // docs. Every NodeKind in canonical display order must
        // appear with its glyph, its operator-facing display
        // name, and a short blurb.
        let theme = Theme::default();
        let lines = body_lines(&theme);
        let plain_lines: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        let body_text = plain_lines.join("\n");
        assert!(
            body_text.contains("Node kind icons"),
            "expected section header in:\n{body_text}",
        );
        for kind in NodeKind::ALL {
            let glyph = node_kind_style(kind, &theme).glyph;
            let name = node_kind_display_name(kind);
            assert!(
                plain_lines
                    .iter()
                    .any(|line| line.contains(&glyph) && line.contains(name)),
                "expected legend row for {kind:?} ({glyph} {name}) in:\n{body_text}",
            );
        }
    }

    #[test]
    fn esc_closes_help_overlay() {
        let mut state = HelpOverlayState::new();
        assert_eq!(state.handle_key(key(KeyCode::Esc)), HelpOutcome::Close);
    }

    #[test]
    fn q_and_question_mark_close_help_overlay() {
        let mut state = HelpOverlayState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('q'))),
            HelpOutcome::Close
        );
        assert_eq!(
            state.handle_key(key(KeyCode::Char('?'))),
            HelpOutcome::Close
        );
    }

    #[test]
    fn ctrl_c_closes_help_overlay() {
        let mut state = HelpOverlayState::new();
        let event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(state.handle_key(event), HelpOutcome::Close);
    }

    #[test]
    fn other_keys_are_swallowed_not_propagated() {
        let mut state = HelpOverlayState::new();
        assert_eq!(
            state.handle_key(key(KeyCode::Char('j'))),
            HelpOutcome::Continue
        );
        assert_eq!(state.handle_key(key(KeyCode::Enter)), HelpOutcome::Continue);
    }

    #[test]
    fn body_lines_cover_every_documented_key() {
        let theme = Theme::default();
        let rendered: String = body_lines(&theme)
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect::<Vec<_>>()
            .join(" ");
        // Every key-literal reference used in a bind() call must
        // appear in the rendered help text.
        for needle in [
            "f ",
            "? ",
            "Enter ",
            "a ",
            "i ",
            "R ",
            "v ",
            "r ",
            "S ",
            "b ",
            "Delete ",
            "q / Ctrl-C",
            "1 – 5",
            "] / [",
            "F ",
            "Ctrl-G",
            "/ ",
            "j / k / ↓ / ↑",
            "l / → / h / ←",
            "PgDn / PgUp",
            "g / G",
            "Tab",
            "J / K",
            "j / k",
            "e ",
            "Backspace",
            "E ",
            "o ",
        ] {
            assert!(
                rendered.contains(needle),
                "help text missing `{needle}` reference; full text:\n{rendered}"
            );
        }
        // Every action description must also be visible so the
        // operator knows what each binding does.
        for desc in [
            "Open the controls overlay",
            "This help",
            "Default action on the selected row",
            "Attach to the selected mux",
            "Copy the selected agent or mux session's full id",
            "Rename the selected agent session or pin's display name",
            "Open the selected session's transcript",
            "Refresh discovery now",
            "Resume the selected un-muxed agent session",
            "Quit",
            "Open the pins overlay (menu listing every action)",
            "New pin — opens the create form",
            "Launch the selected pin",
            "Rebind the selected pin's mux target",
            "Bind picker for the selected PinAmbiguous row",
            "Adopt the selected live mux row as a new pin",
            "Remove the selected pin (two-press confirmation)",
            "Switch directly to view",
            "Cycle to next / previous view",
            "Clear all active filters",
            "Cycle grouping forward",
            "Open the search overlay",
            "Move selection down / up",
            "Expand / collapse the selected left-tree row",
            "Page through the row tree",
            "First / last row",
            "Cycle focus",
            "Scroll the right-panel preview",
            "Move the explorer cursor",
            "Toggle expand/collapse on a multi-link",
            "Back out of the most recent drilldown",
            "Toggle Expanded Node Detail",
            "Toggle edge meta",
            "Open the full untruncated value",
        ] {
            assert!(
                rendered.contains(desc),
                "help text missing description `{desc}`; full text:\n{rendered}"
            );
        }
    }
}
