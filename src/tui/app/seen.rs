//! Which finished turns the operator has seen (ADR 0214): the App's
//! memory of them, and the rest of the cursor that counts as seeing a
//! session with no mux.
//!
//! The rows judge each ready turn when their live values are filled
//! (`rows::seen`); the App keeps every turn they find seen, so a later
//! snapshot that no longer shows the evidence (a client attached to the
//! mux) can't make it unseen again, and remembers a session with no mux
//! once the cursor has rested on one of its rows for
//! [`SEEN_REST`](crate::tui::rows::seen::SEEN_REST). This is view state:
//! nothing is written and no effect is emitted.

use super::*;
use crate::tui::rows::seen::{self, Agent, SEEN_REST, SeenContext};

impl App {
    /// What judges whether a ready turn was seen: the memory, and when
    /// the snapshot on screen loaded.
    pub(crate) fn seen_context(&self) -> SeenContext<'_> {
        SeenContext {
            memory: &self.seen,
            loaded_ms: self
                .loaded_at_epoch
                .map_or(i64::MIN, |epoch| epoch.saturating_mul(1000)),
        }
    }

    /// Keep every ready turn the rows on screen found seen.
    pub(super) fn remember_seen_turns(&mut self) {
        let turns: Vec<(Agent, i64)> = self.tree.live.seen_turns().collect();
        for (agent, ended_ms) in turns {
            self.seen.see(agent, ended_ms);
        }
    }

    /// A snapshot was put on screen: judge its ready turns again with its
    /// own load time (its rows were built before it was set), and keep
    /// the ones it shows seen.
    pub(super) fn settle_seen(&mut self) {
        if self.tree.live.any_ready() {
            self.refill_live(crate::tui::rows::tree_current_unix_ms());
        }
    }

    /// One tick at `now` of the cursor's rest: once it has rested on a
    /// row standing for a session with no mux for [`SEEN_REST`], that
    /// session's finished turn is seen, and the rows take it in place. A
    /// move to another session's row starts the rest again; a row that
    /// stands for no session ends it.
    pub(crate) fn note_cursor_rest(&mut self, now: Instant) {
        let Some(session) = self.resting_session() else {
            self.resting = None;
            return;
        };
        match &self.resting {
            Some((rested, since)) if *rested == session => {
                if now.saturating_duration_since(*since) >= SEEN_REST {
                    self.see_rested(session);
                }
            }
            _ => self.resting = Some((session, now)),
        }
    }

    /// The session the selected row stands for: a session row (nested or
    /// a content search's hit under it included), or a pin on a session.
    fn resting_session(&self) -> Option<NodeId> {
        let node = match self.tree_state.selected()? {
            RowId::AgentSession(node) | RowId::Nested { node, .. } => node.clone(),
            RowId::Hit { session, .. } => session.clone(),
            RowId::Pin { pin_id } => {
                let snapshot = self.handle.as_ref()?.snapshot();
                let pin = NodeId::Pin(PinId::new(pin_id.clone()));
                crate::tui::rows::live::pin_target(snapshot, &pin)?.clone()
            }
            _ => return None,
        };
        matches!(node, NodeId::AgentSession(_)).then_some(node)
    }

    /// The cursor rested on `session`'s row: remember its finished turn
    /// as seen if it is unseen and the session has no mux (a turn in a
    /// mux is seen by attaching to it), and fill the rows again.
    fn see_rested(&mut self, session: NodeId) {
        let Some(cell) = self.tree.live.session(&session).filter(|cell| cell.unseen) else {
            return;
        };
        let Some(handle) = &self.handle else {
            return;
        };
        let agent = Agent::Session(session);
        if seen::has_mux(handle.snapshot(), &agent) {
            return;
        }
        if self.seen.see(agent, cell.since_ms) {
            self.refill_live(crate::tui::rows::tree_current_unix_ms());
        }
    }
}

#[cfg(test)]
#[path = "seen_tests.rs"]
mod tests;
