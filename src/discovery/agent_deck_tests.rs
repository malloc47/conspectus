// Extracted from agent_deck.rs H-HYG-011 rolling wave via #[path = "agent_deck_tests.rs"] mod tests;
use std::process::Command;

use tempfile::TempDir;

use super::*;

struct GitRepoFixture {
    root: PathBuf,
}

impl GitRepoFixture {
    fn init_at(parent: &Path, name: &str) -> Self {
        let root = parent.join(name);
        init_repo(&root);
        Self { root }
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

fn fragment_for(root: &Path) -> GraphFragment {
    AgentDeckDiscovery::new(root)
        .discover(&DiscoveryContext::from_root(root))
        .expect("agent-deck discovery succeeds")
}

/// Create a `state.db` under `profile_dir/state.db` populated
/// with the schema agent-deck uses for the `instances` table
/// columns the title lookup consumes. Only `id` and `title` are
/// required by the adapter; other columns mirror the production
/// schema so the fixture stays representative.
fn write_profile_state_db(profile_dir: &Path, instances: &[(&str, &str)]) {
    fs::create_dir_all(profile_dir).expect("create profile dir");
    let db_path = profile_dir.join("state.db");
    let conn = rusqlite::Connection::open(&db_path).expect("open fixture state.db");
    conn.execute_batch(
        "CREATE TABLE instances (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT ''
            );",
    )
    .expect("create instances table");
    for (id, title) in instances {
        conn.execute(
            "INSERT INTO instances (id, title) VALUES (?1, ?2)",
            rusqlite::params![id, title],
        )
        .expect("insert fixture instance");
    }
}

#[test]
fn missing_root_emits_empty_fragment() {
    let temp = TempDir::new().expect("temp dir");
    let missing = temp.path().join("does-not-exist");
    let fragment = fragment_for(&missing);
    assert!(fragment.nodes.is_empty());
    assert!(fragment.candidate_links.is_empty());
}

#[test]
fn two_symlink_workspace_emits_provider_workspace_and_membership() {
    let temp = TempDir::new().expect("temp dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("abc");
    let external = TempDir::new().expect("external dir");
    let repo_a = GitRepoFixture::init_at(external.path(), "atelier");
    let repo_b = GitRepoFixture::init_at(external.path(), "conspectus");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("atelier"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("conspectus"));

    let fragment = fragment_for(&worktrees_root);

    let workspaces: Vec<&WorkspaceNode> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Workspace(w) => Some(w),
            _ => None,
        })
        .collect();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].provider.as_deref(), Some(AGENT_DECK_PROVIDER));
    assert_eq!(workspaces[0].name.as_deref(), Some("abc"));
    assert_eq!(
        workspaces[0].root,
        crate::discovery::path_to_string(&workspace_id_dir)
    );

    let membership: Vec<&GraphLink> = fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
        .collect();
    assert_eq!(membership.len(), 2);
    for link in &membership {
        assert_eq!(link.provenance, Provenance::StrongDiscovered);
        assert_eq!(
            link.source_metadata.fields.get("member_path_kind"),
            Some(&serde_json::Value::String("symlink".to_string()))
        );
        assert!(link.source_metadata.fields.contains_key("logical_path"));
    }
}

#[test]
fn one_symlink_workspace_is_skipped() {
    let temp = TempDir::new().expect("temp dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("solo");
    let external = TempDir::new().expect("external dir");
    let repo = GitRepoFixture::init_at(external.path(), "only");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo.path(), &workspace_id_dir.join("only"));

    let fragment = fragment_for(&worktrees_root);

    assert!(
        fragment
            .nodes
            .iter()
            .all(|node| !matches!(node, GraphNode::Workspace(_)))
    );
    assert!(
        fragment
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::WorkspaceContainsRepo)
    );
}

#[test]
fn broken_symlinks_are_skipped_but_others_count() {
    let temp = TempDir::new().expect("temp dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("broken-mix");
    let external = TempDir::new().expect("external dir");
    let repo_a = GitRepoFixture::init_at(external.path(), "repo-a");
    let repo_b = GitRepoFixture::init_at(external.path(), "repo-b");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("repo-a"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("repo-b"));
    symlink_dir(
        &workspace_id_dir.join("missing-target"),
        &workspace_id_dir.join("broken"),
    );

    let fragment = fragment_for(&worktrees_root);

    assert_eq!(
        fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::WorkspaceContainsRepo)
            .count(),
        2
    );
}

#[test]
fn non_symlink_child_directories_are_ignored() {
    let temp = TempDir::new().expect("temp dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("dir-only");
    let external = TempDir::new().expect("external dir");
    let linked_repo = GitRepoFixture::init_at(external.path(), "linked");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(linked_repo.path(), &workspace_id_dir.join("linked"));
    // Real subdirectory that happens to be a git repo — agent-deck
    // does not author this and the adapter must not pick it up.
    let _inline_repo = GitRepoFixture::init_at(&workspace_id_dir, "inline");

    let fragment = fragment_for(&worktrees_root);

    // Only one symlinked member → below the ≥2 threshold → no
    // workspace emitted. The presence of the inline directory
    // must not promote the count.
    assert!(
        fragment
            .candidate_links
            .iter()
            .all(|link| link.relation != RelationKind::WorkspaceContainsRepo)
    );
}

#[test]
fn multiple_workspaces_under_root_are_independent() {
    let temp = TempDir::new().expect("temp dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let external = TempDir::new().expect("external dir");
    let repo_a = GitRepoFixture::init_at(external.path(), "atelier");
    let repo_b = GitRepoFixture::init_at(external.path(), "conspectus");
    let repo_c = GitRepoFixture::init_at(external.path(), "tooling");
    let ws_one = worktrees_root.join("one");
    let ws_two = worktrees_root.join("two");
    fs::create_dir_all(&ws_one).expect("ws one");
    fs::create_dir_all(&ws_two).expect("ws two");
    symlink_dir(repo_a.path(), &ws_one.join("atelier"));
    symlink_dir(repo_b.path(), &ws_one.join("conspectus"));
    symlink_dir(repo_b.path(), &ws_two.join("conspectus"));
    symlink_dir(repo_c.path(), &ws_two.join("tooling"));

    let fragment = fragment_for(&worktrees_root);

    let names: Vec<&str> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Workspace(w) => w.name.as_deref(),
            _ => None,
        })
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec!["one", "two"]);
}

