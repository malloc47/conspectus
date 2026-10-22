//! History records (ADR 0140): what Conspectus has observed, gone
//! entities included, with what it takes to rebuild them.
//!
//! The store under `$XDG_STATE_HOME/conspectus/history/` owns these
//! records ([`crate::history`]). Discovery copies them into
//! [`GraphSnapshot::history`] so views, pin slots, exports, and
//! fixtures read recipes and record ids without touching the store.
//! Unknown fields survive a rewrite through the `extra` maps, so a
//! newer binary's fields outlive an older binary's write.

use super::*;

/// Where a record came from.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
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
pub enum RecordOrigin {
    /// Discovery saw the entity.
    #[default]
    Observed,
    /// Conspectus created it (`mux new`, `mux launch`, a rebuild).
    Launched,
    /// `pin migrate` seeded it from a legacy pin.
    Migrated,
}

/// What the History and Pinned views show once the entity is gone.
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RecordDisplay {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// The envelope every record shares.
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RecordMeta {
    /// Stable random id (`h_` and 12 hex digits). Pin slots point at
    /// records by id, so a record can change its key freely.
    pub id: String,
    /// Display form of the graph [`NodeId`] the record last matched.
    /// Informational: projection rebuilds the id from the recipe.
    pub key: String,
    pub first_seen: i64,
    pub last_seen: i64,
    #[serde(default)]
    pub origin: RecordOrigin,
    #[serde(default)]
    pub display: RecordDisplay,
    /// The record a rebuild replaced, when the rebuilt entity landed
    /// under a different key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuilt_from: Option<String>,
}

/// Where a recorded launch argv came from.
#[derive(
    Clone,
    Copy,
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
pub enum ArgvSource {
    /// The argv Conspectus built when it launched the mux.
    Conspectus,
    /// tmux's `pane_start_command` for a mux started elsewhere.
    Observed,
}

/// One running life of a mux: tmux's own session id plus its creation
/// time. A rename keeps the incarnation; a recreated name gets a new
/// one.
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct MuxIncarnation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
}

impl MuxIncarnation {
    /// Both halves known, so the incarnation can identify a session.
    pub fn is_complete(&self) -> bool {
        self.session_id.is_some() && self.created.is_some()
    }
}

/// A session a mux hosted, as the resolver attributed it.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct HostedSession {
    pub harness: String,
    pub session_id: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

/// How many hosted sessions a mux record keeps.
pub const HOSTED_SESSIONS_KEPT: usize = 10;

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
pub struct MuxRecipe {
    pub backend: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<MuxIncarnation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argv_source: Option<ArgvSource>,
    /// Harness of the most recent hosted session, or of the argv.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// The orchestrator that owns the mux's lifecycle (`agent-deck`),
    /// whose muxes Conspectus records but doesn't rebuild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orchestrator: Option<String>,
    /// Most recent first, at most [`HOSTED_SESSIONS_KEPT`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosted: Vec<HostedSession>,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
}

impl MuxRecipe {
    /// `tmux`, `tmux:<socket>`, or `zellij`: the probe this mux answers
    /// to. The default tmux socket has no suffix.
    pub fn probe_key(&self) -> String {
        probe_key(&self.backend, self.socket.as_deref())
    }

    /// The `MuxSessionId` native id discovery gives this mux.
    pub fn native_id(&self) -> String {
        match effective_socket(self.socket.as_deref()) {
            Some(socket) => format!("{}:{socket}:{}", self.backend, self.name),
            None => format!("{}:{}", self.backend, self.name),
        }
    }

    /// The most recent hosted session.
    pub fn last_hosted(&self) -> Option<&HostedSession> {
        self.hosted.iter().max_by_key(|hosted| hosted.last_seen)
    }
}

/// `None` for the default tmux socket, whichever way it is spelled.
pub fn effective_socket(socket: Option<&str>) -> Option<&str> {
    socket.filter(|socket| !socket.is_empty() && *socket != "default")
}

