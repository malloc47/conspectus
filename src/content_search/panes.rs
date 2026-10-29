//! Searching the scrollback of live mux panes (ADR 0149).
//!
//! The source lists every pane with `list-panes -a` (one call per tmux
//! server), reads each pane's scrollback through
//! [`MuxBackend::pane_history`] on worker threads, and matches it line by
//! line with the shared matcher. Scrollback is read as plain text, capped
//! at [`MAX_PANE_LINES`] lines and [`MAX_PANE_BYTES`] bytes per pane, and
//! held in memory only while a pane is being matched: nothing is
//! persisted, put in the graph, or sent to the daemon.
//!
//! A pane that mirrors an agent session would repeat that session's
//! matches, so hits that the attributed sessions already contain are
//! dropped ([`super::dedup`]). Which sessions are attributed to which mux
//! comes from the caller, built by [`attribute_panes`].

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use crate::discovery::tmux::{MuxBackend, TMUX_BACKEND, TmuxCaptureOutcome, TmuxOutcome};
use crate::model::{AgentSessionId, GraphNode, GraphSnapshot, NodeId};
use tuikit::text::{find_ci, fit_snippet, strip_ansi};

use crate::viewer::search::{ContentQuery, SNIPPET_MAX_CELLS};

use super::dedup::{drop_duplicates, matching_bodies, normalize_for_compare};
use super::scan::worker_count;
use super::source::{
    ContentResult, ContentSource, MAX_PANE_HITS_PER_ENTRY, MuxRef, PaneHit, PaneHits, PaneRef,
    SourceEvent, SourceKind,
};
use super::{CancelToken, Progress, ScanOptions, ScanSession, SessionKey, plan_sessions};

/// Lines of scrollback matched per pane.
pub const MAX_PANE_LINES: usize = 5000;

/// Bytes of scrollback matched per pane.
pub const MAX_PANE_BYTES: usize = 2 * 1024 * 1024;

/// The `list-panes -a` format. Tab separated, like the session listing.
pub const PANE_LIST_FORMAT: &str =
    "#{session_name}\t#{pane_id}\t#{window_index}\t#{pane_index}\t#{pane_current_command}";

/// The sessions attributed to each mux, keyed by the mux's graph id
/// (`MuxSessionId::native_id`). Built from the snapshot by
/// [`attribute_panes`]; tests build it by hand.
#[derive(Clone, Default)]
pub struct Attribution {
    by_mux: BTreeMap<String, Vec<ScanSession>>,
}

impl Attribution {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attribute `session` to the mux with graph id `mux`.
    pub fn add(&mut self, mux: impl Into<String>, session: ScanSession) {
        self.by_mux.entry(mux.into()).or_default().push(session);
    }

    pub fn sessions_of(&self, mux: &str) -> &[ScanSession] {
        self.by_mux.get(mux).map_or(&[], Vec::as_slice)
    }
}

/// Attribute each agent session in `snapshot` to the live mux the
/// resolver prefers for it (an active `linked_to_mux` relation). Sessions
/// whose harness has no transcript parser are left out: there is nothing
/// to deduplicate against.
pub fn attribute_panes(snapshot: &GraphSnapshot) -> Attribution {
    let mut linked: BTreeMap<String, Vec<AgentSessionId>> = BTreeMap::new();
    for node in snapshot.live_nodes() {
        let GraphNode::AgentSession(agent) = node else {
            continue;
        };
        let session = NodeId::AgentSession(agent.id.clone());
        if let Some(mux) = snapshot.preferred_mux_for(&session) {
            linked
                .entry(mux.native_id)
                .or_default()
                .push(agent.id.clone());
        }
    }
    let mut attribution = Attribution::new();
    for (mux, ids) in linked {
        for session in plan_sessions(ids.iter()).sessions {
            attribution.add(mux.clone(), session);
        }
    }
    attribution
}

/// The tmux servers worth listing: the default one, and every named
/// socket a live mux in `snapshot` lives on.
pub fn sockets_in(snapshot: &GraphSnapshot) -> Vec<Option<String>> {
    let mut sockets: BTreeSet<Option<String>> = BTreeSet::from([None]);
    for node in snapshot.live_nodes() {
        if let GraphNode::MuxSession(mux) = node
            && mux.backend == TMUX_BACKEND
        {
            sockets.insert(mux.socket.clone().filter(|socket| socket != "default"));
        }
    }
    sockets.into_iter().collect()
}

/// A listed pane, before it is captured.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListedPane {
    pub pane: PaneRef,
}

