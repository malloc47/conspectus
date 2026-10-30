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

use tuikit::age::Age;

use crate::content_search::{MAX_PANE_HITS_PER_ENTRY, PaneHits};
use crate::filter::RowFilter;
use crate::model::{GraphSnapshot, MuxSessionId, NodeId};
use crate::tui::{SearchGrouping, SessionsGrouping, Sort};
use crate::viewer::model::TurnKind;
use crate::viewer::search::{Grouping, Hit, SessionHits, cmp_hits, cmp_hits_across_sessions};

use super::sessions::{SessionsBuildInputs, build_sessions_tree};
use super::{
    GroupRow, HitOrigin, HitRow, PaneHitRow, PaneRow, Row, RowId, RowKind, RowTree, ViewLabel,
};

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

/// The bucket mux panes fall into under the project and harness
/// groupings, which a pane has neither of.
pub const PANES_BUCKET: &str = "Mux panes";

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
    let mut items: Vec<Item> = flat
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
            Item::Session(Entry {
                row,
                hits,
                children,
                detail_pending,
            })
        })
        .collect();
    items.extend(content.panes.iter().map(|pane| {
        let children = if content.group == Grouping::Message {
            pane_hit_rows(pane, now, false)
        } else {
            Vec::new()
        };
        Item::Pane(PaneEntry {
            row: pane_row(pane, now),
            hits: pane.clone(),
            children,
        })
    }));
    items.sort_by(|a, b| {
        let order = match (a.rank(), b.rank()) {
            (Some(a), Some(b)) => match sort {
                Sort::Hierarchy => a.cmp_relevance(&b),
                Sort::Recency => a.cmp_recency(&b),
            },
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        order.then_with(|| a.identity().cmp(&b.identity()))
    });

    if content.group == Grouping::Flat {
        return build_flat(tree, &items, &content.details, grouping, now);
    }

    match grouping {
        SearchGrouping::None => {
            for item in items {
                push_item(&mut tree, item, 0);
            }
        }
        SearchGrouping::Project | SearchGrouping::Harness => {
            // Buckets appear in the order of their best-ranked result.
            let mut buckets: Vec<(String, Vec<Item>)> = Vec::new();
            for item in items {
                let label = item.bucket(grouping);
                match buckets.iter_mut().find(|(name, _)| *name == label) {
                    Some((_, members)) => members.push(item),
                    None => buckets.push((label, vec![item])),
                }
            }
            for (label, members) in buckets {
                tree.rows.push(Row {
                    id: RowId::Bucket(label.clone()),
                    depth: 0,
                    expandable: true,
                    kind: RowKind::Group(GroupRow::at_path(&label, None)),
                });
                for item in members {
                    push_item(&mut tree, item, 1);
                }
            }
        }
    }
    tree
}

