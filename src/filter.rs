//! Structured row-filter predicates shared by the CLI `table`
//! projection and the TUI row-tree builders.
//!
//! Per ADR 0031, filters are persistent typed predicates that narrow
//! the visible row set along structured dimensions; they compose with
//! the transient `/` fuzzy search but never replace it. v1
//! covers `harness`, `max-age`, and `mux-state`; further dimensions
//! land as additional fields without amending the ADR.
//!
//! The predicate type is intentionally non-TUI so `src/output/` can
//! consume it directly. It introduces no new dependencies (ADR 0024
//! policy).
//!
//! Evaluation is driven by [`SessionMatchInputs`], a small struct the
//! caller assembles for each row. The caller owns the cost of looking
//! up mux state, recency, etc., so the predicate stays pure.
//!
//! An agent's live state (ADR 0215) is one more dimension: the caller
//! reads each session's value from a [`LiveStates`] read once for the
//! whole build, so every row of one build is judged at one moment.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::live::LiveState;
use crate::model::{GraphSnapshot, LiveTables, MuxSessionId, NodeId};
use crate::viewer::search::{Grouping, Hit, SessionHits};

/// The sessions a content search matched, by harness and session key.
/// Held in memory only (the payload tenet): it is never serialized.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentHitMap(BTreeMap<String, BTreeMap<String, SessionHits>>);

impl ContentHitMap {
    pub fn get(&self, harness_key: &str, session_key: &str) -> Option<&SessionHits> {
        self.0.get(harness_key)?.get(session_key)
    }

    pub fn insert(&mut self, harness_key: String, session_key: String, hits: SessionHits) {
        self.0
            .entry(harness_key)
            .or_default()
            .insert(session_key, hits);
    }

    pub fn len(&self) -> usize {
        self.0.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Drop the snippet text of all but the `keep` best-ranked sessions,
    /// whose counts stay. Bounds what a search with many matches holds.
    pub fn keep_snippets(&mut self, keep: usize) {
        if self.len() <= keep {
            return;
        }
        let mut ranked: Vec<(&str, &str, &SessionHits)> = self.iter().collect();
        ranked.sort_by(|a, b| crate::viewer::search::cmp_sessions(a.2, b.2));
        let blank: Vec<(String, String)> = ranked
            .into_iter()
            .skip(keep)
            .map(|(harness, session, _)| (harness.to_string(), session.to_string()))
            .collect();
        for (harness, session) in blank {
            if let Some(hits) = self
                .0
                .get_mut(&harness)
                .and_then(|sessions| sessions.get_mut(&session))
            {
                hits.best.snippet.clear();
                hits.best.highlight = 0..0;
            }
        }
    }

    /// How many matching messages the matched sessions hold in all.
    pub fn total_hits(&self) -> usize {
        self.0
            .values()
            .flat_map(BTreeMap::values)
            .map(|hits| hits.count)
            .sum()
    }

    /// Every matched session with its summary, by harness then session.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &SessionHits)> {
        self.0.iter().flat_map(|(harness, sessions)| {
            sessions
                .iter()
                .map(move |(session, hits)| (harness.as_str(), session.as_str(), hits))
        })
    }
}

/// Every matching turn of the sessions a content search found, by
/// harness and session key. Read on demand for the Search view's
/// per-message grouping, and held in memory only like [`ContentHitMap`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentDetailMap(BTreeMap<String, BTreeMap<String, Arc<Vec<Hit>>>>);

impl ContentDetailMap {
    pub fn get(&self, harness_key: &str, session_key: &str) -> Option<&[Hit]> {
        self.0
            .get(harness_key)?
            .get(session_key)
            .map(|hits| hits.as_slice())
    }

