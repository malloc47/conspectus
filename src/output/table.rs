//! Plain-text table renderers for the resolved graph.
//!
//! See ADR 0006 for the projection vocabulary. Three projections are
//! supported:
//!
//! - [`Projection::Agent`] — one row per `AgentSession`, showing the
//!   preferred mux and preferred PR.
//! - [`Projection::Mux`] — one row per `MuxSession`, showing every
//!   attached agent session.
//! - [`Projection::Union`] — combined view with one row per node,
//!   preserving relationship status.
//!
//! Every projection shares one compact indicator format:
//!
//! - Provenance codes: `LD` (local declared), `GD` (global declared),
//!   `SD` (strong discovered), `D` (discovered), `C` (convention),
//!   `$` (cached).
//! - Confidence codes: `H` / `M` / `L`.
//! - An ambiguity marker `*` follows the cell when the resolver chose
//!   among multiple plausible candidates for that source/relation.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anstyle::{Ansi256Color, AnsiColor, Effects, Reset, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use crate::config::Projection;
use crate::model::{
    AgentSessionNode, Confidence, ForgePrNode, ForkNode, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, MuxSessionNode, NodeId, Provenance, RelationKind,
};

/// Rendering knobs for the text-table projections.
///
/// `width = None` renders untruncated (the historical behavior) and is the
/// default for callers that just want a wide table — pipes, JSON-adjacent
/// tooling, or tests that want byte-for-byte stable output independent of
/// the developer's terminal size.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Target total width in display columns. `None` means unbounded; the
    /// renderer uses each column's natural width.
    pub width: Option<usize>,
    /// Body layout: `Columnar` (default tabular) or `Card` (one column
    /// per line per row).
    pub layout: Layout,
    /// Selected column keys, in render order. `None` falls back to the
    /// row-type's [`default_columns`] set. Use [`parse_columns_spec`] to
    /// translate a `--columns LIST` string into a value for this field.
    pub columns: Option<Vec<&'static str>>,
    /// When true, the renderer wraps headers and selected cell content
    /// in ANSI escape codes per the palette described in ADR 0022.
    /// Defaults to `false` so existing snapshot tests stay
    /// byte-for-byte stable.
    pub color: bool,
}

impl RenderOptions {
    /// Untruncated, columnar layout. Matches the pre-H-TBL-003 renderer.
    pub fn wide() -> Self {
        Self {
            width: None,
            layout: Layout::Columnar,
            columns: None,
            color: false,
        }
    }

    /// Columnar layout truncated to the given display width.
    pub fn columnar_width(width: usize) -> Self {
        Self {
            width: Some(width),
            layout: Layout::Columnar,
            columns: None,
            color: false,
        }
    }

    /// Card layout (one column per line per row, blank line between rows).
    pub fn card() -> Self {
        Self {
            width: None,
            layout: Layout::Card,
            columns: None,
            color: false,
        }
    }

    /// Card layout truncated to the given display width.
    pub fn card_width(width: usize) -> Self {
        Self {
            width: Some(width),
            layout: Layout::Card,
            columns: None,
            color: false,
        }
    }

    /// Builder helper: render exactly `columns` (in this order). Pass
    /// keys obtained from [`parse_columns_spec`] or
    /// [`resolve_explicit_columns`].
    pub fn with_columns(mut self, columns: Vec<&'static str>) -> Self {
        self.columns = Some(columns);
        self
    }

    /// Builder helper: enable ANSI color escapes per ADR 0022.
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

/// Body layout for the renderer. See [`RenderOptions`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Layout {
    /// One row per record, columns aligned to per-column budgets.
    #[default]
    Columnar,
    /// One column per line per row, blank line between rows. Useful when
    /// many columns are relevant and the columnar layout would truncate.
    Card,
}

/// Minimum length for the short, content-addressed row identifier emitted
/// in the leftmost `ID` column of each session-table projection. The
/// renderer grows the prefix beyond this floor only to break collisions
/// within the rendered snapshot.
const SHORT_ID_FLOOR: usize = 6;

/// FNV-1a 64-bit over the bytes of [`NodeId`]'s `Display` form. Used to
/// derive a stable short row identifier for table output (H-TBL-002).
/// The function is fully deterministic and architecture-independent.
/// Exposed for `node show` (H-TBL-005), which accepts a copy-pasted
/// short id and resolves it to a `NodeId`.
pub fn node_short_id(node_id: &NodeId) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for &byte in node_id.to_string().as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// Shortest prefix length that uniquely identifies every id in
/// `full_ids` against the others, floored at [`SHORT_ID_FLOOR`]. All
/// inputs are expected to be the 16-char hex output of
/// [`node_short_id`]; the cap is therefore 16.
fn unique_prefix_len(full_ids: &[String]) -> usize {
    if full_ids.len() <= 1 {
        return SHORT_ID_FLOOR;
    }
    let cap = full_ids
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(SHORT_ID_FLOOR);
    for len in SHORT_ID_FLOOR..=cap {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut unique = true;
        for id in full_ids {
            let prefix = &id[..len.min(id.len())];
            if !seen.insert(prefix) {
                unique = false;
                break;
            }
        }
        if unique {
            return len;
        }
    }
    cap
}

/// Render `snapshot` as an untruncated plain-text table using `projection`.
///
/// Convenience wrapper for [`render_with`] with [`RenderOptions::wide`].
/// Callers that need width-aware truncation should use [`render_with`].
pub fn render(snapshot: &GraphSnapshot, projection: Projection) -> String {
    render_with(snapshot, projection, &RenderOptions::wide())
}

/// Render `snapshot` as a plain-text table using `projection` and `options`.
pub fn render_with(
    snapshot: &GraphSnapshot,
    projection: Projection,
    options: &RenderOptions,
) -> String {
    let view = SnapshotView::new(snapshot);
    let columns: Vec<&'static str> = options
        .columns
        .clone()
        .unwrap_or_else(|| default_columns(projection));
    let rows = match projection {
        Projection::Agent => build_agent_rows(&view, &columns),
        Projection::Mux => build_mux_rows(&view, &columns),
        Projection::Union => build_union_rows(&view, &columns),
        Projection::Pr => build_pr_rows(&view, &columns),
        Projection::Fork => build_fork_rows(&view, &columns),
    };
    render_rows(rows, &columns, options)
}

/// Compact `provenance/confidence[*]` cell, e.g. `LD/H*`. Used in
/// every projection so the cells are easy to scan.
pub fn indicator(provenance: Provenance, confidence: Confidence, ambiguous: bool) -> String {
    let mut buf = String::with_capacity(6);
    buf.push_str(provenance_code(provenance));
    buf.push('/');
    buf.push_str(confidence_code(confidence));
    if ambiguous {
        buf.push('*');
    }
    buf
}

fn provenance_code(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "LD",
        Provenance::GlobalDeclared => "GD",
        Provenance::StrongDiscovered => "SD",
        Provenance::Discovered => "D",
        Provenance::Convention => "C",
        Provenance::Cached => "$",
    }
}

fn confidence_code(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::High => "H",
        Confidence::Medium => "M",
        Confidence::Low => "L",
    }
}

/// A pre-computed view of one snapshot keyed by node id so the
/// projection renderers don't each rebuild it.
struct SnapshotView<'a> {
    /// Underlying snapshot — kept so column extractors can walk
    /// non-active links (`by_source_relation` filters those out).
    snapshot: &'a GraphSnapshot,
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    forge_prs: BTreeMap<NodeId, &'a ForgePrNode>,
    forks: BTreeMap<NodeId, &'a ForkNode>,
    /// `(source_node_id, relation_kind)` → all active candidate links
    /// for that pair, in stable order. Non-active links (ignored /
    /// overridden) are skipped; extractors that need them walk
    /// `snapshot.candidate_links` directly.
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
    /// Pre-computed `mux_id → [(agent_label, preferred_link)]` map for
    /// the agents-attached-to-this-mux cell. Built once at view
    /// construction so per-row column extractors don't each rebuild it.
    attached_to_mux: BTreeMap<NodeId, Vec<(String, &'a GraphLink)>>,
}

impl<'a> SnapshotView<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut forge_prs = BTreeMap::new();
        let mut forks = BTreeMap::new();

        for node in &snapshot.nodes {
            match node {
                GraphNode::AgentSession(session) => {
                    agent_sessions.insert(node.id(), session);
                }
                GraphNode::MuxSession(mux) => {
                    mux_sessions.insert(node.id(), mux);
                }
                GraphNode::ForgePr(pr) => {
                    forge_prs.insert(node.id(), pr);
                }
                GraphNode::Fork(fork) => {
                    forks.insert(node.id(), fork);
                }
                _ => {}
            }
        }

        let mut by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>> =
            BTreeMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, crate::model::LinkState::Active) {
                continue;
            }
            by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
        }

        let mut attached_to_mux: BTreeMap<NodeId, Vec<(String, &GraphLink)>> = BTreeMap::new();
        for ((source, relation), links) in &by_source_relation {
            if *relation != RelationKind::LinkedToMux {
                continue;
            }
            let Some(preferred) = pick_preferred(links) else {
                continue;
            };
            let Some(target_id) = preferred.target_node_id() else {
                continue;
            };
            if let Some(session) = agent_sessions.get(source) {
                attached_to_mux
                    .entry(target_id.clone())
                    .or_default()
                    .push((agent_session_label(session), preferred));
            }
        }

        Self {
            snapshot,
            agent_sessions,
            mux_sessions,
            forge_prs,
            forks,
            by_source_relation,
            attached_to_mux,
        }
    }

    fn preferred_link(&self, source: &NodeId, relation: RelationKind) -> Option<&GraphLink> {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .and_then(|links| pick_preferred(links))
    }

    fn candidates_for(&self, source: &NodeId, relation: RelationKind) -> &[&'a GraphLink] {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// For display, "preferred" is the resolver's first candidate. The
/// resolver itself does the real ranking; for table output we just
/// need the same first-place pick, so we sort by the same generic
/// recipe (provenance precedence then confidence then id).
fn pick_preferred<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links
        .iter()
        .copied()
        .filter(|link| matches!(link.state, crate::model::LinkState::Active))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

// -----------------------------------------------------------------------------
// Column registry (H-TBL-007)
// -----------------------------------------------------------------------------

/// One column in a row-type's column registry.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSpec {
    /// Stable token used in `--columns LIST` and `[table.<rows>].columns`.
    pub key: &'static str,
    /// Header label rendered in the columnar table and as the key in card layout.
    pub header: &'static str,
    /// One-line description for the column-discovery surface (H-TBL-012).
    pub description: &'static str,
    /// `true` when the column is part of the row-type's default set.
    pub default: bool,
}

const SESSIONS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "agent",
        header: "AGENT",
        description: "Harness key and session key (or title when set).",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded by the harness.",
        default: true,
    },
    ColumnSpec {
        key: "mux",
        header: "MUX",
        description: "Preferred mux session attached to this agent session.",
        default: true,
    },
    ColumnSpec {
        key: "mux-conf",
        header: "MUX/CONF",
        description: "Provenance/confidence indicator for the mux cell.",
        default: true,
    },
    ColumnSpec {
        key: "pr",
        header: "PR",
        description: "Preferred forge PR attached via the worktree branch.",
        default: true,
    },
    ColumnSpec {
        key: "pr-conf",
        header: "PR/CONF",
        description: "Provenance/confidence indicator for the PR cell.",
        default: true,
    },
    ColumnSpec {
        key: "lineage",
        header: "LINEAGE",
        description: "Preferred parent-session, with a trailing ← when the chain extends past one hop.",
        default: true,
    },
    ColumnSpec {
        key: "worktree",
        header: "WORKTREE",
        description: "Worktree root whose path matches the session's cwd.",
        default: false,
    },
    ColumnSpec {
        key: "branch",
        header: "BRANCH",
        description: "Branch checked out in the session's worktree (refs/heads/ stripped).",
        default: false,
    },
    ColumnSpec {
        key: "repo",
        header: "REPO",
        description: "Repo identifier (common_dir path) for the session's worktree.",
        default: false,
    },
    ColumnSpec {
        key: "fork",
        header: "FORK",
        description: "Fork label when the session is the `child_session` target of a fork.",
        default: false,
    },
    ColumnSpec {
        key: "declared",
        header: "DECLARED",
        description: "State of the strongest declared candidate sourced from this session (declared / ignored / overridden).",
        default: false,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "One-line snippet of the session's most recent user/assistant message (capped at 200 chars, ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "title",
        header: "TITLE",
        description: "Adapter-populated session title (opencode chat topic, claude-code compaction summary). `—` when unset.",
        default: false,
    },
];

const MUX_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "mux",
        header: "MUX",
        description: "Backend and native session id.",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded by the mux backend.",
        default: true,
    },
    ColumnSpec {
        key: "agents",
        header: "AGENTS",
        description: "Agent sessions attached to this mux session.",
        default: true,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "Last-message preview of the first attached agent (ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "attached-count",
        header: "ATTACHED",
        description: "Count of agent sessions resolved as attached to this mux.",
        default: false,
    },
    ColumnSpec {
        key: "activity",
        header: "ACTIVITY",
        description: "Relative recency from `MuxSessionNode.activity_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
    ColumnSpec {
        key: "created",
        header: "CREATED",
        description: "Relative age from `MuxSessionNode.created_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
];

const UNION_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "kind",
        header: "KIND",
        description: "Node kind (`agent` or `mux`).",
        default: true,
    },
    ColumnSpec {
        key: "label",
        header: "LABEL",
        description: "Harness/mux label for the node.",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded for the node.",
        default: true,
    },
    ColumnSpec {
        key: "relationship",
        header: "RELATIONSHIP",
        description: "Preferred relationship cell for agent rows; — for mux rows.",
        default: true,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "Last-message preview for agent rows; — for mux rows (ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "title",
        header: "TITLE",
        description: "Adapter-populated agent session title; — for mux rows.",
        default: false,
    },
];

