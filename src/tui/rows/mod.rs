//! Pure row-tree view-models for the TUI left panel.
//!
//! Builders are deterministic functions of `(GraphSnapshot, RunConfig,
//! now)` → [`RowTree`]. They contain no ratatui types and no I/O so
//! they snapshot-test without a runtime. The TUI's renderer
//! (`crate::tui::ui`) and the CLI's table surface
//! ([`crate::output::table`]) can both consume them.
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

use std::path::Path;

use crate::model::{AgentSessionId, MuxSessionId, NodeId};
use crate::tui::View;

pub mod forks;
pub mod history;
pub mod mux;
pub mod pinned;
pub mod prs;
pub mod sessions;
pub mod union;

pub use sessions::{SessionsBuildInputs, build_sessions_tree};

/// Stable, hashable row identity. Used by the selection state
/// machine to retain selection across refreshes — two
/// builds of the same view over the same node set produce identical
/// `RowId`s.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RowId {
    /// A group row backed by a node (workspace, repo, worktree, mux,
    /// fork, …). Carries the node id so future operations can resolve
    /// back to `node show`.
    Group(NodeId),
    AgentSession(NodeId),
    MuxSession(NodeId),
    Pr(NodeId),
    Fork(NodeId),
    /// A record row in the History view (ADR 0140), keyed on the
    /// record's stable id.
    History {
        record_id: String,
    },
    /// A slot row in the Pinned view (ADR 0141), keyed on the slot
    /// name so it is stable while the slot's target changes state.
    Pin {
        pin_id: String,
    },
    /// An entity listed under a slot in the Pinned view: the sessions
    /// of a mux slot, the muxes of a checkout slot. Keyed on the slot
    /// as well, so the same entity under two slots gets two rows.
    Nested {
        pin_id: String,
        node: NodeId,
    },
    /// A named bucket of a view that groups by something other than a
    /// node: the project or kind buckets of the Pinned view.
    Bucket(String),
    /// A PRs-view group for a forge repo (`owner/repo`) when no local
    /// repo is known for its PRs (ADR 0129).
    Remote(String),
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
    Pinned,
    History,
}

impl From<View> for ViewLabel {
    fn from(view: View) -> Self {
        match view {
            View::Sessions => Self::Sessions,
            View::Mux => Self::Mux,
            View::Union => Self::Union,
            View::Prs => Self::Prs,
            View::Forks => Self::Forks,
            View::Pinned => Self::Pinned,
            View::History => Self::History,
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
    MuxSession(MuxSessionRow),
    Pr(PrRow),
    Fork(ForkRow),
    /// A row of any kind in the Pinned and History views, drawn in the
    /// shared mixed-kind columns.
    Mixed(MixedRow),
}

/// How a mixed-kind row's status reads: dimmed rows are the ones whose
/// entity isn't there to act on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedTone {
    Normal,
    /// Gone, missing, planned, or unknown.
    Dim,
}

/// One row in `<kind> <label> <status> <age> <content>` (ADR 0141).
/// The kind glyph shows for every kind, sessions included, because it
/// is the only kind signal in a list of mixed kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct MixedRow {
    pub node_kind: crate::model::NodeKind,
    pub label: String,
    pub status: String,
    pub tone: MixedTone,
    /// Age as `format_recency` renders it; `None` when unknown.
    pub age: Option<String>,
    /// The instant behind [`Self::age`], which picks the age's recency
    /// color the way the other views do.
    pub age_epoch: Option<i64>,
    /// The state glyph of a row in the Mux grid, where the Mux view
    /// draws a glyph rather than a word.
    pub state: Option<EntityState>,
    /// The agent or program the entity runs, drawn as the same filled
    /// chip the other views use.
    pub chip: Option<MixedChip>,
    /// What follows the chip: a directory, a title, a repo and branch.
    pub content: String,
    pub primary_node: NodeId,
    /// PR state, which tints the PR glyph.
    pub pr_state: Option<String>,
    /// The pin marker cell after the kind glyph.
    pub marker: PinMarker,
    /// Which column grid draws the row.
    pub grid: MixedGrid,
}

