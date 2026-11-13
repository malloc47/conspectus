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
//! The glyphs every control shares (the list cursor, the disclosure
//! arrows, the check and failure marks, the ellipsis, the no-value dash)
//! are `tuikit::glyphs::marks`; this module keeps only the vocabulary
//! that is Conspectus's own, the node kinds and the [`state`] glyphs.
//! [`register_glyphs`] adds those, and the few other domain glyphs the
//! sources draw, to the framework's vetted-glyph table, which the source
//! scan checks every drawn character against.
//!
//! `ForgePr` is the one kind that does not own a color: its hue is
//! driven by PR state (`pr_open`/`pr_closed`/`pr_merged`/`pr_draft`).
//! Its [`NodeKindStyle::color`] is the `pr_open` hue, which every
//! surface that has no PR state draws it in; callers that know the
//! PR's state take its style from [`crate::tui::rows::PrStatus::mark`].

use std::collections::BTreeMap;

use ratatui::style::Color;
use tuikit::glyphs::{cell_width, marks};

use crate::tui::theme::Theme;

pub use crate::model::NodeKind;

mod vetted;

pub use vetted::{register_glyphs, vetted_glyphs};

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
    /// A mux that has stopped running. Drawn in the ghost look.
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
    /// A pull request with no local branch: the other half of the pair
    /// whose first half is the branch kind glyph on a tracked one.
    pub const UNTRACKED: &str = "◌";
    /// A pin on an entity that is running: the filled pin glyph.
    pub const PIN_RUNNING: &str = "◉";
    /// A pin on an entity that is stopped, removed, missing, or not
    /// started yet: the hollow twin of [`PIN_RUNNING`], drawn in the
    /// ghost look.
    pub const PIN_STOPPED: &str = "◎";

    /// Every state glyph, for tests that check width and uniqueness.
    pub const ALL: [&str; 11] = [
        LINKED,
        UNLINKED,
        STOPPED,
        UNKNOWN,
        PR_OPEN,
        PR_DRAFT,
        PR_MERGED,
        PR_CLOSED,
        UNTRACKED,
        PIN_RUNNING,
        PIN_STOPPED,
    ];
}

/// Every glyph the left panel can draw, once each: the kind glyphs, the
/// state glyphs, and the framework marks it uses. The reference document
/// lists exactly these, and a test keeps the two in step. (The ellipsis
/// and the rule cell are marks too, but the reference describes them in
/// prose; they are not part of a row's vocabulary.)
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
/// For [`NodeKind::ForgePr`] the `color` field is the `pr_open` hue; a
/// caller that knows the PR's state takes the style from
/// [`crate::tui::rows::PrStatus::mark`].
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
    let width = cell_width(&glyph);
    let color = theme_color(kind, theme);
    NodeKindStyle {
        glyph,
        color,
        width,
    }
}

/// Default color for `kind`: its own `node_*` theme role, or for
/// `ForgePr`, which has none, the `pr_open` hue a PR glyph without a
/// state takes.
fn theme_color(kind: NodeKind, theme: &Theme) -> Color {
    match kind {
        NodeKind::Workspace => theme.node_workspace(),
        NodeKind::Repo => theme.node_repo(),
        NodeKind::Checkout => theme.node_checkout(),
        NodeKind::AgentSession => theme.node_agent_session(),
        NodeKind::MuxSession => theme.node_mux_session(),
        NodeKind::Pin => theme.node_mux_session(),
        NodeKind::RuntimeProcess => theme.node_runtime_process(),
        NodeKind::Branch => theme.node_branch(),
        NodeKind::Fork => theme.node_fork(),
        NodeKind::ForgePr => theme.pr_open(),
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

    let width = cell_width(raw);
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
