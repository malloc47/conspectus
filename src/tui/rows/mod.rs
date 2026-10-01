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

pub mod forks;
pub mod mux;
pub mod prs;
pub mod sessions;
pub mod union;

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
    /// An unbound session pin row (ADR 0057). Keyed on the pin id
    /// so the row is stable across refreshes even as the pin's
    /// binding state changes — once a pin binds, the same logical
    /// entry switches from a [`RowKind::Pin`] row to a
    /// [`RowKind::AgentSession`] row whose `pin_id` field carries
    /// the pin marker.
    Pin {
        pin_id: String,
    },
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
    MuxSession(MuxSessionRow),
    Pr(PrRow),
    Fork(ForkRow),
    /// Unbound pin row (ADR 0057). Emitted by the sessions builder
    /// for `PinCandidate`s whose `binding` is `Unbound` or
    /// `StaleMux` — i.e. no live agent session is realizing the
    /// pin. Bound pins flow through the existing
    /// [`RowKind::AgentSession`] surface with `pin_id` set.
    Pin(PinRow),
    /// Repo row emitted by the workspaces view (H-WS-002) for
    /// member rows under each workspace. Styled like the agent /
    /// mux rows so it scans as the same visual rhythm rather than
    /// as a bare group header.
    Repo(RepoRow),
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
    /// Project label for flat sessions layouts. Populated when the
    /// sessions view uses `grouping = "none"` so the table-like
    /// row can preserve project context without rendering ancestor
    /// group rows.
    pub project_display: Option<String>,
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
    /// today). The right panel always renders it as a header field;
    /// the left tree row picks it up only when
    /// [`Self::title_disambiguates`] is `true`.
    pub title: Option<String>,
    /// Operator-chosen display name from the ADR 0029 alias overlay,
    /// when set. Takes precedence over [`Self::title`] at every
    /// projection site via [`Self::display_label`].
    pub alias: Option<String>,
    /// P8-015: the sessions row-tree builder flips this on when the
    /// parent project group contains another same-harness session
    /// and this row carries a non-empty `title`. The left-tree
    /// renderer consults it via [`Self::tree_label`] so titles only
    /// surface in the row when they disambiguate. The right pane,
    /// search index, and status hints continue to read
    /// [`Self::display_label`] / [`Self::title`] unconditionally.
    pub title_disambiguates: bool,
    pub primary_node: NodeId,
    /// Pin id when this agent-session row is the live realization of
    /// a [`crate::model::PinCandidate`] (ADR 0057). Renderers add a
    /// pin glyph so the operator can distinguish pin-bound sessions
    /// at a glance; the rest of the row shape stays identical to a
    /// non-pinned session — the alias overlay already injected the
    /// pin's `display_name` via [`crate::resolve::pins`].
    pub pin_id: Option<String>,
}

impl AgentSessionRow {
    /// Apply ADR 0029's `alias > title > id-suffix` precedence and
    /// return the strongest display label available for this row.
    /// Falls back to `None` when neither alias nor title is set so
    /// callers can render the short-id suffix instead.
    pub fn display_label(&self) -> Option<&str> {
        self.alias.as_deref().or(self.title.as_deref())
    }

