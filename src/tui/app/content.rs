//! The content search the whole TUI shares: one query over every
//! session's transcript, narrowing the session lists and feeding the
//! Search view.
//!
//! The state lives here, in memory, for the life of the process. It is
//! never saved with the TUI state, never written into a snapshot, and
//! never sent to the daemon, because it is made of transcript text. A
//! scan runs outside the reducer: the reducer decides what to scan and
//! leaves a [`ScanRequest`] for the runtime, which streams results back
//! as [`Msg::ContentHits`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use tuikit::age::Age;

use super::content_history::SearchHistory;
use super::*;
use crate::content_search::{PaneHits, Progress, SessionKey};
use crate::filter::{ContentDetailMap, ContentFilter, ContentHitMap};
use crate::tui::Effect;
use crate::tui::widgets::search::Reach;
use crate::viewer::search::{ContentKinds, Grouping};

/// What the interface says when a content search cannot start because
/// live preview is off. One text for every place that explains it.
pub const CONTENT_OFF_NOTICE: &str = "Content search is off while live preview is disabled";

/// How the Search view lists results, in the Controls overlay.
pub fn grouping_description(group: Grouping) -> &'static str {
    match group {
        Grouping::Session => "Sessions",
        Grouping::Message | Grouping::Flat => "Messages",
    }
}

/// The most sessions whose matching turns are held in memory at once.
/// The best-ranked sessions get them first; the rest are not read.
pub const MAX_DETAIL_SESSIONS: usize = 100;

/// How many sessions the Search view opens by itself, so the first
/// screen shows matches without reading every matching transcript.
pub const AUTO_EXPAND_SESSIONS: usize = 10;

/// Add two scans' counters, for an incremental scan that continues a
/// finished one.
fn add_progress(a: Progress, b: Progress) -> Progress {
    Progress {
        scanned: a.scanned + b.scanned,
        total: a.total + b.total,
        skipped: a.skipped + b.skipped,
    }
}

/// How far the scan behind the active query has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentStatus {
    /// Sessions are still being read.
    Running(Progress),
    /// Every session has been looked at.
    Done(Progress),
}

impl ContentStatus {
    pub fn progress(&self) -> Progress {
        match self {
            Self::Running(progress) | Self::Done(progress) => *progress,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done(_))
    }
}

/// How far the flat listing's messages have been read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatCoverage {
    /// Matching turns are still being read.
    Reading,
    /// Everything that will be read is in; the rest is not shown.
    Partial { shown: usize, total: usize },
}

/// What a scope-limited search reads: the sessions and muxes the view it
/// started in listed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scope {
    pub sessions: BTreeSet<SessionKey>,
    /// Graph ids of the muxes whose panes are searched.
    pub muxes: BTreeSet<String>,
}

/// The active content query and what it has found so far.
#[derive(Clone, Debug)]
pub struct ContentSearch {
    /// Tells this search apart from every other the process has run, so
    /// its remembered copy can be found again.
    pub serial: u64,
    /// The query text as typed, trimmed.
    pub text: String,
    /// Which kinds of turn are searched.
    pub kinds: ContentKinds,
    /// Identifies the scan whose results are accepted; results stamped
    /// with an older one are dropped.
    pub generation: u64,
    pub status: ContentStatus,
    /// How the Search view lists the results.
    pub group: Grouping,
    /// Whether the search narrows the session lists. A search the `/`
    /// overlay is previewing runs and fills the overlay, but waits to be
    /// applied.
    pub applied: bool,
    /// Whether it read only what the view it started in listed, or every
    /// session and mux.
    pub reach: Reach,
    /// Whether it reads mux pane scrollback as well as transcripts.
    pub with_panes: bool,
    /// The modal closed with the search still running and the Search view
    /// kept it: it finishes in the background and then lets go.
    pub detached: bool,
    /// When the search started, for the age in the Search view.
    pub started_at: Instant,
    scope: Option<Scope>,
    hits: Arc<ContentHitMap>,
    /// The matching turns of each session, read on demand for the
    /// per-message grouping.
    details: Arc<ContentDetailMap>,
    /// A full rescan collects here and replaces `hits` when it ends, so
    /// the list does not empty and refill while it runs.
    staging: Option<ContentHitMap>,
    /// Mux panes whose scrollback matched.
    panes: Arc<Vec<PaneHits>>,
    pane_staging: Option<Vec<PaneHits>>,
    /// Sessions already handed to a scan, so a refresh scans only the
    /// ones that appeared since.
    scanned: BTreeSet<SessionKey>,
    /// Counters of the scans already finished, which a continuing scan
    /// adds to so the totals cover every session searched.
    carried: Progress,
    /// When the last scan finished, for the "scanned 2m ago" note.
    finished_at: Option<Instant>,
    /// The overlay's view of the results, rebuilt only when its inputs
    /// change.
    view_cache: RefCell<Option<(ViewKey, crate::tui::widgets::search::ContentView)>>,
}

/// What the cached overlay view was built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ViewKey {
    generation: u64,
    hits: usize,
    details: usize,
    progress: Progress,
    done: bool,
    group: Grouping,
    snapshot: usize,
    panes: usize,
    reach: Reach,
}

impl ContentSearch {
    /// Sessions that matched so far.
    pub fn hits(&self) -> &ContentHitMap {
        &self.hits
    }

    /// Mux panes whose scrollback matched so far.
    pub fn panes(&self) -> &[PaneHits] {
        &self.panes
    }

    /// Snippets this search keeps: one per matching session and every
    /// kept pane line.
    pub fn snippets(&self) -> usize {
        self.hits.len() + self.panes.iter().map(|pane| pane.hits.len()).sum::<usize>()
    }

    /// Hold no more snippets than one remembered search may: the
    /// lowest-ranked sessions keep their counts but lose their text.
    pub(super) fn keep_snippets_within_limit(&mut self) {
        let pane_snippets: usize = self.panes.iter().map(|pane| pane.hits.len()).sum();
        let room = super::content_history::MAX_SNIPPETS_PER_ENTRY.saturating_sub(pane_snippets);
        if self.hits.len() > room {
            Arc::make_mut(&mut self.hits).keep_snippets(room);
        }
    }

    /// A copy for the remembered searches: the results are shared, the
    /// overlay's cache is not.
    pub(super) fn for_history(&self) -> Self {
        let mut copy = self.clone();
        copy.view_cache = RefCell::new(None);
        copy.staging = None;
        copy.pane_staging = None;
        copy
    }

    /// Whether every session has been looked at.
    pub fn status_is_done(&self) -> bool {
        self.status.is_done()
    }

    /// When the last scan finished; `None` while one is running.
    pub fn finished_at(&self) -> Option<Instant> {
        self.finished_at
    }

    /// The scan's counters: sessions looked at, to look at, and skipped,
    /// across every scan the search has run.
    pub fn progress(&self) -> Progress {
        self.status.progress()
    }

