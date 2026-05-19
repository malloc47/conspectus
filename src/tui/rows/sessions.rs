//! Sessions row-tree builder.
//!
//! Implements the locked sessions-view rules from
//! `docs/tui-sessions-mockup.md` / phase-08:
//!
//! - Workspace → repo → worktree → agent session lineage.
//! - The worktree level renders only when its repo has ≥ 2
//!   worktrees inside the visible set.
//! - Agent sessions with ≥ 2 active `LinkedToMux` candidates expose
//!   an expandable sub-tree of candidate-mux child rows. The
//!   resolver-preferred candidate is marked.
//! - No per-row activity indicator.
//! - Paths render with `~` shortening.
//! - Sessions without a resolved repo/worktree land in the
//!   "Ungrouped" bucket (one synthetic group at the top level,
//!   regardless of `SessionsGrouping`).
//!
//! Today's `AgentSessionNode` does not carry an activity epoch, so
//! `recency` / `activity_epoch` on the row are always `None` in
//! v1. Populating them is tracked as a follow-on backlog story.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{
    AgentSessionNode, GraphLink, GraphNode, GraphSnapshot, MuxSessionNode, NodeId, RelationKind,
    RepoId, WorkspaceId, WorktreeId, WorktreeNode,
};
use crate::tui::SessionsGrouping;
use crate::tui::rows::{
    AgentSessionRow, GroupRow, MuxCandidateRow, MuxIndicator, Row, RowId, RowKind, RowTree,
    ViewLabel, format_recency, harness_label, shorten_home,
};

/// Inputs to the sessions builder. Tests construct these directly;
/// the runtime fills them from the resolved snapshot, the parsed
/// `RunConfig`, and the current wall clock.
#[derive(Debug, Clone)]
pub struct SessionsBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub grouping: SessionsGrouping,
    /// User home directory used for `~`-shortening. Pass `None` to
    /// disable shortening (paths render in full).
    pub home: Option<&'a Path>,
    /// Wall-clock epoch (Unix seconds) used to compute recency.
    /// Pass `None` to leave recency blank — useful for tests.
    pub now: Option<i64>,
}

/// Build the sessions row tree. Pure: depends only on the inputs,
/// no I/O, no clock reads, no env access.
pub fn build_sessions_tree(inputs: SessionsBuildInputs<'_>) -> RowTree {
    let index = SessionsIndex::new(inputs.snapshot);
    let mut tree = RowTree {
        view: ViewLabel::Sessions,
        ..RowTree::default()
    };

    // Bucket sessions by their grouping key. The key shape depends
    // on `inputs.grouping`; the "Ungrouped" bucket catches sessions
    // whose worktree/repo lookup fails.
    let mut buckets: BTreeMap<GroupKey, Vec<SessionEntry<'_>>> = BTreeMap::new();
    let mut ungrouped: Vec<SessionEntry<'_>> = Vec::new();

    for (session_id, session) in &index.agent_sessions {
        let entry = SessionEntry {
            id: session_id.clone(),
            node: session,
        };
        match resolve_group_key(&entry, &index, inputs.grouping) {
            Some(key) => buckets.entry(key).or_default().push(entry),
            None => ungrouped.push(entry),
        }
    }

    let mut session_short_ids =
        ShortIds::from_sessions(buckets.values().flatten().chain(&ungrouped));

    let mut ctx = EmitCtx {
        tree: &mut tree,
        index: &index,
        short_ids: &mut session_short_ids,
        home: inputs.home,
        now: inputs.now,
        grouping: inputs.grouping,
    };

    for (key, sessions) in buckets {
        emit_group(&mut ctx, key, sessions);
    }

    if !ungrouped.is_empty() {
        emit_ungrouped(&mut ctx, ungrouped);
    }

    tree
}

/// Bundle of mutable + read-only context threaded through the emit
/// helpers so each helper isn't an 8-argument signature.
struct EmitCtx<'a, 'snap> {
    tree: &'a mut RowTree,
    index: &'a SessionsIndex<'snap>,
    short_ids: &'a mut ShortIds,
    home: Option<&'a Path>,
    now: Option<i64>,
    grouping: SessionsGrouping,
}