    /// Display label for the left-tree row body (P8-015). Alias wins
    /// when set (operator-chosen names are always meaningful in the
    /// tree); otherwise the title is surfaced only when the builder
    /// flagged this row as needing disambiguation. The right pane,
    /// status hints, and search index keep using
    /// [`Self::display_label`] so the title is never hidden from
    /// surfaces where it carries diagnostic value.
    pub fn tree_label(&self) -> Option<&str> {
        if let Some(alias) = self.alias.as_deref().filter(|s| !s.is_empty()) {
            return Some(alias);
        }
        if self.title_disambiguates {
            return self.title.as_deref().filter(|s| !s.is_empty());
        }
        None
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

/// A mux-session row in the mux or union view.
#[derive(Clone, Debug, PartialEq)]
pub struct MuxSessionRow {
    pub mux: MuxSessionId,
    pub backend: String,
    pub native_id: String,
    pub client_attached: Option<bool>,
    pub cwd_display: Option<String>,
    pub attached_count: usize,
    pub ambiguous_count: usize,
    pub recency: Option<String>,
    pub activity_epoch: Option<i64>,
    /// `session_created` epoch, carried so the mux view can order by
    /// the "created" recency basis (H-MUX-SORT-001).
    pub created_epoch: Option<i64>,
    /// `session_last_attached` epoch, carried so the mux view can
    /// order by the "last attached" recency basis (H-MUX-SORT-001).
    pub last_attached_epoch: Option<i64>,
    /// Unique harness labels for visible agent sessions linked to
    /// this mux. Renderers use these as the primary mux-row labels so
    /// mux rows scan like session rows without repeating session IDs.
    pub agent_labels: Vec<String>,
    /// Last-message preview of the sole attached agent session,
    /// populated only when exactly one visible agent is linked to this
    /// mux. Renderers flow it into the trailing space after the CWD so
    /// single-session muxes carry an inline content cue; multi-session
    /// muxes expose sessions as child rows where each preview is shown
    /// individually.
    pub single_session_preview: Option<String>,
    /// `Some(pin_id)` when this mux is the bound mux of a declared
    /// session pin (ADR 0057). Renderers paint a pin glyph next to
    /// the mux label so the operator can spot pin-linked muxes from
    /// any view.
    pub pin_id: Option<String>,
    pub primary_node: NodeId,
}

/// A PR row in the prs view.
#[derive(Clone, Debug, PartialEq)]
pub struct PrRow {
    pub pr_number: u64,
    pub repo_display: String,
    pub state: Option<String>,
    pub is_draft: bool,
    pub branch_name: Option<String>,
    pub updated_recency: Option<String>,
    pub attached_count: usize,
    pub url: Option<String>,
    pub primary_node: NodeId,
}

/// A fork row in the forks view.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkRow {
    pub fork_label: String,
    pub provider: String,
    pub scope: Option<String>,
    pub parent_label: Option<String>,
    pub child_count: usize,
    pub primary_node: NodeId,
}

/// An unbound session pin row (ADR 0057). Rendered when the pin has
/// no live mux (`PinBinding::Unbound`) or has a live mux but no
/// harness session attributed to it (`PinBinding::StaleMux`).
#[derive(Clone, Debug, PartialEq)]
pub struct PinRow {
    pub pin_id: String,
    pub display_name: String,
    pub harness: String,
    pub cwd: String,
    pub mux_name: String,
    pub mux_socket: Option<String>,
    pub launch_argv: Vec<String>,
    pub store_path: String,
    pub harness_label: String,
    pub cwd_display: String,
    pub mux_label: String,
    /// Human-readable binding state ("unbound" or "stale-mux"). The
    /// renderer surfaces this so the operator can pick the right next
    /// action (launch vs relaunch in existing mux).
    pub state_label: &'static str,
}

/// A workspace member row in the workspaces view (H-WS-002). Carries
/// the short id + display label + canonical path the renderer needs
/// to render the row with the same visual rhythm as
/// [`AgentSessionRow`] and [`MuxSessionRow`] rather than as a bare
/// group header.
#[derive(Clone, Debug, PartialEq)]
pub struct RepoRow {
    pub short_id: String,
    /// Workspace-visible name for the member — atelier's
    /// `[[repos]].name`, agent-deck's symlink leaf, generic
    /// discovery's child name. Same string as the membership link's
    /// `logical_path` basename.
    pub display_name: String,
    /// Operator-recognizable filesystem path for the repo. Prefer the
    /// first non-agent-deck `source_paths` entry (the canonical
    /// checkout), falling back to the git `common_dir`. `None` only
    /// for snapshot shapes that have neither.
    pub canonical_path: Option<String>,
    /// Git `common_dir` (the `.git` path). Kept on the row so the
    /// renderer can fall back when `canonical_path` is absent and so
    /// `node show` and the detail pane have a stable handle.
    pub common_dir: String,
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

/// Assemble the canonical workspace top-row display string used by
/// the Workspaces view (`H-WS-002` polish, ADR 0062) and by the
/// hybrid Sessions/Graph workspace headers (ADR 0064):
/// `<name>  <repo-a+repo-b+...>  (<provider>)`. The member list and
/// provider segment are each prefixed with two spaces so the eye
/// can pick out the three slots without a glyph budget. Sections
/// are omitted cleanly when their data is missing — a workspace
/// with no members drops the join segment, with no provider drops
/// the parens, and with neither degrades to its bare label.
pub fn format_workspace_display(
    workspace_label: &str,
    member_display_names: &[String],
    provider: Option<&str>,
) -> String {
    let mut out = workspace_label.to_string();
    if !member_display_names.is_empty() {
        out.push_str("  ");
        out.push_str(&member_display_names.join("+"));
    }
    if let Some(provider) = provider {
        out.push_str("  (");
        out.push_str(provider);
        out.push(')');
    }
    out
}

/// Translate a harness key to the short label rendered in the row.
/// H-EXT-002: delegates to the adapter registry so
/// `claude-code`'s `claude` collapse (H-TBL-014) lives on
/// [`crate::discovery::harness::ClaudeCodeAdapter::display_label`]
/// rather than in a match table here.
pub fn harness_label(harness_key: &str) -> String {
    crate::discovery::harness::display_label_for(harness_key)
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

/// Coarse activity buckets (Phase 3 of the styling overhaul). The
/// renderer maps each bucket to a [`Theme`] style so the recency
/// column carries a freshness signal in color in addition to the
/// numeric label. Bucket boundaries:
///
/// - `< 5m`   → [`RecencyBucket::Fresh`]
/// - `< 1h`   → [`RecencyBucket::Active`]
/// - `< 1d`   → [`RecencyBucket::Recent`]
/// - `≥ 1d`   → [`RecencyBucket::Cold`]
///
/// Returns `None` when either side of the delta is missing so the
/// caller can fall back to placeholder styling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecencyBucket {
    Fresh,
    Active,
    Recent,
    Cold,
}

// H-HYG-002: shared row-assembly helpers. Pre-H-HYG-002 the
// same `agent_row` / `mux_indicator` / `session_matches_filter`
// / `collect_agent_mux_candidate_counts` bodies were duplicated
// across `rows/{union,prs,forks}.rs` (plus a near-twin in
// `rows/mux.rs`) and `output/{prs,forks}.rs`. They live here now
// as `pub(crate)` helpers the per-view builders share.
//
// `rows/mux.rs::agent_row` stays put because it uses a different
// input struct (`AttachedAgent`) — near-twin, not identical.

/// Snapshot of an `AgentSessionNode` prepared for the sessions
/// / union / prs / forks row builders. The `node_id` field is
/// the stringified `NodeId::AgentSession(agent.id)` used to
/// index the `candidate_counts` map.
#[derive(Clone, Debug)]
pub(crate) struct AgentData<'a> {
    pub node_id: String,
    pub id: crate::model::AgentSessionId,
    pub node: &'a crate::model::AgentSessionNode,
    pub alias: Option<String>,
}

/// Build an `AgentSession` row for a single `AgentData`. Every
/// non-mux row-tree builder consumes this — the mux builder
/// uses its own near-twin (`rows/mux.rs::agent_row`) because
/// it works over a different input struct.
pub(crate) fn agent_row(
    agent: &AgentData<'_>,
    depth: u8,
    candidate_counts: &std::collections::HashMap<String, usize>,
    short_id: String,
    home: Option<&Path>,
    now: Option<i64>,
) -> Row {
    let node_id = NodeId::AgentSession(agent.id.clone());
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    Row {
        id: RowId::AgentSession(node_id.clone()),
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: agent.id.clone(),
            short_id,
            pin_id: None,
            harness_label: harness_label(&agent.id.harness_key),
            cwd_display: agent.node.cwd.as_deref().map(|cwd| shorten_home(cwd, home)),
            project_display: None,
            recency: format_recency(now, agent.node.last_active_epoch),
            activity_epoch: agent.node.last_active_epoch,
            mux_state: mux_indicator(candidate_count),
            preview: agent.node.last_message_preview.clone(),
            title: agent.node.title.clone(),
            alias: agent.alias.clone(),
            title_disambiguates: false,
            primary_node: node_id,
        }),
    }
}

