//! The Pinned view's row tree (ADR 0141).
//!
//! One row per pin slot, whatever its kind, in `pins.toml` order or by
//! recency, grouped by project, kind, or not at all. A mux slot expands
//! to the sessions it hosts and a checkout slot to its muxes and their
//! sessions. Every row is a [`MixedRow`], so kinds share columns.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{
    GraphNode, GraphSnapshot, LinkState, NodeId, NodeKind, PinId, PinKind, PinNode, RelationKind,
    SlotStatus, path_is_ancestor_of,
};
use crate::tui::{PinnedGrouping, Sort};

use super::{
    GroupRow, MixedChip, MixedGrid, MixedRow, MixedTone, PinMarker, PrStatus, Row, RowId, RowKind,
    RowTree, ViewLabel, format_recency, harness_label, pin_nodes, repo_display_path, shorten_home,
};

pub struct PinnedBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub grouping: PinnedGrouping,
    pub sort: Sort,
}

/// The bucket slots with no local project fall into.
pub const OTHER_BUCKET: &str = "Other";

struct Entry<'a> {
    pin: &'a PinNode,
    epoch: Option<i64>,
}

pub fn build_pinned_tree(inputs: PinnedBuildInputs<'_>) -> RowTree {
    let PinnedBuildInputs {
        snapshot,
        home,
        now,
        grouping,
        sort,
    } = inputs;
    let mut entries: Vec<Entry<'_>> = pin_nodes(snapshot)
        .into_iter()
        .map(|pin| Entry {
            epoch: describe(snapshot, pin, home).epoch,
            pin,
        })
        .collect();
    if sort == Sort::Recency {
        entries.sort_by(|a, b| {
            b.epoch
                .is_some()
                .cmp(&a.epoch.is_some())
                .then_with(|| b.epoch.cmp(&a.epoch))
                .then_with(|| a.pin.order.cmp(&b.pin.order))
        });
    }

    let mut tree = RowTree {
        rows: Vec::new(),
        view: ViewLabel::Pinned,
    };
    match grouping {
        PinnedGrouping::None => {
            for entry in &entries {
                push_slot(&mut tree, snapshot, entry.pin, 0, home, now);
            }
        }
        PinnedGrouping::Project | PinnedGrouping::Kind => {
            let mut buckets: BTreeMap<(u8, String), Vec<&Entry<'_>>> = BTreeMap::new();
            for entry in &entries {
                let key = match grouping {
                    PinnedGrouping::Kind => (
                        PinKind::ALL
                            .iter()
                            .position(|k| *k == entry.pin.kind)
                            .map_or(u8::MAX, |i| u8::try_from(i).unwrap_or(u8::MAX)),
                        kind_bucket_label(entry.pin.kind).to_string(),
                    ),
                    _ => match project_of(snapshot, entry.pin, home) {
                        Some(project) => (0, project),
                        None => (1, OTHER_BUCKET.to_string()),
                    },
                };
                buckets.entry(key).or_default().push(entry);
            }
            for ((_, label), members) in buckets {
                tree.rows.push(Row {
                    id: RowId::Bucket(label.clone()),
                    depth: 0,
                    expandable: true,
                    kind: RowKind::Group(GroupRow {
                        display_path: label,
                        primary_node: None,
                        is_launch_context: false,
                    }),
                });
                for entry in members {
                    push_slot(&mut tree, snapshot, entry.pin, 1, home, now);
                }
            }
        }
    }
    tree
}

fn kind_bucket_label(kind: PinKind) -> &'static str {
    match kind {
        PinKind::Mux => "Muxes",
        PinKind::Session => "Sessions",
        PinKind::Checkout => "Worktrees",
        PinKind::Repo => "Repos",
        PinKind::Branch => "Branches",
        PinKind::Pr => "PRs",
        PinKind::Workspace => "Workspaces",
        PinKind::Fork => "Forks",
    }
}

fn node_kind(kind: PinKind) -> NodeKind {
    match kind {
        PinKind::Mux => NodeKind::MuxSession,
        PinKind::Session => NodeKind::AgentSession,
        PinKind::Checkout => NodeKind::Checkout,
        PinKind::Repo => NodeKind::Repo,
        PinKind::Branch => NodeKind::Branch,
        PinKind::Pr => NodeKind::ForgePr,
        PinKind::Workspace => NodeKind::Workspace,
        PinKind::Fork => NodeKind::Fork,
    }
}

