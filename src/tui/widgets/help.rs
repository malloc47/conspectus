//! `?` help overlay.
//!
//! Minimal reference card for the current keymap. Render-only — no
//! mutation, no sub-editors — so the state struct is empty today and
//! exists mainly so the runtime can use the same open/close pattern
//! it uses for the controls, rename, and search overlays. As more
//! capabilities accumulate this can grow tabs or scrolling without
//! changing its boundary with the runtime.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Margin, Rect};
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget};
use ratatui_cheese::help::Binding;
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
            KeyCode::Esc | KeyCode::Char('q' | '?') => HelpOutcome::Close,
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

impl crate::tui::Overlay for HelpOverlayState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> crate::tui::OverlayOutcome {
        match self.handle_key(key) {
            HelpOutcome::Continue => crate::tui::OverlayOutcome::Consumed,
            HelpOutcome::Close => crate::tui::OverlayOutcome::Close,
        }
    }
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
        // The centered-bordered-modal shell — clear,
        // border, title — is owned by `tui_popup::Popup` via
        // `crate::tui::widgets::popup_frame::themed_popup`. The body
        // wrapper reports the same cap dimensions
        // `centered_modal_rect` computed before, so the popup's
        // auto-sizing reproduces the in-tree rect.
        let modal = help_modal_rect(area);
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
        // Reserve the rightmost column for the scrollbar only when the
        // keymap overflows, so short keymaps keep the full width.
        let full = body_lines_for_width(self.theme, Some(area.width as usize));
        let overflows = full.len() > area.height as usize && area.width > 1;
        let text_width = if overflows {
            area.width - 1
        } else {
            area.width
        };
        let lines = if overflows {
            body_lines_for_width(self.theme, Some(text_width as usize))
        } else {
            full
        };
        let content_len = lines.len();
        let viewport = area.height as usize;
        let max_scroll = content_len.saturating_sub(viewport);
        let scroll = (self.state.scroll as usize).min(max_scroll);
        let text_area = Rect {
            width: text_width,
            ..area
        };
        Paragraph::new(lines)
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
            .render(text_area, buf);
        if overflows {
            let bar_area = Rect {
                x: area.x + area.width - 1,
                width: 1,
                ..area
            };
            let mut bar_state = ScrollbarState::new(max_scroll + 1)
                .position(scroll)
                .viewport_content_length(viewport);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(Style::default().remove_modifier(Modifier::REVERSED));
            ratatui::widgets::StatefulWidget::render(
                scrollbar,
                bar_area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                &mut bar_state,
            );
        }
    }
}

/// One named section in the help overlay's keymap. Sections render
/// as a bold header followed by every enabled binding in the
/// section, then a blank spacer row.
struct HelpSection {
    title: &'static str,
    bindings: Vec<Binding>,
}