/// An agent or program name drawn as a chip.
#[derive(Clone, Debug, PartialEq)]
pub struct MixedChip {
    pub label: String,
    /// A registered agent harness (its own color) rather than a plain
    /// program.
    pub agent: bool,
}

impl MixedChip {
    pub fn agent(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            agent: true,
        }
    }

    pub fn program(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            agent: false,
        }
    }
}

/// The pin marker of a row: a pair of glyphs, filled when the pinned
/// entity is running and hollow when it isn't, so the marker answers
/// "is this pinned" and "is the pin's target up" the same way in every
/// view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinMarker {
    /// Not pinned.
    None,
    /// A pin on an entity that is running (`◉`).
    Running,
    /// A pin on an entity that is stopped, removed, missing, or not
    /// started yet (`◎`, in the ghost color).
    Stopped,
}

/// The state a primary-entity row draws as a glyph: sessions and
/// muxes link or don't, and a mux can also be stopped or unchecked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityState {
    /// Linked to a definitive mux, or a client is attached.
    Linked,
    /// No definitive mux, or no client attached.
    Unlinked,
    /// A mux that has stopped.
    Stopped,
    /// A mux that couldn't be checked.
    Unknown,
}

/// How a mixed-kind row lays out its columns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedGrid {
    /// Label, status, age, then chip and content, as in the Pinned and
    /// History views.
    Columns,
    /// The Mux view's grid (name, chip, age, state), so a ghost mux
    /// lines up with the live ones around it.
    Mux,
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
    /// Short content-addressed identifier. Floored at 6 chars; the
    /// dispatcher grows the floor only to break
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
    /// The sessions row-tree builder flips this on when the
    /// parent project group contains another same-harness session
    /// and this row carries a non-empty `title`. The left-tree
    /// renderer consults it via [`Self::tree_label`] so titles only
    /// surface in the row when they disambiguate. The right pane,
    /// search index, and status hints continue to read
    /// [`Self::display_label`] / [`Self::title`] unconditionally.
    pub title_disambiguates: bool,
    /// The mux hosting this session, or the one that hosted it last
    /// when that mux is gone. Untitled sessions are named after it,
    /// and a gone one shows as a `was in` chip.
    pub mux_host: Option<crate::model::MuxHost>,
    pub primary_node: NodeId,
    /// Pin slot name when this row is the live session a slot points
    /// at. Renderers add a pin glyph.
    pub pin_id: Option<String>,
}

impl AgentSessionRow {
    /// Apply the `alias > title > mux name > id-suffix` precedence and
    /// return the strongest display label available for this row.
    /// Falls back to `None` when none of the first three is set so
    /// callers can render the short-id suffix instead.
    pub fn display_label(&self) -> Option<&str> {
        crate::aliases::session_display_name(
            self.alias.as_deref(),
            self.title.as_deref(),
            self.mux_host.as_ref().map(|host| host.name.as_str()),
        )
    }

    /// Display label for the left-tree row body. Alias wins
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
        // An untitled session is named after its mux; a titled one
        // that doesn't need disambiguating keeps its short id.
        if self.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
            return self
                .mux_host
                .as_ref()
                .map(|host| host.name.as_str())
                .filter(|name| !name.is_empty());
        }
        None
    }

    /// The name of the gone mux this session last ran in, for the
    /// `was in` chip. `None` while the mux exists, since the live mux
    /// already shows in the row.
    pub fn gone_mux_name(&self) -> Option<&str> {
        self.mux_host
            .as_ref()
            .filter(|host| !host.live)
            .map(|host| host.name.as_str())
    }
}