/// What a slot's row shows besides its name.
struct Described {
    status: String,
    tone: MixedTone,
    epoch: Option<i64>,
    chip: Option<MixedChip>,
    content: String,
    pr_status: Option<PrStatus>,
}

/// Status, age source, and content for one slot (ADR 0141's kind
/// vocabulary).
fn describe(snapshot: &GraphSnapshot, pin: &PinNode, home: Option<&Path>) -> Described {
    let target = pin
        .target_node
        .as_ref()
        .and_then(|id| snapshot.find_node(id));
    let mut pr_status = None;
    let status = match pin.status {
        SlotStatus::Planned => super::words::PLANNED.to_string(),
        SlotStatus::Missing => super::words::MISSING.to_string(),
        // `gone` names an entity; a pin whose entity isn't there reads
        // as stopped (or removed, for things that don't run).
        SlotStatus::Gone => SlotStatus::Gone.label_for(pin.kind).to_string(),
        SlotStatus::Unknown => super::words::UNKNOWN.to_string(),
        SlotStatus::Live => match (pin.kind, target) {
            (PinKind::Session, _) => {
                if pin
                    .target_node
                    .as_ref()
                    .is_some_and(|id| !muxes_of_session(snapshot, id).is_empty())
                {
                    super::words::IN_MUX.to_string()
                } else {
                    super::words::RESUMABLE.to_string()
                }
            }
            (PinKind::Pr, Some(GraphNode::ForgePr(pr))) => {
                let status = PrStatus::of(pr.state.as_deref(), pr.is_draft);
                pr_status = Some(status);
                status.word().to_string()
            }
            _ => super::words::LIVE.to_string(),
        },
    };
    let tone = if pin.status == SlotStatus::Live {
        MixedTone::Normal
    } else {
        MixedTone::Dim
    };
    // When the entity isn't in the graph, its history record says when
    // it was last seen.
    let recorded = || {
        pin.target
            .as_deref()
            .and_then(|t| t.strip_prefix(crate::model::PinSlot::HISTORY_PREFIX))
            .or(pin.target.as_deref())
            .and_then(|id| {
                snapshot
                    .history
                    .meta(id)
                    .or_else(|| snapshot.history.by_key(id))
            })
            .map(|(_, meta)| meta.last_seen)
    };
    let epoch = match pin.status {
        SlotStatus::Live => match target {
            Some(GraphNode::MuxSession(mux)) => mux.activity_epoch,
            Some(GraphNode::AgentSession(session)) => session.last_active_epoch,
            Some(GraphNode::ForgePr(pr)) => pr.updated_epoch.or_else(recorded),
            Some(_) => None,
            None => recorded(),
        },
        SlotStatus::Gone | SlotStatus::Unknown => pin
            .target_node
            .as_ref()
            .and_then(|id| snapshot.presence(id))
            .map(crate::model::NodePresence::last_seen)
            .or_else(recorded),
        SlotStatus::Missing | SlotStatus::Planned => None,
    };
    let (chip, content) = content_of(pin, home);
    Described {
        status,
        tone,
        epoch,
        chip,
        content,
        pr_status,
    }
}

