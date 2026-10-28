//! `?` help overlay.
//!
//! Minimal reference card for the current keymap. Render-only — no
//! mutation, no sub-editors — so the state struct is empty today and
//! exists mainly so the runtime can use the same open/close pattern
//! it uses for the controls, rename, and search overlays. As more
//! capabilities accumulate this can grow tabs or scrolling without
//! changing its boundary with the runtime.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Margin, Rect};
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget};
use ratatui_cheese::help::Binding;
use tui_popup::KnownSize;

use crate::tui::Theme;
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::widgets::popup_frame::ScrollFit;

/// Pure state for the help overlay. Carries vertical scroll
/// position so long keymaps stay reachable on short terminals.
#[derive(Debug, Clone, Default)]
pub struct HelpOverlayState {
    pub scroll: u16,
    /// Rows of the page the modal showed at the last draw; a page key
    /// moves by this many.
    pub page_rows: u16,
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
        use crate::tui::keybindings::overlay::{HELP_KEYS, PagerKey, resolve};
        match resolve(HELP_KEYS, event) {
            Some(PagerKey::Abort | PagerKey::Close) => HelpOutcome::Close,
            Some(PagerKey::Down) => {
                self.scroll_by(1);
                HelpOutcome::Continue
            }
            Some(PagerKey::Up) => {
                self.scroll_by(-1);
                HelpOutcome::Continue
            }
            Some(PagerKey::PageDown) => {
                self.scroll_by(i32::from(self.page_rows.max(1)));
                HelpOutcome::Continue
            }
            Some(PagerKey::PageUp) => {
                self.scroll_by(-i32::from(self.page_rows.max(1)));
                HelpOutcome::Continue
            }
            Some(PagerKey::Top) => {
                self.scroll = 0;
                HelpOutcome::Continue
            }
            Some(PagerKey::Bottom) => {
                self.scroll = u16::MAX;
                HelpOutcome::Continue
            }
            Some(PagerKey::Copy) | None => HelpOutcome::Continue,
        }
    }

    fn scroll_by(&mut self, delta: i32) {
        let current = i32::from(self.scroll);
        let next = current.saturating_add(delta).max(0);
        self.scroll = u16::try_from(next.min(i32::from(u16::MAX))).unwrap_or(u16::MAX);
    }

    /// Take in what the last draw showed: the offset comes back inside
    /// the page (so `G`, or `j` past the end, is undone by the next key
    /// up) and page keys move by the rows on screen.
    pub fn fit(&mut self, fit: ScrollFit) {
        let max = u16::try_from(fit.max_scroll).unwrap_or(u16::MAX);
        self.scroll = self.scroll.min(max);
        self.page_rows = u16::try_from(fit.rows).unwrap_or(u16::MAX);
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

impl HelpOverlayWidget<'_> {
    /// Draw the help and report what the page showed, for
    /// [`HelpOverlayState::fit`].
    pub fn draw(self, area: Rect, buf: &mut Buffer) -> ScrollFit {
        let theme = self.theme;
        render_help_page(area, buf, theme, self.state.scroll as usize, &|width| {
            body_lines_for_width(theme, Some(width))
        })
    }
}

impl Widget for HelpOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.draw(area, buf);
    }
}

/// Draw a help page: the shared popup frame with the ` Help ` title,
/// the page's lines scrolled to `scroll` and wrapped to the modal's
/// width, and a scrollbar when the page overflows. The keymap help and
/// the transcript viewer's help both draw through here, so the two
/// modals share one frame, size, and scroll behavior. Returns the rows
/// the page's scrolling part had and how far it can scroll; the footer
/// is pinned, so only the lines above it scroll.
pub(super) fn render_help_page(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    scroll: usize,
    lines_for: &dyn Fn(usize) -> Vec<Line<'static>>,
) -> ScrollFit {
    // The centered-bordered-modal shell — clear, border, title — is
    // owned by `tui_popup::Popup` via
    // `crate::tui::widgets::popup_frame::themed_popup`. The body
    // wrapper reports the same cap dimensions `help_modal_rect`
    // computes, so the popup's auto-sizing reproduces that rect.
    let modal = help_modal_rect(area);
    let mut fit = ScrollFit::default();
    let body = HelpBody {
        scroll,
        lines_for,
        inner_width: modal.width.saturating_sub(2) as usize,
        inner_height: modal.height.saturating_sub(2) as usize,
        fit: &mut fit,
    };
    let popup = crate::tui::widgets::popup_frame::themed_popup(body, line![" Help "], theme);
    popup.render(area, buf);
    fit
}

