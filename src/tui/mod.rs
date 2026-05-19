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

mod app;
pub mod rows;
mod runtime;
mod ui;

pub use app::{App, Msg};

/// Knobs the CLI shell passes into the TUI. The TUI does not read
/// any other CLI state — every input arrives here.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Discovery scan roots. Empty means "discover from the current
    /// working directory" at runtime.
    pub scan_roots: Vec<PathBuf>,
    /// Initial left-panel organization.
    pub default_view: View,
    /// Row sort within each group.
    pub default_sort: Sort,
    /// Top-level grouping in the sessions tree.
    pub sessions_grouping: SessionsGrouping,
    /// Background graph refresh cadence.
    pub refresh_interval: Duration,
    /// Selected mux pane capture cadence.
    pub mux_preview_interval: Duration,
    /// When false, suppress live extras (mux pane capture +
    /// transcript-tail reads). Graph-resident previews still render.
    pub live_preview_enabled: bool,
    /// Whether color is enabled, resolved per ADR 0022.
    pub color: bool,
}

impl RunConfig {
    /// Builds a config with v1 defaults. Useful for tests.
    pub fn defaults() -> Self {
        Self {
            scan_roots: Vec::new(),
            default_view: View::Sessions,
            default_sort: Sort::Hierarchy,
            sessions_grouping: SessionsGrouping::Graph,
            refresh_interval: Duration::from_secs(30),
            mux_preview_interval: Duration::from_secs(2),
            live_preview_enabled: true,
            color: true,
        }
    }
}

/// Which of the registered row-tree views to render initially.
/// Mirrors `Projection` from `conspectus table <ROWS>` so the
/// terminology stays consistent across CLI and TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Repo,
    Worktree,
    ScanRoot,
}

/// Entry point. Owns the terminal for the duration of the call and
/// restores it on normal exit, errors, and panics.
pub fn run(config: RunConfig) -> Result<()> {
    runtime::run(config)
}
