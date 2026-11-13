//! The viewer's state: a [`Transcript`] and which of its turns show.
//!
//! Where the viewer is scrolled, its find, and its help page are the
//! framework pager's (`tuikit::pager`); the transcript holds what is
//! the transcript's own. The optional `rendered` cache holds the
//! previously composed body lines so scrolling-only frames don't re-run
//! `tui_markdown::from_str` over every Message turn. It is keyed by the
//! content width and the four toggles.

use std::marker::PhantomData;

use ratatui::text::Line;

use crate::viewer::model::TranscriptDocument;

/// How tool-use / tool-result turns are rendered.
///
/// Four levels matching `claude-history`'s `--show-tools`:
/// * `Hidden`: tool turns drop out entirely (chip-less, no body).
/// * `Summary`: tool chip + the call's name line; outputs and
///   long argument bodies collapse to a one-line `(N lines)` marker.
/// * `Truncated`: full call + up to 8 lines of output, with a
///   trailing `… (N more lines)` marker if more remain.
/// * `Full`: everything verbatim.
///
/// Cycle order: `Hidden → Summary → Truncated → Full → Hidden`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolDetail {
    Hidden,
    Summary,
    Truncated,
    Full,
}

impl ToolDetail {
    /// Every level, in cycle order.
    pub const ALL: [Self; 4] = [Self::Hidden, Self::Summary, Self::Truncated, Self::Full];

    /// Next state in the cycle. Bound to `t` in the viewer.
    pub fn cycle(self) -> Self {
        match self {
            Self::Hidden => Self::Summary,
            Self::Summary => Self::Truncated,
            Self::Truncated => Self::Full,
            Self::Full => Self::Hidden,
        }
    }

    /// Short label for footer chips: `off` / `sum` / `trunc` / `all`.
    pub fn footer_label(self) -> &'static str {
        match self {
            Self::Hidden => "off",
            Self::Summary => "sum",
            Self::Truncated => "trunc",
            Self::Full => "all",
        }
    }

    /// True iff a tool turn renders any body content at all.
    pub fn is_visible(self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

/// Where one turn begins in the composed body lines.
///
/// A run of tool turns shown as one summary line (tool detail
/// `Summary`) gives every turn in the run the same start, flagged
/// `collapsed`: the line is a count of calls, not the turn's text, so
/// nothing in it is highlighted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnStart {
    pub turn_index: usize,
    pub line: usize,
    pub collapsed: bool,
}

/// Cached body-line composition. Lifetime-erased so it can live on
/// the state across draws.
#[derive(Clone, Debug)]
pub struct RenderCache {
    pub content_width: u16,
    pub tool_detail: ToolDetail,
    pub show_thinking: bool,
    pub show_aborted: bool,
    pub show_hidden: bool,
    pub lines: Vec<Line<'static>>,
    /// Where each shown turn begins in `lines`, in order.
    pub turn_starts: Vec<TurnStart>,
    /// Display columns the gutter (chip and separator) takes at the
    /// start of every line; find highlights only what follows it.
    pub leader: usize,
}

/// Where to land when the viewer opens from a search result: the
/// query that found the session and the turn it matched. If the
/// transcript has changed since, the viewer looks near that turn for the
/// match and says so when it can't find one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerOpen {
    pub query: String,
    pub turn_index: usize,
}

/// A transcript as a pager's content (`tuikit::pager::PagerContent`,
/// implemented in [`crate::viewer::widget`]): the document, which of its
/// turns show and how, the rows last laid out from them, and the named
/// style of the header's title. `M` is the message of the pager it sits
/// in, which the transcript never sends: its keys only show or hide
/// turns.
#[derive(Clone, Debug)]
pub struct Transcript<M = ()> {
    pub document: TranscriptDocument,
    /// Tool detail level. Cycled by `t`; default `Hidden`.
    pub tool_detail: ToolDetail,
    /// When `true`, `Thinking` turns are rendered. Off by default
    /// (matches `claude-history --show-thinking`).
    pub show_thinking: bool,
    /// When `true`, turns tagged `aborted: true` by the parser are
    /// rendered. Off by default so the viewer matches what the
    /// harness UI showed at chat time — see
    /// [`crate::viewer::model::TranscriptTurn::aborted`] for how
    /// detection works per harness. Toggled by capital-`I`.
    pub show_aborted: bool,
    /// When `true`, turns tagged `hidden: true` by the parser (content
    /// the harness's UI never showed) are rendered. Off by default;
    /// toggled by capital-`H` (ADR 0122).
    pub show_hidden: bool,
    /// The named style the header's title is drawn in, which the opener
    /// chooses (a harness's identity color); empty for the accent.
    pub title_role: String,
    /// Cache of composed body lines, kept while the width and the
    /// toggles are unchanged (its key holds both). `None` means
    /// "compose on the next layout".
    pub rendered: Option<RenderCache>,
    message: PhantomData<fn() -> M>,
}

impl<M> Transcript<M> {
    /// The document with the toggles' defaults: messages and compaction
    /// summaries shown, tool turns, thinking, aborted, and hidden turns
    /// hidden.
    pub fn new(document: TranscriptDocument) -> Self {
        Self {
            document,
            tool_detail: ToolDetail::Hidden,
            show_thinking: false,
            show_aborted: false,
            show_hidden: false,
            title_role: String::new(),
            rendered: None,
            message: PhantomData,
        }
    }

    /// Draws the header's title in the named style `role`.
    #[must_use]
    pub fn title_role(mut self, role: impl Into<String>) -> Self {
        self.title_role = role.into();
        self
    }

    /// Drop the cached body-line composition.
    pub fn invalidate_render_cache(&mut self) {
        self.rendered = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::model::{SessionLocator, TranscriptDocument};
    use std::path::PathBuf;

    fn empty_doc() -> TranscriptDocument {
        TranscriptDocument::unavailable(&SessionLocator {
            harness_key: "claude-code".to_string(),
            session_key: "k".to_string(),
            state_root: PathBuf::from("/x"),
        })
    }

    #[test]
    fn a_fresh_transcript_shows_only_messages() {
        let transcript: Transcript = Transcript::new(empty_doc());
        assert_eq!(transcript.tool_detail, ToolDetail::Hidden);
        assert!(!transcript.show_thinking);
        assert!(!transcript.show_aborted && !transcript.show_hidden);
        assert!(transcript.rendered.is_none());
        assert_eq!(
            transcript.title_role("harness.codex").title_role,
            "harness.codex"
        );
    }

    #[test]
    fn tool_detail_cycles_through_four_levels() {
        let mut d = ToolDetail::Hidden;
        d = d.cycle();
        assert_eq!(d, ToolDetail::Summary);
        d = d.cycle();
        assert_eq!(d, ToolDetail::Truncated);
        d = d.cycle();
        assert_eq!(d, ToolDetail::Full);
        d = d.cycle();
        assert_eq!(d, ToolDetail::Hidden);
    }
}
