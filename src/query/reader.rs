//! SQLite → GraphSnapshot reader (ADR 0043 / ADR 0044).
//!
//! Inverse of [`crate::query::loader::load`]. Reads every table the
//! loader writes into and reconstructs a [`GraphSnapshot`]. The
//! loader is lossless end-to-end:
//! `snapshot → load(conn) → read_snapshot(conn)` round-trips to an
//! equal snapshot after `canonicalize()` (see the round-trip tests
//! at the bottom of this module).
//!
//! Strategy:
//!
//! - Typed `*Node` structs are rebuilt from the per-kind `node_<kind>`
//!   table columns (the structural source of truth).
//! - `NodeId` foreign references inside `candidate_links`,
//!   `resolved_relationships`, `diagnostics`, and `aliases` are
//!   recovered via `serde_json::from_str::<NodeId>` from the JSON
//!   text the loader writes via `serde_json::to_string(&node_id)`
//!   (ADR 0044). The previous `Display`-form parser is gone — serde
//!   handles structural recovery for every variant including ones
//!   with separator characters in their structural fields.
//!
//! Compile-time exhaustiveness lives in two complementary places:
//!
//! - Model-side: the `Ok(NodeKind { … })` constructions below specify
//!   every field of every typed `*Node` struct, so adding a field to
//!   the model breaks this module's build.
//! - Schema-side: `schema::TABLE_COLUMNS` plus the
//!   `schema_columns_match_constants` test catches column drift
//!   between `schema.sql` and the column lists that the loader and
//!   this reader expect. The
//!   `every_node_id_variant_round_trips_through_json` test in this
//!   module catches serde-contract drift in the JSON payloads
//!   themselves.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde_json::Value;

use crate::aliases::AliasOverlay;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence,
    Diagnostic, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId,
    PinCandidate, Provenance, RelationKind, RepoId, RepoNode, ResolvedRelationship,
    RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SourceMetadata, UnresolvedEndpoint,
    WorkspaceId, WorkspaceNode,
};

/// Read a complete [`GraphSnapshot`] from `conn`. The snapshot is
/// equal (modulo `canonicalize()`) to one passed through
/// [`crate::query::loader::load`].
pub fn read_snapshot(conn: &Connection) -> rusqlite::Result<GraphSnapshot> {
    let mut snap = GraphSnapshot::empty();
    read_repos(conn, &mut snap.nodes)?;
    read_checkouts(conn, &mut snap.nodes)?;
    read_workspaces(conn, &mut snap.nodes)?;
    read_agent_sessions(conn, &mut snap.nodes)?;
    read_mux_sessions(conn, &mut snap.nodes)?;
    read_runtime_processes(conn, &mut snap.nodes)?;
    read_branches(conn, &mut snap.nodes)?;
    read_forks(conn, &mut snap.nodes)?;
    read_forge_prs(conn, &mut snap.nodes)?;
    snap.candidate_links = read_candidate_links(conn)?;
    snap.resolved_relationships = read_resolved(conn)?;
    snap.diagnostics = read_diagnostics(conn)?;
    snap.pins = read_pins(conn)?;
    snap.aliases = read_aliases(conn)?;
    Ok(snap)
}

// -----------------------------------------------------------------------------
// Node tables
// -----------------------------------------------------------------------------

fn read_repos(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt =
        conn.prepare("SELECT common_dir, source_paths, remotes FROM node_repos ORDER BY node_id")?;
    let rows = stmt.query_map([], |row| {
        let common_dir: String = row.get(0)?;
        let source_paths: String = row.get(1)?;
        let remotes: String = row.get(2)?;
        Ok(RepoNode {
            id: RepoId::new(common_dir.clone()),
            common_dir,
            source_paths: parse_json_str_array(&source_paths),
            remotes: parse_json_str_array(&remotes),
        })
    })?;
    for repo in rows {
        out.push(GraphNode::Repo(repo?));
    }
    Ok(())
}