/// Parse a `list-panes -a` listing made with [`PANE_LIST_FORMAT`].
pub fn parse_pane_list(stdout: &str, socket: Option<&str>) -> Vec<ListedPane> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let session = fields.next()?.trim();
            let pane_id = fields.next()?.trim();
            if session.is_empty() || pane_id.is_empty() {
                return None;
            }
            let window_index = fields.next().unwrap_or_default().trim().to_string();
            let pane_index = fields.next().unwrap_or_default().trim().to_string();
            let command = fields
                .next()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string);
            Some(ListedPane {
                pane: PaneRef {
                    mux: MuxRef {
                        backend: TMUX_BACKEND.to_string(),
                        socket: socket.map(str::to_string),
                        session: session.to_string(),
                    },
                    pane_id: pane_id.to_string(),
                    window_index,
                    pane_index,
                    command,
                },
            })
        })
        .collect()
}

/// A pane's scrollback as lines, after the caps.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Scrollback {
    pub lines: Vec<String>,
    /// Older scrollback was dropped to fit the caps.
    pub capped: bool,
}

/// Turn captured text into lines: escapes stripped, trailing blank
/// lines dropped, and the newest [`MAX_PANE_LINES`] lines within
/// [`MAX_PANE_BYTES`] kept.
pub fn scrollback_lines(text: &str) -> Scrollback {
    let plain = strip_ansi(text);
    let mut lines: Vec<&str> = plain.lines().map(str::trim_end).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let mut capped = false;
    if lines.len() > MAX_PANE_LINES {
        lines.drain(..lines.len() - MAX_PANE_LINES);
        capped = true;
    }
    let mut bytes = 0usize;
    let mut keep_from = lines.len();
    for (i, line) in lines.iter().enumerate().rev() {
        bytes += line.len() + 1;
        if bytes > MAX_PANE_BYTES {
            capped = true;
            break;
        }
        keep_from = i;
    }
    Scrollback {
        lines: lines[keep_from..]
            .iter()
            .map(|l| (*l).to_string())
            .collect(),
        capped,
    }
}

/// Searches the scrollback of every pane on the given tmux servers.
pub struct PaneSource {
    backend: Arc<dyn MuxBackend>,
    sockets: Vec<Option<String>>,
    attribution: Attribution,
    /// Epoch seconds recorded as the capture time of every hit.
    captured_at: i64,
    /// Search only the panes of these muxes (graph ids); `None` searches
    /// every pane on the listed servers.
    only: Option<BTreeSet<String>>,
}

impl PaneSource {
    pub fn new(
        backend: Arc<dyn MuxBackend>,
        sockets: Vec<Option<String>>,
        attribution: Attribution,
    ) -> Self {
        Self {
            backend,
            sockets,
            attribution,
            captured_at: crate::discovery::current_epoch(),
            only: None,
        }
    }

    /// Search only the panes of the muxes with these graph ids.
    #[must_use]
    pub fn only_muxes(mut self, muxes: BTreeSet<String>) -> Self {
        self.only = Some(muxes);
        self
    }

    /// Fix the capture time, for tests.
    pub fn at(mut self, captured_at: i64) -> Self {
        self.captured_at = captured_at;
        self
    }

    /// Every pane on every server, and how many listings failed.
    fn list(&self) -> (Vec<ListedPane>, usize) {
        let mut panes = Vec::new();
        let mut failed = 0;
        for socket in &self.sockets {
            match self
                .backend
                .list_panes_on(socket.as_deref(), PANE_LIST_FORMAT)
            {
                Ok(TmuxOutcome::Sessions(stdout)) => {
                    panes.extend(parse_pane_list(&stdout, socket.as_deref()));
                }
                // No server running there: no panes, and nothing wrong.
                Ok(TmuxOutcome::Unavailable(_)) => {}
                Ok(TmuxOutcome::Failed { .. }) | Err(_) => failed += 1,
            }
        }
        if let Some(only) = &self.only {
            panes.retain(|listed| only.contains(&listed.pane.mux.native_id()));
        }
        (panes, failed)
    }
}

/// The transcript text a pane is compared against, parsed at most once
/// per run per session.
struct SessionBodies {
    cells: HashMap<SessionKey, BodyCell>,
}

/// A session and its matching bodies, filled on first use.
type BodyCell = (ScanSession, OnceLock<Option<Vec<String>>>);

impl SessionBodies {
    fn new(sessions: impl Iterator<Item = ScanSession>) -> Self {
        Self {
            cells: sessions
                .map(|s| (s.key.clone(), (s, OnceLock::new())))
                .collect(),
        }
    }

    /// The normalized bodies of `session`'s turns matching `needle`, or
    /// `None` when its transcript can't be read.
    fn of(&self, key: &SessionKey, needle: &str) -> Option<&[String]> {
        let (session, cell) = self.cells.get(key)?;
        cell.get_or_init(|| {
            let document = session.parser.read(&session.locator).ok()?;
            Some(matching_bodies(&document, needle))
        })
        .as_deref()
    }
}