    /// How the search is going, for the find chip and the Search view:
    /// `searching 34/77` while reading, then `3 sessions`, with
    /// `· N skipped` when sessions could not be read and
    /// `· scanned 2m ago` once it is done.
    pub fn status_note(&self, now: Instant) -> String {
        let progress = self.progress();
        let mut note = match self.status {
            ContentStatus::Running(_) => {
                format!("searching {}/{}", progress.scanned, progress.total)
            }
            ContentStatus::Done(_) => {
                let found = self.hits.len();
                let mut note = format!(
                    "{found} {}",
                    if found == 1 { "session" } else { "sessions" }
                );
                if self.with_panes {
                    let panes = self.panes.len();
                    note.push_str(&format!(
                        " · {panes} {}",
                        if panes == 1 { "pane" } else { "panes" }
                    ));
                }
                note
            }
        };
        if let Some(coverage) = self.flat_coverage() {
            note.push_str(&match coverage {
                FlatCoverage::Reading => " · reading messages".to_string(),
                FlatCoverage::Partial { shown, total } => {
                    format!(" · showing {shown} of {total} messages")
                }
            });
        }
        if progress.skipped > 0 {
            note.push_str(&format!(" · {} skipped", progress.skipped));
        }
        if let Some(finished) = self.finished_at {
            let age = now.saturating_duration_since(finished).as_secs();
            note.push_str(&format!(" · scanned {}", Age::from_secs(age).ago()));
        }
        note
    }

    /// How much of the flat listing has been read: `None` outside the
    /// flat grouping, while the scan runs, or once every matching
    /// message is held. The flat listing reads the turns of the sessions
    /// with the newest matches, up to [`MAX_DETAIL_SESSIONS`], so a large
    /// result set shows only the newest part of it.
    pub fn flat_coverage(&self) -> Option<FlatCoverage> {
        if self.group != Grouping::Flat || !self.status.is_done() {
            return None;
        }
        let total = self.hits.total_hits();
        let shown = self.details.total_hits();
        if shown >= total {
            return None;
        }
        let reachable = self.hits.len().min(MAX_DETAIL_SESSIONS);
        if self.details.len() < reachable {
            Some(FlatCoverage::Reading)
        } else {
            Some(FlatCoverage::Partial { shown, total })
        }
    }

    /// The row predicate for this search.
    pub fn filter(&self) -> ContentFilter {
        ContentFilter {
            text: self.text.clone(),
            hits: Arc::clone(&self.hits),
            complete: self.status.is_done(),
            group: self.group,
            details: Arc::clone(&self.details),
            panes: Arc::clone(&self.panes),
        }
    }
}

/// The mux panes a scan reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaneScope {
    /// Graph ids of the muxes to read; `None` reads every pane.
    pub muxes: Option<BTreeSet<String>>,
}

/// What to scan, left for the runtime by the reducer.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanRequest {
    pub generation: u64,
    pub text: String,
    pub kinds: ContentKinds,
    /// The sessions to read.
    pub sessions: Vec<AgentSessionId>,
    /// Also read these panes' scrollback.
    pub panes: Option<PaneScope>,
}

/// The matching turns to read for the Search view's per-message
/// grouping, left for the runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailRequest {
    pub generation: u64,
    /// Set when the turns are for a remembered search rather than the
    /// running one.
    pub serial: Option<u64>,
    pub text: String,
    pub kinds: ContentKinds,
    /// The sessions that matched.
    pub sessions: Vec<AgentSessionId>,
}

/// How a search begins.
pub(super) struct Launch {
    pub text: String,
    pub kinds: ContentKinds,
    pub group: Grouping,
    /// Narrows the session lists as soon as it starts.
    pub applied: bool,
    pub reach: Reach,
    /// Reads pane scrollback as well as transcripts.
    pub with_panes: bool,
    /// Started from the open modal; typing more replaces it.
    pub draft: bool,
}

/// Whose matching turns are being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailSubject {
    /// The running search (the filter, or what the modal previews).
    Live,
    /// The remembered search the Search view shows.
    Entry,
}

impl App {
    /// The active content search, if any. A search that is only a
    /// preview in the `/` overlay is not active.
    pub fn content(&self) -> Option<&ContentSearch> {
        self.content.as_ref().filter(|content| content.applied)
    }

    /// The search the `/` overlay is previewing, if any.
    pub fn content_preview(&self) -> Option<&ContentSearch> {
        self.content.as_ref().filter(|content| !content.applied)
    }

    /// The row predicate for the active search, for tree building.
    pub fn content_filter(&self) -> Option<ContentFilter> {
        self.content().map(ContentSearch::filter)
    }

    /// The searches the Search view remembers.
    pub fn search_history(&self) -> &SearchHistory {
        &self.search_history
    }

    /// The search the Search view shows, whether or not it filters the
    /// lists.
    ///
    /// The running search is read directly while it is the one shown, so
    /// what it has just found is already there.
    pub fn viewed_search(&self) -> Option<&ContentSearch> {
        let entry = self.search_history.viewed()?;
        match &self.content {
            Some(live) if live.serial == entry.search.serial => Some(live),
            _ => Some(&entry.search),
        }
    }

    /// The row predicate the Search view is built from.
    pub fn viewed_content_filter(&self) -> Option<ContentFilter> {
        self.viewed_search().map(ContentSearch::filter)
    }

    /// Whether the search the Search view shows is the one filtering the
    /// lists.
    pub fn viewed_is_filter(&self) -> bool {
        match (self.content(), self.viewed_search()) {
            (Some(filter), Some(viewed)) => filter.serial == viewed.serial,
            _ => false,
        }
    }

    /// Whether the running search is the one the Search view shows, so
    /// what it finds should redraw the view.
    fn live_search_is_viewed(&self) -> bool {
        self.active_view == View::Search
            && match (&self.content, self.viewed_search()) {
                (Some(live), Some(viewed)) => live.serial == viewed.serial,
                _ => false,
            }
    }

    /// The generation of the scan whose results are wanted now; `None`
    /// when no search is active. A runtime holding a different one
    /// should stop it.
    pub fn content_generation(&self) -> Option<u64> {
        self.content.as_ref().map(|content| content.generation)
    }

    /// Whose turns the runtime should be reading, as the search's
    /// generation and, for a remembered search, its serial; `None` when
    /// none are wanted. A runtime reading others should stop.
    pub fn detail_target(&self) -> Option<(u64, Option<u64>)> {
        match self.detail_subject()? {
            DetailSubject::Live => self.content.as_ref().map(|c| (c.generation, None)),
            DetailSubject::Entry => self
                .viewed_search()
                .map(|viewed| (viewed.generation, Some(viewed.serial))),
        }
    }

    /// Which search the matching turns are read for: the Search view's
    /// remembered one when it is showing one the running search is not,
    /// otherwise the running search.
    fn detail_subject(&self) -> Option<DetailSubject> {
        let showing_other = self.active_view == View::Search
            && self.search_overlay().is_none()
            && self.viewed_search().is_some_and(|viewed| {
                self.content
                    .as_ref()
                    .is_none_or(|live| live.serial != viewed.serial)
            });
        if showing_other {
            Some(DetailSubject::Entry)
        } else if self.content.is_some() {
            Some(DetailSubject::Live)
        } else {
            None
        }
    }

