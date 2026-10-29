//! Controls overlay (ADR 0031).
//!
//! Single navigable modal that fronts view switching, per-view
//! grouping, per-view filter editing, the global sort toggle, and the
//! preview pane's wrap mode.
//! The controls overlay is the discoverable surface; single-key
//! accelerators (`v`, `1`–`5`, `]`/`[`, `f`, `F`, `G`) reach the
//! same outcomes for muscle-memory operators.
//!
//! ## Architecture
//!
//! The overlay holds **only** what is unique to its UI: a cursor
//! pointing at the currently-highlighted row and an optional
//! sub-editor (the framework's checklist for harness, mux state, and
//! PR state, its text prompt for max age and contains). Live state —
//! the active view, grouping,
//! filter, and sort — lives on [`crate::tui::app::App`]. Each
//! [`ControlsOverlayState::handle_key`] call takes a
//! [`ControlsContext`] borrowed from the app so the overlay always
//! reflects the current world.
//!
//! Key handling returns a [`ControlsOutcome`] describing what the
//! caller should do: keep the overlay open, close it, or apply a
//! [`crate::tui::Msg`] (and either close or stay open per the
//! outcome). The reducer applies messages; the overlay never
//! mutates the app directly.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use tuikit::frame::{Lines, ModalFrame, ModalSize};
use tuikit::glyphs::marks;
use tuikit::selection::{self, Cursor, Ends};

use crate::filter::{
    HarnessFilter, MuxStateFilter, MuxStateKey, PrBranchFilter, PrStateFilter, PrStateKey,
    RowFilter,
};
use crate::tui::Theme;
use crate::tui::{Grouping, Sort, View};
use tuikit::checklist::{Checklist, Item};
use tuikit::overlay::{Outcome, Overlay};
use tuikit::text_prompt::{PromptResult, TextPrompt};

/// Registered harness keys surfaced in the harness sub-editor.
/// Derives from the adapter registry
/// ([`crate::discovery::harness::harness_keys`]) so registering a
/// new adapter appears in the filter menu automatically.
pub fn harness_options() -> &'static [&'static str] {
    crate::discovery::harness::harness_keys()
}

/// Display order of the mux-state sub-editor entries.
/// The PR states the PRs view's state filter offers (ADR 0134).
pub const PR_STATE_OPTIONS: &[PrStateKey] = &PrStateKey::ALL;

pub const MUX_STATE_OPTIONS: &[MuxStateKey] = &[
    MuxStateKey::Attached,
    MuxStateKey::Ambiguous,
    MuxStateKey::Unmuxed,
];

/// All views displayed in the View section, in stable order. This
/// constant is the single source of truth for the controls-overlay View
/// section, the left-pane tab strip, and the `[` / `]` view-cycle
/// accelerator. It lists every [`View`].
pub const VIEW_OPTIONS: &[View] = &[
    View::Sessions,
    View::Mux,
    View::Prs,
    View::Pinned,
    View::History,
    View::Search,
];

/// Sort options surfaced in the Sort section, in stable order.
pub const SORT_OPTIONS: &[Sort] = &[Sort::Hierarchy, Sort::Recency];

/// Read-only snapshot of the live state the overlay renders against.
/// The renderer and the key dispatcher both consume this so the
/// overlay never holds a stale copy.
#[derive(Debug, Clone)]
pub struct ControlsContext<'a> {
    pub view: View,
    pub grouping: Grouping,
    pub filter: &'a RowFilter,
    pub sort: Sort,
    /// Mux-view recency basis. Read so the overlay
    /// can mark the active basis; only surfaced when the mux view is
    /// active.
    pub mux_recency: crate::tui::MuxRecency,
    /// Preview-pane wrap mode, marked active in the Preview wrap
    /// section (ADR 0106).
    pub preview_wrap: crate::tui::PreviewWrap,
    /// What the Preview shows for agent sessions in tmux, marked
    /// active in the Session preview section (ADR 0112).
    pub session_preview: crate::tui::SessionPreview,
    /// The active content search: its text and how far its scan has got.
    pub content: Option<(&'a str, crate::tui::ContentStatus, usize)>,
    /// Which kinds of turn a new content search reads (`[search]`).
    pub search_kinds: crate::viewer::search::ContentKinds,
    /// How content search results are listed (Search view).
    pub content_group: crate::viewer::search::Grouping,
    /// Which remembered search the Search view shows, and how many there
    /// are.
    pub search_history: Option<(usize, usize)>,
}

