//! Right-pane explorer: what the left selection opens it on, what its
//! keys' outcomes do to the rest of the app, and the words it says.
//!
//! The framework explorer (`tuikit::explorer`) keeps the cursor, the
//! `Other` fold, Expanded Node Detail, the drill frames, and the armed
//! `Backspace`, and answers every key with an outcome. This module lends
//! it out with a source over the app's data, carries out each outcome
//! (the copy, the value modal, the left pane's mirror and restore, the
//! focus shift), and writes every status hint.

use tuikit::explorer::{ExplorerKey, Outcome, Refreshed, Refusal};

use super::tree::row_matches;
use super::*;
use crate::tui::explorer::Graph;

impl App {
    /// Lend the explorer and a source over the rest of the app to `act`,
    /// then put the explorer back. `None` with no explorer or no
    /// snapshot.
    pub(super) fn with_explorer<T>(
        &mut self,
        act: impl FnOnce(&mut GraphExplorer, &GraphSource<'_>) -> T,
    ) -> Option<T> {
        let mut explorer = self.explorer.take()?;
        let result = self
            .graph_source()
            .map(|source| act(&mut explorer, &source));
        self.explorer = Some(explorer);
        result
    }

    /// Point the explorer at the node the left-pane selection names, or
    /// clear it when the selection names no node. While the selection
    /// is still the row the pane reflects (a refresh, a filter or sort
    /// change, `r`), a drilled pane reloads the node it shows instead,
    /// and opens on the selection's node only when that one is gone.
    pub(super) fn recompute_explorer(&mut self) {
        self.reset_preview_scroll();
        let selection = self.tree_state.selected().cloned();
        let reflected = std::mem::replace(&mut self.explorer_reflects, selection.clone());
        let Some(selection) = selection else {
            self.explorer = None;
            return;
        };
        if reflected.as_ref() == Some(&selection) && self.reload_drill() {
            return;
        }
        let Some(handle) = self.handle.as_ref() else {
            self.explorer = None;
            return;
        };
        let snapshot = handle.snapshot();
        let raw_target = match &selection {
            RowId::Group(node) => Some(node.clone()),
            RowId::AgentSession(node) => Some(node.clone()),
            RowId::MuxSession(node) => Some(node.clone()),
            RowId::Pr(node) => Some(node.clone()),
            RowId::Pin { pin_id } => Some(NodeId::Pin(PinId::new(pin_id.clone()))),
            RowId::Nested { node, .. } => Some(node.clone()),
            RowId::Hit { session, .. } => Some(session.clone()),
            RowId::Pane { mux, .. } | RowId::PaneHit { mux, .. } => Some(mux.clone()),
            RowId::History { record_id } => {
                snapshot.history.meta(record_id).and_then(|(kind, _)| {
                    crate::tui::rows::history::node_of(snapshot, kind, record_id)
                })
            }
            RowId::Remote(_) | RowId::Bucket(_) | RowId::Synthetic(_) => None,
        };
        let Some(raw_target) = raw_target else {
            self.explorer = None;
            return;
        };
        self.recompute_explorer_for(raw_target);
    }

    /// Rebuild the node a drilled pane shows from the current snapshot,
    /// keeping its frames, fold, Expanded Node Detail toggle, and cursor.
    /// Older frames are rebuilt only when `Backspace` reaches them. Says
    /// whether it did: not when the pane has no drill, nor when its node
    /// is gone.
    fn reload_drill(&mut self) -> bool {
        if self
            .explorer
            .as_ref()
            .is_none_or(|explorer| explorer.frames().is_empty())
        {
            return false;
        }
        self.with_explorer(|explorer, source| explorer.refresh(source)) == Some(Refreshed::Kept)
    }

    /// Show `target`: reload it in place, keeping the cursor, the fold,
    /// the switch, and the frames, when the explorer already shows it;
    /// otherwise open the explorer on it afresh, in the window the pane
    /// last drew.
    fn recompute_explorer_for(&mut self, target: NodeId) {
        if self
            .explorer
            .as_ref()
            .is_some_and(|explorer| explorer.id() == &target)
        {
            if self.with_explorer(|explorer, source| explorer.refresh(source))
                != Some(Refreshed::Kept)
            {
                self.explorer = None;
            }
            return;
        }
        let window = self.explorer.as_ref().map(GraphExplorer::window);
        let opened = self
            .graph_source()
            .and_then(|source| GraphExplorer::open(&target, &source));
        self.explorer = opened;
        if let (Some(explorer), Some(window)) = (self.explorer.as_mut(), window) {
            explorer.keep_window(window);
        }
    }

    /// A key for the explorer: it does its part, and the outcome says
    /// what is left for the app. With no explorer, `o` says there is
    /// nothing to open and every other key does nothing.
    pub(super) fn explorer_key(&mut self, key: ExplorerKey) -> Vec<crate::tui::Effect> {
        if self.explorer.is_none() {
            if key == ExplorerKey::Inspect {
                self.status_message = Some(Feedback::warning(
                    "explorer: nothing to open here — `o` opens long values on the cursor row",
                ));
            }
            return Vec::new();
        }
        match self.with_explorer(|explorer, source| explorer.handle(key, source)) {
            Some(outcome) => self.explorer_outcome(outcome),
            None => Vec::new(),
        }
    }

    /// Carry out what the explorer's `outcome` asks of the app, and say
    /// what it means in the status line.
    pub(super) fn explorer_outcome(&mut self, outcome: Outcome<Graph>) -> Vec<crate::tui::Effect> {
        match outcome {
            Outcome::None | Outcome::Toggle { .. } => self.status_message = None,
            Outcome::Copy { label, text } => {
                // The executor writes the clipboard (OSC 52, ADR 0056) and
                // says what was copied, or why the write failed.
                return vec![crate::tui::Effect::Copy {
                    text,
                    done: format!("copied: {label}"),
                }];
            }
            Outcome::Inspect { label, text } => {
                self.open_value_modal(crate::tui::widgets::value_modal::ValueText::new(
                    label, text,
                ));
            }
            Outcome::Drill { to } => {
                self.reset_preview_scroll();
                self.status_message = None;
                // Mirror sync: when the drilled neighbor has a row in the
                // current view, the left tree moves to it and opens its
                // ancestors; when it doesn't, the left selection stays so
                // the operator's prior position is kept.
                self.mirror_left_pane_to(&to);
            }
            Outcome::Follow { to } => {
                self.status_message = Some(Feedback::warning(format!(
                    "explorer: drill target {to:?} not in current snapshot"
                )));
            }
            Outcome::Back { saved, .. } => {
                self.reset_preview_scroll();
                self.status_message = None;
                // The left-pane selection active at the time of the drill
                // comes back too, so Backspace unwinds both panes together.
                // It bypasses `set_selection`, which would open the pane
                // again on the row's own node.
                if let Some(selection) = saved {
                    self.tree_state.select(&self.tree.rows, selection);
                }
                self.explorer_reflects = self.tree_state.selected().cloned();
            }
            Outcome::Armed => {
                self.status_message = Some(Feedback::info(
                    "explorer: no drill history — press Backspace again to return to the left pane",
                ));
            }
            Outcome::Leave => {
                self.focus = Focus::Left;
                self.status_message = None;
            }
            Outcome::Refused(refusal) => {
                self.status_message = Some(Feedback::warning(self.refusal_text(refusal)));
            }
        }
        Vec::new()
    }

    /// What the status line says when the explorer refuses a key.
    fn refusal_text(&self, refusal: Refusal) -> String {
        match refusal {
            Refusal::NotInSection => {
                "explorer: nothing to expand here — select the `Other` header to toggle alternates"
                    .to_string()
            }
            Refusal::NothingMore => {
                let core = self
                    .explorer
                    .as_ref()
                    .map_or(0, |explorer| explorer.content().core_fields.len());
                format!("explorer: this node kind has no extra fields ({core} total)")
            }
            // An unresolved stub names no node, but the Preview shows its
            // evidence while the cursor is on it, which is where Enter
            // was just pressed.
            Refusal::NoFarEnd if self.cursor_on_unresolved() => {
                "explorer: evidence names no node — the Preview shows it".to_string()
            }
            Refusal::NoFarEnd => "explorer: this link leads to no node".to_string(),
            Refusal::NothingToInspect => "explorer: row has no truncated value to open".to_string(),
            Refusal::Gone => {
                "explorer: cannot restore breadcrumb hop — node missing from snapshot".to_string()
            }
        }
    }

    /// Whether the explorer's cursor is on an unresolved-evidence row.
    fn cursor_on_unresolved(&self) -> bool {
        self.explorer.as_ref().is_some_and(|explorer| {
            matches!(
                explorer.cursor_key(),
                Some(crate::tui::explorer::ExplorerRowKey::OtherUnresolved { .. })
            )
        })
    }

    /// `Backspace` on the left pane with no reveal to return from: say
    /// so. The right pane's drill is the right pane's `Backspace`'s, so
    /// the hint points there when the pane has drilled.
    pub(super) fn left_back(&mut self) {
        let drilled = self
            .explorer
            .as_ref()
            .is_some_and(|explorer| !explorer.frames().is_empty());
        self.status_message = Some(Feedback::warning(if drilled {
            "Backspace: no reveal to return from · Tab to the right pane to back out of its drill"
        } else {
            "Backspace: no reveal to return from"
        }));
    }

    pub(super) fn toggle_edge_meta(&mut self) {
        self.edge_meta_visible = !self.edge_meta_visible;
        self.status_message = Some(Feedback::success(if self.edge_meta_visible {
            "explorer: edge meta visible (provenance · confidence · state)"
        } else {
            "explorer: edge meta hidden"
        }));
    }

    /// Move the left-pane selection to the row corresponding to
    /// `target`, expanding any ancestor group rows along the path.
    /// No-op when the focused node isn't represented in the current
    /// view's row tree.
    pub(super) fn mirror_left_pane_to(&mut self, target: &NodeId) {
        let Some(row_index) = self
            .tree
            .rows
            .iter()
            .position(|row| row_matches(row, target))
        else {
            return;
        };
        // Expand the ancestor group rows so the target row shows.
        self.tree_state.reveal(&self.tree.rows, row_index);
        // Bypass `set_selection` so we don't trigger the
        // recompute_explorer path — the explorer was just rebuilt for
        // the drilled neighbor and that's what we want to keep.
        let target_id = self.tree.rows[row_index].id.clone();
        self.tree_state.select(&self.tree.rows, target_id);
        self.explorer_reflects = self.tree_state.selected().cloned();
    }
}
