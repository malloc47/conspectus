//! Preview sources, their in-memory cache, and their loaders for the
//! right panel's Preview zone (ADR 0112 §5).
//!
//! A preview is a [`PreviewSource`] resolved from the selection. Its
//! content loads when the selection lands on it and sits in
//! [`PreviewStore`], in memory only (ADR 0086). A source reloads on
//! selection change, on `r`, and when a tmux hand-off returns, showing
//! its previous content until the reload lands. Interval-driven pane
//! refresh is open work (backlog `CSP-185`); until then
//! `RunConfig::mux_preview_interval` is parsed but unused.

mod forge;
mod git;
mod path;
mod process;
mod value;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use ratatui::text::Line;

pub use forge::PrQuery;
pub use git::GitQuery;
pub use value::{glossary_entry, split_command_line, value_lines};

use crate::discovery::tmux::{MuxBackend, PaneCapture, TmuxCaptureOutcome};
use crate::model::{AgentSessionId, GraphNode, MuxSessionId, NodeId};
use crate::tui::app::App;
use crate::tui::explorer::FieldValue;
use crate::tui::rows::{MuxIndicator, RowId, RowKind};
use crate::tui::viewer_bridge::{TranscriptMissing, load_transcript};
use crate::viewer::state::ViewerState;

/// What the Preview zone shows for a selection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PreviewSource {
    /// A tmux pane capture.
    Pane(MuxSessionId),
    /// An agent session's transcript, read by the native viewer.
    Transcript(AgentSessionId),
    /// A path on disk: a directory's listing or a file's contents.
    Path(PathBuf),
    /// A read-only git query: status, history, or a commit.
    Git(GitQuery),
    /// The process tree rooted at a pid.
    Process(i64),
    /// A pull request, read with `gh`.
    PullRequest(PrQuery),
}

impl PreviewSource {
    /// The same source with paths under `from` moved under `to`, for
    /// reading a fixture's previews from a materialized world
    /// (ADR 0113).
    #[cfg(feature = "snapshot")]
    pub fn rebased(&self, from: &std::path::Path, to: &std::path::Path) -> Self {
        let move_path = |path: &std::path::Path| match path.strip_prefix(from) {
            Ok(rest) => to.join(rest),
            Err(_) => path.to_path_buf(),
        };
        match self {
            Self::Transcript(session) => {
                let mut session = session.clone();
                session.state_scope = move_path(std::path::Path::new(&session.state_scope))
                    .display()
                    .to_string();
                Self::Transcript(session)
            }
            Self::Path(path) => Self::Path(move_path(path)),
            Self::Git(query) => Self::Git(match query {
                GitQuery::Status { root } => GitQuery::Status {
                    root: move_path(root),
                },
                GitQuery::Repo { dir } => GitQuery::Repo {
                    dir: move_path(dir),
                },
                GitQuery::Log {
                    dir,
                    rev,
                    ahead_behind,
                } => GitQuery::Log {
                    dir: move_path(dir),
                    rev: rev.clone(),
                    ahead_behind: ahead_behind.clone(),
                },
                GitQuery::Show { dir, commit } => GitQuery::Show {
                    dir: move_path(dir),
                    commit: commit.clone(),
                },
            }),
            Self::Pane(_) | Self::Process(_) | Self::PullRequest(_) => self.clone(),
        }
    }

    /// Whether the source is conversation or terminal content, which
    /// `--no-live-preview` hides (ADR 0112 §6).
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Pane(_) | Self::Transcript(_))
    }

    /// How many entries of this source's kind the cache keeps.
    /// Parsed transcripts can be large, and only the few most recently
    /// previewed are likely to be selected again soon; other loaded
    /// content is capped where it's read.
    fn cache_cap(&self) -> Option<usize> {
        match self {
            Self::Pane(_) => None,
            Self::Transcript(_) => Some(MAX_CACHED_TRANSCRIPTS),
            Self::Path(_) | Self::Git(_) | Self::Process(_) | Self::PullRequest(_) => {
                Some(MAX_CACHED_OTHER)
            }
        }
    }

    fn same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Transcripts kept in the cache at once.
