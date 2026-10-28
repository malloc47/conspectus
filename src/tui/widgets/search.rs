//! The search overlay: one wide modal with two modes.
//!
//! Quick select (`/`) picks an entity by name. It looks at the current
//! view's rows by default and widens to every entity in the snapshot
//! with Tab. Content search looks inside what was said.
//!
//! Reuses the ADR 0030 text-input primitive for the query line and
//! displays matches ranked by [`crate::tui::search::rank`] (rows) or
//! [`crate::tui::search_entities::EntityIndex::rank`] (entities).
//!
//! Per ADR 0031 and 0149 the current scope ranks every row of the active
//! view after filters, collapsed rows included: the host hands the overlay
//! those rows as [`SearchItem`]s.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::model::NodeId;
use crate::tui::Theme;
use crate::tui::search_entities::{EntityIndex, EntityMatch, SearchEntity};

use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::rows::RowId;
use crate::tui::search::{SearchItem, SearchMatch, rank, snippet_around};
use crate::tui::widgets::input::TextInputState;
use crate::viewer::search::{ContentKinds, Grouping};

/// How long the query rests before a contents scan starts, so typing a
/// word does not start a scan per letter.
pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// Why contents search cannot be chosen when live preview is off.
pub const CONTENT_SEARCH_OFF: &str = "Content search is off while live preview is disabled";

/// What the overlay is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    /// Pick an entity by name.
    #[default]
    Quick,
    /// Look inside what was said in the sessions.
    Content,
}

/// How far quick select looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reach {
    /// The rows of the current view.
    #[default]
    Current,
    /// Every entity, whichever view it lives in.
    All,
}

impl Reach {
    pub fn other(self) -> Self {
        match self {
            Self::Current => Self::All,
            Self::All => Self::Current,
        }
    }
}

/// The least and most the modal's width and height grow to, and the share
/// of the terminal it aims for. The modal always leaves a one-cell margin.
const MODAL_WIDTH: (u32, u32, u32) = (85, 60, 200);
const MODAL_HEIGHT: (u32, u32, u32) = (80, 12, 50);

/// The modal's outer size on a terminal of `width` x `height` cells.
pub fn modal_size(width: u16, height: u16) -> (u16, u16) {
    let fit = |terminal: u16, (percent, min, max): (u32, u32, u32)| -> u16 {
        let wanted = (u32::from(terminal) * percent / 100).clamp(min, max);
        u16::try_from(wanted)
            .unwrap_or(u16::MAX)
            .min(terminal.saturating_sub(2))
            .max(1)
    };
    (fit(width, MODAL_WIDTH), fit(height, MODAL_HEIGHT))
}

/// What a content result opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentTarget {
    /// A session's transcript, at the turn the search found.
    Session {
        session: crate::model::AgentSessionId,
        turn_index: usize,
    },
    /// A mux pane's scrollback, at the line the search found.
    Pane {
        pane: crate::content_search::PaneRef,
        line: usize,
    },
}

impl ContentTarget {
    /// The word a result row carries for where it was found.
    pub fn source_word(&self) -> &'static str {
        match self {
            Self::Session { .. } => "session",
            Self::Pane { .. } => "pane",
        }
    }
}

/// One result in the contents scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRow {
    pub target: ContentTarget,
    pub label: String,
    /// The harness's name drawn as a chip, or the command a pane runs.
    pub chip: String,
    /// Matches in the session or pane.
    pub hits: usize,
    /// Per-message results say which turn or line this row is.
    pub turn: Option<String>,
    /// How long ago the session was last active, or the pane was read.
    pub age: String,
    pub snippet: String,
    /// Byte range of the match inside `snippet`.
    pub highlight: Option<std::ops::Range<usize>>,
}

/// What the host tells the widget about the running contents scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentView {
    pub rows: Vec<ContentRow>,
    pub scanned: usize,
    pub total: usize,
    pub skipped: usize,
    pub done: bool,
    /// The scan reads pane scrollback as well as transcripts.
    pub with_panes: bool,
    /// The current context lists no sessions or muxes to search.
    pub empty_scope: bool,
    /// Set when contents search cannot run, and why.
    pub unavailable: Option<&'static str>,
}

/// What the overlay wants the host to do about the contents scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DueSearch {
    Preview {
        text: String,
        kinds: ContentKinds,
        group: Grouping,
        reach: Reach,
    },
    /// Stop previewing.
    Clear,
}

/// Pure state for the search overlay. Holds the query text input,
/// the latest match list (recomputed by the host whenever the
/// query changes), and the cursor inside that list.
#[derive(Debug, Clone)]
pub struct SearchOverlayState {
    input: TextInputState,
    matches: Vec<SearchMatch>,
    cursor: usize,
    mode: SearchMode,
    reach: Reach,
    /// Every entity, built the first time the wider reach is chosen.
    entities: Option<EntityIndex>,
    entity_matches: Vec<EntityMatch>,
    group: Grouping,
    kinds: ContentKinds,
    /// Whether the search reads pane scrollback as well as transcripts.
    panes: bool,
    /// The options pane is open, with its cursor on this option.
    options: Option<usize>,
    /// Whether the contents scope can run (live preview is on).
    contents_enabled: bool,
    /// Results in the contents scope, set by the host before a key.
    content_len: usize,
    /// When the query or options last changed and a scan is owed.
    edited_at: Option<Instant>,
    /// Earlier searches, newest first, for paging back through.
    past: Vec<PastSearch>,
    /// The earlier search the field shows; `None` for the operator's own.
    past_at: Option<usize>,
}

/// An earlier content search the field can be filled from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastSearch {
    pub text: String,
    pub reach: Reach,
    pub kinds: ContentKinds,
    pub panes: bool,
}

impl SearchOverlayState {
    pub fn new() -> Self {
        Self {
            input: TextInputState::new(" search ", String::new()),
            matches: Vec::new(),
            cursor: 0,
            mode: SearchMode::Quick,
            reach: Reach::Current,
            entities: None,
            entity_matches: Vec::new(),
            group: Grouping::default(),
            kinds: ContentKinds::default(),
            panes: true,
            options: None,
            contents_enabled: true,
            content_len: 0,
            edited_at: None,
            past: Vec::new(),
            past_at: None,
        }
    }

