//! Opening the viewer, at its end or at a match from a search result.
//!
//! The find inside the viewer is the framework pager's (`/`, `n`, `N`);
//! its unit is a turn the toggles show ([`crate::viewer::widget`]), and
//! matching uses the shared content matcher ([`crate::viewer::search`]),
//! so `n` and `N` agree with content search about what a match is.
//! Opening at a match is the one place content the viewer hides is
//! revealed: the toggles that show the matched turn are switched on, and
//! the footer says so.
//!
//! Highlights are best effort. A match is highlighted on the rendered
//! lines, so a match the wrapping splits across two lines is found for
//! stepping but not painted.

use tuikit::feedback::Feedback;
use tuikit::pager::Pager;
use tuikit::text::find_ci;

use crate::viewer::model::TurnKind;
use crate::viewer::state::{ToolDetail, Transcript, ViewerOpen};

/// How far from the remembered turn to look for a match when the
/// transcript has changed since the search that found it.
const NEARBY_TURNS: usize = 25;

/// What the note adds when the match was found away from the turn the
/// search remembered.
const CHANGED_NOTE: &str = "the transcript changed; nearest match";

/// The framework's pager over `transcript`, opened at the end, on its
/// newest turns (ADR 0052), and kept there as the toggles lay it out
/// again until a key moves it. With `open`, it is landed on that search
/// result instead ([`land`]).
pub fn viewer<M>(transcript: Transcript<M>, open: Option<&ViewerOpen>) -> Pager<Transcript<M>> {
    let mut pager = Pager::new(transcript).at_end();
    if let Some(open) = open {
        land(&mut pager, open);
    }
    pager
}

/// Land `pager` on a search result: the match nearest the turn the search
/// remembered, with the query as the active find. The toggles that show
/// the matched turn are switched on, and the footer's note says what was
/// turned on (and, when the transcript changed since the search, that
/// this is the nearest match) until the next key. A query that is no
/// longer in the transcript leaves the viewer where it was with the find
/// active and a warning saying so; an empty query does nothing.
pub fn land<M>(pager: &mut Pager<Transcript<M>>, open: &ViewerOpen) {
    let needle = open.query.trim();
    if needle.is_empty() {
        return;
    }
    let Some(target) = pager.content().nearest_match(open.turn_index, needle) else {
        pager.land(needle, open.turn_index);
        pager.set_note(Feedback::warning(format!(
            "\"{needle}\" is no longer in this transcript"
        )));
        return;
    };
    let mut notes = pager.content_mut().reveal(target);
    pager.refresh();
    if target != open.turn_index {
        notes.push(CHANGED_NOTE);
    }
    pager.land(needle, target);
    if !notes.is_empty() {
        pager.set_note(Feedback::info(notes.join(", ")));
    }
}

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

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