/// A help page split into the lines that scroll and the footer that
/// stays on the modal's last rows.
struct PageLayout {
    body: Vec<Line<'static>>,
    footer: Vec<Line<'static>>,
    /// Rows the scrolling part has.
    body_height: usize,
    /// Columns the text has (one fewer when a scrollbar is drawn).
    text_width: usize,
    overflows: bool,
}

/// Rows the footer takes at `width`.
fn footer_rows(width: usize) -> usize {
    if width <= 8 {
        return 1;
    }
    let footer =
        crate::tui::keybindings::overlay::footer(crate::tui::keybindings::overlay::HELP_KEYS);
    crate::viewer::render::word_wrap(&footer, u16::try_from(width).unwrap_or(u16::MAX))
        .len()
        .max(1)
}

fn page_layout(
    lines_for: &dyn Fn(usize) -> Vec<Line<'static>>,
    width: usize,
    height: usize,
) -> PageLayout {
    let split = |lines: Vec<Line<'static>>, text_width: usize| {
        let keep = footer_rows(text_width).min(height.saturating_sub(1));
        let at = lines.len().saturating_sub(keep);
        let mut body = lines;
        let footer = body.split_off(at);
        (body, footer)
    };
    let (body, footer) = split(lines_for(width), width);
    let body_height = height.saturating_sub(footer.len());
    if body.len() <= body_height || width <= 1 {
        return PageLayout {
            body,
            footer,
            body_height,
            text_width: width,
            overflows: false,
        };
    }
    // The scrollbar takes a column once the page overflows.
    let (body, footer) = split(lines_for(width - 1), width - 1);
    let body_height = height.saturating_sub(footer.len());
    PageLayout {
        body,
        footer,
        body_height,
        text_width: width - 1,
        overflows: true,
    }
}

