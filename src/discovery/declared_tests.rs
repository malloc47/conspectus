use std::fs;

use tempfile::TempDir;

use super::*;
use crate::config::{PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
use crate::model::{AgentSessionNode, GraphNode, MuxSessionNode, RelationKind};

fn session_node() -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "s1"),
            "codex".to_string(),
        )
        .with_cwd("/work".to_string()),
    )
}

fn mux_node(native_id: &str) -> GraphNode {
    GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new(native_id),
            "tmux".to_string(),
            native_id.to_string(),
        )
        .with_cwd("/work".to_string()),
    )
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
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved target, got {other:?}")
        }
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
