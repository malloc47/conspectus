//! Which finished turns the operator has seen (ADR 0214): the App's
//! memory of them, and the rest of the cursor that counts as seeing a
//! session with no mux.
//!
//! The rows judge each ready turn when their live values are filled
//! (`rows::seen`); the App keeps every turn they find seen, and every
//! return of the operator's tmux client to the TUI's session (ADR 0231),
//! so a later
//! snapshot that no longer shows the evidence (a client attached to the
//! mux) can't make it unseen again, and remembers a session with no mux
//! once the cursor has rested on one of its rows for
//! [`SEEN_REST`](crate::tui::rows::seen::SEEN_REST). This is view state:
//! nothing is written and no effect is emitted.

use super::*;
use crate::tui::Effect;
use crate::tui::rows::seen::{self, Agent, ClientReturn, SEEN_REST, SeenContext};
use tuikit::feedback::Feedback;

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

    /// Write the state file when a session's read mark moved since it
    /// was last written, from outside the reducer (the tick's own).
    pub(crate) fn persist_read_marks(&mut self) {
        if self.seen.take_changed() {
            self.persist_state();
        }
    }

    /// The effects the reducer owes a session's moved read mark: the
    /// state file's write, once.
    pub(super) fn read_mark_effects(&mut self) -> Vec<Effect> {
        if self.seen.take_changed() {
            vec![Effect::Persist]
        } else {
            Vec::new()
        }
    }

    /// The agents with an unread stop that `node` stands for, each with
    /// when that stop began: a session's own, or a mux's, the agents
    /// counted on it; every agent when `node` is `None`.
    pub(crate) fn unread_of(&self, node: Option<&NodeId>) -> Vec<(Agent, i64)> {
        let Some(handle) = &self.handle else {
            return Vec::new();
        };
        let snapshot = handle.snapshot();
        let stands_for = |agent: &Agent| match node {
            None => true,
            Some(NodeId::MuxSession(mux)) => agent.mux(snapshot).as_ref() == Some(mux),
            Some(session) => *agent == Agent::Session(session.clone()),
        };
        self.tree
            .live
            .unread()
            .filter(|(agent, _)| stands_for(agent))
            .collect()
    }

    /// Read the unread stops of `node`'s agents ([`Self::unread_of`]).
    /// Returns how many it read, and fills the rows again so their marks
    /// go plain in place.
    fn read_unread(&mut self, node: Option<&NodeId>) -> usize {
        let unread = self.unread_of(node);
        let read = unread.len();
        for (agent, since_ms) in unread {
            self.seen.see(agent, since_ms);
        }
        if read > 0 {
            self.refill_live(crate::tui::rows::tree_current_unix_ms());
        }
        read
    }

    /// `c`: mark the agents of the entity in view read, with a toast that
    /// says how many (ADR 0245).
    pub(super) fn mark_read(&mut self) -> Vec<Effect> {
        let node = crate::tui::actions::action_subject(self).node;
        let known = node
            .as_ref()
            .is_some_and(|node| matches!(node, NodeId::AgentSession(_) | NodeId::MuxSession(_)));
        let mut effects = Vec::new();
        let toast = if known {
            match self.read_unread(node.as_ref()) {
                0 => Feedback::info("nothing unread here"),
                1 => Feedback::success("marked 1 agent read"),
                many => Feedback::success(format!("marked {many} agents read")),
            }
        } else {
            Feedback::warning("nothing to mark read: this row has no agent")
        };
        effects.extend(self.read_mark_effects());
        effects.push(Effect::Toast(toast));
        effects
    }

    /// `C`: mark every agent's stop read.
    pub(super) fn clear_unread(&mut self) -> Vec<Effect> {
        let toast = match self.read_unread(None) {
            0 => Feedback::info("nothing unread"),
            1 => Feedback::success("cleared 1 unread agent"),
            many => Feedback::success(format!("cleared {many} unread agents")),
        };
        let mut effects = self.read_mark_effects();
        effects.push(Effect::Toast(toast));
        effects
    }

    /// Keep every stop the rows on screen found read.
    pub(super) fn remember_seen_turns(&mut self) {
        let turns: Vec<(Agent, i64)> = self.tree.live.seen_turns().collect();
        for (agent, ended_ms) in turns {
            self.seen.see(agent, ended_ms);
        }
    }

    /// The operator's client came back to the TUI's session from another
    /// one: remember it, and judge the ready turns on screen again, which
    /// takes the ones that ended on that session while the client was
    /// there as seen, in place.
    pub(super) fn see_return(&mut self, back: ClientReturn) {
        if self.seen.note_return(back) {
            self.settle_seen();
        }
    }

    /// A snapshot was put on screen: judge its ready turns again with its
    /// own load time (its rows were built before it was set), and keep
    /// the ones it shows seen.
    pub(super) fn settle_seen(&mut self) {
        if self.tree.live.any_stopped() {
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
