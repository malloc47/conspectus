//! Pure viewer state.
//!
//! Per ADR 0024 the reducer is pure and synchronous; `widget.rs`
//! consumes this state and writes back per-draw layout metrics
//! (`viewport_height`, `total_lines`) so navigation messages
//! (page-down etc.) know how far to move and the scroll offset
//! can be clamped.
//!
//! The optional `rendered` cache holds the previously composed body
//! lines so scrolling-only frames don't re-run `tui_markdown::from_str`
//! over every Message turn. Invalidated whenever `content_width`,
//! `show_tools`, or `show_thinking` change.

use ratatui::text::Line;

use crate::viewer::find::FindState;
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

/// Everything the viewer needs to render itself.
#[derive(Clone, Debug)]
pub struct ViewerState {
    pub document: TranscriptDocument,
    /// Current scroll offset (line offset from the rendered body
    /// top). Clamped on every draw against `total_lines` and
    /// `viewport_height`.
    pub scroll_offset: usize,
    /// When `true`, the next draw pins the scroll offset to the
    /// bottom of the document. `JumpToEnd` sets it; initial state
    /// starts with it on so the modal opens on the last turn
    /// (ADR 0052).
    pub stick_to_end: bool,
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
    /// When `true`, the modal renders a centered help-overlay panel
    /// over the body listing every viewer keybinding. Toggled by
    /// `?`; closed by `?` or `Esc`.
    pub show_help: bool,
    /// First visible line of the help page while it is open; the help
    /// widget clamps it to what the page can scroll.
    pub help_scroll: usize,
    /// Rows of the help page the last draw showed; its page keys move by
    /// this many (the transcript's own viewport is taller).
    pub help_page: usize,
    /// Viewport height (in cells / lines) last seen during draw.
    /// Used by the reducer to compute page/half-page deltas.
    pub viewport_height: u16,
    /// Total rendered line count produced by the last draw. Used
    /// by the reducer to clamp scroll offsets.
    pub total_lines: usize,
    /// Cache of composed body lines. `None` means "rebuild on next
    /// draw". The widget populates this; the reducer invalidates
    /// it whenever a flag that affects layout changes (`tool_detail`,
    /// `show_thinking`).
    pub rendered: Option<RenderCache>,
    /// Find-in-transcript state: the prompt, the active query, and its
    /// matches.
    pub find: FindState,
    /// Footer hint for the find prompt, generated from the prompt's key
    /// table by whoever opens the viewer. Empty shows no hint.
    pub find_hint: String,
}

impl ViewerState {
    /// Construct a fresh state for the given document. Opens on the
    /// last turn (`stick_to_end = true`).
    pub fn new(document: TranscriptDocument) -> Self {
        Self {
            document,
            scroll_offset: 0,
            stick_to_end: true,
            tool_detail: ToolDetail::Hidden,
            show_thinking: false,
            show_aborted: false,
            show_hidden: false,
            show_help: false,
            help_scroll: 0,
            help_page: 0,
            viewport_height: 0,
            total_lines: 0,
            rendered: None,
            find: FindState::default(),
            find_hint: String::new(),
        }
    }

    /// Drop the cached body-line composition. Called by the reducer
    /// when a flag that affects layout changes.
    pub fn invalidate_render_cache(&mut self) {
        self.rendered = None;
    }

    /// Maximum scroll offset given the latest layout. Saturating
    /// arithmetic, so `total_lines < viewport_height` clamps to 0.
    pub fn max_scroll(&self) -> usize {
        self.total_lines
            .saturating_sub(self.viewport_height as usize)
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
    fn fresh_state_opens_on_last_turn_with_clean_flags() {
        let state = ViewerState::new(empty_doc());
        assert_eq!(state.scroll_offset, 0);
        assert!(state.stick_to_end);
        assert_eq!(state.tool_detail, ToolDetail::Hidden);
        assert!(!state.show_thinking);
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

    #[test]
    fn max_scroll_clamps_to_zero_when_doc_fits() {
        let mut state = ViewerState::new(empty_doc());
        state.total_lines = 10;
        state.viewport_height = 24;
        assert_eq!(state.max_scroll(), 0);
    }

    #[test]
    fn max_scroll_subtracts_viewport_when_doc_exceeds() {
        let mut state = ViewerState::new(empty_doc());
        state.total_lines = 100;
        state.viewport_height = 24;
        assert_eq!(state.max_scroll(), 76);
    }
}
