//! Pure row-tree view-models for the TUI left panel.
//!
//! Builders are deterministic functions of `(GraphSnapshot, RunConfig,
//! now)` → [`RowTree`]. They contain no ratatui types and no I/O so
//! they snapshot-test without a runtime. The TUI's renderer
//! ([`crate::tui::ui`]) and the CLI's table surface
//! ([`crate::output::table`]) can both consume them as the table-
//! parity work in later stories lands.
//!
//! Per ADR 0024 and the locked decisions in
//! `docs/tui-sessions-mockup.md`:
//!
//! - Sessions view collapses the worktree level when a project has a
//!   single worktree.
//! - Sessions with ≥ 2 `LinkedToMux` candidates expose an expandable
//!   per-candidate sub-tree; the resolver-preferred candidate is
//!   marked.
//! - The per-row activity indicator is intentionally absent.
//! - Path rendering uses `~` shortening for `$HOME`.
//!
//! v1 only implements the sessions view in this commit; mux, union,
//! prs, and forks land in follow-on commits per `P8-004`.

use std::path::Path;

use crate::model::{AgentSessionId, MuxSessionId, NodeId};
use crate::tui::View;

pub mod sessions;

pub use sessions::{SessionsBuildInputs, build_sessions_tree};

/// Stable, hashable row identity. Used by the selection state
/// machine in `P8-006` to retain selection across refreshes — two
/// builds of the same view over the same node set produce identical
/// `RowId`s.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RowId {
    /// A group row backed by a node (workspace, repo, worktree, mux,
    /// fork, …). Carries the node id so future operations can resolve
    /// back to `node show`.
    Group(NodeId),
    AgentSession(NodeId),
    /// A candidate-mux child row under an ambiguous agent session.
    /// Keyed on `(parent_session, candidate_mux)` so the row is
    /// distinct from any other agent session that happens to be a
    /// candidate for the same mux.
    AgentSessionMuxCandidate {
        agent: NodeId,
        mux: NodeId,
    },
    MuxSession(NodeId),
    Pr(NodeId),
    Fork(NodeId),
    /// Synthetic row not backed by a single node — used for the
    /// "Ungrouped" bucket and any other rendered-only structure.
    Synthetic(&'static str),
}

/// Built view-model the renderer consumes. Always rendered top-to-
/// bottom, depth-first; the builder is responsible for emitting rows
/// in display order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RowTree {
    pub rows: Vec<Row>,
    /// Which view produced this tree, so renderers and tests can
    /// route accordingly without inspecting the rows themselves.
    pub view: ViewLabel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum ViewLabel {
    #[default]
    Sessions,
    Mux,
    Union,
    Prs,
    Forks,
}

