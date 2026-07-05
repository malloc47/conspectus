// Extracted from aliases.rs H-HYG-011 rolling wave via #[path = "aliases_tests.rs"] mod tests;
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

    let expected = NodeId::AgentSession(AgentSessionId::new("codex", "/home/me/.codex", "alpha"));
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
