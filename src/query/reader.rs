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
            LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, PinBinding,
            PinCandidate, PinLastSession, PinMuxRef, Provenance, RelationKind, RepoId, RepoNode,
            ResolvedRelationship, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole,
            SourceMetadata, UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
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

        // Cover every Diagnostic pin-* variant in the same fixture
        // — these were latent in the SQLite layer until the
        // `details: TEXT` column landed.
        snap.diagnostics.push(Diagnostic::PinUnbound {
            pin_id: "p-unbound".into(),
            expected_mux_native_id: "tmux:p-unbound".into(),
            last_session: Some(PinLastSession {
                session_id: "prior".into(),
                observed_epoch: 1_700_000_004,
            }),
        });
        snap.diagnostics.push(Diagnostic::PinUnbound {
            pin_id: "p-unbound-cold".into(),
            expected_mux_native_id: "tmux:cold".into(),
            last_session: None,
        });
        snap.diagnostics.push(Diagnostic::PinStaleMux {
            pin_id: "p-stale".into(),
            mux: MuxSessionId::new("tmux:0"),
        });
        snap.diagnostics.push(Diagnostic::PinAmbiguous {
            pin_id: "p-ambig".into(),
            chosen: AgentSessionId::new("codex", "default", "winner"),
            competing: vec![AgentSessionId::new("codex", "default", "alt")],
        });
        snap.diagnostics.push(Diagnostic::PinDrift {
            pin_id: "p-drift".into(),
            declared_cwd: "/p/old".into(),
            observed_cwd: "/p/new".into(),
        });

        // Cover every PinBinding state, including binding = None.
        snap.pins.push(PinCandidate {
            id: "pin-bound".into(),
            display_name: "Bound Pin".into(),
            harness: "codex".into(),
            cwd: "/p/work".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "bound".into(),
                socket_name: None,
            },
            launch_argv: Some(vec!["codex".into(), "--resume".into()]),
            reason: Some("paired".into()),
            provenance: Provenance::LocalPin,
            store_path: "/p/work/.conspectus.toml".into(),
            binding: Some(PinBinding::Bound {
                mux: MuxSessionId::new("tmux:bound"),
                session: AgentSessionId::new("codex", "default", "winner"),
            }),
        });
        snap.pins.push(PinCandidate {
            id: "pin-stale".into(),
            display_name: "Stale Pin".into(),
            harness: "claude-code".into(),
            cwd: "/p/work".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "stale".into(),
                socket_name: Some("scratch".into()),
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::GlobalPin,
            store_path: "/home/op/.config/conspectus/config.toml".into(),
            binding: Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:scratch:stale"),
            }),
        });
        snap.pins.push(PinCandidate {
            id: "pin-unbound".into(),
            display_name: "Unbound Pin".into(),
            harness: "opencode".into(),
            cwd: "/p/work".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "unbound".into(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/work/.conspectus.toml".into(),
            binding: Some(PinBinding::Unbound),
        });
        snap.pins.push(PinCandidate {
            id: "pin-preresolve".into(),
            display_name: "Pre-resolve Pin".into(),
            harness: "aider".into(),
            cwd: "/p/work".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "preresolve".into(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/work/.conspectus.toml".into(),
            binding: None,
        });

        snap.aliases.insert(source, "Alias Name".into());

        round_trip(snap);
    }

    // ========================================================
    // P10-FU-002: GraphSnapshot round-trip audit + drift catches
    // ========================================================
    //
    // The two latent bugs that motivated P10-FU-002 (Diagnostic
    // pin-* variants going unhandled in the reader, and pins never
    // being inserted at all) both got past the existing drift
    // catches because those check schema-vs-constants — not
    // model-vs-tables. The tests below close that gap with two
    // compile-time forcing patterns:
    //
    // 1. **Exhaustive destructure on GraphSnapshot.** Adding a new
    //    top-level field is a compile error in
    //    `graph_snapshot_field_drift_guard` until the test (and,
    //    presumably, the loader/reader pair) accounts for it.
    //
    // 2. **Exhaustive match on each carrying enum.** Adding a new
    //    `Diagnostic` / `PinBinding` / `LinkState` / `LinkEndpoint`
    //    variant is a compile error in the corresponding
    //    `every_*_round_trips` test until the matrix covers it.
    //
    // Both patterns mirror the loader's existing exhaustive
    // destructures (`let RepoNode { … } = repo;` etc.) at the
    // snapshot level instead of the per-table level.

    /// Compile-time guard. Adding a new field to `GraphSnapshot`
    /// without updating this destructure fails to build, which
    /// forces the developer to also update `load`, `read_snapshot`,
    /// `clear_all`, and the matrix tests below. Pattern is
    /// intentionally inert at runtime — the value is in the
    /// destructure itself.
    #[test]
    fn graph_snapshot_field_drift_guard() {
        let snap = GraphSnapshot::empty();
        let GraphSnapshot {
            nodes,
            candidate_links,
            resolved_relationships,
            diagnostics,
            aliases,
            pins,
        } = snap;
        // Touch each binding so an unused warning surfaces if any
        // field disappears (the destructure alone catches additions;
        // these asserts make removals visible too).
        assert!(nodes.is_empty());
        assert!(candidate_links.is_empty());
        assert!(resolved_relationships.is_empty());
        assert!(diagnostics.is_empty());
        assert!(aliases.iter().next().is_none());
        assert!(pins.is_empty());
    }

    /// Round-trip every `Diagnostic` variant individually so a
    /// mismatch isolates the broken kind. Exhaustive match forces
    /// a new variant to be added here before it can land.
    #[test]
    fn every_diagnostic_variant_round_trips() {
        use crate::model::{
            AgentSessionId, Diagnostic, MuxSessionId, NodeId, PinLastSession, RelationKind,
        };

        let source = NodeId::AgentSession(AgentSessionId::new("codex", "default", "src"));
        let cases: Vec<Diagnostic> = vec![
            Diagnostic::UnresolvedEndpoint {
                link_id: "L".into(),
                relation: RelationKind::LinkedToMux,
            },
            Diagnostic::Config {
                path: "/etc/x".into(),
                message: "bad".into(),
            },
            Diagnostic::Conflict {
                source: source.clone(),
                relation: RelationKind::AssociatedWith,
                selected_link_id: "winner".into(),
                competing_link_ids: vec!["a".into(), "b".into()],
            },
            Diagnostic::PinUnbound {
                pin_id: "p".into(),
                expected_mux_native_id: "tmux:p".into(),
                last_session: Some(PinLastSession {
                    session_id: "prior".into(),
                    observed_epoch: 1,
                }),
            },
            Diagnostic::PinUnbound {
                pin_id: "p2".into(),
                expected_mux_native_id: "tmux:p2".into(),
                last_session: None,
            },
            Diagnostic::PinStaleMux {
                pin_id: "p3".into(),
                mux: MuxSessionId::new("tmux:p3"),
            },
            Diagnostic::PinAmbiguous {
                pin_id: "p4".into(),
                chosen: AgentSessionId::new("codex", "default", "w"),
                competing: vec![AgentSessionId::new("codex", "default", "a")],
            },
            Diagnostic::PinDrift {
                pin_id: "p5".into(),
                declared_cwd: "/a".into(),
                observed_cwd: "/b".into(),
            },
        ];

        // Exhaustive-match drift catch: every variant in
        // `Diagnostic` must be tagged below. Adding a variant fails
        // to compile until the matcher has an arm for it.
        for diagnostic in &cases {
            match diagnostic {
                Diagnostic::UnresolvedEndpoint { .. }
                | Diagnostic::Config { .. }
                | Diagnostic::Conflict { .. }
                | Diagnostic::PinUnbound { .. }
                | Diagnostic::PinStaleMux { .. }
                | Diagnostic::PinAmbiguous { .. }
                | Diagnostic::PinDrift { .. } => {}
            }
        }

        for diagnostic in cases {
            let mut snap = GraphSnapshot::empty();
            snap.diagnostics.push(diagnostic.clone());
            let back = round_trip(snap);
            assert_eq!(back.diagnostics.len(), 1, "{diagnostic:?}");
            assert_eq!(back.diagnostics[0], diagnostic);
        }
    }

    /// Round-trip every `PinBinding` state plus the `binding = None`
    /// case (pre-resolve pins). Exhaustive match forces a new
    /// `PinBinding` variant to be added here before it can land.
    #[test]
    fn every_pin_binding_state_round_trips() {
        use crate::model::{
            AgentSessionId, MuxSessionId, PinBinding, PinCandidate, PinMuxRef, Provenance,
        };

        let mut pin = PinCandidate {
            id: "pin".into(),
            display_name: "Pin".into(),
            harness: "codex".into(),
            cwd: "/p".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "pin".into(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".into(),
            binding: None,
        };

        let states: Vec<Option<PinBinding>> = vec![
            None,
            Some(PinBinding::Bound {
                mux: MuxSessionId::new("tmux:pin"),
                session: AgentSessionId::new("codex", "default", "w"),
            }),
            Some(PinBinding::StaleMux {
                mux: MuxSessionId::new("tmux:pin"),
            }),
            Some(PinBinding::Unbound),
        ];

        // Exhaustive-match drift catch: every `PinBinding` variant
        // must be tagged below.
        for state in &states {
            match state {
                None => {}
                Some(PinBinding::Bound { .. })
                | Some(PinBinding::StaleMux { .. })
                | Some(PinBinding::Unbound) => {}
            }
        }

        for state in states {
            pin.binding = state.clone();
            let mut snap = GraphSnapshot::empty();
            snap.pins.push(pin.clone());
            let back = round_trip(snap);
            assert_eq!(back.pins.len(), 1, "{state:?}");
            assert_eq!(back.pins[0].binding, state);
        }
    }

    /// Round-trip every `LinkState` variant — the carried payloads
    /// (`reason`, `by`) need to survive the columnar shape used by
    /// `candidate_links`. Exhaustive match guards against new
    /// variants being added without coverage.
    #[test]
    fn every_link_state_round_trips() {
        use crate::model::{
            AgentSessionId, Confidence, Freshness, GraphLink, LinkEndpoint, LinkState,
            MuxSessionId, NodeId, Provenance, RelationKind, SourceMetadata,
        };

        let source = NodeId::AgentSession(AgentSessionId::new("codex", "default", "src"));
        let target_endpoint = LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new("tmux:0")),
        };

        let states: Vec<LinkState> = vec![
            LinkState::Active,
            LinkState::Ignored { reason: None },
            LinkState::Ignored {
                reason: Some("user opted out".into()),
            },
            LinkState::Overridden {
                by: "preferred-link".into(),
                reason: None,
            },
            LinkState::Overridden {
                by: "preferred-link".into(),
                reason: Some("better evidence".into()),
            },
        ];

        // Exhaustive-match drift catch.
        for state in &states {
            match state {
                LinkState::Active | LinkState::Ignored { .. } | LinkState::Overridden { .. } => {}
            }
        }

        for (idx, state) in states.into_iter().enumerate() {
            let mut snap = GraphSnapshot::empty();
            snap.candidate_links.push(GraphLink {
                id: format!("L{idx}"),
                source: source.clone(),
                target: target_endpoint.clone(),
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: state.clone(),
            });
            let back = round_trip(snap);
            assert_eq!(back.candidate_links.len(), 1, "{state:?}");
            assert_eq!(back.candidate_links[0].state, state);
        }
    }

    /// Round-trip both `LinkEndpoint` variants, including
    /// `Unresolved` with a fully-populated `evidence` payload.
    /// Exhaustive match guards.
    #[test]
    fn every_link_endpoint_round_trips() {
        use crate::model::{
            AgentSessionId, Confidence, Freshness, GraphLink, LinkEndpoint, LinkState, Metadata,
            MuxSessionId, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
        };

        let source = NodeId::AgentSession(AgentSessionId::new("codex", "default", "src"));
        let endpoints: Vec<LinkEndpoint> = vec![
            LinkEndpoint::Node {
                id: NodeId::MuxSession(MuxSessionId::new("tmux:0")),
            },
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".into(),
                    harness_key: Some("codex".into()),
                    native_id: Some("native".into()),
                    state_scope: Some("default".into()),
                    path: Some("/p".into()),
                    metadata: {
                        let mut m = Metadata::new();
                        m.insert("hint".into(), serde_json::json!(7));
                        m
                    },
                },
            },
        ];

        for endpoint in &endpoints {
            match endpoint {
                LinkEndpoint::Node { .. } | LinkEndpoint::Unresolved { .. } => {}
            }
        }

        for (idx, endpoint) in endpoints.into_iter().enumerate() {
            let mut snap = GraphSnapshot::empty();
            snap.candidate_links.push(GraphLink {
                id: format!("L{idx}"),
                source: source.clone(),
                target: endpoint.clone(),
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
            let back = round_trip(snap);
            assert_eq!(back.candidate_links.len(), 1);
            assert_eq!(back.candidate_links[0].target, endpoint);
        }
    }

    /// Round-trip every `SessionKind` variant. The reader's
    /// string-to-enum match (see `read_agent_sessions`) silently
    /// drops unknown strings to `None`, so the writer's exhaustive
    /// match catches *adding* a variant but not *renaming* or
    /// changing its serialized form. This per-variant test catches
    /// that asymmetry.
    #[test]
    fn every_session_kind_round_trips() {
        use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, SessionKind};

        let kinds: Vec<Option<SessionKind>> =
            vec![None, Some(SessionKind::Human), Some(SessionKind::Subagent)];

        // Exhaustive-match drift catch.
        for kind in &kinds {
            match kind {
                None | Some(SessionKind::Human) | Some(SessionKind::Subagent) => {}
            }
        }

        for kind in kinds {
            let mut snap = GraphSnapshot::empty();
            snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("codex", "default", "s"),
                harness_key: "codex".into(),
                cwd: None,
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: kind,
            }));
            let back = round_trip(snap);
            assert_eq!(back.nodes.len(), 1, "{kind:?}");
            match &back.nodes[0] {
                GraphNode::AgentSession(session) => {
                    assert_eq!(session.session_kind, kind, "round-trip {kind:?}")
                }
                other => panic!("unexpected node {other:?}"),
            }
        }
    }

    /// Round-trip every `RuntimeProcessRole` variant. Same
    /// reader-side silent-`None` asymmetry as `SessionKind`; the
    /// per-variant matrix catches it.
    #[test]
    fn every_runtime_process_role_round_trips() {
        use crate::model::{GraphNode, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole};

        let roles: Vec<Option<RuntimeProcessRole>> = vec![
            None,
            Some(RuntimeProcessRole::HumanAgent),
            Some(RuntimeProcessRole::Subagent),
            Some(RuntimeProcessRole::Background),
            Some(RuntimeProcessRole::Shell),
            Some(RuntimeProcessRole::Unknown),
        ];

        // Exhaustive-match drift catch.
        for role in &roles {
            match role {
                None
                | Some(RuntimeProcessRole::HumanAgent)
                | Some(RuntimeProcessRole::Subagent)
                | Some(RuntimeProcessRole::Background)
                | Some(RuntimeProcessRole::Shell)
                | Some(RuntimeProcessRole::Unknown) => {}
            }
        }

        for role in roles {
            let mut snap = GraphSnapshot::empty();
            snap.nodes
                .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                    id: RuntimeProcessId::new("tmux:0:1"),
                    observation_key: "tmux:0:1".into(),
                    pid: None,
                    parent_pid: None,
                    root_pane_pid: None,
                    command: None,
                    cwd: None,
                    harness_key: None,
                    role,
                    depth: None,
                    observed_epoch: None,
                }));
            let back = round_trip(snap);
            assert_eq!(back.nodes.len(), 1, "{role:?}");
            match &back.nodes[0] {
                GraphNode::RuntimeProcess(process) => {
                    assert_eq!(process.role, role, "round-trip {role:?}")
                }
                other => panic!("unexpected node {other:?}"),
            }
        }
    }

    /// `clear_all` regression: build a fully-populated snapshot,
    /// then `load(empty_snapshot)` and confirm every materialized
    /// table is empty. The yesterday bug where `pins` was missing
    /// from `clear_all`'s table list would have left ghost rows
    /// after an idempotent reload; this test catches the next such
    /// gap.
    #[test]
    fn clear_all_handles_every_materialized_table() {
        use crate::model::{
            AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
            ForgePrId, ForgePrNode, ForkId, ForkNode, GraphNode, MuxSessionId, MuxSessionNode,
            NodeId, PinBinding, PinCandidate, PinMuxRef, Provenance, RepoId, RepoNode,
            RuntimeProcessId, RuntimeProcessNode, WorkspaceId, WorkspaceNode,
        };

        let mut conn = fresh_conn();

        // Populate at least one row in every materialized table.
        let mut populated = GraphSnapshot::empty();
        populated.nodes.extend([
            GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))),
            GraphNode::Checkout(CheckoutNode::new(
                CheckoutId::new(RepoId::new("/r/.git"), "/r"),
                "/r",
            )),
            GraphNode::Workspace(WorkspaceNode {
                id: WorkspaceId::new("/w"),
                root: "/w".into(),
                provider: None,
                name: None,
            }),
            GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("codex", "default", "s"),
                harness_key: "codex".into(),
                cwd: None,
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            }),
            GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new("tmux:0"),
                native_id: "tmux:0".into(),
                backend: "tmux".into(),
                cwd: None,
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                client_attached: None,
                activity_epoch: None,
                created_epoch: None,
            }),
            GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: RuntimeProcessId::new("tmux:0:1"),
                observation_key: "tmux:0:1".into(),
                pid: None,
                parent_pid: None,
                root_pane_pid: None,
                command: None,
                cwd: None,
                harness_key: None,
                role: None,
                depth: None,
                observed_epoch: None,
            }),
            GraphNode::Branch(BranchNode {
                id: BranchId::new(RepoId::new("/r/.git"), "refs/heads/main"),
                refname: "refs/heads/main".into(),
                current_commit: None,
                upstream: None,
            }),
            GraphNode::Fork(ForkNode {
                id: ForkId::new("provider:src"),
                provider: "provider".into(),
                provider_source_key: "provider:src".into(),
                name: None,
                scope: None,
                capabilities: vec![],
            }),
            GraphNode::ForgePr(ForgePrNode {
                id: ForgePrId::new("github", "github.com", "o", "r", 1),
                provider: "github".into(),
                host: "github.com".into(),
                owner: "o".into(),
                repo: "r".into(),
                number: 1,
                state: None,
                url: None,
                updated_epoch: None,
                is_draft: false,
            }),
        ]);
        populated.diagnostics.push(Diagnostic::Config {
            path: "/etc".into(),
            message: "x".into(),
        });
        populated.pins.push(PinCandidate {
            id: "pin".into(),
            display_name: "Pin".into(),
            harness: "codex".into(),
            cwd: "/p".into(),
            mux: PinMuxRef {
                backend: "tmux".into(),
                name: "pin".into(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/p/.conspectus.toml".into(),
            binding: Some(PinBinding::Unbound),
        });
        populated.aliases.insert(
            NodeId::AgentSession(AgentSessionId::new("codex", "default", "s")),
            "Alias".into(),
        );
        load(&populated, &mut conn).expect("load populated");

        fn row_count(conn: &Connection, table: &str) -> i64 {
            conn.query_row::<i64, _, _>(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
        }

        // Sanity: rows landed.
        for table in [
            "node_repos",
            "node_checkouts",
            "node_workspaces",
            "node_agent_sessions",
            "node_mux_sessions",
            "node_runtime_processes",
            "node_branches",
            "node_forks",
            "node_forge_prs",
            "diagnostics",
            "pins",
            "aliases",
        ] {
            assert!(
                row_count(&conn, table) > 0,
                "{table} should have rows after load",
            );
        }

        // Reload with an empty snapshot — every table should be
        // empty afterwards. Catches `clear_all` missing a table
        // (the recent `pins` regression).
        load(&GraphSnapshot::empty(), &mut conn).expect("load empty");
        for table in [
            "node_repos",
            "node_checkouts",
            "node_workspaces",
            "node_agent_sessions",
            "node_mux_sessions",
            "node_runtime_processes",
            "node_branches",
            "node_forks",
            "node_forge_prs",
            "candidate_links",
            "resolved_relationships",
            "diagnostics",
            "pins",
            "aliases",
        ] {
            assert_eq!(
                row_count(&conn, table),
                0,
                "{table} should be empty after reload with empty snapshot",
            );
        }
    }
}
