// Extracted from declared.rs H-HYG-011 rolling wave via #[path = "declared_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CheckoutId, CheckoutNode, Confidence, ForgePrId,
    ForkId, ForkNode, Freshness, GraphLink, LinkState, MuxSessionId, MuxSessionNode, RepoId,
    RepoNode, SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
};
use tempfile::TempDir;

fn sample_document() -> DeclaredDocument {
    DeclaredDocument {
        declared: Some(DeclaredSection {
            schema_version: DECLARED_SCHEMA_VERSION,
            links: vec![
                DeclaredLink {
                    id: "codex-alpha-to-editor".to_string(),
                    relation: RelationKind::LinkedToMux,
                    state: DeclaredLinkState::Active,
                    source: DeclaredEndpoint::AgentSession {
                        harness_key: "codex".to_string(),
                        state_scope: "/home/me/.codex".to_string(),
                        session_key: "alpha".to_string(),
                    },
                    target: DeclaredEndpoint::MuxSession {
                        native_id: "tmux:editor".to_string(),
                    },
                    reason: None,
                    overridden_by: None,
                    label: Some("alpha editor".to_string()),
                },
                DeclaredLink {
                    id: "ignore-old-editor".to_string(),
                    relation: RelationKind::LinkedToMux,
                    state: DeclaredLinkState::Ignored,
                    source: DeclaredEndpoint::AgentSession {
                        harness_key: "codex".to_string(),
                        state_scope: "/home/me/.codex".to_string(),
                        session_key: "alpha".to_string(),
                    },
                    target: DeclaredEndpoint::MuxSession {
                        native_id: "tmux:old-editor".to_string(),
                    },
                    reason: Some("stale".to_string()),
                    overridden_by: None,
                    label: None,
                },
            ],
        }),
    }
}

#[test]
fn declared_document_round_trips_through_toml() {
    let document = sample_document();

    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_declared_document(&encoded).expect("parse");

    assert_eq!(decoded, document);
    assert!(encoded.contains("[[declared.links]]"));
    assert!(encoded.contains("type = \"agent_session\""));
}

#[test]
fn missing_declared_section_yields_empty_document() {
    let document = parse_declared_document("[session]\nprojection = \"agent\"\n")
        .expect("parse session-only config");

    assert!(document.links().is_empty());
}

#[test]
fn unknown_keys_are_ignored_for_forward_compatibility() {
    let document = parse_declared_document(
        r#"
            [declared]
            schema_version = 1
            future = "ignored"

            [[declared.links]]
            id = "repo-to-checkout"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "checkout", repo_common_dir = "/repo/.git", root = "/repo" }
            target = { type = "repo", common_dir = "/repo/.git" }
            future_link_key = true
            "#,
    )
    .expect("parse with unknown keys");

    assert_eq!(document.links().len(), 1);
}

#[test]
fn all_endpoint_shapes_parse() {
    let document = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "repo-checkout"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "checkout", repo_common_dir = "/repo/.git", root = "/repo" }
            target = { type = "repo", common_dir = "/repo/.git" }

            [[declared.links]]
            id = "workspace-repo"
            relation = "workspace_contains_repo"
            state = "active"
            source = { type = "workspace", root = "/workspace" }
            target = { type = "repo", common_dir = "/workspace/repo/.git" }

            [[declared.links]]
            id = "branch-pr"
            relation = "branch_has_forge_pr"
            state = "active"
            source = { type = "forge_pr", provider = "github", host = "github.com", owner = "octo", repo = "repo", number = 7 }
            target = { type = "branch", repo_common_dir = "/repo/.git", refname = "refs/heads/main" }

            [[declared.links]]
            id = "fork-session"
            relation = "associated_with"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "fork", provider_source_key = "atelier:alpha" }
            "#,
        )
        .expect("parse endpoints");

    assert_eq!(document.links().len(), 4);
}

#[test]
fn malformed_declared_toml_is_an_error() {
    let err = parse_declared_document("[declared\n").expect_err("malformed");

    assert!(matches!(err, DeclaredParseError::MalformedToml(_)));
}

#[test]
fn unsupported_schema_version_is_an_error() {
    let err = parse_declared_document(
        r#"
            [declared]
            schema_version = 99
            "#,
    )
    .expect_err("unsupported version");

    assert_eq!(err, DeclaredParseError::UnsupportedSchemaVersion(99));
}