const MAX_CACHED_TRANSCRIPTS: usize = 8;
/// Entries of each other loaded kind kept in the cache at once.
const MAX_CACHED_OTHER: usize = 64;

/// In-memory cache of loaded previews, keyed by source. Plain data —
/// no I/O, no locks — so the reducer can carry it directly on
/// [`crate::tui::app::App`].
#[derive(Debug, Default, Clone)]
pub struct PreviewStore {
    entries: BTreeMap<PreviewSource, PreviewEntry>,
    /// Sources with a load in flight, so the next selection pass
    /// doesn't start another.
    loading: BTreeSet<PreviewSource>,
}

#[derive(Debug, Clone)]
pub struct PreviewEntry {
    pub content: PreviewContent,
    pub captured_at: Option<Instant>,
    /// The source has likely changed since this was loaded. The
    /// content still shows until the reload replaces it.
    pub stale: bool,
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    /// The pane's captured text (from `tmux capture-pane -p -J -e`)
    /// and the width tmux laid it out at.
    Text(PaneCapture),
    /// A parsed transcript, in the viewer state the preview renders it
    /// with, so its composed lines are cached alongside it.
    Transcript(Box<ViewerState>),
    /// Lines a loader composed: a listing, a file's contents.
    Lines(Vec<Line<'static>>),
    /// The runner reported the target doesn't exist.
    NoTarget,
    /// Nothing to load, with the reason: tmux is missing, or the
    /// viewer can't read the session's transcript.
    Unavailable(String),
    /// Loading failed.
    Failed(String),
    /// The runner doesn't support capture (e.g. test fakes).
    Unsupported,
}

impl PartialEq for PreviewContent {
    /// Transcripts compare by document; the viewer state's render
    /// cache is derived from it.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Transcript(a), Self::Transcript(b)) => a.document == b.document,
            (Self::Lines(a), Self::Lines(b)) => a == b,
            (Self::NoTarget, Self::NoTarget) | (Self::Unsupported, Self::Unsupported) => true,
            (Self::Unavailable(a), Self::Unavailable(b)) | (Self::Failed(a), Self::Failed(b)) => {
                a == b
            }
            _ => false,
        }
    }
}

impl Eq for PreviewContent {}

impl PreviewContent {
    /// Captured text with no reported pane width.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(PaneCapture {
            text: text.into(),
            width: None,
        })
    }
}

impl PreviewStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, source: &PreviewSource) -> Option<&PreviewEntry> {
        self.entries.get(source)
    }

    pub fn get_mut(&mut self, source: &PreviewSource) -> Option<&mut PreviewEntry> {
        self.entries.get_mut(source)
    }

    /// Store a loaded preview, ending its load and clearing any stale
    /// mark. Evicts the oldest entries of its kind past the kind's cap.
    pub fn insert(&mut self, source: PreviewSource, content: PreviewContent) {
        self.insert_at(source, content, Instant::now());
    }

    /// [`Self::insert`] with an explicit load time, so tests can order
    /// entries without sleeping.
    pub fn insert_at(&mut self, source: PreviewSource, content: PreviewContent, at: Instant) {
        self.loading.remove(&source);
        let kind = source.clone();
        self.entries.insert(
            source,
            PreviewEntry {
                content,
                captured_at: Some(at),
                stale: false,
            },
        );
        self.evict_oldest_of(&kind);
    }

    fn evict_oldest_of(&mut self, kind: &PreviewSource) {
        let Some(cap) = kind.cache_cap() else {
            return;
        };
        let mut same: Vec<(Option<Instant>, PreviewSource)> = self
            .entries
            .iter()
            .filter(|(source, _)| source.same_kind(kind))
            .map(|(source, entry)| (entry.captured_at, source.clone()))
            .collect();
        if same.len() <= cap {
            return;
        }
        same.sort();
        let excess = same.len() - cap;
        for (_, source) in same.into_iter().take(excess) {
            self.entries.remove(&source);
        }
    }

    /// Record that a load for `source` started. Returns `false` when
    /// one is already in flight.
    pub fn start_loading(&mut self, source: PreviewSource) -> bool {
        self.loading.insert(source)
    }

    pub fn is_loading(&self, source: &PreviewSource) -> bool {
        self.loading.contains(source)
    }

    /// Mark a source for reload, keeping its content on screen until
    /// the reload lands.
    pub fn mark_stale(&mut self, source: &PreviewSource) {
        if let Some(entry) = self.entries.get_mut(source) {
            entry.stale = true;
        }
    }
}