    pub fn insert(&mut self, harness_key: String, session_key: String, hits: Vec<Hit>) {
        self.0
            .entry(harness_key)
            .or_default()
            .insert(session_key, Arc::new(hits));
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many sessions have their matching turns read.
    pub fn len(&self) -> usize {
        self.0.values().map(BTreeMap::len).sum()
    }

    /// How many matching messages are held, across every session.
    pub fn total_hits(&self) -> usize {
        self.0
            .values()
            .flat_map(BTreeMap::values)
            .map(|hits| hits.len())
            .sum()
    }

    /// Forget a session's matching turns, so they are read again.
    pub fn remove(&mut self, harness_key: &str, session_key: &str) {
        if let Some(sessions) = self.0.get_mut(harness_key) {
            sessions.remove(session_key);
            if sessions.is_empty() {
                self.0.remove(harness_key);
            }
        }
    }
}

/// The content query as the row predicate sees it: which sessions
/// matched so far. App-wide and never persisted, unlike the structured
/// filters around it (the one exception to their per-view, saved model).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentFilter {
    /// The query text, for chips and empty states.
    pub text: String,
    /// Sessions that matched, streamed in while the scan runs.
    pub hits: Arc<ContentHitMap>,
    /// The scan finished; until then a missing session may still match.
    pub complete: bool,
    /// How the Search view lists results: one row per session, a row per
    /// matching turn under each session, or every matching turn in one
    /// flat list.
    pub group: Grouping,
    /// The matching turns of each session, once read (the per-message
    /// groupings only).
    pub details: Arc<ContentDetailMap>,
    /// Mux panes whose scrollback matched, when the search read them
    /// (ADR 0149). Empty for a search that read only sessions.
    pub panes: Arc<Vec<crate::content_search::PaneHits>>,
}

/// Set of independent dimension constraints. Each `None` field means
/// "no constraint on this dimension"; multiple fields AND together at
/// evaluation time.
///
/// In addition to the narrowing predicates, the struct also carries
/// view-scoped *ordering* toggles surfaced through the same Controls
/// overlay (ADR 0031, amendment 2026-05-29). These do not change which
/// rows are visible — they re-order them — and so are skipped by
/// [`Self::matches_session`]. They are included in [`Self::is_empty`]
/// so the modal's "Clear all" affordance resets them alongside the
/// narrowing dimensions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<HarnessFilter>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "duration_secs"
    )]
    pub max_age: Option<Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mux_state: Option<MuxStateFilter>,
    /// Keep only sessions whose agent's live state is one of these
    /// (ADR 0215), read from [`Self::live`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_state: Option<LiveStateFilter>,
    /// Sessions view: float agent sessions resolved to a mux above
    /// sessions that aren't. Within each of the two resulting groups
    /// the existing within-group sort order is preserved.
    #[serde(skip_serializing_if = "is_false")]
    pub float_muxed_sessions_top: bool,
    /// Mux view: float mux sessions with at least one attached agent
    /// session above muxes with none. Within each group the existing
    /// within-group sort order is preserved.
    #[serde(skip_serializing_if = "is_false")]
    pub float_attached_muxes_top: bool,
    /// Sessions and Mux views: float the sessions, or the running muxes,
    /// whose agents wait on the operator above the rest (ADR 0215),
    /// ahead of the view's other float. Read from [`Self::live`].
    #[serde(skip_serializing_if = "is_false")]
    pub float_waiting_top: bool,
    /// Mux view: list every stopped mux history keeps, below the
    /// running ones. Off (the default) lists only the stopped muxes a
    /// pin points at; the rest live in the History view. Like the
    /// ordering toggles it isn't a narrowing predicate over sessions.
    #[serde(skip_serializing_if = "is_false")]
    pub show_stopped_muxes: bool,
    /// Native views: keep only rows for pinned entities, and the
    /// groups that lead to them (ADR 0141). Applied to the finished
    /// tree, so it isn't a predicate over sessions.
    #[serde(skip_serializing_if = "is_false")]
    pub pinned_only: bool,
    /// History view: list records that a pin points at too. Off (the
    /// default) shows unpinned records only, since a pinned record is
    /// already one keypress away in the Pinned view.
    #[serde(skip_serializing_if = "is_false")]
    pub show_pinned_history: bool,
    /// PRs view: include only PRs with, or only PRs without, a local
    /// branch (ADR 0129). `None` shows every PR. Unlike the
    /// predicates above it never looks at a PR's attached sessions, so
    /// it is deliberately left out of [`Self::has_narrowing_predicates`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_branch: Option<PrBranchFilter>,
    /// PRs view: include only PRs in these states (ADR 0134). `None`
    /// shows every PR. Left out of [`Self::has_narrowing_predicates`]
    /// for the same reason as [`Self::pr_branch`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_state: Option<PrStateFilter>,
    /// PRs view: float PRs that have a local branch above those that
    /// don't. Within each of the two groups the existing order is
    /// preserved.
    #[serde(skip_serializing_if = "is_false")]
    pub float_tracked_prs_top: bool,
    /// Keep only sessions whose transcripts match the content query.
    /// Held by the app, injected when a tree is built, and never saved.
    #[serde(skip)]
    pub content: Option<ContentFilter>,
    /// Every session's and mux's live state at the moment a tree is
    /// built, which [`Self::live_state`] and [`Self::float_waiting_top`]
    /// read: injected by the build, never saved, and empty (every
    /// session reads `none`) until then. A tree keeps the order and the
    /// rows it was built with until it is built again, however the
    /// states move in between (ADR 0215).
    #[serde(skip)]
    pub live: LiveStates,
}

