//! Per-node-kind visual identity (ADR 0073).
//!
//! Every `GraphNode` / `NodeId` variant maps to a 1-cell glyph and a
//! color drawn from [`Theme`]. The default slate is the geometric
//! vocabulary defined in ADR 0073; operators override individual
//! glyphs through the `[tui.theme.icons]` config table, parsed by
//! [`parse_icon_override`] and stored on [`Theme::icons`].
//!
//! The single lookup entry point is [`node_kind_style`]; render
//! sites should not reach into the override map or the per-kind
//! constants directly.
//!
//! `ForgePr` is the one kind that does not own a color: its hue is
//! driven by PR state (`pr_open`/`pr_closed`/`pr_merged`/`pr_draft`),
//! so the [`NodeKindStyle::color`] field is the sentinel
//! [`Color::Reset`] for `ForgePr`. Callers that render a PR row pick
//! the color from the appropriate `theme.pr_*` field directly.

use std::collections::BTreeMap;

use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

use crate::tui::theme::Theme;

pub use crate::model::NodeKind;

/// TUI presentation of each kind (ADR 0073).
impl NodeKind {
    /// Theme-key suffix used in `[tui.theme.icons]` and for the
    /// matching `node_*` color field on [`Theme`]. The icons table
    /// key is the same string (e.g. `node_repo = "◆"`), so the
    /// config loader looks up overrides by this name.
    pub fn theme_key(self) -> &'static str {
        match self {
            NodeKind::Workspace => "node_workspace",
            NodeKind::Repo => "node_repo",
            NodeKind::Checkout => "node_checkout",
            NodeKind::AgentSession => "node_agent_session",
            NodeKind::MuxSession => "node_mux_session",
            NodeKind::Pin => "node_pin",
            NodeKind::RuntimeProcess => "node_runtime_process",
            NodeKind::Branch => "node_branch",
            NodeKind::Fork => "node_fork",
            NodeKind::ForgePr => "node_forge_pr",
        }
    }

    /// Default glyph from the ADR 0073 slate. Operators override
    /// this via `[tui.theme.icons]`; the override flow runs through
    /// [`node_kind_style`].
    pub fn default_glyph(self) -> &'static str {
        match self {
            NodeKind::Workspace => "▦",
            NodeKind::Repo => "◆",
            NodeKind::Checkout => "◇",
            NodeKind::AgentSession => "●",
            NodeKind::MuxSession => "▣",
            NodeKind::Pin => state::PIN_RUNNING,
            NodeKind::RuntimeProcess => "◈",
            NodeKind::Branch => "⌥",
            NodeKind::Fork => "⋎",
            NodeKind::ForgePr => "⇄",
        }
    }
}

/// State glyphs: the one-cell marks a row uses for a state that has two
/// or more values, in views of a single primary entity (sessions, muxes,
/// pull requests). Every state is drawn as a pair or set of distinct
/// shapes, so no state hides behind the absence of a glyph and none
/// relies on color alone. Mixed views (Pinned, History) spell the same
/// states as words instead.
pub mod state {
    /// A session linked to a definitive mux, or a mux with a client
    /// attached. Drawn in the attached color.
    pub const LINKED: &str = "⊸";
    /// A session with no definitive mux (none or ambiguous), or a mux
    /// with no client attached. Drawn dim.
    pub const UNLINKED: &str = "∘";
    /// A mux that has stopped running. Drawn in the ghost color.
    pub const STOPPED: &str = "⊘";
    /// A state that couldn't be checked: a mux whose server didn't
    /// answer, or a pull request with no state yet.
    pub const UNKNOWN: &str = "?";
    /// An open pull request.
    pub const PR_OPEN: &str = "○";
    /// An open pull request still marked draft.
    pub const PR_DRAFT: &str = "◐";
    /// A merged pull request.
    pub const PR_MERGED: &str = "⊕";
    /// A closed, unmerged pull request.
    pub const PR_CLOSED: &str = "⊗";
    /// A pin on an entity that is running: the filled pin glyph.
    pub const PIN_RUNNING: &str = "◉";
    /// A pin on an entity that is stopped, removed, missing, or not
    /// started yet: the hollow twin of [`PIN_RUNNING`], drawn in the
    /// ghost color.
    pub const PIN_STOPPED: &str = "◎";