/// Build a typical agent-deck layout (multi-repo-worktrees +
/// profiles siblings) with two members under one workspace
/// folder and a state.db that registers a title for that
/// folder's id-suffix.
fn build_titled_workspace(
    agent_deck_root: &Path,
    folder_name: &str,
    external: &Path,
    title_rows: &[(&str, &str)],
) {
    let worktrees_root = agent_deck_root.join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join(folder_name);
    let repo_a = GitRepoFixture::init_at(external, &format!("{folder_name}-a"));
    let repo_b = GitRepoFixture::init_at(external, &format!("{folder_name}-b"));
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("a"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("b"));
    write_profile_state_db(
        &agent_deck_root.join("profiles").join("default"),
        title_rows,
    );
}

fn workspace_name(fragment: &GraphFragment) -> Option<String> {
    fragment.nodes.iter().find_map(|node| match node {
        GraphNode::Workspace(w) => w.name.clone(),
        _ => None,
    })
}

#[test]
fn instance_title_overrides_folder_basename() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    // Folder shaped `<title-slug>-<8hex>` mirrors the real
    // agent-deck-renamed shape; the id-suffix `c7cf4c65`
    // matches the instance row prefix.
    build_titled_workspace(
        temp.path(),
        "feature-nix-config-c7cf4c65",
        external.path(),
        &[("c7cf4c65-1776435386", "nix-config")],
    );

    let fragment = fragment_for(&temp.path().join("multi-repo-worktrees"));
    assert_eq!(workspace_name(&fragment).as_deref(), Some("nix-config"));
}

#[test]
fn bare_id_folder_uses_full_name_as_lookup_key() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    build_titled_workspace(
        temp.path(),
        "345062f6",
        external.path(),
        &[("345062f6-1778037324", "atelier-and-config")],
    );

    let fragment = fragment_for(&temp.path().join("multi-repo-worktrees"));
    assert_eq!(
        workspace_name(&fragment).as_deref(),
        Some("atelier-and-config")
    );
}

#[test]
fn missing_state_db_falls_back_to_folder_basename() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("abc");
    let repo_a = GitRepoFixture::init_at(external.path(), "atelier");
    let repo_b = GitRepoFixture::init_at(external.path(), "conspectus");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("atelier"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("conspectus"));
    // Deliberately no profiles directory — adapter must tolerate.

    let fragment = fragment_for(&worktrees_root);
    assert_eq!(workspace_name(&fragment).as_deref(), Some("abc"));
}

#[test]
fn no_matching_instance_keeps_folder_basename() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    build_titled_workspace(
        temp.path(),
        "abc",
        external.path(),
        &[("zzzzzzzz-9999999999", "unrelated")],
    );

    let fragment = fragment_for(&temp.path().join("multi-repo-worktrees"));
    assert_eq!(workspace_name(&fragment).as_deref(), Some("abc"));
}

#[test]
fn empty_title_does_not_replace_folder_basename() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    build_titled_workspace(
        temp.path(),
        "abc",
        external.path(),
        &[("abc-1776435386", "   ")],
    );

    let fragment = fragment_for(&temp.path().join("multi-repo-worktrees"));
    assert_eq!(workspace_name(&fragment).as_deref(), Some("abc"));
}

#[test]
fn multiple_profiles_are_enumerated_first_hit_wins() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("abc");
    let repo_a = GitRepoFixture::init_at(external.path(), "x");
    let repo_b = GitRepoFixture::init_at(external.path(), "y");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("x"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("y"));
    // alpha sorts before beta; alpha's title should win.
    write_profile_state_db(
        &temp.path().join("profiles").join("alpha"),
        &[("abc-1", "from-alpha")],
    );
    write_profile_state_db(
        &temp.path().join("profiles").join("beta"),
        &[("abc-2", "from-beta")],
    );

    let fragment = fragment_for(&worktrees_root);
    assert_eq!(workspace_name(&fragment).as_deref(), Some("from-alpha"));
}

#[test]
fn corrupt_state_db_falls_back_to_folder_basename() {
    let temp = TempDir::new().expect("temp dir");
    let external = TempDir::new().expect("external dir");
    let worktrees_root = temp.path().join("multi-repo-worktrees");
    let workspace_id_dir = worktrees_root.join("abc");
    let repo_a = GitRepoFixture::init_at(external.path(), "x");
    let repo_b = GitRepoFixture::init_at(external.path(), "y");
    fs::create_dir_all(&workspace_id_dir).expect("create workspace dir");
    symlink_dir(repo_a.path(), &workspace_id_dir.join("x"));
    symlink_dir(repo_b.path(), &workspace_id_dir.join("y"));
    // Garbage file at the expected DB path — open / prepare
    // must fail and the adapter must continue.
    let profile_dir = temp.path().join("profiles").join("default");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::write(profile_dir.join("state.db"), b"not a sqlite database").expect("write garbage db");

    let fragment = fragment_for(&worktrees_root);
    assert_eq!(workspace_name(&fragment).as_deref(), Some("abc"));
}
