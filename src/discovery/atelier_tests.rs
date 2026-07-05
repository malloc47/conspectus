// Extracted from atelier.rs H-HYG-011 rolling wave via #[path = "atelier_tests.rs"] mod tests;
use std::fs;
use std::process::Command;

use tempfile::TempDir;

use super::*;
use crate::discovery::{DiscoveryContext, LocalDiscoveryConfig, discover_local_with};
use crate::model::{GraphNode, RelationKind};

#[test]
fn parses_minimal_atelier_workspace_config() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("atelier.toml");
    fs::write(
        &path,
        r#"
[workspace]
name = "demo"
"#,
    )
    .expect("write config");

    let config = AtelierWorkspaceConfig::load(&path).expect("parse config");

    assert_eq!(config.workspace.name, "demo");
    assert!(config.repos.is_empty());
}

#[test]
fn malformed_atelier_workspace_config_returns_parse_error() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("atelier.toml");
    fs::write(&path, "not = [valid").expect("write bad config");

    let error = AtelierWorkspaceConfig::load(&path).expect_err("parse should fail");

    assert!(error.to_string().contains("parsing"));
}

#[test]
fn atelier_workspace_discovery_emits_workspace_and_repo_membership() {
    let fixture = AtelierFixture::new();
    fixture.write_config(
        r#"
[workspace]
name = "demo"

[[repos]]
name = "repo-a"
path = "/source/repo-a"

[[repos]]
name = "repo-b"
path = "/source/repo-b"
"#,
    );
    fixture.init_repo("repo-a");
    fixture.init_repo("repo-b");

    let snapshot = discover_local_with([fixture.root()], LocalDiscoveryConfig::empty())
        .expect("local discovery succeeds");

    let atelier_workspaces = snapshot
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node,
                GraphNode::Workspace(workspace)
                    if workspace.provider.as_deref() == Some("atelier")
            )
        })
        .count();
    let workspace_links = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::WorkspaceContainsRepo
                && link.source_metadata.adapter == "atelier"
        })
        .count();

    assert_eq!(atelier_workspaces, 1);
    assert_eq!(workspace_links, 2);

    let repo_a_fields = snapshot
        .candidate_links
        .iter()
        .find(|link| {
            link.relation == RelationKind::WorkspaceContainsRepo
                && link.source_metadata.adapter == "atelier"
                && link.source_metadata.fields.get("repo_name")
                    == Some(&serde_json::Value::String("repo-a".to_string()))
        })
        .expect("repo-a workspace membership link")
        .source_metadata
        .fields
        .clone();
    assert_eq!(
        repo_a_fields.get("logical_path"),
        Some(&serde_json::Value::String(
            crate::discovery::path_to_string(&fixture.root().join("repo-a"))
        ))
    );
    assert_eq!(
        repo_a_fields.get("provider_source_path"),
        Some(&serde_json::Value::String("/source/repo-a".to_string()))
    );
    assert_eq!(
        repo_a_fields.get("canonical_checkout_root"),
        Some(&serde_json::Value::String(
            crate::discovery::path_to_string(
                &fixture
                    .root()
                    .join("repo-a")
                    .canonicalize()
                    .expect("canonical repo-a")
            )
        ))
    );
    assert_eq!(
        repo_a_fields.get("member_path_kind"),
        Some(&serde_json::Value::String("directory".to_string()))
    );
}

#[test]
fn nested_paths_find_parent_atelier_workspace_config() {
    let fixture = AtelierFixture::new();
    fixture.write_config(
        r#"
[workspace]
name = "demo"

[[repos]]
name = "repo-a"
path = "/source/repo-a"
"#,
    );
    let nested = fixture.root().join("repo-a").join("src");
    fixture.init_repo("repo-a");
    fs::create_dir_all(&nested).expect("create nested path");

    let fragment = AtelierWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([nested]).expect("context"))
        .expect("atelier discovery succeeds");

    assert!(
        fragment
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::Workspace(_)))
    );
}

#[test]
fn missing_fork_index_loads_as_empty() {
    let temp = TempDir::new().expect("temp dir");

    let index = AtelierForkIndex::load(temp.path()).expect("load missing index");

    assert!(index.forks.is_empty());
}