    /// Every state glyph, for tests that check width and uniqueness.
    pub const ALL: [&str; 10] = [
        LINKED,
        UNLINKED,
        STOPPED,
        UNKNOWN,
        PR_OPEN,
        PR_DRAFT,
        PR_MERGED,
        PR_CLOSED,
        PIN_RUNNING,
        PIN_STOPPED,
    ];
}

/// The other one-cell marks the left panel draws: disclosure, the group
/// warning, the PR tracking marker, the missing-value dash, and the
/// spinner that stands in for a state while a refresh is pending.
pub mod marks {
    /// A group or row that is expanded.
    pub const EXPANDED: &str = "▼";
    /// A group or row that is collapsed.
    pub const COLLAPSED: &str = "▶";
    /// A group with a session whose mux link is ambiguous.
    pub const WARNING: &str = "▲";
    /// A pull request with no local branch.
    pub const UNTRACKED: &str = "◌";
    /// An age or value that isn't known.
    pub const NO_VALUE: &str = "—";
    /// A path or value that checked out, in forms.
    pub const OK: &str = "√";
    /// A check or value that failed.
    pub const FAIL: &str = "×";
    /// An unchecked item in a multi-select list.
    pub const BULLET: &str = "•";
    /// The row cursor in menus and the "points back" mark in headers.
    pub const POINTER: &str = "►";
    /// The mark for "go back" in the right pane's breadcrumb.
    pub const BACK: &str = "◄";
    /// The frames of the pending-refresh spinner.
    pub const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
}

/// Every glyph the left panel can draw, once each: the kind glyphs, the
/// state glyphs, and the marks. The reference document lists exactly
/// these, and a test keeps the two in step.
pub fn all_glyphs() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let mut push = |glyph: &'static str| {
        if !out.contains(&glyph) {
            out.push(glyph);
        }
    };
    for kind in NodeKind::ALL {
        push(kind.default_glyph());
    }
    for glyph in state::ALL {
        push(glyph);
    }
    for glyph in [
        marks::EXPANDED,
        marks::COLLAPSED,
        marks::WARNING,
        marks::UNTRACKED,
        marks::NO_VALUE,
        marks::OK,
        marks::BULLET,
        marks::FAIL,
        marks::POINTER,
        marks::BACK,
    ] {
        push(glyph);
    }
    for glyph in marks::SPINNER {
        push(glyph);
    }
    out
}

/// Resolved visual identity for one node kind: the glyph string
/// (default or operator override), the color drawn from the theme,
/// and the display-cell width of the glyph (always 1; carried so
/// callers can subtract from column budgets without recomputing).
///
/// For [`NodeKind::ForgePr`] the `color` field is the sentinel
/// [`Color::Reset`]; the caller picks `theme.pr_open` /
/// `theme.pr_closed` / `theme.pr_merged` / `theme.pr_draft` based on
/// PR state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeKindStyle {
    pub glyph: String,
    pub color: Color,
    pub width: usize,
}

/// Look up the visual identity for `kind` against the supplied
/// theme. Honors any operator override in `theme.icons`; falls back
/// to the ADR 0073 slate when no override is present.
pub fn node_kind_style(kind: NodeKind, theme: &Theme) -> NodeKindStyle {
    let glyph = theme
        .icons
        .glyph(kind)
        .unwrap_or_else(|| kind.default_glyph().to_string());
    let width = UnicodeWidthStr::width(glyph.as_str());
    let color = theme_color(kind, theme);
    NodeKindStyle {
        glyph,
        color,
        width,
    }
}

