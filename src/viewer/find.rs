//! Find inside a transcript: the prompt, the matches, stepping between
//! them, and opening the viewer at a match from a search result.
//!
//! Matching uses the shared content matcher ([`crate::viewer::search`])
//! on each turn's raw body, so `n` and `N` agree with content search
//! about what a match is. What is searched is what the viewer shows now:
//! messages always, and tool output, thinking, aborted, and hidden turns
//! only while their toggles show them. Opening at a match is the one
//! place content the viewer hides is revealed: the toggles that show the
//! matched turn are switched on, and the footer says so.
//!
//! Highlights are best effort. A match is highlighted on the rendered
//! lines, so a match the wrapping splits across two lines is found for
//! stepping but not painted.

use crate::viewer::input::ViewerEffect;
use crate::viewer::model::TurnKind;
use tuikit::text::find_ci;
use tuikit::text_edit::{EditKey, LineEdit};

use crate::viewer::search::{ContentKinds, ContentQuery, matching_turns};
use crate::viewer::state::{ToolDetail, Transcript, ViewerOpen, ViewerState};

/// How far from the remembered turn to look for a match when the
/// transcript has changed since the search that found it.
const NEARBY_TURNS: usize = 25;

/// Keys of the find prompt: what the prompt means, and the edits every
/// text field shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerFindMsg {
    /// Leave the viewer from the prompt (`Ctrl-C`): dismissing reaches
    /// past the prompt.
    Quit,
    /// Leave the prompt, keeping any earlier find.
    Cancel,
    /// Run the find and jump to its first match.
    Confirm,
    /// Edit the query: a typed character or an editing key.
    Edit(EditKey),
}

/// The find in progress: the prompt while it is open, and the active
/// query with its matches once confirmed.
#[derive(Clone, Debug, Default)]
pub struct FindState {
    /// `Some` while the operator is typing a query.
    pub prompt: Option<LineEdit>,
    /// The active query, trimmed. Empty when no find is active.
    pub query: String,
    /// Indices of the shown turns that match, in order.
    pub matches: Vec<usize>,
    /// The match the viewer is on, as an index into `matches`.
    pub current: Option<usize>,
    /// Matches in turns the current toggles hide.
    pub hidden_matches: usize,
    /// Turn to scroll to when the next draw has composed the lines.
    pub pending_scroll: Option<usize>,
    /// One-shot note for the footer ("showing thinking"); cleared by the
    /// next key.
    pub note: Option<String>,
}

impl FindState {
    /// Whether a find is active (confirmed with a non-empty query).
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Forget the active find, keeping an open prompt.
    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current = None;
        self.hidden_matches = 0;
        self.pending_scroll = None;
    }

    /// The turn the viewer is on.
    pub fn current_turn(&self) -> Option<usize> {
        self.current.and_then(|i| self.matches.get(i).copied())
    }
}

/// Which match to land on when the matches are recomputed.
enum Prefer {
    /// The first match at or below the top of the viewport.
    Viewport,
    /// This turn, if it matches.
    Turn(usize),
}

impl ViewerState {
    /// The kinds of turn the viewer is showing, which is what find
    /// searches.
    fn find_kinds(&self) -> ContentKinds {
        ContentKinds {
            messages: true,
            tools: self.tool_detail.is_visible(),
            thinking: self.show_thinking,
            aborted: self.show_aborted,
            hidden: self.show_hidden,
        }
    }

    /// Open the find prompt, filled with the active query so it can be
    /// edited.
    pub(crate) fn open_find_prompt(&mut self) {
        self.find.prompt = Some(LineEdit::new(self.find.query.as_str()));
    }

    /// The turn that holds the top line of the viewport, from the last
    /// composed lines.
    fn viewport_turn(&self) -> Option<usize> {
        let cache = self.rendered.as_ref()?;
        let at = cache
            .turn_starts
            .partition_point(|start| start.line <= self.scroll_offset);
        cache
            .turn_starts
            .get(at.saturating_sub(1))
            .map(|start| start.turn_index)
    }

    /// Run `query` and land on a match chosen by `prefer`.
    fn run_find(&mut self, query: &str, prefer: Prefer) {
        let query = query.trim().to_string();
        self.find.query.clone_from(&query);
        self.find.matches = if query.is_empty() {
            Vec::new()
        } else {
            matching_turns(
                &self.document,
                &ContentQuery::new(query.clone(), self.find_kinds()),
            )
            .collect()
        };
        let everything = ContentKinds {
            messages: true,
            tools: true,
            thinking: true,
            aborted: true,
            hidden: true,
        };
        self.find.hidden_matches = if query.is_empty() {
            0
        } else {
            matching_turns(&self.document, &ContentQuery::new(query, everything))
                .count()
                .saturating_sub(self.find.matches.len())
        };
        let from = match prefer {
            Prefer::Turn(turn) => turn,
            Prefer::Viewport => self.viewport_turn().unwrap_or(0),
        };
        self.find.current = if self.find.matches.is_empty() {
            None
        } else {
            Some(
                self.find
                    .matches
                    .iter()
                    .position(|&turn| match prefer {
                        Prefer::Turn(_) => turn == from,
                        Prefer::Viewport => turn >= from,
                    })
                    .unwrap_or(0),
            )
        };
        self.find.pending_scroll = self.find.current_turn();
    }

    /// Confirm `query` as the active find and go to its first match at or
    /// after the top of the viewport.
    pub(crate) fn start_find(&mut self, query: &str) {
        self.run_find(query, Prefer::Viewport);
    }

