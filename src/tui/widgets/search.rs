//! The search overlay: quick select and content search, two palettes.
//!
//! Quick select (`/`) picks an entity by name. It looks at the current
//! view's rows by default and widens to every entity in the snapshot
//! with Tab. Content search looks inside what was said.
//!
//! Both are [`tuikit::palette::Palette`]s: the palette keeps the query,
//! the title's strips and count, the cursor, the scroll window, the
//! status row, the options pane, the debounce, and the footer. This
//! module is the adapter that says what they mean: the ranking
//! ([`crate::tui::search::rank`] over the view's rows,
//! [`crate::tui::search_entities::EntityIndex::rank`] over every entity),
//! the outcome words, the content kinds and the history paging, the
//! columns and cells of a content hit, and the messages a pick commits.
//!
//! Per ADR 0031 and 0149 the current scope ranks every row of the active
//! view after filters, collapsed rows included: the host hands the overlay
//! those rows as [`SearchItem`]s.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tuikit::keys::PaletteKey;
use tuikit::overlay::Outcome;
use tuikit::palette::{Count, Options, PALETTE_SIZE, Palette, PaletteAction, Progress};

use crate::model::NodeId;
use crate::tui::Theme;
use crate::tui::keybindings::overlay::{SEARCH_CONTENT_KEYS, SEARCH_KEYS, SearchKey};
use crate::tui::rows::RowId;
use crate::tui::search::{SearchItem, SearchMatch, rank};
use crate::tui::search_entities::{EntityIndex, EntityMatch, SearchEntity};
use crate::viewer::search::{ContentKinds, Grouping};

mod rows;

use rows::{ContentGrid, QuickRow};
pub(crate) use rows::{
    Hit, HitChip, HitCol, HitShape, hit_columns, hits_tag, line_tag, snippet_cell, turn_tag,
};

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

    /// The reach's choice in the title's first strip ([`REACHES`]).
    fn choice(self) -> usize {
        match self {
            Self::Current => 0,
            Self::All => 1,
        }
    }

    /// The reach the first strip's `choice` names.
    fn chosen(choice: usize) -> Self {
        if choice == 0 {
            Self::Current
        } else {
            Self::All
        }
    }
}

/// The reach choices the title shows, in [`Reach`] order: both are always
/// there, the chosen one in brackets, so the title is as wide whichever
/// is chosen.
const REACHES: [&str; 2] = ["Current", "All"];
/// The granularity choices of content search's second strip.
const GRANULARITIES: [&str; 2] = ["Sessions", "Messages"];

/// The granularity's choice in the second strip.
fn granularity_choice(group: Grouping) -> usize {
    usize::from(group.granularity() != Grouping::Session)
}

/// How large the overlay is: a palette's size, most of the screen, so a
/// wide terminal shows more of each result.
pub(crate) const SEARCH_SIZE: tuikit::frame::ModalSize = PALETTE_SIZE;

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
    /// How recent that is, which tints the age; `None` leaves it
    /// receding.
    pub recency: Option<tuikit::age::Recency>,
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

/// An earlier content search the field can be filled from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastSearch {
    pub text: String,
    pub reach: Reach,
    pub kinds: ContentKinds,
    pub panes: bool,
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

/// The search tables bind the palette's own actions and the search's
/// verbs; the verbs are the overlay's to handle.
impl PaletteAction for SearchKey {
    fn palette_key(self) -> Option<PaletteKey> {
        match self {
            Self::Abort => Some(PaletteKey::Abort),
            Self::Cancel => Some(PaletteKey::Cancel),
            Self::Pick => Some(PaletteKey::Pick),
            Self::Down => Some(PaletteKey::Down),
            Self::Up => Some(PaletteKey::Up),
            Self::Scope => Some(PaletteKey::Scope),
            Self::Options => Some(PaletteKey::Options),
            Self::Granularity
            | Self::ApplyFilter
            | Self::Keep
            | Self::PrevQuery
            | Self::NextQuery => None,
        }
    }
}