    /// Take the detail read the reducer asked for. Returns it once.
    pub fn take_detail_request(&mut self) -> Option<DetailRequest> {
        self.detail_request.take()
    }

    /// Ask for the matching turns the per-message groupings need, once
    /// the scan has finished. Under the nested grouping that is the
    /// sessions the operator has open (closed ones are read when they
    /// are opened); under the flat grouping it is every matching
    /// session, the ones with the newest matches first.
    pub(super) fn queue_details(&mut self) {
        let Some(subject) = self.detail_subject() else {
            return;
        };
        let target = match subject {
            DetailSubject::Live => self.content.as_ref(),
            DetailSubject::Entry => self.viewed_search(),
        };
        let Some(content) = target else {
            return;
        };
        if !content.group.lists_messages() || !content.status.is_done() {
            return;
        }
        let flat = content.group == Grouping::Flat;
        let open: std::collections::BTreeSet<(String, String)> = self
            .tree
            .rows
            .iter()
            .filter(|row| self.tree_state.is_expanded(&row.id))
            .filter_map(|row| match &row.kind {
                crate::tui::rows::RowKind::AgentSession(session) => Some((
                    session.session.harness_key.clone(),
                    session.session.session_key.clone(),
                )),
                _ => None,
            })
            .collect();
        // The best-ranked matches get their turns read first, and no
        // more than the cap are ever held.
        let room = MAX_DETAIL_SESSIONS.saturating_sub(content.details.len());
        let mut ranked: Vec<(AgentSessionId, &crate::viewer::search::SessionHits)> = self
            .snapshot_sessions()
            .into_iter()
            .filter(|session| {
                flat || open.contains(&(session.harness_key.clone(), session.session_key.clone()))
            })
            .filter_map(|session| {
                let hits = content
                    .hits
                    .get(&session.harness_key, &session.session_key)?;
                content
                    .details
                    .get(&session.harness_key, &session.session_key)
                    .is_none()
                    .then_some((session, hits))
            })
            .collect();
        if flat {
            ranked.sort_by(|a, b| crate::viewer::search::cmp_sessions_by_recency(a.1, b.1));
        } else {
            ranked.sort_by(|a, b| crate::viewer::search::cmp_sessions(a.1, b.1));
        }
        let sessions: Vec<AgentSessionId> = ranked
            .into_iter()
            .take(room)
            .map(|(session, _)| session)
            .collect();
        if sessions.is_empty() {
            return;
        }
        let request = DetailRequest {
            generation: content.generation,
            serial: (subject == DetailSubject::Entry).then_some(content.serial),
            text: content.text.clone(),
            kinds: content.kinds,
            sessions,
        };
        self.detail_request = Some(request);
    }

    /// Take the scan the reducer asked for. Returns it once.
    pub fn take_scan_request(&mut self) -> Option<ScanRequest> {
        self.scan_request.take()
    }

    /// Every agent session in the held snapshot.
    fn snapshot_sessions(&self) -> Vec<AgentSessionId> {
        let Some(handle) = &self.handle else {
            return Vec::new();
        };
        handle
            .snapshot()
            .live_nodes()
            .filter_map(|node| match node {
                crate::model::GraphNode::AgentSession(session) => Some(session.id.clone()),
                _ => None,
            })
            .collect()
    }

    /// The sessions a search with this `scope` reads: all of them, or
    /// those the scope names.
    fn scan_sessions(&self, scope: Option<&Scope>) -> Vec<AgentSessionId> {
        let sessions = self.snapshot_sessions();
        match scope {
            None => sessions,
            Some(scope) => sessions
                .into_iter()
                .filter(|session| {
                    scope.sessions.contains(&SessionKey {
                        harness_key: session.harness_key.clone(),
                        session_key: session.session_key.clone(),
                    })
                })
                .collect(),
        }
    }

    /// The sessions and muxes the active view lists, after its filters,
    /// folded rows included: what a search on the current context reads.
    /// The Search view lists a search's results, which are no place to
    /// look for something else, so its context is the one the search it
    /// shows was made in; `None` reads everything.
    pub(crate) fn current_scope(&self) -> Option<Scope> {
        if self.active_view == View::Search {
            return self.viewed_search().and_then(|viewed| viewed.scope.clone());
        }
        let mut scope = Scope::default();
        for row in &self.tree.rows {
            let node = crate::tui::rows::row_node(&row.id).or(match &row.kind {
                RowKind::Mixed(mixed) => Some(&mixed.primary_node),
                _ => None,
            });
            match node {
                Some(NodeId::AgentSession(id)) => {
                    scope.sessions.insert(SessionKey {
                        harness_key: id.harness_key.clone(),
                        session_key: id.session_key.clone(),
                    });
                }
                Some(NodeId::MuxSession(id)) => {
                    scope.muxes.insert(id.native_id.clone());
                }
                _ => {}
            }
        }
        Some(scope)
    }

    /// Start a scan of the held snapshot for the active query. A `fresh`
    /// scan also reads the panes the search wants; one that only picks up
    /// sessions that appeared since does not.
    fn queue_scan(&mut self, sessions: Vec<AgentSessionId>, fresh: bool) {
        let Some(content) = self.content.as_mut() else {
            return;
        };
        self.content_generation += 1;
        content.generation = self.content_generation;
        for session in &sessions {
            content.scanned.insert(SessionKey {
                harness_key: session.harness_key.clone(),
                session_key: session.session_key.clone(),
            });
        }
        content.finished_at = None;
        content.status = ContentStatus::Running(add_progress(
            content.carried,
            Progress {
                scanned: 0,
                total: sessions.len(),
                skipped: 0,
            },
        ));
        let panes = (fresh && content.with_panes)
            .then(|| PaneScope {
                muxes: content.scope.as_ref().map(|scope| scope.muxes.clone()),
            })
            .filter(|panes| panes.muxes.as_ref().is_none_or(|muxes| !muxes.is_empty()));
        self.scan_request = Some(ScanRequest {
            generation: content.generation,
            text: content.text.clone(),
            kinds: content.kinds,
            sessions,
            panes,
        });
    }

    /// After a refresh landed: scan the sessions that appeared since the
    /// last scan was planned. A scan still reading checks again when it
    /// ends, except one that was waiting for the first snapshot. A search
    /// limited to the view it started in does not grow.
    pub(super) fn scan_new_sessions(&mut self) {
        let Some(content) = &self.content else {
            return;
        };
        if content.scope.is_some() {
            return;
        }
        let waiting_for_snapshot =
            content.scanned.is_empty() && self.scan_request.is_none() && !content.status.is_done();
        if matches!(content.status, ContentStatus::Running(_)) && !waiting_for_snapshot {
            return;
        }
        let new: Vec<AgentSessionId> = self
            .snapshot_sessions()
            .into_iter()
            .filter(|session| {
                !content.scanned.contains(&SessionKey {
                    harness_key: session.harness_key.clone(),
                    session_key: session.session_key.clone(),
                })
            })
            .collect();
        if !new.is_empty() {
            if let Some(content) = self.content.as_mut() {
                content.carried = content.status.progress();
            }
            self.queue_scan(new, waiting_for_snapshot);
        }
    }