fn read_checkouts(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT repo_common_dir, root, git_dir, \
                current_branch_repo_common_dir, current_branch_refname \
         FROM node_checkouts ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let repo_common_dir: String = row.get(0)?;
        let root: String = row.get(1)?;
        let git_dir: Option<String> = row.get(2)?;
        let cb_repo: Option<String> = row.get(3)?;
        let cb_ref: Option<String> = row.get(4)?;
        let current_branch = match (cb_repo, cb_ref) {
            (Some(repo), Some(refname)) => Some(BranchId::new(RepoId::new(repo), refname)),
            _ => None,
        };
        Ok(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common_dir), root.clone()),
            root,
            git_dir,
            current_branch,
        })
    })?;
    for checkout in rows {
        out.push(GraphNode::Checkout(checkout?));
    }
    Ok(())
}

fn read_workspaces(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt =
        conn.prepare("SELECT root, provider_name, name FROM node_workspaces ORDER BY node_id")?;
    let rows = stmt.query_map([], |row| {
        let root: String = row.get(0)?;
        let provider: Option<String> = row.get(1)?;
        let name: Option<String> = row.get(2)?;
        Ok(WorkspaceNode {
            id: WorkspaceId::new(root.clone()),
            root,
            provider,
            name,
        })
    })?;
    for workspace in rows {
        out.push(GraphNode::Workspace(workspace?));
    }
    Ok(())
}

fn read_agent_sessions(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT harness_key, state_scope, session_key, cwd, title, \
                last_message_preview, last_active_epoch, session_kind \
         FROM node_agent_sessions ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let harness_key: String = row.get(0)?;
        let state_scope: String = row.get(1)?;
        let session_key: String = row.get(2)?;
        let raw_kind: Option<String> = row.get(7)?;
        let session_kind = raw_kind.and_then(|k| match k.as_str() {
            "subagent" => Some(crate::model::SessionKind::Subagent),
            "human" => Some(crate::model::SessionKind::Human),
            _ => None,
        });
        Ok(AgentSessionNode {
            id: AgentSessionId::new(&harness_key, state_scope, session_key),
            harness_key,
            cwd: row.get(3)?,
            title: row.get(4)?,
            last_message_preview: row.get(5)?,
            last_active_epoch: row.get(6)?,
            session_kind,
        })
    })?;
    for session in rows {
        out.push(GraphNode::AgentSession(session?));
    }
    Ok(())
}

fn read_mux_sessions(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT node_id, native_id, backend, cwd, active_pane_command, active_pane_pid, \
                active_pane_current_path, active_pane_start_command, \
                client_attached, activity_epoch, created_epoch \
         FROM node_mux_sessions ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let node_id: String = row.get(0)?;
        let native_id: String = row.get(1)?;
        let id_native = node_id
            .strip_prefix("mux_session:")
            .unwrap_or(native_id.as_str());
        Ok(MuxSessionNode {
            id: MuxSessionId::new(id_native),
            native_id,
            backend: row.get(2)?,
            cwd: row.get(3)?,
            active_pane_command: row.get(4)?,
            active_pane_pid: row.get(5)?,
            active_pane_current_path: row.get(6)?,
            active_pane_start_command: row.get(7)?,
            client_attached: row.get::<_, Option<i64>>(8)?.map(|value| value != 0),
            activity_epoch: row.get(9)?,
            created_epoch: row.get(10)?,
        })
    })?;
    for mux in rows {
        out.push(GraphNode::MuxSession(mux?));
    }
    Ok(())
}

fn read_runtime_processes(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT observation_key, pid, parent_pid, root_pane_pid, command, cwd, \
                harness_key, role, depth, observed_epoch \
         FROM node_runtime_processes ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let observation_key: String = row.get(0)?;
        let raw_role: Option<String> = row.get(7)?;
        let role = raw_role.and_then(|role| match role.as_str() {
            "human_agent" => Some(RuntimeProcessRole::HumanAgent),
            "subagent" => Some(RuntimeProcessRole::Subagent),
            "background" => Some(RuntimeProcessRole::Background),
            "shell" => Some(RuntimeProcessRole::Shell),
            "unknown" => Some(RuntimeProcessRole::Unknown),
            _ => None,
        });
        Ok(RuntimeProcessNode {
            id: RuntimeProcessId::new(&observation_key),
            observation_key,
            pid: row.get(1)?,
            parent_pid: row.get(2)?,
            root_pane_pid: row.get(3)?,
            command: row.get(4)?,
            cwd: row.get(5)?,
            harness_key: row.get(6)?,
            role,
            depth: row.get(8)?,
            observed_epoch: row.get(9)?,
        })
    })?;
    for process in rows {
        out.push(GraphNode::RuntimeProcess(process?));
    }
    Ok(())
}

