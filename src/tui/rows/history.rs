//! The History view's row tree (ADR 0140).
//!
//! One row per record whose entity is gone, newest sighting first,
//! grouped by project, kind, or not at all. Every row is a
//! [`MixedRow`], so the kinds share columns with the Pinned view.
//! `Enter` rebuilds the record; the menu pins, forgets, or shows the
//! plan.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{
    CheckoutId, ForgePrId, GraphSnapshot, HistoryKind, MuxSessionId, NodeId, NodeKind,
    NodePresence, RecordMeta, RepoId,
};
use crate::tui::HistoryGrouping;

use super::pinned::{join_dot, project_for_path};
use super::{
    GroupRow, MixedChip, MixedGrid, MixedRow, MixedTone, PinMarker, PrStatus, Row, RowId, RowKind,
    RowTree, ViewLabel, format_recency, harness_label, shorten_home,
};

pub struct HistoryBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub grouping: HistoryGrouping,
    /// List records a pin points at too.
    pub show_pinned: bool,
}

/// The bucket records with no local project fall into.
pub const OTHER_BUCKET: &str = "Other";

/// A record the view lists, with what its row shows.
pub struct Entry<'a> {
    pub kind: HistoryKind,
    pub meta: &'a RecordMeta,
    pub last_seen: i64,
    pub row: MixedRow,
    project: Option<String>,
}

/// The records the History view lists: those whose entity a targeted
/// check found gone, plus PRs that left the listing once the forge has
/// been read (an unreachable forge proves nothing).
pub fn gone_records<'a>(snapshot: &'a GraphSnapshot, home: Option<&Path>) -> Vec<Entry<'a>> {
    let forge_unreadable = snapshot.diagnostics.iter().any(|d| {
        matches!(
            d,
            crate::model::Diagnostic::ProviderUnavailable { provider, .. }
                if provider == crate::discovery::providers::GITHUB
        )
    });
    let mut out = Vec::new();
    for (kind, meta) in snapshot.history.metas() {
        let last_seen = match (kind, snapshot.history.presence_of(&meta.id)) {
            (_, Some(NodePresence::Unknown { .. })) => continue,
            (HistoryKind::Pr, _) => {
                let listed =
                    NodeId::parse(&meta.key).is_some_and(|id| snapshot.find_node(&id).is_some());
                if listed || forge_unreadable {
                    continue;
                }
                meta.last_seen
            }
            (_, Some(gone @ NodePresence::Gone { .. })) => gone.last_seen(),
            (_, None) => continue,
        };
        let Some(row) = describe(snapshot, kind, meta, home) else {
            continue;
        };
        let project = project_of(snapshot, kind, &meta.id, home);
        out.push(Entry {
            kind,
            meta,
            last_seen,
            row,
            project,
        });
    }
    out.sort_by(|a, b| {
        b.last_seen
            .cmp(&a.last_seen)
            .then_with(|| a.meta.id.cmp(&b.meta.id))
    });
    out
}

pub fn build_history_tree(inputs: HistoryBuildInputs<'_>) -> RowTree {
    let HistoryBuildInputs {
        snapshot,
        home,
        now,
        grouping,
        show_pinned,
    } = inputs;
    let mut entries = gone_records(snapshot, home);
    if !show_pinned {
        entries.retain(|entry| pinned_by(snapshot, &entry.meta.id).is_empty());
    }
    for entry in &mut entries {
        entry.row.age = format_recency(now, Some(entry.last_seen));
        entry.row.age_epoch = Some(entry.last_seen);
    }
    let mut tree = RowTree {
        rows: Vec::new(),
        view: ViewLabel::History,
    };
    let push = |tree: &mut RowTree, entry: &Entry<'_>, depth: u8| {
        tree.rows.push(Row {
            id: RowId::History {
                record_id: entry.meta.id.clone(),
            },
            depth,
            expandable: false,
            kind: RowKind::Mixed(entry.row.clone()),
        });
    };
    match grouping {
        HistoryGrouping::None => {
            for entry in &entries {
                push(&mut tree, entry, 0);
            }
        }
        HistoryGrouping::Project | HistoryGrouping::Kind => {
            let mut buckets: BTreeMap<(u8, String), Vec<&Entry<'_>>> = BTreeMap::new();
            for entry in &entries {
                let key = match grouping {
                    HistoryGrouping::Kind => (
                        HistoryKind::ALL
                            .iter()
                            .position(|k| *k == entry.kind)
                            .map_or(u8::MAX, |i| u8::try_from(i).unwrap_or(u8::MAX)),
                        kind_bucket_label(entry.kind).to_string(),
                    ),
                    _ => match &entry.project {
                        Some(project) => (0, project.clone()),
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
                    push(&mut tree, entry, 1);
                }
            }
        }
    }
    tree
}

fn kind_bucket_label(kind: HistoryKind) -> &'static str {
    match kind {
        HistoryKind::Mux => "Muxes",
        HistoryKind::Checkout => "Worktrees",
        HistoryKind::Repo => "Repos",
        HistoryKind::Pr => "PRs",
    }
}

