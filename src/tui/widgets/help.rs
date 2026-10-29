//! `?` help overlay: the keymap's reference card, the framework's help
//! page (`tuikit::help::HelpPage`) read in its pager.
//!
//! The pager owns the reading: scrolling and viewport-sized paging, `g`
//! and `G` (which keeps to the last page), the scrollbar, `Esc`, `q`, and
//! `?` (the key that opened it) closing, and the footer generated from
//! those keys. The viewers' pages are the same page generated from their
//! key tables, with the footer of a viewer's help level (where `q` leaves
//! the viewer), so the three share one frame, one column grid, and one
//! heading style. This one's rows are hand-written prose, kept as
//! annotation sections until the program-wide keymap carries help text the
//! page can be generated from (backlog `CSP-760`), followed by legends of
//! the marks the screens draw.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::glyphs::marks;
use tuikit::help::{HelpPage, HelpSection};
use tuikit::pager::Pager;

use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::keybindings::overlay::HELP_KEYS;
use crate::tui::{Msg, Theme};

/// The keymap help: the framework's pager over the keymap's page.
pub type KeymapHelp = Pager<HelpPage<Msg>>;

/// The keymap help at the top, its marks in `theme`'s styles, resolving
/// `HELP_KEYS` (the pager's keys and `?`, closing as it opened).
pub fn open(theme: &Theme) -> KeymapHelp {
    HelpPage::new(sections(theme)).pager(HELP_KEYS)
}

/// The keymap page: the keymap's sections, then the legends.
pub(crate) fn sections(theme: &Theme) -> Vec<HelpSection> {
    let mut sections = keymap_sections();
    sections.extend([
        icon_legend(theme),
        mux_state_legend(theme),
        pr_state_legend(theme),
        pin_marker_legend(theme),
    ]);
    sections
}