impl From<View> for ViewLabel {
    fn from(view: View) -> Self {
        match view {
            View::Sessions => Self::Sessions,
            View::Mux => Self::Mux,
            View::Union => Self::Union,
            View::Prs => Self::Prs,
            View::Forks => Self::Forks,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: RowId,
    /// Indentation depth, 0 = top-level.
    pub depth: u8,
    /// True when the row owns children — drives the `▶`/`▼`
    /// disclosure glyph.
    pub expandable: bool,
    pub kind: RowKind,
}

#[derive(Clone, Debug, PartialEq)]
// AgentSessionRow carries a handful of optional display strings and
// is the only variant the renderer hot-loops over. Boxing would push
// every match arm through an extra indirection for no measurable win
// because the row tree allocates a single owning `Vec<Row>` and never
// stores RowKind in densely-packed collections.
#[allow(clippy::large_enum_variant)]
pub enum RowKind {
    Group(GroupRow),
    AgentSession(AgentSessionRow),
    AgentSessionMuxCandidate(MuxCandidateRow),
}

/// A workspace / repo / worktree label row.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupRow {
    /// Human-friendly path with `$HOME` shortened to `~`.
    pub display_path: String,
    /// Underlying node id when the group corresponds to a single
    /// node, `None` for synthetic buckets.
    pub primary_node: Option<NodeId>,
    /// True when this group corresponds to the launch-time process
    /// cwd (the longest ancestor of cwd among all group rows). The
    /// renderer surfaces this with a subtle marker so the operator
    /// can tell where they launched from; the runtime also uses it
    /// to pick the initial selection and pre-expand ancestors.
    pub is_launch_context: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentSessionRow {
    pub session: AgentSessionId,
    /// Short content-addressed identifier. Floored at 6 chars per
    /// H-TBL-002; the dispatcher grows the floor only to break
    /// collisions inside the row tree.
    pub short_id: String,
    /// Harness label as rendered in the row (`claude`, `codex`,
    /// `opencode`, …). The full harness key remains in `session`.
    pub harness_label: String,
    /// Working directory recorded by the harness, `~`-shortened.
    pub cwd_display: Option<String>,
    /// Recency tag for the right-aligned column. `2m`, `17m`, `1h`,
    /// `2d`. `None` when no activity timestamp is known.
    pub recency: Option<String>,
    /// Source value for the recency string (Unix epoch seconds).
    /// Kept so renderers and state-machine selection can sort on the
    /// raw value without re-parsing the rendered tag.
    pub activity_epoch: Option<i64>,
    /// Mux relationship state — drives the indicator glyph and the
    /// ambiguous-mux expansion eligibility.
    pub mux_state: MuxIndicator,
    /// Graph-resident `last_message_preview` (ADR 0023), if present.
    /// The renderer decides whether and where to display it.
    pub preview: Option<String>,
    /// Title attribute on the session, if set (opencode chat topics
    /// today). Right panel renders it as a header field; the tree
    /// uses it only after `P8-015` lands.
    pub title: Option<String>,
    /// Operator-chosen display name from the ADR 0029 alias overlay,
    /// when set. Takes precedence over [`Self::title`] at every
    /// projection site via [`Self::display_label`].
    pub alias: Option<String>,
    pub primary_node: NodeId,
}

impl AgentSessionRow {
    /// Apply ADR 0029's `alias > title > id-suffix` precedence and
    /// return the strongest display label available for this row.
    /// Falls back to `None` when neither alias nor title is set so
    /// callers can render the short-id suffix instead.
    pub fn display_label(&self) -> Option<&str> {
        self.alias.as_deref().or(self.title.as_deref())
    }
}

/// Which mux indicator glyph the renderer should draw for an agent
/// session row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MuxIndicator {
    /// `◉`. One resolver-preferred mux candidate, no competitors.
    Attached,
    /// `◐`. ≥ 2 active `LinkedToMux` candidates; the resolver picked
    /// one but alternatives exist. Carries the total candidate count
    /// (preferred + competing) so the status bar and right panel can
    /// surface "N candidates" without re-walking the graph.
    Ambiguous { candidate_count: usize },
    /// `◯`. No mux link at all.
    Unmuxed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MuxCandidateRow {
    pub mux: MuxSessionId,
    /// Display label for the candidate mux, e.g. `tmux:editor`.
    pub mux_label: String,
    /// True when this candidate is the resolver-preferred one for
    /// the parent agent session.
    pub is_preferred: bool,
    pub primary_node: NodeId,
}

// -----------------------------------------------------------------------------
// Shared utilities
// -----------------------------------------------------------------------------

/// Shorten an absolute path by replacing the user's `$HOME` prefix
/// with `~`, leaving any other path untouched. The home directory
/// is passed in so callers can stub it in tests.
pub fn shorten_home(path: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return path.to_string();
    };
    let home_str = home.to_string_lossy();
    if home_str.is_empty() {
        return path.to_string();
    }
    let home_str = home_str.trim_end_matches('/');
    if path == home_str {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(home_str)
        && (rest.starts_with('/') || rest.is_empty())
    {
        return format!("~{rest}");
    }
    path.to_string()
}

/// Translate a harness key to the short label rendered in the row.
/// Today's discovery emits `claude-code`, `codex`, and `opencode`;
/// the operator-facing label collapses `claude-code` to `claude` so
/// the column stays tight per H-TBL-014.
pub fn harness_label(harness_key: &str) -> String {
    match harness_key {
        "claude-code" => "claude".to_string(),
        other => other.to_string(),
    }
}

/// Format an activity epoch as a human-readable recency string
/// (`2m`, `17m`, `1h`, `2d`). `now` is passed explicitly so the
/// function stays time-independent for tests. Returns `None` when
/// either side is missing.
///
/// Rules:
/// - 0..60 seconds → "Ns" (rare, but renders cleanly).
/// - 60s..1h → "Nm".
/// - 1h..24h → "Nh".
/// - >= 24h → "Nd".
/// - Future timestamps (clock skew) clamp to "0s".
pub fn format_recency(now: Option<i64>, activity_epoch: Option<i64>) -> Option<String> {
    let now = now?;
    let then = activity_epoch?;
    let delta = (now - then).max(0);
    if delta < 60 {
        return Some(format!("{delta}s"));
    }
    let minutes = delta / 60;
    if minutes < 60 {
        return Some(format!("{minutes}m"));
    }
    let hours = minutes / 60;
    if hours < 24 {
        return Some(format!("{hours}h"));
    }
    let days = hours / 24;
    Some(format!("{days}d"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn shorten_home_replaces_exact_home_prefix() {
        let home = PathBuf::from("/home/op");
        assert_eq!(shorten_home("/home/op", Some(&home)), "~");
        assert_eq!(shorten_home("/home/op/src/x", Some(&home)), "~/src/x");
    }

    #[test]
    fn shorten_home_keeps_non_home_paths_intact() {
        let home = PathBuf::from("/home/op");
        assert_eq!(shorten_home("/var/log", Some(&home)), "/var/log");
        // Prefix match must respect path boundaries — `/home/operator`
        // is NOT `/home/op/erator`.
        assert_eq!(
            shorten_home("/home/operator", Some(&home)),
            "/home/operator"
        );
    }

    #[test]
    fn shorten_home_with_trailing_slash_in_home_still_matches() {
        let home = PathBuf::from("/home/op/");
        assert_eq!(shorten_home("/home/op/src", Some(&home)), "~/src");
    }

    #[test]
    fn shorten_home_no_home_is_identity() {
        assert_eq!(shorten_home("/home/op/x", None), "/home/op/x");
    }

    #[test]
    fn harness_label_shortens_claude_code() {
        assert_eq!(harness_label("claude-code"), "claude");
        assert_eq!(harness_label("codex"), "codex");
        assert_eq!(harness_label("opencode"), "opencode");
    }

    #[test]
    fn format_recency_buckets_seconds_minutes_hours_days() {
        let now = Some(1_000_000);
        assert_eq!(format_recency(now, Some(999_990)), Some("10s".to_string()));
        assert_eq!(
            format_recency(now, Some(1_000_000 - 120)),
            Some("2m".to_string())
        );
        assert_eq!(
            format_recency(now, Some(1_000_000 - 3 * 3600)),
            Some("3h".to_string())
        );
        assert_eq!(
            format_recency(now, Some(1_000_000 - 5 * 86400)),
            Some("5d".to_string())
        );
    }

    #[test]
    fn format_recency_clamps_future_timestamps_to_zero() {
        let now = Some(1_000_000);
        assert_eq!(format_recency(now, Some(1_000_500)), Some("0s".to_string()));
    }

    #[test]
    fn format_recency_is_none_when_either_side_missing() {
        assert_eq!(format_recency(None, Some(100)), None);
        assert_eq!(format_recency(Some(100), None), None);
    }
}