/// The static keymap as data. Sections come first; the node-kind
/// icon legend is rendered separately because it's not a binding
/// list. Bindings are
/// [`ratatui_cheese::help::Binding`](Binding) so future refactors
/// (filter to a view-specific set, disable a binding, etc.) become
/// operations on the data rather than on rendered lines. The
/// renderer keeps the in-tree sectioned-vertical layout — cheese's
/// short / multi-column modes don't fit our 200+ char descriptions.
fn keymap_sections() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: "Discoverable controls",
            bindings: vec![
                Binding::new(
                    "f",
                    "Open the controls overlay (view / grouping / filters / sort / preview wrap)",
                ),
                Binding::new(
                    "p",
                    "Open the pins overlay (create / edit / remove / bind / rebind / adopt)",
                ),
                Binding::new("!", crate::tui::keybindings::MESSAGES_HELP),
                Binding::new("?", "This help"),
            ],
        },
        HelpSection {
            title: "Actions",
            bindings: vec![
                Binding::new(
                    "Enter",
                    "Default action on the selected row: attach mux/muxed sessions, view un-muxed sessions, expand groups",
                ),
                Binding::new("a", "Attach to the selected mux"),
                Binding::new(
                    "i",
                    "Copy the selected agent or mux session's full id to the clipboard",
                ),
                Binding::new(
                    "R",
                    "Rename the selected agent session (alias), mux (tmux + pin cascade), or pin's display name",
                ),
                Binding::new(
                    "w",
                    "Open the worktree action menu for the selected repo / worktree / mux (new, merge, remove, close-down, prune, reveal; mutations need a backend)",
                ),
                Binding::new(
                    "X",
                    "Close down the selected stream: merge or discard the branch, end its sessions, remove the worktree, drop its pins",
                ),
                Binding::new(
                    "v",
                    "Open the selected session's transcript (or the mux row's linked session); q/Esc close, j/k or PgDn/PgUp scroll, g/G start/end, t cycle tool detail, T thinking",
                ),
                Binding::new("r", "Refresh discovery now"),
                Binding::new(
                    "S",
                    "Resume the selected un-muxed agent session in a new terminal",
                ),
                Binding::new("q / Ctrl-C", "Quit"),
            ],
        },
        HelpSection {
            title: "Mux",
            bindings: vec![
                Binding::new(
                    "n",
                    "Create a bare tmux session (no pin, no agent, no worktree) — shell in the selected cwd",
                ),
                Binding::new(
                    "m",
                    "Open the Mux action menu — pick between new bare session and launching a harness in a fresh mux with no pin",
                ),
            ],
        },
        HelpSection {
            title: "Pins",
            bindings: vec![
                Binding::new("p", "Open the pins overlay (menu listing every action)"),
                Binding::new(
                    "N",
                    "New stream — opens the create form (worktree toggle pre-enabled) seeded from the current selection; Space toggles the worktree off for a plain pin",
                ),
                Binding::new(
                    "L",
                    "Launch the selected pin (same code path as Enter on an unbound pin row)",
                ),
                Binding::new(
                    "R",
                    "Rename the selected pin's display name (same key as session rename)",
                ),
                Binding::new(
                    "B",
                    "Rebind the selected pin's mux target (mux name + optional socket)",
                ),
                Binding::new(
                    "b",
                    "Bind picker for the selected PinAmbiguous row (status hint otherwise)",
                ),
                Binding::new("A", "Adopt the selected live mux row as a new pin"),
                Binding::new("Delete", "Remove the selected pin (two-press confirmation)"),
            ],
        },
        HelpSection {
            title: "View switching",
            bindings: vec![
                Binding::new("1 / 2", "Switch directly to the sessions / mux view"),
                Binding::new("] / [", "Cycle to next / previous view"),
            ],
        },
        HelpSection {
            title: "Filters & grouping",
            bindings: vec![
                Binding::new("F", "Clear all active filters for the visible view"),
                Binding::new("Ctrl-G", "Cycle grouping forward for the active view"),
            ],
        },
        HelpSection {
            title: "Search",
            bindings: vec![Binding::new(
                "/",
                "Open the search overlay (ranks within the active filter set)",
            )],
        },
        HelpSection {
            title: "Navigation",
            bindings: vec![
                Binding::new("j / k / ↓ / ↑", "Move selection down / up"),
                Binding::new(
                    "l / → / h / ←",
                    "Expand / collapse the selected left-tree row (vi-style fold)",
                ),
                Binding::new("PgDn / PgUp", "Page through the row tree"),
                Binding::new("g / G", "First / last row"),
                Binding::new(
                    "Enter",
                    "Left tree: row-kind default action (attach / view / expand); right pane: drill or expand a group",
                ),
                Binding::new("Tab", "Cycle focus between left tree and right panel"),
                Binding::new("J / K", "Scroll the right-panel preview"),
            ],
        },
        HelpSection {
            title: "Detail-pane graph explorer (right focus)",
            bindings: vec![
                Binding::new(
                    "j / k",
                    "Move the explorer cursor between Node fields and relationship rows",
                ),
                Binding::new(
                    "Enter",
                    "Copy the value on a Node-zone field row · drill on a link row · expand on a group header",
                ),
                Binding::new("e", "Toggle expand/collapse on a multi-link group header"),
                Binding::new(
                    "Backspace",
                    "Back out of the most recent drilldown hop · once the stack is empty, press twice to return focus to the left pane",
                ),
                Binding::new(
                    "F",
                    "Toggle Expanded Node Detail (every per-kind field) on the focused node",
                ),
                Binding::new(
                    "E",
                    "Toggle edge meta (provenance · confidence · state) on link rows",
                ),
                Binding::new(
                    "o",
                    "Open the full untruncated value for the cursor row in a modal",
                ),
            ],
        },
    ]
}