/// The flat listing: every matching turn of every session whose turns
/// have been read, and every matching line of every pane, newest first.
/// Sorting is by the turn's own time in both sorts (relevance has no
/// meaning across sessions' individual messages), and a pane's lines by
/// the time its scrollback was read; ties fall to the session and then the
/// turn order, and to sessions before panes. Under the project and
/// harness groupings the rows sit under bucket headers ordered by their
/// newest row.
fn build_flat(
    mut tree: RowTree,
    items: &[Item],
    details: &crate::filter::ContentDetailMap,
    grouping: SearchGrouping,
    now: Option<i64>,
) -> RowTree {
    let mut all: Vec<((String, String), usize, &Hit)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Item::Session(entry) = item else {
            continue;
        };
        if entry.hits.is_none() {
            continue;
        }
        let key = session_key(&entry.row);
        for hit in details.get(&key.0, &key.1).unwrap_or_default() {
            all.push((key.clone(), index, hit));
        }
    }
    all.sort_by(|a, b| cmp_hits_across_sessions((&a.0, a.2), (&b.0, b.2)));

    // Each row with its time and the item it came from. Sessions' turns
    // keep their sorted order; a pane's lines go in as a block before the
    // first row older than the capture.
    let mut placed: Vec<(Option<i64>, usize, Row)> = Vec::new();
    for (_, index, hit) in all {
        let Item::Session(entry) = &items[index] else {
            continue;
        };
        let Some(row) = flat_hit_row(&entry.row, hit, now) else {
            continue;
        };
        placed.push((hit.timestamp.map(|t| t.timestamp()), index, row));
    }
    for (index, item) in items.iter().enumerate() {
        let Item::Pane(entry) = item else {
            continue;
        };
        let time = Some(entry.hits.captured_at);
        let at = placed
            .iter()
            .position(|(other, ..)| *other < time)
            .unwrap_or(placed.len());
        let rows = pane_hit_rows(&entry.hits, now, true);
        placed.splice(at..at, rows.into_iter().map(|row| (time, index, row)));
    }

    let mut buckets: Vec<(String, Vec<Row>)> = Vec::new();
    for (_, index, row) in placed {
        let label = match grouping {
            SearchGrouping::None => String::new(),
            _ => items[index].bucket(grouping),
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
                kind: RowKind::Group(GroupRow::at_path(&label, None)),
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

/// A pane row with its matching lines.
struct PaneEntry {
    row: Row,
    hits: PaneHits,
    children: Vec<Row>,
}

/// One ranked result of the Search view: a session or a pane.
enum Item {
    Session(Entry),
    Pane(PaneEntry),
}

/// What results are ranked by, whichever source they come from.
struct Rank {
    count: usize,
    time: Option<i64>,
    pane: bool,
}

impl Rank {
    /// More matches first, then the newer, then sessions before panes.
    fn cmp_relevance(&self, other: &Self) -> Ordering {
        other
            .count
            .cmp(&self.count)
            .then_with(|| other.time.cmp(&self.time))
            .then_with(|| self.pane.cmp(&other.pane))
    }

    /// The newer first, then more matches, then sessions before panes.
    fn cmp_recency(&self, other: &Self) -> Ordering {
        other
            .time
            .cmp(&self.time)
            .then_with(|| other.count.cmp(&self.count))
            .then_with(|| self.pane.cmp(&other.pane))
    }
}

impl Item {
    fn rank(&self) -> Option<Rank> {
        match self {
            Self::Session(entry) => entry.hits.as_ref().map(|hits| Rank {
                count: hits.count,
                time: hits.latest.map(|t| t.timestamp()),
                pane: false,
            }),
            Self::Pane(entry) => Some(Rank {
                count: entry.hits.count,
                time: Some(entry.hits.captured_at),
                pane: true,
            }),
        }
    }

    /// A tie-breaking key: the session's, or the pane's mux and id.
    fn identity(&self) -> (String, String) {
        match self {
            Self::Session(entry) => session_key(&entry.row),
            Self::Pane(entry) => (
                entry.hits.pane.mux.native_id(),
                entry.hits.pane.pane_id.clone(),
            ),
        }
    }

    fn bucket(&self, grouping: SearchGrouping) -> String {
        match self {
            Self::Session(entry) => bucket_label(&entry.row, grouping),
            Self::Pane(_) => PANES_BUCKET.to_string(),
        }
    }
}

fn push_item(tree: &mut RowTree, item: Item, depth: u8) {
    match item {
        Item::Session(entry) => push_entry(tree, entry, depth),
        Item::Pane(entry) => {
            let PaneEntry {
                mut row, children, ..
            } = entry;
            row.depth = depth;
            row.expandable = !children.is_empty();
            tree.rows.push(row);
            for mut child in children {
                child.depth = depth.saturating_add(1);
                tree.rows.push(child);
            }
        }
    }
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

/// The node of a pane's mux in the graph.
fn mux_node(pane: &PaneHits) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(pane.pane.mux.native_id()))
}

/// How long ago a pane's scrollback was read.
fn pane_age(pane: &PaneHits, now: Option<i64>) -> String {
    Age::of(Some(pane.captured_at), now)
        .map_or_else(|| tuikit::glyphs::marks::NO_VALUE.to_string(), Age::label)
}

/// A pane's row: where a session row would be.
fn pane_row(pane: &PaneHits, now: Option<i64>) -> Row {
    let newest = pane.hits.last();
    Row {
        id: RowId::Pane {
            mux: mux_node(pane),
            pane: pane.pane.pane_id.clone(),
        },
        depth: 0,
        expandable: false,
        kind: RowKind::Pane(PaneRow {
            pane: pane.pane.clone(),
            mux_label: pane.pane.mux.session.clone(),
            location: format!("{}.{}", pane.pane.window_index, pane.pane.pane_index),
            command: pane.pane.command.clone(),
            count: pane.count,
            deduplicated: pane.deduplicated,
            age: pane_age(pane, now),
            age_epoch: Some(pane.captured_at),
            line: newest.map_or(1, |hit| hit.line),
            snippet: newest.map(|hit| hit.snippet.clone()).unwrap_or_default(),
            highlight: newest.map_or(0..0, |hit| hit.highlight.clone()),
        }),
    }
}

/// The rows for a pane's kept matching lines, the lowest in the
/// scrollback (the newest) first. A flat row names its pane.
fn pane_hit_rows(pane: &PaneHits, now: Option<i64>, flat: bool) -> Vec<Row> {
    debug_assert!(pane.hits.len() <= MAX_PANE_HITS_PER_ENTRY);
    let origin = format!(
        "{} {}.{}",
        pane.pane.mux.session, pane.pane.window_index, pane.pane.pane_index
    );
    pane.hits
        .iter()
        .rev()
        .map(|hit| Row {
            id: RowId::PaneHit {
                mux: mux_node(pane),
                pane: pane.pane.pane_id.clone(),
                line: hit.line,
            },
            depth: 0,
            expandable: false,
            kind: RowKind::PaneHit(PaneHitRow {
                pane: pane.pane.clone(),
                origin: flat.then(|| origin.clone()),
                line: hit.line,
                age: pane_age(pane, now),
                age_epoch: Some(pane.captured_at),
                snippet: hit.snippet.clone(),
                highlight: hit.highlight.clone(),
            }),
        })
        .collect()
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
                    .and_then(|time| Age::of(Some(time.timestamp()), now))
                    .map_or_else(|| tuikit::glyphs::marks::NO_VALUE.to_string(), Age::label),
                age_epoch: hit.timestamp.map(|time| time.timestamp()),
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