/// Quick select: the view's rows or every entity, ranked by the host.
type QuickPalette = Palette<QuickRow, SearchOutcome, SearchKey>;
/// Content search: the scan's results; a pick is the hit under the
/// cursor.
type ContentPalette = Palette<ContentRow, (), SearchKey>;

/// The overlay's palette, one per mode.
#[derive(Clone, Debug)]
enum Palettes {
    Quick(QuickPalette),
    Content(ContentPalette),
}

/// Quick select's palette, its query filled with `query`.
fn quick_palette(query: &str) -> QuickPalette {
    Palette::new("/ quick select", Vec::new(), |row: &QuickRow| {
        Outcome::Commit(row.pick())
    })
    .keys(SEARCH_KEYS)
    .scopes(REACHES)
    .count(Count::Listed)
    .size(SEARCH_SIZE)
    .empty_note("(start typing)")
    .no_match_note("(no matches)")
    .reserve_scrollbar()
    .with_query(query)
}

/// Content search's palette, its query filled with `query` (not
/// searched: opening is no edit), with the options as given.
fn content_palette(
    query: &str,
    group: Grouping,
    kinds: ContentKinds,
    panes: bool,
) -> ContentPalette {
    let on = [kinds.tools, kinds.thinking, panes];
    let mut palette = Palette::new("/ search contents", Vec::new(), |_: &ContentRow| {
        Outcome::Commit(())
    })
    .keys(SEARCH_CONTENT_KEYS)
    .scopes(REACHES)
    .scopes(GRANULARITIES)
    .count(Count::None)
    .size(SEARCH_SIZE)
    .options(Options::new(SEARCH_OPTIONS.into_iter().zip(on)))
    .debounced(DEBOUNCE)
    .reserve_scrollbar()
    .with_query(query);
    palette.set_scope(1, granularity_choice(group));
    palette
}

/// State for the search overlay: its palette, and what quick select
/// ranked and content search is set to.
#[derive(Debug, Clone)]
pub struct SearchOverlayState {
    palette: Palettes,
    /// The view's rows that match the query, best first (the current
    /// reach).
    matches: Vec<SearchMatch>,
    /// Every entity, built the first time the wider reach is chosen.
    entities: Option<EntityIndex>,
    entity_matches: Vec<EntityMatch>,
    group: Grouping,
    kinds: ContentKinds,
    /// Whether the search reads pane scrollback as well as transcripts.
    panes: bool,
    /// Earlier searches, newest first, for paging back through.
    past: Vec<PastSearch>,
    /// The earlier search the field shows; `None` for the operator's own.
    past_at: Option<usize>,
}

impl SearchOverlayState {
    pub fn new() -> Self {
        Self {
            palette: Palettes::Quick(quick_palette("")),
            matches: Vec::new(),
            entities: None,
            entity_matches: Vec::new(),
            group: Grouping::default(),
            kinds: ContentKinds::default(),
            panes: true,
            past: Vec::new(),
            past_at: None,
        }
    }

    /// An overlay set up the way the operator left it: `query` fills the
    /// field (opening searches nothing; the first change does). It always
    /// opens on the current reach.
    pub fn configured(
        mode: SearchMode,
        group: Grouping,
        kinds: ContentKinds,
        contents_enabled: bool,
        query: &str,
    ) -> Self {
        let group = group.granularity();
        let palette = if mode == SearchMode::Content && contents_enabled {
            Palettes::Content(content_palette(query, group, kinds, true))
        } else {
            Palettes::Quick(quick_palette(query))
        };
        Self {
            palette,
            group,
            kinds,
            ..Self::new()
        }
    }

    /// Open on `reach` instead of the current context.
    #[must_use]
    pub fn with_reach(mut self, reach: Reach) -> Self {
        self.set_reach(reach);
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
        self.set_panes(panes);
        self
    }