fn read_branches(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT repo_common_dir, refname, current_commit, upstream \
         FROM node_branches ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let repo_common_dir: String = row.get(0)?;
        let refname: String = row.get(1)?;
        Ok(BranchNode {
            id: BranchId::new(RepoId::new(repo_common_dir), refname.clone()),
            refname,
            current_commit: row.get(2)?,
            upstream: row.get(3)?,
        })
    })?;
    for branch in rows {
        out.push(GraphNode::Branch(branch?));
    }
    Ok(())
}

fn read_forks(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT provider_source_key, provider_name, name, scope, capabilities \
         FROM node_forks ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let provider_source_key: String = row.get(0)?;
        let capabilities: String = row.get(4)?;
        Ok(ForkNode {
            id: ForkId::new(&provider_source_key),
            provider: row.get(1)?,
            provider_source_key,
            name: row.get(2)?,
            scope: row.get(3)?,
            capabilities: parse_json_str_array(&capabilities),
        })
    })?;
    for fork in rows {
        out.push(GraphNode::Fork(fork?));
    }
    Ok(())
}

fn read_forge_prs(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT provider_name, host, owner, repo, number, state, url, updated_epoch, is_draft \
         FROM node_forge_prs ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let provider: String = row.get(0)?;
        let host: String = row.get(1)?;
        let owner: String = row.get(2)?;
        let repo: String = row.get(3)?;
        let number: i64 = row.get(4)?;
        let is_draft: i64 = row.get(8)?;
        let number_u64 = u64::try_from(number).unwrap_or(0);
        Ok(ForgePrNode {
            id: ForgePrId::new(&provider, &host, &owner, &repo, number_u64),
            provider,
            host,
            owner,
            repo,
            number: number_u64,
            state: row.get(5)?,
            url: row.get(6)?,
            updated_epoch: row.get(7)?,
            is_draft: is_draft != 0,
        })
    })?;
    for pr in rows {
        out.push(GraphNode::ForgePr(pr?));
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Candidate links / resolved relationships / diagnostics / aliases
// -----------------------------------------------------------------------------

fn read_candidate_links(conn: &Connection) -> rusqlite::Result<Vec<GraphLink>> {
    let mut stmt = conn.prepare(
        "SELECT link_id, source, target_kind, target_node, \
                target_node_type, target_harness_key, target_native_id, \
                target_state_scope, target_path, target_metadata, \
                relation, provenance, confidence, freshness, \
                state, state_reason, state_overridden_by, \
                source_adapter, source_evidence, source_fields \
         FROM candidate_links ORDER BY link_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let link_id: String = row.get(0)?;
        let source_json: String = row.get(1)?;
        let target_kind: String = row.get(2)?;
        let target_node_json: Option<String> = row.get(3)?;
        let target_node_type: Option<String> = row.get(4)?;
        let target_harness_key: Option<String> = row.get(5)?;
        let target_native_id: Option<String> = row.get(6)?;
        let target_state_scope: Option<String> = row.get(7)?;
        let target_path: Option<String> = row.get(8)?;
        let target_metadata: String = row.get(9)?;
        let relation: String = row.get(10)?;
        let provenance: String = row.get(11)?;
        let confidence: String = row.get(12)?;
        let freshness: String = row.get(13)?;
        let state: String = row.get(14)?;
        let state_reason: Option<String> = row.get(15)?;
        let state_overridden_by: Option<String> = row.get(16)?;
        let source_adapter: String = row.get(17)?;
        let source_evidence: Option<String> = row.get(18)?;
        let source_fields: String = row.get(19)?;

        let target = match target_kind.as_str() {
            "node" => LinkEndpoint::Node {
                id: parse_node_id_json_required(target_node_json.as_deref(), 3)?,
            },
            "unresolved" => LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: target_node_type.unwrap_or_default(),
                    harness_key: target_harness_key,
                    native_id: target_native_id,
                    state_scope: target_state_scope,
                    path: target_path,
                    metadata: parse_metadata(&target_metadata),
                },
            },
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(BadEnum(format!("target_kind={other}"))),
                ));
            }
        };
        let link_state = match state.as_str() {
            "active" => LinkState::Active,
            "ignored" => LinkState::Ignored {
                reason: state_reason,
            },
            "overridden" => LinkState::Overridden {
                by: state_overridden_by.unwrap_or_default(),
                reason: state_reason,
            },
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(BadEnum(format!("state={other}"))),
                ));
            }
        };

        Ok(GraphLink {
            id: link_id,
            source: parse_node_id_json(&source_json, 1)?,
            target,
            relation: deserialize_tag::<RelationKind>(&relation, 10)?,
            provenance: deserialize_tag::<Provenance>(&provenance, 11)?,
            confidence: deserialize_tag::<Confidence>(&confidence, 12)?,
            freshness: deserialize_tag::<Freshness>(&freshness, 13)?,
            source_metadata: SourceMetadata {
                adapter: source_adapter,
                evidence: source_evidence,
                fields: parse_metadata(&source_fields),
            },
            state: link_state,
        })
    })?;
    rows.collect()
}