impl ContentSource for PaneSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Pane
    }

    fn run(
        &self,
        query: &ContentQuery,
        options: &ScanOptions,
        cancel: &CancelToken,
        emit: &(dyn Fn(SourceEvent) + Sync),
    ) {
        let Some(needle) = query.needle() else {
            return;
        };
        let (panes, failed_listings) = self.list();
        let total = panes.len() + failed_listings;
        // Parse only the sessions of muxes that have panes.
        let muxes: BTreeSet<String> = panes.iter().map(|p| p.pane.mux.native_id()).collect();
        let bodies = SessionBodies::new(
            muxes
                .iter()
                .flat_map(|mux| self.attribution.sessions_of(mux).iter().cloned()),
        );

        let scanned = AtomicUsize::new(failed_listings);
        let skipped = AtomicUsize::new(failed_listings);
        let next = AtomicUsize::new(0);
        let progress = || Progress {
            scanned: scanned.load(Ordering::Relaxed),
            total,
            skipped: skipped.load(Ordering::Relaxed),
        };
        emit(SourceEvent::Progress(progress()));
        let workers = worker_count(options.workers).min(panes.len().max(1));
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        if cancel.is_cancelled() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(listed) = panes.get(i) else {
                            break;
                        };
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                self.search_pane(&listed.pane, needle, &bodies)
                            }))
                            .unwrap_or(PaneOutcome::Skipped);
                        match outcome {
                            PaneOutcome::Matched(hits) => {
                                emit(SourceEvent::Result(ContentResult::Pane(*hits)));
                            }
                            PaneOutcome::Skipped => {
                                skipped.fetch_add(1, Ordering::Relaxed);
                            }
                            PaneOutcome::NoMatch => {}
                        }
                        scanned.fetch_add(1, Ordering::Relaxed);
                        emit(SourceEvent::Progress(progress()));
                    }
                });
            }
        });
    }
}

enum PaneOutcome {
    NoMatch,
    Matched(Box<PaneHits>),
    Skipped,
}

impl PaneSource {
    fn search_pane(&self, pane: &PaneRef, needle: &str, bodies: &SessionBodies) -> PaneOutcome {
        let capture = self
            .backend
            .pane_history(pane.mux.socket.as_deref(), &pane.pane_id);
        let text = match capture {
            Ok(TmuxCaptureOutcome::Captured(capture)) => capture.text,
            // Everything else means the pane could not be read now.
            _ => return PaneOutcome::Skipped,
        };
        let scrollback = scrollback_lines(&text);
        let matched: Vec<usize> = scrollback
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| find_ci(line, needle).is_some())
            .map(|(i, _)| i)
            .collect();
        if matched.is_empty() {
            return PaneOutcome::NoMatch;
        }
        let normalized: Vec<String> = scrollback
            .lines
            .iter()
            .map(|line| normalize_for_compare(line))
            .collect();
        // Everything the attributed sessions say that matches the query.
        let attributed = self.attribution.sessions_of(&pane.mux.native_id());
        let session_bodies: Vec<String> = attributed
            .iter()
            .filter_map(|session| bodies.of(&session.key, needle))
            .flat_map(|bodies| bodies.iter().cloned())
            .collect();
        let (kept, deduplicated) = drop_duplicates(&normalized, &matched, &session_bodies);
        if kept.is_empty() {
            return PaneOutcome::NoMatch;
        }
        let count = kept.len();
        let newest = &kept[count.saturating_sub(MAX_PANE_HITS_PER_ENTRY)..];
        let hits = newest
            .iter()
            .map(|&i| pane_hit(i, &scrollback.lines[i], needle))
            .collect();
        PaneOutcome::Matched(Box::new(PaneHits {
            pane: pane.clone(),
            count,
            hits,
            deduplicated,
            captured_at: self.captured_at,
            capture_capped: scrollback.capped,
        }))
    }
}

/// A hit for line `index` (0-based) of the scrollback.
fn pane_hit(index: usize, line: &str, needle: &str) -> PaneHit {
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let (text, highlight) = match find_ci(&collapsed, needle) {
        Some(start) => {
            let snippet = fit_snippet(
                &collapsed,
                start..start + needle.len(),
                SNIPPET_MAX_CELLS,
                &[],
            );
            (snippet.text, snippet.highlight.unwrap_or(0..0))
        }
        None => (
            fit_snippet(&collapsed, 0..0, SNIPPET_MAX_CELLS, &[]).text,
            0..0,
        ),
    };
    PaneHit {
        line: index + 1,
        snippet: text,
        highlight: clamp(highlight),
    }
}

fn clamp(range: Range<usize>) -> Range<usize> {
    range.start.min(range.end)..range.end
}