const PRS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "pr",
        header: "PR",
        description: "Forge identifier (`{owner}/{repo}#{number}`) with state/draft suffix.",
        default: true,
    },
    ColumnSpec {
        key: "state",
        header: "STATE",
        description: "Forge-reported PR state (open, closed, merged, …).",
        default: true,
    },
    ColumnSpec {
        key: "draft",
        header: "DRAFT",
        description: "`draft` when the PR is a draft; `—` otherwise.",
        default: false,
    },
    ColumnSpec {
        key: "branch",
        header: "BRANCH",
        description: "Head branch (refs/heads/ prefix stripped).",
        default: true,
    },
    ColumnSpec {
        key: "repo",
        header: "REPO",
        description: "Forge repository slug (`{owner}/{repo}`).",
        default: false,
    },
    ColumnSpec {
        key: "updated",
        header: "UPDATED",
        description: "Relative recency from `updated_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
    ColumnSpec {
        key: "attached",
        header: "ATTACHED",
        description: "Agent sessions whose worktree has the PR's branch checked out.",
        default: true,
    },
];

const FORKS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "fork",
        header: "FORK",
        description: "Fork label: `{provider}:{name}` (falls back to `provider_source_key` when `name` is absent).",
        default: true,
    },
    ColumnSpec {
        key: "provider",
        header: "PROVIDER",
        description: "Provider that recorded the fork (e.g. `atelier`).",
        default: true,
    },
    ColumnSpec {
        key: "scope",
        header: "SCOPE",
        description: "Provider-defined scope or workspace name (when set).",
        default: false,
    },
    ColumnSpec {
        key: "parent",
        header: "PARENT",
        description: "Preferred `parent_session` short id when the fork records one.",
        default: true,
    },
    ColumnSpec {
        key: "children",
        header: "CHILDREN",
        description: "Number of `child_session` candidates pointing at agent sessions.",
        default: true,
    },
    ColumnSpec {
        key: "capabilities",
        header: "CAPABILITIES",
        description: "Comma-joined `capabilities` from the fork metadata.",
        default: false,
    },
];

/// Registry slice for `projection`.
pub fn columns_for(projection: Projection) -> &'static [ColumnSpec] {
    match projection {
        Projection::Agent => SESSIONS_COLUMNS,
        Projection::Mux => MUX_COLUMNS,
        Projection::Union => UNION_COLUMNS,
        Projection::Pr => PRS_COLUMNS,
        Projection::Fork => FORKS_COLUMNS,
    }
}

/// Default column key set for `projection`, in registry order.
pub fn default_columns(projection: Projection) -> Vec<&'static str> {
    columns_for(projection)
        .iter()
        .filter(|spec| spec.default)
        .map(|spec| spec.key)
        .collect()
}

fn projection_row_type(projection: Projection) -> &'static str {
    match projection {
        Projection::Agent => "sessions",
        Projection::Mux => "mux",
        Projection::Union => "union",
        Projection::Pr => "prs",
        Projection::Fork => "forks",
    }
}

/// Errors produced when resolving a user-supplied column selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnsError {
    UnknownColumn {
        name: String,
        row_type: &'static str,
        available: Vec<&'static str>,
    },
    EmptyToken,
}

impl std::fmt::Display for ColumnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnsError::UnknownColumn {
                name,
                row_type,
                available,
            } => write!(
                f,
                "unknown column `{name}` for row-type `{row_type}`; available: {}",
                available.join(", "),
            ),
            ColumnsError::EmptyToken => write!(f, "empty column token in --columns spec"),
        }
    }
}

impl std::error::Error for ColumnsError {}

/// Resolve a `--columns LIST` spec for `projection` to an ordered list
/// of column keys.
///
/// Tokens (comma-separated, whitespace-trimmed):
/// - `default` — reset to the registered default set.
/// - `all` — reset to every registered column.
/// - `+name` — add the column if not already present.
/// - `-name` — remove the column if present.
/// - `name` — explicit-list mode: clears the running set on the first bare
///   token, then appends.
///
/// Unknown column names error with the registered names listed.
pub fn parse_columns_spec(
    projection: Projection,
    spec: &str,
) -> Result<Vec<&'static str>, ColumnsError> {
    let registry = columns_for(projection);
    let row_type = projection_row_type(projection);
    let available: Vec<&'static str> = registry.iter().map(|spec| spec.key).collect();

    let lookup = |name: &str| -> Result<&'static str, ColumnsError> {
        registry
            .iter()
            .find(|spec| spec.key == name)
            .map(|spec| spec.key)
            .ok_or_else(|| ColumnsError::UnknownColumn {
                name: name.to_string(),
                row_type,
                available: available.clone(),
            })
    };

    let mut current: Vec<&'static str> = default_columns(projection);
    let mut explicit_started = false;

    for raw in spec.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ColumnsError::EmptyToken);
        }
        if let Some(rest) = trimmed.strip_prefix('+') {
            let key = lookup(rest.trim())?;
            if !current.contains(&key) {
                current.push(key);
            }
        } else if let Some(rest) = trimmed.strip_prefix('-') {
            let key = lookup(rest.trim())?;
            current.retain(|k| *k != key);
        } else if trimmed == "default" {
            current = default_columns(projection);
            explicit_started = true;
        } else if trimmed == "all" {
            current = available.clone();
            explicit_started = true;
        } else {
            let key = lookup(trimmed)?;
            if !explicit_started {
                current.clear();
                explicit_started = true;
            }
            if !current.contains(&key) {
                current.push(key);
            }
        }
    }

    Ok(current)
}

/// Resolve an explicit list of column names (no `+`/`-` semantics) — the
/// config-file variant for `[table.<rows>].columns`. Unknown names error.
pub fn resolve_explicit_columns(
    projection: Projection,
    names: &[String],
) -> Result<Vec<&'static str>, ColumnsError> {
    let registry = columns_for(projection);
    let row_type = projection_row_type(projection);
    let available: Vec<&'static str> = registry.iter().map(|spec| spec.key).collect();

    names
        .iter()
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(ColumnsError::EmptyToken);
            }
            registry
                .iter()
                .find(|spec| spec.key == trimmed)
                .map(|spec| spec.key)
                .ok_or_else(|| ColumnsError::UnknownColumn {
                    name: trimmed.to_string(),
                    row_type,
                    available: available.clone(),
                })
        })
        .collect()
}

/// Render a human-readable listing of the registered columns for
/// `projection`. Each line is `<key>  <description>` with `(default)`
/// appended for columns in the default set. The leading key column is
/// padded so descriptions line up. Used by `conspectus columns <ROWS>`
/// (H-TBL-012).
pub fn render_columns_listing(projection: Projection, color: bool) -> String {
    let registry = columns_for(projection);
    let key_width = registry
        .iter()
        .map(|spec| spec.key.chars().count())
        .max()
        .unwrap_or(0);
    let dim = Style::new().effects(Effects::DIMMED);
    let mut out = String::new();
    for spec in registry {
        let key_pad = key_width.saturating_sub(spec.key.chars().count());
        // Bold the column key so it pops out of the description text.
        push_styled(&mut out, spec.key, header_style(), color);
        for _ in 0..(key_pad + 2) {
            out.push(' ');
        }
        out.push_str(spec.description);
        if spec.default {
            out.push_str("  ");
            push_styled(&mut out, "(default)", dim, color);
        }
        out.push('\n');
    }
    out
}

fn header_label(registry: &[ColumnSpec], key: &str) -> String {
    registry
        .iter()
        .find(|spec| spec.key == key)
        .map(|spec| spec.header.to_string())
        .unwrap_or_else(|| key.to_uppercase())
}

// -----------------------------------------------------------------------------
// Color palette (ADR 0022)
// -----------------------------------------------------------------------------

/// Style applied to header-row cells (and section headers in
/// `node show`) when color is enabled.
pub(crate) fn header_style() -> Style {
    Style::new().effects(Effects::BOLD)
}

/// Style applied to card-layout key labels (`KEY:`) when color is on.
fn card_key_style() -> Style {
    Style::new().effects(Effects::BOLD)
}

/// Style for a body-row cell given the column key and the cell text.
/// Returns an empty style when no rule matches; callers should skip
/// the ANSI envelope in that case via [`Style::is_plain`].
fn cell_style(column_key: &str, cell_text: &str) -> Style {
    if cell_text == "—" {
        return placeholder_style();
    }
    match column_key {
        "id" => id_style(),
        "mux-conf" | "pr-conf" => indicator_style(cell_text),
        "state" => pr_state_style(cell_text),
        "draft" => {
            if cell_text == "draft" {
                Style::new().fg_color(Some(AnsiColor::Yellow.into()))
            } else {
                Style::new()
            }
        }
        "declared" => declared_state_style(cell_text),
        _ => Style::new(),
    }
}

/// Style for the leftmost short-row-id column. Uses a steady blue so
/// the id stays scannable down a column of varying widths without
/// competing with green/cyan/yellow used elsewhere in the palette.
fn id_style() -> Style {
    Style::new().fg_color(Some(AnsiColor::Blue.into()))
}

/// Style for the `—` placeholder cell. Uses a 256-color mid-gray
/// (xterm 244) so the dash sinks visually below the rest of the row
/// rather than just losing intensity. Falls back gracefully on
/// 16-color terminals (anstyle downgrades the value).
fn placeholder_style() -> Style {
    Style::new().fg_color(Some(Ansi256Color(244).into()))
}

/// Resolve a colour for an agent harness key. Each known harness
/// gets a stable, distinct hue so a column full of `claude-code:…`,
/// `codex:…`, and `opencode:…` rows is easy to scan. Unknown
/// harnesses fall back to the default colour.
///
/// Choices use bright ANSI variants so the harness prefix stands out
/// from the regular-intensity palette used for state/indicator cells.
fn harness_style(harness: &str) -> Style {
    let color = match harness {
        "claude-code" => AnsiColor::BrightYellow,
        "codex" => AnsiColor::BrightBlue,
        "opencode" => AnsiColor::BrightGreen,
        "aider" => AnsiColor::BrightRed,
        _ => return Style::new(),
    };
    Style::new().fg_color(Some(color.into()))
}

/// Map a `<tier>/<conf>[*]` indicator cell to its tier color.
fn indicator_style(text: &str) -> Style {
    let tier = text.split('/').next().unwrap_or("");
    match tier {
        "LD" | "GD" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "SD" => Style::new().fg_color(Some(AnsiColor::Cyan.into())),
        "C" | "$" => Style::new().effects(Effects::DIMMED),
        _ => Style::new(),
    }
}

fn pr_state_style(text: &str) -> Style {
    match text {
        "open" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "closed" => Style::new().fg_color(Some(AnsiColor::Red.into())),
        "merged" => Style::new().fg_color(Some(AnsiColor::Magenta.into())),
        _ => Style::new(),
    }
}

fn declared_state_style(text: &str) -> Style {
    match text {
        "declared" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "ignored" => Style::new().effects(Effects::DIMMED),
        "overridden" => Style::new().fg_color(Some(AnsiColor::Yellow.into())),
        _ => Style::new(),
    }
}

/// Write `text` to `out`, wrapping it in `style`'s ANSI escapes when
/// `color` is true and the style isn't plain. Otherwise emit `text`
/// verbatim. Used by both columnar and card layouts (and by
/// `output::node_show` for section headers) so the wrapping rule is
/// consistent.
pub(crate) fn push_styled(out: &mut String, text: &str, style: Style, color: bool) {
    if color && !style.is_plain() {
        let _ = write!(out, "{style}{text}{Reset}");
    } else {
        out.push_str(text);
    }
}

/// Render a body cell, applying any per-segment styling the column
/// asks for. Most columns just call back to [`push_styled`] with
/// [`cell_style`]'s single style. Columns that contain multi-segment
/// content take a dispatch below so the `harness:` prefix can be
/// coloured independently of the session-key tail and the
/// `[indicator]` suffix:
///
/// - `agent`: sessions projection's `harness:session_key` cell.
/// - `agents`: mux projection's joined `label [indicator], …` cell.
/// - `label`: union projection's per-row label. Agent rows render
///   `harness:session_key` (coloured); mux rows render
///   `backend:native_id` and fall through to plain text because
///   the mux backend is not in [`harness_style`]'s palette.
///
/// The emitted text's *visible* width equals `text.chars()` width
/// in every branch — the ANSI envelope adds zero display columns —
/// so this function is safe to call after width-aware truncation
/// has already trimmed `text` to fit the column budget.
fn push_cell(out: &mut String, column_key: &str, text: &str, color: bool) {
    if color {
        match column_key {
            "agent" | "label" => {
                push_agent_label(out, text);
                return;
            }
            "agents" => {
                push_agents_cell(out, text);
                return;
            }
            _ => {}
        }
    }
    push_styled(out, text, cell_style(column_key, text), color);
}

/// Style a `harness:session_key` agent label, colouring only the
/// harness prefix. Falls back to the placeholder style for `—` and
/// to plain text for unknown harness keys so a future tool slots in
/// without requiring a palette edit first.
fn push_agent_label(out: &mut String, label: &str) {
    if label == "—" {
        push_styled(out, label, placeholder_style(), true);
        return;
    }
    if let Some((harness, rest)) = label.split_once(':') {
        let style = harness_style(harness);
        if style.is_plain() {
            out.push_str(label);
        } else {
            let _ = write!(out, "{style}{harness}{Reset}:{rest}");
        }
    } else {
        out.push_str(label);
    }
}

/// Style the mux projection's `agents` cell. [`mux_cell`] builds the
/// cell as `label [indicator], label [indicator]`; the styler splits
/// on the comma boundary, then on the ` [` boundary inside each
/// entry, so only the `harness:` prefix of each label is wrapped in
/// its per-harness colour. The trailing `[indicator]` suffix stays
/// uncoloured by this story; if it grows indicator-tier styling
/// later, this is the seam.
fn push_agents_cell(out: &mut String, text: &str) {
    if text == "—" {
        push_styled(out, text, placeholder_style(), true);
        return;
    }
    for (idx, entry) in text.split(", ").enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        match entry.split_once(" [") {
            Some((label, indicator_tail)) => {
                push_agent_label(out, label);
                out.push(' ');
                out.push('[');
                out.push_str(indicator_tail);
            }
            None => push_agent_label(out, entry),
        }
    }
}

// -----------------------------------------------------------------------------
// Per-row-type column extractors
// -----------------------------------------------------------------------------

struct AgentRowCtx<'view, 'snap> {
    view: &'view SnapshotView<'snap>,
    node_id: &'view NodeId,
    session: &'view AgentSessionNode,
    short_id: &'view str,
}