    /// Turn off pane scrollback, as the options pane would.
    #[cfg(test)]
    pub fn with_panes_off(&mut self) {
        self.set_panes(false);
    }

    /// Open with the options pane showing.
    #[must_use]
    pub fn with_options_open(mut self) -> Self {
        if let Palettes::Content(palette) = &mut self.palette {
            palette.open_options();
        }
        self
    }

    /// Whether pane scrollback is searched.
    pub fn panes(&self) -> bool {
        self.panes
    }

    /// The options pane's cursor, when the pane is open.
    pub fn options_cursor(&self) -> Option<usize> {
        match &self.palette {
            Palettes::Content(palette) => palette
                .options_pane()
                .filter(|options| options.is_open())
                .map(Options::cursor),
            Palettes::Quick(_) => None,
        }
    }

    pub fn mode(&self) -> SearchMode {
        match self.palette {
            Palettes::Quick(_) => SearchMode::Quick,
            Palettes::Content(_) => SearchMode::Content,
        }
    }

    pub fn reach(&self) -> Reach {
        Reach::chosen(match &self.palette {
            Palettes::Quick(palette) => palette.scope(0),
            Palettes::Content(palette) => palette.scope(0),
        })
    }

    /// Whether the entity index is wanted but not built yet.
    pub fn needs_entities(&self) -> bool {
        self.mode() == SearchMode::Quick && self.reach() == Reach::All && self.entities.is_none()
    }

