//! Resolver output: resolved relationships, their score explanations, per-node provenance, and diagnostics.

use super::*;

/// Per-node producing-provider metadata (ADR 0037). The
/// sidecar lives on [`GraphSnapshot`] keyed by `NodeId` so node
/// structs themselves remain provider-agnostic and the producers
/// have a single place to record their origin alongside whatever
/// they were already going to push into the snapshot.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct NodeProvenance {
    /// Stable identifier of the producing discovery provider, e.g.
    /// `git`, `harness::claude_code`, `tmux`, `forge::github`,
    /// `atelier`, `pins`, `aliases`, `declared`, `cross_link`,
    /// `codex_log`, `hook_sidecar`, `agent_deck`, `workspace`.
    /// Convention: lowercase, `::`-separated when an area hosts
    /// multiple adapters; mirrors `SourceMetadata.adapter` on the
    /// link side. The canonical constants live in
    /// `crate::discovery::providers`.
    pub provider: String,
    /// Unix epoch (seconds) captured when the provider produced
    /// this node. `None` when the producer did not record a
    /// timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_epoch: Option<i64>,
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
pub struct ResolvedRelationship {
    pub source: NodeId,
    /// Placeholder target that carries the would-have-been winner's
    /// neighbor for "no-winner" slots (ADR 0077). When
    /// `selected_link_id` is `Some`, this is the resolved target.
    /// When `selected_link_id` is `None`, the resolver could not
    /// pick — consumers MUST gate downstream use of `target` on
    /// `selected_link_id.is_some()` and treat it as a placeholder
    /// otherwise.
    pub target: NodeId,
    pub relation: RelationKind,
    /// The link id of the resolver's chosen winner, when there is
    /// one. `None` when the resolver explicitly cannot pick — at
    /// time of writing the only producer is
    /// `suppress_ambiguous_cwd_mux_links`, which fires when a mux
    /// is claimed by multiple sessions via cwd evidence alone.
    /// Rust consumers pattern-match on `Option` before reading the slot
    /// as "the answer." See ADR 0077.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_link_id: Option<String>,
    /// Candidates that offered a different answer for this slot.
    /// When `selected_link_id` is `Some`, these are the losers whose
    /// target differs from the winner's. When `selected_link_id` is
    /// `None`, this is *every* candidate that was considered —
    /// including what would have been the arbitrary tiebreak winner
    /// — so the Other-zone renderer and the ambiguity signal can both
    /// walk the full candidate set without a parallel inference path.
    /// See ADR 0077 and ADR 0107.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competing_link_ids: Vec<String>,
    /// Non-winning candidates that name the winner's target: other
    /// producers agreeing with the answer. Never a conflict. Empty
    /// when `selected_link_id` is `None`, since there is no answer to
    /// corroborate. See ADR 0107.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corroborating_link_ids: Vec<String>,
    /// Optional resolver score breakdown. Populated by
    /// `resolve::explain_resolved_relationships` for explainer
    /// surfaces; omitted from default graph JSON so the baseline
    /// machine-readable shape remains compact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ResolutionExplanation>,
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
pub struct ResolutionExplanation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<CandidateScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competing: Vec<CandidateScore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corroborating: Vec<CandidateScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisive_axis: Option<String>,
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
pub struct CandidateScore {
    pub link_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<ScoreAxis>,
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
pub struct ScoreAxis {
    pub name: String,
    pub value: String,
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diagnostic {
    UnresolvedEndpoint {
        link_id: String,
        relation: RelationKind,
    },
    /// A provider couldn't read its source this cycle: `gh` missing,
    /// logged out, offline, or a query failed (ADR 0115). Whatever the
    /// provider did read still loads.
    ProviderUnavailable {
        provider: String,
        reason: String,
    },
    Config {
        path: String,
        message: String,
    },
    Conflict {
        source: NodeId,
        relation: RelationKind,
        selected_link_id: String,
        competing_link_ids: Vec<String>,
    },
    /// ADR 0141. Legacy `[[pins.entries]]` are still loaded as
    /// in-memory pin slots; `conspectus pin migrate` writes them to
    /// `pins.toml` and removes the old tables.
    LegacyPins {
        count: u32,
        ids: Vec<String>,
    },
}

impl Diagnostic {
    /// Whether discovery, rather than resolution, produced this
    /// diagnostic. Resolution regenerates its own each run and keeps
    /// these.
    pub fn is_from_discovery(&self) -> bool {
        matches!(
            self,
            Self::Config { .. } | Self::ProviderUnavailable { .. } | Self::LegacyPins { .. }
        )
    }
}
