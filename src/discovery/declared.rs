//! Declared-link discovery.
//!
//! This pass reads ADR 0014 `[declared]` config sections and appends them as
//! `GraphLink` candidates. It is intentionally read-only.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::ConfigLoader;
use crate::declared::{
    DeclaredDocument, DeclaredEndpoint, DeclaredLink, DeclaredLinkState, parse_declared_document,
};
use crate::discovery::DiscoveryContext;
use crate::model::{
    AgentSessionId, BranchId, CheckoutId, Confidence, Diagnostic, ForgePrId, ForkId, Freshness,
    GraphLink, GraphSnapshot, LinkEndpoint, LinkState, Metadata, MuxSessionId, NodeId, PinId,
    Provenance, RepoId, SourceMetadata, UnresolvedEndpoint, WorkspaceId,
};

pub fn apply_declared_links(
    snapshot: &mut GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
) {
    let known_nodes: BTreeSet<NodeId> = snapshot
        .nodes
        .iter()
        .map(super::super::model::GraphNode::id)
        .collect();
    let mut paths = Vec::new();

    if let Some(path) = loader.user_config_path()
        && path.is_file()
    {
        paths.push(DeclaredStore {
            path,
            provenance: Provenance::GlobalDeclared,
            evidence: "user config",
        });
    }

    let mut seen_project_paths = BTreeSet::new();
    for root in context.roots() {
        if let Some(path) = loader.locate_project_config(root)
            && seen_project_paths.insert(path.clone())
        {
            paths.push(DeclaredStore {
                path,
                provenance: Provenance::LocalDeclared,
                evidence: "project config",
            });
        }
    }

    for store in paths {
        let Some(document) = read_declared_document(&store, &mut snapshot.diagnostics) else {
            continue;
        };

        for declared in document.links() {
            snapshot
                .candidate_links
                .push(link_from_declared(&store, declared, &known_nodes));
        }
    }

    // Stamp newly-added declared links with the `declared` provider.
    // First-write-wins protects earlier provenance entries.
    crate::discovery::stamp_snapshot_mutations(
        snapshot,
        crate::discovery::providers::DECLARED,
        crate::discovery::current_epoch(),
    );

    snapshot.canonicalize();
}

#[derive(Clone, Debug)]
struct DeclaredStore {
    path: PathBuf,
    provenance: Provenance,
    evidence: &'static str,
}

fn read_declared_document(
    store: &DeclaredStore,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DeclaredDocument> {
    let text = match fs::read_to_string(&store.path) {
        Ok(text) => text,
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to read declared links: {err}"),
            ));
            return None;
        }
    };

    match parse_declared_document(&text) {
        Ok(document) => Some(document),
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to parse declared links: {err}"),
            ));
            None
        }
    }
}

fn link_from_declared(
    store: &DeclaredStore,
    declared: &DeclaredLink,
    known_nodes: &BTreeSet<NodeId>,
) -> GraphLink {
    let source = node_id(&declared.source);
    let target_id = node_id(&declared.target);
    let target = if known_nodes.contains(&target_id) {
        LinkEndpoint::Node { id: target_id }
    } else {
        LinkEndpoint::Unresolved {
            evidence: unresolved_endpoint(&declared.target),
        }
    };
    let id = format!(
        "declared:{}:{}:{}",
        provenance_key(store.provenance),
        store.path.to_string_lossy(),
        declared.id
    );

    GraphLink {
        id,
        source,
        target,
        relation: declared.relation.clone(),
        provenance: store.provenance,
        confidence: Confidence::High,
        freshness: Freshness::Unknown,
        source_metadata: SourceMetadata {
            adapter: crate::discovery::providers::DECLARED.to_string(),
            evidence: Some(store.evidence.to_string()),
            fields: metadata(store, declared),
            freshness_epoch: None,
        },
        state: link_state(declared),
    }
}

fn node_id(endpoint: &DeclaredEndpoint) -> NodeId {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => NodeId::Repo(RepoId::new(common_dir.clone())),
        DeclaredEndpoint::Checkout {
            repo_common_dir,
            root,
        } => NodeId::Checkout(CheckoutId::new(
            RepoId::new(repo_common_dir.clone()),
            root.clone(),
        )),
        DeclaredEndpoint::Workspace { root } => NodeId::Workspace(WorkspaceId::new(root.clone())),
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => NodeId::AgentSession(AgentSessionId::new(
            harness_key.clone(),
            state_scope.clone(),
            session_key.clone(),
        )),
        DeclaredEndpoint::MuxSession { native_id } => {
            NodeId::MuxSession(MuxSessionId::new(native_id.clone()))
        }
        DeclaredEndpoint::Pin { id } => NodeId::Pin(PinId::new(id.clone())),
        DeclaredEndpoint::RuntimeProcess { observation_key } => {
            NodeId::RuntimeProcess(crate::model::RuntimeProcessId::new(observation_key.clone()))
        }
        DeclaredEndpoint::Branch {
            repo_common_dir,
            refname,
        } => NodeId::Branch(BranchId::new(
            RepoId::new(repo_common_dir.clone()),
            refname.clone(),
        )),
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => NodeId::Fork(ForkId::new(provider_source_key.clone())),
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => NodeId::ForgePr(ForgePrId::new(
            provider.clone(),
            host.clone(),
            owner.clone(),
            repo.clone(),
            *number,
        )),
    }
}