// -----------------------------------------------------------------------------
// Index built once per build call
// -----------------------------------------------------------------------------

struct SessionsIndex<'a> {
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    worktrees: BTreeMap<NodeId, &'a WorktreeNode>,
    /// `(source, relation)` → all *active* candidate links. Mirrors
    /// `SnapshotView::by_source_relation` in `output::table` but
    /// scoped to the TUI's needs.
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
}

impl<'a> SessionsIndex<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut worktrees = BTreeMap::new();

        for node in &snapshot.nodes {
            let id = node.id();
            match node {
                GraphNode::AgentSession(session) => {
                    agent_sessions.insert(id, session);
                }
                GraphNode::MuxSession(mux) => {
                    mux_sessions.insert(id, mux);
                }
                GraphNode::Worktree(worktree) => {
                    worktrees.insert(id, worktree);
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
            worktrees,
            by_source_relation,
        }
    }

    /// Find the worktree node whose root equals `cwd`. Mirrors the
    /// behavior of `output::table::session_worktree_root`.
    fn worktree_for_cwd(&self, cwd: &str) -> Option<(&WorktreeId, &WorktreeNode)> {
        self.worktrees.iter().find_map(|(id, node)| {
            if let NodeId::Worktree(wt_id) = id
                && wt_id.root == cwd
            {
                Some((wt_id, *node))
            } else {
                None
            }
        })
    }

    /// Count worktrees that belong to `repo`. Used to apply the
    /// "show worktree level only when ≥ 2 worktrees" rule.
    fn worktree_count_for_repo(&self, repo: &RepoId) -> usize {
        self.worktrees
            .keys()
            .filter(|id| match id {
                NodeId::Worktree(wt_id) => &wt_id.repo == repo,
                _ => false,
            })
            .count()
    }

    /// Resolve the workspace that contains `repo`, if the graph
    /// records one via `WorkspaceContainsRepo`. Returns the first
    /// preferred-by-provenance link's target.
    fn workspace_for_repo(&self, repo: &NodeId) -> Option<&WorkspaceId> {
        for ((source, relation), links) in &self.by_source_relation {
            if *relation != RelationKind::WorkspaceContainsRepo {
                continue;
            }
            for link in links {
                if let crate::model::LinkEndpoint::Node { id } = &link.target
                    && id == repo
                    && let NodeId::Workspace(ws_id) = source
                {
                    return Some(ws_id);
                }
            }
        }
        None
    }

    /// All active `LinkedToMux` candidate links sourced at this
    /// agent session, in their stored order (deterministic per
    /// `by_source_relation`).
    fn mux_candidates_for_session(&self, session: &NodeId) -> &[&'a GraphLink] {
        self.by_source_relation
            .get(&(session.clone(), RelationKind::LinkedToMux))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

// -----------------------------------------------------------------------------
// Grouping
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct GroupKey {
    /// Optional workspace root (only populated under `Graph`
    /// grouping when a workspace is present).
    workspace: Option<String>,
    /// Repo common-dir.
    repo: String,
    /// Repo id (kept alongside `repo` for the `RowId::Group(repo)`
    /// stable identity).
    repo_id: RepoId,
    /// Worktree key. `None` when the grouping mode doesn't include
    /// a worktree level.
    worktree: Option<String>,
}

#[derive(Clone, Debug)]
struct SessionEntry<'a> {
    id: NodeId,
    node: &'a AgentSessionNode,
}

fn resolve_group_key(
    entry: &SessionEntry<'_>,
    index: &SessionsIndex<'_>,
    grouping: SessionsGrouping,
) -> Option<GroupKey> {
    let cwd = entry.node.cwd.as_deref()?;
    let (worktree_id, _worktree) = index.worktree_for_cwd(cwd)?;
    let repo_id = worktree_id.repo.clone();
    let repo_node_id = NodeId::Repo(repo_id.clone());
    let workspace = match grouping {
        SessionsGrouping::Graph => index
            .workspace_for_repo(&repo_node_id)
            .map(|ws| ws.root.clone()),
        // Repo/Worktree/ScanRoot collapse the workspace level.
        // ScanRoot fallback to repo grouping until the runtime
        // wires scan roots into the builder.
        SessionsGrouping::Repo | SessionsGrouping::Worktree | SessionsGrouping::ScanRoot => None,
    };

    let worktree = match grouping {
        SessionsGrouping::Worktree => Some(worktree_id.root.clone()),
        // For graph/repo grouping, the worktree level is decided by
        // the builder's repo-fan-out rule (≥ 2 worktrees) later.
        // We always include the worktree key here so the grouping
        // is stable; the rendering pass collapses it as needed.
        SessionsGrouping::Graph | SessionsGrouping::Repo | SessionsGrouping::ScanRoot => {
            Some(worktree_id.root.clone())
        }
    };

    Some(GroupKey {
        workspace,
        repo: repo_id.common_dir.clone(),
        repo_id,
        worktree,
    })
}

fn emit_group(ctx: &mut EmitCtx<'_, '_>, key: GroupKey, mut sessions: Vec<SessionEntry<'_>>) {
    sessions.sort_by(|a, b| compare_sessions(a, b));

    let mut depth: u8 = 0;
    if let Some(workspace_root) = &key.workspace {
        ctx.tree.rows.push(Row {
            id: RowId::Group(NodeId::Workspace(WorkspaceId {
                root: workspace_root.clone(),
            })),
            depth,
            expandable: true,
            kind: RowKind::Group(GroupRow {
                display_path: shorten_home(workspace_root, ctx.home),
                primary_node: Some(NodeId::Workspace(WorkspaceId {
                    root: workspace_root.clone(),
                })),
            }),
        });
        depth = depth.saturating_add(1);
    }

    // Repo level
    ctx.tree.rows.push(Row {
        id: RowId::Group(NodeId::Repo(key.repo_id.clone())),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(&key.repo, ctx.home),
            primary_node: Some(NodeId::Repo(key.repo_id.clone())),
        }),
    });

    let worktree_should_render = matches!(ctx.grouping, SessionsGrouping::Worktree)
        || ctx.index.worktree_count_for_repo(&key.repo_id) >= 2;
    let session_depth = if worktree_should_render && let Some(wt_root) = &key.worktree {
        push_worktree_row(
            ctx.tree,
            depth.saturating_add(1),
            &key.repo_id,
            wt_root,
            ctx.home,
        );
        depth.saturating_add(2)
    } else {
        depth.saturating_add(1)
    };

    for entry in sessions {
        emit_session(ctx, session_depth, entry);
    }
}