/// The keymap as hand-written sections. Their prose says more than a
/// key table's one-line descriptions can, so they stay annotations until
/// the program-wide keymap's rows carry help text of their own.
fn keymap_sections() -> Vec<HelpSection> {
    vec![
        HelpSection::new("Discoverable controls")
            .keys(
                "f",
                "Open the controls overlay (view / grouping / filters / sort / preview wrap); Space selects or toggles, Enter applies and closes, Esc closes",
            )
            .keys("!", crate::tui::keybindings::MESSAGES_HELP)
            .keys("?", "This help"),
        HelpSection::new("Actions")
            .keys(
                "Enter",
                "Default action on the selected row: attach mux/muxed sessions, view un-muxed sessions, rebuild a stopped mux, pin, or history record, expand groups",
            )
            .keys("a", "Attach to the selected mux")
            .keys(
                "i",
                "Copy the selected agent or mux session's full id to the clipboard",
            )
            .keys(
                "R",
                "Rename the selected agent session (alias) or mux (tmux + pin cascade)",
            )
            .keys(
                "w",
                "Open the worktree action menu for the selected repo / worktree / mux (new, merge, remove, close-down, prune, reveal; mutations need a backend)",
            )
            .keys(
                "X",
                "Close down the selected stream: merge or discard the branch, end its sessions, remove the worktree, forget its history",
            )
            .keys(
                "v",
                "Open the selected session's transcript (or the mux row's linked session); q/Esc close, j/k or PgDn/PgUp scroll, g/G start/end, t cycle tool detail, T thinking, / find then n/N step through matches, ? lists every key",
            )
            .keys("r", "Refresh discovery now")
            .keys("Ctrl-R", crate::tui::keybindings::HARD_REFRESH_HELP)
            .keys(
                "S",
                "Resume the selected un-muxed agent session in a new terminal",
            )
            .keys("q / Ctrl-C", "Quit"),
        HelpSection::new("Mux")
            .keys(
                "n",
                "Create a bare tmux session (no agent, no worktree) — shell in the selected cwd",
            )
            .keys(
                "N",
                "Launch a harness in a new mux (the form can also create a worktree for it)",
            )
            .keys(
                "m",
                "Open the Mux action menu — pick between new bare session and launching a harness in a fresh mux",
            ),
        HelpSection::new("Row actions")
            .keys(
                "p",
                "Pin the selected row, or unpin it when it is already pinned; pinned rows carry ◉ (◎ when the pinned entity isn't running)",
            )
            .keys(
                crate::tui::keybindings::UNDO_KEY.to_string(),
                "Undo the last pin or unpin, as far back as this session goes; a removed pin comes back with its overrides and its place in the order",
            )
            .keys(
                ".",
                "Open the row action menu — pin or unpin; rebuild, forget, or show the rebuild plan for a history record; edit, rename, retarget, refresh, or reveal a pin in the Pinned view; Clean up… in the History view",
            ),
        HelpSection::new("View switching")
            .keys(
                "1 / 2 / 3 / 4",
                "Switch directly to the sessions / mux / PRs / pinned view",
            )
            .keys("5", "Switch directly to the history view")
            .keys("6", "Switch directly to the Search view")
            .keys("] / [", "Cycle to next / previous view"),
        HelpSection::new("Filters & grouping")
            .keys("F", "Clear all active filters for the visible view")
            .keys("Ctrl-G", "Cycle grouping forward for the active view"),
        HelpSection::new("Search")
            .keys(
                "/",
                "Quick select: find a row of this view by name (rows under collapsed groups too); Enter selects it",
            )
            .keys(
                "/ then Tab",
                "Find anything, whichever view it lives in (Tab again goes back to this view); the word at the right says whether Enter selects it here, follows its relationships in the right pane, or switches views",
            )
            .keys(
                "Ctrl-/ (or Ctrl-_ / Ctrl-7) / s",
                "Search contents: what was said in the sessions and what is in mux scrollback, in this view's sessions and muxes (Tab searches all of them). S-Tab switches between listing sessions and listing messages; Ctrl-O opens the options (tool calls, thinking, pane scrollback); Ctrl-S keeps the search in the Search view and goes there; Ctrl-F keeps the query as a filter on the lists. `s` works in every terminal",
            )
            .keys(
                "< / >",
                "Search view: show the previous (older) or next (newer) remembered search without searching again; Controls lists them",
            )
            .keys(
                "Ctrl-L",
                "Search view: list sessions or messages (the same matches, no new scan)",
            )
            .keys(
                "Ctrl-O",
                "Search view: choose what a search looks in (tool calls, thinking, pane scrollback)",
            )
            .keys(
                "Ctrl-F",
                "Search view: keep the search shown as the filter on the lists (paging does not change the filter; F clears it)",
            ),
        HelpSection::new("Navigation")
            .keys("j / k / ↓ / ↑", "Move selection down / up")
            .keys(
                "l / → / h / ←",
                "Expand / collapse the selected left-tree row (vi-style fold)",
            )
            .keys("PgDn / PgUp", "Page through the row tree")
            .keys("g / G", "First / last row")
            .keys(
                "Enter",
                "Left tree: row-kind default action (attach / view / rebuild / expand); right pane: drill or expand a group",
            )
            .keys(
                "Shift-Enter / Alt-Enter / O",
                "Reveal the row's entity in its own view (Sessions, Mux, PRs, History); Backspace on the left tree returns. Shift-Enter needs a terminal that reports it",
            )
            .keys("Tab", "Cycle focus between left tree and right panel")
            .keys("J / K", "Scroll the right-panel preview"),
        HelpSection::new("Detail-pane graph explorer (right focus)")
            .keys(
                "j / k",
                "Move the explorer cursor between Node fields and relationship rows",
            )
            .keys(
                "Enter",
                "Copy the value on a Node-zone field row · drill on a link row · expand on a group header",
            )
            .keys("e", "Toggle expand/collapse on a multi-link group header")
            .keys(
                "Backspace",
                "Back out of the most recent drilldown hop · once the stack is empty, press twice to return focus to the left pane",
            )
            .keys(
                "F",
                "Toggle Expanded Node Detail (every per-kind field) on the focused node",
            )
            .keys(
                "E",
                "Toggle edge meta (provenance · confidence · state) on link rows",
            )
            .keys(
                "o",
                "Open the full untruncated value for the cursor row in a modal",
            ),
    ]
}

