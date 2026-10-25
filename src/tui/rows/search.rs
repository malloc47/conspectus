//! The Search view's row tree (ADR 0147).
//!
//! Lists the sessions that match the active content search, ranked.
//! Session rows are the Sessions view's own rows (so every action, pin
//! marker, age, and chip is shared) taken from its flat layout and put in
//! the order the search ranks them. In the per-message grouping each
//! session also lists its matching turns beneath it. In the flat
//! listing there are no session rows at all: every matching turn is a
//! row of its own, newest first, naming its session.

use std::cmp::Ordering;
use std::path::Path;

use crate::filter::RowFilter;
use crate::model::GraphSnapshot;
use crate::tui::{SearchGrouping, SessionsGrouping, Sort};
use crate::viewer::model::TurnKind;
use crate::viewer::search::{
    Grouping, Hit, SessionHits, cmp_hits, cmp_hits_across_sessions, cmp_sessions,
    cmp_sessions_by_recency,
};

use super::sessions::{SessionsBuildInputs, build_sessions_tree};
use super::{GroupRow, HitOrigin, HitRow, Row, RowId, RowKind, RowTree, ViewLabel, format_recency};

pub struct SearchBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub cwd: Option<&'a Path>,
    pub filter: RowFilter,
    pub sort: Sort,
    pub grouping: SearchGrouping,
}

/// The bucket sessions with no known project fall into.
pub const OTHER_BUCKET: &str = "Other";

/// The word a turn kind has in a hit row.
pub fn kind_word(kind: TurnKind) -> &'static str {
    match kind {
        TurnKind::Message => "message",
        TurnKind::CompactionSummary => "summary",
        TurnKind::ToolUse => "tool call",
        TurnKind::ToolResult => "tool output",
        TurnKind::Thinking => "thinking",
    }
}

/// The word a speaker has in a hit row, as in the transcript viewer's
/// gutter.
pub fn role_word(role: crate::viewer::model::TurnRole) -> &'static str {
    use crate::viewer::model::TurnRole;
    match role {
        TurnRole::User => "you",
        TurnRole::Assistant => "ai",
        TurnRole::System => "system",
    }
}