fn push_worktree_row(
    tree: &mut RowTree,
    depth: u8,
    repo: &RepoId,
    worktree_root: &str,
    home: Option<&Path>,
) {
    let wt_id = WorktreeId::new(repo.clone(), worktree_root.to_string());
    let node_id = NodeId::Worktree(wt_id);
    tree.rows.push(Row {
        id: RowId::Group(node_id.clone()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: shorten_home(worktree_root, home),
            primary_node: Some(node_id),
        }),
    });
}

fn emit_ungrouped(ctx: &mut EmitCtx<'_, '_>, mut sessions: Vec<SessionEntry<'_>>) {
    sessions.sort_by(|a, b| compare_sessions(a, b));
    ctx.tree.rows.push(Row {
        id: RowId::Synthetic("ungrouped"),
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path: "Ungrouped".to_string(),
            primary_node: None,
        }),
    });
    for entry in sessions {
        emit_session(ctx, 1, entry);
    }
}

fn emit_session(ctx: &mut EmitCtx<'_, '_>, depth: u8, entry: SessionEntry<'_>) {
    let candidates = ctx.index.mux_candidates_for_session(&entry.id);
    let preferred = pick_preferred_link(candidates);
    let mux_state = match candidates.len() {
        0 => MuxIndicator::Unmuxed,
        1 => MuxIndicator::Attached,
        n => MuxIndicator::Ambiguous { candidate_count: n },
    };
    let expandable = candidates.len() >= 2;
    let short_id = ctx.short_ids.short_id(&entry.id);
    let cwd_display = entry
        .node
        .cwd
        .as_deref()
        .map(|cwd| shorten_home(cwd, ctx.home));

    ctx.tree.rows.push(Row {
        id: RowId::AgentSession(entry.id.clone()),
        depth,
        expandable,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: entry.node.id.clone(),
            short_id,
            harness_label: harness_label(&entry.node.harness_key),
            cwd_display,
            recency: format_recency(ctx.now, None),
            activity_epoch: None,
            mux_state,
            preview: entry.node.last_message_preview.clone(),
            title: entry.node.title.clone(),
            primary_node: entry.id.clone(),
        }),
    });

    if expandable {
        let preferred_target = preferred.and_then(|link| link.target_node_id().cloned());
        for link in candidates {
            let Some(target) = link.target_node_id() else {
                continue;
            };
            let mux_id = match target {
                NodeId::MuxSession(id) => id.clone(),
                _ => continue,
            };
            let mux_node = ctx.index.mux_sessions.get(target).copied();
            let mux_label = mux_node
                .map(mux_session_label)
                .unwrap_or_else(|| format!("{}:{}", mux_id.native_id, mux_id.native_id));
            let is_preferred = preferred_target.as_ref() == Some(target);
            ctx.tree.rows.push(Row {
                id: RowId::AgentSessionMuxCandidate {
                    agent: entry.id.clone(),
                    mux: target.clone(),
                },
                depth: depth.saturating_add(1),
                expandable: false,
                kind: RowKind::AgentSessionMuxCandidate(MuxCandidateRow {
                    mux: mux_id,
                    mux_label,
                    is_preferred,
                    primary_node: target.clone(),
                }),
            });
        }
    }
}