#[test]
fn duplicate_declared_ids_are_an_error() {
    let err = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "same"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:a" }

            [[declared.links]]
            id = "same"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s2" }
            target = { type = "mux_session", native_id = "tmux:b" }
            "#,
        )
        .expect_err("duplicate id");

    assert_eq!(err, DeclaredParseError::DuplicateId("same".to_string()));
}

#[test]
fn overridden_links_must_name_replacement() {
    let err = parse_declared_document(
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "old"
            relation = "linked_to_mux"
            state = "overridden"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:a" }
            "#,
        )
        .expect_err("missing overridden_by");

    assert_eq!(
        err,
        DeclaredParseError::MissingOverriddenBy("old".to_string())
    );
}

#[test]
fn store_selection_uses_repo_project_config_when_endpoint_is_repo_rooted() {
    let temp = TempDir::new().expect("temp");
    let repo_root = temp.path().join("repo");
    let common_dir = repo_root.join(".git");
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Repo(RepoNode {
            id: RepoId::new(path_string(&common_dir)),
            common_dir: path_string(&common_dir),
            source_paths: vec![path_string(&repo_root)],
            remotes: Vec::new(),
        })],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::Repo {
            common_dir: path_string(&common_dir),
        },
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.kind, DeclaredStoreKind::Project);
    assert_eq!(selection.path, repo_root.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn store_selection_reuses_existing_workspace_config_for_nested_worktree() {
    let temp = TempDir::new().expect("temp");
    let workspace = temp.path().join("workspace");
    let repo = workspace.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    std::fs::write(workspace.join(PROJECT_CONFIG_FILENAME), "").expect("config");
    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Workspace(WorkspaceNode {
                id: WorkspaceId::new(path_string(&workspace)),
                root: path_string(&workspace),
                provider: None,
                name: None,
            }),
            GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(
                    RepoId::new(path_string(repo.join(".git"))),
                    path_string(&repo),
                ),
                root: path_string(&repo),
                git_dir: None,
                current_branch: None,
            }),
        ],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::Checkout {
            repo_common_dir: path_string(repo.join(".git")),
            root: path_string(&repo),
        },
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.path, workspace.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn store_selection_uses_user_config_for_orphan_agent_and_mux_only_links() {
    let temp = TempDir::new().expect("temp");
    let xdg = temp.path().join("xdg");
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/state".to_string(),
            session_key: "s1".to_string(),
        },
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &GraphSnapshot::empty(),
        &ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg),
    )
    .expect("selection");

    assert_eq!(selection.kind, DeclaredStoreKind::User);
    assert_eq!(
        selection.path,
        xdg.join(crate::config::USER_CONFIG_RELATIVE)
    );
}

#[test]
fn store_selection_uses_session_cwd_when_it_sits_under_known_worktree() {
    let temp = TempDir::new().expect("temp");
    let repo = temp.path().join("repo");
    let child = repo.join("nested");
    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(
                    RepoId::new(path_string(repo.join(".git"))),
                    path_string(&repo),
                ),
                root: path_string(&repo),
                git_dir: None,
                current_branch: None,
            }),
            GraphNode::AgentSession(
                AgentSessionNode::new(
                    AgentSessionId::new("codex", "/state", "s1"),
                    "codex".to_string(),
                )
                .with_cwd(path_string(&child)),
            ),
        ],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/state".to_string(),
            session_key: "s1".to_string(),
        },
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn store_selection_uses_mux_cwd_when_it_sits_under_known_worktree() {
    let temp = TempDir::new().expect("temp");
    let repo = temp.path().join("repo");
    let child = repo.join("nested");
    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Checkout(CheckoutNode {
                id: CheckoutId::new(
                    RepoId::new(path_string(repo.join(".git"))),
                    path_string(&repo),
                ),
                root: path_string(&repo),
                git_dir: None,
                current_branch: None,
            }),
            GraphNode::MuxSession(
                MuxSessionNode::new(
                    MuxSessionId::new("tmux:editor"),
                    "tmux".to_string(),
                    "tmux:editor".to_string(),
                )
                .with_cwd(path_string(&child)),
            ),
        ],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/state".to_string(),
            session_key: "s1".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn store_selection_uses_branch_repo_root_for_branch_pr_links() {
    let temp = TempDir::new().expect("temp");
    let repo = temp.path().join("repo");
    let common_dir = repo.join(".git");
    let branch_id = BranchId::new(RepoId::new(path_string(&common_dir)), "refs/heads/main");
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Repo(RepoNode {
            id: RepoId::new(path_string(&common_dir)),
            common_dir: path_string(&common_dir),
            source_paths: vec![path_string(&repo)],
            remotes: Vec::new(),
        })],
        candidate_links: vec![GraphLink {
            id: "pr-branch".to_string(),
            source: NodeId::ForgePr(pr_id),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance: crate::model::Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::ForgePr {
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "repo".to_string(),
            number: 7,
        },
        &DeclaredEndpoint::Branch {
            repo_common_dir: path_string(&common_dir),
            refname: "refs/heads/main".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.path, repo.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn store_selection_uses_fork_root_when_under_known_workspace() {
    let temp = TempDir::new().expect("temp");
    let workspace = temp.path().join("workspace");
    let fork_root_path = workspace.join("forks/alpha");
    let fork_id = ForkId::new("atelier:alpha");
    let snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::Workspace(WorkspaceNode {
                id: WorkspaceId::new(path_string(&workspace)),
                root: path_string(&workspace),
                provider: None,
                name: None,
            }),
            GraphNode::Fork(ForkNode {
                id: fork_id.clone(),
                provider: "atelier".to_string(),
                provider_source_key: "atelier:alpha".to_string(),
                name: Some("alpha".to_string()),
                scope: None,
                capabilities: Vec::new(),
            }),
        ],
        candidate_links: vec![GraphLink {
            id: "fork-root".to_string(),
            source: NodeId::Fork(fork_id),
            target: LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "path".to_string(),
                    harness_key: None,
                    native_id: None,
                    state_scope: None,
                    path: Some(path_string(&fork_root_path)),
                    metadata: Default::default(),
                },
            },
            relation: RootedAtPath,
            provenance: crate::model::Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }],
        ..GraphSnapshot::empty()
    };
    let selection = select_store_for_declaration(
        &DeclaredEndpoint::Fork {
            provider_source_key: "atelier:alpha".to_string(),
        },
        &DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        &snapshot,
        &ConfigLoader::new().with_home(temp.path()),
    )
    .expect("selection");

    assert_eq!(selection.path, workspace.join(PROJECT_CONFIG_FILENAME));
}

