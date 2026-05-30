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
    GraphLink, GraphSnapshot, LinkEndpoint, LinkState, Metadata, MuxSessionId, NodeId, Provenance,
    RepoId, SourceMetadata, UnresolvedEndpoint, WorkspaceId,
};

pub fn apply_declared_links(
    snapshot: &mut GraphSnapshot,
    context: &DiscoveryContext,
    loader: &ConfigLoader,
) {
    let known_nodes: BTreeSet<NodeId> = snapshot.nodes.iter().map(|node| node.id()).collect();
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
            adapter: "declared".to_string(),
            evidence: Some(store.evidence.to_string()),
            fields: metadata(store, declared),
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
        Provenance::LocalDeclared => "local",
        Provenance::GlobalDeclared => "global",
        Provenance::StrongDiscovered
        | Provenance::Discovered
        | Provenance::Convention
        | Provenance::Cached => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::config::{PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
    use crate::model::{AgentSessionNode, GraphNode, MuxSessionNode, RelationKind};

    fn session_node() -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "s1"),
            harness_key: "codex".to_string(),
            cwd: Some("/work".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn mux_node(native_id: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(native_id),
            backend: "tmux".to_string(),
            native_id: native_id.to_string(),
            cwd: Some("/work".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn declared_link_toml(target_mux: &str, state: &str) -> String {
        format!(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "session-mux"
            relation = "linked_to_mux"
            state = "{state}"
            source = {{ type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }}
            target = {{ type = "mux_session", native_id = "{target_mux}" }}
            "#
        )
    }

    #[test]
    fn loads_project_declared_link_as_local_candidate() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            &declared_link_toml("tmux:editor", "active"),
        );
        let mut snapshot = GraphSnapshot {
            nodes: vec![session_node(), mux_node("tmux:editor")],
            ..GraphSnapshot::empty()
        };
        let context = DiscoveryContext::from_roots([project.as_path()]).expect("context");
        let loader = ConfigLoader::new().with_home(temp.path());

        apply_declared_links(&mut snapshot, &context, &loader);

        assert_eq!(snapshot.candidate_links.len(), 1);
        let link = &snapshot.candidate_links[0];
        assert_eq!(link.provenance, Provenance::LocalDeclared);
        assert_eq!(link.relation, RelationKind::LinkedToMux);
        assert!(matches!(link.target, LinkEndpoint::Node { .. }));
    }

    #[test]
    fn loads_user_declared_link_as_global_candidate() {
        let temp = TempDir::new().expect("temp");
        let xdg = temp.path().join("xdg");
        write_file(
            &xdg.join(USER_CONFIG_RELATIVE),
            &declared_link_toml("tmux:editor", "active"),
        );
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session_node(), mux_node("tmux:editor")],
            ..GraphSnapshot::empty()
        };
        let context = DiscoveryContext::from_roots([project.as_path()]).expect("context");
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);

        apply_declared_links(&mut snapshot, &context, &loader);

        assert_eq!(snapshot.candidate_links.len(), 1);
        assert_eq!(
            snapshot.candidate_links[0].provenance,
            Provenance::GlobalDeclared
        );
    }

    #[test]
    fn missing_target_is_preserved_as_unresolved_endpoint() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            &declared_link_toml("tmux:missing", "active"),
        );
        let mut snapshot = GraphSnapshot {
            nodes: vec![session_node()],
            ..GraphSnapshot::empty()
        };
        let context = DiscoveryContext::from_roots([project.as_path()]).expect("context");
        let loader = ConfigLoader::new().with_home(temp.path());

        apply_declared_links(&mut snapshot, &context, &loader);

        match &snapshot.candidate_links[0].target {
            LinkEndpoint::Unresolved { evidence } => {
                assert_eq!(evidence.node_type, "mux_session");
                assert_eq!(evidence.native_id.as_deref(), Some("tmux:missing"));
            }
            other => panic!("expected unresolved target, got {other:?}"),
        }
    }

    #[test]
    fn ignored_declared_entry_maps_to_ignored_link_state() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project");
        let body = declared_link_toml("tmux:editor", "ignored").replace(
            "target = { type = \"mux_session\", native_id = \"tmux:editor\" }",
            "target = { type = \"mux_session\", native_id = \"tmux:editor\" }\nreason = \"noisy\"",
        );
        write_file(&project.join(PROJECT_CONFIG_FILENAME), &body);
        let mut snapshot = GraphSnapshot {
            nodes: vec![session_node(), mux_node("tmux:editor")],
            ..GraphSnapshot::empty()
        };
        let context = DiscoveryContext::from_roots([project.as_path()]).expect("context");
        let loader = ConfigLoader::new().with_home(temp.path());

        apply_declared_links(&mut snapshot, &context, &loader);

        assert_eq!(
            snapshot.candidate_links[0].state,
            LinkState::Ignored {
                reason: Some("noisy".to_string())
            }
        );
    }

    #[test]
    fn malformed_declared_config_yields_diagnostic() {
        let temp = TempDir::new().expect("temp");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[declared]\nschema_version = 99\n",
        );
        let mut snapshot = GraphSnapshot::empty();
        let context = DiscoveryContext::from_roots([project.as_path()]).expect("context");
        let loader = ConfigLoader::new().with_home(temp.path());

        apply_declared_links(&mut snapshot, &context, &loader);

        assert!(snapshot.candidate_links.is_empty());
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| matches!(diagnostic, Diagnostic::Config { .. }))
        );
    }
}
