//! Interactive terminal UI for Conspectus.
//!
//! Public surface is intentionally tiny: build a [`RunConfig`] from
//! parsed CLI flags and pass it to [`run`]. Terminal lifecycle,
//! input handling, rendering, and (eventually) background discovery
//! all live behind that boundary per ADR 0024.
//!
//! v1 milestones layer in incrementally:
//!
//! - `P8-003` (this story): CLI shell + terminal lifecycle. The TUI
//!   opens, renders a placeholder frame, accepts `q` / Ctrl-C, and
//!   restores the terminal cleanly.
//! - `P8-004` through `P8-007`: row-tree view-models, detail
//!   view-models, navigation state machine, and the real two-panel
//!   render.
//! - `P8-008` onward: background discovery, mux preview, attach,
//!   resume, PR enrichment.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::filter::RowFilter;

pub mod actions;
mod app;
pub mod clipboard;
pub mod detail;
pub mod explorer;
pub mod preview;
pub mod resume;
pub mod rows;
mod runtime;
pub mod search;
pub mod theme;
mod ui;
pub mod viewer;
pub mod viewer_bridge;
pub mod widgets;

pub use app::{App, Msg};
#[cfg(any(test, debug_assertions))]
pub use runtime::run_static;
pub use theme::Theme;

/// Knobs the CLI shell passes into the TUI. The TUI does not read
/// any other CLI state — every input arrives here.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Discovery scan roots. Empty means "discover from the current
    /// working directory" at runtime.
    pub scan_roots: Vec<PathBuf>,
    /// Process working directory at launch. Treated as an
    /// orientation hint only: the row tree highlights the matching
    /// group row and pre-selects it at startup. None disables the
    /// hint (useful for headless tests).
    pub cwd: Option<PathBuf>,
    /// Initial left-panel organization.
    pub default_view: View,
    /// Row sort within each group.
    pub default_sort: Sort,
    /// Top-level grouping in the sessions tree.
    pub sessions_grouping: SessionsGrouping,
    /// Top-level grouping in the mux tree.
    pub mux_grouping: MuxGrouping,
    /// Initial row filter (ADR 0031). Applies to the sessions view
    /// in v1; F8-003 generalizes to per-view state. Empty filter
    /// admits every row.
    pub initial_filter: RowFilter,
    /// Background graph refresh cadence.
    pub refresh_interval: Duration,
    /// Selected mux pane capture cadence.
    pub mux_preview_interval: Duration,
    /// When false, suppress live extras (mux pane capture +
    /// transcript-tail reads). Graph-resident previews still render.
    pub live_preview_enabled: bool,
    /// Whether color is enabled, resolved per ADR 0022.
    pub color: bool,
    /// tmux session that hosts this TUI, when known. Used to
    /// prevent self-attachment loops.
    pub current_tmux_session: Option<String>,
    /// Resolved TUI color theme (ADR 0032). The CLI shell builds
    /// this from `[tui.theme]` config and passes it through; the
    /// renderer reads it via [`App::theme`].
    pub theme: Theme,
    /// Initial visibility of the explorer's `provenance · confidence
    /// · state` link-row meta (T8-042). Sourced from
    /// `[tui.detail].show_edge_meta` in the on-disk config. The
    /// runtime per-session `E` accelerator flips this in memory; the
    /// config knob just sets the default.
    pub show_edge_meta: bool,
}

impl RunConfig {
    /// Builds a config with v1 defaults. Useful for tests.
    pub fn defaults() -> Self {
        Self {
            scan_roots: Vec::new(),
            cwd: None,
            default_view: View::Sessions,
            default_sort: Sort::Hierarchy,
            sessions_grouping: SessionsGrouping::Graph,
            mux_grouping: MuxGrouping::Session,
            initial_filter: RowFilter::default(),
            refresh_interval: Duration::from_secs(30),
            mux_preview_interval: Duration::from_secs(2),
            live_preview_enabled: true,
            color: true,
            current_tmux_session: None,
            theme: Theme::default(),
            show_edge_meta: false,
        }
    }
}

/// Which of the registered row-tree views to render initially.
/// Mirrors `Projection` from `conspectus table <ROWS>` so the
/// terminology stays consistent across CLI and TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum View {
    Sessions,
    Mux,
    Union,
    Prs,
    Forks,
}