/// What the Preview zone is about (ADR 0112 §2–3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSubject {
    /// The left selection's row.
    Selection,
    /// A node the explorer reaches: the neighbor on a Related row, or
    /// the node the explorer drilled into.
    Node(NodeId),
    /// A field row whose value previews by its type (ADR 0112 §2).
    Field(FieldValue),
    /// An unresolved-evidence row, which names no node.
    Unresolved,
}

/// Decide what the Preview is about from the explorer cursor. A Related
/// row is about its neighbor, a field row with a typed value about that
/// value, and any other row about the focused node, which is the left
/// selection until the explorer drills.
pub fn preview_subject(app: &App) -> PreviewSubject {
    use crate::tui::explorer::RowPreview;
    let Some(state) = app.explorer() else {
        return PreviewSubject::Selection;
    };
    let row_preview = state
        .selected_row()
        .and_then(|row| match state.view.row_preview(&row) {
            Some(RowPreview::Link { neighbor_id, .. }) => {
                Some(PreviewSubject::Node(neighbor_id.clone()))
            }
            Some(RowPreview::Unresolved { .. }) => Some(PreviewSubject::Unresolved),
            None => match row {
                crate::tui::explorer::ExplorerRow::NodeField { index, .. } => state
                    .view
                    .fields(state.full_detail_expanded)
                    .get(index)
                    .and_then(|field| field.preview.clone())
                    .map(PreviewSubject::Field),
                _ => None,
            },
        });
    row_preview.unwrap_or_else(|| {
        if state.breadcrumb.is_empty() {
            PreviewSubject::Selection
        } else {
            PreviewSubject::Node(state.view.focused.clone())
        }
    })
}

/// Resolve the Preview to the source it shows (ADR 0112). Pure over
/// [`App`], so the runtime's loader and the renderer resolve the same
/// source.
pub fn preview_source(app: &App) -> Option<PreviewSource> {
    match preview_subject(app) {
        PreviewSubject::Selection => selection_source(app),
        PreviewSubject::Node(node) => node_source(app, &node),
        PreviewSubject::Field(value) => value_source(app, &value),
        PreviewSubject::Unresolved => None,
    }
}

/// The source a field's typed value previews: what its path, ref,
/// commit, or process holds, or the node it names. Text, lists,
/// commands, timestamps, and glossary values preview from the value
/// alone ([`value_lines`]) and have no source.
pub fn value_source(app: &App, value: &FieldValue) -> Option<PreviewSource> {
    Some(match value {
        FieldValue::Path(path) => PreviewSource::Path(PathBuf::from(path)),
        FieldValue::Ref {
            dir,
            rev,
            ahead_behind,
        } => PreviewSource::Git(GitQuery::Log {
            dir: PathBuf::from(dir),
            rev: rev.clone(),
            ahead_behind: ahead_behind.clone(),
        }),
        FieldValue::Commit { dir, commit } => PreviewSource::Git(GitQuery::Show {
            dir: PathBuf::from(dir),
            commit: commit.clone(),
        }),
        FieldValue::Process(pid) => PreviewSource::Process(*pid),
        FieldValue::Node(node) => return node_source(app, node),
        FieldValue::Text(_)
        | FieldValue::List(_)
        | FieldValue::Command(_)
        | FieldValue::Timestamp(_)
        | FieldValue::Glossary { .. } => return None,
    })
}