fn unresolved_endpoint(endpoint: &DeclaredEndpoint) -> UnresolvedEndpoint {
    let mut metadata = Metadata::new();
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => UnresolvedEndpoint {
            node_type: "repo".to_string(),
            harness_key: None,
            native_id: Some(common_dir.clone()),
            state_scope: None,
            path: Some(common_dir.clone()),
            metadata,
        },
        DeclaredEndpoint::Checkout {
            repo_common_dir,
            root,
        } => {
            metadata.insert(
                "repo_common_dir".to_string(),
                Value::String(repo_common_dir.clone()),
            );
            UnresolvedEndpoint {
                node_type: "checkout".to_string(),
                harness_key: None,
                native_id: Some(root.clone()),
                state_scope: None,
                path: Some(root.clone()),
                metadata,
            }
        }
        DeclaredEndpoint::Workspace { root } => UnresolvedEndpoint {
            node_type: "workspace".to_string(),
            harness_key: None,
            native_id: Some(root.clone()),
            state_scope: None,
            path: Some(root.clone()),
            metadata,
        },
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => UnresolvedEndpoint {
            node_type: "agent_session".to_string(),
            harness_key: Some(harness_key.clone()),
            native_id: Some(session_key.clone()),
            state_scope: Some(state_scope.clone()),
            path: None,
            metadata,
        },
        DeclaredEndpoint::MuxSession { native_id } => UnresolvedEndpoint {
            node_type: "mux_session".to_string(),
            harness_key: None,
            native_id: Some(native_id.clone()),
            state_scope: None,
            path: None,
            metadata,
        },
        DeclaredEndpoint::Pin { id } => UnresolvedEndpoint {
            node_type: "pin".to_string(),
            harness_key: None,
            native_id: Some(id.clone()),
            state_scope: None,
            path: None,
            metadata,
        },
        DeclaredEndpoint::RuntimeProcess { observation_key } => UnresolvedEndpoint {
            node_type: "runtime_process".to_string(),
            harness_key: None,
            native_id: Some(observation_key.clone()),
            state_scope: None,
            path: None,
            metadata,
        },
        DeclaredEndpoint::Branch {
            repo_common_dir,
            refname,
        } => {
            metadata.insert(
                "repo_common_dir".to_string(),
                Value::String(repo_common_dir.clone()),
            );
            UnresolvedEndpoint {
                node_type: "branch".to_string(),
                harness_key: None,
                native_id: Some(refname.clone()),
                state_scope: None,
                path: None,
                metadata,
            }
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => UnresolvedEndpoint {
            node_type: "fork".to_string(),
            harness_key: None,
            native_id: Some(provider_source_key.clone()),
            state_scope: None,
            path: None,
            metadata,
        },
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            metadata.insert("provider".to_string(), Value::String(provider.clone()));
            metadata.insert("host".to_string(), Value::String(host.clone()));
            metadata.insert("owner".to_string(), Value::String(owner.clone()));
            metadata.insert("repo".to_string(), Value::String(repo.clone()));
            UnresolvedEndpoint {
                node_type: "forge_pr".to_string(),
                harness_key: None,
                native_id: Some(number.to_string()),
                state_scope: None,
                path: None,
                metadata,
            }
        }
    }
}

fn link_state(declared: &DeclaredLink) -> LinkState {
    match declared.state {
        DeclaredLinkState::Active => LinkState::Active,
        DeclaredLinkState::Ignored => LinkState::Ignored {
            reason: declared.reason.clone(),
        },
        DeclaredLinkState::Overridden => LinkState::Overridden {
            by: declared
                .overridden_by
                .clone()
                .expect("validated overridden links name replacement"),
            reason: declared.reason.clone(),
        },
    }
}

fn metadata(store: &DeclaredStore, declared: &DeclaredLink) -> Metadata {
    let mut fields = Metadata::new();
    fields.insert("id".to_string(), Value::String(declared.id.clone()));
    fields.insert(
        "store".to_string(),
        Value::String(provenance_key(store.provenance).to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::String(store.path.to_string_lossy().to_string()),
    );
    if let Some(reason) = &declared.reason {
        fields.insert("reason".to_string(), Value::String(reason.clone()));
    }
    if let Some(overridden_by) = &declared.overridden_by {
        fields.insert(
            "overridden_by".to_string(),
            Value::String(overridden_by.clone()),
        );
    }
    if let Some(label) = &declared.label {
        fields.insert("label".to_string(), Value::String(label.clone()));
    }
    fields
}

fn config_diagnostic(path: &Path, message: String) -> Diagnostic {
    Diagnostic::Config {
        path: path.to_string_lossy().to_string(),
        message,
    }
}

fn provenance_key(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared | Provenance::LocalPin => "local",
        Provenance::GlobalDeclared | Provenance::GlobalPin => "global",
        Provenance::StrongDiscovered
        | Provenance::Discovered
        | Provenance::Convention
        | Provenance::Cached => "unknown",
    }
}

#[cfg(test)]
#[path = "declared_tests.rs"]
mod tests;