pub(super) fn join_dot(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The chip and the text after it for one pin.
fn content_of(pin: &PinNode, home: Option<&Path>) -> (Option<MixedChip>, String) {
    let text = |name: &str| pin.text(name).unwrap_or_default().to_string();
    let path = |name: &str| {
        pin.text(name)
            .map(|p| shorten_home(p, home))
            .unwrap_or_default()
    };
    let content = match pin.kind {
        PinKind::Mux => {
            let chip = match pin.text("harness") {
                Some(harness) => Some(MixedChip::agent(harness_label(harness))),
                None => super::pin_program(pin).map(MixedChip::program),
            };
            return (chip, path("cwd").replace('\n', " ").trim().to_string());
        }
        PinKind::Session => {
            let chip = pin
                .text("harness")
                .map(|harness| MixedChip::agent(harness_label(harness)));
            return (chip, text("title").replace('\n', " ").trim().to_string());
        }
        PinKind::Checkout => {
            let repo = pin
                .text("repo")
                .map(|r| crate::history::recorder::repo_name(r.strip_prefix("repo:").unwrap_or(r)))
                .unwrap_or_default();
            join_dot([repo, text("branch")])
        }
        PinKind::Repo => {
            let remote = text("remote");
            if remote.is_empty() {
                path("common_dir")
            } else {
                remote
            }
        }
        PinKind::Branch => join_dot([
            pin.text("repo")
                .map(|r| crate::history::recorder::repo_name(r.strip_prefix("repo:").unwrap_or(r)))
                .unwrap_or_default(),
            pin.text("refname")
                .map(|r| crate::history::recorder::short_ref(r).to_string())
                .unwrap_or_default(),
        ]),
        PinKind::Pr => join_dot([text("repo"), text("title")]),
        PinKind::Workspace => path("root"),
        PinKind::Fork => join_dot([text("provider"), text("name")]),
    };
    (None, content.replace('\n', " ").trim().to_string())
}

/// The project a slot belongs to: the nearest local repo above the
/// path its recipe or target names, `~`-shortened.
fn project_of(snapshot: &GraphSnapshot, pin: &PinNode, home: Option<&Path>) -> Option<String> {
    let target_cwd = || {
        pin.target_node
            .as_ref()
            .and_then(|id| snapshot.find_node(id))
            .and_then(|node| match node {
                GraphNode::AgentSession(session) => session.cwd.clone(),
                GraphNode::MuxSession(mux) => mux.cwd.clone(),
                _ => None,
            })
    };
    let path = match pin.kind {
        PinKind::Mux => pin.text("cwd").map(str::to_string).or_else(target_cwd),
        PinKind::Session => target_cwd(),
        PinKind::Checkout => pin
            .text("repo")
            .map(|r| r.strip_prefix("repo:").unwrap_or(r))
            .map(|dir| dir.strip_suffix("/.git").unwrap_or(dir).to_string())
            .or_else(|| pin.text("root").map(str::to_string)),
        PinKind::Repo => pin.text("path").map(str::to_string).or_else(|| {
            pin.text("common_dir")
                .map(|dir| dir.strip_suffix("/.git").unwrap_or(dir).to_string())
        }),
        PinKind::Branch => pin
            .text("repo")
            .map(|r| r.strip_prefix("repo:").unwrap_or(r))
            .map(|dir| dir.strip_suffix("/.git").unwrap_or(dir).to_string()),
        PinKind::Workspace => pin.text("root").map(str::to_string),
        PinKind::Pr | PinKind::Fork => None,
    }?;
    project_for_path(snapshot, &path, pin.kind == PinKind::Repo, home)
}

/// The project a path belongs to: the nearest local repo above it,
/// `~`-shortened. A repo that is gone from the graph names its own
/// path when `self_if_none` is set.
pub(super) fn project_for_path(
    snapshot: &GraphSnapshot,
    path: &str,
    self_if_none: bool,
    home: Option<&Path>,
) -> Option<String> {
    let path = Path::new(path);
    let mut best: Option<String> = None;
    for node in &snapshot.nodes {
        let GraphNode::Repo(repo) = node else {
            continue;
        };
        let root = repo_display_path(&repo.id, Some(repo));
        if path_is_ancestor_of(Path::new(&root), path)
            && best.as_ref().is_none_or(|b| root.len() > b.len())
        {
            best = Some(root);
        }
    }
    let root = best.or_else(|| self_if_none.then(|| path.to_string_lossy().into_owned()))?;
    Some(shorten_home(&root, home))
}

fn push_slot(
    tree: &mut RowTree,
    snapshot: &GraphSnapshot,
    pin: &PinNode,
    depth: u8,
    home: Option<&Path>,
    now: Option<i64>,
) {
    let described = describe(snapshot, pin, home);
    let children = children_of(snapshot, pin, now);
    let pin_node = NodeId::Pin(PinId::new(pin.id.id.clone()));
    tree.rows.push(Row {
        id: RowId::Pin {
            pin_id: pin.id.id.clone(),
        },
        depth,
        expandable: !children.is_empty(),
        kind: RowKind::Mixed(MixedRow {
            node_kind: node_kind(pin.kind),
            label: pin.id.id.clone(),
            status: described.status,
            tone: described.tone,
            age: format_recency(now, described.epoch),
            age_epoch: described.epoch,
            state: None,
            chip: described.chip,
            content: described.content,
            primary_node: pin_node,
            pr_status: described.pr_status,
            marker: if pin.status == SlotStatus::Live {
                PinMarker::Running
            } else {
                PinMarker::Stopped
            },
            grid: MixedGrid::Columns,
        }),
    });
    for (offset, child) in children {
        tree.rows.push(Row {
            id: RowId::Nested {
                pin_id: pin.id.id.clone(),
                node: child.primary_node.clone(),
            },
            depth: depth + offset,
            expandable: false,
            kind: RowKind::Mixed(child),
        });
    }
}

/// Muxes a session is linked to.
fn muxes_of_session(snapshot: &GraphSnapshot, session: &NodeId) -> Vec<NodeId> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && matches!(link.state, LinkState::Active)
                && &link.source == session
        })
        .filter_map(|link| link.target_node_id().cloned())
        .collect()
}