/// Row sort within each group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Hierarchy,
    Recency,
}

/// Top-level grouping in the sessions tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionsGrouping {
    Graph,
    Workspace,
    Repo,
    Checkout,
    ScanRoot,
    None,
}

/// Top-level grouping in the mux view. The row-tree builder lands
/// with P8-004; the enum is defined here so the controls overlay
/// and CLI surface can carry a consistent dispatch type from day
/// one (ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxGrouping {
    Session,
    Host,
    Repo,
}

/// Top-level grouping in the union view (per ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnionGrouping {
    Kind,
    Repo,
}

/// Top-level grouping in the PRs view (per ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrsGrouping {
    Repo,
    State,
}

/// Top-level grouping in the forks view (per ADR 0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForksGrouping {
    Provider,
    Parent,
}

/// Per-view grouping dispatch. Each variant wraps the per-view
/// grouping enum so a single [`Grouping`] value can be persisted in
/// [`crate::tui::app::App`], passed through the CLI surface, or
/// stored in config without leaking view-specific types upward.
///
/// Cycling helpers stay on the variant — [`Grouping::cycle_next`] and
/// [`Grouping::cycle_prev`] rotate through the active view's
/// grouping values in declaration order, wrapping around. The
/// dispatch is constructed against an active [`View`] so the
/// controls overlay never lands on a grouping that doesn't belong
/// to the visible view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    Sessions(SessionsGrouping),
    Mux(MuxGrouping),
    Union(UnionGrouping),
    Prs(PrsGrouping),
    Forks(ForksGrouping),
}

impl Grouping {
    /// The view this grouping value applies to.
    pub fn view(self) -> View {
        match self {
            Self::Sessions(_) => View::Sessions,
            Self::Mux(_) => View::Mux,
            Self::Union(_) => View::Union,
            Self::Prs(_) => View::Prs,
            Self::Forks(_) => View::Forks,
        }
    }

    /// The default grouping for a view. Sessions defaults to
    /// graph-first topology so workspace containment and resolved
    /// lineage are visible without extra controls.
    pub fn default_for(view: View) -> Self {
        match view {
            View::Sessions => Self::Sessions(SessionsGrouping::Graph),
            View::Mux => Self::Mux(MuxGrouping::Session),
            View::Union => Self::Union(UnionGrouping::Kind),
            View::Prs => Self::Prs(PrsGrouping::Repo),
            View::Forks => Self::Forks(ForksGrouping::Provider),
        }
    }

    /// Stable lower-case string name used in chips, config, and the
    /// CLI. Round-trips with [`Grouping::parse_for`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sessions(SessionsGrouping::Graph) => "graph",
            Self::Sessions(SessionsGrouping::Workspace) => "workspace",
            Self::Sessions(SessionsGrouping::Repo)
            | Self::Mux(MuxGrouping::Repo)
            | Self::Union(UnionGrouping::Repo) => "repo",
            Self::Sessions(SessionsGrouping::Checkout) => "checkout",
            Self::Sessions(SessionsGrouping::ScanRoot) => "scan-root",
            Self::Sessions(SessionsGrouping::None) => "none",
            Self::Mux(MuxGrouping::Session) => "session",
            Self::Mux(MuxGrouping::Host) => "host",
            Self::Union(UnionGrouping::Kind) => "kind",
            Self::Prs(PrsGrouping::Repo) => "repo",
            Self::Prs(PrsGrouping::State) => "state",
            Self::Forks(ForksGrouping::Provider) => "provider",
            Self::Forks(ForksGrouping::Parent) => "parent",
        }
    }

    /// Parse a value string for a particular view. Returns `None`
    /// for spellings that don't match any grouping option for that
    /// view; callers raise an actionable error.
    pub fn parse_for(view: View, value: &str) -> Option<Self> {
        let needle = value.trim().to_ascii_lowercase();
        for option in Self::values_for(view) {
            if option.as_str() == needle {
                return Some(*option);
            }
        }
        None
    }

    /// All grouping values available for `view`, in display order.
    /// Used by the controls overlay's grouping section and by CLI
    /// error messages that list the legal values.
    pub fn values_for(view: View) -> &'static [Grouping] {
        match view {
            View::Sessions => &[
                Self::Sessions(SessionsGrouping::Graph),
                Self::Sessions(SessionsGrouping::Workspace),
                Self::Sessions(SessionsGrouping::Repo),
                Self::Sessions(SessionsGrouping::Checkout),
                Self::Sessions(SessionsGrouping::ScanRoot),
                Self::Sessions(SessionsGrouping::None),
            ],
            View::Mux => &[
                Self::Mux(MuxGrouping::Session),
                Self::Mux(MuxGrouping::Host),
                Self::Mux(MuxGrouping::Repo),
            ],
            View::Union => &[
                Self::Union(UnionGrouping::Kind),
                Self::Union(UnionGrouping::Repo),
            ],
            View::Prs => &[Self::Prs(PrsGrouping::Repo), Self::Prs(PrsGrouping::State)],
            View::Forks => &[
                Self::Forks(ForksGrouping::Provider),
                Self::Forks(ForksGrouping::Parent),
            ],
        }
    }

    /// Next grouping value within the active view, wrapping around
    /// at the end. Used by the `G` accelerator and the controls
    /// overlay's grouping section.
    pub fn cycle_next(self) -> Self {
        let values = Self::values_for(self.view());
        let idx = values.iter().position(|v| *v == self).unwrap_or(0);
        values[(idx + 1) % values.len()]
    }

    /// Previous grouping value within the active view, wrapping at
    /// the start.
    pub fn cycle_prev(self) -> Self {
        let values = Self::values_for(self.view());
        let idx = values.iter().position(|v| *v == self).unwrap_or(0);
        let len = values.len();
        values[(idx + len - 1) % len]
    }
}