/// Default color for `kind`. `ForgePr` returns [`Color::Reset`] as
/// the documented sentinel; every other kind reads from its own
/// `node_*` theme field.
fn theme_color(kind: NodeKind, theme: &Theme) -> Color {
    match kind {
        NodeKind::Workspace => theme.node_workspace,
        NodeKind::Repo => theme.node_repo,
        NodeKind::Checkout => theme.node_checkout,
        NodeKind::AgentSession => theme.node_agent_session,
        NodeKind::MuxSession => theme.node_mux_session,
        NodeKind::Pin => theme.node_mux_session,
        NodeKind::RuntimeProcess => theme.node_runtime_process,
        NodeKind::Branch => theme.node_branch,
        NodeKind::Fork => theme.node_fork,
        NodeKind::ForgePr => Color::Reset,
    }
}

/// Operator overrides for individual node-kind glyphs, parsed from
/// the `[tui.theme.icons]` config table. Empty by default; lookups
/// fall through to [`NodeKind::default_glyph`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IconOverrides {
    entries: BTreeMap<NodeKind, String>,
}

impl IconOverrides {
    /// Return the override glyph for `kind`, if any.
    pub fn glyph(&self, kind: NodeKind) -> Option<String> {
        self.entries.get(&kind).cloned()
    }

    /// Insert a single validated override. Used by the config
    /// loader after [`parse_icon_override`] succeeds.
    pub fn insert(&mut self, kind: NodeKind, glyph: String) {
        self.entries.insert(kind, glyph);
    }

