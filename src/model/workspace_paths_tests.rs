use super::*;
use crate::model::{
    Confidence, Freshness, GraphLink, LinkEndpoint, LinkState, Metadata, Provenance, RepoId,
    SourceMetadata, WorkspaceId,
};

fn membership(workspace_root: &str, member: &str) -> GraphLink {
    let workspace = NodeId::Workspace(WorkspaceId::new(workspace_root.to_string()));
    let mut fields = Metadata::new();
    fields.insert(
        source_field::LOGICAL_PATH.to_string(),
        serde_json::Value::String(format!("{workspace_root}/{member}")),
    );
    GraphLink {
        id: format!("test:{workspace_root}:{member}"),
        source: workspace,
        target: LinkEndpoint::Node {
            id: NodeId::Repo(RepoId::new(format!("/canonical/{member}/.git"))),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: None,
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn index(links: Vec<GraphLink>) -> WorkspacePaths {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.candidate_links = links;
    WorkspacePaths::from_snapshot(&snapshot)
}

fn workspace(root: &str) -> NodeId {
    NodeId::Workspace(WorkspaceId::new(root.to_string()))
}

#[test]
fn innermost_matches_the_root_and_member_subdirs() {
    let paths = index(vec![membership("/ws/sysadmin", "config")]);

    assert_eq!(
        paths.innermost("/ws/sysadmin"),
        Some((&workspace("/ws/sysadmin"), "/ws/sysadmin"))
    );
    assert_eq!(
        paths.innermost("/ws/sysadmin/config/nix/"),
        Some((&workspace("/ws/sysadmin"), "/ws/sysadmin/config"))
    );
    assert_eq!(paths.innermost("/ws/sysadminx"), None);
    assert_eq!(paths.innermost("/ws"), None);
}

#[test]
fn canonical_checkouts_outside_the_tree_do_not_match() {
    // The membership target's checkout lives elsewhere; work there is
    // repo-shared, not workspace-rooted.
    let paths = index(vec![membership("/ws/sysadmin", "config")]);
    assert_eq!(paths.innermost("/canonical/config"), None);
}

#[test]
fn innermost_prefers_the_nested_workspace() {
    let paths = index(vec![
        membership("/ws/outer", "inner"),
        membership("/ws/outer/inner", "repo"),
    ]);

    assert_eq!(
        paths.innermost("/ws/outer/inner/repo/src"),
        Some((&workspace("/ws/outer/inner"), "/ws/outer/inner/repo"))
    );
    assert_eq!(paths.matching("/ws/outer/inner/repo/src").len(), 2);
    assert_eq!(
        paths.innermost("/ws/outer"),
        Some((&workspace("/ws/outer"), "/ws/outer"))
    );
}

#[test]
fn empty_index_matches_nothing() {
    assert_eq!(WorkspacePaths::default().innermost("/anything"), None);
    assert!(index(Vec::new()).matching("/").is_empty());
}
