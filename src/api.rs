//! Curated library facade for Conspectus consumers.
//!
//! This module re-exports the entry points most callers need to discover,
//! resolve, and render a graph without depending on CLI internals.
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

pub use crate::aliases::{
    AliasEntry, AliasOverlay, AliasParseError, AliasWriteError, AliasWriteOutcome, AliasesDocument,
    AliasesSection, alias_node_from_node_id, load_alias_entry_for_node, parse_aliases_document,
    remove_alias_entry, resolve_display_label, upsert_alias_entry,
};
pub use crate::config::{Config, ConfigDiagnostic, ConfigLoader, LoadOutcome, Projection};
pub use crate::declared::{
    DeclaredDocument, DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredSection,
    DeclaredStoreKind, DeclaredStoreSelection, DeclaredWriteOutcome,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, select_store_for_declaration, to_toml, upsert_declared_link,
};
pub use crate::discovery::{
    DiscoveryContext, DiscoveryProvider, GraphFragment, LocalDiscovery, LocalDiscoveryConfig,
    discover_local_at_roots, discover_local_with, empty_graph, merge_fragments,
};
pub use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence,
    Diagnostic, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
    LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId, Provenance, RelationKind,
    RepoId, RepoNode, ResolvedRelationship, SourceMetadata, UnresolvedEndpoint, WorkspaceId,
    WorkspaceNode,
};
pub use crate::output::{render_graph_json, table};
pub use crate::rename::{
    AgentAliasWrite, MuxNativeRename, RenamePlan, RenamePlanError, plan_session_rename,
};
pub use crate::resolve::{ResolveOutput, resolve_links, resolve_snapshot};