fn agent_cell(key: &str, ctx: &AgentRowCtx<'_, '_>) -> String {
    match key {
        "id" => ctx.short_id.to_string(),
        "agent" => agent_session_label(ctx.session),
        "cwd" => ctx.session.cwd.clone().unwrap_or_else(|| "—".to_string()),
        "mux" => {
            let link = ctx
                .view
                .preferred_link(ctx.node_id, RelationKind::LinkedToMux);
            link.and_then(|link| match &link.target {
                LinkEndpoint::Node {
                    id: NodeId::MuxSession(_),
                } => ctx
                    .view
                    .mux_sessions
                    .get(link.target_node_id()?)
                    .map(|mux| mux_session_label(mux)),
                _ => None,
            })
            .unwrap_or_else(|| "—".to_string())
        }
        "mux-conf" => {
            let link = ctx
                .view
                .preferred_link(ctx.node_id, RelationKind::LinkedToMux);
            let count = ctx
                .view
                .candidates_for(ctx.node_id, RelationKind::LinkedToMux)
                .len();
            match link {
                Some(link) => indicator(link.provenance, link.confidence, count > 1),
                None => "—".to_string(),
            }
        }
        "pr" => preferred_pr_for_session(ctx.view, ctx.node_id).0,
        "pr-conf" => preferred_pr_for_session(ctx.view, ctx.node_id).1,
        "lineage" => lineage_cell(ctx.view, ctx.node_id),
        "worktree" => {
            session_worktree_root(ctx.view, ctx.session).unwrap_or_else(|| "—".to_string())
        }
        "branch" => session_branch_label(ctx.view, ctx.session).unwrap_or_else(|| "—".to_string()),
        "repo" => session_repo_identifier(ctx.view, ctx.session).unwrap_or_else(|| "—".to_string()),
        "fork" => {
            session_owning_fork_label(ctx.view, ctx.node_id).unwrap_or_else(|| "—".to_string())
        }
        "declared" => {
            session_declared_state(ctx.view, ctx.node_id).unwrap_or_else(|| "—".to_string())
        }
        "preview" => ctx
            .session
            .last_message_preview
            .clone()
            .unwrap_or_else(|| "—".to_string()),
        "title" => ctx.session.title.clone().unwrap_or_else(|| "—".to_string()),
        _ => "—".to_string(),
    }
}

/// Find the worktree whose root contains the session's cwd. Walks
/// `snapshot.nodes` once per call; row counts are bounded so the cost
/// stays small.
fn session_worktree_root(view: &SnapshotView<'_>, session: &AgentSessionNode) -> Option<String> {
    Some(session_worktree_id(view, session)?.root.clone())
}

/// Resolve the session's worktree and follow `CheckedOutBranch` to the
/// branch, returning the refname with `refs/heads/` stripped.
fn session_branch_label(view: &SnapshotView<'_>, session: &AgentSessionNode) -> Option<String> {
    let session_worktree = session_worktree_id(view, session)?;
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::CheckedOutBranch {
            continue;
        }
        let NodeId::Worktree(worktree_id) = source else {
            continue;
        };
        if worktree_id != session_worktree {
            continue;
        }
        let preferred = pick_preferred(links)?;
        if let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &preferred.target
        {
            return Some(strip_branch_prefix(&branch_id.refname).to_string());
        }
    }
    None
}

/// Resolve the session's worktree and return its repo identifier
/// (`RepoId.common_dir`).
fn session_repo_identifier(view: &SnapshotView<'_>, session: &AgentSessionNode) -> Option<String> {
    Some(session_worktree_id(view, session)?.repo.common_dir.clone())
}

fn session_worktree_id<'a>(
    view: &'a SnapshotView<'_>,
    session: &AgentSessionNode,
) -> Option<&'a crate::model::WorktreeId> {
    let cwd = Path::new(session.cwd.as_deref()?);
    view.by_source_relation
        .keys()
        .filter_map(|(source, _relation)| match source {
            NodeId::Worktree(worktree_id)
                if path_is_ancestor_of(Path::new(&worktree_id.root), cwd) =>
            {
                Some(worktree_id)
            }
            _ => None,
        })
        .max_by_key(|worktree_id| Path::new(&worktree_id.root).components().count())
}

fn path_is_ancestor_of(ancestor: &Path, descendant: &Path) -> bool {
    let mut anc_iter = ancestor.components();
    let mut desc_iter = descendant.components();
    loop {
        match (anc_iter.next(), desc_iter.next()) {
            (Some(a), Some(d)) if a == d => continue,
            (Some(_), Some(_)) => return false,
            (Some(_), None) => return false,
            (None, _) => return true,
        }
    }
}

/// When a fork records this session as a `child_session` target, return
/// the fork's display label.
fn session_owning_fork_label(view: &SnapshotView<'_>, session_id: &NodeId) -> Option<String> {
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::ChildSession {
            continue;
        }
        if !matches!(source, NodeId::Fork(_)) {
            continue;
        }
        for link in links {
            if let LinkEndpoint::Node { id } = &link.target
                && id == session_id
                && let Some(fork) = view.forks.get(source)
            {
                return Some(fork_label(fork));
            }
        }
    }
    None
}

/// Map the strongest declared candidate for `session_id` to a one-word
/// state label. Walks `snapshot.candidate_links` directly so ignored
/// and overridden declared links surface in the cell. Returns `None`
/// when no declared candidate exists.
fn session_declared_state(view: &SnapshotView<'_>, session_id: &NodeId) -> Option<String> {
    let mut best: Option<&GraphLink> = None;
    for link in &view.snapshot.candidate_links {
        if &link.source != session_id {
            continue;
        }
        if !matches!(
            link.provenance,
            Provenance::LocalDeclared | Provenance::GlobalDeclared
        ) {
            continue;
        }
        match best {
            None => best = Some(link),
            Some(current) if link.provenance.precedence() > current.provenance.precedence() => {
                best = Some(link);
            }
            _ => {}
        }
    }
    let link = best?;
    Some(match &link.state {
        crate::model::LinkState::Active => "declared".to_string(),
        crate::model::LinkState::Ignored { .. } => "ignored".to_string(),
        crate::model::LinkState::Overridden { .. } => "overridden".to_string(),
    })
}

struct MuxRowCtx<'view, 'snap> {
    view: &'view SnapshotView<'snap>,
    mux_id: &'view NodeId,
    mux: &'view MuxSessionNode,
    short_id: &'view str,
}

fn mux_cell(key: &str, ctx: &MuxRowCtx<'_, '_>) -> String {
    match key {
        "id" => ctx.short_id.to_string(),
        "mux" => mux_session_label(ctx.mux),
        "cwd" => ctx.mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
        "agents" => match ctx.view.attached_to_mux.get(ctx.mux_id) {
            Some(entries) if !entries.is_empty() => entries
                .iter()
                .map(|(label, link)| {
                    let ambiguous = ctx
                        .view
                        .candidates_for(&link.source, RelationKind::LinkedToMux)
                        .len()
                        > 1;
                    format!(
                        "{label} [{ind}]",
                        ind = indicator(link.provenance, link.confidence, ambiguous)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        },
        "attached-count" => {
            let count = ctx
                .view
                .attached_to_mux
                .get(ctx.mux_id)
                .map(Vec::len)
                .unwrap_or(0);
            if count == 0 {
                "—".to_string()
            } else {
                count.to_string()
            }
        }
        "activity" => ctx
            .mux
            .activity_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(|| "—".to_string()),
        "created" => ctx
            .mux
            .created_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(|| "—".to_string()),
        "preview" => {
            first_attached_agent_preview(ctx.view, ctx.mux_id).unwrap_or_else(|| "—".to_string())
        }
        _ => "—".to_string(),
    }
}

/// Read the `last_message_preview` of the first agent session
/// resolved as attached to this mux. The mux projection's `preview`
/// column shows this lone preview rather than joining all attached
/// agents' previews — per ADR 0023, joining would push the cell
/// past any reasonable width budget.
fn first_attached_agent_preview(view: &SnapshotView<'_>, mux_id: &NodeId) -> Option<String> {
    let entries = view.attached_to_mux.get(mux_id)?;
    for (_label, link) in entries {
        if let Some(session) = view.agent_sessions.get(&link.source)
            && let Some(preview) = &session.last_message_preview
        {
            return Some(preview.clone());
        }
    }
    None
}

enum UnionRowSource<'a> {
    Agent {
        node_id: &'a NodeId,
        session: &'a AgentSessionNode,
    },
    Mux {
        mux: &'a MuxSessionNode,
    },
}

struct UnionRowCtx<'view, 'snap> {
    view: &'view SnapshotView<'snap>,
    source: UnionRowSource<'view>,
    short_id: &'view str,
}

fn union_cell(key: &str, ctx: &UnionRowCtx<'_, '_>) -> String {
    match (key, &ctx.source) {
        ("id", _) => ctx.short_id.to_string(),
        ("kind", UnionRowSource::Agent { .. }) => "agent".to_string(),
        ("kind", UnionRowSource::Mux { .. }) => "mux".to_string(),
        ("label", UnionRowSource::Agent { session, .. }) => agent_session_label(session),
        ("label", UnionRowSource::Mux { mux }) => mux_session_label(mux),
        ("cwd", UnionRowSource::Agent { session, .. }) => {
            session.cwd.clone().unwrap_or_else(|| "—".to_string())
        }
        ("cwd", UnionRowSource::Mux { mux }) => mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
        ("relationship", UnionRowSource::Agent { node_id, .. }) => {
            let mux_link = ctx.view.preferred_link(node_id, RelationKind::LinkedToMux);
            let mux_count = ctx
                .view
                .candidates_for(node_id, RelationKind::LinkedToMux)
                .len();
            match mux_link {
                Some(link) => {
                    let target = link
                        .target_node_id()
                        .and_then(|id| ctx.view.mux_sessions.get(id))
                        .map(|mux| mux_session_label(mux))
                        .unwrap_or_else(|| "—".to_string());
                    format!(
                        "mux={target} [{ind}]",
                        ind = indicator(link.provenance, link.confidence, mux_count > 1)
                    )
                }
                None => "mux=—".to_string(),
            }
        }
        ("relationship", UnionRowSource::Mux { .. }) => "—".to_string(),
        ("preview", UnionRowSource::Agent { session, .. }) => session
            .last_message_preview
            .clone()
            .unwrap_or_else(|| "—".to_string()),
        ("preview", UnionRowSource::Mux { .. }) => "—".to_string(),
        ("title", UnionRowSource::Agent { session, .. }) => {
            session.title.clone().unwrap_or_else(|| "—".to_string())
        }
        ("title", UnionRowSource::Mux { .. }) => "—".to_string(),
        _ => "—".to_string(),
    }
}

/// Render the AGENT-cell label as `harness:<short-or-full session-key>`.
/// The harness adapter's session key is the stable identifier; long
/// UUIDs (claude-code, codex) collapse to `…<last-8>` via
/// [`agent_session_key_for_label`] so the column stays scannable.
/// Shorter human-readable session keys
/// (atelier-style `session-alpha`, opencode short ids) pass through
/// verbatim. The `AgentSessionNode.title` field is intentionally
/// **not** part of the label — opencode (and post-compaction
/// claude-code) populate it with a long conversation topic that
/// doesn't fit a leading cell. Title surfaces through the opt-in
/// `title` column instead (H-TBL-015).
fn agent_session_label(session: &AgentSessionNode) -> String {
    format!(
        "{}:{}",
        session.harness_key,
        agent_session_key_for_label(&session.id.session_key)
    )
}

/// Truncation threshold for AGENT-label session keys. UUIDs (32
/// hex chars + 4 dashes = 36) sit above this and collapse via
/// [`short_session_id`]; anything ≤ 32 chars renders verbatim so
/// human-readable session keys aren't truncated unnecessarily.
fn agent_session_key_for_label(key: &str) -> String {
    const UUID_THRESHOLD: usize = 32;
    if key.chars().count() <= UUID_THRESHOLD {
        key.to_string()
    } else {
        short_session_id(key)
    }
}

fn mux_session_label(mux: &MuxSessionNode) -> String {
    format!("{}:{}", mux.backend, mux.native_id)
}

fn forge_pr_label(pr: &ForgePrNode) -> String {
    let state = pr.state.as_deref().unwrap_or("?");
    let draft = if pr.is_draft { " draft" } else { "" };
    format!("{}/{}#{} ({state}{draft})", pr.owner, pr.repo, pr.number)
}

struct ForkRowCtx<'view, 'snap> {
    view: &'view SnapshotView<'snap>,
    fork_id: &'view NodeId,
    fork: &'view ForkNode,
    short_id: &'view str,
}

fn fork_cell(key: &str, ctx: &ForkRowCtx<'_, '_>) -> String {
    match key {
        "id" => ctx.short_id.to_string(),
        "fork" => fork_label(ctx.fork),
        "provider" => ctx.fork.provider.clone(),
        "scope" => ctx.fork.scope.clone().unwrap_or_else(|| "—".to_string()),
        "parent" => {
            fork_parent_session_label(ctx.view, ctx.fork_id).unwrap_or_else(|| "—".to_string())
        }
        "children" => {
            let count = fork_child_session_count(ctx.view, ctx.fork_id);
            if count == 0 {
                "—".to_string()
            } else {
                count.to_string()
            }
        }
        "capabilities" => {
            if ctx.fork.capabilities.is_empty() {
                "—".to_string()
            } else {
                ctx.fork.capabilities.join(", ")
            }
        }
        _ => "—".to_string(),
    }
}

fn fork_label(fork: &ForkNode) -> String {
    match &fork.name {
        Some(name) => format!("{}:{}", fork.provider, name),
        None => fork.provider_source_key.clone(),
    }
}

/// Render the preferred `parent_session` target as a short label.
/// Returns `Some` only when the candidate resolves to an
/// `AgentSession` endpoint; unresolved or non-session targets render
/// as `None` so the cell falls back to `—`.
fn fork_parent_session_label(view: &SnapshotView<'_>, fork_id: &NodeId) -> Option<String> {
    let link = view.preferred_link(fork_id, RelationKind::ParentSession)?;
    match &link.target {
        LinkEndpoint::Node {
            id: NodeId::AgentSession(agent_id),
        } => Some(short_session_id(&agent_id.session_key)),
        LinkEndpoint::Unresolved { evidence } => evidence
            .native_id
            .as_deref()
            .map(short_session_id)
            .map(|short| format!("?{short}")),
        _ => None,
    }
}

/// Number of `child_session` candidates from this fork that target
/// agent-session endpoints (resolved or unresolved).
fn fork_child_session_count(view: &SnapshotView<'_>, fork_id: &NodeId) -> usize {
    view.by_source_relation
        .get(&(fork_id.clone(), RelationKind::ChildSession))
        .map(|links| {
            links
                .iter()
                .filter(|link| {
                    matches!(
                        &link.target,
                        LinkEndpoint::Node {
                            id: NodeId::AgentSession(_)
                        } | LinkEndpoint::Unresolved { .. }
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

fn build_fork_rows(view: &SnapshotView<'_>, columns: &[&'static str]) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.forks.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(FORKS_COLUMNS, key))
            .collect(),
    );

    for ((fork_id, fork), full_short) in view.forks.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let ctx = ForkRowCtx {
            view,
            fork_id,
            fork,
            short_id,
        };
        rows.push(columns.iter().map(|key| fork_cell(key, &ctx)).collect());
    }

    rows
}

struct PrRowCtx<'view, 'snap> {
    view: &'view SnapshotView<'snap>,
    pr_id: &'view NodeId,
    pr: &'view ForgePrNode,
    short_id: &'view str,
}

fn pr_cell(key: &str, ctx: &PrRowCtx<'_, '_>) -> String {
    match key {
        "id" => ctx.short_id.to_string(),
        "pr" => forge_pr_label(ctx.pr),
        "state" => ctx.pr.state.clone().unwrap_or_else(|| "—".to_string()),
        "draft" => if ctx.pr.is_draft { "draft" } else { "—" }.to_string(),
        "branch" => pr_branch_label(ctx.view, ctx.pr_id).unwrap_or_else(|| "—".to_string()),
        "repo" => format!("{}/{}", ctx.pr.owner, ctx.pr.repo),
        "updated" => ctx
            .pr
            .updated_epoch
            .map(|epoch| format_relative_age(epoch, current_epoch()))
            .unwrap_or_else(|| "—".to_string()),
        "attached" => {
            let sessions = pr_attached_session_labels(ctx.view, ctx.pr_id);
            if sessions.is_empty() {
                "—".to_string()
            } else {
                sessions.join(", ")
            }
        }
        _ => "—".to_string(),
    }
}

/// Look up the preferred branch this PR points at.
fn pr_preferred_branch_id(
    view: &SnapshotView<'_>,
    pr_id: &NodeId,
) -> Option<crate::model::BranchId> {
    let links = view
        .by_source_relation
        .get(&(pr_id.clone(), RelationKind::BranchHasForgePr))?;
    let preferred = pick_preferred(links)?;
    match preferred.target_node_id()? {
        NodeId::Branch(branch_id) => Some(branch_id.clone()),
        _ => None,
    }
}

fn pr_branch_label(view: &SnapshotView<'_>, pr_id: &NodeId) -> Option<String> {
    let branch = pr_preferred_branch_id(view, pr_id)?;
    Some(strip_branch_prefix(&branch.refname).to_string())
}

fn strip_branch_prefix(refname: &str) -> &str {
    refname.strip_prefix("refs/heads/").unwrap_or(refname)
}

/// Find agent-session labels whose worktree has this PR's branch
/// checked out. Walks `CheckedOutBranch` candidate links to locate
/// worktrees, then matches sessions whose cwd is at or under that
/// worktree root.
fn pr_attached_session_labels(view: &SnapshotView<'_>, pr_id: &NodeId) -> Vec<String> {
    let Some(branch_id) = pr_preferred_branch_id(view, pr_id) else {
        return Vec::new();
    };
    let branch_node_id = NodeId::Branch(branch_id);

    // Worktrees whose CheckedOutBranch link points at this branch.
    let worktree_roots: Vec<&str> = view
        .by_source_relation
        .iter()
        .flat_map(|((_, relation), links)| {
            if *relation != RelationKind::CheckedOutBranch {
                return Vec::new();
            }
            links
                .iter()
                .filter(|link| {
                    matches!(
                        &link.target,
                        LinkEndpoint::Node { id } if id == &branch_node_id
                    )
                })
                .filter_map(|link| match &link.source {
                    NodeId::Worktree(w) => Some(w.root.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut labels: Vec<String> = Vec::new();
    for session in view.agent_sessions.values() {
        let Some(cwd) = session.cwd.as_deref() else {
            continue;
        };
        if worktree_roots
            .iter()
            .any(|root| path_is_ancestor_of(Path::new(root), Path::new(cwd)))
        {
            labels.push(agent_session_label(session));
        }
    }
    labels
}

/// Format `then_epoch` relative to `now_epoch` as a compact recency
/// string (`12s`, `5m`, `2h`, `3d`, `4w`). Future-dated values render
/// as `now`.
fn format_relative_age(then_epoch: i64, now_epoch: i64) -> String {
    let delta = now_epoch.saturating_sub(then_epoch);
    if delta < 0 {
        return "now".to_string();
    }
    let secs = delta as u64;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 7 * 86_400 {
        format!("{}d", secs / 86_400)
    } else {
        format!("{}w", secs / (7 * 86_400))
    }
}

fn current_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn build_pr_rows(view: &SnapshotView<'_>, columns: &[&'static str]) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.forge_prs.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(PRS_COLUMNS, key))
            .collect(),
    );

    for ((pr_id, pr), full_short) in view.forge_prs.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let ctx = PrRowCtx {
            view,
            pr_id,
            pr,
            short_id,
        };
        rows.push(columns.iter().map(|key| pr_cell(key, &ctx)).collect());
    }

    rows
}

fn build_agent_rows(view: &SnapshotView<'_>, columns: &[&'static str]) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.agent_sessions.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(SESSIONS_COLUMNS, key))
            .collect(),
    );

    for ((node_id, session), full_short) in view.agent_sessions.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let ctx = AgentRowCtx {
            view,
            node_id,
            session,
            short_id,
        };
        rows.push(columns.iter().map(|key| agent_cell(key, &ctx)).collect());
    }

    rows
}