impl RowFilter {
    /// True when no constraints or ordering toggles are active. The
    /// modal uses this to decide whether "Clear all" has anything to
    /// clear; callers gating *row-narrowing work* (predicate
    /// evaluation, empty-bucket suppression) should use
    /// [`Self::has_narrowing_predicates`] instead so ordering-only
    /// state doesn't change the visible row set.
    pub fn is_empty(&self) -> bool {
        !self.has_narrowing_predicates()
            && self.pr_branch.is_none()
            && self.pr_state.is_none()
            && !self.float_muxed_sessions_top
            && !self.float_attached_muxes_top
            && !self.float_waiting_top
            && !self.show_stopped_muxes
            && !self.pinned_only
            && !self.show_pinned_history
            && !self.float_tracked_prs_top
    }

    /// True when at least one narrowing dimension is set. Distinct
    /// from [`Self::is_empty`] in that ordering toggles do not count
    /// — they re-order rows but never drop them.
    pub fn has_narrowing_predicates(&self) -> bool {
        self.harness.is_some()
            || self.max_age.is_some()
            || self.mux_state.is_some()
            || self.live_state.is_some()
            || self.content.is_some()
    }

    /// Whether a predicate other than the live state is set: one that the
    /// agents a mux's mark counts can't all be judged by (an agent in a
    /// pane no session matches has no harness, age, or transcript), so
    /// those agents keep their mux listed only without one.
    pub fn has_session_only_predicates(&self) -> bool {
        self.harness.is_some()
            || self.max_age.is_some()
            || self.mux_state.is_some()
            || self.content.is_some()
    }

    /// Whether a mux is listed for the agents its mark counts, whose live
    /// states are `counted` ([`LiveStates::mux_agents`]): when the live
    /// state is the only predicate and one of them passes it (ADR 0215),
    /// so a mux drawn with a state's mark stays under that state's filter.
    pub fn admits_mux_agents(&self, counted: &BTreeSet<LiveStateKey>) -> bool {
        !self.has_session_only_predicates()
            && self
                .live_state
                .as_ref()
                .is_some_and(|wanted| counted.iter().any(|key| wanted.admits(*key)))
    }

    /// Evaluate the filter against an agent session's per-row data.
    /// Returns `true` when the row passes (or no constraint applies).
    pub fn matches_session(&self, inputs: &SessionMatchInputs<'_>) -> bool {
        if let Some(harness) = &self.harness
            && !harness.matches(inputs.harness_key)
        {
            return false;
        }
        if let Some(max_age) = self.max_age
            && !matches_max_age(inputs.now_epoch, inputs.last_active_epoch, max_age)
        {
            return false;
        }
        if let Some(mux_state) = &self.mux_state
            && !mux_state.matches(inputs.mux_state)
        {
            return false;
        }
        if let Some(live_state) = &self.live_state
            && !live_state.admits(inputs.live_state)
        {
            return false;
        }
        if let Some(content) = &self.content
            && content
                .hits
                .get(inputs.harness_key, inputs.session_key)
                .is_none()
        {
            return false;
        }
        true
    }
}

/// Which PRs the PRs view keeps, by whether the PR resolves to a
/// local branch (ADR 0129).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrBranchFilter {
    Tracked,
    Untracked,
}