    /// An overlay set up the way the operator left it, or with a search
    /// already active: `query` fills the field, and a non-empty one in
    /// content mode is searched at once. It always opens on the current
    /// reach.
    pub fn configured(
        mode: SearchMode,
        group: Grouping,
        kinds: ContentKinds,
        contents_enabled: bool,
        query: &str,
    ) -> Self {
        let mut state = Self::new();
        state.input = TextInputState::new(" search ", query.to_string());
        state.group = group.granularity();
        state.kinds = kinds;
        state.contents_enabled = contents_enabled;
        if mode == SearchMode::Content && contents_enabled {
            state.mode = SearchMode::Content;
        }
        state
    }

    /// Open on `reach` instead of the current context.
    #[must_use]
    pub fn with_reach(mut self, reach: Reach) -> Self {
        self.reach = reach;
        self
    }

    /// Open with `past` (newest first) available to page back through.
    #[must_use]
    pub fn with_past(mut self, past: Vec<PastSearch>) -> Self {
        self.past = past;
        self
    }

    /// Open with the pane scrollback option set as given.
    #[must_use]
    pub fn with_panes(mut self, panes: bool) -> Self {
        self.panes = panes;
        self
    }

    /// Turn off pane scrollback, as the options pane would.
    #[cfg(test)]
    pub fn with_panes_off(&mut self) {
        self.panes = false;
    }

    /// Open with the options pane showing.
    #[must_use]
    pub fn with_options_open(mut self) -> Self {
        self.options = Some(0);
        self
    }

    /// Whether pane scrollback is searched.
    pub fn panes(&self) -> bool {
        self.panes
    }

    /// The options pane's cursor, when the pane is open.
    pub fn options_cursor(&self) -> Option<usize> {
        self.options
    }

    pub fn mode(&self) -> SearchMode {
        self.mode
    }

    pub fn reach(&self) -> Reach {
        self.reach
    }

    /// Whether the entity index is wanted but not built yet.
    pub fn needs_entities(&self) -> bool {
        self.mode == SearchMode::Quick && self.reach == Reach::All && self.entities.is_none()
    }

    /// Whether `event` is the key that widens a quick select that has not
    /// built its entity index yet, so the host can build it first.
    pub fn widens_with(&self, event: KeyEvent) -> bool {
        use crate::tui::keybindings::overlay::{SEARCH_KEYS, SearchKey, resolve};
        self.mode == SearchMode::Quick
            && self.reach == Reach::Current
            && self.entities.is_none()
            && resolve(SEARCH_KEYS, event) == Some(SearchKey::Scope)
    }

    /// Entities matching the query, best first (the wider reach).
    pub fn entity_matches(&self) -> &[EntityMatch] {
        &self.entity_matches
    }

    /// The entity behind a match.
    pub fn entity(&self, hit: &EntityMatch) -> Option<&SearchEntity> {
        self.entities.as_ref()?.get(hit.index)
    }

    pub fn group(&self) -> Grouping {
        self.group
    }

    pub fn kinds(&self) -> ContentKinds {
        self.kinds
    }

    /// The host reports how many contents results there are, so the
    /// cursor can move among them.
    pub fn set_content_len(&mut self, len: usize) {
        self.content_len = len;
        // The cursor belongs to the contents results only in content
        // mode; quick select keeps its own and the host passes no
        // contents then, which must not send it back to the top.
        if self.mode == SearchMode::Content && self.cursor >= len {
            self.cursor = 0;
        }
    }

    /// Owe a contents scan for the current query, as soon as the host
    /// asks (`force`) or after the debounce.
    pub fn mark_edited(&mut self, at: Instant) {
        self.edited_at = Some(at);
    }

    /// The scan the overlay is owed once the query has rested, if any.
    /// `Clear` says to stop previewing (rows scope, or an empty query).
    pub fn take_due_search(&mut self, now: Instant, force: bool) -> Option<DueSearch> {
        let at = self.edited_at?;
        if !force && now.duration_since(at) < DEBOUNCE {
            return None;
        }
        self.edited_at = None;
        let text = self.input.value().trim().to_string();
        if self.mode == SearchMode::Content && !text.is_empty() {
            Some(DueSearch::Preview {
                text,
                kinds: self.kinds,
                group: self.group,
                reach: self.reach,
            })
        } else {
            Some(DueSearch::Clear)
        }
    }

    /// The query as the contents scan will see it.
    pub fn content_text(&self) -> String {
        self.input.value().trim().to_string()
    }