pub fn build_search_tree(inputs: SearchBuildInputs<'_>) -> RowTree {
    let SearchBuildInputs {
        snapshot,
        home,
        now,
        cwd,
        filter,
        sort,
        grouping,
    } = inputs;
    let mut tree = RowTree {
        rows: Vec::new(),
        view: ViewLabel::Search,
    };
    let Some(content) = filter.content.clone() else {
        return tree;
    };
    let flat = build_sessions_tree(SessionsBuildInputs {
        snapshot,
        grouping: SessionsGrouping::None,
        home,
        now,
        cwd,
        filter,
    });
    let mut entries: Vec<Entry> = flat
        .rows
        .into_iter()
        .filter(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| {
            let (harness, key) = session_key(&row);
            let hits = content.hits.get(&harness, &key).cloned();
            let detail_pending = content.group == Grouping::Message
                && hits.is_some()
                && content.details.get(&harness, &key).is_none();
            let children = if content.group == Grouping::Message {
                content
                    .details
                    .get(&harness, &key)
                    .map(|details| hit_rows(&row, details, now))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            Entry {
                row,
                hits,
                children,
                detail_pending,
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        let order = match (&a.hits, &b.hits) {
            (Some(a), Some(b)) => match sort {
                Sort::Hierarchy => cmp_sessions(a, b),
                Sort::Recency => cmp_sessions_by_recency(a, b),
            },
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        order.then_with(|| session_key(&a.row).cmp(&session_key(&b.row)))
    });

    if content.group == Grouping::Flat {
        return build_flat(tree, &entries, &content.details, grouping, now);
    }

    match grouping {
        SearchGrouping::None => {
            for entry in entries {
                push_entry(&mut tree, entry, 0);
            }
        }
        SearchGrouping::Project | SearchGrouping::Harness => {
            // Buckets appear in the order of their best-ranked session.
            let mut buckets: Vec<(String, Vec<Entry>)> = Vec::new();
            for entry in entries {
                let label = bucket_label(&entry.row, grouping);
                match buckets.iter_mut().find(|(name, _)| *name == label) {
                    Some((_, members)) => members.push(entry),
                    None => buckets.push((label, vec![entry])),
                }
            }
            for (label, members) in buckets {
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
                    push_entry(&mut tree, entry, 1);
                }
            }
        }
    }
    tree
}

/// The flat listing: every matching turn of every session whose turns
/// have been read, newest first. Sorting is by the turn's own time in
/// both sorts (relevance has no meaning across sessions' individual
/// messages); ties fall to the session and then the turn order. Under
/// the project and harness groupings the turns sit under bucket headers
/// ordered by their newest turn.
fn build_flat(
    mut tree: RowTree,
    entries: &[Entry],
    details: &crate::filter::ContentDetailMap,
    grouping: SearchGrouping,
    now: Option<i64>,
) -> RowTree {
    let mut all: Vec<((String, String), usize, &Hit)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.hits.is_none() {
            continue;
        }
        let key = session_key(&entry.row);
        for hit in details.get(&key.0, &key.1).unwrap_or_default() {
            all.push((key.clone(), index, hit));
        }
    }
    all.sort_by(|a, b| cmp_hits_across_sessions((&a.0, a.2), (&b.0, b.2)));

    let mut buckets: Vec<(String, Vec<Row>)> = Vec::new();
    for (_, index, hit) in all {
        let Some(row) = flat_hit_row(&entries[index].row, hit, now) else {
            continue;
        };
        let label = match grouping {
            SearchGrouping::None => String::new(),
            _ => bucket_label(&entries[index].row, grouping),
        };
        match buckets.iter_mut().find(|(name, _)| *name == label) {
            Some((_, rows)) => rows.push(row),
            None => buckets.push((label, vec![row])),
        }
    }
    for (label, rows) in buckets {
        let depth = if matches!(grouping, SearchGrouping::None) {
            0
        } else {
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
            1
        };
        for mut row in rows {
            row.depth = depth;
            tree.rows.push(row);
        }
    }
    tree
}

/// A flat row for one matching turn: the turn's columns plus the session
/// it came from.
fn flat_hit_row(session_row: &Row, hit: &Hit, now: Option<i64>) -> Option<Row> {
    let RowKind::AgentSession(session) = &session_row.kind else {
        return None;
    };
    let mut row = hit_rows(session_row, std::slice::from_ref(hit), now).pop()?;
    if let RowKind::Hit(hit_row) = &mut row.kind {
        hit_row.origin = Some(HitOrigin {
            label: session.display_label().map_or_else(
                || session.session.session_key.chars().take(8).collect(),
                str::to_string,
            ),
            harness_label: session.harness_label.clone(),
        });
    }
    Some(row)
}

/// A session row with what the search found in it.
struct Entry {
    row: Row,
    hits: Option<SessionHits>,
    children: Vec<Row>,
    /// The session matched but its turns have not been read yet.
    detail_pending: bool,
}

fn push_entry(tree: &mut RowTree, entry: Entry, depth: u8) {
    let Entry {
        mut row,
        children,
        detail_pending,
        ..
    } = entry;
    row.depth = depth;
    // A session whose turns are not read yet can still be opened: they
    // are read then.
    row.expandable = !children.is_empty() || detail_pending;
    tree.rows.push(row);
    for mut child in children {
        child.depth = depth.saturating_add(1);
        tree.rows.push(child);
    }
}

fn bucket_label(row: &Row, grouping: SearchGrouping) -> String {
    let RowKind::AgentSession(session) = &row.kind else {
        return OTHER_BUCKET.to_string();
    };
    match grouping {
        SearchGrouping::Harness => session.harness_label.clone(),
        _ => session
            .project_display
            .clone()
            .filter(|project| !project.is_empty())
            .unwrap_or_else(|| OTHER_BUCKET.to_string()),
    }
}

/// The rows for a session's matching turns, newest first.
fn hit_rows(session_row: &Row, details: &[Hit], now: Option<i64>) -> Vec<Row> {
    let RowKind::AgentSession(session) = &session_row.kind else {
        return Vec::new();
    };
    let mut ordered: Vec<&Hit> = details.iter().collect();
    ordered.sort_by(|a, b| cmp_hits(a, b));
    ordered
        .into_iter()
        .map(|hit| Row {
            id: RowId::Hit {
                session: session.primary_node.clone(),
                turn: hit.turn_index,
            },
            depth: 0,
            expandable: false,
            kind: RowKind::Hit(HitRow {
                session: session.session.clone(),
                origin: None,
                turn_index: hit.turn_index,
                role: role_word(hit.role),
                kind: kind_word(hit.kind),
                age: hit
                    .timestamp
                    .and_then(|time| format_recency(now, Some(time.timestamp())))
                    .unwrap_or_else(|| crate::tui::icons::marks::NO_VALUE.to_string()),
                snippet: hit.snippet.clone(),
                highlight: hit.highlight.clone(),
            }),
        })
        .collect()
}

fn session_key(row: &Row) -> (String, String) {
    match &row.kind {
        RowKind::AgentSession(session) => (
            session.session.harness_key.clone(),
            session.session.session_key.clone(),
        ),
        _ => (String::new(), String::new()),
    }
}
