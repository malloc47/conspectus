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

use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::filter::{ContentDetailMap, ContentFilter, ContentHitMap};
use crate::tui::Effect;
use crate::viewer::search::{ContentKinds, Grouping};

/// What the interface says when a content search cannot start because
/// live preview is off. One text for every place that explains it.
pub const CONTENT_OFF_NOTICE: &str = "Content search is off while live preview is disabled";

/// How the Search view lists results, in the Controls overlay.
pub fn grouping_description(group: Grouping) -> &'static str {
    match group {
        Grouping::Session => "one row per session",
        Grouping::Message => "messages under their session",
        Grouping::Flat => "every message, newest first",
    }
}

/// How the `/` overlay says what its results are.
pub fn grouping_overlay_label(group: Grouping) -> &'static str {
    match group {
        Grouping::Session => "by session",
        Grouping::Message => "by message",
        Grouping::Flat => "by message, flat",
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

/// The active content query and what it has found so far.
#[derive(Clone, Debug)]
pub struct ContentSearch {
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
    hits: Arc<ContentHitMap>,
    /// The matching turns of each session, read on demand for the
    /// per-message grouping.
    details: Arc<ContentDetailMap>,
    /// A full rescan collects here and replaces `hits` when it ends, so
    /// the list does not empty and refill while it runs.
    staging: Option<ContentHitMap>,
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
}

impl ContentSearch {
    /// Sessions that matched so far.
    pub fn hits(&self) -> &ContentHitMap {
        &self.hits
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
                format!(
                    "{found} {}",
                    if found == 1 { "session" } else { "sessions" }
                )
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
            note.push_str(&format!(" · scanned {}", age_label(age)));
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
        }
    }
}

/// `just now`, or `2m ago`, `1h ago`.
fn age_label(secs: u64) -> String {
    match secs {
        0..=4 => "just now".to_string(),
        5..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        _ => format!("{}h ago", secs / 3600),
    }
}

/// What to scan, left for the runtime by the reducer.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanRequest {
    pub generation: u64,
    pub text: String,
    pub kinds: ContentKinds,
    /// The sessions to read.
    pub sessions: Vec<AgentSessionId>,
}

