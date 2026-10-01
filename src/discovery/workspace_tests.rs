use std::process::Command;

use tempfile::TempDir;

use super::*;
use crate::discovery::{DiscoveryContext, LocalDiscoveryConfig, discover_local_with};

#[test]
fn standalone_repo_root_does_not_fabricate_workspace() {
    let repo = GitRepoFixture::init("repo");

    let fragment = GenericWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([repo.path()]).expect("context"))
        .expect("workspace discovery succeeds");

    assert!(fragment.nodes.is_empty());
    assert!(fragment.candidate_links.is_empty());
}

#[test]
fn multi_repo_scan_root_infers_generic_workspace() {
    let temp = TempDir::new().expect("temp dir");
    let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
    let _second = GitRepoFixture::init_at(temp.path(), "repo-b");

    let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
        .expect("local discovery succeeds");

    let workspace_nodes = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::Workspace(_)))
        .count();
    let repo_nodes = snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::Repo(_)))
        .count();
    let workspace_links = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .count();

    assert_eq!(workspace_nodes, 1);
    assert_eq!(repo_nodes, 2);
    assert_eq!(workspace_links, 2);
}

#[test]
fn generic_workspace_links_preserve_member_paths() {
    let workspace = TempDir::new().expect("workspace dir");
    let external = TempDir::new().expect("external dir");
    let direct = GitRepoFixture::init_at(workspace.path(), "repo-a");
    let linked_target = GitRepoFixture::init_at(external.path(), "repo-b");
    let linked_logical_path = workspace.path().join("repo-b-link");
    symlink_dir(linked_target.path(), &linked_logical_path);

    let fragment = GenericWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([workspace.path()]).expect("context"))
        .expect("workspace discovery succeeds");

    let links = fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 2);

    let direct_fields = links
        .iter()
        .find(|link| {
            link.source_metadata.fields.get("logical_path")
                == Some(&serde_json::Value::String(
                    crate::discovery::path_to_string(direct.path()),
                ))
        })
        .expect("direct member link")
        .source_metadata
        .fields
        .clone();
    assert_eq!(
        direct_fields.get("member_path_kind"),
        Some(&serde_json::Value::String("directory".to_string()))
    );
    assert_eq!(
        direct_fields.get("canonical_checkout_root"),
        Some(&serde_json::Value::String(
            crate::discovery::path_to_string(
                &direct.path().canonicalize().expect("direct canonical path")
            )
        ))
    );

    let symlink_fields = links
        .iter()
        .find(|link| {
            link.source_metadata.fields.get("logical_path")
                == Some(&serde_json::Value::String(
                    crate::discovery::path_to_string(&linked_logical_path),
                ))
        })
        .expect("symlink member link")
        .source_metadata
        .fields
        .clone();
    assert_eq!(
        symlink_fields.get("member_path_kind"),
        Some(&serde_json::Value::String("symlink".to_string()))
    );
    assert_eq!(
        symlink_fields.get("canonical_checkout_root"),
        Some(&serde_json::Value::String(
            crate::discovery::path_to_string(
                &linked_target
                    .path()
                    .canonicalize()
                    .expect("linked canonical path")
            )
        ))
    );
}

#[test]
fn generic_workspace_skips_broken_symlink_members() {
    let temp = TempDir::new().expect("temp dir");
    let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
    let _second = GitRepoFixture::init_at(temp.path(), "repo-b");
    symlink_dir(
        &temp.path().join("missing-target"),
        &temp.path().join("broken-link"),
    );

    let fragment = GenericWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([temp.path()]).expect("context"))
        .expect("workspace discovery succeeds");

    let workspace_links = fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .count();

    assert_eq!(workspace_links, 2);
}

#[test]
fn generic_workspace_distinguishes_duplicate_logical_members() {
    let workspace = TempDir::new().expect("workspace dir");
    let external = TempDir::new().expect("external dir");
    let target = GitRepoFixture::init_at(external.path(), "repo");
    let first = workspace.path().join("repo-a");
    let second = workspace.path().join("repo-b");
    symlink_dir(target.path(), &first);
    symlink_dir(target.path(), &second);

    let fragment = GenericWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([workspace.path()]).expect("context"))
        .expect("workspace discovery succeeds");

    let links = fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 2);
    assert_ne!(links[0].id, links[1].id);
    assert!(
        links
            .iter()
            .any(|link| link.id.contains(&crate::discovery::path_to_string(&first))),
        "first logical path should disambiguate a duplicate target: {links:#?}",
    );
    assert!(
        links
            .iter()
            .any(|link| link.id.contains(&crate::discovery::path_to_string(&second))),
        "second logical path should disambiguate a duplicate target: {links:#?}",
    );
}

#[test]
fn single_child_repo_root_stays_repo_only() {
    let temp = TempDir::new().expect("temp dir");
    let _repo = GitRepoFixture::init_at(temp.path(), "repo");

    let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
        .expect("local discovery succeeds");

    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| !matches!(node, GraphNode::Workspace(_)))
    );
}

#[test]
fn provider_workspace_metadata_suppresses_generic_inference() {
    let temp = TempDir::new().expect("temp dir");
    let _first = GitRepoFixture::init_at(temp.path(), "repo-a");
    let _second = GitRepoFixture::init_at(temp.path(), "repo-b");
    fs::write(
        temp.path().join(ATELIER_CONFIG_FILENAME),
        r#"
[workspace]
name = "provider-owned"
"#,
    )
    .expect("write atelier config");

    let fragment = GenericWorkspaceDiscovery::new()
        .discover(&DiscoveryContext::from_roots([temp.path()]).expect("context"))
        .expect("workspace discovery succeeds");

    assert!(fragment.nodes.is_empty());
    assert!(fragment.candidate_links.is_empty());
}

struct GitRepoFixture {
    root: PathBuf,
    _temp: Option<TempDir>,
}

impl GitRepoFixture {
    fn init(name: &str) -> Self {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path().join(name);
        init_repo(&root);
        Self {
            root,
            _temp: Some(temp),
        }
    }

    fn init_at(parent: &Path, name: &str) -> Self {
        let root = parent.join(name);
        init_repo(&root);
        Self { root, _temp: None }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

fn init_repo(root: &Path) {
    fs::create_dir(root).expect("create repo dir");
    git(root, &["init", "--initial-branch", "main"]);
    git(root, &["config", "user.name", "Conspectus Test"]);
    git(
        root,
        &["config", "user.email", "conspectus@example.invalid"],
    );
    fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
    git(root, &["add", "README.md"]);
    git(root, &["commit", "-m", "initial"]);
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

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create symlink");
}

#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("create symlink");
}
