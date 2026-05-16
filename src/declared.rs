//! Declared-link TOML file models.
//!
//! These types model the durable `[declared]` config section from ADR 0014.
//! Loading them does not imply graph discovery or persistence; later phases
//! convert validated entries into `GraphLink` evidence and write them back.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::model::RelationKind;

pub const DECLARED_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<DeclaredSection>,
}

impl DeclaredDocument {
    pub fn links(&self) -> &[DeclaredLink] {
        self.declared
            .as_ref()
            .map(|declared| declared.links.as_slice())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredSection {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<DeclaredLink>,
}

impl Default for DeclaredSection {
    fn default() -> Self {
        Self {
            schema_version: DECLARED_SCHEMA_VERSION,
            links: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeclaredLink {
    pub id: String,
    pub relation: RelationKind,
    pub state: DeclaredLinkState,
    pub source: DeclaredEndpoint,
    pub target: DeclaredEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredLinkState {
    Active,
    Ignored,
    Overridden,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeclaredEndpoint {
    Repo {
        common_dir: String,
    },
    Worktree {
        repo_common_dir: String,
        root: String,
    },
    Workspace {
        root: String,
    },
    AgentSession {
        harness_key: String,
        state_scope: String,
        session_key: String,
    },
    MuxSession {
        native_id: String,
    },
    Branch {
        repo_common_dir: String,
        refname: String,
    },
    Fork {
        provider_source_key: String,
    },
    ForgePr {
        provider: String,
        host: String,
        owner: String,
        repo: String,
        number: u64,
    },
}

pub fn parse_declared_document(text: &str) -> Result<DeclaredDocument, DeclaredParseError> {
    let document: DeclaredDocument =
        toml::from_str(text).map_err(|err| DeclaredParseError::MalformedToml(err.to_string()))?;
    validate_document(&document)?;
    Ok(document)
}

pub fn to_toml(document: &DeclaredDocument) -> Result<String, DeclaredSerializeError> {
    toml::to_string_pretty(document).map_err(|err| DeclaredSerializeError(err.to_string()))
}

fn validate_document(document: &DeclaredDocument) -> Result<(), DeclaredParseError> {
    let Some(declared) = &document.declared else {
        return Ok(());
    };

    if declared.schema_version != DECLARED_SCHEMA_VERSION {
        return Err(DeclaredParseError::UnsupportedSchemaVersion(
            declared.schema_version,
        ));
    }

    let mut ids = BTreeSet::new();
    for link in &declared.links {
        if link.id.trim().is_empty() {
            return Err(DeclaredParseError::EmptyId);
        }
        if !ids.insert(link.id.clone()) {
            return Err(DeclaredParseError::DuplicateId(link.id.clone()));
        }
        if link.state == DeclaredLinkState::Overridden && link.overridden_by.is_none() {
            return Err(DeclaredParseError::MissingOverriddenBy(link.id.clone()));
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredParseError {
    MalformedToml(String),
    UnsupportedSchemaVersion(u32),
    EmptyId,
    DuplicateId(String),
    MissingOverriddenBy(String),
}

impl fmt::Display for DeclaredParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedToml(err) => write!(f, "malformed declared-link TOML: {err}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported declared.schema_version `{version}`")
            }
            Self::EmptyId => write!(f, "declared link id must not be empty"),
            Self::DuplicateId(id) => write!(f, "duplicate declared link id `{id}`"),
            Self::MissingOverriddenBy(id) => {
                write!(f, "overridden declared link `{id}` must set overridden_by")
            }
        }
    }
}

impl std::error::Error for DeclaredParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredSerializeError(String);

impl fmt::Display for DeclaredSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to serialize declared-link TOML: {}", self.0)
    }
}

impl std::error::Error for DeclaredSerializeError {}

#[cfg(test)]
mod tests {
    use super::*;

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
            id = "repo-to-worktree"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "worktree", repo_common_dir = "/repo/.git", root = "/repo" }
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
            id = "repo-worktree"
            relation = "belongs_to_repo"
            state = "active"
            source = { type = "worktree", repo_common_dir = "/repo/.git", root = "/repo" }
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
}