/// One landable row in the controls overlay's flat list.
/// Section headers are not landable — the cursor skips them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsCursor {
    /// View option at index in [`VIEW_OPTIONS`].
    View(usize),
    /// Grouping option at index in
    /// `Grouping::values_for(ctx.view)`.
    Grouping(usize),
    FilterHarness,
    FilterMaxAge,
    FilterMuxState,
    /// Edits the content query that narrows the session lists.
    FilterContent,
    /// Search-view-only: list results per session or per message.
    ContentGroup,
    /// Search-view-only: pick among the remembered searches.
    SearchHistory,
    /// Open the contents search modal, for terminals that do not send
    /// Ctrl-/ and operators who have not learned `s`.
    SearchContents,
    /// Open the contents search options: what a search looks in.
    SearchOptions,
    /// Sessions-view-only checkbox: float muxed sessions to the top.
    FilterFloatMuxedSessions,
    /// Mux-view-only checkbox: float attached muxes to the top.
    FilterFloatAttachedMuxes,
    /// Mux-view-only checkbox: list every stopped mux, not just pinned ones.
    FilterShowStoppedMuxes,
    FilterPinnedOnly,
    /// History-view-only checkbox: list only records no pin points at.
    FilterHistoryUnpinnedOnly,
    /// PRs-view-only row: edits the PR state set (ADR 0134).
    FilterPrState,
    /// PRs-view-only row: cycles the local-branch filter through any,
    /// tracked, and untracked (ADR 0129).
    FilterPrBranch,
    /// PRs-view-only checkbox: float PRs with a local branch to the top.
    FilterFloatTrackedPrs,
    FilterClear,
    /// Sort option at index in [`SORT_OPTIONS`].
    Sort(usize),
    /// Mux-view-only recency basis at index in
    /// [`crate::tui::MuxRecency::ALL`].
    MuxRecency(usize),
    /// Preview wrap mode at index in [`crate::tui::PreviewWrap::ALL`].
    PreviewWrap(usize),
    /// Session preview at index in [`crate::tui::SessionPreview::ALL`].
    SessionPreview(usize),
}

/// Sub-editor that owns key input while open. The host overlay
/// suspends its own bindings until the sub-editor commits or
/// cancels.
#[derive(Debug, Clone)]
pub enum SubEditor {
    /// The harness filter's checklist, which commits the checked
    /// harness keys.
    Harness(Checklist<Vec<&'static str>>),
    /// The max-age filter's text, parsed by its prompt: no age, or the
    /// age to keep.
    MaxAge(TextPrompt<Option<std::time::Duration>>),
    /// The content query text: none clears the search.
    Content(TextPrompt<Option<String>>),
    /// The mux-state filter's checklist, which commits the checked
    /// states.
    MuxState(Checklist<Vec<MuxStateKey>>),
    /// The PR-state filter's checklist, which commits the checked
    /// states.
    PrState(Checklist<Vec<PrStateKey>>),
}

impl SubEditor {
    /// Draws the editor over `area`, keeping a checklist's scroll window
    /// for its next key.
    fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        match self {
            Self::Harness(list) => list.draw(area, buf, theme),
            Self::MuxState(list) => list.draw(area, buf, theme),
            Self::PrState(list) => list.draw(area, buf, theme),
            Self::MaxAge(prompt) => prompt.draw(area, buf, theme),
            Self::Content(prompt) => prompt.draw(area, buf, theme),
        }
    }
}

/// What the controls overlay returned from a single key event.
/// Committed variants carry a [`crate::tui::Msg`] the runtime
/// dispatches through the reducer (ADR 0085 contract 3).
#[derive(Debug, Clone, PartialEq)]
pub enum ControlsOutcome {
    /// Nothing to do — overlay stays open, no app state change.
    Continue,
    /// Close the overlay without applying anything (Esc at top level).
    Close,
    /// Apply the message and leave the overlay open so the operator
    /// can see chips update in place.
    ApplyAndStay(crate::tui::Msg),
    /// Apply the message and close (view switch is the canonical
    /// case — operators expect the overlay to dismiss after picking
    /// a view).
    ApplyAndClose(crate::tui::Msg),
}

/// Pure state for the controls overlay: cursor position plus the
/// active sub-editor (if any).
#[derive(Debug, Clone)]
pub struct ControlsOverlayState {
    cursor: ControlsCursor,
    sub_editor: Option<SubEditor>,
}

impl ControlsOverlayState {
    /// Open at the top of the overlay. Operators most often want to
    /// switch view, so the cursor lands on the active view row.
    pub fn new(ctx: &ControlsContext<'_>) -> Self {
        let active_view_idx = VIEW_OPTIONS
            .iter()
            .position(|v| *v == ctx.view)
            .unwrap_or(0);
        Self {
            cursor: ControlsCursor::View(active_view_idx),
            sub_editor: None,
        }
    }

    /// Open with the cursor positioned on the Filters > Harness row.
    /// Used by the `f` accelerator so jumping straight into
    /// filters skips the navigation step.
    pub fn new_at_filters(_ctx: &ControlsContext<'_>) -> Self {
        Self {
            cursor: ControlsCursor::FilterHarness,
            sub_editor: None,
        }
    }

    pub fn cursor(&self) -> ControlsCursor {
        self.cursor
    }

    pub fn sub_editor(&self) -> Option<&SubEditor> {
        self.sub_editor.as_ref()
    }

    /// Whether an open editor takes text (the max-age and contains
    /// fields), so typed characters, `q` included, go into it.
    pub fn editor_takes_text(&self) -> bool {
        matches!(
            self.sub_editor,
            Some(SubEditor::MaxAge(_) | SubEditor::Content(_))
        )
    }