/// Sessions a mux hosts now or hosted last, newest first.
fn sessions_of_mux(snapshot: &GraphSnapshot, mux: &NodeId) -> Vec<NodeId> {
    let mut sessions: Vec<NodeId> = Vec::new();
    for link in &snapshot.candidate_links {
        let hosted = match link.relation {
            RelationKind::LinkedToMux
                if matches!(link.state, LinkState::Active)
                    && link.target_node_id() == Some(mux) =>
            {
                Some(link.source.clone())
            }
            RelationKind::MuxLastHosted if &link.source == mux => link.target_node_id().cloned(),
            _ => None,
        };
        if let Some(id @ NodeId::AgentSession(_)) = hosted
            && !sessions.contains(&id)
        {
            sessions.push(id);
        }
    }
    let epoch = |id: &NodeId| match snapshot.find_node(id) {
        Some(GraphNode::AgentSession(s)) => s.last_active_epoch,
        _ => None,
    };
    sessions.sort_by_key(|id| std::cmp::Reverse(epoch(id)));
    sessions
}

fn session_row(
    snapshot: &GraphSnapshot,
    id: &NodeId,
    mux_live: bool,
    home: Option<&Path>,
    now: Option<i64>,
) -> Option<MixedRow> {
    let Some(GraphNode::AgentSession(session)) = snapshot.find_node(id) else {
        return None;
    };
    let key = &session.id.session_key;
    let mux = snapshot.mux_host_of(id);
    let label = crate::aliases::session_display_name(
        snapshot.aliases.get(id),
        session.title.as_deref(),
        mux.as_ref().map(|host| host.name.as_str()),
    )
    .map_or_else(|| key.chars().take(8).collect(), str::to_string);
    let live = mux_live && snapshot.is_live(id);
    Some(MixedRow {
        node_kind: NodeKind::AgentSession,
        label: label.replace('\n', " "),
        status: if live {
            super::words::IN_MUX
        } else {
            super::words::RESUMABLE
        }
        .to_string(),
        tone: MixedTone::Normal,
        age: format_recency(now, session.last_active_epoch),
        age_epoch: session.last_active_epoch,
        state: None,
        chip: Some(MixedChip::agent(harness_label(&session.harness_key))),
        // The label already carries the title, so the content says where
        // the session ran.
        content: session
            .cwd
            .as_deref()
            .map(|cwd| shorten_home(cwd, home))
            .unwrap_or_default(),
        primary_node: id.clone(),
        pr_status: None,
        marker: PinMarker::None,
        grid: MixedGrid::Columns,
    })
}

fn mux_row(
    snapshot: &GraphSnapshot,
    id: &NodeId,
    home: Option<&Path>,
    now: Option<i64>,
) -> Option<MixedRow> {
    let Some(GraphNode::MuxSession(mux)) = snapshot.find_node(id) else {
        return None;
    };
    Some(MixedRow {
        node_kind: NodeKind::MuxSession,
        label: mux.session_name().to_string(),
        status: super::words::LIVE.to_string(),
        tone: MixedTone::Normal,
        age: format_recency(now, mux.activity_epoch),
        age_epoch: mux.activity_epoch,
        state: None,
        chip: super::mux_program_harness(mux, None, None)
            .map(MixedChip::agent)
            .or_else(|| super::mux_program(mux, None).map(MixedChip::program)),
        content: mux
            .cwd
            .as_deref()
            .map(|cwd| shorten_home(cwd, home))
            .unwrap_or_default(),
        primary_node: id.clone(),
        pr_status: None,
        marker: PinMarker::None,
        grid: MixedGrid::Columns,
    })
}

