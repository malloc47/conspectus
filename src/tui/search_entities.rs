//! The wider scope of `/` quick select: every entity in the snapshot with
//! a name the operator would recognize, whichever view it lives in.
//!
//! The index is pure data built from a [`GraphSnapshot`]; ranking reuses the
//! case-insensitive substring matcher the rest of search shares. A match in
//! an entity's label outranks one in its other fields, and ties fall back to
//! kind order and then recency, so the order is stable between keystrokes.

use std::cmp::Reverse;
use std::ops::Range;

use crate::aliases::session_display_name;
use crate::model::{GraphNode, GraphSnapshot, NodeId, NodeKind, NodePresence};
use crate::tui::search::find_ci;

/// One searchable entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntity {
    pub node: NodeId,
    pub kind: NodeKind,
    /// The name shown first and matched first.
    pub label: String,
    /// Dim secondary text: where it lives, or what it is about.
    pub context: String,
    /// Further text a query can find the entity by.
    pub fields: Vec<String>,
    /// Newest activity, for ordering ties; zero when unknown.
    pub recency: i64,
    /// Remembered by history but not present now.
    pub gone: bool,
}

impl SearchEntity {
    /// Where the kind sorts among equally good matches: sessions and muxes
    /// first, remembered entities after the live ones.
    fn kind_rank(&self) -> u8 {
        if self.gone {
            return 6;
        }
        match self.kind {
            NodeKind::AgentSession => 0,
            NodeKind::MuxSession => 1,
            NodeKind::Pin => 2,
            NodeKind::Checkout => 3,
            NodeKind::Repo => 4,
            NodeKind::ForgePr => 5,
            _ => 7,
        }
    }
}

/// What orders matches: label before other fields, earlier before later,
/// then kind, then newest.
type RankKey = (u8, usize, u8, Reverse<i64>);

/// An entity that matched a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMatch {
    /// Position in [`EntityIndex::entities`].
    pub index: usize,
    /// Where the query sits in the label, when the label matched.
    pub label_range: Option<Range<usize>>,
}

/// Every searchable entity in a snapshot.
#[derive(Debug, Clone, Default)]
pub struct EntityIndex {
    entities: Vec<SearchEntity>,
}

impl EntityIndex {
    /// Index the snapshot. A session's message preview is searchable only
    /// when `live_preview` is on, because previews are payload the operator
    /// has turned off.
    pub fn build(snapshot: &GraphSnapshot, live_preview: bool) -> Self {
        let entities = snapshot
            .nodes
            .iter()
            .filter_map(|node| entity(snapshot, node, live_preview))
            .collect();
        Self { entities }
    }

    pub fn entities(&self) -> &[SearchEntity] {
        &self.entities
    }

    pub fn get(&self, index: usize) -> Option<&SearchEntity> {
        self.entities.get(index)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Entities matching `query`, best first. An empty query matches nothing.
    pub fn rank(&self, query: &str) -> Vec<EntityMatch> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(RankKey, EntityMatch)> = self
            .entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                let (tier, position, label_range) =
                    if let Some(start) = find_ci(&entity.label, query) {
                        (0, start, Some(start..start + query.len()))
                    } else {
                        let start = entity
                            .fields
                            .iter()
                            .chain(std::iter::once(&entity.context))
                            .filter_map(|text| find_ci(text, query))
                            .min()?;
                        (1, start, None)
                    };
                let key = (tier, position, entity.kind_rank(), Reverse(entity.recency));
                Some((key, EntityMatch { index, label_range }))
            })
            .collect();
        // Stable sort: equal keys keep the snapshot's canonical node order.
        hits.sort_by_key(|hit| hit.0);
        hits.into_iter().map(|(_, hit)| hit).collect()
    }
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
}

fn short_ref(refname: &str) -> &str {
    refname.strip_prefix("refs/heads/").unwrap_or(refname)
}