impl PrBranchFilter {
    /// Parse from the CLI text form; `None` for an unknown spelling.
    pub fn from_str_ci(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tracked" => Some(Self::Tracked),
            "untracked" => Some(Self::Untracked),
            _ => None,
        }
    }

    /// Stable string form used in chips and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tracked => "tracked",
            Self::Untracked => "untracked",
        }
    }

    /// Whether a PR passes when `tracked` says it has a local branch.
    pub fn admits(self, tracked: bool) -> bool {
        match self {
            Self::Tracked => tracked,
            Self::Untracked => !tracked,
        }
    }

    /// The next state in the any → tracked → untracked → any cycle the
    /// Controls overlay steps through.
    pub fn cycle(current: Option<Self>) -> Option<Self> {
        match current {
            None => Some(Self::Tracked),
            Some(Self::Tracked) => Some(Self::Untracked),
            Some(Self::Untracked) => None,
        }
    }
}

/// A PR state the PRs view can filter on (ADR 0134).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrStateKey {
    /// Open, drafts included, as on the forge.
    Open,
    /// Open drafts only.
    Draft,
    Merged,
    Closed,
}

impl PrStateKey {
    pub const ALL: [Self; 4] = [Self::Open, Self::Draft, Self::Merged, Self::Closed];

    pub fn from_str_ci(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "open" => Some(Self::Open),
            "draft" => Some(Self::Draft),
            "merged" => Some(Self::Merged),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }

    /// Stable string form used in chips, config, and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Draft => "draft",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }

    /// Whether a PR with forge state `state` (any case) and draft flag
    /// `is_draft` is in this state.
    pub fn matches(self, state: Option<&str>, is_draft: bool) -> bool {
        let state = state.unwrap_or_default();
        let is = |wanted: &str| state.eq_ignore_ascii_case(wanted);
        match self {
            Self::Open => is("open"),
            Self::Draft => is("open") && is_draft,
            Self::Merged => is("merged"),
            Self::Closed => is("closed"),
        }
    }
}

/// Set membership over PR states: a PR passes when it is in any listed
/// state. Empty matches nothing (callers pass `None` for no constraint).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrStateFilter {
    Any(Vec<PrStateKey>),
}

impl PrStateFilter {
    pub fn from_values<I>(values: I) -> Self
    where
        I: IntoIterator<Item = PrStateKey>,
    {
        let mut canonical: Vec<PrStateKey> = values.into_iter().collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    pub fn admits(&self, state: Option<&str>, is_draft: bool) -> bool {
        match self {
            Self::Any(values) => values.iter().any(|key| key.matches(state, is_draft)),
        }
    }

    pub fn values(&self) -> &[PrStateKey] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Set membership over harness keys. An empty set matches nothing
/// (callers should pass `None` instead to express "no constraint").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessFilter {
    Any(Vec<String>),
}

impl HarnessFilter {
    /// Construct from any iterator of strings. Trims whitespace and
    /// lowercases each entry so CLI input like `Claude` and
    /// ` codex ` are treated as the canonical lower-case form the
    /// adapters use.
    pub fn from_values<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut canonical: Vec<String> = values
            .into_iter()
            .map(|s| s.as_ref().trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    fn matches(&self, harness_key: &str) -> bool {
        let needle = harness_key.trim().to_ascii_lowercase();
        match self {
            Self::Any(values) => values.iter().any(|v| v == &needle),
        }
    }

    /// Canonical sorted values, useful for chip rendering and config
    /// round-trip.
    pub fn values(&self) -> &[String] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Set membership over derived mux states. Empty set matches nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuxStateFilter {
    Any(Vec<MuxStateKey>),
}

impl MuxStateFilter {
    pub fn from_values<I>(values: I) -> Self
    where
        I: IntoIterator<Item = MuxStateKey>,
    {
        let mut canonical: Vec<MuxStateKey> = values.into_iter().collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    fn matches(&self, state: MuxStateKey) -> bool {
        match self {
            Self::Any(values) => values.contains(&state),
        }
    }

    pub fn values(&self) -> &[MuxStateKey] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Coarse mux state derived from active `LinkedToMux` candidate
/// count. Mirrors `MuxIndicator` in the TUI row tree but lives in the
/// shared crate so non-TUI callers can use it without pulling in the
/// TUI module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuxStateKey {
    Attached,
    Ambiguous,
    Unmuxed,
}

impl MuxStateKey {
    /// Parse from the CLI / config text form. Returns `None` for any
    /// unknown spelling so the caller can raise an actionable error.
    pub fn from_str_ci(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "attached" => Some(Self::Attached),
            "ambiguous" => Some(Self::Ambiguous),
            "unmuxed" => Some(Self::Unmuxed),
            _ => None,
        }
    }

    /// Stable string form used in chips, config, and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Attached => "attached",
            Self::Ambiguous => "ambiguous",
            Self::Unmuxed => "unmuxed",
        }
    }