fn mux_session_label(node: &MuxSessionNode) -> String {
    format!("{}:{}", node.backend, node.native_id)
}

/// Sessions within a group sort by recency desc (None last), then
/// alphabetical by harness then short id for a stable tie-breaker.
/// Today, `activity_epoch` is always `None`, so this collapses to
/// the alphabetical tiebreak; once harness adapters populate the
/// epoch, sorting becomes recency-first automatically.
fn compare_sessions(a: &SessionEntry<'_>, b: &SessionEntry<'_>) -> std::cmp::Ordering {
    a.node
        .harness_key
        .cmp(&b.node.harness_key)
        .then_with(|| a.id.cmp(&b.id))
}

fn pick_preferred_link<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links.to_vec();
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
// Short ids
// -----------------------------------------------------------------------------

/// Tracks the per-tree short-id prefix length so all rows share a
/// consistent floor and grow only as collisions force. Mirrors
/// `output::table::unique_prefix_len` but tailored to the agent
/// session set.
struct ShortIds {
    prefix_len: usize,
    table: BTreeMap<NodeId, String>,
}

impl ShortIds {
    const FLOOR: usize = 6;

    fn from_sessions<'a, I>(sessions: I) -> Self
    where
        I: Iterator<Item = &'a SessionEntry<'a>>,
    {
        let full_ids: Vec<(NodeId, String)> = sessions
            .map(|entry| {
                let full = crate::output::table::node_short_id(&entry.id);
                (entry.id.clone(), full)
            })
            .collect();

        let prefix_len = unique_prefix_len(&full_ids);
        let mut table = BTreeMap::new();
        for (id, full) in full_ids {
            let take = prefix_len.min(full.len());
            table.insert(id, full[..take].to_string());
        }
        Self { prefix_len, table }
    }

    fn short_id(&self, id: &NodeId) -> String {
        if let Some(found) = self.table.get(id) {
            return found.clone();
        }
        // Fallback: compute on the fly at the same prefix length.
        // Should not happen in normal use, but the API is total.
        let full = crate::output::table::node_short_id(id);
        let take = self.prefix_len.min(full.len());
        full[..take].to_string()
    }
}