    /// Whether `event` is the key that widens a quick select that has not
    /// built its entity index yet, so the host can build it first.
    pub fn widens_with(&self, event: KeyEvent) -> bool {
        let Palettes::Quick(palette) = &self.palette else {
            return false;
        };
        self.reach() == Reach::Current
            && self.entities.is_none()
            && palette.action(event) == Some(SearchKey::Scope)
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

    /// The host hands over the contents results, in the order the overlay
    /// lists them, so the cursor can move among them.
    fn set_content(&mut self, rows: &[ContentRow]) {
        // Quick select passes no contents, and its cursor is its own.
        if let Palettes::Content(palette) = &mut self.palette {
            palette.set_rows(rows.to_vec());
        }
    }

    /// The scan the overlay is owed once the query has rested, if any.
    /// `Clear` says to stop previewing (an empty query). Quick select
    /// owes none.
    pub fn take_due_search(&mut self, now: Instant, force: bool) -> Option<DueSearch> {
        let Palettes::Content(palette) = &mut self.palette else {
            return None;
        };
        if !palette.take_due(now, force) {
            return None;
        }
        let text = self.content_text();
        if text.is_empty() {
            Some(DueSearch::Clear)
        } else {
            Some(DueSearch::Preview {
                text,
                kinds: self.kinds,
                group: self.group,
                reach: self.reach(),
            })
        }
    }

    /// The query as the contents scan will see it.
    pub fn content_text(&self) -> String {
        self.query().trim().to_string()
    }

    /// Current query text (post-edit, pre-rank).
    pub fn query(&self) -> &str {
        match &self.palette {
            Palettes::Quick(palette) => palette.query(),
            Palettes::Content(palette) => palette.query(),
        }
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub fn cursor(&self) -> usize {
        match &self.palette {
            Palettes::Quick(palette) => palette.cursor(),
            Palettes::Content(palette) => palette.cursor(),
        }
    }

    /// Refresh the match list against `items` using `backend`. The
    /// host calls this after every key dispatch so the operator
    /// sees the list shrink/grow as they type. The cursor stays on its
    /// position while that is still a result, and goes to the top
    /// otherwise.
    pub fn refresh_matches(&mut self, items: &[SearchItem<'_>]) {
        let quick = self.mode() == SearchMode::Quick;
        let reach = self.reach();
        let query = self.query().to_string();
        self.matches = if quick && reach == Reach::Current {
            rank(&query, items)
        } else {
            Vec::new()
        };
        self.entity_matches = match (&self.entities, quick, reach) {
            (Some(index), true, Reach::All) => index.rank(&query),
            _ => Vec::new(),
        };
        self.list_results(items);
    }

    /// Hands quick select's palette its results: the matching rows with
    /// the labels `items` give them now, or the matching entities.
    fn list_results(&mut self, items: &[SearchItem<'_>]) {
        let reach = self.reach();
        let Palettes::Quick(palette) = &mut self.palette else {
            return;
        };
        let rows = match reach {
            Reach::Current => {
                let by_id: HashMap<&RowId, &SearchItem<'_>> =
                    items.iter().map(|item| (&item.id, item)).collect();
                self.matches
                    .iter()
                    .map(|hit| {
                        let item = by_id.get(&hit.id);
                        QuickRow::Row {
                            hit: hit.clone(),
                            label: item.map_or_else(
                                || "<missing>".to_string(),
                                |item| item.label.to_string(),
                            ),
                            haystack: item
                                .map_or_else(String::new, |item| item.haystack.to_string()),
                        }
                    })
                    .collect()
            }
            Reach::All => self
                .entity_matches
                .iter()
                .filter_map(|hit| {
                    let entity = self.entities.as_ref()?.get(hit.index)?;
                    Some(QuickRow::Entity {
                        hit: hit.clone(),
                        entity: entity.clone(),
                    })
                })
                .collect(),
        };
        palette.set_rows(rows);
    }

    /// Hand the overlay the entity index the wider reach searches.
    pub fn set_entities(&mut self, entities: EntityIndex) {
        self.entities.get_or_insert(entities);
    }

    fn set_reach(&mut self, reach: Reach) {
        match &mut self.palette {
            Palettes::Quick(palette) => palette.set_scope(0, reach.choice()),
            Palettes::Content(palette) => palette.set_scope(0, reach.choice()),
        }
    }

    /// Sets what a search looks in, in the options pane too.
    fn set_options(&mut self, kinds: ContentKinds, panes: bool) {
        self.kinds = kinds;
        self.panes = panes;
        if let Palettes::Content(palette) = &mut self.palette
            && let Some(options) = palette.options_mut()
        {
            options.set(0, kinds.tools);
            options.set(1, kinds.thinking);
            options.set(2, panes);
        }
    }

    fn set_panes(&mut self, panes: bool) {
        self.set_options(self.kinds, panes);
    }

    /// Reads back what the options pane has set.
    fn read_options(&mut self) {
        let Palettes::Content(palette) = &self.palette else {
            return;
        };
        if let Some(options) = palette.options_pane() {
            self.kinds.tools = options.is_on(0);
            self.kinds.thinking = options.is_on(1);
            self.panes = options.is_on(2);
        }
    }
}

impl Default for SearchOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// What a palette's outcome asks the overlay's host to do, for a palette
/// whose pick is `picked`.
fn search_outcome<M>(
    outcome: Outcome<M>,
    picked: impl FnOnce(M) -> SearchOutcome,
) -> SearchOutcome {
    match outcome {
        Outcome::Commit(message) | Outcome::Back(message) | Outcome::Stay(message) => {
            picked(message)
        }
        Outcome::Close | Outcome::Dismiss => SearchOutcome::Cancel,
        Outcome::Consumed => SearchOutcome::Continue,
    }
}

impl SearchOverlayState {
    /// Dispatch a crossterm key event: the palette's keys (`Esc` and
    /// `Ctrl-C` close, `Enter` picks the result under the cursor or closes
    /// an empty list, the arrows and `Ctrl-N`/`Ctrl-P` move, wrapping,
    /// `Tab` switches the reach, `Ctrl-O` opens the options, and the rest
    /// edit the query, `j` and `k` included), and content search's own
    /// verbs.
    pub fn handle_key(&mut self, event: KeyEvent) -> SearchOutcome {
        match self.palette {
            Palettes::Quick(_) => self.handle_quick(event),
            Palettes::Content(_) => self.handle_content(event),
        }
    }

    fn handle_quick(&mut self, event: KeyEvent) -> SearchOutcome {
        let Palettes::Quick(palette) = &mut self.palette else {
            return SearchOutcome::Continue;
        };
        let before = palette.cursor();
        let query = palette.query().to_string();
        let outcome = palette.handle_key(event);
        // Quick select keeps the cursor's position across an edit while
        // it is still a result (the host's next ranking goes to the top
        // when it is not); the palette would put it on the first.
        if palette.query() != query {
            palette.select(before);
        }
        search_outcome(outcome, |picked| picked)
    }

    fn handle_content(&mut self, event: KeyEvent) -> SearchOutcome {
        let Palettes::Content(palette) = &mut self.palette else {
            return SearchOutcome::Continue;
        };
        if let Some(verb) = palette.host_action(event) {
            return self.content_verb(verb);
        }
        let outcome = palette.handle_key(event);
        let cursor = palette.cursor();
        self.read_options();
        search_outcome(outcome, |()| SearchOutcome::OpenHit(cursor))
    }

    /// Content search's own verbs: regroup, promote, and page through
    /// earlier searches.
    fn content_verb(&mut self, verb: SearchKey) -> SearchOutcome {
        match verb {
            SearchKey::Granularity => {
                // Listing the same matches another way is not a new search.
                self.group = match self.group.granularity() {
                    Grouping::Session => Grouping::Flat,
                    _ => Grouping::Session,
                };
                if let Palettes::Content(palette) = &mut self.palette {
                    palette.set_scope(1, granularity_choice(self.group));
                    palette.select(0);
                }
                SearchOutcome::Regroup(self.group)
            }
            SearchKey::ApplyFilter => self.promote(SearchOutcome::ApplyFilter),
            SearchKey::Keep => self.promote(SearchOutcome::Keep),
            SearchKey::PrevQuery => self.page_past(1),
            SearchKey::NextQuery => self.page_past(-1),
            SearchKey::Abort
            | SearchKey::Cancel
            | SearchKey::Pick
            | SearchKey::Down
            | SearchKey::Up
            | SearchKey::Scope
            | SearchKey::Options => SearchOutcome::Continue,
        }
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
        self.set_reach(past.reach);
        self.set_options(past.kinds, past.panes);
        self.fill_query(past.text)
    }

    /// Fill the field with `text` and owe a search for it, from the first
    /// result.
    fn fill_query(&mut self, text: String) -> SearchOutcome {
        if let Palettes::Content(palette) = &mut self.palette {
            palette.set_query(text);
            palette.select(0);
            palette.mark_edited(Instant::now());
        }
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
}

/// What a result's outcome word says, for the host to compute per row.
pub type OutcomeWords<'a> = &'a dyn Fn(&NodeId) -> String;

/// Draws the overlay: the palette of its mode, with the rows the host
/// holds now. Drawing keeps the results' scroll window, so it borrows the
/// state mutably.
pub struct SearchOverlayWidget<'a> {
    state: &'a mut SearchOverlayState,
    items: &'a [SearchItem<'a>],
    theme: &'a Theme,
    content: Option<&'a ContentView>,
    outcomes: Option<OutcomeWords<'a>>,
}

impl<'a> SearchOverlayWidget<'a> {
    pub fn new(
        state: &'a mut SearchOverlayState,
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

impl Widget for SearchOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Self {
            state,
            items,
            theme,
            content,
            outcomes,
        } = self;
        // The current reach's labels come from the rows as they are now,
        // so a label never lags behind a refresh.
        if state.mode() == SearchMode::Quick && state.reach() == Reach::Current {
            state.list_results(items);
        }
        let kinds = state.kinds;
        match &mut state.palette {
            Palettes::Quick(palette) => {
                palette.draw_with(area, buf, theme, |row, cells| {
                    row.cells(cells, theme, outcomes)
                });
            }
            Palettes::Content(palette) => {
                let empty = ContentView::default();
                let view = content.unwrap_or(&empty);
                palette.set_rows(view.rows.clone());
                palette.set_status(Some(status_text(view, kinds)));
                let note = content_note(view, palette.query());
                palette.set_empty_note(note);
                palette.set_no_match_note(note);
                let grid = ContentGrid::new(&view.rows, theme);
                palette.draw_with(area, buf, theme, |row, cells| {
                    grid.cells(row, cells.width, theme)
                });
            }
        }
    }
}

/// The contents scope's status row: how many results, what is searched,
/// and how far the scan has got.
fn status_text(view: &ContentView, kinds: ContentKinds) -> String {
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
    let noun = if view.with_panes {
        "sessions and panes"
    } else if view.total == 1 {
        "session"
    } else {
        "sessions"
    };
    let progress = Progress {
        scanned: view.scanned,
        total: view.total,
        skipped: view.skipped,
        done: view.done,
    };
    if let Some(progress) = progress.status(noun) {
        text.push_str(" · ");
        text.push_str(&progress);
    }
    text
}

/// What the contents scope says while it lists nothing, for the query
/// `query`.
fn content_note(view: &ContentView, query: &str) -> &'static str {
    if let Some(reason) = view.unavailable {
        reason
    } else if view.empty_scope {
        "(this view lists no sessions or muxes; Tab searches all of them)"
    } else if query.trim().is_empty() {
        "(type to search what was said)"
    } else if view.done && view.with_panes {
        "(no sessions or panes match)"
    } else if view.done {
        "(no sessions match)"
    } else {
        "(searching…)"
    }
}

/// What the search overlay reads on each key: the current visible-row
/// items, so it can rerank matches on every keystroke, its content
/// results, and, the first time it widens, every entity.
pub struct SearchContext<'a> {
    pub items: &'a [SearchItem<'a>],
    /// The contents results, in the order the overlay lists them.
    pub content: &'a [ContentRow],
    /// Every entity, supplied when the key about to be handled widens the
    /// reach for the first time.
    pub entities: Option<EntityIndex>,
}

