//! Pin slots (ADR 0141): named, ordered, editable places the operator
//! keeps at hand, each pointing at anything rebuildable.

use super::*;

/// What a slot points at.
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
pub enum PinKind {
    Mux,
    Session,
    Checkout,
    Repo,
    Branch,
    Pr,
    Workspace,
    Fork,
}

impl PinKind {
    pub const ALL: [PinKind; 8] = [
        PinKind::Mux,
        PinKind::Session,
        PinKind::Checkout,
        PinKind::Repo,
        PinKind::Branch,
        PinKind::Pr,
        PinKind::Workspace,
        PinKind::Fork,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PinKind::Mux => "mux",
            PinKind::Session => "session",
            PinKind::Checkout => "checkout",
            PinKind::Repo => "repo",
            PinKind::Branch => "branch",
            PinKind::Pr => "pr",
            PinKind::Workspace => "workspace",
            PinKind::Fork => "fork",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }

    /// The slot kind for a node, or `None` for kinds that can't be
    /// pinned: runtime processes are ephemeral and a pin can't be
    /// pinned (ADR 0141).
    pub fn of_node(id: &NodeId) -> Option<Self> {
        Some(match id {
            NodeId::MuxSession(_) => PinKind::Mux,
            NodeId::AgentSession(_) => PinKind::Session,
            NodeId::Checkout(_) => PinKind::Checkout,
            NodeId::Repo(_) => PinKind::Repo,
            NodeId::Branch(_) => PinKind::Branch,
            NodeId::ForgePr(_) => PinKind::Pr,
            NodeId::Workspace(_) => PinKind::Workspace,
            NodeId::Fork(_) => PinKind::Fork,
            NodeId::RuntimeProcess(_) | NodeId::Pin(_) => return None,
        })
    }

    /// Whether the kind's identity can change (a rename, a move), so
    /// a slot points at its history record instead of its node id.
    pub fn targets_history(self) -> bool {
        matches!(self, PinKind::Mux | PinKind::Checkout | PinKind::Repo)
    }

    /// The history kind backing this slot kind, if any.
    pub fn history_kind(self) -> Option<HistoryKind> {
        Some(match self {
            PinKind::Mux => HistoryKind::Mux,
            PinKind::Checkout => HistoryKind::Checkout,
            PinKind::Repo => HistoryKind::Repo,
            PinKind::Pr => HistoryKind::Pr,
            _ => return None,
        })
    }

    /// Fields the operator may override (ADR 0141).
    pub fn overridable_fields(self) -> &'static [&'static str] {
        match self {
            PinKind::Mux => &[
                "name",
                "socket",
                "cwd",
                "harness",
                "argv",
                "worktree_branch",
            ],
            PinKind::Session => &["resume_into"],
            PinKind::Checkout => &["branch", "base", "location"],
            PinKind::Repo => &["remote", "path"],
            _ => &[],
        }
    }

    /// Every field of the kind's recipe, in display order.
    pub fn recipe_fields(self) -> &'static [&'static str] {
        match self {
            PinKind::Mux => &[
                "name",
                "socket",
                "cwd",
                "harness",
                "argv",
                "worktree_branch",
            ],
            PinKind::Session => &["harness", "session_id", "title", "resume_into"],
            PinKind::Checkout => &["repo", "root", "branch", "head", "base", "location"],
            PinKind::Repo => &["common_dir", "remote", "path"],
            PinKind::Pr => &["repo", "number", "title", "state", "url"],
            PinKind::Branch => &["repo", "refname"],
            PinKind::Workspace => &["root"],
            PinKind::Fork => &["provider", "name"],
        }
    }

    /// Whether `field` may be overridden. `label` is always allowed.
    pub fn allows_override(self, field: &str) -> bool {
        field == "label" || self.overridable_fields().contains(&field)
    }

    /// Whether `field` holds a list (the rest hold text).
    pub fn field_is_list(field: &str) -> bool {
        field == "argv"
    }
}

/// A slot field's value.
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
#[serde(untagged)]
pub enum FieldValue {
    Text(String),
    List(Vec<String>),
}

impl FieldValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(text) => Some(text),
            FieldValue::List(_) => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            FieldValue::List(list) => Some(list),
            FieldValue::Text(_) => None,
        }
    }

    /// One line for tables and detail panes.
    pub fn display(&self) -> String {
        match self {
            FieldValue::Text(text) => text.clone(),
            FieldValue::List(list) => list.join(" "),
        }
    }
}

pub type SlotFields = BTreeMap<String, FieldValue>;

/// A slot as the store holds it and the snapshot carries it.
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
pub struct PinSlot {
    /// Unique name; the Pinned view shows it and `pin:<id>` is its
    /// graph identity.
    pub id: String,
    pub kind: PinKind,
    /// `history:<record id>`, a node id, or none for a planned slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// RFC 3339, set when the slot was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: SlotFields,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub snapshot: SlotFields,
    /// Position in `pins.toml` (or after it, for legacy slots).
    pub order: u32,
    /// Loaded from a legacy `[[pins.entries]]` table, not `pins.toml`.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub legacy: bool,
}

/// A parsed slot target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlotTarget {
    /// No target: built on the first rebuild.
    Planned,
    /// A history record id.
    History(String),
    /// A graph node.
    Node(NodeId),
}

impl PinSlot {
    pub const HISTORY_PREFIX: &'static str = "history:";

    /// The target text for a history record.
    pub fn history_target(record_id: &str) -> String {
        format!("{}{record_id}", Self::HISTORY_PREFIX)
    }

    pub fn parsed_target(&self) -> Result<SlotTarget, String> {
        match self.target.as_deref() {
            None => Ok(SlotTarget::Planned),
            Some(text) => {
                if let Some(id) = text.strip_prefix(Self::HISTORY_PREFIX) {
                    Ok(SlotTarget::History(id.to_string()))
                } else {
                    NodeId::parse(text)
                        .map(SlotTarget::Node)
                        .ok_or_else(|| format!("unrecognized target `{text}`"))
                }
            }
        }
    }

    /// What to call the slot in lists: its `label` override, else its
    /// snapshot label, else its id.
    pub fn label(&self) -> String {
        self.overrides
            .get("label")
            .or_else(|| self.snapshot.get("label"))
            .and_then(FieldValue::as_text)
            .map_or_else(|| self.id.clone(), str::to_string)
    }
}

/// Where a slot's status comes from (ADR 0141).
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
pub enum SlotStatus {
    /// The target exists now.
    Live,
    /// The target is in history but gone.
    Gone,
    /// The target's presence couldn't be checked.
    Unknown,
    /// The target is in neither the graph nor history; the slot
    /// renders from its snapshot.
    Missing,
    /// No target yet; the first rebuild builds it.
    Planned,
}

impl SlotStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SlotStatus::Live => "live",
            SlotStatus::Gone => "gone",
            SlotStatus::Unknown => "unknown",
            SlotStatus::Missing => "missing",
            SlotStatus::Planned => "planned",
        }
    }
}

/// The layer a recipe field's value came from.
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
pub enum FieldSource {
    Override,
    Live,
    History,
    Snapshot,
    Default,
}

impl FieldSource {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldSource::Override => "override",
            FieldSource::Live => "live",
            FieldSource::History => "history",
            FieldSource::Snapshot => "snapshot",
            FieldSource::Default => "default",
        }
    }
}

/// One resolved recipe field and the layer that supplied it.
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
pub struct RecipeField {
    pub name: String,
    pub value: FieldValue,
    pub source: FieldSource,
}