/// Map a `LinkedToMux` candidate count to a
/// [`MuxIndicator`] variant.
pub(crate) fn mux_indicator(candidate_count: usize) -> MuxIndicator {
    match candidate_count {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    }
}

/// True when `agent` should render given `filter`. Non-narrowing
/// filters always match; otherwise defer to
/// `RowFilter::matches_session` with a
/// [`SessionMatchInputs`] built from the agent's fields.
pub(crate) fn session_matches_filter(
    agent: &AgentData<'_>,
    candidate_counts: &std::collections::HashMap<String, usize>,
    now: Option<i64>,
    filter: &crate::filter::RowFilter,
) -> bool {
    use crate::filter::{MuxStateKey, SessionMatchInputs};
    if !filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = candidate_counts.get(&agent.node_id).copied().unwrap_or(0);
    filter.matches_session(&SessionMatchInputs {
        harness_key: &agent.id.harness_key,
        now_epoch: now,
        last_active_epoch: agent.node.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    })
}

pub fn recency_bucket(now: Option<i64>, activity_epoch: Option<i64>) -> Option<RecencyBucket> {
    let now = now?;
    let then = activity_epoch?;
    let delta = (now - then).max(0);
    Some(if delta < 5 * 60 {
        RecencyBucket::Fresh
    } else if delta < 60 * 60 {
        RecencyBucket::Active
    } else if delta < 24 * 60 * 60 {
        RecencyBucket::Recent
    } else {
        RecencyBucket::Cold
    })
}