    /// Current query text (post-edit, pre-rank).
    pub fn query(&self) -> &str {
        self.input.value()
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Refresh the match list against `items` using `backend`. The
    /// host calls this after every key dispatch so the operator
    /// sees the list shrink/grow as they type. Resets the cursor
    /// to 0 so a newly typed character doesn't strand it past the
    /// end of the new result list.
    pub fn refresh_matches(&mut self, items: &[SearchItem<'_>]) {
        self.matches = if self.mode == SearchMode::Quick && self.reach == Reach::Current {
            rank(self.input.value(), items)
        } else {
            Vec::new()
        };
        self.entity_matches = match (&self.entities, self.mode, self.reach) {
            (Some(index), SearchMode::Quick, Reach::All) => index.rank(self.input.value()),
            _ => Vec::new(),
        };
        // The cursor belongs to the contents results in content mode.
        if self.mode == SearchMode::Quick && self.cursor >= self.result_len() {
            self.cursor = 0;
        }
    }

    /// Hand the overlay the entity index the wider reach searches.
    pub fn set_entities(&mut self, entities: EntityIndex) {
        self.entities.get_or_insert(entities);
    }

    fn result_len(&self) -> usize {
        match (self.mode, self.reach) {
            (SearchMode::Quick, Reach::Current) => self.matches.len(),
            (SearchMode::Quick, Reach::All) => self.entity_matches.len(),
            (SearchMode::Content, _) => self.content_len,
        }
    }
}

impl Default for SearchOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// What the host should do after passing a key event through the
/// overlay. `Continue` keeps it open; `Cancel` closes without a
/// selection change; `Confirm(id)` closes and asks the host to
/// move the selection to `id`. The id is boxed so the enum stays
/// small (RowId carries a NodeId that can be large for sessions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOutcome {
    Continue,
    Cancel,
    Confirm(Box<RowId>),
    /// Wider reach: open the entity under the cursor wherever it lives.
    OpenEntity(Box<NodeId>),
    /// Content mode: open the result under the cursor.
    OpenHit(usize),
    /// Content mode: keep the query as the filter on the lists.
    ApplyFilter,
    /// Content mode: keep the search in the Search view and go there.
    Keep,
    /// Content mode: list the same results at another granularity.
    Regroup(Grouping),
}

/// The options the pane lists, in order.
pub const SEARCH_OPTIONS: [&str; 3] = ["Tool calls and output", "Thinking", "Pane scrollback"];

impl SearchOverlayState {
    /// Dispatch a crossterm key event.
    ///
    /// - `Esc` / `Ctrl-C`: cancel.
    /// - `Enter`: confirm with the cursor's match, or cancel if no
    ///   matches are present.
    /// - `Down` / `Ctrl-N`: move cursor down (wraps).
    /// - `Up` / `Ctrl-P`: move cursor up (wraps).
    /// - Everything else: passed through to the text input. We do
    ///   not intercept `j` / `k` here because typing them into the
    ///   query is the common case; result-list navigation uses
    ///   arrow keys + ctrl-N/P instead.
    pub fn handle_key(&mut self, event: KeyEvent) -> SearchOutcome {
        use crate::tui::keybindings::overlay::{
            SEARCH_CONTENT_KEYS, SEARCH_KEYS, SearchKey, resolve,
        };
        let table = match self.mode {
            SearchMode::Quick => SEARCH_KEYS,
            SearchMode::Content => SEARCH_CONTENT_KEYS,
        };
        let contents = self.mode == SearchMode::Content;
        if contents && self.options.is_some() {
            return self.handle_options_key(event);
        }
        match resolve(table, event) {
            Some(SearchKey::Abort | SearchKey::Cancel) => SearchOutcome::Cancel,
            Some(SearchKey::Pick) if contents => {
                if self.content_len == 0 {
                    SearchOutcome::Cancel
                } else {
                    SearchOutcome::OpenHit(self.cursor)
                }
            }
            Some(SearchKey::Pick) if self.reach == Reach::All => {
                match self.entity_matches.get(self.cursor) {
                    Some(hit) => match self.entity(hit) {
                        Some(entity) => SearchOutcome::OpenEntity(Box::new(entity.node.clone())),
                        None => SearchOutcome::Cancel,
                    },
                    None => SearchOutcome::Cancel,
                }
            }
            Some(SearchKey::Pick) => match self.matches.get(self.cursor) {
                Some(m) => SearchOutcome::Confirm(Box::new(m.id.clone())),
                None => SearchOutcome::Cancel,
            },
            Some(SearchKey::Down) => {
                self.move_cursor(1);
                SearchOutcome::Continue
            }
            Some(SearchKey::Up) => {
                self.move_cursor(-1);
                SearchOutcome::Continue
            }
            Some(SearchKey::Scope) => {
                self.cursor = 0;
                self.reach = self.reach.other();
                if contents {
                    self.edited_at = Some(Instant::now());
                }
                SearchOutcome::Continue
            }
            Some(SearchKey::Granularity) => {
                // Listing the same matches another way is not a new search.
                self.group = match self.group.granularity() {
                    Grouping::Session => Grouping::Flat,
                    _ => Grouping::Session,
                };
                self.cursor = 0;
                SearchOutcome::Regroup(self.group)
            }
            Some(SearchKey::Options) => {
                self.options = Some(0);
                SearchOutcome::Continue
            }
            Some(SearchKey::ApplyFilter) => self.promote(SearchOutcome::ApplyFilter),
            Some(SearchKey::Keep) => self.promote(SearchOutcome::Keep),
            Some(SearchKey::PrevQuery) => self.page_past(1),
            Some(SearchKey::NextQuery) => self.page_past(-1),
            None => {
                // Let the text-input primitive handle the key. We
                // ignore its outcome here because Enter/Esc are
                // intercepted above; everything else is "still
                // editing the query."
                let before = self.input.value().to_string();
                let _ = self.input.handle_key(event);
                if contents && self.input.value() != before {
                    self.cursor = 0;
                    self.edited_at = Some(Instant::now());
                }
                SearchOutcome::Continue
            }
        }
    }

    /// A key while the options pane is open.
    fn handle_options_key(&mut self, event: KeyEvent) -> SearchOutcome {
        use crate::tui::keybindings::overlay::{SEARCH_OPTION_KEYS, SearchOptionKey, resolve};
        let at = self.options.unwrap_or(0);
        match resolve(SEARCH_OPTION_KEYS, event) {
            Some(SearchOptionKey::Abort) => return SearchOutcome::Cancel,
            Some(SearchOptionKey::Back) => self.options = None,
            Some(SearchOptionKey::Down) => {
                self.options = Some(crate::tui::cursor::wrap_step(at, SEARCH_OPTIONS.len(), 1));
            }
            Some(SearchOptionKey::Up) => {
                self.options = Some(crate::tui::cursor::wrap_step(at, SEARCH_OPTIONS.len(), -1));
            }
            Some(SearchOptionKey::Toggle) => {
                match at {
                    0 => self.kinds.tools = !self.kinds.tools,
                    1 => self.kinds.thinking = !self.kinds.thinking,
                    _ => self.panes = !self.panes,
                }
                self.cursor = 0;
                self.edited_at = Some(Instant::now());
            }
            None => {}
        }
        SearchOutcome::Continue
    }

    /// Fill the field from the earlier search `delta` places back (negative
    /// is newer; past the newest the field empties). It stops at the
    /// oldest, and a changed field is searched like any edit.
    fn page_past(&mut self, delta: isize) -> SearchOutcome {
        let next = match self.past_at {
            None if delta > 0 => 0,
            None => return SearchOutcome::Continue,
            Some(at) => match at.checked_add_signed(delta) {
                Some(next) if next < self.past.len() => next,
                Some(_) => return SearchOutcome::Continue,
                None => {
                    self.past_at = None;
                    return self.fill_query(String::new());
                }
            },
        };
        let Some(past) = self.past.get(next).cloned() else {
            return SearchOutcome::Continue;
        };
        self.past_at = Some(next);
        self.reach = past.reach;
        self.kinds = past.kinds;
        self.panes = past.panes;
        self.fill_query(past.text)
    }