fn unique_prefix_len(ids: &[(NodeId, String)]) -> usize {
    if ids.len() <= 1 {
        return ShortIds::FLOOR;
    }
    let cap = ids
        .iter()
        .map(|(_, s)| s.len())
        .max()
        .unwrap_or(ShortIds::FLOOR);
    for len in ShortIds::FLOOR..=cap {
        let mut seen = std::collections::HashSet::new();
        let mut unique = true;
        for (_, full) in ids {
            let take = len.min(full.len());
            if !seen.insert(&full[..take]) {
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

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, GraphLink, GraphSnapshot, LinkEndpoint,
        LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId, RepoNode, WorktreeId,
        WorktreeNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    fn repo(common_dir: &str) -> GraphNode {
        GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
    }

    fn worktree(repo_common: &str, root: &str) -> GraphNode {
        GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(RepoId::new(repo_common), root.to_string()),
            root: root.to_string(),
            git_dir: None,
            current_branch: None,
        })
    }

    fn agent_session(
        harness: &str,
        scope: &str,
        key: &str,
        cwd: Option<&str>,
        title: Option<&str>,
        preview: Option<&str>,
    ) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: title.map(str::to_string),
            last_message_preview: preview.map(str::to_string),
        })
    }

    fn mux_node(backend: &str, native_id: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native_id),
            backend: backend.to_string(),
            native_id: native_id.to_string(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux(
        session: &NodeId,
        mux: &NodeId,
        provenance: Provenance,
        suffix: &str,
    ) -> GraphLink {
        GraphLink {
            id: format!("session-mux-{suffix}"),
            source: session.clone(),
            target: LinkEndpoint::Node { id: mux.clone() },
            relation: RelationKind::LinkedToMux,
            provenance,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    fn build(inputs: SessionsBuildInputs<'_>) -> RowTree {
        build_sessions_tree(inputs)
    }

    #[test]
    fn empty_snapshot_produces_empty_tree() {
        let snapshot = GraphSnapshot::empty();
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });
        assert!(tree.rows.is_empty());
        assert_eq!(tree.view, ViewLabel::Sessions);
    }

    #[test]
    fn single_session_with_one_worktree_collapses_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            Some("first message"),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        // Expect: repo row, then session row. No worktree row.
        assert_eq!(tree.rows.len(), 2, "{:#?}", tree.rows);
        assert!(matches!(tree.rows[0].kind, RowKind::Group(_)));
        let group = match &tree.rows[0].kind {
            RowKind::Group(g) => g,
            _ => unreachable!(),
        };
        assert_eq!(group.display_path, "~/src/proj");
        assert_eq!(tree.rows[0].depth, 0);

        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
        assert_eq!(tree.rows[1].depth, 1);
    }

    #[test]
    fn two_worktrees_in_same_repo_show_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/wt/featx"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "def",
            Some("/home/op/wt/featx"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        // Expect: repo row (0) → worktree row (1) → session row (2) for each worktree group.
        let depths: Vec<u8> = tree.rows.iter().map(|r| r.depth).collect();
        // Two repo rows (one per group key — workspace is None, so
        // grouping is per (repo, worktree); same repo appears twice).
        // The builder emits the repo group row once per group; given
        // two groups with distinct worktree keys, the repo row
        // repeats. That is acceptable for v1 — collapsing duplicate
        // repo rows is its own follow-on (T8-* below).
        assert!(
            depths.contains(&1),
            "expected at least one worktree-depth row, got depths={depths:?}"
        );
        let kinds: Vec<&RowKind> = tree.rows.iter().map(|r| &r.kind).collect();
        let group_count = kinds
            .iter()
            .filter(|k| {
                matches!(
                    k,
                    RowKind::Group(GroupRow {
                        primary_node: Some(NodeId::Worktree(_)),
                        ..
                    })
                )
            })
            .count();
        assert_eq!(
            group_count, 2,
            "expected two worktree group rows, kinds={kinds:#?}"
        );
    }

    #[test]
    fn unmuxed_session_has_unmuxed_indicator_and_is_a_leaf() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row present");
        match &session_row.kind {
            RowKind::AgentSession(row) => {
                assert_eq!(row.mux_state, MuxIndicator::Unmuxed);
            }
            _ => unreachable!(),
        }
        assert!(!session_row.expandable);
    }

    #[test]
    fn single_mux_link_yields_attached_indicator() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &mux_id,
            Provenance::Discovered,
            "1",
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        let row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row");
        match &row.kind {
            RowKind::AgentSession(s) => assert_eq!(s.mux_state, MuxIndicator::Attached),
            _ => unreachable!(),
        }
        assert!(!row.expandable);
    }

    #[test]
    fn two_mux_links_yield_ambiguous_and_expandable_with_candidate_children() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        snapshot.nodes.push(mux_node("tmux", "editor"));
        snapshot.nodes.push(mux_node("tmux", "scratch"));
        // editor is StrongDiscovered, so it should be the preferred
        // candidate; scratch is Discovered.
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &editor,
            Provenance::StrongDiscovered,
            "1",
        ));
        snapshot.candidate_links.push(linked_to_mux(
            &session_id,
            &scratch,
            Provenance::Discovered,
            "2",
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        let session_row = tree
            .rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::AgentSession(_)))
            .expect("session row");
        match &session_row.kind {
            RowKind::AgentSession(s) => {
                assert_eq!(s.mux_state, MuxIndicator::Ambiguous { candidate_count: 2 });
            }
            _ => unreachable!(),
        }
        assert!(session_row.expandable);

        let candidate_rows: Vec<&MuxCandidateRow> = tree
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::AgentSessionMuxCandidate(row) => Some(row),
                _ => None,
            })
            .collect();
        assert_eq!(candidate_rows.len(), 2, "two candidate child rows");
        let preferred = candidate_rows
            .iter()
            .find(|c| c.is_preferred)
            .expect("a preferred candidate is marked");
        assert_eq!(preferred.mux_label, "tmux:editor");
        let alt = candidate_rows
            .iter()
            .find(|c| !c.is_preferred)
            .expect("an alternate candidate is present");
        assert_eq!(alt.mux_label, "tmux:scratch");
    }

    #[test]
    fn orphan_session_lands_in_ungrouped_bucket() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent_session("codex", "/state", "abc", None, None, None));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        assert_eq!(tree.rows.len(), 2);
        match &tree.rows[0].kind {
            RowKind::Group(g) => assert_eq!(g.display_path, "Ungrouped"),
            _ => panic!("first row should be the Ungrouped group"),
        }
        assert!(matches!(tree.rows[1].kind, RowKind::AgentSession(_)));
    }

    #[test]
    fn session_row_carries_title_preview_and_short_id() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "claude-code",
            "/state",
            "xyz",
            Some("/home/op/src/proj"),
            Some("Phase 8 mockup"),
            Some("could you give me a bit more co…"),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Graph,
            home: Some(home().as_path()),
            now: None,
        });

        let row = tree
            .rows
            .iter()
            .find_map(|r| match &r.kind {
                RowKind::AgentSession(s) => Some(s),
                _ => None,
            })
            .expect("session row");
        assert_eq!(row.harness_label, "claude");
        assert_eq!(row.title.as_deref(), Some("Phase 8 mockup"));
        assert_eq!(
            row.preview.as_deref(),
            Some("could you give me a bit more co…")
        );
        assert_eq!(row.cwd_display.as_deref(), Some("~/src/proj"));
        assert_eq!(row.short_id.len(), 6, "short id floor at 6 chars");
    }

    #[test]
    fn worktree_grouping_always_shows_worktree_level() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(repo("/home/op/src/proj"));
        snapshot
            .nodes
            .push(worktree("/home/op/src/proj", "/home/op/src/proj"));
        snapshot.nodes.push(agent_session(
            "codex",
            "/state",
            "abc",
            Some("/home/op/src/proj"),
            None,
            None,
        ));
        let snapshot = resolve_snapshot(snapshot);
        let tree = build(SessionsBuildInputs {
            snapshot: &snapshot,
            grouping: SessionsGrouping::Worktree,
            home: Some(home().as_path()),
            now: None,
        });

        // With explicit worktree grouping, the worktree row is
        // always present even though there's a single worktree.
        let has_worktree_group = tree.rows.iter().any(|r| {
            matches!(
                r.kind,
                RowKind::Group(GroupRow {
                    primary_node: Some(NodeId::Worktree(_)),
                    ..
                })
            )
        });
        assert!(
            has_worktree_group,
            "explicit worktree grouping should show the worktree level"
        );
    }
}