fn read_resolved(conn: &Connection) -> rusqlite::Result<Vec<ResolvedRelationship>> {
    let mut stmt = conn.prepare(
        "SELECT source, target, relation, selected_link_id, competing_link_ids \
         FROM resolved_relationships ORDER BY source, relation, target",
    )?;
    let rows = stmt.query_map([], |row| {
        let source: String = row.get(0)?;
        let target: String = row.get(1)?;
        let relation: String = row.get(2)?;
        let selected: String = row.get(3)?;
        let competing: String = row.get(4)?;
        Ok(ResolvedRelationship {
            source: parse_node_id_json(&source, 0)?,
            target: parse_node_id_json(&target, 1)?,
            relation: deserialize_tag::<RelationKind>(&relation, 2)?,
            selected_link_id: selected,
            competing_link_ids: parse_json_str_array(&competing),
        })
    })?;
    rows.collect()
}

fn read_diagnostics(conn: &Connection) -> rusqlite::Result<Vec<Diagnostic>> {
    let mut stmt = conn.prepare(
        "SELECT kind, link_id, relation, config_path, config_message, \
                conflict_source, conflict_selected_link_id, conflict_competing_link_ids, details \
         FROM diagnostics ORDER BY kind, link_id, conflict_selected_link_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind: String = row.get(0)?;
        match kind.as_str() {
            "unresolved_endpoint" => Ok(Diagnostic::UnresolvedEndpoint {
                link_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                relation: deserialize_tag::<RelationKind>(
                    &row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    2,
                )?,
            }),
            "config" => Ok(Diagnostic::Config {
                path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                message: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            }),
            "conflict" => Ok(Diagnostic::Conflict {
                source: parse_node_id_json(
                    &row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    5,
                )?,
                relation: deserialize_tag::<RelationKind>(
                    &row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    2,
                )?,
                selected_link_id: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                competing_link_ids: parse_json_str_array(
                    &row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                ),
            }),
            // Pin diagnostics (ADR 0057 / ADR 0058) round-trip via
            // the `details` column rather than the columnar shape
            // because their structured fields (pin_id, mux,
            // last_session, competing candidates, etc.) don't fit
            // it cleanly. The loader writes
            // `serde_json::to_string(&Diagnostic)` into `details`
            // for these variants.
            "pin_unbound" | "pin_stale_mux" | "pin_ambiguous" | "pin_drift" => {
                let details = row.get::<_, Option<String>>(8)?.unwrap_or_default();
                serde_json::from_str::<Diagnostic>(&details).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        8,
                        rusqlite::types::Type::Text,
                        Box::new(BadEnum(format!(
                            "diagnostic kind={kind}: details parse: {err}"
                        ))),
                    )
                })
            }
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(BadEnum(format!("diagnostic kind={other}"))),
            )),
        }
    })?;
    rows.collect()
}