/// The key a mux probe reports under: the backend, plus the socket
/// when it isn't the default.
pub fn probe_key(backend: &str, socket: Option<&str>) -> String {
    match effective_socket(socket) {
        Some(socket) => format!("{backend}:{socket}"),
        None => backend.to_string(),
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
pub struct MuxRecord {
    #[serde(flatten)]
    pub meta: RecordMeta,
    pub recipe: MuxRecipe,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
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
pub struct CheckoutRecipe {
    /// The repo record's key (`repo:<common dir>`).
    pub repo: String,
    pub root: String,
    #[serde(default)]
    pub linked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
}

impl CheckoutRecipe {
    /// The repo's git common dir, from the repo key.
    pub fn common_dir(&self) -> &str {
        self.repo.strip_prefix("repo:").unwrap_or(&self.repo)
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
pub struct CheckoutRecord {
    #[serde(flatten)]
    pub meta: RecordMeta,
    pub recipe: CheckoutRecipe,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
}

/// A named fetch remote.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RecordedRemote {
    pub name: String,
    pub url: String,
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
pub struct RepoRecipe {
    pub common_dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remotes: Vec<RecordedRemote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
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
pub struct RepoRecord {
    #[serde(flatten)]
    pub meta: RecordMeta,
    pub recipe: RepoRecipe,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
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
pub struct PrRecipe {
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_ref: Option<String>,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
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
pub struct PrRecord {
    #[serde(flatten)]
    pub meta: RecordMeta,
    pub recipe: PrRecipe,
    #[serde(flatten)]
    #[rkyv(with = MetadataAsJson)]
    pub extra: Metadata,
}

/// The kinds history records.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Mux,
    Checkout,
    Repo,
    Pr,
}

impl HistoryKind {
    pub const ALL: [HistoryKind; 4] = [
        HistoryKind::Mux,
        HistoryKind::Checkout,
        HistoryKind::Repo,
        HistoryKind::Pr,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HistoryKind::Mux => "mux",
            HistoryKind::Checkout => "checkout",
            HistoryKind::Repo => "repo",
            HistoryKind::Pr => "pr",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mux" => Some(HistoryKind::Mux),
            "checkout" | "worktree" => Some(HistoryKind::Checkout),
            "repo" => Some(HistoryKind::Repo),
            "pr" => Some(HistoryKind::Pr),
            _ => None,
        }
    }

    /// The store file holding this kind.
    pub fn file_name(self) -> &'static str {
        match self {
            HistoryKind::Mux => "muxes.json",
            HistoryKind::Checkout => "checkouts.json",
            HistoryKind::Repo => "repos.json",
            HistoryKind::Pr => "prs.json",
        }
    }
}

/// A record's presence this cycle (ADR 0140). Live is the default and
/// has no entry anywhere.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "presence", rename_all = "snake_case")]
pub enum NodePresence {
    /// A targeted check of the recorded entity failed.
    Gone { last_seen: i64 },
    /// The check couldn't run.
    Unknown { last_seen: i64 },
}

impl NodePresence {
    pub fn last_seen(self) -> i64 {
        match self {
            NodePresence::Gone { last_seen } | NodePresence::Unknown { last_seen } => last_seen,
        }
    }

    pub fn is_gone(self) -> bool {
        matches!(self, NodePresence::Gone { .. })
    }

    pub fn label(self) -> &'static str {
        match self {
            NodePresence::Gone { .. } => "gone",
            NodePresence::Unknown { .. } => "unknown",
        }
    }
}

/// Every record, as the snapshot carries them, plus each non-live
/// record's presence keyed by record id.
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
pub struct HistorySnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub muxes: Vec<MuxRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkouts: Vec<CheckoutRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<RepoRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prs: Vec<PrRecord>,
    /// Presence of each record that isn't live, by record id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presence: BTreeMap<String, NodePresence>,
}

impl HistorySnapshot {
    pub fn is_empty(&self) -> bool {
        self.muxes.is_empty()
            && self.checkouts.is_empty()
            && self.repos.is_empty()
            && self.prs.is_empty()
            && self.presence.is_empty()
    }

    pub fn len(&self) -> usize {
        self.muxes.len() + self.checkouts.len() + self.repos.len() + self.prs.len()
    }

    /// The envelope of the record with this id, and its kind.
    pub fn meta(&self, id: &str) -> Option<(HistoryKind, &RecordMeta)> {
        self.muxes
            .iter()
            .map(|r| (HistoryKind::Mux, &r.meta))
            .chain(
                self.checkouts
                    .iter()
                    .map(|r| (HistoryKind::Checkout, &r.meta)),
            )
            .chain(self.repos.iter().map(|r| (HistoryKind::Repo, &r.meta)))
            .chain(self.prs.iter().map(|r| (HistoryKind::Pr, &r.meta)))
            .find(|(_, meta)| meta.id == id)
    }

    /// Every record's envelope with its kind.
    pub fn metas(&self) -> impl Iterator<Item = (HistoryKind, &RecordMeta)> {
        self.muxes
            .iter()
            .map(|r| (HistoryKind::Mux, &r.meta))
            .chain(
                self.checkouts
                    .iter()
                    .map(|r| (HistoryKind::Checkout, &r.meta)),
            )
            .chain(self.repos.iter().map(|r| (HistoryKind::Repo, &r.meta)))
            .chain(self.prs.iter().map(|r| (HistoryKind::Pr, &r.meta)))
    }

    pub fn mux(&self, id: &str) -> Option<&MuxRecord> {
        self.muxes.iter().find(|r| r.meta.id == id)
    }

    pub fn checkout(&self, id: &str) -> Option<&CheckoutRecord> {
        self.checkouts.iter().find(|r| r.meta.id == id)
    }

    pub fn repo(&self, id: &str) -> Option<&RepoRecord> {
        self.repos.iter().find(|r| r.meta.id == id)
    }

    pub fn pr(&self, id: &str) -> Option<&PrRecord> {
        self.prs.iter().find(|r| r.meta.id == id)
    }

    /// The record whose last-matched key is `key`.
    pub fn by_key(&self, key: &str) -> Option<(HistoryKind, &RecordMeta)> {
        self.metas().find(|(_, meta)| meta.key == key)
    }

    /// A record's presence; `None` means live.
    pub fn presence_of(&self, id: &str) -> Option<NodePresence> {
        self.presence.get(id).copied()
    }
}

/// Access to a record's envelope, whatever its kind.
pub trait Record {
    fn meta(&self) -> &RecordMeta;
    fn meta_mut(&mut self) -> &mut RecordMeta;
}

macro_rules! impl_record {
    ($($ty:ty),*) => {$(
        impl Record for $ty {
            fn meta(&self) -> &RecordMeta {
                &self.meta
            }
            fn meta_mut(&mut self) -> &mut RecordMeta {
                &mut self.meta
            }
        }
    )*};
}

impl_record!(MuxRecord, CheckoutRecord, RepoRecord, PrRecord);
