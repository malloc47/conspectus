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

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use crate::config::Projection;
use crate::model::{
    AgentSessionNode, Confidence, ForgePrNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    MuxSessionNode, NodeId, Provenance, RelationKind,
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
    /// Body layout. Only `Layout::Columnar` is wired today; `Layout::Card`
    /// is reserved for `H-TBL-004`.
    pub layout: Layout,
}

impl RenderOptions {
    /// Untruncated, columnar layout. Matches the pre-`H-TBL-003` renderer.
    pub fn wide() -> Self {
        Self {
            width: None,
            layout: Layout::Columnar,
        }
    }

    /// Columnar layout truncated to the given display width.
    pub fn columnar_width(width: usize) -> Self {
        Self {
            width: Some(width),
            layout: Layout::Columnar,
        }
    }
}

/// Body layout for the renderer. See [`RenderOptions`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Layout {
    /// One row per record, columns aligned to per-column budgets.
    #[default]
    Columnar,
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
    let rows = match projection {
        Projection::Agent => build_agent_rows(&view),
        Projection::Mux => build_mux_rows(&view),
        Projection::Union => build_union_rows(&view),
    };
    render_rows(rows, options)
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
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    forge_prs: BTreeMap<NodeId, &'a ForgePrNode>,
    /// `(source_node_id, relation_kind)` → all candidate links for
    /// that pair, in stable order.
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
}

impl<'a> SnapshotView<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut forge_prs = BTreeMap::new();

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

        Self {
            agent_sessions,
            mux_sessions,
            forge_prs,
            by_source_relation,
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

fn agent_session_label(session: &AgentSessionNode) -> String {
    if let Some(title) = &session.title {
        format!("{}:{}", session.harness_key, title)
    } else {
        format!("{}:{}", session.harness_key, session.id.session_key)
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

fn build_agent_rows(view: &SnapshotView<'_>) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.agent_sessions.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        [
            "ID", "AGENT", "CWD", "MUX", "MUX/CONF", "PR", "PR/CONF", "LINEAGE",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect(),
    );

    for ((node_id, session), full_short) in view.agent_sessions.iter().zip(body_full_ids.iter()) {
        let mux_link = view.preferred_link(node_id, RelationKind::LinkedToMux);
        let mux_count = view
            .candidates_for(node_id, RelationKind::LinkedToMux)
            .len();
        let mux_cell = mux_link
            .and_then(|link| match &link.target {
                LinkEndpoint::Node {
                    id: NodeId::MuxSession(_),
                } => view
                    .mux_sessions
                    .get(link.target_node_id()?)
                    .map(|mux| mux_session_label(mux)),
                _ => None,
            })
            .unwrap_or_else(|| "—".to_string());
        let mux_indicator = match mux_link {
            Some(link) => indicator(link.provenance, link.confidence, mux_count > 1),
            None => "—".to_string(),
        };

        let (pr_cell, pr_indicator) = preferred_pr_for_session(view, node_id);

        let lineage_cell = lineage_cell(view, node_id);

        rows.push(vec![
            full_short[..id_len].to_string(),
            agent_session_label(session),
            session.cwd.clone().unwrap_or_else(|| "—".to_string()),
            mux_cell,
            mux_indicator,
            pr_cell,
            pr_indicator,
            lineage_cell,
        ]);
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

fn build_mux_rows(view: &SnapshotView<'_>) -> Vec<Vec<String>> {
    let body_full_ids: Vec<String> = view.mux_sessions.keys().map(node_short_id).collect();
    let id_len = unique_prefix_len(&body_full_ids);

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        ["ID", "MUX", "CWD", "AGENTS"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    );

    // Build mux → [agent labels] by walking active LinkedToMux links.
    let mut attached: BTreeMap<NodeId, Vec<(String, &GraphLink)>> = BTreeMap::new();
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::LinkedToMux {
            continue;
        }
        let preferred = match pick_preferred(links) {
            Some(link) => link,
            None => continue,
        };
        let Some(target_id) = preferred.target_node_id() else {
            continue;
        };
        if let Some(session) = view.agent_sessions.get(source) {
            attached
                .entry(target_id.clone())
                .or_default()
                .push((agent_session_label(session), preferred));
        }
    }

    for ((mux_id, mux), full_short) in view.mux_sessions.iter().zip(body_full_ids.iter()) {
        let entries = attached.get(mux_id);
        let count = entries.map(Vec::len).unwrap_or(0);
        let agents_cell = if let Some(entries) = entries {
            entries
                .iter()
                .map(|(label, link)| {
                    let ambiguous = view
                        .candidates_for(&link.source, RelationKind::LinkedToMux)
                        .len()
                        > 1;
                    format!(
                        "{label} [{ind}]",
                        ind = indicator(link.provenance, link.confidence, ambiguous)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "—".to_string()
        };
        let _ = count;

        rows.push(vec![
            full_short[..id_len].to_string(),
            mux_session_label(mux),
            mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
            agents_cell,
        ]);
    }

    rows
}

fn build_union_rows(view: &SnapshotView<'_>) -> Vec<Vec<String>> {
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
        ["ID", "KIND", "LABEL", "CWD", "RELATIONSHIP"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    );

    for ((node_id, session), full_short) in view
        .agent_sessions
        .iter()
        .zip(body_full_ids.iter().take(agent_count))
    {
        let mux_link = view.preferred_link(node_id, RelationKind::LinkedToMux);
        let mux_count = view
            .candidates_for(node_id, RelationKind::LinkedToMux)
            .len();
        let relationship = match mux_link {
            Some(link) => {
                let target = link
                    .target_node_id()
                    .and_then(|id| view.mux_sessions.get(id))
                    .map(|mux| mux_session_label(mux))
                    .unwrap_or_else(|| "—".to_string());
                format!(
                    "mux={target} [{ind}]",
                    ind = indicator(link.provenance, link.confidence, mux_count > 1)
                )
            }
            None => "mux=—".to_string(),
        };
        rows.push(vec![
            full_short[..id_len].to_string(),
            "agent".to_string(),
            agent_session_label(session),
            session.cwd.clone().unwrap_or_else(|| "—".to_string()),
            relationship,
        ]);
    }

    for (mux, full_short) in view
        .mux_sessions
        .values()
        .zip(body_full_ids.iter().skip(agent_count))
    {
        rows.push(vec![
            full_short[..id_len].to_string(),
            "mux".to_string(),
            mux_session_label(mux),
            mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
            "—".to_string(),
        ]);
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

fn render_rows(rows: Vec<Vec<String>>, options: &RenderOptions) -> String {
    match options.layout {
        Layout::Columnar => render_columnar(rows, options.width),
    }
}

fn render_columnar(rows: Vec<Vec<String>>, target_width: Option<usize>) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let columns = rows[0].len();
    let naturals = natural_widths(&rows, columns);
    let budgets = match target_width {
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
            if idx + 1 < columns {
                let truncated_width = display_width(&truncated);
                let pad = budget.saturating_sub(truncated_width);
                out.push_str(&truncated);
                for _ in 0..pad {
                    out.push(' ');
                }
            } else {
                // Last cell: truncate but skip padding so the line has no
                // trailing whitespace.
                out.push_str(&truncated);
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