/// Nested rows with their depth offset below the slot row.
fn children_of(snapshot: &GraphSnapshot, pin: &PinNode, now: Option<i64>) -> Vec<(u8, MixedRow)> {
    let Some(target) = pin.target_node.as_ref() else {
        return Vec::new();
    };
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let home = home.as_deref();
    let mut out = Vec::new();
    match pin.kind {
        PinKind::Mux => {
            let mux_live = pin.status == SlotStatus::Live;
            for id in sessions_of_mux(snapshot, target) {
                out.extend(session_row(snapshot, &id, mux_live, home, now).map(|row| (1, row)));
            }
        }
        PinKind::Checkout if pin.status == SlotStatus::Live => {
            let Some(root) = pin.text("root") else {
                return out;
            };
            let root = Path::new(root);
            let mut muxes: Vec<(&crate::model::MuxSessionNode, NodeId)> = snapshot
                .live_nodes()
                .filter_map(|node| match node {
                    GraphNode::MuxSession(mux)
                        if mux
                            .cwd
                            .as_deref()
                            .is_some_and(|cwd| path_is_ancestor_of(root, Path::new(cwd))) =>
                    {
                        Some((mux, NodeId::MuxSession(mux.id.clone())))
                    }
                    _ => None,
                })
                .collect();
            muxes.sort_by_key(|(mux, _)| std::cmp::Reverse(mux.activity_epoch));
            for (_, id) in muxes {
                out.extend(mux_row(snapshot, &id, home, now).map(|row| (1, row)));
                for session in sessions_of_mux(snapshot, &id) {
                    out.extend(
                        session_row(snapshot, &session, true, home, now).map(|row| (2, row)),
                    );
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
#[path = "pinned_tests.rs"]
mod tests;

/// A world with a slot of every pinnable kind in each state that kind
/// can be in, for the Pinned view's width snapshots.
#[cfg(test)]
pub(crate) fn all_kinds_world() -> GraphSnapshot {
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
        CheckoutRecipe, CheckoutRecord, FieldValue, ForgePrId, ForgePrNode, ForkId, ForkNode,
        HistorySnapshot, MuxRecipe, MuxRecord, MuxSessionId, MuxSessionNode, NodePresence, PinSlot,
        RecordMeta, RepoId, RepoNode, RepoRecipe, RepoRecord, SlotFields, WorkspaceId,
        WorkspaceNode,
    };

    const NOW: i64 = 1_700_000_000;
    fn meta(id: &str, key: &str, last_seen: i64) -> RecordMeta {
        RecordMeta {
            id: id.into(),
            key: key.into(),
            first_seen: 1,
            last_seen,
            origin: Default::default(),
            display: Default::default(),
            rebuilt_from: None,
        }
    }
    fn slot(id: &str, kind: PinKind, target: Option<String>, order: u32) -> PinSlot {
        PinSlot {
            id: id.into(),
            kind,
            target,
            pinned_at: None,
            overrides: SlotFields::new(),
            snapshot: SlotFields::from([("label".to_string(), FieldValue::Text(id.into()))]),
            order,
            legacy: false,
        }
    }
    fn mux_record(id: &str, name: &str, last_seen: i64) -> MuxRecord {
        MuxRecord {
            meta: meta(id, &format!("mux_session:tmux:{name}"), last_seen),
            recipe: MuxRecipe {
                backend: "tmux".into(),
                name: name.into(),
                cwd: Some("/home/op/src/conspectus".into()),
                harness: Some("codex".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }
    }
    fn pr(number: u64, state: &str, draft: bool) -> ForgePrNode {
        ForgePrNode {
            id: ForgePrId::new("github", "github.com", "malloc47", "conspectus", number),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "malloc47".into(),
            repo: "conspectus".into(),
            number,
            state: Some(state.into()),
            title: Some(format!("Change number {number}")),
            url: None,
            updated_epoch: None,
            is_draft: draft,
            head_ref: None,
        }
    }

    let repo_id = RepoId::new("/home/op/src/conspectus/.git");
    let mut repo = RepoNode::new(repo_id.clone());
    repo.source_paths = vec!["/home/op/src/conspectus".into()];
    let checkout = CheckoutNode::new(
        CheckoutId::new(repo_id.clone(), "/home/op/src/conspectus"),
        "/home/op/src/conspectus",
    );
    let mut live_mux = MuxSessionNode::new(MuxSessionId::new("tmux:main"), "tmux", "main");
    live_mux.cwd = Some("/home/op/src/conspectus".into());
    live_mux.activity_epoch = Some(NOW - 60);
    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "aaaaaaaa11"),
        "claude-code",
    );
    session.cwd = Some("/home/op/src/conspectus".into());
    session.title = Some("Fix the ledger merge".into());
    session.last_active_epoch = Some(NOW - 600);
    let branch = BranchNode {
        id: BranchId::new(repo_id.clone(), "refs/heads/main"),
        refname: "refs/heads/main".into(),
        current_commit: None,
        upstream: Some("origin/main".into()),
    };
    let workspace = WorkspaceNode {
        id: WorkspaceId::new("/home/op/src"),
        root: "/home/op/src".into(),
        provider: Some("atelier".into()),
        name: Some("src".into()),
    };
    let fork = ForkNode {
        id: ForkId::new("atelier/fork-1"),
        provider: "atelier".into(),
        provider_source_key: "atelier/fork-1".into(),
        name: Some("spike".into()),
        scope: None,
        capabilities: Vec::new(),
    };

    let history = HistorySnapshot {
        muxes: vec![
            mux_record("h_main", "main", NOW - 60),
            mux_record("h_gone", "gone-mux", NOW - 400_000),
            mux_record("h_unknown", "far-mux", NOW - 7_200),
        ],
        checkouts: vec![CheckoutRecord {
            meta: meta(
                "h_wt",
                "checkout:/home/op/src/conspectus-old",
                NOW - 172_800,
            ),
            recipe: CheckoutRecipe {
                repo: "repo:/home/op/src/conspectus/.git".into(),
                root: "/home/op/src/conspectus-old".into(),
                linked: true,
                branch: Some("feature/old".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        repos: vec![RepoRecord {
            meta: meta("h_repo", "repo:/home/op/src/lost/.git", NOW - 864_000),
            recipe: RepoRecipe {
                common_dir: "/home/op/src/lost/.git".into(),
                primary_root: Some("/home/op/src/lost".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        presence: std::collections::BTreeMap::from([
            (
                "h_gone".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 400_000,
                },
            ),
            (
                "h_unknown".to_string(),
                NodePresence::Unknown {
                    last_seen: NOW - 7_200,
                },
            ),
            (
                "h_wt".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 172_800,
                },
            ),
            (
                "h_repo".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 864_000,
                },
            ),
        ]),
        ..Default::default()
    };

    let node = |id: NodeId| id.to_string();
    let pins = vec![
        slot("mux-live", PinKind::Mux, Some("history:h_main".into()), 0),
        slot("mux-gone", PinKind::Mux, Some("history:h_gone".into()), 1),
        slot(
            "mux-unknown",
            PinKind::Mux,
            Some("history:h_unknown".into()),
            2,
        ),
        slot("mux-planned", PinKind::Mux, None, 3),
        slot(
            "mux-missing",
            PinKind::Mux,
            Some("history:h_deleted".into()),
            4,
        ),
        slot(
            "session-live",
            PinKind::Session,
            Some(node(NodeId::AgentSession(AgentSessionId::new(
                "claude-code",
                "/state",
                "aaaaaaaa11",
            )))),
            5,
        ),
        slot(
            "session-missing",
            PinKind::Session,
            Some(node(NodeId::AgentSession(AgentSessionId::new(
                "claude-code",
                "/state",
                "bbbbbbbb22",
            )))),
            6,
        ),
        slot(
            "worktree-live",
            PinKind::Checkout,
            Some(node(NodeId::Checkout(CheckoutId::new(
                repo_id.clone(),
                "/home/op/src/conspectus",
            )))),
            7,
        ),
        slot(
            "worktree-gone",
            PinKind::Checkout,
            Some("history:h_wt".into()),
            8,
        ),
        slot("worktree-planned", PinKind::Checkout, None, 9),
        slot(
            "repo-live",
            PinKind::Repo,
            Some(node(NodeId::Repo(repo_id.clone()))),
            10,
        ),
        slot(
            "repo-gone",
            PinKind::Repo,
            Some("history:h_repo".into()),
            11,
        ),
        slot(
            "pr-open",
            PinKind::Pr,
            Some(node(NodeId::ForgePr(pr(41, "open", false).id))),
            12,
        ),
        slot(
            "pr-draft",
            PinKind::Pr,
            Some(node(NodeId::ForgePr(pr(42, "open", true).id))),
            13,
        ),
        slot(
            "pr-closed",
            PinKind::Pr,
            Some(node(NodeId::ForgePr(pr(43, "closed", false).id))),
            14,
        ),
        slot(
            "branch-live",
            PinKind::Branch,
            Some(node(NodeId::Branch(BranchId::new(
                repo_id.clone(),
                "refs/heads/main",
            )))),
            15,
        ),
        slot(
            "branch-missing",
            PinKind::Branch,
            Some(node(NodeId::Branch(BranchId::new(
                repo_id,
                "refs/heads/deleted",
            )))),
            16,
        ),
        slot(
            "workspace-live",
            PinKind::Workspace,
            Some(node(NodeId::Workspace(WorkspaceId::new("/home/op/src")))),
            17,
        ),
        slot(
            "fork-live",
            PinKind::Fork,
            Some(node(NodeId::Fork(ForkId::new("atelier/fork-1")))),
            18,
        ),
    ];

    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Repo(repo),
            GraphNode::Checkout(checkout),
            GraphNode::MuxSession(live_mux),
            GraphNode::AgentSession(session),
            GraphNode::Branch(branch),
            GraphNode::Workspace(workspace),
            GraphNode::Fork(fork),
            GraphNode::ForgePr(pr(41, "open", false)),
            GraphNode::ForgePr(pr(42, "open", true)),
            GraphNode::ForgePr(pr(43, "closed", false)),
        ],
        history,
        pins,
        ..GraphSnapshot::empty()
    };
    crate::resolve::resolve_snapshot(snapshot)
}

/// A world with one slot of each state across kinds, for tests and the
/// Pinned view's snapshot checks.
#[cfg(test)]
pub(crate) fn fixture_world() -> GraphSnapshot {
    use crate::model::{
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, CheckoutRecipe, CheckoutRecord,
        FieldValue, HistorySnapshot, MuxIncarnation, MuxRecipe, MuxRecord, MuxSessionId,
        MuxSessionNode, NodePresence, PinSlot, PrRecipe, PrRecord, RecordMeta, RepoId, RepoNode,
        SlotFields,
    };

    fn meta(id: &str, key: &str, last_seen: i64) -> RecordMeta {
        RecordMeta {
            id: id.into(),
            key: key.into(),
            first_seen: 1,
            last_seen,
            origin: Default::default(),
            display: Default::default(),
            rebuilt_from: None,
        }
    }
    fn slot(id: &str, kind: PinKind, target: Option<&str>, order: u32) -> PinSlot {
        PinSlot {
            id: id.into(),
            kind,
            target: target.map(str::to_string),
            pinned_at: None,
            overrides: SlotFields::new(),
            snapshot: SlotFields::from([("label".to_string(), FieldValue::Text(id.into()))]),
            order,
            legacy: false,
        }
    }
    fn text(value: &str) -> FieldValue {
        FieldValue::Text(value.into())
    }

    const NOW: i64 = 1_700_000_000;
    let repo_id = RepoId::new("/home/op/src/conspectus/.git");
    let mut repo = RepoNode::new(repo_id.clone());
    repo.source_paths = vec!["/home/op/src/conspectus".into()];
    let checkout = CheckoutNode::new(
        CheckoutId::new(repo_id, "/home/op/src/conspectus"),
        "/home/op/src/conspectus",
    );

    let mut main_mux = MuxSessionNode::new(
        MuxSessionId::new("tmux:conspectus-main"),
        "tmux",
        "conspectus-main",
    );
    main_mux.cwd = Some("/home/op/src/conspectus".into());
    main_mux.activity_epoch = Some(NOW - 120);
    main_mux.created_epoch = Some(NOW - 9_000);
    main_mux.backend_session_id = Some("$1".into());

    let mut session = AgentSessionNode::new(
        AgentSessionId::new("claude-code", "/state", "e44d01cb5a"),
        "claude-code",
    );
    session.cwd = Some("/home/op/src/conspectus".into());
    session.title = Some("Fix the ledger merge".into());
    session.last_active_epoch = Some(NOW - 1_020);

    let history = HistorySnapshot {
        muxes: vec![
            MuxRecord {
                meta: meta("h_main", "mux_session:tmux:conspectus-main", NOW - 120),
                recipe: MuxRecipe {
                    backend: "tmux".into(),
                    name: "conspectus-main".into(),
                    cwd: Some("/home/op/src/conspectus".into()),
                    incarnation: Some(MuxIncarnation {
                        session_id: Some("$1".into()),
                        created: Some(NOW - 9_000),
                    }),
                    harness: Some("claude-code".into()),
                    ..Default::default()
                },
                extra: Default::default(),
            },
            MuxRecord {
                meta: meta("h_agent", "mux_session:tmux:config-agent3", NOW - 259_200),
                recipe: MuxRecipe {
                    backend: "tmux".into(),
                    name: "config-agent3".into(),
                    cwd: Some("/home/op/src/config".into()),
                    harness: Some("claude-code".into()),
                    ..Default::default()
                },
                extra: Default::default(),
            },
        ],
        checkouts: vec![CheckoutRecord {
            meta: meta(
                "h_wt",
                "checkout:/home/op/src/conspectus-ledger",
                NOW - 172_800,
            ),
            recipe: CheckoutRecipe {
                repo: "repo:/home/op/src/conspectus/.git".into(),
                root: "/home/op/src/conspectus-ledger".into(),
                linked: true,
                branch: Some("feature/ledger".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        prs: vec![PrRecord {
            meta: meta(
                "h_pr",
                "forge_pr:github:github.com/malloc47/conspectus#41",
                NOW - 18_000,
            ),
            recipe: PrRecipe {
                provider: "github".into(),
                host: "github.com".into(),
                owner: "malloc47".into(),
                repo: "conspectus".into(),
                number: 41,
                title: Some("Ledger store and merge".into()),
                state: Some("merged".into()),
                ..Default::default()
            },
            extra: Default::default(),
        }],
        presence: std::collections::BTreeMap::from([
            (
                "h_agent".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 259_200,
                },
            ),
            (
                "h_wt".to_string(),
                NodePresence::Gone {
                    last_seen: NOW - 172_800,
                },
            ),
        ]),
        ..Default::default()
    };

    let mut legacy = slot("old-api", PinKind::Mux, None, 6);
    legacy.legacy = true;
    legacy.overrides = SlotFields::from([
        ("name".to_string(), text("old-api")),
        ("cwd".to_string(), text("/home/op/src/api")),
        ("harness".to_string(), text("codex")),
    ]);
    let mut planned = slot("nightly", PinKind::Mux, None, 5);
    planned.overrides = SlotFields::from([
        ("name".to_string(), text("nightly")),
        ("cwd".to_string(), text("/home/op/src/conspectus")),
        ("harness".to_string(), text("claude-code")),
    ]);

    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Repo(repo),
            GraphNode::Checkout(checkout),
            GraphNode::MuxSession(main_mux),
            GraphNode::AgentSession(session),
        ],
        history,
        pins: vec![
            slot("conspectus-main", PinKind::Mux, Some("history:h_main"), 0),
            slot("config-agent3", PinKind::Mux, Some("history:h_agent"), 1),
            slot(
                "e44d01cb",
                PinKind::Session,
                Some("agent_session:claude-code:/state:e44d01cb5a"),
                2,
            ),
            slot(
                "ledger-worktree",
                PinKind::Checkout,
                Some("history:h_wt"),
                3,
            ),
            slot(
                "ledger-pr",
                PinKind::Pr,
                Some("forge_pr:github:github.com/malloc47/conspectus#41"),
                4,
            ),
            planned,
            legacy,
        ],
        ..GraphSnapshot::empty()
    };
    let mut hosted = crate::model::GraphLink::new(
        "fixture:session-in-main".to_string(),
        NodeId::AgentSession(crate::model::AgentSessionId::new(
            "claude-code",
            "/state",
            "e44d01cb5a",
        )),
        crate::model::LinkEndpoint::Node {
            id: NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:conspectus-main")),
        },
        RelationKind::LinkedToMux,
        crate::model::Provenance::Discovered,
    );
    hosted.state = LinkState::Active;
    snapshot.candidate_links.push(hosted);
    snapshot
        .diagnostics
        .push(crate::model::Diagnostic::LegacyPins {
            count: 1,
            ids: vec!["old-api".into()],
        });
    crate::resolve::resolve_snapshot(snapshot)
}