/// The graph node a record stands for, ghost or not, built from its
/// recipe so a record's key can change without losing its node.
pub fn node_of(snapshot: &GraphSnapshot, kind: HistoryKind, id: &str) -> Option<NodeId> {
    let history = &snapshot.history;
    Some(match kind {
        HistoryKind::Mux => {
            NodeId::MuxSession(MuxSessionId::new(history.mux(id)?.recipe.native_id()))
        }
        HistoryKind::Checkout => {
            let r = &history.checkout(id)?.recipe;
            NodeId::Checkout(CheckoutId::new(RepoId::new(r.common_dir()), r.root.clone()))
        }
        HistoryKind::Repo => NodeId::Repo(RepoId::new(history.repo(id)?.recipe.common_dir.clone())),
        HistoryKind::Pr => {
            let r = &history.pr(id)?.recipe;
            NodeId::ForgePr(ForgePrId::new(
                r.provider.clone(),
                r.host.clone(),
                r.owner.clone(),
                r.repo.clone(),
                r.number,
            ))
        }
    })
}

fn describe(
    snapshot: &GraphSnapshot,
    kind: HistoryKind,
    meta: &RecordMeta,
    home: Option<&Path>,
) -> Option<MixedRow> {
    let history = &snapshot.history;
    let path = |p: &str| shorten_home(p, home);
    let (node_kind, label, status, chip, content, pr_status) = match kind {
        HistoryKind::Mux => {
            let r = &history.mux(&meta.id)?.recipe;
            (
                NodeKind::MuxSession,
                r.name.clone(),
                super::words::STOPPED.to_string(),
                r.harness
                    .as_deref()
                    .map(|harness| MixedChip::agent(harness_label(harness))),
                r.cwd.as_deref().map(path).unwrap_or_default(),
                None,
            )
        }
        HistoryKind::Checkout => {
            let r = &history.checkout(&meta.id)?.recipe;
            let label = if meta.display.label.is_empty() {
                Path::new(&r.root)
                    .file_name()
                    .map_or_else(|| r.root.clone(), |n| n.to_string_lossy().into_owned())
            } else {
                meta.display.label.clone()
            };
            (
                NodeKind::Checkout,
                label,
                super::words::REMOVED.to_string(),
                None,
                join_dot([
                    crate::history::recorder::repo_name(r.common_dir()),
                    r.branch.clone().unwrap_or_default(),
                ]),
                None,
            )
        }
        HistoryKind::Repo => {
            let r = &history.repo(&meta.id)?.recipe;
            let label = if meta.display.label.is_empty() {
                crate::history::recorder::repo_name(&r.common_dir)
            } else {
                meta.display.label.clone()
            };
            let content = r.remotes.first().map_or_else(
                || path(r.primary_root.as_deref().unwrap_or(&r.common_dir)),
                |remote| remote.url.clone(),
            );
            (
                NodeKind::Repo,
                label,
                super::words::REMOVED.to_string(),
                None,
                content,
                None,
            )
        }
        HistoryKind::Pr => {
            let r = &history.pr(&meta.id)?.recipe;
            let status = PrStatus::of(r.state.as_deref(), r.draft);
            (
                NodeKind::ForgePr,
                format!("#{}", r.number),
                status.word().to_string(),
                None,
                join_dot([
                    format!("{}/{}", r.owner, r.repo),
                    r.title.clone().unwrap_or_default(),
                ]),
                Some(status),
            )
        }
    };
    Some(MixedRow {
        node_kind,
        label: label.replace('\n', " "),
        status,
        tone: MixedTone::Dim,
        age: None,
        age_epoch: None,
        state: None,
        chip,
        content: content.replace('\n', " ").trim().to_string(),
        primary_node: node_of(snapshot, kind, &meta.id)?,
        pr_status,
        marker: if pinned_by(snapshot, &meta.id).is_empty() {
            PinMarker::None
        } else {
            // A record is an entity that stopped or was removed.
            PinMarker::Stopped
        },
        grid: MixedGrid::Columns,
    })
}

fn project_of(
    snapshot: &GraphSnapshot,
    kind: HistoryKind,
    id: &str,
    home: Option<&Path>,
) -> Option<String> {
    let history = &snapshot.history;
    let (path, is_repo) = match kind {
        HistoryKind::Mux => (history.mux(id)?.recipe.cwd.clone()?, false),
        // A linked worktree belongs to the repo it was made from, not to
        // wherever its directory happens to sit.
        HistoryKind::Checkout => {
            let r = &history.checkout(id)?.recipe;
            let common = r.common_dir();
            (
                common.strip_suffix("/.git").unwrap_or(common).to_string(),
                true,
            )
        }
        HistoryKind::Repo => {
            let r = &history.repo(id)?.recipe;
            (
                r.primary_root.clone().unwrap_or_else(|| {
                    r.common_dir
                        .strip_suffix("/.git")
                        .unwrap_or(&r.common_dir)
                        .to_string()
                }),
                true,
            )
        }
        HistoryKind::Pr => return None,
    };
    project_for_path(snapshot, &path, is_repo, home)
}

/// The slots pointing at record `id`, by name.
pub fn pinned_by(snapshot: &GraphSnapshot, id: &str) -> Vec<String> {
    let target = crate::model::PinSlot::history_target(id);
    let key = snapshot.history.meta(id).map(|(_, m)| m.key.clone());
    snapshot
        .pins
        .iter()
        .filter(|slot| {
            slot.target
                .as_deref()
                .is_some_and(|t| t == target || Some(t) == key.as_deref())
        })
        .map(|slot| slot.id.clone())
        .collect()
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
