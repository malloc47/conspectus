//! Declared-link snapshot-aware helpers (H-REF-005).
//!
//! Everything here consults a live `GraphSnapshot` to make a
//! declaration decision — picking which config file scope owns
//! a new link (`select_store_for_declaration`), or projecting
//! a discovered node back into a declared endpoint
//! (`declared_endpoint_from_node_id`). Kept separate from
//! `super::store` because the write path shouldn't need to
//! carry graph knowledge, and separate from `super` because
//! the graph model shouldn't need to know about TOML file
//! layout.

use std::path::{Path, PathBuf};

use super::DeclaredEndpoint;
use super::store::{DeclaredStoreKind, DeclaredStoreSelection};
use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use crate::model::{
    GraphNode, GraphSnapshot, LinkEndpoint, NodeId, RelationKind, RelationKind::RootedAtPath,
};

/// Pick the declared store that should own a new link based
/// on which project (if any) the source or target endpoint
/// resolves to via the live `snapshot`. Falls back to the
/// user config when neither endpoint resolves.
pub fn select_store_for_declaration(
    source: &DeclaredEndpoint,
    target: &DeclaredEndpoint,
    snapshot: &GraphSnapshot,
    loader: &ConfigLoader,
) -> Option<DeclaredStoreSelection> {
    if let Some(root) =
        endpoint_project_root(source, snapshot).or_else(|| endpoint_project_root(target, snapshot))
    {
        let path = loader
            .locate_project_config(&root)
            .unwrap_or_else(|| root.join(PROJECT_CONFIG_FILENAME));
        return Some(DeclaredStoreSelection {
            kind: DeclaredStoreKind::Project,
            path,
        });
    }

    loader
        .user_config_path()
        .map(|path| DeclaredStoreSelection {
            kind: DeclaredStoreKind::User,
            path,
        })
}

/// Convert a discovered [`NodeId`] back into a
/// [`DeclaredEndpoint`]. Used by `conspectus declared confirm`
/// / `ignore` to derive a declared link's endpoints from a
/// candidate link's source / target in the current graph.
pub fn declared_endpoint_from_node_id(id: &NodeId) -> DeclaredEndpoint {
    match id {
        NodeId::Repo(repo) => DeclaredEndpoint::Repo {
            common_dir: repo.common_dir.clone(),
        },
        NodeId::Checkout(checkout) => DeclaredEndpoint::Checkout {
            repo_common_dir: checkout.repo.common_dir.clone(),
            root: checkout.root.clone(),
        },
        NodeId::Workspace(workspace) => DeclaredEndpoint::Workspace {
            root: workspace.root.clone(),
        },
        NodeId::AgentSession(session) => DeclaredEndpoint::AgentSession {
            harness_key: session.harness_key.clone(),
            state_scope: session.state_scope.clone(),
            session_key: session.session_key.clone(),
        },
        NodeId::MuxSession(mux) => DeclaredEndpoint::MuxSession {
            native_id: mux.native_id.clone(),
        },
        NodeId::Pin(pin) => DeclaredEndpoint::Pin { id: pin.id.clone() },
        NodeId::RuntimeProcess(process) => DeclaredEndpoint::RuntimeProcess {
            observation_key: process.observation_key.clone(),
        },
        NodeId::Branch(branch) => DeclaredEndpoint::Branch {
            repo_common_dir: branch.repo.common_dir.clone(),
            refname: branch.refname.clone(),
        },
        NodeId::Fork(fork) => DeclaredEndpoint::Fork {
            provider_source_key: fork.provider_source_key.clone(),
        },
        NodeId::ForgePr(pr) => DeclaredEndpoint::ForgePr {
            provider: pr.provider.clone(),
            host: pr.host.clone(),
            owner: pr.owner.clone(),
            repo: pr.repo.clone(),
            number: pr.number,
        },
    }
}