/// An agent session's mux attachment state. The row chip renders only
/// `◉` (attached) or `◯` (everything else); filters, header counts,
/// and the detail pane read the full three-way distinction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MuxIndicator {
    /// `◉`. One resolver-preferred mux candidate, no competitors.
    Attached,
    /// ≥ 2 active `LinkedToMux` candidates; the resolver picked
    /// one but alternatives exist. Carries the total candidate count
    /// (preferred + competing) so the status bar and right panel can
    /// surface "N candidates" without re-walking the graph.
    Ambiguous { candidate_count: usize },
    /// `◯`. No mux link at all.
    Unmuxed,
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
    pub recency: Option<String>,
    pub activity_epoch: Option<i64>,
    /// `session_created` epoch, carried so the mux view can order by
    /// the "created" recency basis.
    pub created_epoch: Option<i64>,
    /// `session_last_attached` epoch, carried so the mux view can
    /// order by the "last attached" recency basis.
    pub last_attached_epoch: Option<i64>,
    /// Unique harness labels for visible agent sessions linked to
    /// this mux. Renderers use these as the primary mux-row labels so
    /// mux rows scan like session rows without repeating session IDs.
    pub agent_labels: Vec<String>,
    /// Program the mux runs, shown in place of agent labels when no
    /// agent is linked so shell, build, and server panes still say what
    /// they run. See `mux_program` for where it comes from.
    pub program: Option<String>,
    /// Display label of the harness the pane runs when no agent session
    /// is attributed to the mux, so the row keeps the harness badge and
    /// color instead of a plain program chip. See `mux_program_harness`.
    pub program_harness: Option<String>,
    /// Last-message preview of the sole attached agent session,
    /// populated only when exactly one visible agent is linked to this
    /// mux. Renderers flow it into the trailing space after the CWD so
    /// single-session muxes carry an inline content cue; multi-session
    /// muxes expose sessions as child rows where each preview is shown
    /// individually.
    pub single_session_preview: Option<String>,
    /// `Some(slot id)` when a pin slot points at this mux (ADR 0141).
    /// Renderers mark it so pinned muxes stand out in any view.
    pub pin_id: Option<String>,
    pub primary_node: NodeId,
}

/// A PR row in the prs view.
#[derive(Clone, Debug, PartialEq)]
pub struct PrRow {
    pub pr_number: u64,
    /// `owner/repo#N`: the text search matches, and the left-pane
    /// label unless the group names the repo.
    pub repo_display: String,
    /// What the left pane shows for the PR: `#N` under a group that
    /// names the forge repo, else [`Self::repo_display`] (ADR 0131).
    pub label: String,
    pub state: Option<String>,
    pub is_draft: bool,
    pub branch_name: Option<String>,
    /// True when the PR resolves to a local branch (ADR 0129).
    pub tracked: bool,
    /// The PR title on one line, when the forge listing gave one.
    pub title: Option<String>,
    pub updated_recency: Option<String>,
    /// The instant behind [`Self::updated_recency`], for its color.
    pub updated_epoch: Option<i64>,
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
/// the Workspaces view (ADR 0062) and by the
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

/// Operator-facing path for a repo (ADR 0034). A non-bare repo's
/// `common_dir` is `<checkout>/.git`, so its parent is the canonical
/// checkout whichever checkout a probe reached first; discovery keeps
/// only the first probe's `source_paths` (first-write-wins merge), so
/// reading those instead would make labels, sort order, and launch
/// directories depend on probe order. A bare repo implies no checkout,
/// so it falls back to the first probed checkout outside agent-deck's
/// multi-repo worktrees, then to the common dir itself.
pub fn repo_display_path(
    repo: &crate::model::RepoId,
    node: Option<&crate::model::RepoNode>,
) -> String {
    if let Some(canonical) = repo.common_dir.strip_suffix("/.git") {
        return canonical.to_string();
    }
    node.and_then(|node| {
        node.source_paths
            .iter()
            .find(|path| !path.contains("/.agent-deck/multi-repo-worktrees/"))
    })
    .cloned()
    .unwrap_or_else(|| repo.common_dir.clone())
}

/// Header text for a workspace group, shared by the sessions and mux
/// views so both read identically: the workspace node's `name`, else
/// the root's last component, else the `~`-shortened root, followed by
/// its members and provider per [`format_workspace_display`].
pub fn workspace_header_label(
    snapshot: &crate::model::GraphSnapshot,
    workspace_root: &str,
    home: Option<&Path>,
) -> String {
    let label = workspace_display_name(snapshot, workspace_root, home);
    let provider = workspace_node(snapshot, workspace_root).and_then(|w| w.provider.as_deref());
    let members = workspace_member_names(snapshot, workspace_root);
    format_workspace_display(&label, &members, provider)
}

/// The name a workspace header leads with (its bold label): the
/// workspace node's `name`, else the root's last component, else the
/// `~`-shortened root.
pub fn workspace_display_name(
    snapshot: &crate::model::GraphSnapshot,
    workspace_root: &str,
    home: Option<&Path>,
) -> String {
    workspace_node(snapshot, workspace_root)
        .and_then(|w| w.name.clone())
        .or_else(|| {
            Path::new(workspace_root)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| shorten_home(workspace_root, home))
}

fn workspace_node<'a>(
    snapshot: &'a crate::model::GraphSnapshot,
    workspace_root: &str,
) -> Option<&'a crate::model::WorkspaceNode> {
    snapshot.nodes.iter().find_map(|node| match node {
        crate::model::GraphNode::Workspace(w) if w.id.root == workspace_root => Some(w),
        _ => None,
    })
}

