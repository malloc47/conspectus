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
use crate::filter::{ContentFilter, ContentHitMap};
use crate::tui::Effect;
use crate::viewer::search::ContentKinds;

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
    hits: Arc<ContentHitMap>,
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

impl App {
    /// The active content search, if any.
    pub fn content(&self) -> Option<&ContentSearch> {
        self.content.as_ref()
    }

    /// The row predicate for the active search, for tree building.
    pub fn content_filter(&self) -> Option<ContentFilter> {
        self.content.as_ref().map(ContentSearch::filter)
    }

    /// The generation of the scan whose results are wanted now; `None`
    /// when no search is active. A runtime holding a different one
    /// should stop it.
    pub fn content_generation(&self) -> Option<u64> {
        self.content.as_ref().map(|content| content.generation)
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

    pub(super) fn reduce_content(&mut self, msg: Msg) -> Vec<Effect> {
        let mut effects = Vec::new();
        match msg {
            Msg::StartContentSearch { text, kinds } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return self.reduce_content(Msg::ClearContentSearch);
                }
                if !self.config.live_preview_enabled {
                    let note = "Content search is off while live preview is disabled";
                    self.status_message = Some(note.to_string());
                    effects.push(Effect::Toast(note.to_string()));
                    return effects;
                }
                self.content = Some(ContentSearch {
                    text,
                    kinds,
                    generation: 0,
                    status: ContentStatus::Running(Progress::default()),
                    hits: Arc::new(ContentHitMap::default()),
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
                self.rebuild_tree_in_place();
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

    /// Drop the content search and stop its scan.
    fn clear_content(&mut self) {
        if self.content.take().is_some() {
            // A new generation makes the runtime cancel whatever it runs.
            self.content_generation += 1;
        }
        self.scan_request = None;
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

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