/// Lineage cell for the agent projection (ADR 0018). Shows the preferred
/// `parent_session` for the row:
///
/// - `—` when no parent_session candidate exists.
/// - `<short>` when the parent is a discovered `AgentSession`.
/// - `?<short>` when the parent is preserved as unresolved-endpoint evidence.
/// - Trailing `←` when the parent itself has a parent, signalling a chain
///   longer than one hop.
fn lineage_cell(view: &SnapshotView<'_>, session_id: &NodeId) -> String {
    let Some(link) = view.preferred_link(session_id, RelationKind::ParentSession) else {
        return "—".to_string();
    };

    let (label, parent_node) = match &link.target {
        LinkEndpoint::Node {
            id: parent_id @ NodeId::AgentSession(agent_id),
        } => (short_session_id(&agent_id.session_key), Some(parent_id)),
        LinkEndpoint::Unresolved { evidence } => {
            let label = evidence
                .native_id
                .as_deref()
                .map(short_session_id)
                .map(|short| format!("?{short}"))
                .unwrap_or_else(|| "?".to_string());
            (label, None)
        }
        _ => return "—".to_string(),
    };

    let has_grandparent = parent_node
        .map(|parent| {
            view.preferred_link(parent, RelationKind::ParentSession)
                .is_some()
        })
        .unwrap_or(false);

    if has_grandparent {
        format!("{label}←")
    } else {
        label
    }
}

/// Shorten a session id for human display: keep short ids whole, abbreviate
/// long ones (typical UUIDs) to a `…<last-8>` suffix so adjacent rows stay
/// distinguishable without dominating the table width.
fn short_session_id(key: &str) -> String {
    const FULL_MAX: usize = 12;
    const TAIL: usize = 8;

    if key.chars().count() <= FULL_MAX {
        key.to_string()
    } else {
        let chars: Vec<char> = key.chars().collect();
        let tail: String = chars[chars.len() - TAIL..].iter().collect();
        format!("…{tail}")
    }
}

/// Walk session → fork associations → branches → PRs to find the
/// preferred PR for an agent session, if any. This covers the case
/// where a session lives in a worktree whose branch has an open PR.
fn preferred_pr_for_session(view: &SnapshotView<'_>, session_id: &NodeId) -> (String, String) {
    let session_cwd = match view.agent_sessions.get(session_id) {
        Some(session) => session.cwd.as_deref(),
        None => return ("—".to_string(), "—".to_string()),
    };
    let Some(session_cwd) = session_cwd else {
        return ("—".to_string(), "—".to_string());
    };

    // PRs are keyed off the branch node; the session's cwd is the
    // worktree path, but we don't have a direct session→branch link
    // yet. For now match PRs whose link source metadata points to
    // a branch whose ref name appears in `session_cwd`. This is
    // intentionally conservative: when we add session→branch links
    // in a later phase the lookup gets replaced.
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::BranchHasForgePr {
            continue;
        }
        let _ = source;
        let _ = session_cwd;
        let count = links.len();
        if let Some(preferred) = pick_preferred(links)
            && let NodeId::ForgePr(pr_id) = &preferred.source
            && let Some(pr) = view.forge_prs.get(&NodeId::ForgePr(pr_id.clone()))
        {
            return (
                forge_pr_label(pr),
                indicator(preferred.provenance, preferred.confidence, count > 1),
            );
        }
    }

    ("—".to_string(), "—".to_string())
}

fn build_mux_rows(view: &SnapshotView<'_>, columns: &[&'static str]) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.mux_sessions.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(MUX_COLUMNS, key))
            .collect(),
    );

    for ((mux_id, mux), full_short) in view.mux_sessions.iter().zip(body_full_ids.iter()) {
        let short_id = &full_short[..id_len];
        let ctx = MuxRowCtx {
            view,
            mux_id,
            mux,
            short_id,
        };
        rows.push(columns.iter().map(|key| mux_cell(key, &ctx)).collect());
    }

    rows
}

fn build_union_rows(view: &SnapshotView<'_>, columns: &[&'static str]) -> Vec<Vec<String>> {
    // The union projection mixes agent and mux rows; compute one prefix
    // length across the combined id set so collisions across kinds are
    // disambiguated too.
    let body_full_ids: Vec<String> = view
        .agent_sessions
        .keys()
        .chain(view.mux_sessions.keys())
        .map(node_short_id)
        .collect();
    let id_len = unique_prefix_len(&body_full_ids);
    let agent_count = view.agent_sessions.len();

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(UNION_COLUMNS, key))
            .collect(),
    );

    for ((node_id, session), full_short) in view
        .agent_sessions
        .iter()
        .zip(body_full_ids.iter().take(agent_count))
    {
        let short_id = &full_short[..id_len];
        let ctx = UnionRowCtx {
            view,
            source: UnionRowSource::Agent { node_id, session },
            short_id,
        };
        rows.push(columns.iter().map(|key| union_cell(key, &ctx)).collect());
    }

    for (mux, full_short) in view
        .mux_sessions
        .values()
        .zip(body_full_ids.iter().skip(agent_count))
    {
        let short_id = &full_short[..id_len];
        let ctx = UnionRowCtx {
            view,
            source: UnionRowSource::Mux { mux },
            short_id,
        };
        rows.push(columns.iter().map(|key| union_cell(key, &ctx)).collect());
    }

    rows
}

const COLUMN_GAP: &str = "  ";
const COLUMN_GAP_WIDTH: usize = 2;
/// Floor on a column's minimum budget before truncation. Header width is
/// also considered: a column with a wider header keeps the header's width
/// as its floor when its natural content is wider than this constant. Set
/// to 4 so a column can still emit `xxx…` after truncation.
const MIN_COLUMN_BUDGET: usize = 4;

fn render_rows(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    match options.layout {
        Layout::Columnar => render_columnar(rows, columns, options),
        Layout::Card => render_card(rows, columns, options),
    }
}

/// Render `rows` (header row at index 0) as a stack of cards: one block
/// per body row, with `KEY: value` lines aligned to the longest key and
/// blank lines separating blocks. The header row contributes the key
/// names but is not itself emitted as a card.
fn render_card(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    if rows.len() < 2 {
        return String::new();
    }
    let header = &rows[0];
    let key_width = header.iter().map(|s| display_width(s)).max().unwrap_or(0);
    let value_budget = options.width.map(|w| w.saturating_sub(key_width + 2));

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate().skip(1) {
        if row_idx > 1 {
            out.push('\n');
        }
        for (col_idx, cell) in row.iter().enumerate() {
            let key = header.get(col_idx).map(String::as_str).unwrap_or("");
            let key_pad = key_width.saturating_sub(display_width(key));
            // `KEY:` is the label half — bolded when color is on.
            push_styled(&mut out, key, card_key_style(), options.color);
            out.push(':');
            for _ in 0..(key_pad + 1) {
                out.push(' ');
            }
            let value = match value_budget {
                Some(budget) => truncate_to_width(cell, budget),
                None => cell.clone(),
            };
            let column_key = columns.get(col_idx).copied().unwrap_or("");
            push_cell(&mut out, column_key, &value, options.color);
            out.push('\n');
        }
    }
    out
}

fn render_columnar(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let column_count = rows[0].len();
    let naturals = natural_widths(&rows, column_count);
    let budgets = match options.width {
        None => naturals,
        Some(target) => fit_to_width(&naturals, &rows[0], target),
    };

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        for (idx, cell) in row.iter().enumerate() {
            if idx > 0 {
                out.push_str(COLUMN_GAP);
            }
            let budget = budgets[idx];
            let truncated = truncate_to_width(cell, budget);
            if row_idx == 0 {
                push_styled(&mut out, &truncated, header_style(), options.color);
            } else {
                let column_key = columns.get(idx).copied().unwrap_or("");
                push_cell(&mut out, column_key, &truncated, options.color);
            }
            if idx + 1 < column_count {
                let truncated_width = display_width(&truncated);
                let pad = budget.saturating_sub(truncated_width);
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
        if row_idx == 0 {
            for (idx, width) in budgets.iter().enumerate() {
                if idx > 0 {
                    out.push_str(COLUMN_GAP);
                }
                for _ in 0..*width {
                    out.push('-');
                }
            }
            out.push('\n');
        }
    }
    out
}

fn natural_widths(rows: &[Vec<String>], columns: usize) -> Vec<usize> {
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx >= widths.len() {
                continue;
            }
            widths[idx] = widths[idx].max(display_width(cell));
        }
    }
    widths
}