fn entity(snapshot: &GraphSnapshot, node: &GraphNode, live_preview: bool) -> Option<SearchEntity> {
    let id = node.id();
    let presence = snapshot.presence(&id);
    let gone = presence.is_some();
    let history_seen = presence.map_or(0, NodePresence::last_seen);
    let (label, context, fields, recency): (String, String, Vec<String>, i64) = match node {
        GraphNode::AgentSession(session) => {
            let alias = snapshot.aliases.get(&id);
            let label = session_display_name(alias, session.title.as_deref(), None).map_or_else(
                || format!("{}:{}", session.harness_key, session.id.session_key),
                str::to_string,
            );
            let mut fields = vec![session.id.session_key.clone(), session.harness_key.clone()];
            fields.extend(alias.map(str::to_string));
            fields.extend(session.title.clone());
            if live_preview {
                fields.extend(session.last_message_preview.clone());
            }
            (
                label,
                session.cwd.clone().unwrap_or_default(),
                fields,
                session.last_active_epoch.unwrap_or(0),
            )
        }
        GraphNode::MuxSession(mux) => {
            let mut fields = vec![mux.native_id.clone()];
            fields.extend(mux.cwd.clone());
            fields.extend(mux.active_pane_command.clone());
            fields.extend(mux.active_pane_start_command.clone());
            (
                mux.session_name().to_string(),
                mux.cwd.clone().unwrap_or_default(),
                fields,
                mux.activity_epoch.or(mux.created_epoch).unwrap_or(0),
            )
        }
        GraphNode::Repo(repo) => {
            let root = repo.common_dir.trim_end_matches('/');
            let root = root.strip_suffix("/.git").unwrap_or(root);
            let mut fields = vec![repo.common_dir.clone()];
            fields.extend(repo.source_paths.iter().cloned());
            fields.extend(repo.remotes.iter().cloned());
            (basename(root).to_string(), root.to_string(), fields, 0)
        }
        GraphNode::Checkout(checkout) => {
            let mut fields = vec![checkout.root.clone()];
            let branch = checkout
                .current_branch
                .as_ref()
                .map(|branch| short_ref(&branch.refname).to_string());
            fields.extend(branch.clone());
            fields.push(checkout.id.repo.common_dir.clone());
            let context = match branch {
                Some(branch) => format!("{branch} · {}", checkout.root),
                None => checkout.root.clone(),
            };
            (basename(&checkout.root).to_string(), context, fields, 0)
        }
        GraphNode::ForgePr(pr) => {
            let mut fields = vec![format!("#{}", pr.number), pr.number.to_string()];
            fields.extend(pr.title.clone());
            fields.extend(pr.head_ref.clone());
            (
                format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
                pr.title.clone().unwrap_or_default(),
                fields,
                pr.updated_epoch.unwrap_or(0),
            )
        }
        GraphNode::Pin(pin) => {
            let mut fields = vec![pin.id.id.clone(), pin.label.clone()];
            fields.extend(pin.target.clone());
            // What the pin points at, by name, when that is still known.
            let context = pin
                .target_node
                .as_ref()
                .and_then(|target| snapshot.find_node(target))
                .map(|target| crate::tui::explorer::title_line(snapshot, target))
                .unwrap_or_default();
            (pin.label.clone(), context, fields, 0)
        }
        GraphNode::Workspace(workspace) => {
            let mut fields = vec![workspace.root.clone()];
            fields.extend(workspace.provider.clone());
            fields.extend(workspace.name.clone());
            (
                workspace
                    .name
                    .clone()
                    .unwrap_or_else(|| basename(&workspace.root).to_string()),
                workspace.root.clone(),
                fields,
                0,
            )
        }
        GraphNode::Fork(fork) => {
            let name = fork
                .name
                .clone()
                .unwrap_or_else(|| fork.provider_source_key.clone());
            let mut fields = vec![fork.provider.clone(), name.clone()];
            fields.extend(fork.scope.clone());
            (name, fork.provider.clone(), fields, 0)
        }
        GraphNode::Branch(_) | GraphNode::RuntimeProcess(_) => return None,
    };
    if label.trim().is_empty() {
        return None;
    }
    Some(SearchEntity {
        kind: NodeKind::from(&id),
        node: id,
        label,
        context,
        fields,
        recency: recency.max(history_seen),
        gone,
    })
}

#[cfg(test)]
#[path = "search_entities_tests.rs"]
mod tests;
