//! Node identities: the typed id for each node kind and the `NodeId` sum over them.

use super::*;

macro_rules! simple_id {
    ($name:ident, $kind:literal, $field:ident) => {
        #[derive(
            Clone,
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
        #[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
        pub struct $name {
            pub $field: String,
        }

        impl $name {
            pub fn new($field: impl Into<String>) -> Self {
                Self {
                    $field: $field.into(),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $kind, self.$field)
            }
        }
    };
}

simple_id!(RepoId, "repo", common_dir);

simple_id!(WorkspaceId, "workspace", root);

simple_id!(MuxSessionId, "mux_session", native_id);

simple_id!(ForkId, "fork", provider_source_key);

simple_id!(RuntimeProcessId, "runtime_process", observation_key);

#[derive(
    Clone,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct CheckoutId {
    pub repo: RepoId,
    pub root: String,
}

impl CheckoutId {
    pub fn new(repo: RepoId, root: impl Into<String>) -> Self {
        Self {
            repo,
            root: root.into(),
        }
    }
}

impl fmt::Display for CheckoutId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checkout:{}@{}", self.repo, self.root)
    }
}

#[derive(
    Clone,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct AgentSessionId {
    pub harness_key: String,
    pub state_scope: String,
    pub session_key: String,
}

impl AgentSessionId {
    pub fn new(
        harness_key: impl Into<String>,
        state_scope: impl Into<String>,
        session_key: impl Into<String>,
    ) -> Self {
        Self {
            harness_key: harness_key.into(),
            state_scope: state_scope.into(),
            session_key: session_key.into(),
        }
    }
}

impl fmt::Display for AgentSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "agent_session:{}:{}:{}",
            self.harness_key, self.state_scope, self.session_key
        )
    }
}

#[derive(
    Clone,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct BranchId {
    pub repo: RepoId,
    pub refname: String,
}

impl BranchId {
    pub fn new(repo: RepoId, refname: impl Into<String>) -> Self {
        Self {
            repo,
            refname: refname.into(),
        }
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "branch:{}@{}", self.repo, self.refname)
    }
}

#[derive(
    Clone,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct ForgePrId {
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl ForgePrId {
    pub fn new(
        provider: impl Into<String>,
        host: impl Into<String>,
        owner: impl Into<String>,
        repo: impl Into<String>,
        number: u64,
    ) -> Self {
        Self {
            provider: provider.into(),
            host: host.into(),
            owner: owner.into(),
            repo: repo.into(),
            number,
        }
    }
}

impl fmt::Display for ForgePrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "forge_pr:{}:{}/{}/{}#{}",
            self.provider, self.host, self.owner, self.repo, self.number
        )
    }
}

#[derive(
    Clone,
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
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct PinId {
    pub id: String,
}

impl PinId {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl fmt::Display for PinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pin:{}", self.id)
    }
}

#[derive(
    Clone,
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
#[serde(tag = "type", rename_all = "snake_case")]
#[rkyv(derive(PartialEq, Eq, PartialOrd, Ord))]
pub enum NodeId {
    Repo(RepoId),
    Checkout(CheckoutId),
    Workspace(WorkspaceId),
    AgentSession(AgentSessionId),
    MuxSession(MuxSessionId),
    Pin(PinId),
    RuntimeProcess(RuntimeProcessId),
    Branch(BranchId),
    Fork(ForkId),
    ForgePr(ForgePrId),
}

impl NodeId {
    pub fn checkout(repo: RepoId, root: impl Into<String>) -> Self {
        Self::Checkout(CheckoutId::new(repo, root))
    }

    pub fn as_checkout(&self) -> Option<&CheckoutId> {
        match self {
            Self::Checkout(id) => Some(id),
            _ => None,
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repo(id) => id.fmt(f),
            Self::Checkout(id) => id.fmt(f),
            Self::Workspace(id) => id.fmt(f),
            Self::AgentSession(id) => id.fmt(f),
            Self::MuxSession(id) => id.fmt(f),
            Self::Pin(id) => id.fmt(f),
            Self::RuntimeProcess(id) => id.fmt(f),
            Self::Branch(id) => id.fmt(f),
            Self::Fork(id) => id.fmt(f),
            Self::ForgePr(id) => id.fmt(f),
        }
    }
}

impl NodeId {
    /// Parse the `Display` form back into an id. Paths may contain
    /// `:` and `@`, so the splits follow each kind's own shape: the
    /// last `@` separates a checkout's or branch's repo from its tail,
    /// and an agent session's key is everything after its scope.
    /// `None` for text that isn't a node id.
    pub fn parse(text: &str) -> Option<Self> {
        let (kind, rest) = text.split_once(':')?;
        if rest.is_empty() {
            return None;
        }
        Some(match kind {
            "repo" => Self::Repo(RepoId::new(rest)),
            "workspace" => Self::Workspace(WorkspaceId::new(rest)),
            "mux_session" => Self::MuxSession(MuxSessionId::new(rest)),
            "fork" => Self::Fork(ForkId::new(rest)),
            "runtime_process" => Self::RuntimeProcess(RuntimeProcessId::new(rest)),
            "pin" => Self::Pin(PinId::new(rest)),
            "checkout" => {
                let (repo, root) = rest.rsplit_once('@')?;
                let Self::Repo(repo) = Self::parse(repo)? else {
                    return None;
                };
                Self::Checkout(CheckoutId::new(repo, root))
            }
            "branch" => {
                let (repo, refname) = rest.rsplit_once('@')?;
                let Self::Repo(repo) = Self::parse(repo)? else {
                    return None;
                };
                Self::Branch(BranchId::new(repo, refname))
            }
            "agent_session" => {
                let mut parts = rest.splitn(3, ':');
                let (harness, scope, key) = (parts.next()?, parts.next()?, parts.next()?);
                Self::AgentSession(AgentSessionId::new(harness, scope, key))
            }
            "forge_pr" => {
                let (provider, rest) = rest.split_once(':')?;
                let (path, number) = rest.rsplit_once('#')?;
                let mut parts = path.splitn(3, '/');
                let (host, owner, repo) = (parts.next()?, parts.next()?, parts.next()?);
                Self::ForgePr(ForgePrId::new(
                    provider,
                    host,
                    owner,
                    repo,
                    number.parse().ok()?,
                ))
            }
            _ => return None,
        })
    }
}