/// Entry point. Owns the terminal for the duration of the call and
/// restores it on normal exit, errors, and panics.
pub fn run(config: RunConfig) -> Result<()> {
    runtime::run(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_for_each_view_matches_adr_first_entry() {
        assert_eq!(
            Grouping::default_for(View::Sessions),
            Grouping::Sessions(SessionsGrouping::Graph)
        );
        assert_eq!(
            Grouping::default_for(View::Mux),
            Grouping::Mux(MuxGrouping::Session)
        );
        assert_eq!(
            Grouping::default_for(View::Union),
            Grouping::Union(UnionGrouping::Kind)
        );
        assert_eq!(
            Grouping::default_for(View::Prs),
            Grouping::Prs(PrsGrouping::Repo)
        );
        assert_eq!(
            Grouping::default_for(View::Forks),
            Grouping::Forks(ForksGrouping::Provider)
        );
    }

    #[test]
    fn parse_and_as_str_round_trip_per_view() {
        for view in [
            View::Sessions,
            View::Mux,
            View::Union,
            View::Prs,
            View::Forks,
        ] {
            for option in Grouping::values_for(view) {
                let parsed = Grouping::parse_for(view, option.as_str());
                assert_eq!(parsed, Some(*option), "round-trip for {option:?}");
            }
        }
    }

    #[test]
    fn parse_rejects_other_views_values() {
        // "session" is a mux grouping, not a sessions grouping.
        assert!(Grouping::parse_for(View::Sessions, "session").is_none());
        // "host" is mux-only.
        assert!(Grouping::parse_for(View::Prs, "host").is_none());
    }

    #[test]
    fn parse_is_case_insensitive_and_trimmed() {
        assert_eq!(
            Grouping::parse_for(View::Sessions, "  GRAPH  "),
            Some(Grouping::Sessions(SessionsGrouping::Graph))
        );
    }

    #[test]
    fn cycle_next_wraps_within_active_view() {
        let start = Grouping::default_for(View::Sessions);
        let mut g = start;
        for _ in 0..Grouping::values_for(View::Sessions).len() {
            g = g.cycle_next();
        }
        assert_eq!(g, start, "full cycle returns to start");
    }

    #[test]
    fn cycle_prev_is_inverse_of_cycle_next() {
        let start = Grouping::default_for(View::Mux);
        assert_eq!(start.cycle_next().cycle_prev(), start);
        assert_eq!(start.cycle_prev().cycle_next(), start);
    }

    #[test]
    fn view_dispatch_is_consistent() {
        for view in [
            View::Sessions,
            View::Mux,
            View::Union,
            View::Prs,
            View::Forks,
        ] {
            assert_eq!(Grouping::default_for(view).view(), view);
        }
    }
}
