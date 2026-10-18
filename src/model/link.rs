//! Candidate links: relation kinds, provenance, confidence, and the `GraphLink` evidence record.

use super::*;

/// Rank active candidate links by provenance precedence, then confidence,
/// then link id, and return the first. Non-active links are skipped, so the
/// helper is safe to call on either a list filtered ahead of time or a raw
/// `Vec<&GraphLink>`.
pub fn pick_preferred<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links
        .iter()
        .copied()
        .filter(|link| matches!(link.state, LinkState::Active))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    AssociatedWith,
    BelongsToRepo,
    CheckedOutBranch,
    WorkspaceContainsRepo,
    BranchHasForgePr,
    LinkedToMux,
    RootedIn,
    ForksWorkspace,
    ForksRepo,
    CreatedCheckout,
    ReferencedCheckout,
    ParentSession,
    ChildSession,
    CreatedBranch,
    AssociatedBranch,
    ParentFork,
    RootedAtPath,
    MuxContainsProcess,
    ProcessIdentifiesSession,
    ProcessCandidatesSession,
    PinTargetsMux,
    PinRealizedBySession,
}

impl RelationKind {
    /// Stable snake_case label used across CLI, TUI, and snapshot
    /// outputs. Matches the `serde(rename_all = "snake_case")` form.
    pub fn snake_case(&self) -> &'static str {
        match self {
            Self::AssociatedWith => "associated_with",
            Self::BelongsToRepo => "belongs_to_repo",
            Self::CheckedOutBranch => "checked_out_branch",
            Self::WorkspaceContainsRepo => "workspace_contains_repo",
            Self::BranchHasForgePr => "branch_has_forge_pr",
            Self::LinkedToMux => "linked_to_mux",
            Self::RootedIn => "rooted_in",
            Self::ForksWorkspace => "forks_workspace",
            Self::ForksRepo => "forks_repo",
            Self::CreatedCheckout => "created_checkout",
            Self::ReferencedCheckout => "referenced_checkout",
            Self::ParentSession => "parent_session",
            Self::ChildSession => "child_session",
            Self::CreatedBranch => "created_branch",
            Self::AssociatedBranch => "associated_branch",
            Self::ParentFork => "parent_fork",
            Self::RootedAtPath => "rooted_at_path",
            Self::MuxContainsProcess => "mux_contains_process",
            Self::ProcessIdentifiesSession => "process_identifies_session",
            Self::ProcessCandidatesSession => "process_candidates_session",
            Self::PinTargetsMux => "pin_targets_mux",
            Self::PinRealizedBySession => "pin_realized_by_session",
        }
    }

    /// Inverse of [`Self::snake_case`]. Parses the
    /// stable snake_case label back to its `RelationKind`.
    /// Returns `Err` with an operator-friendly message for
    /// unknown labels. Callers (CLI, declared parser, table
    /// renderer) all consult this so an added variant is a
    /// single change instead of parallel updates in three
    /// files.
    pub fn from_snake_case(raw: &str) -> Result<Self, String> {
        match raw {
            "associated_with" => Ok(Self::AssociatedWith),
            "belongs_to_repo" => Ok(Self::BelongsToRepo),
            "checked_out_branch" => Ok(Self::CheckedOutBranch),
            "workspace_contains_repo" => Ok(Self::WorkspaceContainsRepo),
            "branch_has_forge_pr" => Ok(Self::BranchHasForgePr),
            "linked_to_mux" => Ok(Self::LinkedToMux),
            "rooted_in" => Ok(Self::RootedIn),
            "forks_workspace" => Ok(Self::ForksWorkspace),
            "forks_repo" => Ok(Self::ForksRepo),
            "created_checkout" => Ok(Self::CreatedCheckout),
            "referenced_checkout" => Ok(Self::ReferencedCheckout),
            "parent_session" => Ok(Self::ParentSession),
            "child_session" => Ok(Self::ChildSession),
            "created_branch" => Ok(Self::CreatedBranch),
            "associated_branch" => Ok(Self::AssociatedBranch),
            "parent_fork" => Ok(Self::ParentFork),
            "rooted_at_path" => Ok(Self::RootedAtPath),
            "mux_contains_process" => Ok(Self::MuxContainsProcess),
            "process_identifies_session" => Ok(Self::ProcessIdentifiesSession),
            "process_candidates_session" => Ok(Self::ProcessCandidatesSession),
            "pin_targets_mux" => Ok(Self::PinTargetsMux),
            "pin_realized_by_session" => Ok(Self::PinRealizedBySession),
            _ => Err(format!(
                "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
            )),
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    LocalDeclared,
    LocalPin,
    GlobalDeclared,
    GlobalPin,
    StrongDiscovered,
    Discovered,
    Convention,
    Cached,
}

impl Provenance {
    /// Numeric precedence used for ranking competing candidate links.
    /// Higher wins.
    ///
    /// The local/global axis dominates the explicit/auto axis: a
    /// project-local pin beats a global declared link because project
    /// context is more specific than user-wide intent. Within a
    /// scope, explicit declared links (operator typed `conspectus
    /// declared create` or `pin bind`) beat pin auto-attribution, so
    /// `pin bind --to <session>` lands as `LocalDeclared` and
    /// authoritatively overrides the pin's `LocalPin` evidence per
    /// ADR 0057 §Resolver Binding Semantics step 5.
    pub fn precedence(self) -> u8 {
        match self {
            Self::LocalDeclared => 7,
            Self::LocalPin => 6,
            Self::GlobalDeclared => 5,
            Self::GlobalPin => 4,
            Self::StrongDiscovered => 3,
            Self::Discovered | Self::Convention => 2,
            Self::Cached => 1,
        }
    }

    /// Stable snake_case label used in operator-facing surfaces.
    pub fn snake_case(self) -> &'static str {
        match self {
            Self::LocalDeclared => "local_declared",
            Self::LocalPin => "local_pin",
            Self::GlobalDeclared => "global_declared",
            Self::GlobalPin => "global_pin",
            Self::StrongDiscovered => "strong_discovered",
            Self::Discovered => "discovered",
            Self::Convention => "convention",
            Self::Cached => "cached",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    /// Stable snake_case label used in operator-facing surfaces.
    pub fn snake_case(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkEndpoint {
    Node { id: NodeId },
    Unresolved { evidence: UnresolvedEndpoint },
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct UnresolvedEndpoint {
    pub node_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    #[rkyv(with = MetadataAsJson)]
    pub metadata: Metadata,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LinkState {
    Active,
    Ignored {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Overridden {
        by: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl LinkState {
    pub fn is_ignored(&self) -> bool {
        matches!(self, Self::Ignored { .. })
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct GraphLink {
    pub id: String,
    pub source: NodeId,
    pub target: LinkEndpoint,
    pub relation: RelationKind,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub freshness: Freshness,
    pub source_metadata: SourceMetadata,
    pub state: LinkState,
}

impl GraphLink {
    pub fn new(
        id: impl Into<String>,
        source: NodeId,
        target: LinkEndpoint,
        relation: RelationKind,
        provenance: Provenance,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            target,
            relation,
            provenance,
            confidence: Confidence::Medium,
            freshness: Freshness::Unknown,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    pub fn target_node_id(&self) -> Option<&NodeId> {
        match &self.target {
            LinkEndpoint::Node { id } => Some(id),
            LinkEndpoint::Unresolved { .. } => None,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct SourceMetadata {
    pub adapter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// Keys populated in this map come from
    /// [`source_field`] constants. Producers stamp
    /// `Metadata::insert(source_field::MATCH_KIND, …)`;
    /// consumers read via
    /// `metadata.get(source_field::MATCH_KIND)`. New keys
    /// gain a constant so producer/consumer sides can't
    /// silently drift.
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    #[rkyv(with = MetadataAsJson)]
    pub fields: Metadata,
    /// Unix epoch (seconds) captured when the producing adapter ran
    /// against the live world. Feeds the per-provider TTL comparison
    /// in the warm-start gate. `None` when the adapter did not record
    /// a timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
}

impl SourceMetadata {
    /// The link's [`MatchKind`], read from the
    /// [`source_field::MATCH_KIND`] field or, failing that, from
    /// `evidence`.
    pub fn match_kind(&self) -> Option<MatchKind> {
        self.fields
            .get(source_field::MATCH_KIND)
            .and_then(Value::as_str)
            .or(self.evidence.as_deref())
            .and_then(MatchKind::from_snake_case)
    }
}

/// Canonical field-name constants for
/// [`SourceMetadata::fields`]. Every producer that stamps a
/// value into the map and every consumer that reads one back
/// should reference the constants here so a typo or rename
/// surfaces at compile time instead of silently dropping the
/// evidence.
///
/// Missing constants can be added in the same commit that
/// introduces the field — the compile-time drift check is
/// the `every_source_field_constant_matches_its_string_literal`
/// unit test in the model module.
pub mod source_field {
    /// Resolver-visible match kind for mux + PR + session ↔
    /// process links. Populated by discovery adapters (hook
    /// sidecar, cross_link, codex_log) and read by the
    /// resolver's per-relation scoring.
    pub const MATCH_KIND: &str = "match_kind";
    /// Millisecond activity epoch used as a per-mux tie-break
    /// axis by the resolver.
    pub const MUX_ACTIVITY_EPOCH: &str = "mux_activity_epoch";
    /// Second-precision epoch used by the forge PR scoring.
    pub const UPDATED_EPOCH: &str = "updated_epoch";
    /// Filesystem root associated with a fork lineage link.
    pub const FORK_ROOT: &str = "fork_root";
    /// `"fork"` | `"spawn"` (etc) lineage classifier stamped
    /// by harness adapters onto `ParentSession` /
    /// `ChildSession` links.
    pub const LINEAGE_KIND: &str = "lineage_kind";
    /// Forge PR state (`"open"` / `"closed"` / `"merged"`)
    /// stamped by the GitHub adapter and consumed by the PR
    /// comparator.
    pub const STATE: &str = "state";
    /// Forge PR draft flag stamped by the GitHub adapter.
    pub const IS_DRAFT: &str = "is_draft";
    /// Canonical filesystem-facing name atelier / agent-deck /
    /// generic workspace producers stamp onto their
    /// `WorkspaceContainsRepo` evidence so the detail-panel
    /// member row shows the workspace-visible label.
    pub const LOGICAL_PATH: &str = "logical_path";
    /// Free-form evidence provenance scope stamp (per-adapter
    /// substring; used by cross_link tie-breakers).
    pub const SCOPE: &str = "scope";
}

/// How a discovery adapter matched the two ends of a mux-attribution
/// link. Stored as its snake_case string in
/// [`SourceMetadata::fields`] under [`source_field::MATCH_KIND`] and
/// usually repeated in [`SourceMetadata::evidence`]; the resolver
/// ranks candidates by it. Read it back with
/// [`SourceMetadata::match_kind`]. See `docs/mux-link-resolution.md`
/// for the ranking.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatchKind {
    /// The session's cwd equals the mux's cwd.
    ExactCwdMatch,
    /// One cwd is a prefix of the other.
    CwdPrefixMatch,
    /// The session file most recently written under the pane's
    /// harness state matches the session.
    SessionFileActivityMatch,
    /// The mux's active pane holds an open harness state file (session
    /// JSONL, rollout log, or OpenCode DB path).
    ActivePaneFdSessionMatch,
    /// The open state file agrees with a session key on the pane's
    /// start command.
    ActivePaneFdCommandSessionMatch,
    /// The pane's start command carries a session key argument (for
    /// example `--resume <uuid>`). Demoted by fresher hook and
    /// codex-log evidence (ADR 0028, ADR 0048).
    ActivePaneCommandSessionMatch,
    /// The active pane's process tree contains a harness process.
    ActivePaneProcessMatch,
    /// The process tree walk observed a harness process in the pane.
    ActivePaneProcessObservation,
    /// A hook sidecar record names the session (ADR 0028).
    HookSessionMatch,
    /// A hook sidecar record names the session's transcript path.
    HookSessionPathMatch,
    /// A hook sidecar record observed a harness process in the pane.
    HookProcessObservation,
    /// The Codex logs DB shows the pane's Codex process writing the
    /// session's thread (ADR 0048).
    CodexLogCurrentThreadMatch,
    /// The Codex logs DB binds a process to a thread (ADR 0048).
    CodexLogProcessThreadMatch,
    /// The Codex logs DB observed a Codex process in the pane.
    CodexLogProcessObservation,
    /// The resolver derived the link from a runtime process that
    /// identifies the session.
    RuntimeProcessIdentifiesSession,
    /// The resolver derived the link from a runtime process that is
    /// one of several candidates for the session.
    RuntimeProcessCandidatesSession,
}

impl MatchKind {
    pub const ALL: [Self; 16] = [
        Self::ExactCwdMatch,
        Self::CwdPrefixMatch,
        Self::SessionFileActivityMatch,
        Self::ActivePaneFdSessionMatch,
        Self::ActivePaneFdCommandSessionMatch,
        Self::ActivePaneCommandSessionMatch,
        Self::ActivePaneProcessMatch,
        Self::ActivePaneProcessObservation,
        Self::HookSessionMatch,
        Self::HookSessionPathMatch,
        Self::HookProcessObservation,
        Self::CodexLogCurrentThreadMatch,
        Self::CodexLogProcessThreadMatch,
        Self::CodexLogProcessObservation,
        Self::RuntimeProcessIdentifiesSession,
        Self::RuntimeProcessCandidatesSession,
    ];

    pub fn snake_case(self) -> &'static str {
        match self {
            Self::ExactCwdMatch => "exact_cwd_match",
            Self::CwdPrefixMatch => "cwd_prefix_match",
            Self::SessionFileActivityMatch => "session_file_activity_match",
            Self::ActivePaneFdSessionMatch => "active_pane_fd_session_match",
            Self::ActivePaneFdCommandSessionMatch => "active_pane_fd_command_session_match",
            Self::ActivePaneCommandSessionMatch => "active_pane_command_session_match",
            Self::ActivePaneProcessMatch => "active_pane_process_match",
            Self::ActivePaneProcessObservation => "active_pane_process_observation",
            Self::HookSessionMatch => "hook_session_match",
            Self::HookSessionPathMatch => "hook_session_path_match",
            Self::HookProcessObservation => "hook_process_observation",
            Self::CodexLogCurrentThreadMatch => "codex_log_current_thread_match",
            Self::CodexLogProcessThreadMatch => "codex_log_process_thread_match",
            Self::CodexLogProcessObservation => "codex_log_process_observation",
            Self::RuntimeProcessIdentifiesSession => "runtime_process_identifies_session",
            Self::RuntimeProcessCandidatesSession => "runtime_process_candidates_session",
        }
    }

    /// `None` for strings outside the vocabulary, such as free-form
    /// evidence text from other adapters.
    pub fn from_snake_case(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.snake_case() == raw)
    }
}

impl fmt::Display for MatchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.snake_case())
    }
}
