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

use crate::viewer::search::{ContentKinds, ContentQuery, matching_turns};
use crate::viewer::state::{ToolDetail, ViewerOpen, ViewerState};

/// How far from the remembered turn to look for a match when the
/// transcript has changed since the search that found it.
const NEARBY_TURNS: usize = 25;

/// Keys of the find prompt. Typed characters arrive as [`Self::Insert`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerFindMsg {
    /// Leave the prompt, keeping any earlier find.
    Cancel,
    /// Run the find and jump to its first match.
    Confirm,
    Backspace,
    Delete,
    Left,
    Right,
    /// Move to the start of the word before the cursor.
    WordLeft,
    /// Move to the start of the word after the cursor.
    WordRight,
    Home,
    End,
    /// Delete everything before the cursor.
    ClearLine,
    /// A typed character.
    Insert(char),
}

/// The line being typed at the find prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindPrompt {
    pub text: String,
    /// Cursor position in characters.
    pub cursor: usize,
}

impl FindPrompt {
    pub fn with_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            cursor: text.chars().count(),
        }
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(index, _)| index)
    }

    pub fn insert(&mut self, c: char) {
        let at = self.byte_at(self.cursor);
        self.text.insert(at, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let at = self.byte_at(self.cursor);
        self.text.remove(at);
    }

    pub fn delete(&mut self) {
        let at = self.byte_at(self.cursor);
        if at < self.text.len() {
            self.text.remove(at);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    /// Move to the start of the word before the cursor, the way the
    /// TUI's other text fields do: back over anything that isn't part of
    /// a word, then over the word.
    pub fn word_left(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut at = self.cursor.min(chars.len());
        while at > 0 && !is_word_char(chars[at - 1]) {
            at -= 1;
        }
        while at > 0 && is_word_char(chars[at - 1]) {
            at -= 1;
        }
        self.cursor = at;
    }

    /// Move to the start of the word after the cursor (the end of the
    /// text when there is none): over the rest of this word, then over
    /// what separates it from the next.
    pub fn word_right(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut at = self.cursor.min(chars.len());
        while at < chars.len() && is_word_char(chars[at]) {
            at += 1;
        }
        while at < chars.len() && !is_word_char(chars[at]) {
            at += 1;
        }
        self.cursor = at;
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    pub fn clear_line(&mut self) {
        let at = self.byte_at(self.cursor);
        self.text.drain(..at);
        self.cursor = 0;
    }
}

/// Whether `c` is part of a word for word motion: anything but
/// whitespace and ASCII punctuation, as in the TUI's line editor.
fn is_word_char(c: char) -> bool {
    !c.is_whitespace() && !c.is_ascii_punctuation()
}

/// The find in progress: the prompt while it is open, and the active
/// query with its matches once confirmed.
#[derive(Clone, Debug, Default)]
pub struct FindState {
    /// `Some` while the operator is typing a query.
    pub prompt: Option<FindPrompt>,
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
        self.find.prompt = Some(FindPrompt::with_text(&self.find.query));
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
        let turns = &self.document.turns;
        let matches = |index: usize| {
            turns
                .get(index)
                .is_some_and(|turn| find_ci(&turn.body, needle).is_some())
        };
        let target = if matches(open.turn_index) {
            Some(open.turn_index)
        } else {
            (1..=NEARBY_TURNS)
                .find_map(|distance| {
                    open.turn_index
                        .checked_sub(distance)
                        .filter(|&i| matches(i))
                        .or_else(|| {
                            let after = open.turn_index + distance;
                            matches(after).then_some(after)
                        })
                })
                .or_else(|| (0..turns.len()).find(|&i| matches(i)))
        };
        let Some(target) = target else {
            self.find.clear();
            self.find.query = needle.to_string();
            self.find.note = Some(format!("\"{needle}\" is no longer in this transcript"));
            return;
        };
        let mut notes: Vec<&str> = self.reveal_turn(target);
        if target != open.turn_index {
            notes.push("the transcript changed; nearest match");
        }
        self.find.note = (!notes.is_empty()).then(|| notes.join(", "));
        self.stick_to_end = false;
        self.run_find(needle, Prefer::Turn(target));
    }

    /// Switch on whatever hides turn `index`, returning what was shown.
    fn reveal_turn(&mut self, index: usize) -> Vec<&'static str> {
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
/// open.
pub fn reduce_find(mut state: ViewerState, msg: ViewerFindMsg) -> (ViewerState, ViewerEffect) {
    state.find.note = None;
    state.stick_to_end = false;
    let Some(prompt) = state.find.prompt.as_mut() else {
        return (state, ViewerEffect::None);
    };
    match msg {
        ViewerFindMsg::Insert(c) => prompt.insert(c),
        ViewerFindMsg::Backspace => prompt.backspace(),
        ViewerFindMsg::Delete => prompt.delete(),
        ViewerFindMsg::Left => prompt.left(),
        ViewerFindMsg::Right => prompt.right(),
        ViewerFindMsg::WordLeft => prompt.word_left(),
        ViewerFindMsg::WordRight => prompt.word_right(),
        ViewerFindMsg::Home => prompt.home(),
        ViewerFindMsg::End => prompt.end(),
        ViewerFindMsg::ClearLine => prompt.clear_line(),
        ViewerFindMsg::Cancel => state.find.prompt = None,
        ViewerFindMsg::Confirm => {
            let text = prompt.text.trim().to_string();
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