    /// Derive from the active mux-candidate count for one session.
    /// Mirrors the TUI's `MuxIndicator` mapping so both surfaces
    /// agree.
    pub fn from_candidate_count(count: usize) -> Self {
        match count {
            0 => Self::Unmuxed,
            1 => Self::Attached,
            _ => Self::Ambiguous,
        }
    }
}

/// A session's live state as the filter reads it (ADR 0215): one value
/// for each mark the TUI's live cell draws, so a session the filter keeps
/// for `waiting` is one drawn with `!`. The TUI's marks are drawn from
/// [`Self::of_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveStateKey {
    /// Blocked on the operator.
    Waiting,
    /// The turn is over and the agent takes a prompt, seen or not.
    Ready,
    /// A turn, a compaction, or a start under way, quiet or not.
    Working,
    /// The turn ended on an error.
    Error,
    /// Evidence too old to trust.
    Unknown,
    /// No evidence, or the session exited.
    None,
}

impl LiveStateKey {
    /// Every value, in the order the command line lists them.
    pub const ALL: [Self; 6] = [
        Self::Waiting,
        Self::Ready,
        Self::Working,
        Self::Error,
        Self::Unknown,
        Self::None,
    ];

    /// The value of a session whose effective state is `state`:
    /// compacting and starting are work under way, and a session that
    /// exited has nothing to show, like one with no evidence.
    pub fn of_state(state: LiveState) -> Self {
        match state {
            LiveState::Waiting => Self::Waiting,
            LiveState::Ready => Self::Ready,
            LiveState::Working | LiveState::Compacting | LiveState::Starting => Self::Working,
            LiveState::Error => Self::Error,
            LiveState::Unknown => Self::Unknown,
            LiveState::Exited => Self::None,
        }
    }

    /// The value of a session with effective state `state`, `none`
    /// without evidence.
    pub fn of(state: Option<LiveState>) -> Self {
        state.map_or(Self::None, Self::of_state)
    }

    /// Parse from the CLI / config text form. Returns `None` for any
    /// unknown spelling so the caller can raise an actionable error.
    pub fn from_str_ci(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|key| key.as_str() == value)
    }

    /// Stable string form used in chips, config, and CLI output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Ready => "ready",
            Self::Working => "working",
            Self::Error => "error",
            Self::Unknown => "unknown",
            Self::None => "none",
        }
    }

    /// Every value's spelling, comma-separated, for a message that lists
    /// what is accepted.
    pub fn spellings() -> String {
        Self::ALL
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Set membership over live states. Empty set matches nothing (callers
/// pass `None` for no constraint).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveStateFilter {
    Any(Vec<LiveStateKey>),
}

impl LiveStateFilter {
    pub fn from_values<I>(values: I) -> Self
    where
        I: IntoIterator<Item = LiveStateKey>,
    {
        let mut canonical: Vec<LiveStateKey> = values.into_iter().collect();
        canonical.sort();
        canonical.dedup();
        Self::Any(canonical)
    }

    /// Whether a session whose live state reads `key` passes.
    pub fn admits(&self, key: LiveStateKey) -> bool {
        match self {
            Self::Any(values) => values.contains(&key),
        }
    }

    pub fn values(&self) -> &[LiveStateKey] {
        match self {
            Self::Any(values) => values,
        }
    }
}

/// Every session's and mux's live state at one moment, as the rows of a
/// build show it (ADR 0215): what the live-state filter and the
/// float-waiting order read, read once for the whole build so they agree
/// with the marks the rows are filled with. Held in memory only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveStates {
    /// Each session with live state, by its node.
    sessions: Arc<BTreeMap<NodeId, LiveStateKey>>,
    /// For each mux, the states of the agents its mark counts: the
    /// sessions whose preferred mux it is and the agents in its panes no
    /// session matches.
    muxes: Arc<BTreeMap<MuxSessionId, BTreeSet<LiveStateKey>>>,
}