/// Member display names for a workspace, derived from the
/// resolver-selected `WorkspaceContainsRepo` candidate links'
/// `logical_path` source field. Sorted alphabetically and deduped.
fn workspace_member_names(
    snapshot: &crate::model::GraphSnapshot,
    workspace_root: &str,
) -> Vec<String> {
    let workspace_id = NodeId::Workspace(crate::model::WorkspaceId {
        root: workspace_root.to_string(),
    });
    let mut names: Vec<String> = snapshot
        .resolved_relationships
        .iter()
        .filter(|rel| {
            rel.relation == crate::model::RelationKind::WorkspaceContainsRepo
                && rel.source == workspace_id
        })
        .filter_map(|rel| {
            // ADR 0077: only resolved slots feed the member-name
            // lookup; no-winner slots have no link to anchor a
            // `logical_path` field on.
            let winner_id = rel.selected_link_id.as_deref()?;
            let link = snapshot
                .candidate_links
                .iter()
                .find(|cl| cl.id == winner_id)?;
            let logical_path = link.source_metadata.fields.get("logical_path")?.as_str()?;
            Path::new(logical_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Translate a harness key to the short label rendered in the row.
/// Delegates to the adapter registry so
/// `claude-code`'s `claude` collapse lives on
/// `crate::discovery::harness::ClaudeCodeAdapter::display_label`
/// rather than in a match table here.
/// The program a mux runs, by basename, for rows with no linked agent
/// to label them. Prefers what the active pane runs now
/// (`#{pane_current_command}`), then the program the pane was started
/// with, then the launch program of the slot pinning the mux, so a
/// freshly launched mux keeps its label even if the backend hasn't
/// reported the pane yet. `None` when nothing names a program.
pub(crate) fn mux_program(
    mux: &crate::model::MuxSessionNode,
    pin: Option<&crate::model::PinNode>,
) -> Option<String> {
    let start_program = mux
        .active_pane_start_command
        .as_deref()
        .and_then(|command| command.split_whitespace().next());
    mux.active_pane_command
        .as_deref()
        .and_then(program_name)
        .or_else(|| start_program.and_then(program_name))
        .or_else(|| pin.and_then(pin_program))
}

/// Harness each mux's pane runs, from `mux_contains_process` evidence.
/// The process walk sees through launch wrappers (`atelier exec`,
/// `mise exec`, `nix develop -c`), which tmux reports as the pane
/// command instead of the harness they spawn. The shallowest harness
/// process wins so a subagent never outranks the agent that started it.
pub(crate) fn mux_process_harnesses(
    snapshot: &crate::model::GraphSnapshot,
) -> std::collections::HashMap<NodeId, String> {
    use crate::model::{GraphNode, LinkState, RelationKind};

    let processes: std::collections::HashMap<NodeId, (i64, &str)> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::RuntimeProcess(process) => Some((
                NodeId::RuntimeProcess(process.id.clone()),
                (
                    process.depth.unwrap_or(i64::MAX),
                    process.harness_key.as_deref()?,
                ),
            )),
            _ => None,
        })
        .collect();
    let mut best: std::collections::HashMap<NodeId, (i64, &str)> = std::collections::HashMap::new();
    for link in &snapshot.candidate_links {
        if link.relation != RelationKind::MuxContainsProcess
            || !matches!(link.state, LinkState::Active)
        {
            continue;
        }
        let Some(&process) = link.target_node_id().and_then(|id| processes.get(id)) else {
            continue;
        };
        best.entry(link.source.clone())
            .and_modify(|current| *current = (*current).min(process))
            .or_insert(process);
    }
    best.into_iter()
        .map(|(mux, (_, harness))| (mux, harness.to_string()))
        .collect()
}

/// Display label of the harness an agentless mux runs: the observed
/// process-tree harness when there is one, else a harness binary named
/// in what the pane or its pin launched. The launch command is scanned
/// token by token because wrappers name the harness after their own
/// arguments (`atelier exec claude …`); it only counts while the pane
/// still runs that command's program, so a shell left behind after the
/// harness exits reads as the shell.
pub(crate) fn mux_program_harness(
    mux: &crate::model::MuxSessionNode,
    pin: Option<&crate::model::PinNode>,
    observed_harness: Option<&str>,
) -> Option<String> {
    use crate::discovery::harness::adapter_in_command;

    if let Some(harness) = observed_harness {
        return Some(harness_label(harness));
    }
    let current = mux.active_pane_command.as_deref().and_then(program_name);
    let start = mux
        .active_pane_start_command
        .as_deref()
        .filter(|command| !command.trim().is_empty());
    let start_program = start
        .and_then(|command| command.split_whitespace().next())
        .and_then(program_name);
    let adapter = match (current.as_deref(), start) {
        (Some(current), _) if start_program.as_deref() != Some(current) => {
            adapter_in_command([current])
        }
        (_, Some(start)) => adapter_in_command(start.split_whitespace()),
        (_, None) => pin.and_then(|pin| match pin.list("argv") {
            Some(argv) if !argv.is_empty() => adapter_in_command(argv.iter().map(String::as_str)),
            _ => pin_program(pin)
                .as_deref()
                .and_then(|program| adapter_in_command([program])),
        }),
    };
    adapter.map(|adapter| adapter.display_label().to_string())
}

/// The program a mux slot launches: the first token of its `argv`,
/// else the harness's default argv, else the harness key itself.
pub(crate) fn pin_program(pin: &crate::model::PinNode) -> Option<String> {
    if let Some(program) = pin.list("argv").and_then(|argv| argv.first()) {
        return program_name(program);
    }
    let harness = pin.text("harness")?;
    let default_argv = crate::discovery::harness::launch_argv_for(harness);
    match default_argv.first() {
        Some(program) => program_name(&program.to_string_lossy()),
        None => program_name(harness),
    }
}

/// Every slot in the snapshot, in `pins.toml` order.
pub(crate) fn pin_nodes(snapshot: &crate::model::GraphSnapshot) -> Vec<&crate::model::PinNode> {
    let mut pins: Vec<&crate::model::PinNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            crate::model::GraphNode::Pin(pin) => Some(pin),
            _ => None,
        })
        .collect();
    pins.sort_by_key(|pin| pin.order);
    pins
}

