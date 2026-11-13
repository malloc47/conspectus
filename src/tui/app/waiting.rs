//! The key that jumps to the agent waiting on the operator longest
//! (ADR 0216).
//!
//! The agents are the ones the rows' live values read as waiting, the set
//! the header's `! N` counts, the longest wait first. Each is somewhere:
//! at a row of the current view that stands for it (its own row, a pin on
//! it, a row listed under a pin; in the Mux view, its mux's row when it
//! has none of its own; for an agent in a pane no session matches, its
//! mux's row), or else in the view a reveal of it lands in, when that
//! view lists it. The key goes to the next one after the agent the cursor
//! is on, wrapping, and says in the status line where it went or why it
//! stayed. Moving the cursor and switching views are view state, so no
//! effect is emitted beyond the view's persisting.

use std::collections::BTreeMap;

use tuikit::age::Age;

use super::listing::Listed;
use super::tree::row_primary_node;
use super::*;
use crate::tui::Effect;
use crate::tui::keybindings::key_name;
use crate::tui::rows::seen::Agent;
use crate::tui::widgets::controls::view_label;

/// Where a waiting agent is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    /// The row at this index of the current view's tree.
    Here(usize),
    /// The row of `node` in `view`, which a reveal of it lands in.
    There(View, NodeId),
}

impl App {
    /// Whether some agent waits on the operator, so the jump key has
    /// somewhere to go and the status bar names it.
    pub fn has_waiting(&self) -> bool {
        self.tree.waiting_anywhere > 0
    }

    /// Move to the agent waiting on the operator that comes after the one
    /// the cursor is on (the longest-waiting one when it is on none),
    /// wrapping, and say where it went; with nowhere to go, stay and say
    /// why.
    pub(super) fn jump_to_next_waiting(&mut self) -> Vec<Effect> {
        let key = key_name("waiting.next");
        let waiting = self.tree.live.waiting_agents();
        let Some(handle) = self.handle.as_ref().filter(|_| !waiting.is_empty()) else {
            self.status_message = Some(Feedback::warning(format!(
                "{key}: no agent is waiting on you"
            )));
            return Vec::new();
        };
        let snapshot = handle.snapshot().clone();
        let mut listed: BTreeMap<View, Listed> = BTreeMap::new();
        let places: Vec<Option<Place>> = waiting
            .iter()
            .map(|(agent, _)| self.place_of(&snapshot, agent, &mut listed))
            .collect();
        let cursor = self.cursor_row();
        let current = cursor.and_then(|at| {
            places
                .iter()
                .position(|place| *place == Some(Place::Here(at)))
        });
        let total = waiting.len();
        let start = current.map_or(0, |index| index + 1);
        let elsewhere = |place: &Option<Place>| match place {
            Some(Place::Here(at)) => Some(*at) != cursor,
            Some(Place::There(..)) => true,
            None => false,
        };
        let Some(next) = (0..total)
            .map(|step| (start + step) % total)
            .find(|index| elsewhere(&places[*index]))
        else {
            let hidden = places.iter().filter(|place| place.is_none()).count();
            self.status_message = Some(Feedback::warning(match (current, hidden) {
                (Some(_), 0) => format!("{key}: no other agent is waiting on you"),
                (Some(_), hidden) => {
                    format!("{key}: no other waiting agent is listed; the filters hide {hidden}")
                }
                (None, 1) => format!("{key}: 1 waiting, but the filters hide it"),
                (None, hidden) => format!("{key}: {hidden} waiting, but the filters hide them"),
            }));
            return Vec::new();
        };
        let since = Age::between(
            waiting[next].1.div_euclid(1000),
            crate::tui::rows::tree_current_unix_epoch(),
        )
        .ago();
        let reached = format!("{} of {total} waiting, since {since}", next + 1);
        match places[next].clone() {
            Some(Place::Here(at)) => {
                self.tree_state.select_at(&self.tree.rows, at);
                self.explorer_reflects = None;
                self.recompute_explorer();
                self.status_message = Some(Feedback::info(reached));
                Vec::new()
            }
            Some(Place::There(view, node)) => {
                let from = self.active_view;
                match self.reveal_node(&node, None) {
                    Ok(()) => {
                        self.status_message = Some(Feedback::info(format!(
                            "{reached} · switched to {}: {} doesn't list it",
                            view_label(view),
                            view_label(from)
                        )));
                        vec![Effect::Persist]
                    }
                    Err(why) => {
                        self.status_message = Some(Feedback::warning(format!("{key}: it {why}")));
                        Vec::new()
                    }
                }
            }
            None => Vec::new(),
        }
    }

    /// Where `agent` is: a row of this view that stands for it, or, in
    /// the Mux view, its mux's row; else the view a reveal of it lands in
    /// when that view lists it (what each view lists is built once per
    /// key, in `listed`); else nowhere.
    fn place_of(
        &self,
        snapshot: &GraphSnapshot,
        agent: &Agent,
        listed: &mut BTreeMap<View, Listed>,
    ) -> Option<Place> {
        let node = match agent {
            Agent::Session(node) => node.clone(),
            Agent::Pane(pane) => NodeId::MuxSession(pane.mux.clone()),
        };
        if let Some(at) = self.row_standing_for(snapshot, &node) {
            return Some(Place::Here(at));
        }
        if self.active_view == View::Mux
            && let Some(mux) = agent.mux(snapshot)
            && let Some(at) = self.row_standing_for(snapshot, &NodeId::MuxSession(mux))
        {
            return Some(Place::Here(at));
        }
        let view = crate::tui::actions::natural_view(snapshot, &node)?;
        if view == self.active_view {
            return None;
        }
        listed
            .entry(view)
            .or_insert_with(|| self.listed_by(view))
            .contains(&node, None)
            .then_some(Place::There(view, node))
    }

    /// The index of the first row of the tree that stands for `node`: its
    /// own row, a row listed under a pin, or a pin on it.
    fn row_standing_for(&self, snapshot: &GraphSnapshot, node: &NodeId) -> Option<usize> {
        self.tree.rows.iter().position(|row| match &row.id {
            RowId::Pin { pin_id } => {
                let pin = NodeId::Pin(PinId::new(pin_id.clone()));
                crate::tui::rows::live::pin_target(snapshot, &pin) == Some(node)
            }
            _ => row_primary_node(row) == Some(node),
        })
    }

    /// The index in the tree of the row the cursor is on.
    fn cursor_row(&self) -> Option<usize> {
        let rows = &self.tree.rows;
        let position = self.tree_state.cursor(rows)?;
        self.tree_state.visible_indices(rows).get(position).copied()
    }
}