/// Built-in legend mapping each `NodeKind` glyph to its
/// human-readable name. Operators learn the symbol vocabulary by
/// pressing `?` instead of reading the docs. Mirrors ADR 0073's
/// canonical display order (`NodeKind::ALL`) so the icon column
/// down-reads the same sequence the row tree and detail pane use.
fn icon_legend(theme: &Theme) -> HelpSection {
    NodeKind::ALL
        .into_iter()
        .fold(HelpSection::new("Node kind icons"), |section, kind| {
            // A PR's glyph takes the `pr_open` hue: the legend carries no
            // PR state.
            let style = node_kind_style(kind, theme);
            section.legend(
                Span::styled(style.glyph, Style::default().fg(style.color)),
                node_kind_display_name(kind),
                node_kind_help_blurb(kind),
            )
        })
}

/// Legend for the glyphs in the row tree's attach-state column plus the
/// ambiguity warning on group rows and the header. The same glyph can
/// answer a different question on a session row than on a mux row, so
/// the blurb names both where they differ.
fn mux_state_legend(theme: &Theme) -> HelpSection {
    use crate::tui::icons::state;
    let bold_warning = Style::default()
        .fg(theme.warning)
        .add_modifier(Modifier::BOLD);
    let ghost = Style::default()
        .fg(theme.inert)
        .add_modifier(Modifier::ITALIC);
    let muted = Style::default().add_modifier(theme.muted);
    HelpSection::new("Link and mux state")
        .legend(
            Span::styled(state::LINKED, Style::default().fg(theme.mux_attached())),
            "Linked",
            "session: one mux, Enter/a attaches · mux: client attached",
        )
        .legend(
            Span::styled(state::UNLINKED, muted),
            "Not linked",
            "session: no mux, or several candidates · mux: no client",
        )
        .legend(
            Span::styled(state::STOPPED, ghost),
            "Stopped",
            "a mux that is no longer running; Enter starts it again",
        )
        .legend(
            Span::styled(state::UNKNOWN, muted),
            "Unknown",
            "the mux backend didn't report whether a client is attached",
        )
        .legend(
            Span::styled(marks::WARNING, bold_warning),
            "Ambiguous mux",
            "sessions with several candidate muxes; the detail pane lists them",
        )
}

/// Legend for the pull request state glyphs. The Pinned and History
/// views spell the same states as words instead.
fn pr_state_legend(theme: &Theme) -> HelpSection {
    use crate::tui::rows::PrStatus;
    [
        (PrStatus::Open, "Open", "ready for review"),
        (PrStatus::Draft, "Draft", "open, but marked as a draft"),
        (PrStatus::Merged, "Merged", "landed on its base branch"),
        (PrStatus::Closed, "Closed", "closed without merging"),
    ]
    .into_iter()
    .fold(
        HelpSection::new("Pull request state"),
        |section, (status, name, blurb)| {
            let (glyph, style) = status.mark(theme);
            section.legend(Span::styled(glyph, style), name, blurb)
        },
    )
}

/// Legend for the pin marker: filled for something a pin points at,
/// hollow and in the ghost style for a pin with nothing running.
fn pin_marker_legend(theme: &Theme) -> HelpSection {
    use crate::tui::icons::state;
    let pin_color = node_kind_style(NodeKind::Pin, theme).color;
    HelpSection::new("Pins")
        .legend(
            Span::styled(state::PIN_RUNNING, Style::default().fg(pin_color)),
            "Pinned",
            "a pin points at this row and it is running; the Pinned view lists every pin",
        )
        .legend(
            Span::styled(
                state::PIN_STOPPED,
                Style::default()
                    .fg(theme.inert)
                    .add_modifier(Modifier::ITALIC),
            ),
            "Pinned, not running",
            "the pinned entity is stopped, removed, missing, or not started yet; Enter starts it",
        )
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

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