#[test]
fn declared_write_creates_config_and_parent_dirs() {
    let temp = TempDir::new().expect("temp");
    let path = temp
        .path()
        .join("xdg")
        .join(crate::config::USER_CONFIG_RELATIVE);

    let outcome = upsert_declared_link(&path, declared_link("alpha")).expect("write");

    assert!(outcome.changed);
    assert_eq!(outcome.link_count, 1);
    let parsed =
        parse_declared_document(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(parsed.links().len(), 1);
    assert_eq!(parsed.links()[0].id, "alpha");
}

#[test]
fn declared_write_preserves_unrelated_config_sections() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    std::fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");

    upsert_declared_link(&path, declared_link("alpha")).expect("write");

    let text = std::fs::read_to_string(&path).expect("read");
    assert!(text.contains("[session]"));
    assert!(text.contains("projection = \"mux\""));
    assert!(text.contains("[declared]"));
    assert!(text.contains("[[declared.links]]"));
}

#[test]
fn declared_write_sorts_links_by_id() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);

    upsert_declared_link(&path, declared_link("zulu")).expect("write zulu");
    upsert_declared_link(&path, declared_link("alpha")).expect("write alpha");

    let parsed =
        parse_declared_document(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    let ids: Vec<_> = parsed.links().iter().map(|link| link.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "zulu"]);
}

#[test]
fn declared_write_replaces_duplicate_id() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    upsert_declared_link(&path, declared_link("alpha")).expect("write");
    let mut replacement = declared_link("alpha");
    replacement.reason = Some("new reason".to_string());

    let outcome = upsert_declared_link(&path, replacement).expect("replace");

    assert!(outcome.changed);
    let parsed =
        parse_declared_document(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(parsed.links().len(), 1);
    assert_eq!(parsed.links()[0].reason.as_deref(), Some("new reason"));
}

#[test]
fn declared_write_skips_unchanged_replacement() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    let link = declared_link("alpha");
    upsert_declared_link(&path, link.clone()).expect("write");

    let outcome = upsert_declared_link(&path, link).expect("same");

    assert!(!outcome.changed);
    assert_eq!(outcome.link_count, 1);
}

#[test]
fn declared_write_removes_link_by_id() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    upsert_declared_link(&path, declared_link("alpha")).expect("write alpha");
    upsert_declared_link(&path, declared_link("zulu")).expect("write zulu");

    let outcome = remove_declared_link(&path, "alpha").expect("remove");

    assert!(outcome.changed);
    assert_eq!(outcome.link_count, 1);
    let parsed =
        parse_declared_document(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(parsed.links().len(), 1);
    assert_eq!(parsed.links()[0].id, "zulu");
}