#[test]
fn parses_worktree_selected_research_and_standalone_fork_records() {
    let temp = TempDir::new().expect("temp dir");
    let index_path = AtelierForkIndex::path_for(temp.path());
    fs::create_dir_all(index_path.parent().expect("index parent")).expect("create parent");
    fs::write(
        &index_path,
        r#"
[[forks]]
name = "alpha"
created-epoch = 1
mode = "worktree"
root = ".atelier/forks/alpha"
state = "isolated"

[[forks.repos]]
name = "repo-a"
source = "/src/repo-a"
parent-worktree = "/workspace/repo-a"
fork-worktree = ".atelier/forks/alpha/repo-a"
branch = "fork/alpha/repo-a"
forked = true

[[forks.harness]]
key = "codex"
source-session = "parent-session"
fork-session = "child-session"
capability = "native"

[[forks]]
name = "beta"
parent = "alpha"
created-epoch = 2
mode = "selected"
root = ".atelier/forks/beta"
read-only = true

[[forks.repos]]
name = "repo-b"
source = "/src/repo-b"
parent-worktree = "/workspace/repo-b"
link = true

[[forks]]
name = "research"
created-epoch = 3
mode = "research"
root = ".atelier/forks/research"

[[forks]]
name = "standalone"
created-epoch = 4
mode = "worktree"
root = "/tmp/standalone"

[[forks.repos]]
name = "repo-c"
source = "/src/repo-c"
parent-worktree = "/workspace/repo-c"
"#,
    )
    .expect("write fork index");

    let records = AtelierForkIndex::load(temp.path())
        .expect("load fork index")
        .into_provider_records(temp.path());

    assert_eq!(records.len(), 4);
    assert_eq!(records[0].provider, "atelier");
    assert_eq!(records[0].source_key, "alpha");
    assert_eq!(records[0].mode, AtelierForkMode::Worktree);
    assert_eq!(
        records[0].repos[0].fork_worktree.as_deref(),
        Some(Path::new(".atelier/forks/alpha/repo-a"))
    );
    assert_eq!(records[1].mode, AtelierForkMode::Selected);
    assert_eq!(records[1].parent.as_deref(), Some("alpha"));
    assert_eq!(records[2].mode, AtelierForkMode::Research);
    assert_eq!(records[3].root, PathBuf::from("/tmp/standalone"));
}

#[test]
fn harness_lineage_emits_unresolved_parent_and_child_session_evidence() {
    let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
    let record = AtelierForkRecord {
        provider: "atelier".to_string(),
        source_key: "alpha".to_string(),
        name: "alpha".to_string(),
        parent: None,
        created_epoch: 1,
        mode: AtelierForkMode::Worktree,
        root: PathBuf::from("/workspace/.atelier/forks/alpha"),
        read_only: false,
        state: AtelierForkState::Isolated,
        repos: Vec::new(),
        harness: vec![AtelierForkHarnessEntry {
            key: "codex".to_string(),
            source_session: Some("parent-session".to_string()),
            fork_session: Some("child-session".to_string()),
            capability: AtelierHarnessCapability::Native,
            degraded_warning: None,
        }],
    };

    let fragment = fork_records_fragment(&workspace, &[record]);
    let parent_link = lineage_link(&fragment, RelationKind::ParentSession);
    let child_link = lineage_link(&fragment, RelationKind::ChildSession);

    let parent_endpoint = unresolved_endpoint(parent_link);
    assert_eq!(parent_endpoint.node_type, "agent_session");
    assert_eq!(parent_endpoint.harness_key.as_deref(), Some("codex"));
    assert_eq!(parent_endpoint.native_id.as_deref(), Some("parent-session"));
    assert_eq!(
        parent_endpoint.path.as_deref(),
        Some("/workspace/.atelier/forks/alpha")
    );
    assert_eq!(
        parent_endpoint.metadata.get("lineage_kind"),
        Some(&serde_json::Value::String("fork".to_string()))
    );
    assert_eq!(
        parent_endpoint.metadata.get("lineage_fidelity"),
        Some(&serde_json::Value::String("native".to_string()))
    );

    assert_eq!(parent_link.confidence, Confidence::High);
    assert_eq!(parent_link.provenance, Provenance::StrongDiscovered);

    assert_eq!(
        unresolved_endpoint(child_link).native_id.as_deref(),
        Some("child-session")
    );
}

#[test]
fn each_lineage_capability_maps_to_unresolved_endpoint() {
    let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
    let cases = [
        (
            AtelierHarnessCapability::Native,
            "fork",
            "native",
            Confidence::High,
        ),
        (
            AtelierHarnessCapability::Approximate,
            "fork",
            "approximate",
            Confidence::Medium,
        ),
        (
            AtelierHarnessCapability::Unsupported,
            "fork",
            "unsupported",
            Confidence::Low,
        ),
        (
            AtelierHarnessCapability::Fresh,
            "fresh",
            "fresh",
            Confidence::Low,
        ),
    ];

    for (capability, kind_name, fidelity_name, expected_confidence) in cases {
        let record = AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: format!("fork-{kind_name}"),
            name: format!("fork-{kind_name}"),
            parent: None,
            created_epoch: 1,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from(format!("/workspace/.atelier/forks/fork-{kind_name}")),
            read_only: false,
            state: AtelierForkState::Inherit,
            repos: Vec::new(),
            harness: vec![AtelierForkHarnessEntry {
                key: "codex".to_string(),
                source_session: Some(format!("parent-{kind_name}")),
                fork_session: Some(format!("child-{kind_name}")),
                capability,
                degraded_warning: Some("provider degraded".to_string()),
            }],
        };
        let fragment = fork_records_fragment(&workspace, &[record]);
        let parent_link = lineage_link(&fragment, RelationKind::ParentSession);

        assert_eq!(
            parent_link.confidence, expected_confidence,
            "capability {capability:?} should map to {expected_confidence:?}"
        );
        assert_eq!(
            parent_link
                .source_metadata
                .fields
                .get("lineage_kind")
                .and_then(serde_json::Value::as_str),
            Some(kind_name)
        );
        assert_eq!(
            parent_link
                .source_metadata
                .fields
                .get("lineage_fidelity")
                .and_then(serde_json::Value::as_str),
            Some(fidelity_name)
        );
        assert_eq!(
            parent_link
                .source_metadata
                .fields
                .get("degraded_warning")
                .and_then(serde_json::Value::as_str),
            Some("provider degraded")
        );
    }
}

