//! The keys that jump to the agent waiting on the operator longest
//! (ADR 0216), and to the next and the previous agent with an unread stop
//! (ADR 0247).
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
//!
//! The unread walk is the same walk over the agents with an unread stop,
//! the oldest stop first, in either direction.

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

/// Which agents a jump walks, and which way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Walk {
    /// The agents waiting on the operator, the longest wait first, on.
    Waiting,
    /// The agents with an unread stop, the oldest stop first, on.
    NextUnread,
    /// The same agents, back.
    PreviousUnread,
}

impl Walk {
    /// The id of the binding that starts the walk.
    fn binding(self) -> &'static str {
        match self {
            Self::Waiting => "waiting.next",
            Self::NextUnread => "unread.next",
            Self::PreviousUnread => "unread.previous",
        }
    }

    /// What the walked agents are doing, as the status line says it.
    fn word(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::NextUnread | Self::PreviousUnread => "unread",
        }
    }

    /// The walk goes on from the cursor rather than back.
    fn forward(self) -> bool {
        self != Self::PreviousUnread
    }

    /// The status line when no agent is in the walk.
    fn none(self) -> &'static str {
        match self {
            Self::Waiting => "no agent is waiting on you",
            Self::NextUnread | Self::PreviousUnread => "no agent is unread",
        }
    }

    /// The status line when the cursor's agent is the only one.
    fn no_other(self) -> &'static str {
        match self {
            Self::Waiting => "no other agent is waiting on you",
            Self::NextUnread | Self::PreviousUnread => "no other agent is unread",
        }
    }
}

impl App {
    /// Whether some agent waits on the operator, so the jump key has
    /// somewhere to go and the status bar names it.
    pub fn has_waiting(&self) -> bool {
        self.tree.waiting_anywhere > 0
    }

    /// Whether some agent has a stop the operator has not read, so the
    /// unread keys have somewhere to go and the status bar names them.
    pub fn has_unread(&self) -> bool {
        self.tree.live.unread().next().is_some()
    }

    /// Move to the agent of `walk` that comes after (or, for the walk
    /// back, before) the one the cursor is on, the first (or last) one
    /// when it is on none, wrapping, and say where it went; with nowhere
    /// to go, stay and say why.
    pub(super) fn jump_along(&mut self, walk: Walk) -> Vec<Effect> {
        let key = key_name(walk.binding());
        let word = walk.word();
        let waiting = match walk {
            Walk::Waiting => self.tree.live.waiting_agents(),
            Walk::NextUnread | Walk::PreviousUnread => self.tree.live.unread_agents(),
        };
        let Some(handle) = self.handle.as_ref().filter(|_| !waiting.is_empty()) else {
            self.status_message = Some(Feedback::warning(format!("{key}: {}", walk.none())));
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
        // The index `step` places on from the cursor's agent (from the
        // start of the walk when the cursor is on none).
        let at_step = |step: usize| {
            if walk.forward() {
                (current.map_or(0, |index| index + 1) + step) % total
            } else {
                (current.unwrap_or(0) + 2 * total - 1 - step) % total
            }
        };
        let here =
            |place: &Option<Place>| matches!(place, Some(Place::Here(at)) if Some(*at) != cursor);
        let elsewhere =
            |place: &Option<Place>| here(place) || matches!(place, Some(Place::There(..)));
        // The unread walk stays in the view while it can: in Mux, an agent
        // with no mux is not an unread mux, and a press there does not
        // switch views unless nothing else in this one is unread.
        let first = |reachable: &dyn Fn(&Option<Place>) -> bool| {
            (0..total)
                .map(at_step)
                .find(|index| reachable(&places[*index]))
        };
        let next = match walk {
            Walk::Waiting => first(&elsewhere),
            Walk::NextUnread | Walk::PreviousUnread => first(&here).or_else(|| first(&elsewhere)),
        };
        let Some(next) = next else {
            let hidden = places.iter().filter(|place| place.is_none()).count();
            self.status_message = Some(Feedback::warning(match (current, hidden) {
                (Some(_), 0) => format!("{key}: {}", walk.no_other()),
                (Some(_), hidden) => {
                    format!("{key}: no other {word} agent is listed; the filters hide {hidden}")
                }
                (None, 1) => format!("{key}: 1 {word}, but the filters hide it"),
                (None, hidden) => format!("{key}: {hidden} {word}, but the filters hide them"),
            }));
            return Vec::new();
        };
        let since = Age::between(
            waiting[next].1.div_euclid(1000),
            crate::tui::rows::tree_current_unix_epoch(),
        )
        .ago();
        let reached = format!("{} of {total} {word}, since {since}", next + 1);
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
