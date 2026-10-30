//! `?` help overlay: the keymap's reference card, the framework's help
//! page (`tuikit::help::HelpPage`) read in its pager.
//!
//! The pager owns the reading: scrolling and viewport-sized paging, `g`
//! and `G` (which keeps to the last page), the scrollbar, `Esc`, `q`, and
//! `?` (the key that opened it) closing, and the footer generated from
//! those keys. The viewers' pages are the same page generated from their
//! key tables, with the footer of a viewer's help level (where `q` leaves
//! the viewer), so the three share one frame, one column grid, and one
//! heading style. This one is generated from the program's keymap: a row
//! per binding with its help text, under the section the binding names.
//! What the keymap cannot say is an annotation ([`annotations`]): the keys
//! of the overlay a key opens, a key sequence the keymap does not bind as
//! one key, and the legends of the marks the screens draw.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::glyphs::marks;
use tuikit::help::{HelpAnnotation, HelpPage, HelpRow, HelpSection, keymap_sections};
use tuikit::pager::Pager;

use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::keybindings::keymap;
use crate::tui::keybindings::overlay::HELP_KEYS;
use crate::tui::{Msg, Theme};

/// The keymap help: the framework's pager over the keymap's page.
pub type KeymapHelp = Pager<HelpPage<Msg>>;

/// The keymap help at the top, its marks in `theme`'s styles, resolving
/// `HELP_KEYS` (the pager's keys and `?`, closing as it opened).
pub fn open(theme: &Theme) -> KeymapHelp {
    HelpPage::new(sections(theme)).pager(HELP_KEYS)
}

/// The keymap page: a row per binding of the program's keymap, then what
/// it cannot say ([`annotations`]).
pub(crate) fn sections(theme: &Theme) -> Vec<HelpSection> {
    keymap_sections(keymap(), &annotations(theme))
}

/// What the keymap page says that the keymap's bindings cannot, each
/// anchored to a binding by its id: the keys of the overlay a key opens
/// (Controls, quick select, the content search, the transcript viewer),
/// which their own tables bind; quick select's `Tab`, a key of the
/// overlay `/` opens, as a row of its own; and the legends of the marks
/// the screens draw.
pub(crate) fn annotations(theme: &Theme) -> Vec<HelpAnnotation> {
    vec![
        HelpAnnotation::more(
            "controls",
            "; Space selects or toggles, Enter applies and closes, Esc closes",
        ),
        HelpAnnotation::more(
            "transcript.view",
            "; q/Esc close, j/k or PgDn/PgUp scroll, g/G start/end, t cycle tool detail, T thinking, / find then n/N step through matches, ? lists every key",
        ),
        HelpAnnotation::more("search.quick", "; Enter selects it"),
        HelpAnnotation::row_after(
            "search.quick",
            HelpRow::keys(
                "/ then Tab",
                "Find anything, whichever view it lives in (Tab again goes back to this view); the word at the right says whether Enter selects it here, follows its relationships in the right pane, or switches views",
            ),
        ),
        HelpAnnotation::more(
            "search.contents",
            " (Tab searches all of them). S-Tab switches between listing sessions and listing messages; Ctrl-O opens the options (tool calls, thinking, pane scrollback); Ctrl-S keeps the search in the Search view and goes there; Ctrl-F keeps the query as a filter on the lists. `s` works in every terminal",
        ),
        HelpAnnotation::section(icon_legend(theme)),
        HelpAnnotation::section(mux_state_legend(theme)),
        HelpAnnotation::section(pr_state_legend(theme)),
        HelpAnnotation::section(pin_marker_legend(theme)),
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
