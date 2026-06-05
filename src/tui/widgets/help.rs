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
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::Theme;

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
        let modal = centered_modal_rect(area);
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Help "));
        let inner = block.inner(modal);
        block.render(modal, buf);
        let para = Paragraph::new(body_lines(self.theme)).scroll((self.state.scroll, 0));
        para.render(inner, buf);
    }
}

/// Build the static keymap. The first column is the key, the second
/// is a one-line description. Sections are bold; bindings are plain
/// text.
fn body_lines(theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let bind = |lines: &mut Vec<Line<'static>>, key: &str, desc: &str| {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {key:<14}"),
                Style::default().fg(theme.panel_focus_accent),
            ),
            Span::raw(desc.to_string()),
        ]));
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

    lines.push(Line::from(Span::styled(
        "Press Esc, q, or ? to close.",
        Style::default().add_modifier(theme.placeholder),
    )));
    lines
}

fn section(lines: &mut Vec<Line<'static>>, title: &str) {
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
}

fn blank(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(""));
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