    /// Start a scan for `launch.text` and remember it as the newest
    /// search. Returns the reason it did not start.
    fn begin_content(&mut self, launch: Launch) -> Option<String> {
        if !self.config.live_preview_enabled {
            return Some(CONTENT_OFF_NOTICE.to_string());
        }
        self.search_auto_expanded.clear();
        self.content_serial += 1;
        let scope = if launch.reach == Reach::Current {
            self.current_scope()
        } else {
            None
        };
        // A search limited to a view that lists no muxes has no panes to
        // read.
        let with_panes =
            launch.with_panes && scope.as_ref().is_none_or(|scope| !scope.muxes.is_empty());
        let previous = self.content.replace(ContentSearch {
            serial: self.content_serial,
            text: launch.text,
            kinds: launch.kinds,
            generation: 0,
            status: ContentStatus::Running(Progress::default()),
            group: launch.group,
            applied: launch.applied,
            reach: launch.reach,
            with_panes,
            detached: false,
            started_at: Instant::now(),
            scope: scope.clone(),
            hits: Arc::new(ContentHitMap::default()),
            details: Arc::new(ContentDetailMap::default()),
            staging: None,
            panes: Arc::new(Vec::new()),
            pane_staging: None,
            scanned: BTreeSet::new(),
            carried: Progress::default(),
            finished_at: None,
            view_cache: RefCell::new(None),
        });
        // A search this one replaces is cut short.
        if let Some(old) = previous {
            self.stop_entry(old.serial);
        }
        let sessions = self.scan_sessions(scope.as_ref());
        if scope.is_none() && sessions.is_empty() && self.handle.is_none() {
            // No snapshot yet: the first refresh starts the scan.
            self.content_generation += 1;
            if let Some(content) = self.content.as_mut() {
                content.generation = self.content_generation;
            }
        } else {
            self.queue_scan(sessions, true);
        }
        // A search typed in the modal is remembered when the modal exits,
        // so the Search view stays put meanwhile.
        if !launch.draft {
            self.record_search();
        }
        self.start_search_spinner();
        None
    }

    /// Remember the running search as the newest one.
    fn record_search(&mut self) {
        let Some(content) = &self.content else {
            return;
        };
        let copy = content.for_history();
        let keep = self.config.search.history;
        self.search_history.record(copy, keep);
    }

    /// Copy the running search into its remembered entry.
    fn sync_history(&mut self) {
        if let Some(content) = &self.content
            && self.search_history.by_serial(content.serial).is_some()
        {
            self.search_history.update(content.for_history());
        }
    }

    /// Mark the entry as cut short when its scan was still running: what
    /// it found is all it will find.
    fn stop_entry(&mut self, serial: u64) {
        if let Some(entry) = self.search_history.by_serial_mut(serial)
            && !entry.search.status.is_done()
        {
            entry.stopped = true;
            let progress = entry.search.status.progress();
            entry.search.status = ContentStatus::Done(progress);
        }
    }

    /// Whether a search made from the modal reads pane scrollback: as the
    /// open modal has it, or as it was last left.
    fn modal_panes(&self) -> bool {
        self.search_overlay()
            .map(crate::tui::widgets::search::SearchOverlayState::panes)
            .or_else(|| self.search_memory.map(|memory| memory.panes))
            .unwrap_or(true)
    }

    /// Keep the entry for good, remembering the running search first when
    /// it is the one typed in the modal and not yet remembered.
    fn keep_entry(&mut self, serial: u64) {
        if self.search_history.by_serial(serial).is_none()
            && self
                .content
                .as_ref()
                .is_some_and(|content| content.serial == serial)
        {
            self.record_search();
        }
        self.search_history.keep(serial);
    }

    /// The modal exits: the search it was running becomes the newest
    /// remembered one, unless it never had a query.
    pub(crate) fn commit_modal_search(&mut self) {
        let serial = self
            .content
            .as_ref()
            .filter(|content| !content.applied && !content.detached)
            .map(|content| content.serial);
        if let Some(serial) = serial {
            self.keep_entry(serial);
        }
    }