/// The pin marker each pinned entity wears: filled while the entity is
/// running, hollow once it is stopped or gone.
pub(crate) fn pinned_node_markers(
    snapshot: &crate::model::GraphSnapshot,
) -> std::collections::BTreeMap<NodeId, PinMarker> {
    pin_nodes(snapshot)
        .into_iter()
        .filter_map(|pin| pin.target_node.clone())
        .map(|node| {
            let marker =
                if snapshot.find_node(&node).is_some() && snapshot.presence(&node).is_none() {
                    PinMarker::Running
                } else {
                    PinMarker::Stopped
                };
            (node, marker)
        })
        .collect()
}

/// The entities pin slots point at, whatever their status.
pub(crate) fn pinned_node_set(
    snapshot: &crate::model::GraphSnapshot,
) -> std::collections::BTreeSet<NodeId> {
    pin_nodes(snapshot)
        .into_iter()
        .filter_map(|pin| pin.target_node.clone())
        .collect()
}

/// The graph node a row stands for, when it stands for one.
pub(crate) fn row_node(id: &RowId) -> Option<&NodeId> {
    match id {
        RowId::Group(node)
        | RowId::AgentSession(node)
        | RowId::MuxSession(node)
        | RowId::Pr(node)
        | RowId::Fork(node)
        | RowId::Nested { node, .. } => Some(node),
        _ => None,
    }
}