/// The source a node previews (ADR 0112 §1):
///
/// - an agent session shows its transcript, or its pane when the
///   operator chose `session_preview = pane` and the session is in tmux;
/// - a mux session shows its pane, unless it hosts this TUI;
/// - a workspace lists its root;
/// - a checkout shows `git status` above a listing of its root;
/// - a repo shows recent commits on its default branch and its remotes;
/// - a branch shows recent commits at its ref and, with an upstream,
///   how far it is ahead and behind;
/// - a runtime process shows its process tree;
/// - a pull request shows its title, state, checks, reviews, and body;
/// - a pin shows its bound session's preview, or, with no session but
///   a live mux, that mux's pane;
/// - a fork has none, and the renderer shows its fields.
pub fn node_source(app: &App, node: &NodeId) -> Option<PreviewSource> {
    match node {
        NodeId::AgentSession(session) => {
            let pane = (app.session_preview() == crate::tui::SessionPreview::Pane)
                .then(|| app.snapshot_handle())
                .flatten()
                .and_then(|handle| {
                    crate::tui::actions::preferred_mux_for_session(handle.snapshot(), node)
                })
                .filter(|mux| !hosts_this_tui(app, mux));
            Some(pane.map_or_else(
                || PreviewSource::Transcript(session.clone()),
                PreviewSource::Pane,
            ))
        }
        NodeId::MuxSession(mux) => {
            (!hosts_this_tui(app, mux)).then(|| PreviewSource::Pane(mux.clone()))
        }
        NodeId::Pin(pin) => {
            // The pin sidecar is the source of truth for bindings; pin
            // nodes are projected from it.
            let snapshot = app.snapshot_handle()?.snapshot();
            let candidate = snapshot.pins.iter().find(|p| p.id == pin.id)?;
            match candidate.binding.as_ref()? {
                crate::model::PinBinding::Bound { session, .. } => {
                    node_source(app, &NodeId::AgentSession(session.clone()))
                }
                crate::model::PinBinding::StaleMux { mux } => {
                    node_source(app, &NodeId::MuxSession(mux.clone()))
                }
                crate::model::PinBinding::Unbound => None,
            }
        }
        NodeId::Workspace(_)
        | NodeId::Checkout(_)
        | NodeId::Repo(_)
        | NodeId::Branch(_)
        | NodeId::RuntimeProcess(_)
        | NodeId::ForgePr(_) => match graph_node(app, node)? {
            GraphNode::Workspace(workspace) => {
                Some(PreviewSource::Path(PathBuf::from(&workspace.root)))
            }
            GraphNode::Checkout(checkout) => Some(PreviewSource::Git(GitQuery::Status {
                root: PathBuf::from(&checkout.root),
            })),
            GraphNode::Repo(repo) => Some(PreviewSource::Git(GitQuery::Repo {
                dir: PathBuf::from(&repo.common_dir),
            })),
            GraphNode::RuntimeProcess(process) => process.pid.map(PreviewSource::Process),
            GraphNode::ForgePr(pr) => Some(PreviewSource::PullRequest(PrQuery {
                provider: pr.provider.clone(),
                host: pr.host.clone(),
                owner: pr.owner.clone(),
                repo: pr.repo.clone(),
                number: pr.number,
            })),
            GraphNode::Branch(branch) => Some(PreviewSource::Git(GitQuery::Log {
                dir: PathBuf::from(&branch.id.repo.common_dir),
                rev: branch.refname.clone(),
                ahead_behind: branch
                    .upstream
                    .clone()
                    .map(|upstream| (branch.refname.clone(), upstream)),
            })),
            _ => None,
        },
        // A fork has no preview beyond its fields (ADR 0112 §1).
        NodeId::Fork(_) => None,
    }
}

/// Whether `mux` is the tmux session this TUI runs in. Its pane would
/// show the TUI itself.
fn hosts_this_tui(app: &App, mux: &MuxSessionId) -> bool {
    app.config()
        .current_tmux_session
        .as_deref()
        .is_some_and(|current| pane_native_id(app, mux).as_deref() == Some(current))
}