    pub(super) fn reduce_content(&mut self, msg: Msg) -> Vec<Effect> {
        let mut effects = Vec::new();
        match msg {
            Msg::StartContentSearch { text, kinds } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return self.reduce_content(Msg::ClearContentSearch);
                }
                let group = self.config.search.group;
                if let Some(note) = self.begin_content(Launch {
                    text,
                    kinds,
                    group,
                    applied: true,
                    reach: Reach::All,
                    with_panes: false,
                    draft: false,
                }) {
                    effects.push(Effect::Toast(Feedback::warning(note)));
                    return effects;
                }
                self.content_stash = None;
                self.rebuild_tree_in_place();
            }
            Msg::PreviewContentSearch {
                text,
                kinds,
                group,
                reach,
            } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return self.reduce_content(Msg::DiscardContentPreview);
                }
                let displaced = self
                    .content
                    .take_if(|content| content.applied)
                    .or_else(|| self.content_stash.take());
                if let Some(note) = self.begin_content(Launch {
                    text,
                    kinds,
                    group,
                    applied: false,
                    reach,
                    with_panes: self.modal_panes(),
                    draft: true,
                }) {
                    // Nothing started: the search that was active stays.
                    self.content = displaced;
                    effects.push(Effect::Toast(Feedback::warning(note)));
                } else {
                    self.content_stash = displaced;
                }
            }
            Msg::ApplyContentSearch {
                text,
                kinds,
                group,
                reach,
                show_results,
            } => {
                let text = text.trim().to_string();
                let same = self.content.as_ref().is_some_and(|content| {
                    content.text == text
                        && content.kinds == kinds
                        && content.group == group
                        && content.reach == reach
                });
                if same {
                    if let Some(content) = self.content.as_mut() {
                        content.applied = true;
                        content.detached = false;
                        let serial = content.serial;
                        self.keep_entry(serial);
                    }
                    self.content_stash = None;
                    self.rebuild_tree_in_place();
                } else if let Some(note) = self.begin_content(Launch {
                    text,
                    kinds,
                    group,
                    applied: true,
                    reach,
                    with_panes: self.modal_panes(),
                    draft: false,
                }) {
                    effects.push(Effect::Toast(Feedback::warning(note)));
                    return effects;
                } else {
                    self.content_stash = None;
                    self.rebuild_tree_in_place();
                }
                if show_results {
                    effects.extend(self.update(Msg::SwitchView(View::Search)));
                }
            }
            Msg::KeepContentSearch {
                text,
                kinds,
                group,
                reach,
            } => {
                let text = text.trim().to_string();
                let same = self.content.as_ref().is_some_and(|content| {
                    content.text == text
                        && content.kinds == kinds
                        && content.group == group
                        && content.reach == reach
                });
                // The modal opened on a remembered search and left alone: it
                // is kept as it is, and not searched again.
                let shown = self.viewed_search().is_some_and(|viewed| {
                    viewed.text == text
                        && viewed.kinds == kinds
                        && viewed.group == group
                        && viewed.reach == reach
                });
                if !same && shown {
                    return self.update(Msg::SwitchView(View::Search));
                }
                if !same {
                    effects.extend(self.reduce_content(Msg::PreviewContentSearch {
                        text,
                        kinds,
                        group,
                        reach,
                    }));
                }
                let Some(content) = self.content.as_mut() else {
                    return effects;
                };
                let (serial, done, applied) =
                    (content.serial, content.status.is_done(), content.applied);
                self.keep_entry(serial);
                if !applied {
                    // The scan goes on without the modal and lets go of
                    // the engine when it ends.
                    if done {
                        self.release_preview();
                    } else if let Some(content) = self.content.as_mut() {
                        content.detached = true;
                    }
                }
                effects.extend(self.update(Msg::SwitchView(View::Search)));
            }
            Msg::DiscardContentPreview => {
                if self
                    .content
                    .as_ref()
                    .is_some_and(|content| !content.applied && !content.detached)
                {
                    let stash = self.content_stash.take();
                    self.clear_content();
                    if let Some(stash) = stash {
                        let done = stash.status.is_done();
                        self.content = Some(stash);
                        if !done {
                            return self.reduce_content(Msg::RescanContent);
                        }
                    }
                    self.rebuild_tree_in_place();
                }
            }
            Msg::OpenPaneHit {
                pane,
                line,
                text,
                reveal,
            } => {
                if reveal {
                    let mux =
                        NodeId::MuxSession(crate::model::MuxSessionId::new(pane.mux.native_id()));
                    if let Err(why) = self.reveal_node(&mux, None) {
                        self.status_message =
                            Some(Feedback::warning(format!("{} {why}", pane.mux.session)));
                    }
                }
                effects.push(Effect::Exec(crate::tui::effect::ExecSpec::PaneScrollback {
                    pane,
                    line,
                    text,
                }));
            }
            Msg::OpenSelectedPane => {
                let selected = self
                    .tree_state
                    .selected()
                    .and_then(|id| self.tree.rows.iter().find(|row| &row.id == id));
                let (pane, line) = match selected.map(|row| &row.kind) {
                    Some(RowKind::Pane(row)) => (row.pane.clone(), row.line),
                    Some(RowKind::PaneHit(row)) => (row.pane.clone(), row.line),
                    _ => return effects,
                };
                let text = self
                    .viewed_search()
                    .map(|viewed| viewed.text.clone())
                    .unwrap_or_default();
                effects.extend(self.reduce_content(Msg::OpenPaneHit {
                    pane,
                    line,
                    text,
                    reveal: false,
                }));
            }
            Msg::OpenContentHit {
                session,
                text,
                turn_index,
            } => {
                effects.push(Effect::Exec(crate::tui::effect::ExecSpec::ViewSession(
                    session,
                    Some(crate::viewer::state::ViewerOpen {
                        query: text,
                        turn_index,
                    }),
                )));
            }
            Msg::ClearContentSearch => {
                self.clear_content();
                self.rebuild_tree_in_place();
            }
            Msg::ClearFilters => {
                self.filter = crate::filter::RowFilter::default();
                self.clear_content();
                self.rebuild_tree_in_place();
                effects.push(Effect::Persist);
            }
            Msg::SetContentGroup(group) => {
                let viewed = self.viewed_search().map(|viewed| viewed.serial);
                // A search being typed in the modal is not the Search
                // view's until the modal exits: leave the view alone.
                let typing = self.search_overlay().is_some()
                    && self.content.as_ref().is_some_and(|content| {
                        !content.applied && !content.detached && viewed != Some(content.serial)
                    });
                if let Some(content) = self.content.as_mut()
                    && (typing || viewed.is_none() || viewed == Some(content.serial))
                {
                    content.group = group;
                }
                if !typing && let Some(entry) = self.search_history.viewed_mut() {
                    entry.search.group = group;
                }
                self.queue_details();
                self.rebuild_tree_in_place();
            }
            Msg::PageSearchHistory(_) | Msg::ShowSearchHistory(_)
                if self.active_view != View::Search =>
            {
                // Only the Search view shows a remembered search.
            }
            Msg::PageSearchHistory(delta) => {
                let left = self.viewed_search().map(|viewed| viewed.serial);
                if self.search_history.page(delta) {
                    self.after_history_move(left);
                }
            }
            Msg::ShowSearchHistory(index) => {
                let left = self.viewed_search().map(|viewed| viewed.serial);
                if self.search_history.show(index) {
                    self.after_history_move(left);
                }
            }
            Msg::OpenSearchOptions => {
                if !self.config.live_preview_enabled {
                    effects.push(Effect::Toast(Feedback::warning(CONTENT_OFF_NOTICE)));
                    return effects;
                }
                self.open_content_search_overlay();
                if let Some(state) = self.search_overlay_mut() {
                    *state = state.clone().with_options_open();
                }
            }
            Msg::ToggleSearchGranularity if self.active_view != View::Search => {
                // Only the Search view lists a search's results.
            }
            Msg::ToggleSearchGranularity => {
                let next = match self
                    .viewed_search()
                    .map(|viewed| viewed.group.granularity())
                {
                    Some(Grouping::Session) => Grouping::Flat,
                    _ => Grouping::Session,
                };
                return self.reduce_content(Msg::SetContentGroup(next));
            }
            Msg::OpenContentSearchModal => self.open_content_search_entry(),
            Msg::OpenSearchHistory => {
                if self.search_history.is_empty() {
                    effects.push(Effect::Toast(Feedback::warning("No searches yet")));
                    return effects;
                }
                let now = Instant::now();
                let viewed = self
                    .search_history
                    .position()
                    .map(|(position, _)| position - 1);
                let items: Vec<crate::tui::widgets::picker::PickerItem> = self
                    .search_history
                    .entries()
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(index, entry)| {
                        crate::tui::widgets::picker::PickerItem::new(
                            format!("\"{}\"", entry.search.text),
                            index.to_string(),
                        )
                        .detail(entry.describe(now))
                    })
                    .collect();
                let viewed = viewed.map(|viewed| viewed.to_string());
                self.open_picker(crate::tui::widgets::picker::search_history(
                    items,
                    viewed.as_deref(),
                ));
            }
            Msg::ApplySearchEntry if self.active_view != View::Search => {
                effects.push(Effect::Toast(Feedback::warning(
                    "Ctrl-F filters by the search the Search view shows (6)",
                )));
            }
            Msg::ApplySearchEntry => {
                let Some(viewed) = self.viewed_search().cloned() else {
                    effects.push(Effect::Toast(Feedback::warning("No search to filter by")));
                    return effects;
                };
                if self.viewed_is_filter() {
                    return effects;
                }
                if let Some(live) = self.content.as_mut()
                    && live.serial == viewed.serial
                {
                    live.applied = true;
                    live.detached = false;
                } else {
                    if let Some(old) = self.content.take() {
                        self.stop_entry(old.serial);
                    }
                    self.content_generation += 1;
                    let mut filter = viewed;
                    filter.applied = true;
                    filter.detached = false;
                    filter.generation = self.content_generation;
                    self.content = Some(filter);
                }
                self.content_stash = None;
                self.scan_new_sessions();
                self.rebuild_tree_in_place();
                if let Some(content) = self.content() {
                    effects.push(Effect::Toast(Feedback::success(format!(
                        "Filtering the lists by \"{}\"",
                        content.text
                    ))));
                }
            }
            Msg::ContentDetails { generation, batch } => {
                let Some(content) = self.content.as_mut() else {
                    return effects;
                };
                if content.generation != generation {
                    return effects;
                }
                let details = Arc::make_mut(&mut content.details);
                for (key, hits) in batch {
                    details.insert(key.harness_key, key.session_key, hits);
                }
                // A request still waiting for the reader need not ask
                // for turns that have just arrived.
                if let Some(request) = self.detail_request.as_mut() {
                    request.sessions.retain(|session| {
                        content
                            .details
                            .get(&session.harness_key, &session.session_key)
                            .is_none()
                    });
                    if request.sessions.is_empty() {
                        self.detail_request = None;
                    }
                }
                if content.applied || self.live_search_is_viewed() {
                    self.rebuild_tree_in_place();
                }
            }
            Msg::HistoryDetails { serial, batch } => {
                let Some(entry) = self.search_history.by_serial_mut(serial) else {
                    return effects;
                };
                let details = Arc::make_mut(&mut entry.search.details);
                for (key, hits) in batch {
                    details.insert(key.harness_key, key.session_key, hits);
                }
                let entry = &entry.search;
                if let Some(request) = self.detail_request.as_mut()
                    && request.serial == Some(serial)
                {
                    request.sessions.retain(|session| {
                        entry
                            .details
                            .get(&session.harness_key, &session.session_key)
                            .is_none()
                    });
                    if request.sessions.is_empty() {
                        self.detail_request = None;
                    }
                }
                if self.viewed_search().is_some_and(|v| v.serial == serial)
                    && self.active_view == View::Search
                {
                    self.rebuild_tree_in_place();
                }
            }
            Msg::RescanContent => {
                if self.content.is_some() {
                    let scope = self.content.as_ref().and_then(|c| c.scope.clone());
                    let sessions = self.scan_sessions(scope.as_ref());
                    if let Some(content) = self.content.as_mut() {
                        content.scanned.clear();
                        content.carried = Progress::default();
                        content.staging = Some(ContentHitMap::default());
                        content.pane_staging = Some(Vec::new());
                        // Turns read for the old results are read again.
                        content.details = Arc::new(ContentDetailMap::default());
                    }
                    self.queue_scan(sessions, true);
                    self.start_search_spinner();
                }
            }
            Msg::ContentPanes { generation, batch } => {
                let Some(content) = self.content.as_mut() else {
                    return effects;
                };
                if content.generation != generation {
                    return effects;
                }
                let changed = !batch.is_empty();
                match content.pane_staging.as_mut() {
                    Some(staging) => staging.extend(batch),
                    None => Arc::make_mut(&mut content.panes).extend(batch),
                }
                if changed && (content.applied || self.live_search_is_viewed()) {
                    self.rebuild_tree_in_place();
                }
            }
            Msg::ContentHits {
                generation,
                batch,
                progress,
                done,
            } => {
                let Some(content) = self.content.as_mut() else {
                    return effects;
                };
                if content.generation != generation {
                    return effects;
                }
                let changed = !batch.is_empty() || done;
                let progress = add_progress(content.carried, progress);
                for (key, hits) in batch {
                    let SessionKey {
                        harness_key,
                        session_key,
                    } = key;
                    // The turns read for an earlier result may no longer
                    // be the matching ones.
                    Arc::make_mut(&mut content.details).remove(&harness_key, &session_key);
                    match content.staging.as_mut() {
                        Some(staging) => staging.insert(harness_key, session_key, hits),
                        None => {
                            Arc::make_mut(&mut content.hits).insert(harness_key, session_key, hits);
                        }
                    }
                }
                if done {
                    if let Some(staging) = content.staging.take() {
                        content.hits = Arc::new(staging);
                    }
                    if let Some(staging) = content.pane_staging.take() {
                        content.panes = Arc::new(staging);
                    }
                    content.keep_snippets_within_limit();
                    content.status = ContentStatus::Done(progress);
                    content.finished_at = Some(Instant::now());
                } else {
                    content.status = ContentStatus::Running(progress);
                }
                let (applied, detached) = (content.applied, content.detached);
                if done {
                    self.update_in_flight(InFlightKind::Search, None);
                    self.scan_new_sessions();
                    self.queue_details();
                } else {
                    let label = format!("Searching {}/{}", progress.scanned, progress.total);
                    self.update_in_flight(InFlightKind::Search, Some(label));
                }
                // A preview in the overlay does not change the lists.
                if changed && (applied || self.live_search_is_viewed()) {
                    self.rebuild_tree_in_place();
                }
                if done && detached && !applied {
                    self.sync_history();
                    self.release_preview();
                }
            }
            _ => {}
        }
        self.sync_history();
        effects
    }

    /// A preview that the Search view kept has finished: let go of it and
    /// bring back the filter it displaced.
    fn release_preview(&mut self) {
        let stash = self.content_stash.take();
        self.content = None;
        self.update_in_flight(InFlightKind::Search, None);
        if let Some(stash) = stash {
            let done = stash.status.is_done();
            self.content = Some(stash);
            if !done {
                let _ = self.reduce_content(Msg::RescanContent);
            }
        }
        self.queue_details();
        self.rebuild_tree_in_place();
    }

    /// The Search view moved to another remembered search.
    fn after_history_move(&mut self, left: Option<u64>) {
        // The search left behind keeps its summary but not the per-message
        // turns read for it; they are read again if it comes back.
        let live = self.content.as_ref().map(|content| content.serial);
        if let Some(serial) = left.filter(|serial| Some(*serial) != live)
            && let Some(entry) = self.search_history.by_serial_mut(serial)
        {
            entry.search.details = Arc::default();
        }
        self.search_auto_expanded.clear();
        self.detail_request = None;
        self.rebuild_tree_in_place();
        self.queue_details();
        self.show_top_of_tree();
    }

    /// Open the Search view's buckets and sessions the first time they
    /// appear, so results show their matches without a keypress.
    pub(super) fn auto_expand_search_rows(&mut self) {
        if self.active_view != View::Search {
            return;
        }
        let is_session = |row: &crate::tui::rows::Row| {
            matches!(row.kind, crate::tui::rows::RowKind::AgentSession(_))
        };
        let mut open_sessions = self
            .tree
            .rows
            .iter()
            .filter(|row| row.expandable && is_session(row) && self.tree_state.is_expanded(&row.id))
            .count();
        for row in &self.tree.rows {
            if !row.expandable || !self.search_auto_expanded.insert(row.id.clone()) {
                continue;
            }
            if is_session(row) {
                // Only the first few open by themselves; the rest are
                // read when the operator opens them.
                if open_sessions >= AUTO_EXPAND_SESSIONS {
                    continue;
                }
                open_sessions += 1;
            }
            self.tree_state.expand_id(row.id.clone());
        }
        self.queue_details();
    }

    /// Drop the content search and stop its scan. The remembered copy of
    /// it stays in the Search view.
    fn clear_content(&mut self) {
        self.content_stash = None;
        self.search_auto_expanded.clear();
        if let Some(old) = self.content.take() {
            self.stop_entry(old.serial);
            // A new generation makes the runtime cancel whatever it runs.
            self.content_generation += 1;
        }
        self.scan_request = None;
        self.detail_request = None;
        self.update_in_flight(InFlightKind::Search, None);
    }

    fn start_search_spinner(&mut self) {
        self.update_in_flight(InFlightKind::Search, Some("Searching".to_string()));
    }

    /// Start, relabel, or (with `None`) finish the status-bar chip for
    /// `kind`.
    fn update_in_flight(&mut self, kind: InFlightKind, label: Option<String>) {
        match label {
            Some(label) => {
                let started_at = self
                    .in_flight_ops
                    .iter()
                    .find(|op| op.kind == kind)
                    .map_or_else(Instant::now, |op| op.started_at);
                let op = InFlightOp {
                    kind: kind.clone(),
                    label,
                    started_at,
                };
                match self.in_flight_ops.iter_mut().find(|o| o.kind == kind) {
                    Some(existing) => *existing = op,
                    None => self.in_flight_ops.push(op),
                }
            }
            None => self.in_flight_ops.retain(|op| op.kind != kind),
        }
    }
}

