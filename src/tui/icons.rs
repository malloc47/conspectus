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
//!
//! H-VIS-002 of the per-node-type visual-identity workstream.

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
            NodeKind::Pin => "◉",
            NodeKind::RuntimeProcess => "⚙",
            NodeKind::Branch => "⎇",
            NodeKind::Fork => "⑂",
            NodeKind::ForgePr => "⇄",
        }
    }
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
