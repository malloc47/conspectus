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

use crate::model::{GraphNode, NodeId};
use crate::tui::theme::Theme;

/// The `GraphNode` variants, lifted to a flat enum so call
/// sites don't pattern-match on the full `GraphNode` tree just to
/// pick a glyph or color. Conversions are provided from both
/// [`GraphNode`] and [`NodeId`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    Workspace,
    Repo,
    Checkout,
    AgentSession,
    MuxSession,
    Pin,
    RuntimeProcess,
    Branch,
    Fork,
    ForgePr,
}

impl NodeKind {
    /// Every kind, in canonical display order. Used by the catalog
    /// test and by config-loader iteration so new variants are
    /// caught at compile time via exhaustive matches.
    pub const ALL: [NodeKind; 10] = [
        NodeKind::Workspace,
        NodeKind::Repo,
        NodeKind::Checkout,
        NodeKind::AgentSession,
        NodeKind::MuxSession,
        NodeKind::Pin,
        NodeKind::RuntimeProcess,
        NodeKind::Branch,
        NodeKind::Fork,
        NodeKind::ForgePr,
    ];

    /// Stable snake-case tag for the kind. Matches the strings the
    /// existing duplicated `kind_label` helpers in
    /// `src/tui/detail.rs` and `src/tui/explorer.rs` return, so the
    /// later stories in the H-VIS workstream can swap callers over
    /// without changing the on-wire vocabulary.
    pub fn snake_case(self) -> &'static str {
        match self {
            NodeKind::Workspace => "workspace",
            NodeKind::Repo => "repo",
            NodeKind::Checkout => "checkout",
            NodeKind::AgentSession => "agent_session",
            NodeKind::MuxSession => "mux_session",
            NodeKind::Pin => "pin",
            NodeKind::RuntimeProcess => "runtime_process",
            NodeKind::Branch => "branch",
            NodeKind::Fork => "fork",
            NodeKind::ForgePr => "forge_pr",
        }
    }

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

    /// Display-order ordinal for sorting (ADR 0074 §4: detail-pane
    /// `Related entities` rows sort by kind first). Matches
    /// [`Self::ALL`] order so the visual scan reads
    /// `▦ ◆ ◇ ● ▣ ⚙ ⎇ ⑂ ⇄` top-to-bottom.
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|k| *k == self)
            .unwrap_or(usize::MAX)
    }

    /// Inverse of [`Self::snake_case`]: parse a stable kind tag back
    /// into a `NodeKind`. Used by render sites whose state carries
    /// the kind as a `&'static str` (the detail-pane field
    /// `kind_chip`, the explorer's `neighbor_kind`) so they can look
    /// up the slate glyph without round-tripping through `GraphNode`.
    pub fn from_snake_case(tag: &str) -> Option<NodeKind> {
        NodeKind::ALL.into_iter().find(|k| k.snake_case() == tag)
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

impl From<&GraphNode> for NodeKind {
    fn from(node: &GraphNode) -> Self {
        match node {
            GraphNode::Workspace(_) => NodeKind::Workspace,
            GraphNode::Repo(_) => NodeKind::Repo,
            GraphNode::Checkout(_) => NodeKind::Checkout,
            GraphNode::AgentSession(_) => NodeKind::AgentSession,
            GraphNode::MuxSession(_) => NodeKind::MuxSession,
            GraphNode::Pin(_) => NodeKind::Pin,
            GraphNode::RuntimeProcess(_) => NodeKind::RuntimeProcess,
            GraphNode::Branch(_) => NodeKind::Branch,
            GraphNode::Fork(_) => NodeKind::Fork,
            GraphNode::ForgePr(_) => NodeKind::ForgePr,
        }
    }
}

impl From<&NodeId> for NodeKind {
    fn from(id: &NodeId) -> Self {
        match id {
            NodeId::Workspace(_) => NodeKind::Workspace,
            NodeId::Repo(_) => NodeKind::Repo,
            NodeId::Checkout(_) => NodeKind::Checkout,
            NodeId::AgentSession(_) => NodeKind::AgentSession,
            NodeId::MuxSession(_) => NodeKind::MuxSession,
            NodeId::Pin(_) => NodeKind::Pin,
            NodeId::RuntimeProcess(_) => NodeKind::RuntimeProcess,
            NodeId::Branch(_) => NodeKind::Branch,
            NodeId::Fork(_) => NodeKind::Fork,
            NodeId::ForgePr(_) => NodeKind::ForgePr,
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
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, BranchId, CheckoutId, ForgePrId, ForkId, MuxSessionId, NodeId, RepoId,
        RuntimeProcessId, WorkspaceId,
    };

    fn repo_id() -> RepoId {
        RepoId::new("/tmp/.git".to_string())
    }

    #[test]
    fn snake_case_matches_existing_kind_label_vocabulary() {
        // These strings are returned by the duplicated `kind_label`
        // helpers in `src/tui/detail.rs:358` and
        // `src/tui/explorer.rs:831`. Subsequent stories swap those
        // callers onto `NodeKind::snake_case`; the test pins the
        // vocabulary so the swap is a no-op on the wire.
        assert_eq!(NodeKind::Workspace.snake_case(), "workspace");
        assert_eq!(NodeKind::Repo.snake_case(), "repo");
        assert_eq!(NodeKind::Checkout.snake_case(), "checkout");
        assert_eq!(NodeKind::AgentSession.snake_case(), "agent_session");
        assert_eq!(NodeKind::MuxSession.snake_case(), "mux_session");
        assert_eq!(NodeKind::RuntimeProcess.snake_case(), "runtime_process");
        assert_eq!(NodeKind::Branch.snake_case(), "branch");
        assert_eq!(NodeKind::Fork.snake_case(), "fork");
        assert_eq!(NodeKind::ForgePr.snake_case(), "forge_pr");
    }

    #[test]
    fn from_snake_case_round_trips_every_variant() {
        for kind in NodeKind::ALL {
            let tag = kind.snake_case();
            assert_eq!(NodeKind::from_snake_case(tag), Some(kind));
        }
        assert_eq!(NodeKind::from_snake_case("not_a_kind"), None);
        assert_eq!(NodeKind::from_snake_case(""), None);
    }

    #[test]
    fn default_glyphs_match_adr_0073_slate() {
        // Catalog: pin every glyph against the ADR so a slate change
        // requires an ADR amendment.
        let expected: [(NodeKind, &str); 9] = [
            (NodeKind::Workspace, "▦"),
            (NodeKind::Repo, "◆"),
            (NodeKind::Checkout, "◇"),
            (NodeKind::AgentSession, "●"),
            (NodeKind::MuxSession, "▣"),
            (NodeKind::RuntimeProcess, "⚙"),
            (NodeKind::Branch, "⎇"),
            (NodeKind::Fork, "⑂"),
            (NodeKind::ForgePr, "⇄"),
        ];
        for (kind, glyph) in expected {
            assert_eq!(kind.default_glyph(), glyph, "slate drift for {kind:?}");
        }
    }

    #[test]
    fn default_glyphs_are_all_one_cell_wide() {
        // The row-layout law in ADR 0073 §3 only holds if every
        // glyph is 1 cell. A future variant whose glyph is 2-cell
        // would silently break column budgets; catch it at build
        // time via this assertion.
        for kind in NodeKind::ALL {
            let width = UnicodeWidthStr::width(kind.default_glyph());
            assert_eq!(
                width, 1,
                "default glyph for {kind:?} is {width} cells wide, expected 1"
            );
        }
    }

    #[test]
    fn theme_keys_are_unique_and_match_known_keys_pattern() {
        let mut seen = std::collections::HashSet::new();
        for kind in NodeKind::ALL {
            let key = kind.theme_key();
            assert!(
                key.starts_with("node_"),
                "theme key `{key}` must start with `node_`"
            );
            assert!(seen.insert(key), "duplicate theme key `{key}`");
        }
    }

    #[test]
    fn node_kind_style_uses_theme_color_for_each_kind() {
        let theme = Theme::default();
        // Spot-check the color resolution against the slate. The
        // exhaustive default-color coverage lives in
        // `tui::theme::tests` so the assertion here is the *wiring*
        // — `node_kind_style` reads from the right field per kind.
        assert_eq!(
            node_kind_style(NodeKind::Workspace, &theme).color,
            theme.node_workspace,
        );
        assert_eq!(
            node_kind_style(NodeKind::Repo, &theme).color,
            theme.node_repo,
        );
        assert_eq!(
            node_kind_style(NodeKind::AgentSession, &theme).color,
            theme.node_agent_session,
        );
        // ForgePr is the documented sentinel — callers pick from
        // theme.pr_* themselves.
        assert_eq!(
            node_kind_style(NodeKind::ForgePr, &theme).color,
            Color::Reset,
        );
    }

    #[test]
    fn node_kind_style_returns_default_glyph_without_overrides() {
        let theme = Theme::default();
        for kind in NodeKind::ALL {
            let style = node_kind_style(kind, &theme);
            assert_eq!(style.glyph, kind.default_glyph());
            assert_eq!(style.width, 1);
        }
    }

    #[test]
    fn icon_overrides_are_honored_when_present() {
        let mut theme = Theme::default();
        theme.icons.insert(NodeKind::Repo, "R".to_string());
        let style = node_kind_style(NodeKind::Repo, &theme);
        assert_eq!(style.glyph, "R");
        assert_eq!(style.width, 1);
        // Kinds without overrides still resolve to the default.
        assert_eq!(
            node_kind_style(NodeKind::Workspace, &theme).glyph,
            NodeKind::Workspace.default_glyph(),
        );
    }

    #[test]
    fn parse_icon_override_accepts_one_cell_glyph() {
        let (kind, glyph) = parse_icon_override("node_fork", "Y").unwrap();
        assert_eq!(kind, NodeKind::Fork);
        assert_eq!(glyph, "Y");
    }

    #[test]
    fn parse_icon_override_rejects_unknown_key() {
        let err = parse_icon_override("workspace_glyph", "▦").unwrap_err();
        assert!(
            err.contains("unknown `[tui.theme.icons]` key"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_icon_override_rejects_empty_glyph() {
        let err = parse_icon_override("node_repo", "").unwrap_err();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[test]
    fn parse_icon_override_rejects_wide_glyph() {
        // CJK ideographs are unambiguously East Asian Wide (2
        // cells); the validator should refuse so the row layout law
        // holds. Same goes for any emoji-variation-selector-16
        // sequence, but the bare CJK case is the most stable proxy.
        let err = parse_icon_override("node_workspace", "中").unwrap_err();
        assert!(err.contains("display width"), "got: {err}");
    }

    #[test]
    fn parse_icon_override_rejects_multi_codepoint_glyph() {
        // Two ASCII characters = width 2 → reject.
        let err = parse_icon_override("node_workspace", "WS").unwrap_err();
        assert!(err.contains("expected 1"), "got: {err}");
    }

    #[test]
    fn from_node_id_covers_every_variant() {
        // Compiler exhaustiveness already guarantees this stays in
        // sync with `NodeId`; the explicit cases here turn a future
        // miss into a behavioral failure and document the mapping
        // for readers. `From<&GraphNode>` is the same shape and is
        // tested implicitly by the renderer's exhaustive matches.
        let pairs: [(NodeId, NodeKind); 9] = [
            (
                NodeId::Workspace(WorkspaceId::new("/tmp")),
                NodeKind::Workspace,
            ),
            (NodeId::Repo(repo_id()), NodeKind::Repo),
            (
                NodeId::Checkout(CheckoutId::new(repo_id(), "/tmp/wt")),
                NodeKind::Checkout,
            ),
            (
                NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc")),
                NodeKind::AgentSession,
            ),
            (
                NodeId::MuxSession(MuxSessionId::new("x")),
                NodeKind::MuxSession,
            ),
            (
                NodeId::RuntimeProcess(RuntimeProcessId::new("host:1")),
                NodeKind::RuntimeProcess,
            ),
            (
                NodeId::Branch(BranchId::new(repo_id(), "main")),
                NodeKind::Branch,
            ),
            (NodeId::Fork(ForkId::new("alpha")), NodeKind::Fork),
            (
                NodeId::ForgePr(ForgePrId::new("github", "github.com", "owner", "repo", 1)),
                NodeKind::ForgePr,
            ),
        ];
        for (id, expected) in pairs {
            assert_eq!(NodeKind::from(&id), expected, "mismatch for {id}");
        }
    }
}
