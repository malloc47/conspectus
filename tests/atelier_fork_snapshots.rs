use std::path::PathBuf;

use conspectus::discovery::atelier::{
    AtelierForkMode, AtelierForkRecord, AtelierForkRepoEntry, AtelierForkState,
    fork_records_fragment,
};
use conspectus::model::{NodeId, WorkspaceId};
use conspectus::output::render_graph_json;
use conspectus::resolve::resolve_snapshot;

#[test]
fn atelier_fork_context_effects_snapshot() {
    let workspace = NodeId::Workspace(WorkspaceId::new("/workspace"));
    let records = vec![
        AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "alpha".to_string(),
            name: "alpha".to_string(),
            parent: None,
            created_epoch: 1,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from("/workspace/.atelier/forks/alpha"),
            read_only: false,
            state: AtelierForkState::Isolated,
            repos: vec![AtelierForkRepoEntry {
                name: "repo-a".to_string(),
                source: PathBuf::from("/sources/repo-a"),
                parent_worktree: PathBuf::from("/workspace/repo-a"),
                fork_worktree: Some(PathBuf::from("/workspace/.atelier/forks/alpha/repo-a")),
                branch: Some("fork/alpha/repo-a".to_string()),
                forked: true,
                link: false,
            }],
            harness: Vec::new(),
        },
        AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "beta".to_string(),
            name: "beta".to_string(),
            parent: Some("alpha".to_string()),
            created_epoch: 2,
            mode: AtelierForkMode::Selected,
            root: PathBuf::from("/workspace/.atelier/forks/beta"),
            read_only: true,
            state: AtelierForkState::Shared,
            repos: vec![AtelierForkRepoEntry {
                name: "repo-b".to_string(),
                source: PathBuf::from("/sources/repo-b"),
                parent_worktree: PathBuf::from("/workspace/repo-b"),
                fork_worktree: None,
                branch: Some("feature/beta".to_string()),
                forked: false,
                link: true,
            }],
            harness: Vec::new(),
        },
        AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "research".to_string(),
            name: "research".to_string(),
            parent: None,
            created_epoch: 3,
            mode: AtelierForkMode::Research,
            root: PathBuf::from("/workspace/.atelier/forks/research"),
            read_only: false,
            state: AtelierForkState::Inherit,
            repos: Vec::new(),
            harness: Vec::new(),
        },
        AtelierForkRecord {
            provider: "atelier".to_string(),
            source_key: "standalone".to_string(),
            name: "standalone".to_string(),
            parent: None,
            created_epoch: 4,
            mode: AtelierForkMode::Worktree,
            root: PathBuf::from("/outside/standalone"),
            read_only: false,
            state: AtelierForkState::Inherit,
            repos: vec![AtelierForkRepoEntry {
                name: "repo-c".to_string(),
                source: PathBuf::from("/sources/repo-c"),
                parent_worktree: PathBuf::from("/workspace/repo-c"),
                fork_worktree: None,
                branch: None,
                forked: false,
                link: false,
            }],
            harness: Vec::new(),
        },
    ];

    let snapshot = resolve_snapshot(fork_records_fragment(&workspace, &records).into_snapshot());
    let rendered = render_graph_json(&snapshot).expect("render fork graph");

    insta::assert_snapshot!("atelier_fork_context_effects", rendered);
}
