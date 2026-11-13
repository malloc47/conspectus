//! Which view lists an entity, as a reveal would show that view: what
//! decides whether quick select promises a view and whether a pin's menu
//! offers to reveal its entity.
//!
//! A reveal switches to the entity's view ([`natural_view`]) and selects
//! its row; the view's tree is built from that view's own grouping and
//! filter, so a repo with no sessions, or a row the view's filter hides,
//! is not there even though the view lists its kind. The functions here
//! build that tree without switching, so a gesture is offered or promised
//! only where it lands.
//!
//! [`natural_view`]: crate::tui::actions::natural_view

use std::collections::{BTreeMap, BTreeSet};

use super::tree::row_primary_node;
use super::*;

/// The views a reveal can land in.
const REVEAL_VIEWS: [View; 5] = [
    View::Sessions,
    View::Mux,
    View::Prs,
    View::Pinned,
    View::History,
];

/// What one view's tree lists: the entities its rows stand for, and the
/// History records its rows are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Listed {
    nodes: BTreeSet<NodeId>,
    records: BTreeSet<String>,
}

impl Listed {
    fn of(tree: &RowTree) -> Self {
        let mut listed = Self::default();
        for row in &tree.rows {
            if let Some(node) = row_primary_node(row) {
                listed.nodes.insert(node.clone());
            }
            if let RowId::History { record_id } = &row.id {
                listed.records.insert(record_id.clone());
            }
        }
        listed
    }

    /// Whether a row stands for `node`, or is its History `record`.
    pub(crate) fn contains(&self, node: &NodeId, record: Option<&str>) -> bool {
        self.nodes.contains(node) || record.is_some_and(|record| self.records.contains(record))
    }
}

/// What every view a reveal can land in lists, built once for the many
/// results of a quick-select reach.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing(BTreeMap<View, Listed>);

impl Listing {
    /// Whether `view` lists `node` (or its History `record`).
    pub(crate) fn lists(&self, view: View, node: &NodeId, record: Option<&str>) -> bool {
        self.0
            .get(&view)
            .is_some_and(|listed| listed.contains(node, record))
    }
}

impl App {
    /// The History record a reveal of `node` looks for: `hint` when the
    /// caller knows it (a pin's target), else the record the node's key
    /// names, else, for a mux, the record of a mux with its name.
    pub(crate) fn reveal_record(&self, node: &NodeId, hint: Option<String>) -> Option<String> {
        let snapshot = self.snapshot_handle()?.snapshot();
        hint.or_else(|| {
            snapshot
                .history
                .by_key(&node.to_string())
                .map(|(_, meta)| meta.id.clone())
        })
        .or_else(|| match node {
            NodeId::MuxSession(mux) => snapshot
                .history
                .muxes
                .iter()
                .find(|record| record.recipe.native_id() == mux.native_id)
                .map(|record| record.meta.id.clone()),
            _ => None,
        })
    }

    /// `view`'s tree as switching to it would build it: its own grouping
    /// and filter (the current ones when it is the active view), the
    /// History view showing pinned records, and the sort a flat Sessions
    /// view switches to. `None` before any data is loaded.
    fn revealed_tree(&self, view: View) -> Option<RowTree> {
        let snapshot = self.snapshot_handle()?.snapshot();
        let switches = view != self.active_view;
        let (grouping, mut filter) = if switches {
            self.view_states.get(&view).map_or_else(
                || {
                    let slot = ViewStateSlot::defaults_for(view);
                    (slot.grouping, slot.filter)
                },
                |slot| (slot.grouping, slot.filter.clone()),
            )
        } else {
            (self.grouping, self.filter.clone())
        };
        if view == View::History {
            filter.show_pinned_history = true;
        }
        filter.content = self.content_filter();
        let flat_sessions = matches!(
            grouping,
            crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::None)
        );
        let sort = if switches && flat_sessions {
            crate::tui::Sort::Recency
        } else {
            self.sort
        };
        Some(crate::tui::rows::build_tree_for_view(
            crate::tui::rows::TreeInputs {
                snapshot,
                view,
                grouping,
                filter,
                sort,
                mux_recency: self.mux_recency,
                cwd: self.config.cwd.as_deref(),
            },
        ))
    }

    /// What `view` lists as a reveal would show it.
    pub(crate) fn listed_by(&self, view: View) -> Listed {
        self.revealed_tree(view)
            .map(|tree| Listed::of(&tree))
            .unwrap_or_default()
    }

    /// What every view a reveal can land in lists.
    pub(crate) fn listing(&self) -> Listing {
        Listing(
            REVEAL_VIEWS
                .into_iter()
                .map(|view| (view, self.listed_by(view)))
                .collect(),
        )
    }

    /// The view a reveal of `node` lands in, if any: its kind's view, when
    /// that view lists it (by node, or by `record_hint` or its own
    /// History record).
    pub(crate) fn reveal_view(&self, node: &NodeId, record_hint: Option<String>) -> Option<View> {
        let snapshot = self.snapshot_handle()?.snapshot();
        let view = crate::tui::actions::natural_view(snapshot, node)?;
        let record = self.reveal_record(node, record_hint);
        self.listed_by(view)
            .contains(node, record.as_deref())
            .then_some(view)
    }

    /// The entity pin `slot` points at, and the History record its target
    /// names: what revealing the pin reveals.
    pub(crate) fn slot_target(&self, slot: &str) -> Result<(NodeId, Option<String>), String> {
        let pin = self
            .snapshot_handle()
            .and_then(|handle| {
                crate::tui::rows::pin_nodes(handle.snapshot())
                    .into_iter()
                    .find(|pin| pin.id.id == slot)
            })
            .ok_or_else(|| format!("pin `{slot}` has no entity to reveal"))?;
        let node = pin
            .target_node
            .clone()
            .ok_or_else(|| format!("pin `{slot}` has no entity to reveal"))?;
        let record = pin
            .target
            .as_deref()
            .and_then(|target| target.strip_prefix(crate::model::PinSlot::HISTORY_PREFIX))
            .map(str::to_string);
        Ok((node, record))
    }

    /// The view revealing pin `slot` lands in, if any.
    pub(crate) fn slot_reveal_view(&self, slot: &str) -> Option<View> {
        let (node, record) = self.slot_target(slot).ok()?;
        self.reveal_view(&node, record)
    }
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