    /// Dispatch a crossterm key event. Returns the outcome the
    /// caller should apply (or [`ControlsOutcome::Continue`] for
    /// in-overlay navigation that didn't change anything).
    pub fn handle_key(&mut self, ctx: &ControlsContext<'_>, event: KeyEvent) -> ControlsOutcome {
        use crate::tui::keybindings::overlay::{CONTROLS_KEYS, ControlsKey};
        use tuikit::keys::resolve;
        let action = resolve(CONTROLS_KEYS, event);
        // Ctrl-C dismisses the overlay from any level, and `q` does too
        // unless an editor that takes text has the keys (house rule 4).
        if action == Some(ControlsKey::Abort)
            || action == Some(ControlsKey::Quit) && !self.editor_takes_text()
        {
            return ControlsOutcome::Close;
        }

        // While a sub-editor owns input, route every key through it
        // and translate its outcome. We dispatch directly on the
        // `Option<SubEditor>` slot so the helper can clear it on
        // commit/cancel without taking a second mutable borrow.
        if self.sub_editor.is_some() {
            let outcome = Self::dispatch_sub_editor(&mut self.sub_editor, ctx, event);
            return outcome;
        }

        match action {
            Some(ControlsKey::Abort | ControlsKey::Quit | ControlsKey::Close) => {
                ControlsOutcome::Close
            }
            Some(ControlsKey::Up) => {
                self.cursor = move_cursor(self.cursor, ctx, -1);
                ControlsOutcome::Continue
            }
            Some(ControlsKey::Down) => {
                self.cursor = move_cursor(self.cursor, ctx, 1);
                ControlsOutcome::Continue
            }
            Some(ControlsKey::Toggle) => self.activate(ctx),
            Some(ControlsKey::Done) => self.done(ctx),
            None => ControlsOutcome::Continue,
        }
    }

    /// Enter: apply the focused choice and close; open an editor row's
    /// editor or press a button; otherwise (checkboxes and cycles, which
    /// Space changes and which are already applied) just close.
    fn done(&mut self, ctx: &ControlsContext<'_>) -> ControlsOutcome {
        let is_choice = matches!(
            self.cursor,
            ControlsCursor::View(_)
                | ControlsCursor::Grouping(_)
                | ControlsCursor::Sort(_)
                | ControlsCursor::MuxRecency(_)
                | ControlsCursor::PreviewWrap(_)
                | ControlsCursor::SessionPreview(_)
        );
        // A button presses, and an editor row opens its editor, on Enter
        // as on Space.
        if matches!(
            self.cursor,
            ControlsCursor::FilterClear
                | ControlsCursor::FilterHarness
                | ControlsCursor::FilterMaxAge
                | ControlsCursor::FilterMuxState
                | ControlsCursor::FilterContent
                | ControlsCursor::FilterPrState
                | ControlsCursor::SearchHistory
                | ControlsCursor::SearchContents
                | ControlsCursor::SearchOptions
        ) {
            return self.activate(ctx);
        }
        if !is_choice {
            return ControlsOutcome::Close;
        }
        match self.activate(ctx) {
            ControlsOutcome::ApplyAndStay(msg) => ControlsOutcome::ApplyAndClose(msg),
            _ => ControlsOutcome::Close,
        }
    }