/// The left selection's source:
///
/// - an agent session row resolves as its node does ([`node_source`]),
///   keeping its row's tmux state;
/// - a mux row, a mux candidate, and a pin placeholder whose mux is
///   live show the pane;
/// - any other row resolves as the node the right pane shows for it;
///   without a source, the renderer decides what to say.
fn selection_source(app: &App) -> Option<PreviewSource> {
    let selection = app.selection()?;
    let row = app.tree().rows.iter().find(|r| &r.id == selection)?;
    let pane = || {
        crate::tui::actions::resolve_attach_target(app)
            .ok()
            .map(|target| PreviewSource::Pane(target.mux))
    };
    match (&row.kind, selection) {
        (_, RowId::Pin { pin_id }) => {
            node_source(app, &NodeId::Pin(crate::model::PinId::new(pin_id.clone())))
        }
        (RowKind::AgentSession(session), _) => {
            let in_tmux = !matches!(session.mux_state, MuxIndicator::Unmuxed);
            if in_tmux && app.session_preview() == crate::tui::SessionPreview::Pane {
                pane()
            } else {
                Some(PreviewSource::Transcript(session.session.clone()))
            }
        }
        (RowKind::AgentSessionMuxCandidate(_), _)
        | (_, RowId::Group(NodeId::MuxSession(_)) | RowId::MuxSession(NodeId::MuxSession(_))) => {
            pane()
        }
        // Any other row previews the node the right pane shows for it.
        _ => app
            .explorer()
            .and_then(|state| node_source(app, &state.view.focused)),
    }
}

/// The snapshot's node for `id`.
fn graph_node<'a>(app: &'a App, id: &NodeId) -> Option<&'a GraphNode> {
    app.snapshot_handle()?
        .snapshot()
        .nodes
        .iter()
        .find(|node| &node.id() == id)
}

/// The name tmux knows a mux session by (`editor`, not the graph id
/// `tmux:editor`), from the snapshot's mux node.
pub fn pane_native_id(app: &App, mux: &MuxSessionId) -> Option<String> {
    app.snapshot_handle()?
        .snapshot()
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::MuxSession(node) if &node.id == mux => Some(node.native_id.clone()),
            _ => None,
        })
}

/// Load a source that needs no mux backend. Blocking: the live runtime
/// calls it on a worker thread. `None` for a pane, which captures
/// through the executor's mux op.
pub fn load_offline_preview(source: &PreviewSource) -> Option<PreviewContent> {
    match source {
        PreviewSource::Pane(_) => None,
        PreviewSource::Transcript(session) => Some(load_transcript_preview(session)),
        PreviewSource::Path(path) => Some(path::load_path_preview(path)),
        PreviewSource::Git(query) => Some(git::load_git_preview(query)),
        PreviewSource::PullRequest(query) => Some(forge::load_pr_preview(query)),
        PreviewSource::Process(pid) => Some(process::load_process_tree(
            std::path::Path::new("/proc"),
            *pid,
        )),
    }
}

/// Load a session's transcript into a [`PreviewContent`]. Blocking:
/// the live runtime calls it on a worker thread.
pub fn load_transcript_preview(session: &AgentSessionId) -> PreviewContent {
    match load_transcript(session) {
        Ok(document) => PreviewContent::Transcript(Box::new(ViewerState::new(document))),
        Err(TranscriptMissing::Unavailable(reason)) => PreviewContent::Unavailable(reason),
        Err(TranscriptMissing::Failed(message)) => PreviewContent::Failed(message),
    }
}