fn endpoint_project_root(endpoint: &DeclaredEndpoint, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => repo_root(common_dir, snapshot),
        DeclaredEndpoint::Checkout { root, .. } | DeclaredEndpoint::Workspace { root } => {
            Some(PathBuf::from(root))
        }
        DeclaredEndpoint::Branch {
            repo_common_dir, ..
        } => repo_root(repo_common_dir, snapshot),
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => {
            let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
                harness_key.clone(),
                state_scope.clone(),
                session_key.clone(),
            ));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::MuxSession { native_id } => {
            let id = NodeId::MuxSession(crate::model::MuxSessionId::new(native_id.clone()));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::Pin { id } => snapshot
            .pins
            .iter()
            .find(|pin| pin.id == *id)
            .and_then(|pin| nearest_known_root(Path::new(&pin.cwd), snapshot)),
        DeclaredEndpoint::RuntimeProcess { observation_key } => {
            let id = NodeId::RuntimeProcess(crate::model::RuntimeProcessId::new(
                observation_key.clone(),
            ));
            node_cwd(&id, snapshot).and_then(|cwd| nearest_known_root(Path::new(&cwd), snapshot))
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => {
            let id = NodeId::Fork(crate::model::ForkId::new(provider_source_key.clone()));
            fork_root(&id, snapshot).and_then(|root| nearest_known_root(Path::new(&root), snapshot))
        }
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            let id = NodeId::ForgePr(crate::model::ForgePrId::new(
                provider.clone(),
                host.clone(),
                owner.clone(),
                repo.clone(),
                *number,
            ));
            branch_for_pr(&id, snapshot)
                .and_then(|repo_common_dir| repo_root(&repo_common_dir, snapshot))
        }
    }
}

fn repo_root(common_dir: &str, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::Repo(repo) if repo.id.common_dir == common_dir => repo
            .source_paths
            .first()
            .map(PathBuf::from)
            .or_else(|| common_dir_parent(common_dir)),
        _ => None,
    })
}

fn common_dir_parent(common_dir: &str) -> Option<PathBuf> {
    let path = Path::new(common_dir);
    if path.file_name().is_some_and(|name| name == ".git") {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn node_cwd(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::AgentSession(session) if NodeId::AgentSession(session.id.clone()) == *id => {
            session.cwd.clone()
        }
        GraphNode::MuxSession(mux) if NodeId::MuxSession(mux.id.clone()) == *id => mux.cwd.clone(),
        GraphNode::RuntimeProcess(process) if NodeId::RuntimeProcess(process.id.clone()) == *id => {
            process.cwd.clone()
        }
        _ => None,
    })
}

fn fork_root(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    // H-HYG-006 wave 7: consult SnapshotIndex instead of a
    // linear scan. The index is (re)built here per call; a
    // future shared-index passthrough optimization can remove
    // the rebuild if the helper becomes hot.
    let index = crate::model::SnapshotIndex::new(snapshot);
    index.links_for(id, RootedAtPath).iter().find_map(|link| {
        if let LinkEndpoint::Unresolved { evidence } = &link.target {
            return evidence.path.clone();
        }
        None
    })
}

fn branch_for_pr(id: &NodeId, snapshot: &GraphSnapshot) -> Option<String> {
    // H-HYG-006 wave 7: same shape as fork_root.
    let index = crate::model::SnapshotIndex::new(snapshot);
    index
        .links_for(id, RelationKind::BranchHasForgePr)
        .iter()
        .find_map(|link| {
            if let LinkEndpoint::Node {
                id: NodeId::Branch(branch),
            } = &link.target
            {
                return Some(branch.repo.common_dir.clone());
            }
            None
        })
}

fn nearest_known_root(path: &Path, snapshot: &GraphSnapshot) -> Option<PathBuf> {
    let mut candidates = known_project_roots(snapshot);
    candidates.sort_by_key(|root| std::cmp::Reverse(root.as_os_str().len()));
    candidates
        .into_iter()
        .find(|root| path == root || path.starts_with(root))
}

fn known_project_roots(snapshot: &GraphSnapshot) -> Vec<PathBuf> {
    let mut roots = std::collections::BTreeSet::new();
    for node in &snapshot.nodes {
        match node {
            GraphNode::Repo(repo) => {
                roots.extend(repo.source_paths.iter().map(PathBuf::from));
                if let Some(parent) = common_dir_parent(&repo.common_dir) {
                    roots.insert(parent);
                }
            }
            GraphNode::Checkout(worktree) => {
                roots.insert(PathBuf::from(&worktree.root));
            }
            GraphNode::Workspace(workspace) => {
                roots.insert(PathBuf::from(&workspace.root));
            }
            _ => {}
        }
    }
    roots.into_iter().collect()
}