/// A session's name as the lists show it: alias, title, hosting mux
/// name, then the start of its key.
fn session_label(snapshot: &crate::model::GraphSnapshot, session: &AgentSessionId) -> String {
    let node = NodeId::AgentSession(session.clone());
    let alias = snapshot.aliases.get(&node);
    let (title, mux_host) = match snapshot.find_node(&node) {
        Some(crate::model::GraphNode::AgentSession(s)) => {
            (s.title.clone(), snapshot.mux_host_of(&node))
        }
        _ => (None, None),
    };
    crate::aliases::session_display_name(
        alias,
        title.as_deref(),
        mux_host.as_ref().map(|host| host.name.as_str()),
    )
    .map_or_else(
        || session.session_key.chars().take(8).collect(),
        str::to_string,
    )
}

/// What the search overlay was left set to, for the next time it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchMemory {
    pub group: Grouping,
    pub kinds: ContentKinds,
    pub panes: bool,
}

/// The most results the overlay lists.
pub const OVERLAY_RESULTS: usize = 50;

/// A session or pane and the rows it contributes to the modal, ranked
/// together.
struct Unit {
    count: usize,
    time: Option<i64>,
    pane: bool,
    origin: String,
    rows: Vec<crate::tui::widgets::search::ContentRow>,
}