    fn activate(&mut self, ctx: &ControlsContext<'_>) -> ControlsOutcome {
        match self.cursor {
            ControlsCursor::View(idx) => {
                let view = VIEW_OPTIONS[idx];
                if view == ctx.view {
                    ControlsOutcome::Continue
                } else {
                    ControlsOutcome::ApplyAndStay(crate::tui::Msg::SwitchView(view))
                }
            }
            ControlsCursor::Grouping(idx) => {
                let options = Grouping::values_for(ctx.view);
                let chosen = options.get(idx).copied().unwrap_or(ctx.grouping);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetGrouping(chosen))
            }
            ControlsCursor::FilterHarness => {
                self.sub_editor = Some(SubEditor::Harness(build_harness_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterMaxAge => {
                self.sub_editor = Some(SubEditor::MaxAge(build_max_age_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterMuxState => {
                self.sub_editor = Some(SubEditor::MuxState(build_mux_state_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterContent => {
                let initial = ctx.content.map(|(text, ..)| text).unwrap_or_default();
                self.sub_editor = Some(SubEditor::Content(build_content_editor(initial)));
                ControlsOutcome::Continue
            }
            ControlsCursor::ContentGroup => {
                use crate::viewer::search::Grouping as Content;
                let next = match ctx.content_group.granularity() {
                    Content::Session => Content::Flat,
                    _ => Content::Session,
                };
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetContentGroup(next))
            }
            ControlsCursor::SearchHistory => {
                ControlsOutcome::ApplyAndClose(crate::tui::Msg::OpenSearchHistory)
            }
            ControlsCursor::SearchOptions => {
                ControlsOutcome::ApplyAndClose(crate::tui::Msg::OpenSearchOptions)
            }
            ControlsCursor::SearchContents => {
                ControlsOutcome::ApplyAndClose(crate::tui::Msg::OpenContentSearchModal)
            }
            ControlsCursor::FilterFloatMuxedSessions => {
                let mut new_filter = ctx.filter.clone();
                new_filter.float_muxed_sessions_top = !new_filter.float_muxed_sessions_top;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterFloatAttachedMuxes => {
                let mut new_filter = ctx.filter.clone();
                new_filter.float_attached_muxes_top = !new_filter.float_attached_muxes_top;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterShowStoppedMuxes => {
                let mut new_filter = ctx.filter.clone();
                new_filter.show_stopped_muxes = !new_filter.show_stopped_muxes;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterPinnedOnly => {
                let mut new_filter = ctx.filter.clone();
                new_filter.pinned_only = !new_filter.pinned_only;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterHistoryUnpinnedOnly => {
                let mut new_filter = ctx.filter.clone();
                new_filter.show_pinned_history = !new_filter.show_pinned_history;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterPrState => {
                self.sub_editor = Some(SubEditor::PrState(build_pr_state_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterPrBranch => {
                let mut new_filter = ctx.filter.clone();
                new_filter.pr_branch = PrBranchFilter::cycle(new_filter.pr_branch);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterFloatTrackedPrs => {
                let mut new_filter = ctx.filter.clone();
                new_filter.float_tracked_prs_top = !new_filter.float_tracked_prs_top;
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetFilter(new_filter))
            }
            ControlsCursor::FilterClear => {
                if ctx.filter.is_empty() && ctx.content.is_none() {
                    // Nothing to clear; keep the overlay open silently.
                    ControlsOutcome::Continue
                } else {
                    ControlsOutcome::ApplyAndStay(crate::tui::Msg::ClearFilters)
                }
            }
            ControlsCursor::Sort(idx) => {
                let sort = SORT_OPTIONS.get(idx).copied().unwrap_or(ctx.sort);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetSort(sort))
            }
            ControlsCursor::MuxRecency(idx) => {
                let basis = crate::tui::MuxRecency::ALL
                    .get(idx)
                    .copied()
                    .unwrap_or(ctx.mux_recency);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetMuxRecency(basis))
            }
            ControlsCursor::PreviewWrap(idx) => {
                let wrap = crate::tui::PreviewWrap::ALL
                    .get(idx)
                    .copied()
                    .unwrap_or(ctx.preview_wrap);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetPreviewWrap(wrap))
            }
            ControlsCursor::SessionPreview(idx) => {
                let choice = crate::tui::SessionPreview::ALL
                    .get(idx)
                    .copied()
                    .unwrap_or(ctx.session_preview);
                ControlsOutcome::ApplyAndStay(crate::tui::Msg::SetSessionPreview(choice))
            }
        }
    }

    fn dispatch_sub_editor(
        slot: &mut Option<SubEditor>,
        ctx: &ControlsContext<'_>,
        event: KeyEvent,
    ) -> ControlsOutcome {
        let editor = slot
            .as_mut()
            .expect("dispatch called with no sub-editor open");
        // An editor checks what it was given: a checklist commits its
        // checked values, and a text editor's prompt the text it parsed
        // (a refusal stays open with its reason). Closing an editor (Esc)
        // steps back to the panel; dismissing it (Ctrl-C, taken above
        // already, or `q` in a checklist) dismisses the panel.
        match editor {
            SubEditor::Harness(list) => {
                let outcome = list.handle(event, &());
                settle_editor(slot, outcome, |keys| {
                    let values = keys.into_iter().map(str::to_string).collect();
                    crate::tui::Msg::SetFilter(with_harness(ctx.filter.clone(), values))
                })
            }
            SubEditor::MuxState(list) => {
                let outcome = list.handle(event, &());
                settle_editor(slot, outcome, |values| {
                    crate::tui::Msg::SetFilter(with_mux_state(ctx.filter.clone(), values))
                })
            }
            SubEditor::PrState(list) => {
                let outcome = list.handle(event, &());
                settle_editor(slot, outcome, |values| {
                    crate::tui::Msg::SetFilter(with_pr_state(ctx.filter.clone(), values))
                })
            }
            SubEditor::Content(prompt) => {
                let outcome = prompt.handle(event, &());
                settle_editor(slot, outcome, |text| match text {
                    None => crate::tui::Msg::ClearContentSearch,
                    Some(text) => crate::tui::Msg::StartContentSearch {
                        text,
                        kinds: ctx.search_kinds,
                    },
                })
            }
            SubEditor::MaxAge(prompt) => {
                let outcome = prompt.handle(event, &());
                settle_editor(slot, outcome, |max_age| {
                    crate::tui::Msg::SetFilter(with_max_age(ctx.filter.clone(), max_age))
                })
            }
        }
    }
}

/// What an editor's `outcome` means for the panel: a commit closes the
/// editor and applies the message `apply` builds from the value while the
/// panel stays open; a close steps back to the panel; a dismissal closes
/// the panel; anything else keeps the editor open.
fn settle_editor<T>(
    slot: &mut Option<SubEditor>,
    outcome: Outcome<T>,
    apply: impl FnOnce(T) -> crate::tui::Msg,
) -> ControlsOutcome {
    match outcome {
        Outcome::Commit(value) | Outcome::Back(value) => {
            *slot = None;
            ControlsOutcome::ApplyAndStay(apply(value))
        }
        Outcome::Close => {
            *slot = None;
            ControlsOutcome::Continue
        }
        Outcome::Dismiss => ControlsOutcome::Close,
        Outcome::Consumed | Outcome::Stay(_) => ControlsOutcome::Continue,
    }
}

/// A checklist over `options`, each labeled by `label` and checked when
/// `checked` says so, that commits the checked options.
fn checklist_of<T: Clone + Send + Sync + 'static>(
    title: &str,
    options: &'static [T],
    label: impl Fn(&T) -> &str,
    checked: impl Fn(&T) -> bool,
) -> Checklist<Vec<T>> {
    let items = options
        .iter()
        .map(|option| Item::new(label(option)).checked(checked(option)))
        .collect();
    Checklist::new(title, items, move |indices: Vec<usize>| {
        Outcome::Commit(
            indices
                .into_iter()
                .filter_map(|index| options.get(index).cloned())
                .collect(),
        )
    })
}

fn build_harness_editor(filter: &RowFilter) -> Checklist<Vec<&'static str>> {
    let values = match &filter.harness {
        Some(HarnessFilter::Any(values)) => values.as_slice(),
        None => &[],
    };
    checklist_of(
        "harness",
        harness_options(),
        |key| key,
        |key| values.iter().any(|value| value == key),
    )
}

fn build_mux_state_editor(filter: &RowFilter) -> Checklist<Vec<MuxStateKey>> {
    let values = match &filter.mux_state {
        Some(MuxStateFilter::Any(values)) => values.as_slice(),
        None => &[],
    };
    checklist_of(
        "mux state",
        MUX_STATE_OPTIONS,
        |key| key.as_str(),
        |key| values.contains(key),
    )
}

fn build_pr_state_editor(filter: &RowFilter) -> Checklist<Vec<PrStateKey>> {
    let values = match &filter.pr_state {
        Some(PrStateFilter::Any(values)) => values.as_slice(),
        None => &[],
    };
    checklist_of(
        "PR state",
        PR_STATE_OPTIONS,
        |key| key.as_str(),
        |key| values.contains(key),
    )
}

fn with_pr_state(mut filter: RowFilter, values: Vec<PrStateKey>) -> RowFilter {
    filter.pr_state = if values.is_empty() {
        None
    } else {
        Some(PrStateFilter::from_values(values))
    };
    filter
}

/// The max-age editor, seeded with the filter's age. Blank text clears
/// the filter; text [`parse_max_age`] refuses keeps the editor open with
/// the reason.
fn build_max_age_editor(filter: &RowFilter) -> TextPrompt<Option<std::time::Duration>> {
    let initial = filter
        .max_age
        .map(format_duration_for_input)
        .unwrap_or_default();
    TextPrompt::new("max age", initial, |text: &str| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return PromptResult::Commit(None);
        }
        match parse_max_age(trimmed) {
            Ok(max_age) => PromptResult::Commit(Some(max_age)),
            Err(why) => PromptResult::Reject(format!("max age: {why}")),
        }
    })
}

/// The contents editor, seeded with the current query. Blank text
/// clears the content search.
fn build_content_editor(initial: &str) -> TextPrompt<Option<String>> {
    TextPrompt::new("contains", initial, |text: &str| {
        let text = text.trim();
        PromptResult::Commit((!text.is_empty()).then(|| text.to_string()))
    })
}

fn with_harness(mut filter: RowFilter, values: Vec<String>) -> RowFilter {
    filter.harness = if values.is_empty() {
        None
    } else {
        Some(HarnessFilter::from_values(values))
    };
    filter
}

fn with_mux_state(mut filter: RowFilter, values: Vec<MuxStateKey>) -> RowFilter {
    filter.mux_state = if values.is_empty() {
        None
    } else {
        Some(MuxStateFilter::from_values(values))
    };
    filter
}

fn with_max_age(mut filter: RowFilter, max_age: Option<std::time::Duration>) -> RowFilter {
    filter.max_age = max_age;
    filter
}

/// Render a duration as the most compact `<n><unit>` shape that
/// round-trips through [`parse_max_age`]. Used to seed the
/// text-input editor with a value the operator can edit in place.
pub fn format_duration_for_input(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        return "0s".to_string();
    }
    if secs.is_multiple_of(86_400) {
        format!("{}d", secs / 86_400)
    } else if secs.is_multiple_of(3_600) {
        format!("{}h", secs / 3_600)
    } else if secs.is_multiple_of(60) {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// Parse a duration token used in the max-age editor. Mirrors the
/// CLI `parse_filter_duration` shape so a value typed in the
/// overlay matches what the operator would put on the command
/// line. Returns the duration as a [`std::time::Duration`]; failure
/// is propagated as a short error string the host can show inline.
pub fn parse_max_age(raw: &str) -> Result<std::time::Duration, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h/d)".to_string())?;
    let (digits, suffix) = trimmed.split_at(split);
    let value: u64 = digits
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{digits}`"))?;
    let secs = match suffix {
        "ms" => return Ok(std::time::Duration::from_millis(value)),
        "s" => value,
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(3_600),
        "d" => value.saturating_mul(86_400),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h/d)")),
    };
    Ok(std::time::Duration::from_secs(secs))
}

/// Move the cursor by `delta` rows across the section-skipping flat
/// list. Wraps top-to-bottom and bottom-to-top so j/k keep moving
/// off either edge.
fn move_cursor(cursor: ControlsCursor, ctx: &ControlsContext<'_>, delta: i32) -> ControlsCursor {
    let list = flatten_rows(ctx);
    if list.is_empty() {
        return cursor;
    }
    let idx = list.iter().position(|c| *c == cursor).unwrap_or(0);
    list[Ends::Wrap.step(idx, list.len(), delta)]
}

/// All actionable rows in display order. Section headers are
/// implicit (they live in the renderer) so the cursor only needs to
/// know about the rows it can land on.
pub(crate) fn flatten_rows(ctx: &ControlsContext<'_>) -> Vec<ControlsCursor> {
    let mut rows: Vec<ControlsCursor> = Vec::new();
    for idx in 0..VIEW_OPTIONS.len() {
        rows.push(ControlsCursor::View(idx));
    }
    for idx in 0..Grouping::values_for(ctx.view).len() {
        rows.push(ControlsCursor::Grouping(idx));
    }
    if matches!(ctx.view, View::Search) {
        rows.push(ControlsCursor::ContentGroup);
        rows.push(ControlsCursor::SearchHistory);
    }
    rows.push(ControlsCursor::SearchContents);
    rows.push(ControlsCursor::SearchOptions);
    rows.push(ControlsCursor::FilterHarness);
    rows.push(ControlsCursor::FilterMaxAge);
    rows.push(ControlsCursor::FilterMuxState);
    if matches!(
        ctx.view,
        View::Sessions | View::Mux | View::Prs | View::Search
    ) {
        rows.push(ControlsCursor::FilterContent);
    }
    if matches!(ctx.view, View::Sessions) {
        rows.push(ControlsCursor::FilterFloatMuxedSessions);
    }
    if matches!(ctx.view, View::Mux) {
        rows.push(ControlsCursor::FilterFloatAttachedMuxes);
        rows.push(ControlsCursor::FilterShowStoppedMuxes);
    }
    if matches!(ctx.view, View::Prs) {
        rows.push(ControlsCursor::FilterPrState);
        rows.push(ControlsCursor::FilterPrBranch);
        rows.push(ControlsCursor::FilterFloatTrackedPrs);
    }
    if matches!(ctx.view, View::Sessions | View::Mux | View::Prs) {
        rows.push(ControlsCursor::FilterPinnedOnly);
    }
    if matches!(ctx.view, View::History) {
        rows.push(ControlsCursor::FilterHistoryUnpinnedOnly);
    }
    rows.push(ControlsCursor::FilterClear);
    for idx in 0..SORT_OPTIONS.len() {
        rows.push(ControlsCursor::Sort(idx));
    }
    // Mux-only recency basis: which signal
    // `Sort::Recency` orders the mux tree by.
    if matches!(ctx.view, View::Mux) {
        for idx in 0..crate::tui::MuxRecency::ALL.len() {
            rows.push(ControlsCursor::MuxRecency(idx));
        }
    }
    for idx in 0..crate::tui::PreviewWrap::ALL.len() {
        rows.push(ControlsCursor::PreviewWrap(idx));
    }
    for idx in 0..crate::tui::SessionPreview::ALL.len() {
        rows.push(ControlsCursor::SessionPreview(idx));
    }
    rows
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Centered modal widget for the controls overlay. Borrows the
/// state plus a live context so the rendered values never lag
/// behind the app; the state is borrowed mutably so an open checklist
/// keeps the scroll window it drew.
pub struct ControlsOverlayWidget<'a> {
    state: &'a mut ControlsOverlayState,
    ctx: ControlsContext<'a>,
    theme: &'a Theme,
}

impl<'a> ControlsOverlayWidget<'a> {
    pub fn new(
        state: &'a mut ControlsOverlayState,
        ctx: ControlsContext<'a>,
        theme: &'a Theme,
    ) -> Self {
        Self { state, ctx, theme }
    }
}

impl Widget for ControlsOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frame = ModalFrame::new(self.theme, ModalSize::MENU).title("Controls");
        let (mut lines, cursor_line) = self.body();
        lines.push(Line::default());
        // The footer wraps one column inside the modal's inner width, so
        // no hint touches the border.
        let footer_width = usize::from(frame.inner_width(area)).saturating_sub(1);
        let footer = tuikit::keys::footer(crate::tui::keybindings::overlay::CONTROLS_KEYS);
        let separator = tuikit::keys::HINT_SEPARATOR;
        for chunk in tuikit::text::wrap_at(&footer, separator, footer_width) {
            lines.push(Line::from(span!(self.theme.muted; "{}", chunk.trim_end())));
        }
        // The body scrolls to keep the cursor's line in view when the
        // terminal is too short for every row.
        let body = match cursor_line {
            Some(line) => Lines::new(lines).following(line..line + 1),
            None => Lines::new(lines),
        };
        frame.render(area, buf, body);

        // If a sub-editor is open, render it on top of the overlay.
        if let Some(editor) = self.state.sub_editor.as_mut() {
            editor.draw(area, buf, self.theme);
        }
    }
}

impl ControlsOverlayWidget<'_> {
    #[cfg(test)]
    fn body_lines(&self) -> Vec<Line<'static>> {
        self.body().0
    }

    #[cfg(test)]
    fn cursor_line_index(&self) -> Option<usize> {
        self.body().1
    }

    /// The overlay body and the index of the line the cursor is on,
    /// recorded as rows are pushed so the two can't drift apart.
    fn body(&self) -> (Vec<Line<'static>>, Option<usize>) {
        let cursor = self.state.cursor();
        // Behind an open sub-editor the list is not the one taking keys.
        let focused = self.state.sub_editor().is_none();
        let mut body = Body::new(self.theme, focused);

        body.push(section_header("View"));
        for (idx, view) in VIEW_OPTIONS.iter().enumerate() {
            let row = ControlsCursor::View(idx);
            let active = *view == self.ctx.view;
            let label = format!("{} [{}]", view_label(*view), idx + 1);
            body.push_row(row_line(label, active), cursor == row);
        }
        body.push(line![""]);

        body.push(section_header(&format!(
            "Grouping ({})",
            view_label(self.ctx.view)
        )));
        for (idx, grouping) in Grouping::values_for(self.ctx.view).iter().enumerate() {
            let row = ControlsCursor::Grouping(idx);
            let active = *grouping == self.ctx.grouping;
            let label = grouping.as_str().to_string();
            body.push_row(row_line(label, active), cursor == row);
        }
        body.push(line![""]);

        if matches!(self.ctx.view, View::Search) {
            body.push(section_header("Results"));
            let here = cursor == ControlsCursor::ContentGroup;
            let value = crate::tui::app::grouping_description(self.ctx.content_group);
            body.push_row(cycle_row("listed as", value.to_string()), here);
            let here = cursor == ControlsCursor::SearchHistory;
            let value = match self.ctx.search_history {
                Some((position, total)) => format!("{position} of {total}"),
                None => "none yet".to_string(),
            };
            body.push_row(filter_row("searches", value), here);
            body.push(line![""]);
        }

        body.push(section_header("Search"));
        let here = cursor == ControlsCursor::SearchContents;
        body.push_row(filter_row("contents", "Enter searches".to_string()), here);
        let here = cursor == ControlsCursor::SearchOptions;
        body.push_row(filter_row("options", "Enter opens".to_string()), here);
        body.push(line![""]);

        body.push(section_header(&format!(
            "Filters ({})",
            view_label(self.ctx.view)
        )));
        let here = cursor == ControlsCursor::FilterHarness;
        body.push_row(filter_row("harness", harness_value(self.ctx.filter)), here);
        let here = cursor == ControlsCursor::FilterMaxAge;
        body.push_row(filter_row("max age", max_age_value(self.ctx.filter)), here);
        let here = cursor == ControlsCursor::FilterMuxState;
        body.push_row(
            filter_row("mux state", mux_state_value(self.ctx.filter)),
            here,
        );
        if matches!(
            self.ctx.view,
            View::Sessions | View::Mux | View::Prs | View::Search
        ) {
            let here = cursor == ControlsCursor::FilterContent;
            body.push_row(
                filter_row("contains", content_value(self.ctx.content)),
                here,
            );
        }
        if matches!(self.ctx.view, View::Sessions) {
            let here = cursor == ControlsCursor::FilterFloatMuxedSessions;
            body.push_row(
                checkbox_row(
                    "Float muxed sessions to top",
                    self.ctx.filter.float_muxed_sessions_top,
                ),
                here,
            );
        }
        if matches!(self.ctx.view, View::Mux) {
            let here = cursor == ControlsCursor::FilterFloatAttachedMuxes;
            body.push_row(
                checkbox_row(
                    "Float attached muxes to top",
                    self.ctx.filter.float_attached_muxes_top,
                ),
                here,
            );
            let here = cursor == ControlsCursor::FilterShowStoppedMuxes;
            body.push_row(
                checkbox_row("Show all stopped muxes", self.ctx.filter.show_stopped_muxes),
                here,
            );
        }
        if matches!(self.ctx.view, View::Prs) {
            let here = cursor == ControlsCursor::FilterPrState;
            body.push_row(
                filter_row("PR state", pr_state_value(self.ctx.filter)),
                here,
            );
            let here = cursor == ControlsCursor::FilterPrBranch;
            body.push_row(
                cycle_row("local branch", pr_branch_value(self.ctx.filter)),
                here,
            );
            let here = cursor == ControlsCursor::FilterFloatTrackedPrs;
            body.push_row(
                checkbox_row(
                    "Float PRs with a local branch to top",
                    self.ctx.filter.float_tracked_prs_top,
                ),
                here,
            );
        }
        if matches!(self.ctx.view, View::Sessions | View::Mux | View::Prs) {
            let here = cursor == ControlsCursor::FilterPinnedOnly;
            body.push_row(
                checkbox_row("Pinned only", self.ctx.filter.pinned_only),
                here,
            );
        }
        if matches!(self.ctx.view, View::History) {
            let here = cursor == ControlsCursor::FilterHistoryUnpinnedOnly;
            body.push_row(
                checkbox_row("Unpinned only", !self.ctx.filter.show_pinned_history),
                here,
            );
        }
        let here = cursor == ControlsCursor::FilterClear;
        body.push_row(row_line("Clear all filters".to_string(), false), here);
        body.push(line![""]);

        body.push(section_header("Sort"));
        for (idx, sort) in SORT_OPTIONS.iter().enumerate() {
            let row = ControlsCursor::Sort(idx);
            let active = *sort == self.ctx.sort;
            body.push_row(
                row_line(sort_label(*sort).to_string(), active),
                cursor == row,
            );
        }

        // Mux-only recency basis: which signal the
        // Recency sort orders by. Only meaningful — and only shown —
        // for the mux view.
        if matches!(self.ctx.view, View::Mux) {
            body.push(line![""]);
            body.push(section_header("Recency by (Mux)"));
            for (idx, basis) in crate::tui::MuxRecency::ALL.iter().enumerate() {
                let row = ControlsCursor::MuxRecency(idx);
                let active = *basis == self.ctx.mux_recency;
                body.push_row(row_line(basis.label().to_string(), active), cursor == row);
            }
        }

        body.push(line![""]);
        body.push(section_header("Preview wrap"));
        for (idx, wrap) in crate::tui::PreviewWrap::ALL.iter().enumerate() {
            let row = ControlsCursor::PreviewWrap(idx);
            let active = *wrap == self.ctx.preview_wrap;
            body.push_row(row_line(wrap.label().to_string(), active), cursor == row);
        }

        body.push(line![""]);
        body.push(section_header("Session preview"));
        for (idx, choice) in crate::tui::SessionPreview::ALL.iter().enumerate() {
            let row = ControlsCursor::SessionPreview(idx);
            let active = *choice == self.ctx.session_preview;
            body.push_row(row_line(choice.label().to_string(), active), cursor == row);
        }
        (body.lines, body.cursor_line)
    }
}

/// Overlay lines under construction, plus where the cursor row landed.
struct Body<'t> {
    lines: Vec<Line<'static>>,
    cursor_line: Option<usize>,
    theme: &'t Theme,
    /// Whether the list has focus: its cursor row takes the active look.
    focused: bool,
}

impl<'t> Body<'t> {
    fn new(theme: &'t Theme, focused: bool) -> Self {
        Self {
            lines: Vec::new(),
            cursor_line: None,
            theme,
            focused,
        }
    }

    fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
    }

    /// A row the cursor can land on, behind the cursor gutter.
    fn push_row(&mut self, spans: Vec<Span<'static>>, cursored: bool) {
        if cursored {
            self.cursor_line = Some(self.lines.len());
        }
        let cursor = Cursor::of(cursored, self.focused);
        self.lines.push(selection::row(spans, cursor, self.theme));
    }
}

fn section_header(label: &str) -> Line<'static> {
    line![span!(Modifier::BOLD; "{label}")]
}

/// A choice row; the active choice is bold and says so.
fn row_line(label: String, active: bool) -> Vec<Span<'static>> {
    let style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let active_tag = if active { "  active" } else { "" };
    vec![span!(style; "{label}{active_tag}")]
}

fn filter_row(name: &str, value: String) -> Vec<Span<'static>> {
    vec![span!("{name:12} {value}  [ Edit ]")]
}

/// A filter row whose value Space steps to the next choice rather
/// than opening an editor.
fn cycle_row(name: &str, value: String) -> Vec<Span<'static>> {
    vec![span!("{name:12} {value}  [ Cycle ]")]
}

fn pr_state_value(filter: &RowFilter) -> String {
    match &filter.pr_state {
        Some(PrStateFilter::Any(values)) if !values.is_empty() => values
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

fn pr_branch_value(filter: &RowFilter) -> String {
    filter
        .pr_branch
        .map_or_else(|| "any".to_string(), |value| value.as_str().to_string())
}

fn checkbox_row(label: &str, checked: bool) -> Vec<Span<'static>> {
    let style = if checked {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let box_glyph = if checked { "[x]" } else { "[ ]" };
    vec![span!(style; "{box_glyph} {label}")]
}

fn harness_value(filter: &RowFilter) -> String {
    match &filter.harness {
        Some(HarnessFilter::Any(values)) if !values.is_empty() => values.join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

/// How the Content row reads: the query and where its scan is.
fn content_value(content: Option<(&str, crate::tui::ContentStatus, usize)>) -> String {
    match content {
        None => marks::NO_VALUE.to_string(),
        Some((text, crate::tui::ContentStatus::Running(progress), _)) => {
            format!(
                "\"{text}\" searching {}/{}",
                progress.scanned, progress.total
            )
        }
        Some((text, crate::tui::ContentStatus::Done(_), found)) => {
            format!(
                "\"{text}\" {found} {}",
                if found == 1 { "session" } else { "sessions" }
            )
        }
    }
}

fn max_age_value(filter: &RowFilter) -> String {
    filter
        .max_age
        .map_or_else(|| marks::NO_VALUE.to_string(), format_duration_for_input)
}

fn mux_state_value(filter: &RowFilter) -> String {
    match &filter.mux_state {
        Some(MuxStateFilter::Any(values)) if !values.is_empty() => values
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

pub(crate) fn view_label(view: View) -> &'static str {
    match view {
        View::Sessions => "Sessions",
        View::Mux => "Mux",
        View::Prs => "PRs",
        View::Pinned => "Pinned",
        View::History => "History",
        View::Search => "Search",
    }
}

fn sort_label(sort: Sort) -> &'static str {
    match sort {
        Sort::Hierarchy => "Hierarchy",
        Sort::Recency => "Recency",
    }
}

#[cfg(test)]
#[path = "controls_tests.rs"]
mod tests;