fn read_pins(conn: &Connection) -> rusqlite::Result<Vec<PinCandidate>> {
    let mut stmt = conn.prepare("SELECT details FROM pins ORDER BY pin_id")?;
    let rows = stmt.query_map([], |row| {
        let details: String = row.get(0)?;
        serde_json::from_str::<PinCandidate>(&details).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(BadEnum(format!("pin details parse: {err}"))),
            )
        })
    })?;
    rows.collect()
}

fn read_aliases(conn: &Connection) -> rusqlite::Result<AliasOverlay> {
    let mut stmt = conn.prepare("SELECT node, display_name FROM aliases ORDER BY node")?;
    let rows = stmt.query_map([], |row| {
        let node_json: String = row.get(0)?;
        let display_name: String = row.get(1)?;
        Ok((parse_node_id_json(&node_json, 0)?, display_name))
    })?;
    let mut overlay = AliasOverlay::new();
    for entry in rows {
        let (id, name) = entry?;
        overlay.insert(id, name);
    }
    Ok(overlay)
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn parse_json_str_array(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn parse_metadata(s: &str) -> Metadata {
    let parsed: BTreeMap<String, Value> = serde_json::from_str(s).unwrap_or_default();
    parsed
}

fn deserialize_tag<T>(s: &str, idx: usize) -> rusqlite::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let quoted = format!("\"{s}\"");
    serde_json::from_str::<T>(&quoted).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(BadEnum(format!("{s}: {err}"))),
        )
    })
}

/// Deserialize a JSON-encoded [`NodeId`] from a non-NULL endpoint
/// column (ADR 0044). Inverse of `json_node_id` in
/// [`crate::query::loader`].
pub(crate) fn parse_node_id_json(s: &str, idx: usize) -> rusqlite::Result<NodeId> {
    serde_json::from_str::<NodeId>(s).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(BadEnum(format!("NodeId JSON: {err} (input was: {s})"))),
        )
    })
}

/// Variant of [`parse_node_id_json`] for a nullable column that must
/// be populated when its sibling discriminator says so (i.e.
/// `target_node` when `target_kind = 'node'`). Errors if NULL.
fn parse_node_id_json_required(text: Option<&str>, idx: usize) -> rusqlite::Result<NodeId> {
    let s = text.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(BadEnum(
                "target_node required when target_kind='node'".into(),
            )),
        )
    })?;
    parse_node_id_json(s, idx)
}

#[derive(Debug)]
pub(crate) struct BadEnum(pub(crate) String);

