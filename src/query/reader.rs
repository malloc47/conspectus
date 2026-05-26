//! SQLite → GraphSnapshot reader (spike, ADR 0043 candidate).
//!
//! Inverse of [`crate::query::loader::load`]. Reads every table the
//! loader writes into and reconstructs a [`GraphSnapshot`]. The point
//! of this module is to validate the loader is *lossless* end-to-end:
//! `snapshot → load(conn) → read_snapshot(conn)` round-trips to an
//! equal snapshot after canonicalization.
//!
//! Strategy: typed `*Node` structs are rebuilt from the per-kind table
//! columns (the structural source of truth), not by parsing the
//! `node_id` text. The `node_id` text *is* parsed for foreign
//! references inside `candidate_links`, `resolved_relationships`,
//! `diagnostics`, and `aliases` where only the [`fmt::Display`] form
//! is stored. See `parse_node_id`.

use std::collections::BTreeMap;

use rusqlite::{Connection, Row};
use serde_json::Value;

use crate::aliases::AliasOverlay;
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence,
    Diagnostic, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId,
    Provenance, RelationKind, RepoId, RepoNode, ResolvedRelationship, SourceMetadata,
    UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
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
    read_branches(conn, &mut snap.nodes)?;
    read_forks(conn, &mut snap.nodes)?;
    read_forge_prs(conn, &mut snap.nodes)?;
    snap.candidate_links = read_candidate_links(conn)?;
    snap.resolved_relationships = read_resolved(conn)?;
    snap.diagnostics = read_diagnostics(conn)?;
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
                last_message_preview, last_active_epoch \
         FROM node_agent_sessions ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let harness_key: String = row.get(0)?;
        let state_scope: String = row.get(1)?;
        let session_key: String = row.get(2)?;
        Ok(AgentSessionNode {
            id: AgentSessionId::new(&harness_key, state_scope, session_key),
            harness_key,
            cwd: row.get(3)?,
            title: row.get(4)?,
            last_message_preview: row.get(5)?,
            last_active_epoch: row.get(6)?,
        })
    })?;
    for session in rows {
        out.push(GraphNode::AgentSession(session?));
    }
    Ok(())
}

fn read_mux_sessions(conn: &Connection, out: &mut Vec<GraphNode>) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT native_id, backend, cwd, active_pane_command, active_pane_pid, \
                active_pane_current_path, active_pane_start_command, \
                activity_epoch, created_epoch \
         FROM node_mux_sessions ORDER BY node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let native_id: String = row.get(0)?;
        Ok(MuxSessionNode {
            id: MuxSessionId::new(&native_id),
            native_id,
            backend: row.get(1)?,
            cwd: row.get(2)?,
            active_pane_command: row.get(3)?,
            active_pane_pid: row.get(4)?,
            active_pane_current_path: row.get(5)?,
            active_pane_start_command: row.get(6)?,
            activity_epoch: row.get(7)?,
            created_epoch: row.get(8)?,
        })
    })?;
    for mux in rows {
        out.push(GraphNode::MuxSession(mux?));
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
        "SELECT link_id, source_node_id, target_kind, target_node_id, \
                target_node_type, target_harness_key, target_native_id, \
                target_state_scope, target_path, target_metadata, \
                relation, provenance, confidence, freshness, \
                state, state_reason, state_overridden_by, \
                source_adapter, source_evidence, source_fields \
         FROM candidate_links ORDER BY link_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let link_id: String = row.get(0)?;
        let source_node_id: String = row.get(1)?;
        let target_kind: String = row.get(2)?;
        let target_node_id: Option<String> = row.get(3)?;
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
                id: parse_node_id_row(target_node_id.as_deref(), row)?,
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
            source: parse_node_id_str(&source_node_id, row)?,
            target,
            relation: deserialize_tag::<RelationKind>(&relation, row, 10)?,
            provenance: deserialize_tag::<Provenance>(&provenance, row, 11)?,
            confidence: deserialize_tag::<Confidence>(&confidence, row, 12)?,
            freshness: deserialize_tag::<Freshness>(&freshness, row, 13)?,
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
        "SELECT source_node_id, target_node_id, relation, selected_link_id, competing_link_ids \
         FROM resolved_relationships ORDER BY source_node_id, relation, target_node_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let source: String = row.get(0)?;
        let target: String = row.get(1)?;
        let relation: String = row.get(2)?;
        let selected: String = row.get(3)?;
        let competing: String = row.get(4)?;
        Ok(ResolvedRelationship {
            source: parse_node_id_str(&source, row)?,
            target: parse_node_id_str(&target, row)?,
            relation: deserialize_tag::<RelationKind>(&relation, row, 2)?,
            selected_link_id: selected,
            competing_link_ids: parse_json_str_array(&competing),
        })
    })?;
    rows.collect()
}

fn read_diagnostics(conn: &Connection) -> rusqlite::Result<Vec<Diagnostic>> {
    let mut stmt = conn.prepare(
        "SELECT kind, link_id, relation, config_path, config_message, \
                conflict_source_node_id, conflict_selected_link_id, conflict_competing_link_ids \
         FROM diagnostics ORDER BY kind, link_id, conflict_selected_link_id",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind: String = row.get(0)?;
        match kind.as_str() {
            "unresolved_endpoint" => Ok(Diagnostic::UnresolvedEndpoint {
                link_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                relation: deserialize_tag::<RelationKind>(
                    &row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row,
                    2,
                )?,
            }),
            "config" => Ok(Diagnostic::Config {
                path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                message: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            }),
            "conflict" => Ok(Diagnostic::Conflict {
                source: parse_node_id_str(
                    &row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    row,
                )?,
                relation: deserialize_tag::<RelationKind>(
                    &row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row,
                    2,
                )?,
                selected_link_id: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                competing_link_ids: parse_json_str_array(
                    &row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                ),
            }),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(BadEnum(format!("diagnostic kind={other}"))),
            )),
        }
    })?;
    rows.collect()
}