/// Render the keymap as `Vec<Line<'static>>` lines, one per
/// binding plus section headers, blank spacers, the icon legend,
/// and the close hint. Layout stays sectioned-vertical because
/// cheese's short / multi-column Help modes don't fit our long
/// descriptions; the win is that the keymap is now data
/// ([`keymap_sections`]) rather than imperative `bind(...)` calls.
#[cfg(test)]
fn body_lines(theme: &Theme) -> Vec<Line<'static>> {
    body_lines_for_width(theme, None)
}

/// [`body_lines`] with binding descriptions wrapped to `width` cells
/// under a hanging indent aligned with the description column. `None`
/// leaves every binding on one line.
fn body_lines_for_width(theme: &Theme, width: Option<usize>) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for section in keymap_sections() {
        self::section(&mut lines, section.title);
        for binding in &section.bindings {
            if !binding.is_enabled() {
                continue;
            }
            lines.extend(binding_lines(
                binding.key(),
                binding.description(),
                theme,
                width,
            ));
        }
        blank(&mut lines);
    }

    self::section(&mut lines, "Node kind icons");
    push_icon_legend(&mut lines, theme);
    blank(&mut lines);

    lines.push(line![
        span!(theme.placeholder; "j/k or PgDn/PgUp to scroll · Esc, q, or ? to close.")
    ]);
    lines
}

/// Width of the `  <key:<14>` column that precedes each description.
const KEY_COLUMN_WIDTH: usize = 16;

/// Render a binding as `  <key:<14>  <description>`, wrapping the
/// description under a hanging indent when `width` is too narrow. The
/// key column inherits `theme.panel_focus_accent` so operators scan
/// the keymap by accent color.
fn binding_lines(
    key: &str,
    description: &str,
    theme: &Theme,
    width: Option<usize>,
) -> Vec<Line<'static>> {
    let key_span = span!(Style::default().fg(theme.panel_focus_accent); "  {key:<14}");
    let Some(width) = width.filter(|w| *w > KEY_COLUMN_WIDTH + 8) else {
        return vec![line![key_span, description.to_string()]];
    };
    let budget = u16::try_from(width - KEY_COLUMN_WIDTH).unwrap_or(u16::MAX);
    let mut chunks = crate::viewer::render::word_wrap(description, budget).into_iter();
    let first = chunks.next().unwrap_or_default();
    let mut out = vec![line![key_span, first.trim_end().to_string()]];
    for chunk in chunks {
        out.push(line![
            " ".repeat(KEY_COLUMN_WIDTH),
            chunk.trim_end().to_string()
        ]);
    }
    out
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
            span!(Modifier::BOLD; "{:<16}", node_kind_display_name(kind)),
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
        NodeKind::Pin => "Pin",
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
        NodeKind::Pin => "declared pinned session intent",
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

/// Help-overlay modal: as tall as the terminal allows (less a small
/// margin) so more of the keymap is visible at once, and up to 100
/// columns wide so long descriptions wrap less.
fn help_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(100, area.width.saturating_sub(4)).max(40);
    let height = area.height.saturating_sub(4).max(10);
    super::popup_frame::centered_rect(area, width, height)
}

/// Centered modal sized to roughly two thirds of the terminal,
/// capped so it stays readable on wide screens. Shared with the
/// value modal.
pub fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(40);
    let max_height = area.height.saturating_sub(2);
    let height = 28u16.clamp(10, max_height.max(10));
    super::popup_frame::centered_rect(area, width, height)
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