/// The states of a mux no agent counts toward.
static NO_STATES: BTreeSet<LiveStateKey> = BTreeSet::new();

impl LiveStates {
    /// The states of `tables`, bound to `snapshot`, read at `now_ms`: a
    /// session's effective state, and each mux's agents counted as
    /// [`GraphSnapshot::live_counts_of_mux`] counts them.
    pub fn read(snapshot: &GraphSnapshot, tables: LiveTables<'_>, now_ms: i64) -> Self {
        let sessions = tables
            .nodes
            .iter()
            .map(|(node, binding)| {
                (
                    node.clone(),
                    LiveStateKey::of_state(binding.effective(now_ms).state),
                )
            })
            .collect();
        let muxes = snapshot
            .live_counts_by_mux(tables, now_ms)
            .into_iter()
            .map(|(mux, counts)| {
                let states = LiveState::ALL
                    .into_iter()
                    .filter(|state| counts.get(*state) > 0)
                    .map(LiveStateKey::of_state)
                    .collect();
                (mux, states)
            })
            .collect();
        Self {
            sessions: Arc::new(sessions),
            muxes: Arc::new(muxes),
        }
    }

    /// The live state of the session `id`: `none` without evidence.
    pub fn session(&self, id: &NodeId) -> LiveStateKey {
        self.sessions.get(id).copied().unwrap_or(LiveStateKey::None)
    }

    /// The states of the agents `mux`'s mark counts, as
    /// [`GraphSnapshot::live_counts_of_mux`] counts them: empty when none
    /// has live state.
    pub fn mux_agents(&self, mux: &MuxSessionId) -> &BTreeSet<LiveStateKey> {
        self.muxes.get(mux).unwrap_or(&NO_STATES)
    }

    /// Whether an agent counted on `mux` waits on the operator, so its
    /// mark is `!`.
    pub fn mux_waits(&self, mux: &MuxSessionId) -> bool {
        self.mux_agents(mux).contains(&LiveStateKey::Waiting)
    }
}

/// Per-row data the predicate evaluates against. Callers assemble
/// one of these for each candidate row; the predicate is read-only.
#[derive(Clone, Copy, Debug)]
pub struct SessionMatchInputs<'a> {
    pub harness_key: &'a str,
    /// The harness's own key for the session, for the content filter.
    pub session_key: &'a str,
    /// Wall-clock epoch (Unix seconds) used as the recency anchor.
    /// `None` disables age-based filtering for the row (the predicate
    /// treats absent `now` as "no constraint can be evaluated" and
    /// admits the row).
    pub now_epoch: Option<i64>,
    /// Session's last-active timestamp (Unix seconds).
    pub last_active_epoch: Option<i64>,
    pub mux_state: MuxStateKey,
    /// The session's live state, from the [`LiveStates`] of the build.
    pub live_state: LiveStateKey,
}

fn matches_max_age(
    now_epoch: Option<i64>,
    last_active_epoch: Option<i64>,
    max_age: Duration,
) -> bool {
    // Without either side of the comparison we can't evaluate the
    // constraint; admit the row rather than silently dropping it.
    let (Some(now), Some(last)) = (now_epoch, last_active_epoch) else {
        return true;
    };
    if now < last {
        // Clock skew — treat as fresh.
        return true;
    }
    let age_secs = (now - last) as u64;
    let max_secs = max_age.as_secs();
    age_secs <= max_secs
}

/// Serde helper that serializes `Option<Duration>` as optional u64
/// seconds for human-readable state files.
mod duration_secs {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(dur: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dur {
            Some(d) => d.as_secs().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<u64> = Option::deserialize(deserializer)?;
        Ok(secs.map(Duration::from_secs))
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