/// Run a single capture against the runner and translate it into a
/// [`PreviewContent`]. Pure (modulo the runner call); callers
/// supply the runner so tests inject [`crate::discovery::tmux::FakeTmux`].
pub fn capture_via(runner: &dyn MuxBackend, native_id: &str) -> PreviewContent {
    // The preview always queries the default socket, because
    // discovery doesn't cover non-default sockets yet.
    match runner.capture_pane(None, native_id) {
        Ok(TmuxCaptureOutcome::Captured(capture)) => PreviewContent::Text(capture),
        Ok(TmuxCaptureOutcome::NoTarget) => PreviewContent::NoTarget,
        Ok(TmuxCaptureOutcome::Unavailable(reason)) => {
            PreviewContent::Unavailable(reason.as_str().to_string())
        }
        Ok(TmuxCaptureOutcome::Failed { code, message }) => {
            let code_label = code.map(|c| format!(" (code {c})")).unwrap_or_default();
            PreviewContent::Failed(format!("tmux capture-pane failed{code_label}: {message}"))
        }
        Ok(TmuxCaptureOutcome::Unsupported) => PreviewContent::Unsupported,
        Err(err) => PreviewContent::Failed(format!("tmux capture-pane error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome, UnavailableReason};

    #[test]
    fn capture_via_text_passes_content_through() {
        let runner = FakeTmux::with_sessions("")
            .with_capture("editor", TmuxCaptureOutcome::captured("hello".to_string()));
        assert_eq!(
            capture_via(&runner, "editor"),
            PreviewContent::text("hello")
        );
    }

    #[test]
    fn capture_via_no_target_propagates() {
        let runner =
            FakeTmux::with_sessions("").with_capture("missing", TmuxCaptureOutcome::NoTarget);
        assert_eq!(capture_via(&runner, "missing"), PreviewContent::NoTarget);
    }

    #[test]
    fn capture_via_unsupported_when_runner_has_no_capture() {
        let runner = FakeTmux::with_sessions("");
        assert_eq!(
            capture_via(&runner, "anything"),
            PreviewContent::Unsupported
        );
    }

    #[test]
    fn capture_via_unavailable_includes_reason() {
        let runner = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::Unavailable(UnavailableReason::BinaryNotFound),
        );
        match capture_via(&runner, "editor") {
            PreviewContent::Unavailable(reason) => assert!(reason.contains("binary not found")),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn capture_via_failed_includes_code_and_message() {
        let runner = FakeTmux::with_sessions("").with_capture(
            "editor",
            TmuxCaptureOutcome::Failed {
                code: Some(2),
                message: "denied".to_string(),
            },
        );
        match capture_via(&runner, "editor") {
            PreviewContent::Failed(text) => {
                assert!(text.contains("code 2"));
                assert!(text.contains("denied"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn preview_store_round_trips() {
        use crate::model::MuxSessionId;
        let mut store = PreviewStore::new();
        let id = PreviewSource::Pane(MuxSessionId::new("editor"));
        assert!(store.get(&id).is_none());
        store.insert(id.clone(), PreviewContent::text("hi"));
        let entry = store.get(&id).expect("present after insert");
        assert_eq!(entry.content, PreviewContent::text("hi"));
        assert!(entry.captured_at.is_some());
    }

    fn transcript(key: &str) -> PreviewSource {
        PreviewSource::Transcript(crate::model::AgentSessionId::new("codex", "/state", key))
    }

    #[test]
    fn preview_store_tracks_loads_and_stale_marks() {
        let mut store = PreviewStore::new();
        let source = transcript("a");
        assert!(store.start_loading(source.clone()));
        assert!(!store.start_loading(source.clone()), "already in flight");
        store.insert(
            source.clone(),
            PreviewContent::Unavailable("gone".to_string()),
        );
        assert!(!store.is_loading(&source), "landing ends the load");

        store.mark_stale(&source);
        assert!(store.get(&source).expect("kept").stale);
        store.insert(
            source.clone(),
            PreviewContent::Unavailable("gone".to_string()),
        );
        assert!(!store.get(&source).expect("kept").stale);
    }

    #[test]
    fn preview_store_keeps_the_most_recent_transcripts() {
        let mut store = PreviewStore::new();
        let start = Instant::now();
        let pane = PreviewSource::Pane(MuxSessionId::new("editor"));
        store.insert_at(pane.clone(), PreviewContent::text("hi"), start);
        for n in 0..=MAX_CACHED_TRANSCRIPTS {
            store.insert_at(
                transcript(&n.to_string()),
                PreviewContent::Unavailable("gone".to_string()),
                start + std::time::Duration::from_secs(n as u64 + 1),
            );
        }
        assert!(store.get(&transcript("0")).is_none(), "the oldest goes");
        assert!(
            store
                .get(&transcript(&MAX_CACHED_TRANSCRIPTS.to_string()))
                .is_some()
        );
        assert!(store.get(&pane).is_some(), "pane captures don't count");
    }
}