impl std::fmt::Display for BadEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BadEnum {}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GraphSnapshot;
    use crate::query::loader::load;
    use crate::query::schema::apply_schema;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory open");
        apply_schema(&conn).expect("apply schema");
        conn
    }

    fn round_trip(mut snap: GraphSnapshot) -> GraphSnapshot {
        let mut conn = fresh_conn();
        load(&snap, &mut conn).expect("load");
        let mut back = read_snapshot(&conn).expect("read_snapshot");
        snap.canonicalize();
        back.canonicalize();
        // Aliases are skipped by serde and not part of canonicalize();
        // they are BTreeMap-keyed already, so equality holds without
        // sorting.
        assert_eq!(snap, back);
        back
    }

    #[test]
    fn empty_snapshot_round_trips() {
        round_trip(GraphSnapshot::empty());
    }

    /// Variant-coverage guard for ADR 0044: every `NodeId` variant
    /// must serde-round-trip through the JSON encoding the loader
    /// writes and the reader reads. This is the symmetric companion
    /// to `schema_columns_match_constants` — the schema test catches
    /// column shape drift; this one catches serde-contract drift
    /// inside the JSON payloads.
    #[test]
    fn every_node_id_variant_round_trips_through_json() {
        use crate::model::{
            AgentSessionId, BranchId, CheckoutId, ForgePrId, ForkId, MuxSessionId, RepoId,
            RuntimeProcessId, WorkspaceId,
        };
        let cases = vec![
            NodeId::Repo(RepoId::new("/r/.git")),
            NodeId::Workspace(WorkspaceId::new("/w")),
            NodeId::MuxSession(MuxSessionId::new("tmux:0")),
            NodeId::RuntimeProcess(RuntimeProcessId::new("tmux:0:12345")),
            NodeId::Fork(ForkId::new("provider:src")),
            NodeId::AgentSession(AgentSessionId::new("claude-code", "default", "abc")),
            NodeId::Checkout(CheckoutId::new(RepoId::new("/r/.git"), "/r")),
            NodeId::Branch(BranchId::new(RepoId::new("/r/.git"), "refs/heads/main")),
            NodeId::ForgePr(ForgePrId::new("github", "github.com", "owner", "repo", 42)),
        ];
        for original in cases {
            let encoded = serde_json::to_string(&original).expect("serialize");
            let decoded =
                parse_node_id_json(&encoded, 0).unwrap_or_else(|e| panic!("decode {encoded}: {e}"));
            assert_eq!(decoded, original, "round-trip {encoded}");
        }
    }

    /// ADR 0044's central correctness claim: structural fields that
    /// would break the prior `Display`-form parser (separator chars
    /// inside `common_dir` / refname / path) round-trip losslessly
    /// through the JSON encoding. Picks values containing every
    /// separator the old parser keyed on: `:`, `@`, `#`, `/`.
    #[test]
    fn endpoints_with_separator_chars_round_trip_through_json() {
        use crate::model::{
            BranchId, CheckoutId, ForgePrId, GraphLink, GraphSnapshot, LinkEndpoint, NodeId,
            RepoId, ResolvedRelationship, SourceMetadata,
        };

        let repo_id = RepoId::new("/weird@repo:path#with/separators/.git");
        let checkout_id = CheckoutId::new(repo_id.clone(), "/weird@repo:path#with/separators");
        let branch_id = BranchId::new(repo_id.clone(), "refs/heads/feature/@odd:tag");
        let pr_id = ForgePrId::new("github", "github.com", "owner", "repo:with#weird@chars", 7);

        let mut snap = GraphSnapshot::empty();
        snap.candidate_links.push(GraphLink {
            id: "link-weird".into(),
            source: NodeId::Checkout(checkout_id.clone()),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id.clone()),
            },
            relation: crate::model::RelationKind::CheckedOutBranch,
            provenance: crate::model::Provenance::Discovered,
            confidence: crate::model::Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: crate::model::LinkState::Active,
        });
        snap.resolved_relationships.push(ResolvedRelationship {
            source: NodeId::Branch(branch_id),
            target: NodeId::ForgePr(pr_id),
            relation: crate::model::RelationKind::BranchHasForgePr,
            selected_link_id: "winner".into(),
            competing_link_ids: vec![],
        });

        round_trip(snap);
    }

    #[test]
    fn full_snapshot_round_trips() {
        // Reuse the fixture builder pattern from the loader tests.
        use crate::model::{
            AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
            Confidence, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
            LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, Provenance,
            RelationKind, RepoId, RepoNode, ResolvedRelationship, RuntimeProcessId,
            RuntimeProcessNode, RuntimeProcessRole, SourceMetadata, UnresolvedEndpoint,
            WorkspaceId, WorkspaceNode,
        };

        let mut snap = GraphSnapshot::empty();

        let repo = RepoNode {
            id: RepoId::new("/r/.git"),
            common_dir: "/r/.git".into(),
            source_paths: vec!["/r".into(), "/r-alt".into()],
            remotes: vec!["origin".into()],
        };
        snap.nodes.push(GraphNode::Repo(repo));

        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new("/r/.git"), "/r"),
            root: "/r".into(),
            git_dir: Some("/r/.git".into()),
            current_branch: Some(BranchId::new(RepoId::new("/r/.git"), "refs/heads/main")),
        }));

        snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new("/w"),
            root: "/w".into(),
            provider: Some("atelier".into()),
            name: Some("ws-name".into()),
        }));

        snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "default", "s1"),
            harness_key: "claude-code".into(),
            cwd: Some("/cwd".into()),
            title: Some("title".into()),
            last_message_preview: Some("hello".into()),
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        }));

        snap.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("tmux:0"),
            native_id: "tmux:0".into(),
            backend: "tmux".into(),
            cwd: Some("/cwd".into()),
            active_pane_command: Some("zsh".into()),
            active_pane_pid: Some(12345),
            active_pane_current_path: Some("/cwd/sub".into()),
            active_pane_start_command: Some("zsh -l".into()),
            client_attached: None,
            activity_epoch: Some(1_700_000_001),
            created_epoch: Some(1_699_000_000),
        }));

        snap.nodes
            .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: RuntimeProcessId::new("tmux:0:12345"),
                observation_key: "tmux:0:12345".into(),
                pid: Some(12345),
                parent_pid: Some(123),
                root_pane_pid: Some(123),
                command: Some("codex".into()),
                cwd: Some("/cwd".into()),
                harness_key: Some("codex".into()),
                role: Some(RuntimeProcessRole::HumanAgent),
                depth: Some(1),
                observed_epoch: Some(1_700_000_003),
            }));

        snap.nodes.push(GraphNode::Branch(BranchNode {
            id: BranchId::new(RepoId::new("/r/.git"), "refs/heads/main"),
            refname: "refs/heads/main".into(),
            current_commit: Some("deadbeef".into()),
            upstream: Some("origin/main".into()),
        }));

        snap.nodes.push(GraphNode::Fork(ForkNode {
            id: ForkId::new("provider:src"),
            provider: "atelier".into(),
            provider_source_key: "provider:src".into(),
            name: Some("fork-name".into()),
            scope: Some("scope".into()),
            capabilities: vec!["a".into(), "b".into()],
        }));

        snap.nodes.push(GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "owner", "repo", 42),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "owner".into(),
            repo: "repo".into(),
            number: 42,
            state: Some("open".into()),
            url: Some("https://github.com/owner/repo/pull/42".into()),
            updated_epoch: Some(1_700_000_002),
            is_draft: true,
        }));

        // Candidate links: one node-target, one unresolved-target, one
        // ignored, one overridden.
        let source = NodeId::AgentSession(AgentSessionId::new("claude-code", "default", "s1"));
        let target = NodeId::MuxSession(MuxSessionId::new("tmux:0"));

        snap.candidate_links.push(GraphLink {
            id: "link-1".into(),
            source: source.clone(),
            target: LinkEndpoint::Node { id: target.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "test-adapter".into(),
                evidence: Some("ev".into()),
                fields: {
                    let mut m = Metadata::new();
                    m.insert("k".into(), serde_json::json!("v"));
                    m
                },
            },
            state: LinkState::Active,
        });

        snap.candidate_links.push(GraphLink {
            id: "link-unresolved".into(),
            source: source.clone(),
            target: LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".into(),
                    harness_key: Some("claude-code".into()),
                    native_id: Some("native".into()),
                    state_scope: Some("default".into()),
                    path: Some("/p".into()),
                    metadata: {
                        let mut m = Metadata::new();
                        m.insert("hint".into(), serde_json::json!(1));
                        m
                    },
                },
            },
            relation: RelationKind::AssociatedWith,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Stale,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });

        snap.candidate_links.push(GraphLink {
            id: "link-ignored".into(),
            source: source.clone(),
            target: LinkEndpoint::Node { id: target.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Ignored {
                reason: Some("user-ignored".into()),
            },
        });

        snap.candidate_links.push(GraphLink {
            id: "link-overridden".into(),
            source: source.clone(),
            target: LinkEndpoint::Node { id: target.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Overridden {
                by: "preferred-link".into(),
                reason: Some("better-source".into()),
            },
        });

        snap.resolved_relationships.push(ResolvedRelationship {
            source: source.clone(),
            target: target.clone(),
            relation: RelationKind::LinkedToMux,
            selected_link_id: "link-1".into(),
            competing_link_ids: vec!["link-ignored".into(), "link-overridden".into()],
        });

        snap.diagnostics.push(Diagnostic::UnresolvedEndpoint {
            link_id: "link-unresolved".into(),
            relation: RelationKind::AssociatedWith,
        });
        snap.diagnostics.push(Diagnostic::Config {
            path: "/etc/x".into(),
            message: "bad".into(),
        });
        snap.diagnostics.push(Diagnostic::Conflict {
            source: source.clone(),
            relation: RelationKind::AssociatedWith,
            selected_link_id: "winner".into(),
            competing_link_ids: vec!["a".into()],
        });

        snap.aliases.insert(source, "Alias Name".into());

        round_trip(snap);
    }
}
