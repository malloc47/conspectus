//! Session-alias overlay discovery.
//!
//! Reads ADR 0029 `[aliases]` config sections from the same stores
//! used by declared-link discovery and populates the
//! [`AliasOverlay`] sidecar on the resulting [`GraphSnapshot`]. The
//! pass is intentionally read-only and parallel to
//! [`crate::discovery::declared::apply_declared_links`].

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::aliases::parse_aliases_document;
use crate::config::ConfigLoader;
use crate::declared::DeclaredEndpoint;
use crate::discovery::DiscoveryContext;
use crate::model::{
    AgentSessionId, BranchId, CheckoutId, Diagnostic, ForgePrId, ForkId, GraphSnapshot,
    MuxSessionId, NodeId, PinId, RepoId, WorkspaceId,
};

pub fn apply_aliases(
    snapshot: &mut GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
) {
    // Local stores first so they win over global. Per ADR 0029 the
    // first writer wins inside [`AliasOverlay::insert_if_absent`].
    let mut paths: Vec<AliasStore> = Vec::new();

    let mut seen_project_paths = BTreeSet::new();
    for root in context.roots() {
        if let Some(path) = loader.locate_project_config(root)
            && seen_project_paths.insert(path.clone())
        {
            paths.push(AliasStore {
                path,
                scope: AliasStoreScope::Local,
            });
        }
    }

    if let Some(path) = loader.user_config_path()
        && path.is_file()
    {
        paths.push(AliasStore {
            path,
            scope: AliasStoreScope::Global,
        });
    }

    for store in paths {
        let Some(document) = read_aliases_document(&store, &mut snapshot.diagnostics) else {
            continue;
        };

        for entry in document.entries() {
            let id = node_id(&entry.node);
            snapshot
                .aliases
                .insert_if_absent(id, entry.display_name.clone());
        }
    }
}

#[derive(Clone, Debug)]
struct AliasStore {
    path: PathBuf,
    #[allow(dead_code)]
    scope: AliasStoreScope,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum AliasStoreScope {
    Local,
    Global,
}

fn read_aliases_document(
    store: &AliasStore,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<crate::aliases::AliasesDocument> {
    let text = match fs::read_to_string(&store.path) {
        Ok(text) => text,
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to read aliases: {err}"),
            ));
            return None;
        }
    };

    match parse_aliases_document(&text) {
        Ok(document) => Some(document),
        Err(err) => {
            diagnostics.push(config_diagnostic(
                &store.path,
                format!("failed to parse aliases: {err}"),
            ));
            None
        }
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

fn config_diagnostic(path: &Path, message: String) -> Diagnostic {
    Diagnostic::Config {
        path: path.to_string_lossy().to_string(),
        message,
    }
}

#[cfg(test)]
#[path = "aliases_tests.rs"]
mod tests;
