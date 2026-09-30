// Extracted from aliases.rs H-HYG-011 rolling wave via #[path = "aliases_tests.rs"] mod tests;
use super::*;
use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME, USER_CONFIG_RELATIVE};
use tempfile::TempDir;

fn sample_entry(session_key: &str, display_name: &str) -> AliasEntry {
    AliasEntry {
        node: DeclaredEndpoint::AgentSession {
            harness_key: "codex".to_string(),
            state_scope: "/home/me/.codex".to_string(),
            session_key: session_key.to_string(),
        },
        display_name: display_name.to_string(),
        reason: None,
    }
}

#[test]
fn aliases_document_round_trips_through_toml() {
    let document = AliasesDocument {
        aliases: Some(AliasesSection {
            schema_version: ALIASES_SCHEMA_VERSION,
            entries: vec![
                sample_entry("alpha", "ingest-refactor"),
                AliasEntry {
                    node: DeclaredEndpoint::MuxSession {
                        native_id: "editor".to_string(),
                    },
                    display_name: "editor".to_string(),
                    reason: Some("matches harness title intentionally".to_string()),
                },
            ],
        }),
    };

    let encoded = to_toml(&document).expect("serialize");
    let decoded = parse_aliases_document(&encoded).expect("parse");

    assert_eq!(decoded, document);
    assert!(encoded.contains("[[aliases.entries]]"));
    assert!(encoded.contains("display_name"));
    assert!(encoded.contains("type = \"agent_session\""));
}

#[test]
fn missing_aliases_section_yields_empty_document() {
    let document = parse_aliases_document("[session]\nprojection = \"agent\"\n").expect("parse");
    assert!(document.entries().is_empty());
}

#[test]
fn unknown_keys_are_ignored_for_forward_compatibility() {
    let document = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1
            future = "ignored"

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "x"
            future_alias_key = true
            "#,
        )
        .expect("parse with unknown keys");
    assert_eq!(document.entries().len(), 1);
}

#[test]
fn unsupported_schema_version_is_an_error() {
    let err = parse_aliases_document(
        r"
            [aliases]
            schema_version = 99
            ",
    )
    .expect_err("unsupported version");
    assert_eq!(err, AliasParseError::UnsupportedSchemaVersion(99));
}

#[test]
fn empty_display_name_is_an_error() {
    let err = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "   "
            "#,
        )
        .expect_err("empty display name");
    assert!(matches!(err, AliasParseError::EmptyDisplayName(_)));
}

#[test]
fn duplicate_node_is_an_error() {
    let err = parse_aliases_document(
            r#"
            [aliases]
            schema_version = 1

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "first"

            [[aliases.entries]]
            node = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "alpha" }
            display_name = "second"
            "#,
        )
        .expect_err("duplicate node");
    assert!(matches!(err, AliasParseError::DuplicateNode(_)));
}

#[test]
fn upsert_creates_file_and_parent_dirs() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("xdg").join(USER_CONFIG_RELATIVE);
    let outcome = upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("write");
    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 1);

    let parsed = parse_aliases_document(&fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(parsed.entries().len(), 1);
    assert_eq!(parsed.entries()[0].display_name, "name");
}

#[test]
fn upsert_preserves_unrelated_sections() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");

    upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("write");

    let text = fs::read_to_string(&path).expect("read");
    assert!(text.contains("[session]"));
    assert!(text.contains("projection = \"mux\""));
    assert!(text.contains("[aliases]"));
    assert!(text.contains("[[aliases.entries]]"));
}

#[test]
fn upsert_replaces_entry_for_same_node() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    upsert_alias_entry(&path, sample_entry("alpha", "old")).expect("seed");

    let outcome = upsert_alias_entry(&path, sample_entry("alpha", "new")).expect("replace");

    assert!(outcome.changed);
    let parsed = parse_aliases_document(&fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(parsed.entries().len(), 1);
    assert_eq!(parsed.entries()[0].display_name, "new");
}

#[test]
fn upsert_skips_unchanged_replacement() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    let entry = sample_entry("alpha", "name");
    upsert_alias_entry(&path, entry.clone()).expect("seed");

    let outcome = upsert_alias_entry(&path, entry).expect("same");

    assert!(!outcome.changed);
    assert_eq!(outcome.entry_count, 1);
}

#[test]
fn upsert_rejects_empty_display_name() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    let err = upsert_alias_entry(&path, sample_entry("alpha", "   ")).expect_err("empty name");
    assert!(matches!(err, AliasWriteError::EmptyDisplayName));
    assert!(!path.exists());
}

