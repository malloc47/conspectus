//! Curated library facade for Conspectus consumers (ADR 0015).
//!
//! This module re-exports the entry points most callers need to
//! discover, resolve, and render a graph without depending on
//! CLI internals. Items are grouped into three tiers:
//!
//! 1. **Core API** — the graph model, discovery pipeline, and
//!    rendering surface every library consumer will use.
//! 2. **Persistence + orchestration** — declared / aliases /
//!    rename write paths and the config file loader. Consumers
//!    building operator-facing tooling on top of Conspectus
//!    reach for these; simpler consumers should not.
//! 3. **Internal support** (`#[doc(hidden)]`) — file-format
//!    schema types (`AliasesDocument`, `DeclaredSection`) and
//!    orchestration primitives (`DeclaredStoreKind`,
//!    `DeclaredStoreSelection`, `AliasWriteError`,
//!    `AliasWriteOutcome`) that stay `pub use` only so the
//!    Conspectus binary's own tests + `dev_scenarios` still
//!    reach them through `conspectus::api::…`. Hidden from
//!    rustdoc so they don't clutter the operator-facing
//!    surface.
//!
//! ```
//! use std::fs;
//! use std::time::{SystemTime, UNIX_EPOCH};
//!
//! use conspectus::api::{LocalDiscoveryConfig, discover_local_with};
//! use conspectus::api::{render_graph_json, resolve_snapshot};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let unique = SystemTime::now()
//!     .duration_since(UNIX_EPOCH)?
//!     .as_nanos();
//! let root = std::env::temp_dir().join(format!("conspectus-api-{unique}"));
//! fs::create_dir(&root)?;
//!
//! let graph = discover_local_with([root.clone()], LocalDiscoveryConfig::empty())?;
//! let graph = resolve_snapshot(graph);
//! let json = render_graph_json(&graph)?;
//!
//! assert!(json.contains("\"nodes\""));
//! fs::remove_dir_all(root)?;
//! # Ok(())
//! # }
//! ```

// -----------------------------------------------------------------
// Tier 1: core API — graph model, discovery, resolution, render.
// -----------------------------------------------------------------

pub use crate::config::{Config, ConfigDiagnostic, ConfigLoader, LoadOutcome, Projection};
pub use crate::discovery::{
    DiscoveryContext, DiscoveryProvider, GraphFragment, LocalDiscovery, LocalDiscoveryConfig,
    discover_local_at_roots, discover_local_with, empty_graph, merge_fragments,
};
pub use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, CandidateScore, CheckoutId,
    CheckoutNode, Confidence, Diagnostic, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness,
    GraphLink, GraphNode, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
    Provenance, RelationKind, RepoId, RepoNode, ResolutionExplanation, ResolvedRelationship,
    ScoreAxis, SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
};
pub use crate::output::{render_graph_json, table};
pub use crate::resolve::{
    ResolveOutput, explain_resolved_relationships, resolve_links, resolve_snapshot,
};

// -----------------------------------------------------------------
// Tier 2: persistence + orchestration — declared / aliases /
// rename write paths and the parse entrypoints. Operator-facing
// tooling consumers reach for these; simpler read-only
// consumers do not.
// -----------------------------------------------------------------

pub use crate::aliases::{
    AliasEntry, AliasOverlay, AliasParseError, parse_aliases_document, resolve_display_label,
};
pub use crate::declared::{
    DeclaredDocument, DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredWriteOutcome,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, to_toml, upsert_declared_link,
};
pub use crate::rename::{
    AgentAliasWrite, MuxNativeRename, RenamePlan, RenamePlanError, plan_session_rename,
};

// -----------------------------------------------------------------
// Tier 3: `#[doc(hidden)]` internal support. `pub use` because
// dev_scenarios and integration tests reach them through
// `conspectus::api::…`, but not part of the operator-facing
// library contract.
// -----------------------------------------------------------------

#[doc(hidden)]
pub use crate::aliases::{
    AliasWriteError, AliasWriteOutcome, AliasesDocument, AliasesSection, alias_node_from_node_id,
    load_alias_entry_for_node, remove_alias_entry, upsert_alias_entry,
};
#[doc(hidden)]
pub use crate::declared::{
    DeclaredSection, DeclaredStoreKind, DeclaredStoreSelection, select_store_for_declaration,
};