/// The matching turns to read for the Search view's per-message
/// grouping, left for the runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailRequest {
    pub generation: u64,
    pub text: String,
    pub kinds: ContentKinds,
    /// The sessions that matched.
    pub sessions: Vec<AgentSessionId>,
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

    /// The generation of the scan whose results are wanted now; `None`
    /// when no search is active. A runtime holding a different one
    /// should stop it.
    pub fn content_generation(&self) -> Option<u64> {
        self.content.as_ref().map(|content| content.generation)
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
        let Some(content) = &self.content else {
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
            .filter(|row| self.expanded.contains(&row.id))
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
        self.detail_request = Some(DetailRequest {
            generation: content.generation,
            text: content.text.clone(),
            kinds: content.kinds,
            sessions,
        });
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

    /// Start a full scan of the held snapshot for the active query.
    fn queue_scan(&mut self, sessions: Vec<AgentSessionId>) {
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
        self.scan_request = Some(ScanRequest {
            generation: content.generation,
            text: content.text.clone(),
            kinds: content.kinds,
            sessions,
        });
    }

    /// After a refresh landed: scan the sessions that appeared since the
    /// last scan was planned. A scan still reading checks again when it
    /// ends, except one that was waiting for the first snapshot.
    pub(super) fn scan_new_sessions(&mut self) {
        let Some(content) = &self.content else {
            return;
        };
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
            self.queue_scan(new);
        }
    }

    /// Start a scan for `text`. Returns the reason it did not start.
    fn begin_content(
        &mut self,
        text: String,
        kinds: ContentKinds,
        group: Grouping,
        applied: bool,
    ) -> Option<String> {
        if !self.config.live_preview_enabled {
            return Some(CONTENT_OFF_NOTICE.to_string());
        }
        self.search_auto_expanded.clear();
        self.content = Some(ContentSearch {
            text,
            kinds,
            generation: 0,
            status: ContentStatus::Running(Progress::default()),
            group,
            applied,
            hits: Arc::new(ContentHitMap::default()),
            details: Arc::new(ContentDetailMap::default()),
            staging: None,
            scanned: BTreeSet::new(),
            carried: Progress::default(),
            finished_at: None,
            view_cache: RefCell::new(None),
        });
        let sessions = self.snapshot_sessions();
        if sessions.is_empty() && self.handle.is_none() {
            // No snapshot yet: the first refresh starts the scan.
            self.content_generation += 1;
            if let Some(content) = self.content.as_mut() {
                content.generation = self.content_generation;
            }
        } else {
            self.queue_scan(sessions);
        }
        self.start_search_spinner();
        None
    }

    pub(super) fn reduce_content(&mut self, msg: Msg) -> Vec<Effect> {
        let mut effects = Vec::new();
        match msg {
            Msg::StartContentSearch { text, kinds } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return self.reduce_content(Msg::ClearContentSearch);
                }
                if let Some(note) = self.begin_content(text, kinds, self.config.search.group, true)
                {
                    effects.push(Effect::Toast(note));
                    return effects;
                }
                self.content_stash = None;
                self.rebuild_tree_in_place();
            }
            Msg::PreviewContentSearch { text, kinds, group } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return self.reduce_content(Msg::DiscardContentPreview);
                }
                let displaced = self
                    .content
                    .take_if(|content| content.applied)
                    .or_else(|| self.content_stash.take());
                if let Some(note) = self.begin_content(text, kinds, group, false) {
                    // Nothing started: the search that was active stays.
                    self.content = displaced;
                    effects.push(Effect::Toast(note));
                } else {
                    self.content_stash = displaced;
                }
            }
            Msg::ApplyContentSearch {
                text,
                kinds,
                group,
                show_results,
            } => {
                let text = text.trim().to_string();
                let same = self.content.as_ref().is_some_and(|content| {
                    content.text == text && content.kinds == kinds && content.group == group
                });
                if same {
                    if let Some(content) = self.content.as_mut() {
                        content.applied = true;
                    }
                    self.content_stash = None;
                    self.rebuild_tree_in_place();
                } else if let Some(note) = self.begin_content(text, kinds, group, true) {
                    effects.push(Effect::Toast(note));
                    return effects;
                } else {
                    self.content_stash = None;
                    self.rebuild_tree_in_place();
                }
                if show_results {
                    effects.extend(self.update(Msg::SwitchView(View::Search)));
                }
            }
            Msg::DiscardContentPreview => {
                if self
                    .content
                    .as_ref()
                    .is_some_and(|content| !content.applied)
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
                if let Some(content) = self.content.as_mut() {
                    content.group = group;
                }
                self.queue_details();
                self.rebuild_tree_in_place();
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
                if content.applied {
                    self.rebuild_tree_in_place();
                }
            }
            Msg::RescanContent => {
                if self.content.is_some() {
                    let sessions = self.snapshot_sessions();
                    if let Some(content) = self.content.as_mut() {
                        content.scanned.clear();
                        content.carried = Progress::default();
                        content.staging = Some(ContentHitMap::default());
                        // Turns read for the old results are read again.
                        content.details = Arc::new(ContentDetailMap::default());
                    }
                    self.queue_scan(sessions);
                    self.start_search_spinner();
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
                    content.status = ContentStatus::Done(progress);
                    content.finished_at = Some(Instant::now());
                } else {
                    content.status = ContentStatus::Running(progress);
                }
                if done {
                    self.update_in_flight(InFlightKind::Search, None);
                    self.scan_new_sessions();
                    self.queue_details();
                } else {
                    let label = format!("Searching {}/{}", progress.scanned, progress.total);
                    self.update_in_flight(InFlightKind::Search, Some(label));
                }
                // A preview in the overlay does not change the lists.
                if changed && self.content.as_ref().is_some_and(|c| c.applied) {
                    self.rebuild_tree_in_place();
                }
            }
            _ => {}
        }
        effects
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
            .filter(|row| row.expandable && is_session(row) && self.expanded.contains(&row.id))
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
            self.expanded.insert(row.id.clone());
        }
        self.queue_details();
    }

    /// Drop the content search and stop its scan.
    fn clear_content(&mut self) {
        self.content_stash = None;
        self.search_auto_expanded.clear();
        if self.content.take().is_some() {
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

/// What the `/` overlay was left set to, for the next time it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchMemory {
    pub scope: crate::tui::widgets::search::SearchScope,
    pub group: Grouping,
    pub kinds: ContentKinds,
}

/// The most results the overlay lists.
pub const OVERLAY_RESULTS: usize = 50;

impl App {
    /// What the contents scope of the `/` overlay shows: the best
    /// matches of the running search (or, with the per-message
    /// grouping, its matching turns), ranked as the Search view ranks.
    pub fn content_view(&self) -> crate::tui::widgets::search::ContentView {
        use crate::tui::widgets::search::ContentView;
        let mut view = ContentView::default();
        if !self.config.live_preview_enabled {
            view.unavailable = Some(CONTENT_OFF_NOTICE);
            return view;
        }
        let Some(content) = &self.content else {
            return view;
        };
        let key = ViewKey {
            generation: content.generation,
            hits: content.hits.len(),
            details: content.details.len(),
            progress: content.status.progress(),
            done: content.status.is_done(),
            group: content.group,
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

    /// Rank and describe the matches for the overlay. Costly (it walks
    /// every session), so [`App::content_view`] caches it.
    fn build_content_view(
        &self,
        content: &ContentSearch,
        mut view: crate::tui::widgets::search::ContentView,
    ) -> crate::tui::widgets::search::ContentView {
        let progress = content.status.progress();
        view.scanned = progress.scanned;
        view.total = progress.total;
        view.skipped = progress.skipped;
        view.done = content.status.is_done();
        let Some(handle) = &self.handle else {
            return view;
        };
        let snapshot = handle.snapshot();
        let mut found: Vec<(AgentSessionId, &crate::viewer::search::SessionHits)> = self
            .snapshot_sessions()
            .into_iter()
            .filter_map(|session| {
                let hits = content
                    .hits
                    .get(&session.harness_key, &session.session_key)?;
                Some((session, hits))
            })
            .collect();
        found.sort_by(|a, b| {
            crate::viewer::search::cmp_sessions(a.1, b.1).then_with(|| {
                (&a.0.harness_key, &a.0.session_key).cmp(&(&b.0.harness_key, &b.0.session_key))
            })
        });
        if content.group == Grouping::Flat && !content.details.is_empty() {
            // Every message of the sessions read so far, as one list.
            let mut all: Vec<(&AgentSessionId, &crate::viewer::search::Hit)> = Vec::new();
            for (session, _) in &found {
                for hit in content
                    .details
                    .get(&session.harness_key, &session.session_key)
                    .unwrap_or_default()
                {
                    all.push((session, hit));
                }
            }
            all.sort_by(|a, b| {
                crate::viewer::search::cmp_hits_across_sessions(
                    (&(&a.0.harness_key, &a.0.session_key), a.1),
                    (&(&b.0.harness_key, &b.0.session_key), b.1),
                )
            });
            for (session, hit) in all.into_iter().take(OVERLAY_RESULTS) {
                let hits = content
                    .hits
                    .get(&session.harness_key, &session.session_key)
                    .map_or(0, |hits| hits.count);
                view.rows.push(crate::tui::widgets::search::ContentRow {
                    session: session.clone(),
                    label: session_label(snapshot, session),
                    harness: crate::tui::rows::harness_label(&session.harness_key),
                    hits,
                    turn: Some(format!(
                        "turn {} · {}",
                        hit.turn_index + 1,
                        crate::tui::rows::search::kind_word(hit.kind)
                    )),
                    snippet: hit.snippet.clone(),
                    highlight: Some(hit.highlight.clone()),
                    turn_index: hit.turn_index,
                });
            }
            return view;
        }
        for (session, hits) in found {
            if view.rows.len() >= OVERLAY_RESULTS {
                break;
            }
            let label = session_label(snapshot, &session);
            let details = (content.group == Grouping::Message)
                .then(|| {
                    content
                        .details
                        .get(&session.harness_key, &session.session_key)
                })
                .flatten();
            let row_for = |hit: &crate::viewer::search::Hit, turn: Option<String>| {
                crate::tui::widgets::search::ContentRow {
                    session: session.clone(),
                    label: label.clone(),
                    harness: crate::tui::rows::harness_label(&session.harness_key),
                    hits: hits.count,
                    turn,
                    snippet: hit.snippet.clone(),
                    highlight: Some(hit.highlight.clone()),
                    turn_index: hit.turn_index,
                }
            };
            match details {
                Some(details) => {
                    let mut ordered: Vec<&crate::viewer::search::Hit> = details.iter().collect();
                    ordered.sort_by(|a, b| crate::viewer::search::cmp_hits(a, b));
                    for hit in ordered {
                        if view.rows.len() >= OVERLAY_RESULTS {
                            break;
                        }
                        let turn = format!(
                            "turn {} · {}",
                            hit.turn_index + 1,
                            crate::tui::rows::search::kind_word(hit.kind)
                        );
                        view.rows.push(row_for(hit, Some(turn)));
                    }
                }
                None => view.rows.push(row_for(&hits.best, None)),
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
            DueSearch::Preview { text, kinds, group } => {
                Msg::PreviewContentSearch { text, kinds, group }
            }
            DueSearch::Clear => Msg::DiscardContentPreview,
        };
        let _ = self.update(msg);
    }

    /// Remember how the overlay was set, as it closes.
    pub(crate) fn remember_search_overlay(&mut self) {
        if let Some(state) = self.search_overlay() {
            self.search_memory = Some(SearchMemory {
                scope: state.scope(),
                group: state.group(),
                kinds: state.kinds(),
            });
        }
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "content_preview_tests.rs"]
mod preview_tests;

#[cfg(test)]
#[path = "content_state_tests.rs"]
mod state_tests;