/// A flat-listing row with what orders it: time, origin, position.
type FlatRow = (
    Option<i64>,
    (String, String),
    usize,
    crate::tui::widgets::search::ContentRow,
);

impl App {
    /// What the contents scope of the `/` overlay shows: the best
    /// matches of the running search (or, with the per-message
    /// grouping, its matching turns), ranked as the Search view ranks.
    pub fn content_view(&self) -> crate::tui::widgets::search::ContentView {
        let query = self
            .search_overlay()
            .map(crate::tui::widgets::search::SearchOverlayState::content_text)
            .unwrap_or_default();
        self.content_view_for(&query)
    }

    /// [`App::content_view`] for a contents modal whose query is `query`:
    /// what the modal itself asks for while it handles a key or draws.
    pub fn content_view_for(&self, query: &str) -> crate::tui::widgets::search::ContentView {
        use crate::tui::widgets::search::ContentView;
        let mut view = ContentView::default();
        if !self.config.live_preview_enabled {
            view.unavailable = Some(CONTENT_OFF_NOTICE);
            return view;
        }
        let Some(content) = self.modal_content(query) else {
            return view;
        };
        let key = ViewKey {
            generation: content.generation,
            hits: content.hits.len(),
            details: content.details.len(),
            panes: content.panes.len(),
            progress: content.status.progress(),
            done: content.status.is_done(),
            group: content.group,
            reach: content.reach,
            snapshot: self
                .handle
                .as_ref()
                .map_or(0, |handle| std::ptr::from_ref(handle.snapshot()) as usize),
        };
        if let Some((cached, view)) = &*content.view_cache.borrow()
            && *cached == key
        {
            return view.clone();
        }
        let view = self.build_content_view(content, view);
        *content.view_cache.borrow_mut() = Some((key, view.clone()));
        view
    }

    /// The search whose results the contents modal lists: the one it is
    /// previewing, or else the search whose query (`query`) it was opened
    /// with (the one the Search view shows, or the one filtering the
    /// lists).
    fn modal_content(&self, query: &str) -> Option<&ContentSearch> {
        let live = self.content.as_ref();
        if live.is_some_and(|content| !content.applied) {
            return live;
        }
        self.viewed_search()
            .filter(|viewed| !query.is_empty() && viewed.text == query)
            .or(live)
    }