#[test]
fn declared_remove_deletes_file_when_last_link_strips_section() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    upsert_declared_link(&path, declared_link("alpha")).expect("write");
    assert!(path.is_file(), "file should be created by upsert");

    let outcome = remove_declared_link(&path, "alpha").expect("remove");

    assert!(outcome.changed);
    assert_eq!(outcome.link_count, 0);
    assert!(
        !path.exists(),
        "file should be deleted when the last declared link is removed",
    );
}

#[test]
fn declared_remove_preserves_unrelated_sections_when_section_empties() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    std::fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");
    upsert_declared_link(&path, declared_link("alpha")).expect("write");

    let outcome = remove_declared_link(&path, "alpha").expect("remove");

    assert!(outcome.changed);
    assert_eq!(outcome.link_count, 0);
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(
        text.contains("[session]"),
        "session section preserved:\n{text}"
    );
    assert!(
        text.contains("projection = \"mux\""),
        "value preserved:\n{text}"
    );
    assert!(
        !text.contains("[declared]"),
        "declared section should be pruned:\n{text}",
    );
}

#[test]
fn declared_write_reports_malformed_existing_toml_without_mutating() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    let original = "[declared\n";
    std::fs::write(&path, original).expect("seed");

    let err = upsert_declared_link(&path, declared_link("alpha")).expect_err("error");

    assert!(matches!(err, DeclaredWriteError::Parse { .. }));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), original);
}

#[test]
fn declared_remove_missing_link_does_not_create_file() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("missing").join(PROJECT_CONFIG_FILENAME);

    let outcome = remove_declared_link(&path, "missing").expect("remove missing");

    assert!(!outcome.changed);
    assert_eq!(outcome.link_count, 0);
    assert!(!path.exists());
}

fn declared_link(id: &str) -> DeclaredLink {
    DeclaredLink {
        id: id.to_string(),
        relation: RelationKind::LinkedToMux,
        state: DeclaredLinkState::Active,
        source: DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/state".to_string(),
            session_key: "s1".to_string(),
        },
        target: DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        reason: None,
        overridden_by: None,
        label: None,
    }
}

fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().to_string()
}

/// H-REF-001: every `DeclaredEndpoint` variant must
/// round-trip through the compact-form codec exposed on the
/// enum. Regressions here mean the CLI parse (`parse_compact`)
/// and the CLI label (`compact_label`) have drifted.
#[test]
fn declared_endpoint_compact_form_round_trips_for_every_variant() {
    let variants = [
        DeclaredEndpoint::Repo {
            common_dir: "/r/.git".to_string(),
        },
        DeclaredEndpoint::Checkout {
            repo_common_dir: "/r/.git".to_string(),
            root: "/r".to_string(),
        },
        DeclaredEndpoint::Workspace {
            root: "/ws".to_string(),
        },
        DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/state".to_string(),
            session_key: "s1".to_string(),
        },
        DeclaredEndpoint::MuxSession {
            native_id: "tmux:editor".to_string(),
        },
        DeclaredEndpoint::Pin {
            id: "pin-1".to_string(),
        },
        DeclaredEndpoint::RuntimeProcess {
            observation_key: "proc-1".to_string(),
        },
        DeclaredEndpoint::Branch {
            repo_common_dir: "/r/.git".to_string(),
            refname: "refs/heads/main".to_string(),
        },
        DeclaredEndpoint::Fork {
            provider_source_key: "github:owner:repo".to_string(),
        },
        DeclaredEndpoint::ForgePr {
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "widget".to_string(),
            number: 42,
        },
    ];
    for endpoint in &variants {
        let label = endpoint.compact_label();
        let parsed =
            DeclaredEndpoint::parse_compact(&label).expect("compact form parses back for variant");
        assert_eq!(
            &parsed, endpoint,
            "compact form did not round-trip for variant"
        );
    }
}

#[test]
fn declared_endpoint_parse_compact_rejects_unknown_kind() {
    let err = DeclaredEndpoint::parse_compact("wonderful:key=value").expect_err("unknown kind");
    assert!(
        err.contains("invalid endpoint syntax"),
        "expected syntax error, got {err:?}"
    );
}

#[test]
fn declared_endpoint_parse_compact_reports_missing_required_field() {
    let err =
        DeclaredEndpoint::parse_compact("repo:not_a_field=x").expect_err("missing required field");
    assert!(
        err.contains("missing endpoint field `common_dir`"),
        "expected required-field error, got {err:?}"
    );
}