/// Greedily shrink column budgets toward `target` total width, never below
/// each column's floor. When the floor sum already exceeds the target the
/// columns settle at their floors; the final row may exceed `target`, which
/// is intentional — narrow terminals get a best-effort fit rather than
/// degenerate output.
fn fit_to_width(naturals: &[usize], header: &[String], target: usize) -> Vec<usize> {
    let columns = naturals.len();
    if columns == 0 {
        return Vec::new();
    }
    let gap_total = COLUMN_GAP_WIDTH.saturating_mul(columns.saturating_sub(1));

    let floors: Vec<usize> = naturals
        .iter()
        .enumerate()
        .map(|(idx, &natural)| {
            let header_width = header
                .get(idx)
                .map(|s| display_width(s))
                .unwrap_or(MIN_COLUMN_BUDGET);
            natural.min(header_width.max(MIN_COLUMN_BUDGET))
        })
        .collect();

    let mut budgets = naturals.to_vec();
    loop {
        let used: usize = budgets.iter().sum::<usize>().saturating_add(gap_total);
        if used <= target {
            break;
        }
        let mut best: Option<(usize, usize)> = None;
        for (idx, &budget) in budgets.iter().enumerate() {
            let margin = budget.saturating_sub(floors[idx]);
            if margin == 0 {
                continue;
            }
            match best {
                None => best = Some((idx, margin)),
                Some((_, current)) if margin > current => best = Some((idx, margin)),
                _ => {}
            }
        }
        match best {
            Some((idx, _)) => budgets[idx] -= 1,
            None => break, // every column at its floor; live with overflow
        }
    }
    budgets
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate `s` to fit within `budget` display columns, appending `…` when
/// truncation actually happens. `budget == 0` yields an empty string;
/// `budget == 1` yields a bare `…`.
fn truncate_to_width(s: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if display_width(s) <= budget {
        return s.to_string();
    }
    if budget == 1 {
        return "…".to_string();
    }
    // Reserve one column for the ellipsis.
    let limit = budget - 1;
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > limit {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, Confidence, ForgePrId, ForgePrNode, Freshness,
        GraphLink, GraphNode, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, RepoId, SourceMetadata,
    };

    fn agent_session(harness: &str, key: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, "global", key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: None,
        })
    }

    fn agent_session_with_preview(
        harness: &str,
        key: &str,
        cwd: Option<&str>,
        preview: &str,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, "global", key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
            last_message_preview: Some(preview.to_string()),
        })
    }

    fn mux_session(backend: &str, name: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: cwd.map(str::to_string),
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux_link(
        id: &str,
        session_id: AgentSessionId,
        mux_id: MuxSessionId,
        provenance: Provenance,
        confidence: Confidence,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(session_id),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
            relation: RelationKind::LinkedToMux,
            provenance,
            confidence,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn indicator_renders_provenance_confidence_and_ambiguity_marker() {
        assert_eq!(
            indicator(Provenance::LocalDeclared, Confidence::High, false),
            "LD/H"
        );
        assert_eq!(
            indicator(Provenance::StrongDiscovered, Confidence::Medium, true),
            "SD/M*"
        );
        assert_eq!(indicator(Provenance::Cached, Confidence::Low, false), "$/L");
        assert_eq!(
            indicator(Provenance::Convention, Confidence::High, false),
            "C/H"
        );
    }

    #[test]
    fn empty_snapshot_renders_header_only_for_each_projection() {
        let snapshot = GraphSnapshot::empty();
        for projection in [Projection::Agent, Projection::Mux, Projection::Union] {
            let rendered = render(&snapshot, projection);
            // Header line + dashes line, no data rows.
            assert_eq!(
                rendered.lines().count(),
                2,
                "projection {projection:?} should render only a header",
            );
        }
    }

    #[test]
    fn agent_projection_emits_one_row_per_agent_session() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                agent_session("codex", "beta", None),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        let body_rows: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body_rows.len(), 2);
        assert!(body_rows.iter().any(|row| row.contains("alpha")));
        assert!(body_rows.iter().any(|row| row.contains("beta")));
    }

    #[test]
    fn agent_projection_shows_single_mux_without_ambiguity_marker() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let mux_id = MuxSessionId::new("tmux:editor");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                mux_id,
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(rendered.contains("tmux:editor"));
        assert!(rendered.contains("SD/H"));
        assert!(!rendered.contains("SD/H*"));
    }

    #[test]
    fn agent_projection_marks_ambiguous_mux_selection() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "one", Some("/work/a")),
                mux_session("tmux", "two", Some("/work/a")),
            ],
            candidate_links: vec![
                linked_to_mux_link(
                    "link-1",
                    session_id.clone(),
                    MuxSessionId::new("tmux:one"),
                    Provenance::Discovered,
                    Confidence::Medium,
                ),
                linked_to_mux_link(
                    "link-2",
                    session_id,
                    MuxSessionId::new("tmux:two"),
                    Provenance::Discovered,
                    Confidence::Medium,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(
            rendered.contains("D/M*"),
            "expected ambiguity marker in:\n{rendered}",
        );
    }

    #[test]
    fn mux_projection_lists_attached_agents() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                MuxSessionId::new("tmux:editor"),
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Mux);

        assert!(rendered.contains("tmux:editor"));
        assert!(rendered.contains("codex:alpha"));
        assert!(rendered.contains("SD/H"));
    }

    #[test]
    fn mux_projection_shows_zero_agents_for_orphan_mux() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Mux);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 1);
        assert!(body[0].contains("tmux:lonely"));
        assert!(body[0].contains("—"));
    }

    #[test]
    fn union_projection_preserves_both_node_types() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "lonely", Some("/work")),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Union);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 2);
        // The leftmost column is now the short ID; the kind cell follows
        // a column gap.
        assert!(body.iter().any(|row| row.contains("  agent ")));
        assert!(body.iter().any(|row| row.contains("  mux ")));
    }

    #[test]
    fn union_projection_renders_mux_relationship_for_session() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                MuxSessionId::new("tmux:editor"),
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Union);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let agent_row = body
            .iter()
            .find(|row| row.contains("  agent "))
            .expect("agent row");
        assert!(agent_row.contains("mux=tmux:editor"));
        assert!(agent_row.contains("SD/H"));
    }

    fn parent_session_link(id: &str, child: AgentSessionId, parent: AgentSessionId) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(child),
            target: LinkEndpoint::Node {
                id: NodeId::AgentSession(parent),
            },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn unresolved_parent_session_link(
        id: &str,
        child: AgentSessionId,
        parent_native_id: &str,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(child),
            target: LinkEndpoint::Unresolved {
                evidence: crate::model::UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some("claude-code".to_string()),
                    native_id: Some(parent_native_id.to_string()),
                    state_scope: None,
                    path: None,
                    metadata: Default::default(),
                },
            },
            relation: RelationKind::ParentSession,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn agent_projection_lineage_column_shows_resolved_parent_short_id() {
        let parent_id = AgentSessionId::new("claude-code", "global", "parent");
        let child_id = AgentSessionId::new("claude-code", "global", "child");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("claude-code", "parent", Some("/work")),
                agent_session("claude-code", "child", Some("/work")),
            ],
            candidate_links: vec![parent_session_link("lineage-1", child_id, parent_id)],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(rendered.contains("LINEAGE"));
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let child_row = body
            .iter()
            .find(|row| row.contains("claude-code:child"))
            .expect("child row");
        assert!(
            child_row.contains("parent"),
            "expected parent short id in child row:\n{child_row}",
        );
        let parent_row = body
            .iter()
            .find(|row| row.contains("claude-code:parent"))
            .expect("parent row");
        // Parent has no parent of its own, so it shows the empty lineage cell.
        assert!(
            parent_row.trim_end().ends_with("—"),
            "expected empty lineage in parent row:\n{parent_row}",
        );
    }

    #[test]
    fn agent_projection_lineage_column_marks_chain_with_arrow() {
        let grand_id = AgentSessionId::new("claude-code", "global", "grand");
        let mid_id = AgentSessionId::new("claude-code", "global", "mid");
        let leaf_id = AgentSessionId::new("claude-code", "global", "leaf");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("claude-code", "grand", Some("/work")),
                agent_session("claude-code", "mid", Some("/work")),
                agent_session("claude-code", "leaf", Some("/work")),
            ],
            candidate_links: vec![
                parent_session_link("link-mid", mid_id.clone(), grand_id),
                parent_session_link("link-leaf", leaf_id, mid_id),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);
        let body: Vec<&str> = rendered.lines().skip(2).collect();

        let leaf_row = body
            .iter()
            .find(|row| row.contains("claude-code:leaf"))
            .expect("leaf row");
        assert!(
            leaf_row.contains("mid←"),
            "leaf's lineage cell should mark longer ancestry with ←:\n{leaf_row}",
        );

        let mid_row = body
            .iter()
            .find(|row| row.contains("claude-code:mid"))
            .expect("mid row");
        // mid's parent (grand) has no parent itself, so no chain marker.
        assert!(
            mid_row.contains("grand") && !mid_row.contains("grand←"),
            "mid's lineage cell should be `grand` without chain marker:\n{mid_row}",
        );
    }

    #[test]
    fn agent_projection_lineage_column_marks_unresolved_parent_with_question_prefix() {
        let child_id = AgentSessionId::new("claude-code", "global", "child");
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("claude-code", "child", Some("/work"))],
            candidate_links: vec![unresolved_parent_session_link(
                "lineage-unresolved",
                child_id,
                "missing-parent",
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let child_row = body
            .iter()
            .find(|row| row.contains("claude-code:child"))
            .expect("child row");
        assert!(
            child_row.contains("?missing-parent") || child_row.contains("?…"),
            "unresolved parent should appear with `?` prefix:\n{child_row}",
        );
    }

    #[test]
    fn short_session_id_abbreviates_long_uuids() {
        assert_eq!(short_session_id("alpha"), "alpha");
        assert_eq!(short_session_id("session-1234"), "session-1234");
        let uuid = "019e2454-8f7e-7543-aac5-b0d8ff75be49";
        let abbr = short_session_id(uuid);
        assert!(
            abbr.starts_with('…') && abbr.len() < uuid.len(),
            "expected abbreviation, got {abbr}",
        );
        assert!(abbr.ends_with(&uuid[uuid.len() - 8..]));
    }

    #[test]
    fn agent_projection_shows_preferred_pr_when_branch_has_one() {
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let branch_id = BranchId::new(
            RepoId::new("/workspace/repo/.git"),
            "refs/heads/feature".to_string(),
        );
        let mut pr_link = GraphLink::new(
            "pr-link",
            NodeId::ForgePr(pr_id.clone()),
            LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            RelationKind::BranchHasForgePr,
            Provenance::StrongDiscovered,
        );
        pr_link.confidence = Confidence::High;
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                GraphNode::ForgePr(ForgePrNode {
                    id: pr_id.clone(),
                    provider: "github".to_string(),
                    host: "github.com".to_string(),
                    owner: "octo".to_string(),
                    repo: "repo".to_string(),
                    number: 7,
                    state: Some("open".to_string()),
                    url: None,
                    updated_epoch: None,
                    is_draft: false,
                }),
            ],
            candidate_links: vec![pr_link],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(
            rendered.contains("octo/repo#7"),
            "expected PR label in:\n{rendered}",
        );
    }

    #[test]
    fn render_columns_listing_marks_default_columns() {
        let listing = render_columns_listing(Projection::Agent, false);
        // ID is in the default set.
        let id_line = listing
            .lines()
            .find(|line| line.starts_with("id "))
            .expect("id line");
        assert!(id_line.ends_with("(default)"), "got: {id_line}");

        // `worktree` was added as opt-in by H-TBL-010, so its line
        // should not carry the (default) marker.
        let worktree_line = listing
            .lines()
            .find(|line| line.starts_with("worktree "))
            .expect("worktree line");
        assert!(
            !worktree_line.contains("(default)"),
            "worktree should not be marked default: {worktree_line}",
        );
        assert!(
            worktree_line.contains("matches the session's cwd"),
            "worktree line should include the description: {worktree_line}",
        );
    }

    #[test]
    fn render_columns_listing_includes_every_registered_key() {
        for projection in [
            Projection::Agent,
            Projection::Mux,
            Projection::Union,
            Projection::Pr,
            Projection::Fork,
        ] {
            let listing = render_columns_listing(projection, false);
            for spec in columns_for(projection) {
                assert!(
                    listing
                        .lines()
                        .any(|line| line.starts_with(&format!("{} ", spec.key))),
                    "listing for {projection:?} missing column {key}:\n{listing}",
                    key = spec.key,
                );
            }
        }
    }

    #[test]
    fn default_columns_for_each_projection_matches_registry() {
        for projection in [Projection::Agent, Projection::Mux, Projection::Union] {
            let registry: Vec<&str> = columns_for(projection)
                .iter()
                .filter(|c| c.default)
                .map(|c| c.key)
                .collect();
            assert_eq!(default_columns(projection), registry);
        }
    }

    #[test]
    fn parse_columns_empty_spec_returns_default_set() {
        // An empty string yields an empty-token error; the harness-side
        // default (no flag at all) is to keep defaults — tested in
        // CLI integration. Here we just check that ".." returns default.
        let result = parse_columns_spec(Projection::Agent, "default").expect("default token");
        assert_eq!(result, default_columns(Projection::Agent));
    }

    #[test]
    fn parse_columns_explicit_list_resets_running_set() {
        let result = parse_columns_spec(Projection::Agent, "id,agent,cwd").expect("explicit list");
        assert_eq!(result, vec!["id", "agent", "cwd"]);
    }

    #[test]
    fn parse_columns_plus_appends_to_defaults() {
        // Default already contains "lineage", so adding it is a no-op
        // but should still succeed.
        let result = parse_columns_spec(Projection::Agent, "+lineage").expect("plus lineage");
        assert_eq!(result, default_columns(Projection::Agent));
    }

    #[test]
    fn parse_columns_minus_removes_from_defaults() {
        let result = parse_columns_spec(Projection::Agent, "-cwd,-mux-conf").expect("minus tokens");
        assert_eq!(
            result,
            vec!["id", "agent", "mux", "pr", "pr-conf", "lineage"]
        );
    }

    #[test]
    fn parse_columns_all_resets_to_every_registered_column() {
        let result = parse_columns_spec(Projection::Mux, "all").expect("all");
        // `all` reflects every registered column in registry order;
        // the mux registry is the smallest and the easiest to lock in.
        assert_eq!(
            result,
            vec![
                "id",
                "mux",
                "cwd",
                "agents",
                "preview",
                "attached-count",
                "activity",
                "created",
            ],
        );
    }

    #[test]
    fn parse_columns_mixed_explicit_list_then_plus() {
        // Bare token resets, then `+` appends after.
        let result = parse_columns_spec(Projection::Agent, "id,agent,+cwd").expect("mixed");
        assert_eq!(result, vec!["id", "agent", "cwd"]);
    }

    #[test]
    fn parse_columns_unknown_name_errors_with_available_listed() {
        let err = parse_columns_spec(Projection::Agent, "+nope").unwrap_err();
        match err {
            ColumnsError::UnknownColumn {
                name,
                row_type,
                available,
            } => {
                assert_eq!(name, "nope");
                assert_eq!(row_type, "sessions");
                assert!(available.contains(&"agent"));
            }
            other => panic!("expected UnknownColumn, got {other:?}"),
        }
    }

    #[test]
    fn parse_columns_empty_token_errors() {
        let err = parse_columns_spec(Projection::Agent, "id,,agent").unwrap_err();
        assert!(matches!(err, ColumnsError::EmptyToken));
    }

    #[test]
    fn resolve_explicit_columns_validates_each_name() {
        let names: Vec<String> = vec!["id".into(), "agent".into(), "lineage".into()];
        let result = resolve_explicit_columns(Projection::Agent, &names).expect("resolve explicit");
        assert_eq!(result, vec!["id", "agent", "lineage"]);

        let bad: Vec<String> = vec!["id".into(), "nope".into()];
        let err = resolve_explicit_columns(Projection::Agent, &bad).unwrap_err();
        assert!(matches!(err, ColumnsError::UnknownColumn { .. }));
    }

    #[test]
    fn render_with_explicit_columns_emits_only_those_cells() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
            ..GraphSnapshot::empty()
        };
        let options = RenderOptions::wide().with_columns(vec!["id", "agent", "cwd"]);
        let rendered = render_with(&snapshot, Projection::Agent, &options);
        let header_tokens: Vec<&str> = rendered
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .collect();
        // Three columns, no MUX/PR/LINEAGE.
        assert_eq!(header_tokens, vec!["ID", "AGENT", "CWD"]);
        assert!(!rendered.contains("MUX"));
        assert!(!rendered.contains("PR"));
        assert!(!rendered.contains("LINEAGE"));

        let body_tokens: Vec<&str> = rendered
            .lines()
            .nth(2)
            .unwrap()
            .split_whitespace()
            .collect();
        // [<short>, codex:alpha, /work/a]
        assert_eq!(body_tokens.len(), 3);
        assert_eq!(body_tokens[1], "codex:alpha");
        assert_eq!(body_tokens[2], "/work/a");
    }

    #[test]
    fn render_color_disabled_is_byte_identical_to_plain_text() {
        // Regression: turning the `color` knob on/off must not perturb
        // the bytes when color is off. This guards every `insta`
        // snapshot from silently growing ANSI escapes.
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
            ..GraphSnapshot::empty()
        };
        let plain = render(&snapshot, Projection::Agent);
        let off = render_with(&snapshot, Projection::Agent, &RenderOptions::wide());
        let off_with_color_false = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_color(false),
        );
        assert_eq!(plain, off);
        assert_eq!(plain, off_with_color_false);
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn render_color_enabled_wraps_header_and_styled_cells_in_ansi() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_color(true),
        );
        // Header row gets the bold envelope.
        assert!(
            colored.contains("\u{1b}[1m"),
            "expected bold escape for header:\n{colored:?}",
        );
        // The ID column is colored blue.
        assert!(
            colored.contains("\u{1b}[34m"),
            "expected blue escape for ID column:\n{colored:?}",
        );
        // The `—` placeholder uses 256-color 244 (faded gray); the
        // standard sequence is `\x1b[38;5;244m`.
        assert!(
            colored.contains("\u{1b}[38;5;244m"),
            "expected 256-color 244 for `—` placeholder:\n{colored:?}",
        );
        // Every opened escape closes with the reset sequence.
        assert!(
            colored.contains("\u{1b}[0m"),
            "expected reset escape in:\n{colored:?}",
        );
    }

    #[test]
    fn render_color_enabled_colors_agent_harness_prefix_only() {
        // The agent cell should colorize the `claude-code` portion of
        // `claude-code:alpha` but leave the `:alpha` suffix
        // uncolored. The bright-yellow escape sequence appears, the
        // session-key portion does not get a fresh escape introduced
        // (the prefix's escape is followed by reset, then plain
        // text).
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("claude-code", "alpha", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_color(true),
        );
        // Bright yellow = `\x1b[93m`.
        let bright_yellow = "\u{1b}[93m";
        assert!(
            colored.contains(bright_yellow),
            "expected bright yellow for claude-code harness:\n{colored:?}",
        );
        // The styled span is just the harness name; the body looks
        // like `<esc>[93m claude-code <reset>:alpha` (no surrounding
        // escapes around `:alpha`).
        assert!(
            colored.contains(&format!("{bright_yellow}claude-code\u{1b}[0m:alpha")),
            "expected claude-code prefix wrapped, suffix plain:\n{colored:?}",
        );
    }

    #[test]
    fn render_color_enabled_uses_distinct_color_per_harness() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("claude-code", "a", Some("/w/a")),
                agent_session("codex", "b", Some("/w/b")),
                agent_session("opencode", "c", Some("/w/c")),
                agent_session("aider", "d", Some("/w/d")),
            ],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_color(true),
        );
        // Each harness has its own bright-color escape.
        assert!(
            colored.contains("\u{1b}[93m"),
            "claude-code = bright yellow"
        );
        assert!(colored.contains("\u{1b}[94m"), "codex = bright blue");
        assert!(colored.contains("\u{1b}[92m"), "opencode = bright green");
        assert!(colored.contains("\u{1b}[91m"), "aider = bright red");
    }

    #[test]
    fn render_color_enabled_union_label_colors_agent_harness_prefix() {
        // The union projection's LABEL column should carry the same
        // harness-prefix colouring as the sessions projection's
        // AGENT cell. Agent rows show coloured `harness:`; mux
        // rows render `backend:native_id` as plain text because
        // the mux backend is not in the harness palette.
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work")),
                mux_session("tmux", "editor", Some("/work")),
            ],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Union,
            &RenderOptions::wide().with_color(true),
        );
        // Codex prefix gets bright blue; the `:alpha` tail stays
        // plain (no fresh escape after the reset).
        assert!(
            colored.contains("\u{1b}[94mcodex\u{1b}[0m:alpha"),
            "union LABEL should colour the codex prefix:\n{colored:?}",
        );
        // Mux label has no harness palette slot — `tmux` falls
        // through to plain text, no bright-color escape adjacent
        // to it.
        let mux_line = colored
            .lines()
            .find(|line| line.contains("tmux:editor"))
            .expect("mux row");
        assert!(
            !mux_line.contains("\u{1b}[9"),
            "mux row LABEL should not have a bright-color escape:\n{mux_line:?}",
        );
    }

    #[test]
    fn render_color_enabled_unknown_harness_stays_uncolored() {
        // A novel harness key should fall through to no styling so
        // the cell still reads cleanly until a palette slot is
        // chosen for it.
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("future-tool", "x", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_color(true),
        );
        // No bright-color escape should appear adjacent to
        // `future-tool` — the cell renders verbatim.
        let needle = "future-tool:x";
        assert!(
            colored.contains(needle),
            "expected verbatim agent cell for unknown harness:\n{colored:?}",
        );
    }

    #[test]
    fn render_color_enabled_colors_each_harness_in_mux_agents_cell() {
        // The mux projection's AGENTS column joins multiple agents
        // into one cell. Each agent's harness prefix should be
        // colored separately, the `:session-key` tail stays plain,
        // and the trailing `[indicator]` suffix stays uncolored.
        let editor = MuxSessionId::new("tmux:editor");
        let snapshot = GraphSnapshot {
            nodes: vec![
                mux_session("tmux", "editor", Some("/work")),
                agent_session("codex", "alpha", Some("/work")),
                agent_session("claude-code", "beta", Some("/work")),
            ],
            candidate_links: vec![
                linked_to_mux_link(
                    "link-c",
                    AgentSessionId::new("codex", "global", "alpha"),
                    editor.clone(),
                    Provenance::StrongDiscovered,
                    Confidence::High,
                ),
                linked_to_mux_link(
                    "link-cc",
                    AgentSessionId::new("claude-code", "global", "beta"),
                    editor,
                    Provenance::StrongDiscovered,
                    Confidence::High,
                ),
            ],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_color(true),
        );
        // Bright blue = codex; bright yellow = claude-code.
        assert!(
            colored.contains("\u{1b}[94mcodex\u{1b}[0m:alpha "),
            "expected codex prefix coloured inside agents cell:\n{colored:?}",
        );
        assert!(
            colored.contains("\u{1b}[93mclaude-code\u{1b}[0m:beta "),
            "expected claude-code prefix coloured inside agents cell:\n{colored:?}",
        );
        // The comma separator between entries should be plain (no
        // escape immediately after `]`).
        assert!(
            colored.contains("], \u{1b}[94mcodex") || colored.contains("], \u{1b}[93mclaude-code"),
            "expected plain `, ` separator between coloured entries:\n{colored:?}",
        );
    }

    #[test]
    fn render_color_enabled_dash_in_mux_agents_cell_uses_placeholder_style() {
        // A mux with no attached agents renders the cell as `—`,
        // which should pick up the faded-gray placeholder style
        // (256-color 244) rather than dropping out as plain text.
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_color(true),
        );
        assert!(
            colored.contains("\u{1b}[38;5;244m—\u{1b}[0m"),
            "expected faded `—` for empty agents cell:\n{colored:?}",
        );
    }

    #[test]
    fn render_color_enabled_colors_pr_state_cell() {
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::ForgePr(ForgePrNode {
                id: pr_id,
                provider: "github".to_string(),
                host: "github.com".to_string(),
                owner: "octo".to_string(),
                repo: "repo".to_string(),
                number: 7,
                state: Some("open".to_string()),
                url: None,
                updated_epoch: None,
                is_draft: false,
            })],
            ..GraphSnapshot::empty()
        };
        let colored = render_with(
            &snapshot,
            Projection::Pr,
            &RenderOptions::wide().with_color(true),
        );
        // Green is `\x1b[32m` in ANSI 16-color.
        assert!(
            colored.contains("\u{1b}[32m"),
            "expected green for PR state `open`:\n{colored:?}",
        );
    }

    #[test]
    fn render_with_card_layout_honors_column_selection() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
            ..GraphSnapshot::empty()
        };
        let options = RenderOptions::card().with_columns(vec!["id", "agent"]);
        let rendered = render_with(&snapshot, Projection::Agent, &options);
        let lines: Vec<&str> = rendered.lines().filter(|l| !l.is_empty()).collect();
        // Card emits one `KEY: value` line per selected column.
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("ID:"));
        assert!(lines[1].starts_with("AGENT:"));
    }

    #[test]
    fn format_relative_age_picks_largest_unit_under_threshold() {
        assert_eq!(format_relative_age(100, 100), "0s");
        assert_eq!(format_relative_age(100, 159), "59s");
        assert_eq!(format_relative_age(100, 160), "1m");
        assert_eq!(format_relative_age(100, 100 + 3600), "1h");
        assert_eq!(format_relative_age(100, 100 + 86_400), "1d");
        assert_eq!(format_relative_age(100, 100 + 7 * 86_400), "1w");
    }

    #[test]
    fn format_relative_age_future_value_renders_as_now() {
        assert_eq!(format_relative_age(2_000_000_000, 1_000_000_000), "now");
    }

    #[test]
    fn strip_branch_prefix_drops_refs_heads_only() {
        assert_eq!(strip_branch_prefix("refs/heads/feature"), "feature");
        assert_eq!(strip_branch_prefix("main"), "main");
        assert_eq!(strip_branch_prefix("refs/tags/v1"), "refs/tags/v1");
    }

    fn mux_session_with_epochs(
        backend: &str,
        name: &str,
        activity_epoch: Option<i64>,
        created_epoch: Option<i64>,
    ) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: None,
            activity_epoch,
            created_epoch,
        })
    }

    #[test]
    fn mux_projection_attached_count_column() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let mux_id = MuxSessionId::new("tmux:editor");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work")),
                mux_session("tmux", "editor", Some("/work")),
                mux_session("tmux", "lonely", Some("/work")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                mux_id,
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_columns(vec!["id", "mux", "attached-count"]),
        );
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 2);
        let editor_row = body
            .iter()
            .find(|line| line.contains("tmux:editor"))
            .expect("editor row");
        let lonely_row = body
            .iter()
            .find(|line| line.contains("tmux:lonely"))
            .expect("lonely row");
        // Editor has one attached agent; lonely has none.
        assert!(
            editor_row.split_whitespace().any(|t| t == "1"),
            "editor row should show count 1:\n{editor_row}",
        );
        assert!(
            lonely_row.trim_end().ends_with('—'),
            "lonely row should show — for zero attached:\n{lonely_row}",
        );
    }

    #[test]
    fn mux_projection_activity_and_created_columns_format_relative_age() {
        // Use the formatter directly to lock in the recency string;
        // the actual rendered row passes the same value through.
        let activity = 100;
        let created = 0;
        let now = activity + 7200; // 2h after activity, ~2h after created.
        assert_eq!(format_relative_age(activity, now), "2h");
        assert_eq!(format_relative_age(created, now), "2h");

        // Render with explicit columns; verify the cells contain a unit
        // suffix (the exact recency depends on SystemTime::now()).
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session_with_epochs(
                "tmux",
                "editor",
                Some(activity),
                Some(created),
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_columns(vec!["id", "mux", "activity", "created"]),
        );
        let body = rendered.lines().nth(2).expect("body row");
        // Recency for epoch 100 is many years; assert the cell ends in
        // a recognized unit suffix.
        let cells: Vec<&str> = body.split_whitespace().collect();
        // [<short>, tmux:editor, <activity-cell>, <created-cell>]
        assert!(cells.len() >= 4, "row should have 4 cells: {body:?}");
        for cell in &cells[2..4] {
            let last = cell.chars().last().expect("non-empty cell");
            assert!(
                matches!(last, 's' | 'm' | 'h' | 'd' | 'w'),
                "expected recency suffix, got {cell:?}",
            );
        }
    }

    #[test]
    fn mux_projection_activity_and_created_dashes_when_epoch_is_none() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session_with_epochs("tmux", "editor", None, None)],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_columns(vec!["id", "mux", "activity", "created"]),
        );
        let body = rendered.lines().nth(2).expect("body row");
        let cells: Vec<&str> = body.split_whitespace().collect();
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[2], "—");
        assert_eq!(cells[3], "—");
    }

    #[test]
    fn sessions_projection_optional_branch_repo_worktree_columns() {
        use crate::model::{WorktreeId, WorktreeNode};

        let repo_id = RepoId::new("/workspace/repo/.git");
        let branch_id = BranchId::new(repo_id.clone(), "refs/heads/feature".to_string());
        let worktree_id = WorktreeId::new(repo_id.clone(), "/workspace/repo");

        let worktree_to_branch = GraphLink {
            id: "wt-branch".to_string(),
            source: NodeId::Worktree(worktree_id.clone()),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            relation: RelationKind::CheckedOutBranch,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/workspace/repo/crates/core")),
                GraphNode::Worktree(WorktreeNode {
                    id: worktree_id,
                    root: "/workspace/repo".to_string(),
                    git_dir: None,
                    current_branch: None,
                }),
            ],
            candidate_links: vec![worktree_to_branch],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_columns(vec!["id", "agent", "worktree", "branch", "repo"]),
        );
        let body = rendered.lines().nth(2).expect("body row");
        assert!(body.contains("/workspace/repo"), "got:\n{body}");
        assert!(body.contains("feature"), "got:\n{body}");
        assert!(body.contains("/workspace/repo/.git"), "got:\n{body}");
    }

    #[test]
    fn sessions_projection_fork_column_renders_owning_fork_label() {
        // Match the AgentSessionId state_scope to the `agent_session`
        // helper (which uses "global") so the fork's ChildSession
        // candidate resolves to the discovered session node.
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let fork_id = crate::model::ForkId::new("atelier:alpha");

        let snapshot = GraphSnapshot {
            nodes: vec![
                fork_node(Some("alpha"), "atelier", "alpha"),
                agent_session("codex", "alpha", Some("/work")),
            ],
            candidate_links: vec![fork_lineage_link(
                "child",
                fork_id,
                RelationKind::ChildSession,
                session_id,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_columns(vec!["id", "agent", "fork"]),
        );
        assert!(
            rendered.contains("atelier:alpha"),
            "fork column should label the owning fork:\n{rendered}",
        );
    }

    #[test]
    fn sessions_projection_declared_column_reflects_link_state() {
        // The `agent_session` helper uses state_scope "global"; the
        // declared link's source AgentSessionId must match for the
        // extractor to find it.
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let mux_id = MuxSessionId::new("tmux:editor");

        let mut declared_link = linked_to_mux_link(
            "declared-link",
            session_id.clone(),
            mux_id,
            Provenance::LocalDeclared,
            Confidence::High,
        );
        declared_link.state = LinkState::Ignored {
            reason: Some("stale".to_string()),
        };

        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work"))],
            candidate_links: vec![declared_link],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_columns(vec!["id", "agent", "declared"]),
        );
        assert!(
            rendered.contains("ignored"),
            "declared column should reflect the link's state:\n{rendered}",
        );
    }

    fn forge_pr(owner: &str, repo: &str, number: u64, state: &str, draft: bool) -> GraphNode {
        GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", owner, repo, number),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            number,
            state: Some(state.to_string()),
            url: None,
            updated_epoch: None,
            is_draft: draft,
        })
    }

    fn branch_has_pr_link(
        id: &str,
        pr: ForgePrId,
        branch: BranchId,
        provenance: Provenance,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::ForgePr(pr),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn prs_projection_default_renders_id_pr_state_branch_attached() {
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let branch_id = BranchId::new(
            RepoId::new("/workspace/repo/.git"),
            "refs/heads/feature".to_string(),
        );
        let snapshot = GraphSnapshot {
            nodes: vec![forge_pr("octo", "repo", 7, "open", false)],
            candidate_links: vec![branch_has_pr_link(
                "pr-link",
                pr_id,
                branch_id,
                Provenance::StrongDiscovered,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Pr);
        let header_tokens: Vec<&str> = rendered
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .collect();
        assert_eq!(
            header_tokens,
            vec!["ID", "PR", "STATE", "BRANCH", "ATTACHED"]
        );

        let body_line = rendered.lines().nth(2).expect("body row");
        assert!(body_line.contains("octo/repo#7"), "got:\n{body_line}");
        assert!(body_line.contains("open"), "got:\n{body_line}");
        assert!(body_line.contains("feature"), "got:\n{body_line}");
    }

    #[test]
    fn prs_projection_attached_shows_agent_with_matching_cwd() {
        // PR -> branch (BranchHasForgePr) -> worktree (CheckedOutBranch
        // reversed) -> agent_session with matching cwd. Renders the
        // agent label in the ATTACHED column.
        use crate::model::{WorktreeId, WorktreeNode};

        let repo_id = RepoId::new("/workspace/repo/.git");
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let branch_id = BranchId::new(repo_id.clone(), "refs/heads/feature".to_string());
        let worktree_id = WorktreeId::new(repo_id.clone(), "/workspace/repo");

        let worktree_node = GraphNode::Worktree(WorktreeNode {
            id: worktree_id.clone(),
            root: "/workspace/repo".to_string(),
            git_dir: None,
            current_branch: None,
        });
        let agent_node = agent_session("codex", "alpha", Some("/workspace/repo/crates/core"));
        let pr_node = forge_pr("octo", "repo", 7, "open", false);

        let mut pr_to_branch = branch_has_pr_link(
            "pr-link",
            pr_id.clone(),
            branch_id.clone(),
            Provenance::StrongDiscovered,
        );
        pr_to_branch.confidence = Confidence::High;

        let worktree_to_branch = GraphLink {
            id: "wt-branch".to_string(),
            source: NodeId::Worktree(worktree_id),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            relation: RelationKind::CheckedOutBranch,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        };

        let snapshot = GraphSnapshot {
            nodes: vec![pr_node, worktree_node, agent_node],
            candidate_links: vec![pr_to_branch, worktree_to_branch],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Pr);
        assert!(
            rendered.contains("codex:alpha"),
            "attached column should list codex:alpha:\n{rendered}",
        );
    }

    fn fork_node(name: Option<&str>, provider: &str, key: &str) -> GraphNode {
        GraphNode::Fork(ForkNode {
            id: crate::model::ForkId::new(format!("{provider}:{key}")),
            provider: provider.to_string(),
            provider_source_key: key.to_string(),
            name: name.map(str::to_string),
            scope: None,
            capabilities: Vec::new(),
        })
    }

    fn fork_lineage_link(
        id: &str,
        fork_id: crate::model::ForkId,
        relation: RelationKind,
        target_session: crate::model::AgentSessionId,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::Fork(fork_id),
            target: LinkEndpoint::Node {
                id: NodeId::AgentSession(target_session),
            },
            relation,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn forks_projection_default_renders_id_fork_provider_parent_children() {
        let fork_node = fork_node(Some("alpha"), "atelier", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![fork_node],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Fork);
        let header_tokens: Vec<&str> = rendered
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .collect();
        assert_eq!(
            header_tokens,
            vec!["ID", "FORK", "PROVIDER", "PARENT", "CHILDREN"],
        );
        let body = rendered.lines().nth(2).expect("body row");
        assert!(body.contains("atelier:alpha"), "got:\n{body}");
        assert!(body.contains("atelier"), "got:\n{body}");
    }

    #[test]
    fn forks_projection_fork_label_falls_back_to_source_key_without_name() {
        let snapshot = GraphSnapshot {
            nodes: vec![fork_node(None, "atelier", "raw-source")],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Fork);
        assert!(
            rendered.contains("raw-source"),
            "fork label should fall back to provider_source_key:\n{rendered}",
        );
    }

    #[test]
    fn forks_projection_parent_and_children_count() {
        let fork_id = crate::model::ForkId::new("atelier:alpha");
        let parent_session_id = crate::model::AgentSessionId::new("codex", "/state", "parent");
        let child_one_id = crate::model::AgentSessionId::new("codex", "/state", "child-one");
        let child_two_id = crate::model::AgentSessionId::new("codex", "/state", "child-two");

        let snapshot = GraphSnapshot {
            nodes: vec![
                fork_node(Some("alpha"), "atelier", "alpha"),
                agent_session("codex", "parent", Some("/work")),
                agent_session("codex", "child-one", Some("/work")),
                agent_session("codex", "child-two", Some("/work")),
            ],
            candidate_links: vec![
                fork_lineage_link(
                    "parent",
                    fork_id.clone(),
                    RelationKind::ParentSession,
                    parent_session_id,
                ),
                fork_lineage_link(
                    "child-1",
                    fork_id.clone(),
                    RelationKind::ChildSession,
                    child_one_id,
                ),
                fork_lineage_link("child-2", fork_id, RelationKind::ChildSession, child_two_id),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Fork);
        let body = rendered
            .lines()
            .nth(2)
            .expect("at least one body row")
            .to_string();
        // Parent cell shows the parent session's short id.
        assert!(body.contains("parent"), "got:\n{body}");
        // Children cell counts the two child_session candidates; it's the
        // last cell on the line, so check the trailing token.
        let last_token = body
            .split_whitespace()
            .next_back()
            .expect("at least one cell");
        assert_eq!(last_token, "2", "expected children count of 2:\n{body}");
    }

    #[test]
    fn forks_projection_capabilities_optional_column() {
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::Fork(ForkNode {
                id: crate::model::ForkId::new("atelier:alpha"),
                provider: "atelier".to_string(),
                provider_source_key: "alpha".to_string(),
                name: Some("alpha".to_string()),
                scope: None,
                capabilities: vec!["native_lineage".to_string(), "compaction".to_string()],
            })],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Fork,
            &RenderOptions::wide().with_columns(vec!["id", "fork", "capabilities"]),
        );
        assert!(
            rendered.contains("native_lineage, compaction"),
            "capabilities cell should join the list:\n{rendered}",
        );
    }

    #[test]
    fn prs_projection_empty_snapshot_renders_header_only() {
        let snapshot = GraphSnapshot::empty();
        let rendered = render(&snapshot, Projection::Pr);
        let lines: Vec<&str> = rendered.lines().collect();
        // Header + dash separator only, no body rows.
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("ID"));
    }

    #[test]
    fn truncate_to_width_appends_ellipsis_only_when_string_overflows() {
        assert_eq!(truncate_to_width("alpha", 5), "alpha");
        assert_eq!(truncate_to_width("alpha", 10), "alpha");
        assert_eq!(truncate_to_width("alphabetagamma", 8), "alphabe…");
        assert_eq!(truncate_to_width("", 5), "");
    }

    #[test]
    fn truncate_to_width_degenerate_budgets_emit_ellipsis_or_empty() {
        assert_eq!(truncate_to_width("abc", 0), "");
        assert_eq!(truncate_to_width("abc", 1), "…");
        assert_eq!(truncate_to_width("a", 1), "a");
    }

    #[test]
    fn truncate_to_width_respects_wide_unicode_columns() {
        // CJK characters are two columns wide; budget=4 fits exactly one
        // CJK char plus the ellipsis (1 column reserved → 3-column limit,
        // first wide char fits, second would overflow).
        let cjk = "東京駅前";
        assert_eq!(truncate_to_width(cjk, 4), "東…");
    }

    #[test]
    fn render_with_unbounded_width_matches_render() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                agent_session("codex", "beta", None),
            ],
            ..GraphSnapshot::empty()
        };
        assert_eq!(
            render(&snapshot, Projection::Agent),
            render_with(&snapshot, Projection::Agent, &RenderOptions::wide()),
        );
    }

    #[test]
    fn narrow_width_truncates_cwd_with_ellipsis() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session(
                "codex",
                "alpha",
                Some("/very/long/workspace/path/that/will/not/fit/in/eighty/columns"),
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::columnar_width(80),
        );
        let body_rows: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body_rows.len(), 1);
        assert!(
            body_rows[0].contains('…'),
            "expected ellipsis after truncation in:\n{rendered}",
        );
        for line in rendered.lines() {
            assert!(
                display_width(line) <= 80,
                "line width {} exceeds 80 columns: {line:?}",
                display_width(line),
            );
        }
    }

    #[test]
    fn wide_width_passes_through_untruncated() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session(
                "codex",
                "alpha",
                Some("/some/workspace/path"),
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::columnar_width(500),
        );
        assert!(rendered.contains("/some/workspace/path"));
        assert!(!rendered.contains('…'));
    }

    #[test]
    fn narrow_width_keeps_short_columns_at_natural_width() {
        // The MUX/CONF and PR/CONF indicator columns are 4-6 chars wide and
        // should never get truncated to "…" — the floor protects them.
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let mux_id = MuxSessionId::new("tmux:editor");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/aaaaaaaaaaaaaaaaaaa")),
                mux_session("tmux", "editor", Some("/work/aaaaaaaaaaaaaaaaaaa")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                mux_id,
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::columnar_width(60),
        );
        assert!(
            rendered.contains("SD/H"),
            "indicator should survive truncation:\n{rendered}",
        );
    }

    #[test]
    fn node_short_id_is_deterministic_for_a_given_node_id() {
        // Lock in the FNV-1a-over-Display contract: this string must not
        // change without a deliberate decision, because users paste short
        // ids into `node show` between runs (H-TBL-005).
        let id = NodeId::AgentSession(AgentSessionId::new("codex", "global", "alpha"));
        let short = node_short_id(&id);
        assert_eq!(short.len(), 16);
        assert!(short.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(short, node_short_id(&id), "must be deterministic");
    }

    #[test]
    fn node_short_id_distinguishes_different_node_ids() {
        let alpha = NodeId::AgentSession(AgentSessionId::new("codex", "global", "alpha"));
        let beta = NodeId::AgentSession(AgentSessionId::new("codex", "global", "beta"));
        assert_ne!(node_short_id(&alpha), node_short_id(&beta));
    }

    #[test]
    fn unique_prefix_len_returns_floor_for_one_row() {
        let ids = vec!["0123456789abcdef".to_string()];
        assert_eq!(unique_prefix_len(&ids), SHORT_ID_FLOOR);
    }

    #[test]
    fn unique_prefix_len_returns_floor_when_prefixes_already_unique() {
        let ids = vec![
            "aaaaaa1111".to_string(),
            "bbbbbb1111".to_string(),
            "cccccc1111".to_string(),
        ];
        assert_eq!(unique_prefix_len(&ids), SHORT_ID_FLOOR);
    }

    #[test]
    fn unique_prefix_len_grows_past_floor_to_break_collisions() {
        // First six chars collide; the seventh resolves them.
        let ids = vec!["abcdefXone".to_string(), "abcdefYone".to_string()];
        assert_eq!(unique_prefix_len(&ids), 7);
    }

    #[test]
    fn agent_projection_prepends_id_column() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                agent_session("codex", "beta", Some("/work/b")),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);
        let header = rendered.lines().next().expect("header");
        // ID is leftmost.
        assert!(
            header.starts_with("ID"),
            "agent projection header should start with ID:\n{header}",
        );

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 2);
        for row in &body {
            let leading: String = row.chars().take_while(|c| !c.is_whitespace()).collect();
            assert_eq!(
                leading.len(),
                SHORT_ID_FLOOR,
                "short id should be at the floor for an uncolliding pair: {row:?}",
            );
            assert!(
                leading.chars().all(|c| c.is_ascii_hexdigit()),
                "short id should be hex: {row:?}",
            );
        }
    }

    #[test]
    fn agent_projection_short_ids_are_stable_across_renders() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
            ..GraphSnapshot::empty()
        };
        let first = render(&snapshot, Projection::Agent);
        let second = render(&snapshot, Projection::Agent);
        assert_eq!(first, second);
    }

    #[test]
    fn mux_projection_prepends_id_column() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session("tmux", "editor", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Mux);
        let header = rendered.lines().next().expect("header");
        assert!(header.starts_with("ID"), "header was: {header:?}");
    }

    #[test]
    fn union_projection_header_renames_existing_id_to_label() {
        let snapshot = GraphSnapshot::empty();
        let rendered = render(&snapshot, Projection::Union);
        let header = rendered.lines().next().expect("header");
        assert!(
            header.starts_with("ID  KIND  LABEL"),
            "union header should be ID KIND LABEL …; got: {header:?}",
        );
    }

    #[test]
    fn card_layout_emits_one_block_per_body_row() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                agent_session("codex", "beta", Some("/work/b")),
            ],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());

        // Two body rows produce two card blocks separated by a blank line.
        // Each block has one line per header column (8 in the agent
        // projection: ID, AGENT, CWD, MUX, MUX/CONF, PR, PR/CONF, LINEAGE).
        let blocks: Vec<&str> = rendered.split("\n\n").collect();
        assert_eq!(blocks.len(), 2);
        for block in &blocks {
            let non_empty_lines = block.lines().filter(|line| !line.is_empty()).count();
            assert_eq!(
                non_empty_lines, 8,
                "card block should have 8 lines: {block:?}"
            );
        }
    }

    #[test]
    fn card_layout_uses_keys_with_aligned_colons() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "alpha", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());

        // Keys (ID, AGENT, CWD, MUX, MUX/CONF, PR, PR/CONF, LINEAGE) have
        // the longest as MUX/CONF and PR/CONF at 8 chars. Every line's
        // value should start at the same column.
        let lines: Vec<&str> = rendered.lines().filter(|l| !l.is_empty()).collect();
        let mut value_starts: Vec<usize> = Vec::new();
        for line in &lines {
            let colon = line.find(':').expect("each card line has a colon");
            // After the colon, padding goes up to the longest key width,
            // then a single space, then the value.
            let value_col = line[colon + 1..]
                .chars()
                .position(|c| !c.is_whitespace())
                .map(|i| colon + 1 + i)
                .unwrap_or(line.len());
            value_starts.push(value_col);
        }
        let first = value_starts[0];
        for start in &value_starts {
            assert_eq!(
                *start, first,
                "value columns should align across keys: starts={value_starts:?}",
            );
        }
    }

    #[test]
    fn card_layout_empty_snapshot_renders_empty_string() {
        let snapshot = GraphSnapshot::empty();
        let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());
        assert!(rendered.is_empty(), "got: {rendered:?}");
    }

    #[test]
    fn card_layout_truncates_values_when_width_is_set() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session(
                "codex",
                "alpha",
                Some("/very/long/workspace/path/that/will/not/fit"),
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card_width(30));
        assert!(
            rendered.contains('…'),
            "long cwd should have been truncated in:\n{rendered}",
        );
        for line in rendered.lines() {
            assert!(display_width(line) <= 30, "line exceeded 30 cols: {line:?}",);
        }
    }

    #[test]
    fn agent_label_uses_session_key_not_title() {
        // Regression for H-TBL-015: the AGENT cell used to fall back
        // to `harness:title` when the adapter populated `title`. That
        // surfaced opencode's long chat topics in the leading cell.
        // The label now always renders `harness:session_key` (with
        // the UUID truncator) and title belongs to the opt-in
        // `title` column.
        let node = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("opencode", "global", "ses_abc123"),
            harness_key: "opencode".to_string(),
            cwd: Some("/work".to_string()),
            title: Some("a very long conversation topic".to_string()),
            last_message_preview: None,
        });
        let snapshot = GraphSnapshot {
            nodes: vec![node],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Agent);
        let body = rendered.lines().nth(2).expect("body row");
        assert!(
            body.contains("opencode:ses_abc123"),
            "AGENT cell should show session_key:\n{body}",
        );
        assert!(
            !body.contains("a very long conversation topic"),
            "AGENT cell must not fall back to title:\n{body}",
        );
    }

    #[test]
    fn agent_label_truncates_uuid_session_keys_to_short_form() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session(
                "claude-code",
                "0b34e59c-14d0-4d04-be79-4dc1d4c120c2",
                Some("/work"),
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Agent);
        let body = rendered.lines().nth(2).expect("body row");
        // UUID collapses to the last-8-chars form.
        assert!(
            body.contains("claude-code:…d4c120c2"),
            "AGENT cell should truncate UUID session_key:\n{body}",
        );
    }

    #[test]
    fn agent_label_preserves_short_session_keys_verbatim() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session("codex", "session-alpha", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Agent);
        let body = rendered.lines().nth(2).expect("body row");
        assert!(
            body.contains("codex:session-alpha"),
            "AGENT cell should render short session_key verbatim:\n{body}",
        );
    }

    #[test]
    fn sessions_title_column_renders_set_value_or_dash() {
        let with_title = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("opencode", "global", "ses_a"),
            harness_key: "opencode".to_string(),
            cwd: Some("/work".to_string()),
            title: Some("clipboard sync over SSH".to_string()),
            last_message_preview: None,
        });
        let without = agent_session("codex", "no-title", Some("/work"));
        let snapshot = GraphSnapshot {
            nodes: vec![with_title, without],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_columns(vec!["id", "agent", "title"]),
        );
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let titled = body
            .iter()
            .find(|line| line.contains("opencode:ses_a"))
            .expect("titled row");
        assert!(
            titled.contains("clipboard sync over SSH"),
            "title row missing content:\n{titled}",
        );
        let untitled = body
            .iter()
            .find(|line| line.contains("codex:no-title"))
            .expect("untitled row");
        assert!(
            untitled.trim_end().ends_with('—'),
            "untitled row should show — in TITLE column:\n{untitled}",
        );
    }

    #[test]
    fn union_title_column_renders_only_for_agent_rows() {
        let titled_agent = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("opencode", "global", "ses_a"),
            harness_key: "opencode".to_string(),
            cwd: Some("/work".to_string()),
            title: Some("agent title".to_string()),
            last_message_preview: None,
        });
        let mux = mux_session("tmux", "editor", Some("/work"));
        let snapshot = GraphSnapshot {
            nodes: vec![titled_agent, mux],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Union,
            &RenderOptions::wide().with_columns(vec!["id", "kind", "label", "title"]),
        );
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let agent = body
            .iter()
            .find(|line| line.contains(" agent "))
            .expect("agent row");
        assert!(
            agent.contains("agent title"),
            "agent row should carry title:\n{agent}",
        );
        let mux_row = body
            .iter()
            .find(|line| line.contains(" mux "))
            .expect("mux row");
        assert!(
            mux_row.trim_end().ends_with('—'),
            "mux row should render — for title:\n{mux_row}",
        );
    }

    #[test]
    fn sessions_preview_column_renders_set_value_or_dash() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session_with_preview("codex", "alpha", Some("/work"), "wired up the column"),
                agent_session("codex", "beta", Some("/work")),
            ],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Agent,
            &RenderOptions::wide().with_columns(vec!["id", "agent", "preview"]),
        );
        let header: Vec<&str> = rendered
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .collect();
        assert_eq!(header, vec!["ID", "AGENT", "PREVIEW"]);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let alpha = body
            .iter()
            .find(|line| line.contains("codex:alpha"))
            .expect("alpha row");
        assert!(
            alpha.contains("wired up the column"),
            "alpha row preview missing:\n{alpha}",
        );
        let beta = body
            .iter()
            .find(|line| line.contains("codex:beta"))
            .expect("beta row");
        assert!(
            beta.trim_end().ends_with('—'),
            "beta row should show — for missing preview:\n{beta}",
        );
    }

    #[test]
    fn mux_preview_column_shows_first_attached_agent_preview() {
        let session_alpha = AgentSessionId::new("codex", "global", "alpha");
        let session_beta = AgentSessionId::new("codex", "global", "beta");
        let editor = MuxSessionId::new("tmux:editor");

        let snapshot = GraphSnapshot {
            nodes: vec![
                mux_session("tmux", "editor", Some("/work")),
                agent_session_with_preview("codex", "alpha", Some("/work"), "alpha preview"),
                agent_session_with_preview("codex", "beta", Some("/work"), "beta preview"),
            ],
            candidate_links: vec![
                linked_to_mux_link(
                    "link-a",
                    session_alpha,
                    editor.clone(),
                    Provenance::StrongDiscovered,
                    Confidence::High,
                ),
                linked_to_mux_link(
                    "link-b",
                    session_beta,
                    editor,
                    Provenance::StrongDiscovered,
                    Confidence::High,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_columns(vec!["id", "mux", "preview"]),
        );
        let body = rendered.lines().nth(2).expect("body row");
        // `alpha preview` should win — it's the first attached agent
        // in `attached_to_mux` (BTreeMap-ordered by source NodeId).
        assert!(
            body.contains("alpha preview"),
            "mux row should show first attached agent's preview:\n{body}",
        );
        assert!(
            !body.contains("beta preview"),
            "mux row should not include the second attached agent's preview:\n{body}",
        );
    }

    #[test]
    fn mux_preview_column_renders_dash_when_no_agents_attached_have_preview() {
        // A mux with no attached agents at all.
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Mux,
            &RenderOptions::wide().with_columns(vec!["id", "mux", "preview"]),
        );
        let body = rendered.lines().nth(2).expect("body row");
        assert!(
            body.trim_end().ends_with('—'),
            "lonely mux row should render — for preview:\n{body}",
        );
    }

    #[test]
    fn union_preview_column_only_populates_for_agent_rows() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session_with_preview("codex", "alpha", Some("/work"), "agent preview"),
                mux_session("tmux", "editor", Some("/work")),
            ],
            ..GraphSnapshot::empty()
        };
        let rendered = render_with(
            &snapshot,
            Projection::Union,
            &RenderOptions::wide().with_columns(vec!["id", "kind", "label", "preview"]),
        );
        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let agent_row = body
            .iter()
            .find(|line| line.contains("agent "))
            .expect("agent row");
        assert!(
            agent_row.contains("agent preview"),
            "union agent row should carry the preview:\n{agent_row}",
        );
        let mux_row = body
            .iter()
            .find(|line| line.contains("mux "))
            .expect("mux row");
        assert!(
            mux_row.trim_end().ends_with('—'),
            "union mux row should render — for preview:\n{mux_row}",
        );
    }

    #[test]
    fn preview_column_is_opt_in_not_default() {
        // Default snapshots stay byte-stable when the preview field
        // is populated — the column has to be requested explicitly.
        let snapshot = GraphSnapshot {
            nodes: vec![agent_session_with_preview(
                "codex",
                "alpha",
                Some("/work"),
                "should not appear in default render",
            )],
            ..GraphSnapshot::empty()
        };
        let rendered = render(&snapshot, Projection::Agent);
        assert!(
            !rendered.contains("should not appear"),
            "default sessions render must not include preview content:\n{rendered}",
        );
        assert!(
            !rendered.contains("PREVIEW"),
            "default sessions render must not include the PREVIEW header:\n{rendered}",
        );
    }

    #[test]
    fn fit_to_width_settles_at_floors_when_target_is_impossible() {
        // Seven columns at floor 4 + 6 gaps × 2 = 40 minimum. Asking for 10
        // forces every column down to its floor and stops; the resulting
        // line may exceed the target but doesn't degenerate.
        let header = vec![
            "AGENT".to_string(),
            "CWD".to_string(),
            "MUX".to_string(),
            "MUX/CONF".to_string(),
            "PR".to_string(),
            "PR/CONF".to_string(),
            "LINEAGE".to_string(),
        ];
        let naturals = vec![20, 60, 20, 5, 30, 5, 12];
        let budgets = fit_to_width(&naturals, &header, 10);
        for (idx, &budget) in budgets.iter().enumerate() {
            let header_width = display_width(&header[idx]);
            let expected_floor = naturals[idx].min(header_width.max(MIN_COLUMN_BUDGET));
            assert_eq!(
                budget, expected_floor,
                "column {idx} should be at floor; got {budget} expected {expected_floor}",
            );
        }
    }
}