/// Keep the rows of pinned entities and the groups above them
/// (`pinned only`). The tree is depth-first, so a row stays when it or
/// a descendant is pinned.
fn retain_pinned_rows(tree: &mut RowTree, pinned: &std::collections::BTreeSet<NodeId>) {
    let rows = std::mem::take(&mut tree.rows);
    let keep: Vec<bool> = (0..rows.len())
        .map(|index| {
            let depth = rows[index].depth;
            rows[index..]
                .iter()
                .enumerate()
                .take_while(|(offset, row)| *offset == 0 || row.depth > depth)
                .any(|(_, row)| row_node(&row.id).is_some_and(|node| pinned.contains(node)))
        })
        .collect();
    tree.rows = rows
        .into_iter()
        .zip(keep)
        .filter_map(|(row, keep)| keep.then_some(row))
        .collect();
}

/// The node a slot points at when it is live in the graph.
pub(crate) fn live_pin_target(
    snapshot: &crate::model::GraphSnapshot,
    pin: &crate::model::PinNode,
) -> Option<NodeId> {
    if pin.status != crate::model::SlotStatus::Live {
        return None;
    }
    let id = pin.target_node.clone()?;
    snapshot.find_node(&id).is_some().then_some(id)
}

fn program_name(command: &str) -> Option<String> {
    let command = command.trim();
    let name = command.rsplit('/').next().unwrap_or(command);
    (!name.is_empty()).then(|| name.to_string())
}

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
/// renderer maps each bucket to a `Theme` style so the recency
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

// Shared row-assembly helpers (`agent_row`, `mux_indicator`,
// `session_matches_filter`, …) used by the per-view builders in
// `rows/{union,prs,forks}.rs` and `output/{prs,forks}.rs`.
//
// `rows/mux.rs::agent_row` is separate because it takes a different
// input struct (`AttachedAgent`).

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
            mux_host: None,
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
    /// active `Theme`. The mapping pulls the per-bucket `StyleSpec`
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
    let pinned_only =
        inputs.filter.pinned_only && matches!(inputs.view, View::Sessions | View::Mux | View::Prs);
    let pinned = pinned_only.then(|| pinned_node_set(inputs.snapshot));
    let mut tree = build_tree_for_view_unfiltered(inputs);
    if let Some(pinned) = pinned {
        retain_pinned_rows(&mut tree, &pinned);
    }
    tree
}

fn build_tree_for_view_unfiltered(inputs: TreeInputs<'_>) -> RowTree {
    let home = tree_home_dir();
    let now = Some(tree_current_unix_epoch());
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
            grouping: match grouping {
                crate::tui::Grouping::Prs(g) => g,
                _ => crate::tui::PrsGrouping::Checkout,
            },
            filter,
            now,
            cwd,
        }),
        View::Forks => forks::build_forks_tree(forks::ForksBuildInputs {
            snapshot,
            home: home.as_deref(),
        }),
        View::History => history::build_history_tree(history::HistoryBuildInputs {
            snapshot,
            home: home.as_deref(),
            now,
            show_pinned: filter.show_pinned_history,
            grouping: match grouping {
                crate::tui::Grouping::History(g) => g,
                _ => crate::tui::HistoryGrouping::Project,
            },
        }),
        View::Pinned => pinned::build_pinned_tree(pinned::PinnedBuildInputs {
            snapshot,
            home: home.as_deref(),
            now,
            grouping: match grouping {
                crate::tui::Grouping::Pinned(g) => g,
                _ => crate::tui::PinnedGrouping::Project,
            },
            sort,
        }),
    }
}

fn tree_home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Wall-clock unix epoch that view-model derivations (row trees, the
/// node explorer) use for relative ages.
/// A fixed time snapshot mode renders against (ADR 0113). Unset, the
/// TUI reads the clock.
static FROZEN_NOW: std::sync::OnceLock<i64> = std::sync::OnceLock::new();

/// Render ages against `epoch` for the rest of the process.
#[cfg(feature = "snapshot")]
pub(crate) fn freeze_clock(epoch: i64) {
    let _ = FROZEN_NOW.set(epoch);
}

pub(crate) fn tree_current_unix_epoch() -> i64 {
    FROZEN_NOW
        .get()
        .copied()
        .unwrap_or_else(crate::discovery::current_epoch)
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