impl SearchOverlayState {
    /// One key with what the overlay reads (`ctx`): edit the query, move,
    /// pick a row, an entity, or a hit, regroup, keep the search, or close.
    pub fn handle_in(&mut self, ctx: SearchContext<'_>, key: KeyEvent) -> Outcome<crate::tui::Msg> {
        self.set_content(ctx.content);
        if let Some(entities) = ctx.entities {
            self.set_entities(entities);
        }
        let outcome = self.handle_key(key);
        self.refresh_matches(ctx.items);
        match outcome {
            SearchOutcome::OpenEntity(node) => Outcome::Commit(crate::tui::Msg::OpenEntity(*node)),
            SearchOutcome::Continue => Outcome::Consumed,
            SearchOutcome::Cancel => Outcome::Close,
            SearchOutcome::Confirm(id) => Outcome::Commit(crate::tui::Msg::SelectRow(id)),
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
                    Outcome::Commit(msg)
                }
                None => Outcome::Close,
            },
            SearchOutcome::ApplyFilter => Outcome::Commit(crate::tui::Msg::ApplyContentSearch {
                text: self.content_text(),
                kinds: self.kinds,
                group: self.group,
                reach: self.reach(),
                show_results: false,
            }),
            SearchOutcome::Regroup(group) => Outcome::Stay(crate::tui::Msg::SetContentGroup(group)),
            SearchOutcome::Keep => Outcome::Commit(crate::tui::Msg::KeepContentSearch {
                text: self.content_text(),
                kinds: self.kinds,
                group: self.group,
                reach: self.reach(),
            }),
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