/// Body wrapper for `tui_popup::Popup`. Reports the inner width /
/// height the in-tree `help_modal_rect` cap produces so the popup's
/// auto-sizing reproduces that rect.
struct HelpBody<'a> {
    scroll: usize,
    lines_for: &'a dyn Fn(usize) -> Vec<Line<'static>>,
    inner_width: usize,
    inner_height: usize,
    /// Where the body reports the rows it drew.
    fit: &'a mut ScrollFit,
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
        let page = page_layout(self.lines_for, area.width as usize, area.height as usize);
        let text_width = u16::try_from(page.text_width).unwrap_or(area.width);
        let body_height = u16::try_from(page.body_height).unwrap_or(area.height);
        let max_scroll = page.body.len().saturating_sub(page.body_height);
        let scroll = self.scroll.min(max_scroll);
        *self.fit = ScrollFit {
            rows: page.body_height,
            max_scroll,
        };
        let body_area = Rect {
            width: text_width,
            height: body_height,
            ..area
        };
        Paragraph::new(page.body)
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
            .render(body_area, buf);
        // The footer stays on the last rows whatever the scroll.
        let footer_area = Rect {
            x: area.x,
            y: area.y + body_height,
            width: text_width,
            height: area.height.saturating_sub(body_height),
        };
        Paragraph::new(page.footer).render(footer_area, buf);
        if page.overflows {
            let bar_area = Rect {
                x: area.x + area.width - 1,
                width: 1,
                height: body_height,
                ..area
            };
            let mut bar_state = ScrollbarState::new(max_scroll + 1)
                .position(scroll)
                .viewport_content_length(page.body_height);
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
                    "Open the controls overlay (view / grouping / filters / sort / preview wrap); Space selects or toggles, Enter applies and closes, Esc closes",
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
                    "Default action on the selected row: attach mux/muxed sessions, view un-muxed sessions, rebuild a stopped mux, pin, or history record, expand groups",
                ),
                Binding::new("a", "Attach to the selected mux"),
                Binding::new(
                    "i",
                    "Copy the selected agent or mux session's full id to the clipboard",
                ),
                Binding::new(
                    "R",
                    "Rename the selected agent session (alias) or mux (tmux + pin cascade)",
                ),
                Binding::new(
                    "w",
                    "Open the worktree action menu for the selected repo / worktree / mux (new, merge, remove, close-down, prune, reveal; mutations need a backend)",
                ),
                Binding::new(
                    "X",
                    "Close down the selected stream: merge or discard the branch, end its sessions, remove the worktree, forget its history",
                ),
                Binding::new(
                    "v",
                    "Open the selected session's transcript (or the mux row's linked session); q/Esc close, j/k or PgDn/PgUp scroll, g/G start/end, t cycle tool detail, T thinking, / find then n/N step through matches, ? lists every key",
                ),
                Binding::new("r", "Refresh discovery now"),
                Binding::new("Ctrl-R", crate::tui::keybindings::HARD_REFRESH_HELP),
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
                    "Create a bare tmux session (no agent, no worktree) — shell in the selected cwd",
                ),
                Binding::new(
                    "N",
                    "Launch a harness in a new mux (the form can also create a worktree for it)",
                ),
                Binding::new(
                    "m",
                    "Open the Mux action menu — pick between new bare session and launching a harness in a fresh mux",
                ),
            ],
        },
        HelpSection {
            title: "Row actions",
            bindings: vec![
                Binding::new(
                    "p",
                    "Pin the selected row, or unpin it when it is already pinned; pinned rows carry ◉ (◎ when the pinned entity isn't running)",
                ),
                Binding::new(
                    crate::tui::keybindings::UNDO_KEY.to_string(),
                    "Undo the last pin or unpin, as far back as this session goes; a removed pin comes back with its overrides and its place in the order",
                ),
                Binding::new(
                    ".",
                    "Open the row action menu — pin or unpin; rebuild, forget, or show the rebuild plan for a history record; edit, rename, retarget, refresh, or reveal a pin in the Pinned view; Clean up… in the History view",
                ),
            ],
        },
        HelpSection {
            title: "View switching",
            bindings: vec![
                Binding::new(
                    "1 / 2 / 3 / 4",
                    "Switch directly to the sessions / mux / PRs / pinned view",
                ),
                Binding::new("5", "Switch directly to the history view"),
                Binding::new("6", "Switch directly to the Search view"),
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
            bindings: vec![
                Binding::new(
                    "/",
                    "Quick select: find a row of this view by name (rows under collapsed groups too); Enter selects it",
                ),
                Binding::new(
                    "/ then Tab",
                    "Find anything, whichever view it lives in (Tab again goes back to this view); the word at the right says whether Enter selects it here, follows its relationships in the right pane, or switches views",
                ),
                Binding::new(
                    "Ctrl-/ (or Ctrl-_ / Ctrl-7) / s",
                    "Search contents: what was said in the sessions and what is in mux scrollback, in this view's sessions and muxes (Tab searches all of them). Ctrl-L switches between listing sessions and listing messages; Ctrl-O opens the options (tool calls, thinking, pane scrollback); Ctrl-S keeps the search in the Search view and goes there; Ctrl-F keeps the query as a filter on the lists. `s` works in every terminal",
                ),
                Binding::new(
                    "< / >",
                    "Search view: show the previous (older) or next (newer) remembered search without searching again; Controls lists them",
                ),
                Binding::new(
                    "Ctrl-L",
                    "Search view: list sessions or messages (the same matches, no new scan)",
                ),
                Binding::new(
                    "Ctrl-O",
                    "Search view: choose what a search looks in (tool calls, thinking, pane scrollback)",
                ),
                Binding::new(
                    "Ctrl-F",
                    "Search view: keep the search shown as the filter on the lists (paging does not change the filter; F clears it)",
                ),
            ],
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
                    "Left tree: row-kind default action (attach / view / rebuild / expand); right pane: drill or expand a group",
                ),
                Binding::new(
                    "Shift-Enter / Alt-Enter / O",
                    "Reveal the row's entity in its own view (Sessions, Mux, PRs, History); Backspace on the left tree returns. Shift-Enter needs a terminal that reports it",
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
    push_icon_legend(&mut lines, theme, width);
    blank(&mut lines);

    self::section(&mut lines, "Link and mux state");
    push_mux_state_legend(&mut lines, theme, width);
    blank(&mut lines);

    self::section(&mut lines, "Pull request state");
    push_pr_state_legend(&mut lines, theme, width);
    blank(&mut lines);

    self::section(&mut lines, "Pins");
    push_pin_marker_legend(&mut lines, theme, width);
    blank(&mut lines);

    push_footer(&mut lines, theme, width);
    lines
}

/// The closing line of a help page, generated from the key table that
/// scrolls and closes it, wrapped to `width` when known.
pub(super) fn push_footer(lines: &mut Vec<Line<'static>>, theme: &Theme, width: Option<usize>) {
    let footer =
        crate::tui::keybindings::overlay::footer(crate::tui::keybindings::overlay::HELP_KEYS);
    match width.filter(|w| *w > 8) {
        Some(w) => {
            let budget = u16::try_from(w).unwrap_or(u16::MAX);
            for chunk in crate::viewer::render::word_wrap(&footer, budget) {
                lines.push(line![span!(theme.placeholder; "{}", chunk.trim_end())]);
            }
        }
        None => lines.push(line![span!(theme.placeholder; "{footer}")]),
    }
}

/// The footer as plain text lines at `width`, for tests that compare the
/// two help pages.
#[cfg(test)]
pub(crate) fn footer_lines_for_test(theme: &Theme, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    push_footer(&mut lines, theme, Some(width));
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

/// Cells before the blurb in a legend row: two of indent, the glyph and
/// its space, and a 16-cell name column.
const LEGEND_LEAD_WIDTH: usize = 2 + 2 + 16;
/// The same for the pin legend, whose names are longer.
const PIN_LEGEND_LEAD_WIDTH: usize = 2 + 2 + 20;

/// Push one legend row: `lead` spans after a two-cell indent, then the
/// blurb, wrapped under a hanging indent that lines up with it when
/// `width` is known so no text runs past the modal edge.
fn legend_entry(
    lines: &mut Vec<Line<'static>>,
    lead: Vec<ratatui::text::Span<'static>>,
    lead_width: usize,
    blurb: &str,
    width: Option<usize>,
) {
    let mut first = vec![ratatui::text::Span::raw("  ")];
    first.extend(lead);
    let Some(width) = width.filter(|w| *w > lead_width + 8) else {
        first.push(ratatui::text::Span::raw(blurb.to_string()));
        lines.push(Line::from(first));
        return;
    };
    let budget = u16::try_from(width - lead_width).unwrap_or(u16::MAX);
    let mut chunks = crate::viewer::render::word_wrap(blurb, budget).into_iter();
    first.push(ratatui::text::Span::raw(
        chunks.next().unwrap_or_default().trim_end().to_string(),
    ));
    lines.push(Line::from(first));
    for chunk in chunks {
        lines.push(line![" ".repeat(lead_width), chunk.trim_end().to_string()]);
    }
}

/// Width of the `  <key:<14>` column that precedes each description.
const KEY_COLUMN_WIDTH: usize = 16;

/// Render a binding as `  <key:<14>  <description>`, wrapping the
/// description under a hanging indent when `width` is too narrow. The
/// key column inherits `theme.panel_focus_accent` so operators scan
/// the keymap by accent color.
pub(super) fn binding_lines(
    key: &str,
    description: &str,
    theme: &Theme,
    width: Option<usize>,
) -> Vec<Line<'static>> {
    // A key label wider than its column gets a line to itself, with the
    // description wrapped beneath on the hanging indent.
    if unicode_width::UnicodeWidthStr::width(key) > KEY_COLUMN_WIDTH - 2 {
        let key_line = line![span!(Style::default().fg(theme.panel_focus_accent); "  {key}")];
        let mut out = vec![key_line];
        let budget = width.map_or(u16::MAX, |w| {
            u16::try_from(w.saturating_sub(KEY_COLUMN_WIDTH)).unwrap_or(u16::MAX)
        });
        for chunk in crate::viewer::render::word_wrap(description, budget.max(8)) {
            out.push(line![
                " ".repeat(KEY_COLUMN_WIDTH),
                chunk.trim_end().to_string()
            ]);
        }
        return out;
    }
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
fn push_icon_legend(lines: &mut Vec<Line<'static>>, theme: &Theme, width: Option<usize>) {
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
        legend_entry(
            lines,
            vec![
                span!(Style::default().fg(color); "{} ", style.glyph),
                span!(Modifier::BOLD; "{:<16}", node_kind_display_name(kind)),
            ],
            LEGEND_LEAD_WIDTH,
            node_kind_help_blurb(kind),
            width,
        );
    }
}

/// Legend for the glyphs in the row tree's attach-state column plus the
/// ambiguity warning on group rows and the header. The same glyph can
/// answer a different question on a session row than on a mux row, so
/// the blurb names both where they differ.
fn push_mux_state_legend(lines: &mut Vec<Line<'static>>, theme: &Theme, width: Option<usize>) {
    use crate::tui::icons::state;
    let bold_warning = Style::default()
        .fg(theme.warning)
        .add_modifier(Modifier::BOLD);
    let ghost = Style::default()
        .fg(theme.ghost)
        .add_modifier(Modifier::ITALIC);
    let entries: [(&str, Style, &str, &str); 5] = [
        (
            state::LINKED,
            Style::default().fg(theme.mux_attached),
            "Linked",
            "session: one mux, Enter/a attaches · mux: client attached",
        ),
        (
            state::UNLINKED,
            Style::default().add_modifier(theme.mux_unmuxed),
            "Not linked",
            "session: no mux, or several candidates · mux: no client",
        ),
        (
            state::STOPPED,
            ghost,
            "Stopped",
            "a mux that is no longer running; Enter starts it again",
        ),
        (
            state::UNKNOWN,
            Style::default().add_modifier(theme.placeholder),
            "Unknown",
            "the mux backend didn't report whether a client is attached",
        ),
        (
            "▲",
            bold_warning,
            "Ambiguous mux",
            "sessions with several candidate muxes; the detail pane lists them",
        ),
    ];
    for (glyph, style, name, blurb) in entries {
        legend_entry(
            lines,
            vec![
                span!(style; "{glyph} "),
                span!(Modifier::BOLD; "{name:<16}"),
            ],
            LEGEND_LEAD_WIDTH,
            blurb,
            width,
        );
    }
}

/// Legend for the pull request state glyphs. The Pinned and History
/// views spell the same states as words instead.
fn push_pr_state_legend(lines: &mut Vec<Line<'static>>, theme: &Theme, width: Option<usize>) {
    use crate::tui::icons::state;
    let entries: [(&str, Style, &str, &str); 4] = [
        (
            state::PR_OPEN,
            Style::default().fg(theme.pr_open),
            "Open",
            "ready for review",
        ),
        (
            state::PR_DRAFT,
            Style::default().fg(theme.pr_draft),
            "Draft",
            "open, but marked as a draft",
        ),
        (
            state::PR_MERGED,
            Style::default().fg(theme.pr_merged),
            "Merged",
            "landed on its base branch",
        ),
        (
            state::PR_CLOSED,
            Style::default().fg(theme.pr_closed),
            "Closed",
            "closed without merging",
        ),
    ];
    for (glyph, style, name, blurb) in entries {
        legend_entry(
            lines,
            vec![
                span!(style; "{glyph} "),
                span!(Modifier::BOLD; "{name:<16}"),
            ],
            LEGEND_LEAD_WIDTH,
            blurb,
            width,
        );
    }
}

/// Legend for the pin marker: filled for something a pin points at,
/// hollow and in the ghost style for a pin with nothing running.
fn push_pin_marker_legend(lines: &mut Vec<Line<'static>>, theme: &Theme, width: Option<usize>) {
    use crate::tui::icons::state;
    let pin_color = node_kind_style(NodeKind::Pin, theme).color;
    let entries: [(&str, Style, &str, &str); 2] = [
        (
            state::PIN_RUNNING,
            Style::default().fg(pin_color),
            "Pinned",
            "a pin points at this row and it is running; the Pinned view lists every pin",
        ),
        (
            state::PIN_STOPPED,
            Style::default()
                .fg(theme.ghost)
                .add_modifier(Modifier::ITALIC),
            "Pinned, not running",
            "the pinned entity is stopped, removed, missing, or not started yet; Enter starts it",
        ),
    ];
    for (glyph, style, name, blurb) in entries {
        legend_entry(
            lines,
            vec![
                span!(style; "{glyph} "),
                span!(Modifier::BOLD; "{name:<20}"),
            ],
            PIN_LEGEND_LEAD_WIDTH,
            blurb,
            width,
        );
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
        NodeKind::Pin => "pin: a named place pointing at a mux, session, checkout, repo, or PR",
        NodeKind::RuntimeProcess => "live process attached to a mux pane",
        NodeKind::Branch => "git branch reference",
        NodeKind::Fork => "atelier fork (worktree-backed branch family)",
        NodeKind::ForgePr => "forge pull request (GitHub, …)",
    }
}

pub(super) fn section(lines: &mut Vec<Line<'static>>, title: &str) {
    lines.push(line![span!(Modifier::BOLD; "{title}")]);
}

pub(super) fn blank(lines: &mut Vec<Line<'static>>) {
    lines.push(line![""]);
}

/// Help-overlay modal: as tall as the terminal allows (less a small
/// margin) so more of the keymap is visible at once, and up to 100
/// columns wide so long descriptions wrap less.
pub(crate) fn help_modal_rect(area: Rect) -> Rect {
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