    /// Recompute the matches after a toggle changed which turns are
    /// shown, staying on the current match when it is still shown.
    pub(crate) fn refresh_find(&mut self) {
        if !self.find.is_active() {
            return;
        }
        let query = self.find.query.clone();
        let stay = self.find.current_turn();
        match stay {
            Some(turn) => self.run_find(&query, Prefer::Turn(turn)),
            None => self.run_find(&query, Prefer::Viewport),
        }
        // A toggle changing the layout is not a request to scroll.
        self.find.pending_scroll = None;
    }

    /// Step to the next (`forward`) or previous match, wrapping at the
    /// ends and scrolling it into view.
    pub(crate) fn step_find(&mut self, forward: bool) {
        if !self.find.is_active() {
            self.find.note = Some("nothing to find yet: press / to search".to_string());
            return;
        }
        let count = self.find.matches.len();
        if count == 0 {
            return;
        }
        let (next, wrapped) = match self.find.current {
            Some(current) if forward => ((current + 1) % count, current + 1 >= count),
            Some(current) => ((current + count - 1) % count, current == 0),
            None => {
                let from = self.viewport_turn().unwrap_or(0);
                if forward {
                    let at = self.find.matches.iter().position(|&turn| turn >= from);
                    (at.unwrap_or(0), at.is_none())
                } else {
                    let at = self.find.matches.iter().rposition(|&turn| turn < from);
                    (at.unwrap_or(count - 1), at.is_none())
                }
            }
        };
        self.find.current = Some(next);
        self.find.pending_scroll = self.find.current_turn();
        if wrapped {
            self.find.note = Some(
                if forward {
                    "wrapped to the first match"
                } else {
                    "wrapped to the last match"
                }
                .to_string(),
            );
        }
    }

    /// Land on a search result: find the match near `open.turn_index`,
    /// show the turn if the toggles hide it, and highlight the query.
    pub fn open_at(&mut self, open: &ViewerOpen) {
        let needle = open.query.trim();
        if needle.is_empty() {
            return;
        }
        let Some(target) = self.nearest_match(open.turn_index, needle) else {
            self.find.clear();
            self.find.query = needle.to_string();
            self.find.note = Some(gone_note(needle));
            return;
        };
        let mut notes: Vec<&str> = self.reveal(target);
        if target != open.turn_index {
            notes.push(CHANGED_NOTE);
        }
        self.find.note = (!notes.is_empty()).then(|| notes.join(", "));
        self.stick_to_end = false;
        self.run_find(needle, Prefer::Turn(target));
    }
}

/// What the note says when a search's match is no longer in the
/// transcript.
fn gone_note(needle: &str) -> String {
    format!("\"{needle}\" is no longer in this transcript")
}

/// What the note adds when the match was found away from the turn the
/// search remembered.
const CHANGED_NOTE: &str = "the transcript changed; nearest match";

impl<M> Transcript<M> {
    /// The turn nearest `turn` whose body holds `needle`, shown or not:
    /// `turn` itself, then within [`NEARBY_TURNS`] of it (the earlier of
    /// two as near first), then the first anywhere. `None` when no turn
    /// holds it.
    pub fn nearest_match(&self, turn: usize, needle: &str) -> Option<usize> {
        let turns = &self.document.turns;
        let matches = |index: usize| {
            turns
                .get(index)
                .is_some_and(|turn| find_ci(&turn.body, needle).is_some())
        };
        if matches(turn) {
            return Some(turn);
        }
        (1..=NEARBY_TURNS)
            .find_map(|distance| {
                turn.checked_sub(distance)
                    .filter(|&i| matches(i))
                    .or_else(|| {
                        let after = turn + distance;
                        matches(after).then_some(after)
                    })
            })
            .or_else(|| (0..turns.len()).find(|&i| matches(i)))
    }

    /// Switch on whatever hides turn `index`, returning what was shown.
    pub fn reveal(&mut self, index: usize) -> Vec<&'static str> {
        let Some(turn) = self.document.turns.get(index) else {
            return Vec::new();
        };
        let (aborted, hidden, kind) = (turn.aborted, turn.hidden, turn.kind);
        let mut shown = Vec::new();
        if aborted && !self.show_aborted {
            self.show_aborted = true;
            shown.push("showing aborted turns");
        }
        if hidden && !self.show_hidden {
            self.show_hidden = true;
            shown.push("showing hidden turns");
        }
        match kind {
            TurnKind::Thinking if !self.show_thinking => {
                self.show_thinking = true;
                shown.push("showing thinking");
            }
            TurnKind::ToolUse | TurnKind::ToolResult if self.tool_detail != ToolDetail::Full => {
                self.tool_detail = ToolDetail::Full;
                shown.push("showing full tool output");
            }
            _ => {}
        }
        if !shown.is_empty() {
            self.invalidate_render_cache();
        }
        shown
    }
}

/// Apply one key of the find prompt. Does nothing when no prompt is
/// open. [`ViewerFindMsg::Quit`] asks to leave the viewer.
pub fn reduce_find(mut state: ViewerState, msg: ViewerFindMsg) -> (ViewerState, ViewerEffect) {
    state.find.note = None;
    state.stick_to_end = false;
    let Some(prompt) = state.find.prompt.as_mut() else {
        return (state, ViewerEffect::None);
    };
    match msg {
        ViewerFindMsg::Edit(edit) => {
            prompt.apply(edit);
        }
        ViewerFindMsg::Quit => return (state, ViewerEffect::Close),
        ViewerFindMsg::Cancel => state.find.prompt = None,
        ViewerFindMsg::Confirm => {
            let text = prompt.value().trim().to_string();
            state.find.prompt = None;
            if text.is_empty() {
                state.find.clear();
            } else {
                state.start_find(&text);
            }
        }
    }
    (state, ViewerEffect::None)
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