#[test]
fn fresh_session_without_source_emits_only_child_link() {
    let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
    let record = AtelierForkRecord {
        provider: "atelier".to_string(),
        source_key: "fresh-fork".to_string(),
        name: "fresh-fork".to_string(),
        parent: None,
        created_epoch: 1,
        mode: AtelierForkMode::Worktree,
        root: PathBuf::from("/workspace/.atelier/forks/fresh"),
        read_only: false,
        state: AtelierForkState::Isolated,
        repos: Vec::new(),
        harness: vec![AtelierForkHarnessEntry {
            key: "codex".to_string(),
            source_session: None,
            fork_session: Some("brand-new".to_string()),
            capability: AtelierHarnessCapability::Fresh,
            degraded_warning: None,
        }],
    };

    let fragment = fork_records_fragment(&workspace, &[record]);

    assert!(
        fragment
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::ParentSession),
        "fresh sessions without source_session must not invent a parent link"
    );

    let child = lineage_link(&fragment, RelationKind::ChildSession);
    assert_eq!(
        unresolved_endpoint(child).metadata.get("lineage_kind"),
        Some(&serde_json::Value::String("fresh".to_string()))
    );
}

#[test]
fn harness_lineage_does_not_emit_placeholder_session_nodes() {
    let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
    let record = AtelierForkRecord {
        provider: "atelier".to_string(),
        source_key: "alpha".to_string(),
        name: "alpha".to_string(),
        parent: None,
        created_epoch: 1,
        mode: AtelierForkMode::Worktree,
        root: PathBuf::from("/workspace/.atelier/forks/alpha"),
        read_only: false,
        state: AtelierForkState::Isolated,
        repos: Vec::new(),
        harness: vec![AtelierForkHarnessEntry {
            key: "codex".to_string(),
            source_session: Some("parent-session".to_string()),
            fork_session: Some("child-session".to_string()),
            capability: AtelierHarnessCapability::Native,
            degraded_warning: None,
        }],
    };

    let fragment = fork_records_fragment(&workspace, &[record]);

    assert!(
        !fragment
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::AgentSession(_))),
        "atelier must not fabricate AgentSession nodes from lineage evidence"
    );
}

fn lineage_link(fragment: &GraphFragment, relation: RelationKind) -> &GraphLink {
    fragment
        .candidate_links
        .iter()
        .find(|link| link.relation == relation)
        .unwrap_or_else(|| panic!("expected lineage link with relation {relation:?}"))
}

fn unresolved_endpoint(link: &GraphLink) -> &UnresolvedEndpoint {
    match &link.target {
        LinkEndpoint::Unresolved { evidence } => evidence,
        LinkEndpoint::Node { .. } => {
            panic!("expected unresolved endpoint, got concrete node target")
        }
    }
}

#[test]
fn malformed_fork_index_returns_parse_error() {
    let temp = TempDir::new().expect("temp dir");
    let index_path = AtelierForkIndex::path_for(temp.path());
    fs::create_dir_all(index_path.parent().expect("index parent")).expect("create parent");
    fs::write(
        &index_path,
        r#"
[[forks]]
name = "bad"
created-epoch = 1
mode = "not-a-mode"
root = ".atelier/forks/bad"
"#,
    )
    .expect("write bad fork index");

    let error = AtelierForkIndex::load(temp.path()).expect_err("parse should fail");

    assert!(error.to_string().contains("parsing"));
}

struct AtelierFixture {
    temp: TempDir,
}

impl AtelierFixture {
    fn new() -> Self {
        Self {
            temp: TempDir::new().expect("temp dir"),
        }
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn write_config(&self, text: &str) {
        fs::write(self.root().join("atelier.toml"), text).expect("write atelier config");
    }

    fn init_repo(&self, name: &str) {
        let root = self.root().join(name);
        fs::create_dir(&root).expect("create repo dir");
        git(&root, &["init", "--initial-branch", "main"]);
        git(&root, &["config", "user.name", "Conspectus Test"]);
        git(
            &root,
            &["config", "user.email", "conspectus@example.invalid"],
        );
        fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
        git(&root, &["add", "README.md"]);
        git(&root, &["commit", "-m", "initial"]);
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git command");

    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}
