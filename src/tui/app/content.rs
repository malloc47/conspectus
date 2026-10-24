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

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::content_search::{Progress, SessionKey};
use crate::filter::{ContentDetailMap, ContentFilter, ContentHitMap};
use crate::tui::Effect;
use crate::viewer::search::{ContentKinds, Grouping};

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
}

impl ContentSearch {
    /// Sessions that matched so far.
    pub fn hits(&self) -> &ContentHitMap {
        &self.hits
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

    /// Ask for the matching turns of every session found so far, when
    /// the per-message grouping needs them and the scan has finished.
    fn queue_details(&mut self) {
        let Some(content) = &self.content else {
            return;
        };
        if content.group != Grouping::Message || !content.status.is_done() {
            return;
        }
        let sessions: Vec<AgentSessionId> = self
            .snapshot_sessions()
            .into_iter()
            .filter(|session| {
                content
                    .hits
                    .get(&session.harness_key, &session.session_key)
                    .is_some()
                    && content
                        .details
                        .get(&session.harness_key, &session.session_key)
                        .is_none()
            })
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
        content.status = ContentStatus::Running(Progress {
            scanned: 0,
            total: sessions.len(),
            skipped: 0,
        });
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
            let note = "Content search is off while live preview is disabled";
            self.status_message = Some(note.to_string());
            return Some(note.to_string());
        }
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
                self.rebuild_tree_in_place();
            }
            Msg::RescanContent => {
                let sessions = self.snapshot_sessions();
                if let Some(content) = self.content.as_mut() {
                    content.scanned.clear();
                    content.staging = Some(ContentHitMap::default());
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
                for (key, hits) in batch {
                    let SessionKey {
                        harness_key,
                        session_key,
                    } = key;
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
                if changed {
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
        for row in &self.tree.rows {
            if row.expandable && self.search_auto_expanded.insert(row.id.clone()) {
                self.expanded.insert(row.id.clone());
            }
        }
    }

    /// Drop the content search and stop its scan.
    fn clear_content(&mut self) {
        self.content_stash = None;
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
            view.unavailable = Some("Content search is off while live preview is disabled");
            return view;
        }
        let Some(content) = &self.content else {
            return view;
        };
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
        for (session, hits) in found {
            if view.rows.len() >= OVERLAY_RESULTS {
                break;
            }
            let node = NodeId::AgentSession(session.clone());
            let alias = snapshot.aliases.get(&node);
            let (title, mux_host) = match snapshot.find_node(&node) {
                Some(crate::model::GraphNode::AgentSession(s)) => {
                    (s.title.clone(), snapshot.mux_host_of(&node))
                }
                _ => (None, None),
            };
            let label = crate::aliases::session_display_name(
                alias,
                title.as_deref(),
                mux_host.as_ref().map(|host| host.name.as_str()),
            )
            .map_or_else(
                || session.session_key.chars().take(8).collect(),
                str::to_string,
            );
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