    /// Number of override entries. Mostly useful for tests.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parse a single `[tui.theme.icons].<key> = "<glyph>"` entry.
///
/// Validates two things:
///
/// 1. The key matches a `NodeKind::theme_key()` (case-sensitive).
/// 2. The glyph's display-cell width is exactly 1, so row layout
///    arithmetic stays sound regardless of override (ADR 0073 §4).
///
/// Returns the resolved `(NodeKind, glyph)` pair on success or a
/// human-readable error string the config loader wraps into a
/// `ConfigDiagnostic`.
pub fn parse_icon_override(key: &str, raw: &str) -> Result<(NodeKind, String), String> {
    let kind = NodeKind::ALL
        .iter()
        .copied()
        .find(|k| k.theme_key() == key)
        .ok_or_else(|| {
            format!(
                "unknown `[tui.theme.icons]` key `{key}` \
                 (expected one of node_workspace, node_repo, node_checkout, \
                 node_agent_session, node_mux_session, node_runtime_process, \
                 node_branch, node_fork, node_forge_pr)"
            )
        })?;

    if raw.is_empty() {
        return Err(format!(
            "`[tui.theme.icons].{key}`: glyph must not be empty"
        ));
    }

    let width = UnicodeWidthStr::width(raw);
    if width != 1 {
        return Err(format!(
            "`[tui.theme.icons].{key}`: glyph `{raw}` has display width {width}, expected 1"
        ));
    }

    Ok((kind, raw.to_string()))
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;

/// Glyphs that have been checked to draw inside one terminal cell, with
/// how. A glyph the TUI draws that is missing here fails
/// `all_glyphs_are_vetted`.
///
/// A terminal falls back to a proportional font for a glyph its monospace
/// font lacks, and that glyph then overflows its cell (and a selection
/// highlight cuts it in half). So every glyph here is in DejaVu Sans Mono
/// at exactly one cell advance, in Latin-1, or in the WGL4 set that
/// Liberation Mono, Courier New and Andale Mono also carry. Measure a new
/// glyph with `scripts/glyph-widths.py` and record it here.
///
/// `ambiguous` marks glyphs whose East Asian width is ambiguous: one cell
/// in Western locales, two in some CJK ones, as every Geometric Shapes
/// glyph is. Wide and emoji-presentation glyphs are not allowed at all.
pub const VETTED: &[(&str, &str)] = &[
    (
        "⠋",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠙",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠹",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠸",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠼",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠴",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠦",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    (
        "⠧",
        "Braille Patterns; kept by operator preference; some fonts draw it past one cell",
    ),
    ("▦", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    ("◆", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    ("◇", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    (
        "●",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    ("▣", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    ("◈", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    ("⌥", "Miscellaneous Technical; DejaVu Sans Mono 1.00"),
    ("⋎", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("⇄", "Arrows; DejaVu Sans Mono 1.00"),
    ("⊸", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("∘", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("⊘", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("?", "ASCII"),
    (
        "○",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        "◐",
        "Geometric Shapes; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    ("⊕", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("⊗", "Mathematical Operators; DejaVu Sans Mono 1.00"),
    ("◉", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    ("◎", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
    (
        "▼",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        "►",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        "▶",
        "Geometric Shapes; kept by operator preference; a terminal with an emoji font may draw it wide",
    ),
    (
        "◄",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        "▲",
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        "◌",
        "Geometric Shapes; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    ("—", "Latin-1 range punctuation; every font; ambiguous"),
    (
        "√",
        "Mathematical Operators; DejaVu Sans Mono and Liberation Mono 1.00",
    ),
    ("×", "Latin-1; every font"),
    // Punctuation, arrows, and line drawing the interface text uses.
    ("·", "Latin-1; every font; ambiguous"),
    ("•", "WGL4; every font; ambiguous"),
    ("…", "WGL4; every font; ambiguous"),
    ("›", "WGL4; every font"),
    ("é", "Latin-1; every font"),
    ("§", "Latin-1; every font; ambiguous"),
    ("←", "WGL4 arrows; every font; ambiguous"),
    ("↑", "WGL4 arrows; every font; ambiguous"),
    ("→", "WGL4 arrows; every font; ambiguous"),
    ("↓", "WGL4 arrows; every font; ambiguous"),
    (
        "─",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "│",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "┃",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "║",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "┌",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "┐",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "└",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "┘",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╭",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╮",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╰",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╯",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╔",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╗",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╚",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
    (
        "╝",
        "Box Drawing; drawn by the terminal itself in most; ambiguous",
    ),
];

/// Why `glyph` must not be drawn in a one-cell slot, if it can't be: it
/// is wide, can take emoji presentation, or needs a joiner or variation
/// selector to draw.
pub fn glyph_risk(glyph: &str) -> Option<&'static str> {
    if UnicodeWidthStr::width(glyph) != 1 || glyph.chars().count() != 1 {
        return Some("not exactly one cell wide, or more than one character");
    }
    let cp = glyph.chars().next().map_or(0, |c| c as u32);
    match cp {
        0x200D | 0xFE00..=0xFE0F => Some("joiner or variation selector"),
        0x1F000..=0x1FFFF => Some("emoji block"),
        // Text-presentation characters that terminals commonly draw as
        // color emoji, wide, when an emoji font is installed.
        // The collapsed-tree arrow stays `▶` by operator preference.
        0x25B6 => None,
        _ if EMOJI_CAPABLE.contains(&cp) => Some("can take emoji presentation"),
        _ => None,
    }
}

/// Characters that have an emoji presentation and so may draw wide:
/// a deny-list of the ones near the TUI's vocabulary.
const EMOJI_CAPABLE: &[u32] = &[
    0x25B6, 0x25C0, 0x25AA, 0x25AB, 0x25FB, 0x25FC, 0x25FD, 0x25FE, 0x2699, 0x26A0, 0x2714, 0x2716,
    0x2764, 0x2611, 0x2B05, 0x2B06, 0x2B07, 0x2B1B, 0x2B1C, 0x2B50, 0x2B55, 0x231A, 0x231B, 0x23E9,
    0x23EA, 0x23F0, 0x23F3, 0x2705, 0x274C, 0x2753, 0x2757, 0x26D3, 0x2139,
];
