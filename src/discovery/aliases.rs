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
mod tests {
    use super::*;
    use crate::aliases::{AliasEntry, upsert_alias_entry};
    use crate::config::PROJECT_CONFIG_FILENAME;
    use tempfile::TempDir;

    fn agent_endpoint(session_key: &str) -> DeclaredEndpoint {
        DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/home/me/.codex".to_string(),
            session_key: session_key.to_string(),
        }
    }

    #[test]
    fn empty_loader_leaves_overlay_empty() {
        let temp = TempDir::new().expect("temp");
        let context = DiscoveryContext::from_root(temp.path());
        let loader = ConfigLoader::new().with_home(temp.path());
        let mut snapshot = GraphSnapshot::empty();

        apply_aliases(&mut snapshot, &context, &loader);

        assert!(snapshot.aliases.is_empty());
        assert!(snapshot.diagnostics.is_empty());
    }

    #[test]
    fn project_store_populates_overlay() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join(PROJECT_CONFIG_FILENAME);
        upsert_alias_entry(
            &project,
            AliasEntry {
                node: agent_endpoint("alpha"),
                display_name: "ingest-refactor".to_string(),
                reason: None,
            },
        )
        .expect("write");

        let context = DiscoveryContext::from_root(temp.path());
        let loader = ConfigLoader::new().with_home(temp.path());
        let mut snapshot = GraphSnapshot::empty();

        apply_aliases(&mut snapshot, &context, &loader);

        let expected =
            NodeId::AgentSession(AgentSessionId::new("codex", "/home/me/.codex", "alpha"));
        assert_eq!(snapshot.aliases.get(&expected), Some("ingest-refactor"));
    }

    #[test]
    fn local_store_overrides_global_store() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join(PROJECT_CONFIG_FILENAME);
        let xdg = temp.path().join("xdg");
        let user = xdg.join(crate::config::USER_CONFIG_RELATIVE);
        upsert_alias_entry(
            &project,
            AliasEntry {
                node: agent_endpoint("alpha"),
                display_name: "project-name".to_string(),
                reason: None,
            },
        )
        .expect("project");
        upsert_alias_entry(
            &user,
            AliasEntry {
                node: agent_endpoint("alpha"),
                display_name: "user-name".to_string(),
                reason: None,
            },
        )
        .expect("user");

        let context = DiscoveryContext::from_root(temp.path());
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);
        let mut snapshot = GraphSnapshot::empty();

        apply_aliases(&mut snapshot, &context, &loader);

        let id = NodeId::AgentSession(AgentSessionId::new("codex", "/home/me/.codex", "alpha"));
        assert_eq!(snapshot.aliases.get(&id), Some("project-name"));
    }

    #[test]
    fn malformed_store_emits_diagnostic() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join(PROJECT_CONFIG_FILENAME);
        fs::write(&project, "[aliases\n").expect("seed");

        let context = DiscoveryContext::from_root(temp.path());
        let loader = ConfigLoader::new().with_home(temp.path());
        let mut snapshot = GraphSnapshot::empty();

        apply_aliases(&mut snapshot, &context, &loader);

        assert!(snapshot.aliases.is_empty());
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diag| matches!(diag, Diagnostic::Config { .. })),
            "expected a config diagnostic, got {:?}",
            snapshot.diagnostics,
        );
    }
}