fn read_aliases(conn: &Connection) -> rusqlite::Result<AliasOverlay> {
    let mut stmt = conn.prepare("SELECT node_id, display_name FROM aliases ORDER BY node_id")?;
    let rows = stmt.query_map([], |row| {
        let node_id: String = row.get(0)?;
        let display_name: String = row.get(1)?;
        Ok((parse_node_id_str(&node_id, row)?, display_name))
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

fn deserialize_tag<T>(s: &str, _row: &Row<'_>, idx: usize) -> rusqlite::Result<T>
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

fn parse_node_id_row(text: Option<&str>, row: &Row<'_>) -> rusqlite::Result<NodeId> {
    let s = text.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(BadEnum(
                "target_node_id required for target_kind=node".into(),
            )),
        )
    })?;
    parse_node_id_str(s, row)
}

/// Spike-local NodeId parser. Splits the [`fmt::Display`] form back
/// into typed pieces. Not robust to common_dir / refname / path values
/// containing `:` / `@` / `#` / `/` — the existing fixtures don't hit
/// those, and the ADR write-up notes the limitation.
fn parse_node_id_str(s: &str, _row: &Row<'_>) -> rusqlite::Result<NodeId> {
    parse_node_id(s).map_err(|reason| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(BadEnum(format!("node_id={s}: {reason}"))),
        )
    })
}

fn parse_node_id(s: &str) -> Result<NodeId, String> {
    let (kind, rest) = s
        .split_once(':')
        .ok_or_else(|| format!("missing kind prefix in {s}"))?;
    match kind {
        "repo" => Ok(NodeId::Repo(RepoId::new(rest))),
        "workspace" => Ok(NodeId::Workspace(WorkspaceId::new(rest))),
        "mux_session" => Ok(NodeId::MuxSession(MuxSessionId::new(rest))),
        "fork" => Ok(NodeId::Fork(ForkId::new(rest))),
        "agent_session" => {
            let mut parts = rest.splitn(3, ':');
            let harness = parts.next().ok_or("missing harness_key")?;
            let scope = parts.next().ok_or("missing state_scope")?;
            let session = parts.next().ok_or("missing session_key")?;
            Ok(NodeId::AgentSession(AgentSessionId::new(
                harness, scope, session,
            )))
        }
        "checkout" => {
            // rest = "repo:<common_dir>@<root>"
            let rest = rest
                .strip_prefix("repo:")
                .ok_or("checkout body missing repo: prefix")?;
            let (common_dir, root) = rest
                .rsplit_once('@')
                .ok_or("checkout body missing @ separator")?;
            Ok(NodeId::Checkout(CheckoutId::new(
                RepoId::new(common_dir),
                root,
            )))
        }
        "branch" => {
            // rest = "repo:<common_dir>@<refname>"
            let rest = rest
                .strip_prefix("repo:")
                .ok_or("branch body missing repo: prefix")?;
            let (common_dir, refname) = rest
                .rsplit_once('@')
                .ok_or("branch body missing @ separator")?;
            Ok(NodeId::Branch(BranchId::new(
                RepoId::new(common_dir),
                refname,
            )))
        }
        "forge_pr" => {
            // rest = "<provider>:<host>/<owner>/<repo>#<number>"
            let (provider, rest2) = rest.split_once(':').ok_or("forge_pr missing provider")?;
            let (path, number_s) = rest2.rsplit_once('#').ok_or("forge_pr missing #")?;
            let mut parts = path.splitn(3, '/');
            let host = parts.next().ok_or("forge_pr missing host")?;
            let owner = parts.next().ok_or("forge_pr missing owner")?;
            let repo = parts.next().ok_or("forge_pr missing repo")?;
            let number: u64 = number_s
                .parse()
                .map_err(|e| format!("forge_pr number parse: {e}"))?;
            Ok(NodeId::ForgePr(ForgePrId::new(
                provider, host, owner, repo, number,
            )))
        }
        other => Err(format!("unknown node kind {other}")),
    }
}

#[derive(Debug)]
struct BadEnum(String);

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

    #[test]
    fn parses_known_node_id_forms() {
        let cases = [
            "repo:/r/.git",
            "workspace:/w",
            "mux_session:tmux:0",
            "fork:provider:src",
            "agent_session:claude-code:default:abc",
            "checkout:repo:/r/.git@/r",
            "branch:repo:/r/.git@refs/heads/main",
            "forge_pr:github:github.com/owner/repo#42",
        ];
        for case in cases {
            let id = parse_node_id(case).unwrap_or_else(|e| panic!("parse {case}: {e}"));
            assert_eq!(id.to_string(), case, "round-trip {case}");
        }
    }

    #[test]
    fn full_snapshot_round_trips() {
        // Reuse the fixture builder pattern from the loader tests.
        use crate::model::{
            AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
            Confidence, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
            LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, Provenance,
            RelationKind, RepoId, RepoNode, ResolvedRelationship, SourceMetadata,
            UnresolvedEndpoint, WorkspaceId, WorkspaceNode,
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
            activity_epoch: Some(1_700_000_001),
            created_epoch: Some(1_699_000_000),
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