#[test]
fn remove_drops_entry_and_deletes_file_when_section_empties() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("seed");
    assert!(path.is_file());

    let outcome = remove_alias_entry(&path, &sample_entry("alpha", "name").node).expect("remove");

    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 0);
    assert!(!path.exists());
}

#[test]
fn remove_preserves_unrelated_sections_when_section_empties() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    fs::write(&path, "[session]\nprojection = \"mux\"\n").expect("seed");
    upsert_alias_entry(&path, sample_entry("alpha", "name")).expect("seed alias");

    let outcome = remove_alias_entry(&path, &sample_entry("alpha", "name").node).expect("remove");

    assert!(outcome.changed);
    assert_eq!(outcome.entry_count, 0);
    let text = fs::read_to_string(&path).expect("read");
    assert!(
        text.contains("[session]"),
        "session section preserved:\n{text}"
    );
    assert!(
        !text.contains("[aliases]"),
        "aliases section pruned:\n{text}"
    );
}

#[test]
fn remove_missing_entry_does_not_create_file() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("missing").join(PROJECT_CONFIG_FILENAME);

    let outcome =
        remove_alias_entry(&path, &sample_entry("alpha", "x").node).expect("remove missing");

    assert!(!outcome.changed);
    assert_eq!(outcome.entry_count, 0);
    assert!(!path.exists());
}

#[test]
fn upsert_reports_malformed_existing_toml_without_mutating() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);
    let original = "[aliases\n";
    fs::write(&path, original).expect("seed");

    let err = upsert_alias_entry(&path, sample_entry("alpha", "name")).expect_err("malformed");

    assert!(matches!(err, AliasWriteError::Parse { .. }));
    assert_eq!(fs::read_to_string(&path).expect("read"), original);
}

#[test]
fn overlay_insert_if_absent_respects_existing_entry() {
    let mut overlay = AliasOverlay::new();
    let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
        "codex", "/state", "alpha",
    ));
    assert!(overlay.insert_if_absent(id.clone(), "first".to_string()));
    assert_eq!(overlay.get(&id), Some("first"));
    assert!(!overlay.insert_if_absent(id.clone(), "second".to_string()));
    assert_eq!(overlay.get(&id), Some("first"));
}

#[test]
fn resolve_display_label_prefers_alias_over_title() {
    let id = NodeId::AgentSession(crate::model::AgentSessionId::new(
        "codex", "/state", "alpha",
    ));
    let mut overlay = AliasOverlay::new();
    overlay.insert(id.clone(), "alias".to_string());

    assert_eq!(
        resolve_display_label(Some(&overlay), &id, Some("title")),
        Some("alias")
    );
    assert_eq!(
        resolve_display_label(Some(&overlay), &id, None),
        Some("alias")
    );
    assert_eq!(
        resolve_display_label(None, &id, Some("title")),
        Some("title")
    );
    assert_eq!(resolve_display_label(None, &id, None), None);
}

#[test]
fn load_alias_entry_for_node_returns_first_matching_store() {
    let temp = TempDir::new().expect("temp");
    let project = temp.path().join(PROJECT_CONFIG_FILENAME);
    let user = temp.path().join(USER_CONFIG_RELATIVE);
    upsert_alias_entry(&project, sample_entry("alpha", "project-name")).expect("project");
    upsert_alias_entry(&user, sample_entry("alpha", "user-name")).expect("user");

    let (found_path, entry) =
        load_alias_entry_for_node(&[project.clone(), user], &sample_entry("alpha", "x").node)
            .expect("ok")
            .expect("found");

    assert_eq!(found_path, project);
    assert_eq!(entry.display_name, "project-name");
}

#[test]
fn load_alias_entry_for_missing_node_returns_none() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join(PROJECT_CONFIG_FILENAME);

    let result = load_alias_entry_for_node(&[path], &sample_entry("alpha", "x").node).expect("ok");
    assert!(result.is_none());
}

// Touch `ConfigLoader` to keep import-pruning tools honest — this
// module's discovery wiring lives in `crate::discovery::aliases`
// which exercises `ConfigLoader::user_config_path` and friends.
#[test]
fn config_loader_remains_a_dependency_marker() {
    let _ = ConfigLoader::new();
    let _ = alias_node_from_node_id(&NodeId::AgentSession(crate::model::AgentSessionId::new(
        "codex", "/state", "alpha",
    )));
}