impl RecencyBucket {
    /// Build the [`ratatui::style::Style`] for this bucket from the
    /// active [`Theme`]. The mapping pulls the per-bucket `StyleSpec`
    /// directly so operators who override a single bucket through
    /// `[tui.theme]` see the change immediately.
    pub fn style(self, theme: &crate::tui::Theme) -> ratatui::style::Style {
        match self {
            Self::Fresh => theme.recency_fresh.into_style(),
            Self::Active => theme.recency_active.into_style(),
            Self::Recent => theme.recency_recent.into_style(),
            Self::Cold => theme.recency_cold.into_style(),
        }
    }
}

/// Projection state consumed by [`build_tree_for_view`]. Read from
/// [`crate::tui::App`], which is the sole owner of projection
/// state (ADR 0085 contract 4). `App`'s fields are seeded from
/// [`crate::tui::RunConfig`] in `App::new`, so cold-start builds
/// see the initial config values through the same read path.
pub(crate) struct TreeInputs<'a> {
    pub snapshot: &'a crate::model::GraphSnapshot,
    pub view: View,
    pub grouping: crate::tui::Grouping,
    pub filter: crate::filter::RowFilter,
    pub sort: crate::tui::Sort,
    pub mux_recency: crate::tui::MuxRecency,
    pub cwd: Option<&'a Path>,
}

impl<'a> TreeInputs<'a> {
    /// Assemble tree-derivation inputs from an [`crate::tui::App`]'s
    /// current projection state plus the passed-in snapshot. Used
    /// by the reducer's projection-change arms and by the runtime's
    /// discovery-result handler.
    pub fn from_app(snapshot: &'a crate::model::GraphSnapshot, app: &'a crate::tui::App) -> Self {
        Self {
            snapshot,
            view: app.active_view(),
            grouping: app.grouping(),
            filter: app.filter().clone(),
            sort: app.sort(),
            mux_recency: app.mux_recency(),
            cwd: app.config().cwd.as_deref(),
        }
    }
}

/// Pure row-tree derivation (ADR 0085 contract 4). Reads only the
/// projection tuple; discovery-triggering paths pull it from
/// [`crate::tui::RunConfig`] at startup, and post-first-load
/// projection changes pull it from `App` state.
pub(crate) fn build_tree_for_view(inputs: TreeInputs<'_>) -> RowTree {
    let home = tree_home_dir();
    let now = tree_current_unix_epoch();
    let TreeInputs {
        snapshot,
        view,
        grouping,
        filter,
        sort,
        mux_recency,
        cwd,
    } = inputs;
    match view {
        View::Sessions => {
            let sessions_grouping = match grouping {
                crate::tui::Grouping::Sessions(g) => g,
                _ => crate::tui::SessionsGrouping::Graph,
            };
            sessions::build_sessions_tree(sessions::SessionsBuildInputs {
                snapshot,
                grouping: sessions_grouping,
                home: home.as_deref(),
                now,
                cwd,
                filter,
            })
        }
        View::Mux => {
            let mux_grouping = match grouping {
                crate::tui::Grouping::Mux(g) => g,
                _ => crate::tui::MuxGrouping::Session,
            };
            mux::build_mux_tree(mux::MuxBuildInputs {
                snapshot,
                home: home.as_deref(),
                now,
                filter,
                grouping: mux_grouping,
                sort,
                mux_recency,
            })
        }
        View::Union => union::build_union_tree(union::UnionBuildInputs {
            snapshot,
            home: home.as_deref(),
            filter,
        }),
        View::Prs => prs::build_prs_tree(prs::PrsBuildInputs {
            snapshot,
            home: home.as_deref(),
        }),
        View::Forks => forks::build_forks_tree(forks::ForksBuildInputs {
            snapshot,
            home: home.as_deref(),
        }),
    }
}

fn tree_home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Wall-clock unix epoch that view-model derivations (row trees, the
/// node explorer) use for relative ages.
pub(crate) fn tree_current_unix_epoch() -> Option<i64> {
    Some(crate::discovery::current_epoch())
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