    fn fill_query(&mut self, text: String) -> SearchOutcome {
        self.input = TextInputState::new(" search ", text);
        self.cursor = 0;
        self.edited_at = Some(Instant::now());
        SearchOutcome::Continue
    }

    /// `outcome` for a query worth keeping; nothing for an empty one.
    fn promote(&self, outcome: SearchOutcome) -> SearchOutcome {
        if self.content_text().is_empty() {
            SearchOutcome::Continue
        } else {
            outcome
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = self.result_len();
        if len == 0 {
            self.cursor = 0;
            return;
        }
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, len, delta);
    }
}

/// What a result's outcome word says, for the host to compute per row.
pub type OutcomeWords<'a> = &'a dyn Fn(&NodeId) -> String;

/// Centered modal that renders the query line on top and a
/// ranked match list below. Caps the visible match list at the
/// available height; the cursor scrolls within it.
pub struct SearchOverlayWidget<'a> {
    state: &'a SearchOverlayState,
    items: &'a [SearchItem<'a>],
    theme: &'a Theme,
    content: Option<&'a ContentView>,
    outcomes: Option<OutcomeWords<'a>>,
}

impl<'a> SearchOverlayWidget<'a> {
    pub fn new(
        state: &'a SearchOverlayState,
        items: &'a [SearchItem<'a>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            state,
            items,
            theme,
            content: None,
            outcomes: None,
        }
    }

    /// What content mode shows; ignored in quick select.
    #[must_use]
    pub fn with_content(mut self, content: &'a ContentView) -> Self {
        self.content = Some(content);
        self
    }

    /// The word that says what opening an entity does, for the wider reach.
    #[must_use]
    pub fn with_outcomes(mut self, outcomes: OutcomeWords<'a>) -> Self {
        self.outcomes = Some(outcomes);
        self
    }
}

/// The reach choices as a title segment: the chosen one in brackets, so
/// the title is as wide whichever is chosen.
fn reach_segments(reach: Reach, theme: &Theme) -> Vec<Span<'static>> {
    choice_segments(
        [
            (reach == Reach::Current, "Current"),
            (reach == Reach::All, "All"),
        ],
        theme,
    )
}

/// The granularity choices as a title segment, the same way.
fn granularity_segments(group: Grouping, theme: &Theme) -> Vec<Span<'static>> {
    let sessions = group.granularity() == Grouping::Session;
    choice_segments([(sessions, "Sessions"), (!sessions, "Messages")], theme)
}