    /// Rank and describe the matches for the overlay. Costly (it walks
    /// every session), so [`App::content_view`] caches it.
    fn build_content_view(
        &self,
        content: &ContentSearch,
        mut view: crate::tui::widgets::search::ContentView,
    ) -> crate::tui::widgets::search::ContentView {
        use crate::tui::widgets::search::{ContentRow, ContentTarget};
        use crate::viewer::search::{Hit, cmp_hits};

        let progress = content.status.progress();
        view.scanned = progress.scanned;
        view.total = progress.total;
        view.skipped = progress.skipped;
        view.done = content.status.is_done();
        view.with_panes = content.with_panes;
        view.empty_scope = content
            .scope
            .as_ref()
            .is_some_and(|scope| scope.sessions.is_empty() && scope.muxes.is_empty());
        let Some(handle) = &self.handle else {
            return view;
        };
        let snapshot = handle.snapshot();
        let now = Some(crate::tui::rows::tree_current_unix_epoch());
        // An age's label and its tint, read off one clock.
        let age = |time: Option<i64>| {
            let age = Age::of(time, now);
            (
                age.map_or_else(|| tuikit::glyphs::marks::NO_VALUE.to_string(), Age::label),
                age.map(Age::recency),
            )
        };
        let turn_tag = |hit: &Hit| {
            crate::tui::widgets::search::turn_tag(
                hit.turn_index,
                crate::tui::rows::search::kind_word(hit.kind),
            )
        };

        // Sessions and panes are ranked together: more matches, then the
        // newer, then sessions before panes.
        let mut units: Vec<Unit> = Vec::new();
        let mut session_hits: Vec<(AgentSessionId, &crate::viewer::search::SessionHits)> = self
            .snapshot_sessions()
            .into_iter()
            .filter_map(|session| {
                let hits = content
                    .hits
                    .get(&session.harness_key, &session.session_key)?;
                Some((session, hits))
            })
            .collect();
        session_hits.sort_by(|a, b| {
            crate::viewer::search::cmp_sessions(a.1, b.1).then_with(|| {
                (&a.0.harness_key, &a.0.session_key).cmp(&(&b.0.harness_key, &b.0.session_key))
            })
        });
        let flat_ready = content.group == Grouping::Flat && !content.details.is_empty();
        let mut flat_rows: Vec<FlatRow> = Vec::new();
        for (session, hits) in session_hits {
            let label = session_label(snapshot, &session);
            let chip = crate::tui::rows::harness_label(&session.harness_key);
            let latest = hits.latest.map(|time| time.timestamp());
            let row_for = |hit: &Hit, turn: Option<String>| {
                let (age, recency) = age(hit.timestamp.map(|time| time.timestamp()).or(latest));
                ContentRow {
                    target: ContentTarget::Session {
                        session: session.clone(),
                        turn_index: hit.turn_index,
                    },
                    label: label.clone(),
                    chip: chip.clone(),
                    hits: hits.count,
                    turn,
                    age,
                    recency,
                    snippet: hit.snippet.clone(),
                    highlight: Some(hit.highlight.clone()),
                }
            };
            let details = (content.group.lists_messages())
                .then(|| {
                    content
                        .details
                        .get(&session.harness_key, &session.session_key)
                })
                .flatten();
            if flat_ready {
                for hit in details.unwrap_or_default() {
                    flat_rows.push((
                        hit.timestamp.map(|time| time.timestamp()),
                        (session.harness_key.clone(), session.session_key.clone()),
                        hit.turn_index,
                        row_for(hit, Some(turn_tag(hit))),
                    ));
                }
                continue;
            }
            let rows = match details {
                Some(details) if content.group == Grouping::Message => {
                    let mut ordered: Vec<&Hit> = details.iter().collect();
                    ordered.sort_by(|a, b| cmp_hits(a, b));
                    ordered
                        .into_iter()
                        .map(|hit| row_for(hit, Some(turn_tag(hit))))
                        .collect()
                }
                _ => vec![row_for(&hits.best, None)],
            };
            units.push(Unit {
                count: hits.count,
                time: latest,
                pane: false,
                origin: format!("{}\0{}", session.harness_key, session.session_key),
                rows,
            });
        }
        for pane in content.panes.iter() {
            let label = format!(
                "{} {}.{}",
                pane.pane.mux.session, pane.pane.window_index, pane.pane.pane_index
            );
            let chip = pane.pane.command.clone().unwrap_or_default();
            let captured = Some(pane.captured_at);
            let (pane_age, pane_recency) = age(captured);
            let row_for = |hit: &crate::content_search::PaneHit, tag: Option<String>| ContentRow {
                target: ContentTarget::Pane {
                    pane: pane.pane.clone(),
                    line: hit.line,
                },
                label: label.clone(),
                chip: chip.clone(),
                hits: pane.count,
                turn: tag,
                age: pane_age.clone(),
                recency: pane_recency,
                snippet: hit.snippet.clone(),
                highlight: Some(hit.highlight.clone()),
            };
            let newest = pane.hits.last();
            if content.group == Grouping::Flat {
                for hit in pane.hits.iter().rev() {
                    flat_rows.push((
                        captured,
                        (pane.pane.mux.native_id(), pane.pane.pane_id.clone()),
                        usize::MAX - hit.line,
                        row_for(hit, Some(crate::tui::widgets::search::line_tag(hit.line))),
                    ));
                }
                continue;
            }
            let rows = if content.group == Grouping::Message {
                pane.hits
                    .iter()
                    .rev()
                    .map(|hit| row_for(hit, Some(crate::tui::widgets::search::line_tag(hit.line))))
                    .collect()
            } else {
                newest.map(|hit| row_for(hit, None)).into_iter().collect()
            };
            units.push(Unit {
                count: pane.count,
                time: captured,
                pane: true,
                origin: format!("{}\0{}", pane.pane.mux.native_id(), pane.pane.pane_id),
                rows,
            });
        }
        if flat_ready || content.group == Grouping::Flat && !flat_rows.is_empty() {
            // Every message and scrollback line, newest first.
            flat_rows.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| {
                        (matches!(a.3.target, ContentTarget::Pane { .. }))
                            .cmp(&matches!(b.3.target, ContentTarget::Pane { .. }))
                    })
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(&b.2))
            });
            view.rows = flat_rows
                .into_iter()
                .map(|(_, _, _, row)| row)
                .take(OVERLAY_RESULTS)
                .collect();
            return view;
        }
        units.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| b.time.cmp(&a.time))
                .then_with(|| a.pane.cmp(&b.pane))
                .then_with(|| a.origin.cmp(&b.origin))
        });
        for unit in units {
            for row in unit.rows {
                if view.rows.len() >= OVERLAY_RESULTS {
                    return view;
                }
                view.rows.push(row);
            }
        }
        view
    }

    /// Start (or stop) the scan the overlay owes once its query has
    /// rested. With `force` the debounce is skipped. Called every loop
    /// pass, and after each key where there is no loop.
    pub fn search_overlay_tick(&mut self, now: Instant, force: bool) {
        use crate::tui::widgets::search::DueSearch;
        let Some(state) = self.search_overlay_mut() else {
            return;
        };
        let Some(due) = state.take_due_search(now, force) else {
            return;
        };
        let msg = match due {
            DueSearch::Preview {
                text,
                kinds,
                group,
                reach,
            } => Msg::PreviewContentSearch {
                text,
                kinds,
                group,
                reach,
            },
            DueSearch::Clear => Msg::DiscardContentPreview,
        };
        let _ = self.update(msg);
    }

    /// The search overlay `state` has closed: remember how it was set,
    /// and keep the search it ran.
    pub(crate) fn search_closed(
        &mut self,
        state: &crate::tui::widgets::search::SearchOverlayState,
    ) {
        self.search_memory = Some(SearchMemory {
            group: state.group(),
            kinds: state.kinds(),
            panes: state.panes(),
        });
        self.commit_modal_search();
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "content_preview_tests.rs"]
mod preview_tests;

#[cfg(test)]
#[path = "content_history_tests.rs"]
mod history_tests;

#[cfg(test)]
#[path = "content_state_tests.rs"]
mod state_tests;