fn choice_segments(choices: [(bool, &'static str); 2], theme: &Theme) -> Vec<Span<'static>> {
    let chosen = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD | Modifier::REVERSED);
    let mut spans = Vec::new();
    for (i, (on, word)) in choices.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        if on {
            spans.push(Span::styled(format!("[ {word} ]"), chosen));
        } else {
            spans.push(Span::raw(word));
        }
    }
    spans
}

impl Widget for SearchOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`; the body wrapper reports the
        // inner dims of the rect `centered_modal_rect` produces so
        // auto-sizing reproduces it.
        let modal = centered_modal_rect(area);
        let title = match (self.state.mode(), self.content) {
            (SearchMode::Content, _) => {
                let mut spans = vec![Span::raw(" / search contents  ")];
                spans.extend(reach_segments(self.state.reach(), self.theme));
                spans.push(Span::raw("  "));
                spans.extend(granularity_segments(self.state.group(), self.theme));
                spans.push(Span::raw(" "));
                Line::from(spans)
            }
            (SearchMode::Quick, _) => {
                let n = match self.state.reach() {
                    Reach::Current => self.state.matches().len(),
                    Reach::All => self.state.entity_matches().len(),
                };
                let mut spans = vec![Span::raw(" / quick select  ")];
                spans.extend(reach_segments(self.state.reach(), self.theme));
                spans.push(Span::raw(format!("  ({n}) ")));
                Line::from(spans)
            }
        };
        let body = SearchBody {
            state: self.state,
            items: self.items,
            content: self.content,
            outcomes: self.outcomes,
            theme: self.theme,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let popup = crate::tui::widgets::popup_frame::themed_popup(body, title, self.theme);
        popup.render(area, buf);
    }
}

/// Body wrapper for `tui_popup::Popup`. Renders the query row +
/// ranked match list into the inner area; sizing is reported via
/// the cap dims so the popup's auto-sizing reproduces the in-tree
/// rect.
struct SearchBody<'a> {
    state: &'a SearchOverlayState,
    items: &'a [SearchItem<'a>],
    content: Option<&'a ContentView>,
    outcomes: Option<OutcomeWords<'a>>,
    theme: &'a Theme,
    inner_width: usize,
    inner_height: usize,
}

impl KnownSize for SearchBody<'_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

/// Widths of the columns in an entity result, left to right after the
/// two-cell cursor mark and two-cell kind glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntityColumns {
    pub label: usize,
    pub context: usize,
    pub outcome: usize,
}

/// The widest an outcome word gets (`in Sessions view`).
const OUTCOME_WIDTH: usize = 16;
/// Cursor mark plus kind glyph.
const ROW_LEAD: usize = 4;
const COLUMN_GAP: usize = 2;
/// The label column stops growing here, so a wide modal gives the room to
/// the context instead of padding.
const LABEL_CAP: usize = 48;
/// A context narrower than this is not worth showing.
const MIN_CONTEXT: usize = 10;

/// Split a row `inner` cells wide into columns. The outcome word keeps its
/// width; the label takes up to 45% of the rest (at most [`LABEL_CAP`]) and
/// the context what is left, dropped when too narrow to read.
pub(crate) fn entity_columns(inner: usize) -> EntityColumns {
    let outcome = OUTCOME_WIDTH.min(inner / 3);
    let body = inner.saturating_sub(ROW_LEAD + outcome + COLUMN_GAP);
    let label = (body * 45 / 100).clamp(16, LABEL_CAP).min(body);
    let context = body.saturating_sub(label + COLUMN_GAP);
    if context < MIN_CONTEXT {
        EntityColumns {
            label: body,
            context: 0,
            outcome,
        }
    } else {
        EntityColumns {
            label,
            context,
            outcome,
        }
    }
}

impl Widget for SearchBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // First row: the query input (rendered as plain text with a
        // leading `/` so the operator sees the live query). Reuse
        // TextInputState's value rather than instantiating its
        // widget, since we want the query and the result list in a
        // single bordered modal.
        let query_line = line![
            span!(Style::default().fg(self.theme.accent); "/"),
            self.state.query().to_string(),
        ];
        let query_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(query_line).render(query_area, buf);

        if self.state.mode() == SearchMode::Content {
            self.render_contents(area, buf);
            return;
        }

        // Footer hints at the bottom, then the ranked list.
        let hint_rows = self.render_footer(
            area,
            buf,
            &crate::tui::keybindings::overlay::footer(
                crate::tui::keybindings::overlay::SEARCH_KEYS,
            ),
        );
        let list_area = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: area.height.saturating_sub(1).saturating_sub(hint_rows),
        };
        let total = match self.state.reach() {
            Reach::Current => self.state.matches().len(),
            Reach::All => self.state.entity_matches().len(),
        };
        if total == 0 {
            let label = if self.state.query().is_empty() {
                "(start typing)"
            } else {
                "(no matches)"
            };
            Paragraph::new(line![span!(self.theme.muted; "{label}")]).render(list_area, buf);
            return;
        }

        let visible_rows = list_area.height as usize;
        let scroll = compute_scroll(self.state.cursor(), visible_rows, total);
        let range = scroll..(scroll + visible_rows).min(total);
        let lines: Vec<Line<'static>> = match self.state.reach() {
            Reach::All => range
                .map(|idx| self.entity_line(idx, list_area.width as usize))
                .collect(),
            Reach::Current => {
                let id_to_item: std::collections::HashMap<&RowId, &SearchItem<'_>> =
                    self.items.iter().map(|item| (&item.id, item)).collect();
                // Budget the snippet width against the modal so the line
                // (label + separator + snippet) fits without wrapping; the
                // label itself can be wide for verbose aliases, so the
                // snippet always reserves at least ~24 chars.
                let snippet_budget = (list_area.width as usize)
                    .saturating_sub(8 /* prefix + label spacer + ellipsis */)
                    .max(24);
                range
                    .map(|idx| {
                        let m = &self.state.matches()[idx];
                        let item = id_to_item.get(&m.id);
                        let label =
                            item.map_or_else(|| "<missing>".to_string(), |i| i.label.to_string());
                        build_match_line(
                            label,
                            item.copied(),
                            m,
                            idx == self.state.cursor(),
                            snippet_budget,
                            self.theme,
                        )
                    })
                    .collect()
            }
        };
        Paragraph::new(lines).render(list_area, buf);
    }
}

impl SearchBody<'_> {
    /// One entity result: cursor mark, kind glyph, label, context, and the
    /// outcome word at the right edge.
    fn entity_line(&self, idx: usize, width: usize) -> Line<'static> {
        use crate::tui::widgets::popup_frame::ellipsize;

        let is_cursor = idx == self.state.cursor();
        let Some(entity) = self
            .state
            .entity_matches()
            .get(idx)
            .and_then(|hit| self.state.entity(hit).map(|e| (e, hit)))
        else {
            return Line::default();
        };
        let (entity, hit) = entity;
        // Leave a blank cell before the frame.
        let width = width.saturating_sub(1);
        let columns = entity_columns(width);
        let mark = if is_cursor {
            Modifier::REVERSED
        } else {
            Modifier::empty()
        };
        let plain = Style::default().add_modifier(mark);
        let dim = if is_cursor || entity.gone {
            plain.add_modifier(if entity.gone && !is_cursor {
                self.theme.muted
            } else {
                Modifier::empty()
            })
        } else {
            Style::default().add_modifier(self.theme.muted)
        };
        let label = ellipsize(&entity.label, columns.label);
        let label_width = cells(&label);
        let mut spans: Vec<Span<'static>> = vec![
            span!(plain; "{}", if is_cursor { "> " } else { "  " }),
            search_kind_glyph_span_for(NodeKind::from(&entity.node), self.theme, is_cursor),
        ];
        let label_style = if entity.gone && !is_cursor {
            dim
        } else {
            plain
        };
        match hit.label_range.clone().filter(|r| r.end <= label.len()) {
            Some(range)
                if label.is_char_boundary(range.start) && label.is_char_boundary(range.end) =>
            {
                spans.push(span!(label_style; "{}", &label[..range.start]));
                spans.push(
                    span!(self.theme.match_style().add_modifier(mark); "{}", &label[range.clone()]),
                );
                spans.push(span!(label_style; "{}", &label[range.end..]));
            }
            _ => spans.push(span!(label_style; "{label}")),
        }
        let mut used = ROW_LEAD + label_width;
        if columns.context > 0 {
            let pad = columns.label.saturating_sub(label_width) + COLUMN_GAP;
            let context = ellipsize(&entity.context, columns.context);
            used += pad + cells(&context);
            spans.push(span!(plain; "{}", " ".repeat(pad)));
            spans.push(span!(dim; "{context}"));
        }
        let word = self
            .outcomes
            .map(|outcomes| outcomes(&entity.node))
            .unwrap_or_default();
        let word = ellipsize(&word, columns.outcome);
        let word_width = cells(&word);
        let gap = width.saturating_sub(used + word_width);
        spans.push(span!(plain; "{}", " ".repeat(gap)));
        spans.push(span!(dim; "{word}"));
        Line::from(spans)
    }
}

impl SearchBody<'_> {
    /// The contents scope: scan status, results, and the key hints.
    fn render_contents(&self, area: Rect, buf: &mut Buffer) {
        use crate::tui::keybindings::overlay::{SEARCH_CONTENT_KEYS, footer};
        use crate::tui::widgets::popup_frame::ellipsize;

        let width = area.width as usize;
        let empty = ContentView::default();
        let view = self.content.unwrap_or(&empty);

        let options = self.state.options_cursor();
        let hints = match options {
            Some(_) => footer(crate::tui::keybindings::overlay::SEARCH_OPTION_KEYS),
            None => footer(SEARCH_CONTENT_KEYS),
        };
        let hint_rows = self.render_footer(area, buf, &hints);
        let hint_top = area.y + area.height - hint_rows;

        // Status row: what is searched and how far the scan has got.
        let status = ellipsize(&self.status_text(view), width);
        let status_rect = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: 1,
        };
        Paragraph::new(line![span!(self.theme.muted; "{status}")]).render(status_rect, buf);

        let list_top = area.y.saturating_add(2);
        let list_height = hint_top.saturating_sub(list_top) as usize;
        let list_area = Rect {
            x: area.x,
            y: list_top,
            width: area.width,
            height: u16::try_from(list_height).unwrap_or(0),
        };
        if let Some(at) = options {
            self.render_options(list_area, buf, at);
            return;
        }
        if view.rows.is_empty() {
            let label = if let Some(reason) = view.unavailable {
                reason
            } else if view.empty_scope {
                "(this view lists no sessions or muxes; Tab searches all of them)"
            } else if self.state.content_text().is_empty() {
                "(type to search what was said)"
            } else if view.done && view.with_panes {
                "(no sessions or panes match)"
            } else if view.done {
                "(no sessions match)"
            } else {
                "(searching…)"
            };
            let label = ellipsize(label, width);
            Paragraph::new(line![span!(self.theme.muted; "{label}")]).render(list_area, buf);
            return;
        }
        let scroll = compute_scroll(self.state.cursor(), list_height, view.rows.len());
        // Leave a blank cell before the frame.
        let columns = content_columns(width.saturating_sub(1));
        let lines: Vec<Line<'static>> = (scroll..(scroll + list_height).min(view.rows.len()))
            .map(|idx| {
                content_row_line(
                    &view.rows[idx],
                    idx == self.state.cursor(),
                    columns,
                    self.theme,
                )
            })
            .collect();
        Paragraph::new(lines).render(list_area, buf);
    }

    /// The options pane: what a search looks in. Changing one searches
    /// again.
    fn render_options(&self, area: Rect, buf: &mut Buffer, at: usize) {
        let kinds = self.state.kinds();
        let on = [kinds.tools, kinds.thinking, self.state.panes()];
        let lines: Vec<Line<'static>> = SEARCH_OPTIONS
            .iter()
            .zip(on)
            .enumerate()
            .map(|(i, (name, on))| {
                let mark = if i == at { "> " } else { "  " };
                let boxed = if on { "[x]" } else { "[ ]" };
                let text = format!("{mark}{boxed} {name}");
                if i == at {
                    Line::from(Span::styled(
                        text,
                        Style::default().add_modifier(Modifier::REVERSED),
                    ))
                } else {
                    Line::from(text)
                }
            })
            .collect();
        Paragraph::new(lines).render(area, buf);
    }

    /// The key hints, wrapped to the modal and drawn on its last rows.
    /// Returns how many rows they took.
    fn render_footer(&self, area: Rect, buf: &mut Buffer, hints: &str) -> u16 {
        let lines = crate::tui::widgets::popup_frame::wrapped_lines(hints, 0, area.width as usize);
        let rows = lines
            .len()
            .min(4)
            .min(area.height.saturating_sub(3) as usize);
        let rows = u16::try_from(rows).unwrap_or(0);
        let top = area.y + area.height - rows;
        for (i, text) in lines.iter().take(usize::from(rows)).enumerate() {
            let rect = Rect {
                x: area.x,
                y: top + u16::try_from(i).unwrap_or(0),
                width: area.width,
                height: 1,
            };
            Paragraph::new(line![span!(self.theme.muted; "{text}")]).render(rect, buf);
        }
        rows
    }

    fn status_text(&self, view: &ContentView) -> String {
        let kinds = self.state.kinds();
        let mut what = vec!["messages"];
        if kinds.tools {
            what.push("tools");
        }
        if kinds.thinking {
            what.push("thinking");
        }
        let n = view.rows.len();
        let mut text = format!(
            "{n} {} · {}",
            if n == 1 { "result" } else { "results" },
            what.join(" + ")
        );
        if view.total > 0 {
            let noun = if view.with_panes {
                "sessions and panes"
            } else if view.total == 1 {
                "session"
            } else {
                "sessions"
            };
            if view.done {
                text.push_str(&format!(" · {} {noun} searched", view.total));
            } else {
                text.push_str(&format!(" · {}/{} {noun}", view.scanned, view.total));
            }
            if view.skipped > 0 {
                text.push_str(&format!(" · {} skipped", view.skipped));
            }
        }
        text
    }
}

/// The widths of a content result's cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContentColumns {
    /// The row's width.
    pub inner: usize,
    /// `session` or `pane`; 0 when dropped.
    pub source: usize,
    /// The harness chip or pane command; 0 when dropped.
    pub chip: usize,
    pub label: usize,
    /// The hit count or turn; at least a few cells.
    pub tag: usize,
    /// The age; 0 when dropped.
    pub age: usize,
}

/// The source word's width: the longer of `session` and `pane`.
const SOURCE_WIDTH: usize = 7;
/// The harness chip's width.
const CHIP_WIDTH: usize = 10;
/// A snippet narrower than this is not worth showing, so cells give way
/// to it.
const MIN_SNIPPET: usize = 20;

impl ContentColumns {
    /// Cells the snippet gets: what is left after the other cells, their
    /// two-cell cursor mark, and the gaps between them.
    fn snippet(&self) -> usize {
        let cells = [self.source, self.chip, self.age];
        let gaps: usize = cells.iter().filter(|cell| **cell > 0).count();
        // mark, label, tag, and one gap after each cell.
        self.inner
            .saturating_sub(2 + cells.iter().sum::<usize>() + gaps + self.label + self.tag + 2 + 2)
    }
}

/// Split a content result `inner` cells wide: the label takes a fifth of
/// the width (12 to 32 cells) and the snippet the rest. When the snippet
/// would be narrower than [`MIN_SNIPPET`] the chip, then the source word,
/// then the age give up their cells, then the label shrinks.
pub(crate) fn content_columns(inner: usize) -> ContentColumns {
    let mut columns = ContentColumns {
        inner,
        source: SOURCE_WIDTH,
        chip: CHIP_WIDTH,
        label: (inner / 5).clamp(12, 32),
        tag: match inner {
            100.. => 22,
            78.. => 20,
            _ => 14,
        },
        age: 4,
    };
    if columns.snippet() < MIN_SNIPPET {
        columns.chip = 0;
    }
    if columns.snippet() < MIN_SNIPPET {
        columns.source = 0;
    }
    if columns.snippet() < MIN_SNIPPET {
        columns.age = 0;
    }
    while columns.snippet() < MIN_SNIPPET && columns.label > 8 {
        columns.label -= 1;
    }
    columns
}

/// One contents result: cursor mark, where it was found, harness chip or
/// pane command, label, hit count or position, age, and the snippet with
/// the match marked.
fn content_row_line(
    row: &ContentRow,
    is_cursor: bool,
    columns: ContentColumns,
    theme: &Theme,
) -> Line<'static> {
    use crate::tui::widgets::badge::harness_badge;
    use crate::tui::widgets::popup_frame::ellipsize;

    let mark = if is_cursor {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    };
    let plain = Style::default().add_modifier(mark);
    let dim = if is_cursor {
        plain
    } else {
        Style::default().add_modifier(theme.muted)
    };
    let pad = |text: &str, width: usize| {
        let text = ellipsize(text, width);
        let gap = width.saturating_sub(cells(&text));
        format!("{text}{}", " ".repeat(gap))
    };
    let mut spans: Vec<Span<'static>> =
        vec![span!(plain; "{}", if is_cursor { "> " } else { "  " })];
    if columns.source > 0 {
        spans.push(span!(dim; "{} ", pad(row.target.source_word(), columns.source)));
    }
    if columns.chip > 0 {
        match row.target {
            ContentTarget::Session { .. } => {
                spans.push(harness_badge(&row.chip, theme));
                spans.push(span!(plain; " "));
            }
            ContentTarget::Pane { .. } => {
                spans.push(span!(dim; "{} ", pad(&row.chip, columns.chip)));
            }
        }
    }
    spans.push(span!(plain; "{}  ", pad(&row.label, columns.label)));
    let tag = match &row.turn {
        Some(turn) => turn.clone(),
        None => format!(
            "{} {}",
            row.hits,
            if row.hits == 1 { "hit" } else { "hits" }
        ),
    };
    spans.push(span!(dim; "{}  ", pad(&tag, columns.tag)));
    if columns.age > 0 {
        spans
            .push(span!(dim; "{:>width$} ", ellipsize(&row.age, columns.age), width = columns.age));
    }
    let budget = columns.snippet().max(8);
    let snippet = crate::tui::widgets::snippet::fit(
        &row.snippet,
        &row.highlight.clone().unwrap_or(0..0),
        budget,
    );
    match snippet.highlight.clone() {
        Some(range) => {
            let match_style = theme.match_style().add_modifier(mark);
            spans.push(span!(dim; "{}", &snippet.text[..range.start]));
            spans.push(span!(match_style; "{}", &snippet.text[range.clone()]));
            spans.push(span!(dim; "{}", &snippet.text[range.end..]));
        }
        None => spans.push(span!(dim; "{}", snippet.text)),
    }
    Line::from(spans)
}

/// Render a single match row with prefix, label, and a snippet of
/// the haystack around the matched range. The cursor row carries a
/// background highlight on every span; the matched bytes inside
/// the snippet are bolded so they stand out even on the highlighted
/// row.
fn build_match_line(
    label: String,
    item: Option<&SearchItem<'_>>,
    m: &SearchMatch,
    is_cursor: bool,
    snippet_budget: usize,
    theme: &Theme,
) -> Line<'static> {
    // Compose a per-span style. The cursor row uses
    // `Modifier::REVERSED` rather than an explicit background color
    // so the highlight adapts to whatever fg/bg the terminal theme
    // provides — a fixed dark-gray bg renders dark-on-dark on
    // light themes. Off-cursor rows dim the snippet context so the
    // matched portion pops; the cursor row skips DIM because the
    // reversed surface already separates it from neighbors.
    let span_style = |dim_when_not_cursor: bool, extra: Style| -> Style {
        let mut style = extra;
        if is_cursor {
            style = style.add_modifier(Modifier::REVERSED);
        } else if dim_when_not_cursor {
            style = style.add_modifier(theme.muted);
        }
        style
    };
    let prefix = if is_cursor { "> " } else { "  " };
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(span!(span_style(false, Style::default()); "{prefix}"));
    // Prepend a kind glyph so operators scan
    // results by symbol (`● session`, `▣ mux`, `⇄ pr`, …) instead
    // of relying on the textual `kind:` prefix some labels carry.
    // RowIds without a NodeKind mapping (Pin, Synthetic) get two
    // spaces so the label column stays aligned across the result
    // list — operators don't see the label jiggle row by row.
    spans.push(search_kind_glyph_span(&m.id, theme, is_cursor));
    spans.push(span!(span_style(false, Style::default()); "{}", label));

    // If we can show a snippet (haystack present and either
    // distinct from the label or carrying a match range), append
    // `· …<context>…` after the label so the operator sees *why*
    // the row matched.
    let haystack = item.map_or("", |i| i.haystack.as_ref());
    let matched_range = m.matched_range.clone().unwrap_or(0..0);
    let snippet_distinct = haystack != label;
    if !haystack.is_empty() && snippet_distinct {
        spans.push(span!(span_style(true, Style::default()); "  · "));
        let snippet = snippet_around(haystack, matched_range, snippet_budget);
        // The matched portion always renders bold + yellow so it
        // pops on both cursor and non-cursor rows; pre/post context
        // dims only when the row isn't the cursor.
        let match_style = span_style(false, theme.match_style());
        if let Some(range) = snippet.highlight.clone() {
            let pre = snippet.text[..range.start].to_string();
            let mid = snippet.text[range.clone()].to_string();
            let post = snippet.text[range.end..].to_string();
            let dim = span_style(true, Style::default());
            spans.push(span!(dim; "{pre}"));
            spans.push(span!(match_style; "{mid}"));
            spans.push(span!(dim; "{post}"));
        } else {
            spans.push(span!(span_style(true, Style::default()); "{}", snippet.text));
        }
    }

    Line::from(spans)
}

/// Resolve the NodeKind a search result row represents, when there
/// is one. `RowId::Pin` and `RowId::Synthetic` are not graph nodes
/// and return `None`; callers render two spaces in the glyph slot
/// to keep label-column alignment across the result list.
fn search_row_node_kind(id: &RowId) -> Option<NodeKind> {
    match id {
        RowId::Group(node_id) | RowId::AgentSession(node_id) | RowId::MuxSession(node_id) => {
            Some(NodeKind::from(node_id))
        }
        RowId::Hit { session, .. } => Some(NodeKind::from(session)),
        RowId::Pane { mux, .. } | RowId::PaneHit { mux, .. } => Some(NodeKind::from(mux)),
        RowId::Pr(_) => Some(NodeKind::ForgePr),
        RowId::Pin { .. } => Some(NodeKind::Pin),
        RowId::Nested { node, .. } => Some(NodeKind::from(node)),
        RowId::History { .. } | RowId::Remote(_) | RowId::Bucket(_) | RowId::Synthetic(_) => None,
    }
}

/// Build the leading glyph span for a search result row. Two cells
/// wide: `<glyph> ` for graph-backed rows; `  ` for Pin / Synthetic
/// rows that don't carry a NodeKind. The glyph keeps its kind color
/// even on the cursor row — the surrounding `REVERSED` modifier on
/// the prefix and label already separates the highlighted row from
/// its neighbors, so leaving the color intact keeps the symbol's
/// identity legible.
fn search_kind_glyph_span(id: &RowId, theme: &Theme, is_cursor: bool) -> Span<'static> {
    let Some(kind) = search_row_node_kind(id) else {
        let style = if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        return span!(style; "  ");
    };
    search_kind_glyph_span_for(kind, theme, is_cursor)
}

/// The glyph span for a known kind.
fn search_kind_glyph_span_for(kind: NodeKind, theme: &Theme, is_cursor: bool) -> Span<'static> {
    let style = node_kind_style(kind, theme);
    // ForgePr's slate color is `Color::Reset`; mirror the dodge
    // `kind_chip_span` and the breadcrumb renderer use — fall back
    // to `theme.pr_open()` since search results don't carry PR state.
    let color = if matches!(kind, NodeKind::ForgePr) {
        theme.pr_open()
    } else {
        style.color
    };
    let mut span_style = Style::default().fg(color);
    if is_cursor {
        span_style = span_style.add_modifier(Modifier::REVERSED);
    }
    span!(span_style; "{} ", style.glyph)
}

/// The modal's outer rect, centered in `area`.
pub(crate) fn centered_modal_rect(area: Rect) -> Rect {
    let (width, height) = modal_size(area.width, area.height);
    super::popup_frame::centered_rect(area, width, height)
}

/// Terminal cells `text` occupies.
fn cells(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn compute_scroll(cursor: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if cursor >= visible_rows {
        cursor + 1 - visible_rows
    } else {
        0
    }
}

/// Per-event context the search overlay reads through the
/// [`crate::tui::Overlay`] trait. Carries the current
/// visible-row items so the widget can rerank matches on every
/// keystroke.
pub struct SearchContext<'a> {
    pub items: &'a [SearchItem<'a>],
    /// The contents results, in the order the overlay lists them.
    pub content: &'a [ContentRow],
    /// Every entity, supplied when the key about to be handled widens the
    /// reach for the first time.
    pub entities: Option<EntityIndex>,
}

impl crate::tui::Overlay for SearchOverlayState {
    type Ctx<'a> = SearchContext<'a>;

    fn handle(&mut self, ctx: SearchContext<'_>, key: KeyEvent) -> crate::tui::OverlayOutcome {
        self.set_content_len(ctx.content.len());
        if let Some(entities) = ctx.entities {
            self.set_entities(entities);
        }
        let outcome = self.handle_key(key);
        self.refresh_matches(ctx.items);
        match outcome {
            SearchOutcome::OpenEntity(node) => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::OpenEntity(*node)))
            }
            SearchOutcome::Continue => crate::tui::OverlayOutcome::Consumed,
            SearchOutcome::Cancel => crate::tui::OverlayOutcome::Close,
            SearchOutcome::Confirm(id) => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::SelectRow(id)))
            }
            SearchOutcome::OpenHit(index) => match ctx.content.get(index) {
                Some(row) => {
                    let text = self.content_text();
                    let msg = match &row.target {
                        ContentTarget::Session {
                            session,
                            turn_index,
                        } => crate::tui::Msg::OpenContentHit {
                            session: session.clone(),
                            text,
                            turn_index: *turn_index,
                        },
                        ContentTarget::Pane { pane, line } => crate::tui::Msg::OpenPaneHit {
                            pane: pane.clone(),
                            line: *line,
                            text,
                            reveal: true,
                        },
                    };
                    crate::tui::OverlayOutcome::Commit(Box::new(msg))
                }
                None => crate::tui::OverlayOutcome::Close,
            },
            SearchOutcome::ApplyFilter => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::ApplyContentSearch {
                    text: self.content_text(),
                    kinds: self.kinds,
                    group: self.group,
                    reach: self.reach,
                    show_results: false,
                }))
            }
            SearchOutcome::Regroup(group) => crate::tui::OverlayOutcome::CommitAndStay(Box::new(
                crate::tui::Msg::SetContentGroup(group),
            )),
            SearchOutcome::Keep => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::KeepContentSearch {
                    text: self.content_text(),
                    kinds: self.kinds,
                    group: self.group,
                    reach: self.reach,
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "search_content_tests.rs"]
mod content_tests;

#[cfg(test)]
#[path = "search_quick_tests.rs"]
mod quick_tests;
