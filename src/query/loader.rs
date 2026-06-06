//! GraphSnapshot → SQLite loader (P9-003).
//!
//! Populates every table in the P9-002 schema from a snapshot, in one
//! transaction. Node iteration is direct over the producer-side
//! [`GraphSnapshot`]; consumers read the resulting SQLite tables.
//!
//! Idempotency: every call clears the affected tables before inserting,
//! so re-running the loader on the same snapshot reproduces the same
//! database state.
//!
//! Polymorphic shapes (`LinkEndpoint`, `LinkState`, `Diagnostic`,
//! `SourceMetadata.fields`, `Vec<String>` columns) are flattened into
//! discriminator + per-variant columns and JSON-text blobs per the
//! schema documentation.

use rusqlite::{Connection, Transaction, params};

use crate::aliases::AliasOverlay;
use crate::model::{
    AgentSessionNode, BranchNode, CheckoutNode, Diagnostic, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, RepoNode,
    ResolvedRelationship, RuntimeProcessNode, WorkspaceNode,
};

use super::schema::{
    confidence_tag, diagnostic_kind_tag, freshness_tag, link_state_tag, provenance_tag,
    relation_kind_tag,
};

/// Load `snapshot` into `conn`, replacing all existing rows in a single
/// transaction. Safe to call against a fresh database that has had
/// [`super::apply_schema`] applied, or against one previously populated
/// by an earlier call to this function.
///
/// Partial-eviction (only one provider's rows) is not implemented here.
/// It is the natural extension once P7-002 lands provider-provenance
/// fields on the in-memory model; the schema is ready for it
/// (the `discovery_provider` columns) but the data is not.
pub fn load(snapshot: &GraphSnapshot, conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    clear_all(&tx)?;
    insert_repos(&tx, &snapshot.nodes)?;
    insert_checkouts(&tx, &snapshot.nodes)?;
    insert_workspaces(&tx, &snapshot.nodes)?;
    insert_agent_sessions(&tx, &snapshot.nodes)?;
    insert_mux_sessions(&tx, &snapshot.nodes)?;
    insert_runtime_processes(&tx, &snapshot.nodes)?;
    insert_branches(&tx, &snapshot.nodes)?;
    insert_forks(&tx, &snapshot.nodes)?;
    insert_forge_prs(&tx, &snapshot.nodes)?;
    insert_candidate_links(&tx, &snapshot.candidate_links)?;
    insert_resolved(&tx, &snapshot.resolved_relationships)?;
    insert_diagnostics(&tx, &snapshot.diagnostics)?;
    insert_aliases(&tx, &snapshot.aliases)?;
    tx.commit()
}

fn clear_all(tx: &Transaction) -> rusqlite::Result<()> {
    for table in [
        "aliases",
        "diagnostics",
        "resolved_relationships",
        "candidate_links",
        "node_forge_prs",
        "node_forks",
        "node_branches",
        "node_runtime_processes",
        "node_mux_sessions",
        "node_agent_sessions",
        "node_workspaces",
        "node_checkouts",
        "node_repos",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    Ok(())
}

fn insert_repos(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_repos (node_id, common_dir, source_paths, remotes) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for node in nodes {
        let GraphNode::Repo(repo) = node else {
            continue;
        };
        let node_id = node.id();
        let RepoNode {
            id: _,
            common_dir,
            source_paths,
            remotes,
        } = repo;
        stmt.execute(params![
            node_id.to_string(),
            common_dir,
            json_array(source_paths),
            json_array(remotes),
        ])?;
    }
    Ok(())
}

fn insert_checkouts(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_checkouts (\
           node_id, repo_common_dir, root, git_dir, \
           current_branch_repo_common_dir, current_branch_refname\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for node in nodes {
        let GraphNode::Checkout(checkout) = node else {
            continue;
        };
        let node_id = node.id();
        let CheckoutNode {
            id,
            root,
            git_dir,
            current_branch,
        } = checkout;
        let (cb_repo_common_dir, cb_refname) = current_branch
            .as_ref()
            .map(|b| (Some(b.repo.common_dir.as_str()), Some(b.refname.as_str())))
            .unwrap_or((None, None));
        stmt.execute(params![
            node_id.to_string(),
            id.repo.common_dir,
            root,
            git_dir,
            cb_repo_common_dir,
            cb_refname,
        ])?;
    }
    Ok(())
}

fn insert_workspaces(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_workspaces (node_id, root, provider_name, name) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for node in nodes {
        let GraphNode::Workspace(workspace) = node else {
            continue;
        };
        let node_id = node.id();
        let WorkspaceNode {
            id: _,
            root,
            provider,
            name,
        } = workspace;
        stmt.execute(params![node_id.to_string(), root, provider, name])?;
    }
    Ok(())
}

fn insert_agent_sessions(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_agent_sessions (\
           node_id, harness_key, state_scope, session_key, cwd, title, \
           last_message_preview, last_active_epoch, session_kind\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for node in nodes {
        let GraphNode::AgentSession(session) = node else {
            continue;
        };
        let node_id = node.id();
        let AgentSessionNode {
            id,
            harness_key,
            cwd,
            title,
            last_message_preview,
            last_active_epoch,
            session_kind,
        } = session;
        let session_kind_str = session_kind.map(|k| match k {
            crate::model::SessionKind::Human => "human".to_string(),
            crate::model::SessionKind::Subagent => "subagent".to_string(),
        });
        stmt.execute(params![
            node_id.to_string(),
            harness_key,
            id.state_scope,
            id.session_key,
            cwd,
            title,
            last_message_preview,
            last_active_epoch,
            session_kind_str,
        ])?;
    }
    Ok(())
}

fn insert_mux_sessions(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_mux_sessions (\
           node_id, native_id, backend, cwd, \
           active_pane_command, active_pane_pid, active_pane_current_path, \
           active_pane_start_command, client_attached, activity_epoch, created_epoch\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    for node in nodes {
        let GraphNode::MuxSession(mux) = node else {
            continue;
        };
        let node_id = node.id();
        let MuxSessionNode {
            id: _,
            native_id,
            backend,
            cwd,
            active_pane_command,
            active_pane_pid,
            active_pane_current_path,
            active_pane_start_command,
            client_attached,
            activity_epoch,
            created_epoch,
        } = mux;
        let client_attached = client_attached.map(i64::from);
        stmt.execute(params![
            node_id.to_string(),
            native_id,
            backend,
            cwd,
            active_pane_command,
            active_pane_pid,
            active_pane_current_path,
            active_pane_start_command,
            client_attached,
            activity_epoch,
            created_epoch,
        ])?;
    }
    Ok(())
}

fn insert_runtime_processes(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_runtime_processes (\
           node_id, observation_key, pid, parent_pid, root_pane_pid, command, cwd, \
           harness_key, role, depth, observed_epoch\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    for node in nodes {
        let GraphNode::RuntimeProcess(process) = node else {
            continue;
        };
        let node_id = node.id();
        let RuntimeProcessNode {
            id: _,
            observation_key,
            pid,
            parent_pid,
            root_pane_pid,
            command,
            cwd,
            harness_key,
            role,
            depth,
            observed_epoch,
        } = process;
        let role_str = role.map(|role| match role {
            crate::model::RuntimeProcessRole::HumanAgent => "human_agent".to_string(),
            crate::model::RuntimeProcessRole::Subagent => "subagent".to_string(),
            crate::model::RuntimeProcessRole::Background => "background".to_string(),
            crate::model::RuntimeProcessRole::Shell => "shell".to_string(),
            crate::model::RuntimeProcessRole::Unknown => "unknown".to_string(),
        });
        stmt.execute(params![
            node_id.to_string(),
            observation_key,
            pid,
            parent_pid,
            root_pane_pid,
            command,
            cwd,
            harness_key,
            role_str,
            depth,
            observed_epoch,
        ])?;
    }
    Ok(())
}

fn insert_branches(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_branches (\
           node_id, repo_common_dir, refname, current_commit, upstream\
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for node in nodes {
        let GraphNode::Branch(branch) = node else {
            continue;
        };
        let node_id = node.id();
        let BranchNode {
            id,
            refname,
            current_commit,
            upstream,
        } = branch;
        stmt.execute(params![
            node_id.to_string(),
            id.repo.common_dir,
            refname,
            current_commit,
            upstream,
        ])?;
    }
    Ok(())
}

fn insert_forks(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_forks (\
           node_id, provider_source_key, provider_name, name, scope, capabilities\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for node in nodes {
        let GraphNode::Fork(fork) = node else {
            continue;
        };
        let node_id = node.id();
        let ForkNode {
            id: _,
            provider,
            provider_source_key,
            name,
            scope,
            capabilities,
        } = fork;
        stmt.execute(params![
            node_id.to_string(),
            provider_source_key,
            provider,
            name,
            scope,
            json_array(capabilities),
        ])?;
    }
    Ok(())
}

fn insert_forge_prs(tx: &Transaction, nodes: &[GraphNode]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO node_forge_prs (\
           node_id, provider_name, host, owner, repo, number, \
           state, url, updated_epoch, is_draft\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;
    for node in nodes {
        let GraphNode::ForgePr(pr) = node else {
            continue;
        };
        let node_id = node.id();
        let ForgePrNode {
            id: _,
            provider,
            host,
            owner,
            repo,
            number,
            state,
            url,
            updated_epoch,
            is_draft,
        } = pr;
        stmt.execute(params![
            node_id.to_string(),
            provider,
            host,
            owner,
            repo,
            i64::try_from(*number).expect("forge PR number fits in i64"),
            state,
            url,
            updated_epoch,
            i64::from(*is_draft),
        ])?;
    }
    Ok(())
}

fn insert_candidate_links(tx: &Transaction, links: &[GraphLink]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO candidate_links (\
           link_id, source, \
           target_kind, target_node, target_node_type, target_harness_key, \
           target_native_id, target_state_scope, target_path, target_metadata, \
           relation, provenance, confidence, freshness, \
           state, state_reason, state_overridden_by, \
           source_adapter, source_evidence, source_fields\
         ) VALUES (\
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
           ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20\
         )",
    )?;
    for link in links {
        let (target_kind, target_node_json, evidence) = match &link.target {
            LinkEndpoint::Node { id } => ("node", Some(json_node_id(id)), None),
            LinkEndpoint::Unresolved { evidence } => ("unresolved", None, Some(evidence)),
        };
        let (state_reason, state_overridden_by) = match &link.state {
            LinkState::Active => (None, None),
            LinkState::Ignored { reason } => (reason.as_deref(), None),
            LinkState::Overridden { by, reason } => (reason.as_deref(), Some(by.as_str())),
        };
        let target_node_type = evidence.map(|e| e.node_type.as_str());
        let target_harness_key = evidence.and_then(|e| e.harness_key.as_deref());
        let target_native_id = evidence.and_then(|e| e.native_id.as_deref());
        let target_state_scope = evidence.and_then(|e| e.state_scope.as_deref());
        let target_path = evidence.and_then(|e| e.path.as_deref());
        let target_metadata = evidence
            .map(|e| json_object(&e.metadata))
            .unwrap_or_else(|| "{}".into());
        stmt.execute(params![
            link.id,
            json_node_id(&link.source),
            target_kind,
            target_node_json,
            target_node_type,
            target_harness_key,
            target_native_id,
            target_state_scope,
            target_path,
            target_metadata,
            relation_kind_tag(&link.relation),
            provenance_tag(link.provenance),
            confidence_tag(link.confidence),
            freshness_tag(link.freshness),
            link_state_tag(&link.state),
            state_reason,
            state_overridden_by,
            link.source_metadata.adapter,
            link.source_metadata.evidence,
            json_object(&link.source_metadata.fields),
        ])?;
    }
    Ok(())
}

fn insert_resolved(tx: &Transaction, items: &[ResolvedRelationship]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO resolved_relationships (\
           source, target, relation, selected_link_id, competing_link_ids\
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for r in items {
        stmt.execute(params![
            json_node_id(&r.source),
            json_node_id(&r.target),
            relation_kind_tag(&r.relation),
            r.selected_link_id,
            json_array(&r.competing_link_ids),
        ])?;
    }
    Ok(())
}

fn insert_diagnostics(tx: &Transaction, items: &[Diagnostic]) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO diagnostics (\
           kind, link_id, relation, config_path, config_message, \
           conflict_source, conflict_selected_link_id, conflict_competing_link_ids\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for d in items {
        let kind = diagnostic_kind_tag(d);
        match d {
            Diagnostic::UnresolvedEndpoint { link_id, relation } => {
                stmt.execute(params![
                    kind,
                    link_id,
                    relation_kind_tag(relation),
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
            Diagnostic::Config { path, message } => {
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    None::<&str>,
                    path,
                    message,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
            Diagnostic::Conflict {
                source,
                relation,
                selected_link_id,
                competing_link_ids,
            } => {
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    relation_kind_tag(relation),
                    None::<&str>,
                    None::<&str>,
                    json_node_id(source),
                    selected_link_id,
                    json_array(competing_link_ids),
                ])?;
            }
            // Pin diagnostics (ADR 0057) reuse the `config_message`
            // column for now; a future query-layer story can grow
            // dedicated columns or a structured JSON `details` field.
            Diagnostic::PinUnbound {
                pin_id,
                expected_mux_native_id,
                last_session: _,
            } => {
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    format!(
                        "pin '{pin_id}' has no live mux with native_id '{expected_mux_native_id}'"
                    ),
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
            Diagnostic::PinStaleMux { pin_id, mux } => {
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    format!(
                        "pin '{pin_id}' mux {} is live but no matching harness session is attributed",
                        mux.native_id
                    ),
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
            Diagnostic::PinAmbiguous {
                pin_id,
                chosen,
                competing,
            } => {
                let competing_keys: Vec<String> =
                    competing.iter().map(|id| id.session_key.clone()).collect();
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    format!(
                        "pin '{pin_id}' bound to '{}' with {} competing candidate(s): [{}]",
                        chosen.session_key,
                        competing.len(),
                        competing_keys.join(", "),
                    ),
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
            Diagnostic::PinDrift {
                pin_id,
                declared_cwd,
                observed_cwd,
            } => {
                stmt.execute(params![
                    kind,
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    format!(
                        "pin '{pin_id}' cwd drift: declared '{declared_cwd}', observed '{observed_cwd}'"
                    ),
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                ])?;
            }
        }
    }
    Ok(())
}

fn insert_aliases(tx: &Transaction, aliases: &AliasOverlay) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare("INSERT INTO aliases (node, display_name) VALUES (?1, ?2)")?;
    for (node_id, display_name) in aliases.iter() {
        stmt.execute(params![json_node_id(node_id), display_name])?;
    }
    Ok(())
}

fn json_array(values: &[String]) -> String {
    serde_json::to_string(values).expect("Vec<String> serializes")
}

fn json_object(metadata: &crate::model::Metadata) -> String {
    serde_json::to_string(metadata).expect("Metadata serializes")
}

/// Serialize a [`NodeId`] to its canonical JSON form for storage in
/// the endpoint columns (ADR 0044). Inverse: `serde_json::from_str`
/// in [`crate::query::reader`].
fn json_node_id(node_id: &crate::model::NodeId) -> String {
    serde_json::to_string(node_id).expect("NodeId serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
        Confidence, ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode,
        GraphSnapshot, LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, RepoId, RepoNode, ResolvedRelationship, RuntimeProcessId,
        RuntimeProcessNode, RuntimeProcessRole, SourceMetadata, UnresolvedEndpoint, WorkspaceId,
        WorkspaceNode,
    };
    use crate::query::schema::apply_schema;

    fn fresh_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory open");
        apply_schema(&conn).expect("apply schema");
        // Hand back a connection ready for the loader to take a tx on.
        let _ = &mut conn;
        conn
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn make_repo(common_dir: &str) -> RepoNode {
        let id = RepoId::new(common_dir);
        let mut node = RepoNode::new(id);
        node.source_paths = vec!["/r".into(), "/r-alt".into()];
        node.remotes = vec!["origin".into()];
        node
    }

    fn make_checkout(repo_common: &str, root: &str) -> CheckoutNode {
        CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common), root),
            root: root.into(),
            git_dir: Some(repo_common.to_string()),
            current_branch: Some(BranchId::new(RepoId::new(repo_common), "refs/heads/main")),
        }
    }

    fn make_workspace(root: &str) -> WorkspaceNode {
        WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.into(),
            provider: Some("atelier".into()),
            name: Some("ws-name".into()),
        }
    }

    fn make_agent(session_key: &str, harness: &str) -> AgentSessionNode {
        AgentSessionNode {
            id: AgentSessionId::new(harness, "default", session_key),
            harness_key: harness.into(),
            cwd: Some("/cwd".into()),
            title: Some("title".into()),
            last_message_preview: Some("hello".into()),
            last_active_epoch: Some(1_700_000_000),
            session_kind: None,
        }
    }

    fn make_mux(native_id: &str) -> MuxSessionNode {
        MuxSessionNode {
            id: MuxSessionId::new(native_id),
            native_id: native_id.into(),
            backend: "tmux".into(),
            cwd: Some("/cwd".into()),
            active_pane_command: Some("zsh".into()),
            active_pane_pid: Some(12345),
            active_pane_current_path: Some("/cwd/sub".into()),
            active_pane_start_command: Some("zsh -l".into()),
            client_attached: None,
            activity_epoch: Some(1_700_000_001),
            created_epoch: Some(1_699_000_000),
        }
    }

    fn make_runtime_process(observation_key: &str) -> RuntimeProcessNode {
        RuntimeProcessNode {
            id: RuntimeProcessId::new(observation_key),
            observation_key: observation_key.into(),
            pid: Some(12345),
            parent_pid: Some(123),
            root_pane_pid: Some(123),
            command: Some("codex".into()),
            cwd: Some("/cwd".into()),
            harness_key: Some("codex".into()),
            role: Some(RuntimeProcessRole::HumanAgent),
            depth: Some(1),
            observed_epoch: Some(1_700_000_003),
        }
    }

    fn make_branch(repo_common: &str, refname: &str) -> BranchNode {
        BranchNode {
            id: BranchId::new(RepoId::new(repo_common), refname),
            refname: refname.into(),
            current_commit: Some("deadbeef".into()),
            upstream: Some("origin/main".into()),
        }
    }

    fn make_fork(key: &str) -> ForkNode {
        ForkNode {
            id: ForkId::new(key),
            provider: "atelier".into(),
            provider_source_key: key.into(),
            name: Some("fork-name".into()),
            scope: Some("scope".into()),
            capabilities: vec!["a".into(), "b".into()],
        }
    }

    fn make_forge_pr() -> ForgePrNode {
        ForgePrNode {
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
        }
    }

    fn full_snapshot() -> GraphSnapshot {
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::Repo(make_repo("/r/.git")));
        snap.nodes
            .push(GraphNode::Checkout(make_checkout("/r/.git", "/r")));
        snap.nodes.push(GraphNode::Workspace(make_workspace("/w")));
        snap.nodes
            .push(GraphNode::AgentSession(make_agent("s1", "claude-code")));
        snap.nodes.push(GraphNode::MuxSession(make_mux("tmux:0")));
        snap.nodes
            .push(GraphNode::RuntimeProcess(make_runtime_process(
                "tmux:0:12345",
            )));
        snap.nodes
            .push(GraphNode::Branch(make_branch("/r/.git", "refs/heads/main")));
        snap.nodes.push(GraphNode::Fork(make_fork("provider:src")));
        snap.nodes.push(GraphNode::ForgePr(make_forge_pr()));
        snap
    }

    #[test]
    fn empty_snapshot_loads_cleanly() {
        let mut conn = fresh_conn();
        load(&GraphSnapshot::empty(), &mut conn).expect("load empty");
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM v_nodes"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM candidate_links"), 0);
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM resolved_relationships"),
            0
        );
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM diagnostics"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM aliases"), 0);
    }

    #[test]
    fn every_node_kind_round_trips() {
        let mut conn = fresh_conn();
        load(&full_snapshot(), &mut conn).expect("load full snapshot");
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
        ] {
            let n = count(&conn, &format!("SELECT COUNT(*) FROM {table}"));
            assert_eq!(n, 1, "expected one row in {table}, got {n}");
        }
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM v_nodes"), 9);
    }

    #[test]
    fn repo_fields_round_trip() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::Repo(make_repo("/r/.git")));
        load(&snap, &mut conn).unwrap();
        let (common_dir, source_paths, remotes): (String, String, String) = conn
            .query_row(
                "SELECT common_dir, source_paths, remotes FROM node_repos",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(common_dir, "/r/.git");
        assert_eq!(source_paths, "[\"/r\",\"/r-alt\"]");
        assert_eq!(remotes, "[\"origin\"]");
    }

    #[test]
    fn checkout_with_current_branch_round_trips() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        snap.nodes
            .push(GraphNode::Checkout(make_checkout("/r/.git", "/r")));
        load(&snap, &mut conn).unwrap();
        let (repo, root, refname): (String, String, String) = conn
            .query_row(
                "SELECT repo_common_dir, root, current_branch_refname FROM node_checkouts",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(repo, "/r/.git");
        assert_eq!(root, "/r");
        assert_eq!(refname, "refs/heads/main");
    }

    #[test]
    fn forge_pr_number_and_draft_round_trip() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        snap.nodes.push(GraphNode::ForgePr(make_forge_pr()));
        load(&snap, &mut conn).unwrap();
        let (number, is_draft, state): (i64, i64, String) = conn
            .query_row(
                "SELECT number, is_draft, state FROM node_forge_prs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(number, 42);
        assert_eq!(is_draft, 1);
        assert_eq!(state, "open");
    }

    fn make_node_link(source: NodeId, target: NodeId) -> GraphLink {
        GraphLink {
            id: "link-1".into(),
            source,
            target: LinkEndpoint::Node { id: target },
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
        }
    }

    fn make_unresolved_link(source: NodeId) -> GraphLink {
        GraphLink {
            id: "link-unresolved".into(),
            source,
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
        }
    }

    #[test]
    fn candidate_link_with_node_endpoint_round_trips() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let source = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        let target = NodeId::MuxSession(MuxSessionId::new("tmux:0"));
        snap.candidate_links
            .push(make_node_link(source.clone(), target.clone()));
        load(&snap, &mut conn).unwrap();
        let (
            target_kind,
            target_node_json,
            target_node_kind,
            relation,
            provenance,
            confidence,
            freshness,
            state,
        ): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT target_kind, target_node, target_node_kind, relation, provenance, \
                 confidence, freshness, state FROM candidate_links",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(target_kind, "node");
        assert_eq!(target_node_kind, "mux_session");
        let decoded: crate::model::NodeId = serde_json::from_str(&target_node_json).unwrap();
        assert_eq!(decoded, target);
        assert_eq!(relation, "linked_to_mux");
        assert_eq!(provenance, "strong_discovered");
        assert_eq!(confidence, "high");
        assert_eq!(freshness, "fresh");
        assert_eq!(state, "active");
    }

    #[test]
    fn candidate_link_with_unresolved_endpoint_round_trips() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let source = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        snap.candidate_links.push(make_unresolved_link(source));
        load(&snap, &mut conn).unwrap();
        let (target_kind, target_node_type, target_harness_key, target_metadata): (
            String,
            String,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT target_kind, target_node_type, target_harness_key, target_metadata \
                 FROM candidate_links",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(target_kind, "unresolved");
        assert_eq!(target_node_type, "agent_session");
        assert_eq!(target_harness_key, "claude-code");
        assert!(target_metadata.contains("hint"));
    }

    #[test]
    fn link_state_variants_round_trip() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let source = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        let target = NodeId::MuxSession(MuxSessionId::new("tmux:0"));

        let mut ignored = make_node_link(source.clone(), target.clone());
        ignored.id = "ignored".into();
        ignored.state = LinkState::Ignored {
            reason: Some("user-ignored".into()),
        };
        snap.candidate_links.push(ignored);

        let mut overridden = make_node_link(source.clone(), target.clone());
        overridden.id = "overridden".into();
        overridden.state = LinkState::Overridden {
            by: "preferred-link".into(),
            reason: Some("better-source".into()),
        };
        snap.candidate_links.push(overridden);

        load(&snap, &mut conn).unwrap();
        let rows: Vec<(String, String, Option<String>, Option<String>)> = conn
            .prepare(
                "SELECT link_id, state, state_reason, state_overridden_by FROM candidate_links \
                 ORDER BY link_id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "ignored");
        assert_eq!(rows[0].1, "ignored");
        assert_eq!(rows[0].2.as_deref(), Some("user-ignored"));
        assert_eq!(rows[0].3, None);
        assert_eq!(rows[1].0, "overridden");
        assert_eq!(rows[1].1, "overridden");
        assert_eq!(rows[1].2.as_deref(), Some("better-source"));
        assert_eq!(rows[1].3.as_deref(), Some("preferred-link"));
    }

    #[test]
    fn resolved_relationship_round_trips() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let source = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        let target = NodeId::MuxSession(MuxSessionId::new("tmux:0"));
        snap.resolved_relationships.push(ResolvedRelationship {
            source: source.clone(),
            target: target.clone(),
            relation: RelationKind::LinkedToMux,
            selected_link_id: "winner".into(),
            competing_link_ids: vec!["a".into(), "b".into()],
        });
        load(&snap, &mut conn).unwrap();
        let (s_json, s_kind, t_json, t_kind, r, sel, comp): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT source, source_kind, target, target_kind, relation, selected_link_id, \
                 competing_link_ids FROM resolved_relationships",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .unwrap();
        let s_decoded: crate::model::NodeId = serde_json::from_str(&s_json).unwrap();
        let t_decoded: crate::model::NodeId = serde_json::from_str(&t_json).unwrap();
        assert_eq!(s_decoded, source);
        assert_eq!(t_decoded, target);
        assert_eq!(s_kind, "agent_session");
        assert_eq!(t_kind, "mux_session");
        assert_eq!(r, "linked_to_mux");
        assert_eq!(sel, "winner");
        assert_eq!(comp, "[\"a\",\"b\"]");
    }

    #[test]
    fn diagnostic_variants_round_trip() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let source = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        snap.diagnostics.push(Diagnostic::UnresolvedEndpoint {
            link_id: "L1".into(),
            relation: RelationKind::LinkedToMux,
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
        load(&snap, &mut conn).unwrap();
        type DiagnosticRow = (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let rows: Vec<DiagnosticRow> = conn
            .prepare(
                "SELECT kind, link_id, relation, config_path, config_message, \
                 conflict_source, conflict_selected_link_id, conflict_competing_link_ids \
                 FROM diagnostics ORDER BY kind",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 3);
        let kinds: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(kinds, vec!["config", "conflict", "unresolved_endpoint"]);

        // config
        assert_eq!(rows[0].3.as_deref(), Some("/etc/x"));
        assert_eq!(rows[0].4.as_deref(), Some("bad"));

        // conflict
        assert_eq!(rows[1].2.as_deref(), Some("associated_with"));
        let conflict_source_decoded: crate::model::NodeId =
            serde_json::from_str(rows[1].5.as_deref().unwrap()).unwrap();
        assert_eq!(conflict_source_decoded, source);
        assert_eq!(rows[1].6.as_deref(), Some("winner"));
        assert_eq!(rows[1].7.as_deref(), Some("[\"a\"]"));

        // unresolved_endpoint
        assert_eq!(rows[2].1.as_deref(), Some("L1"));
        assert_eq!(rows[2].2.as_deref(), Some("linked_to_mux"));
    }

    #[test]
    fn aliases_round_trip() {
        let mut conn = fresh_conn();
        let mut snap = GraphSnapshot::empty();
        let agent = NodeId::AgentSession(AgentSessionId::new("h", "s", "k"));
        snap.aliases.insert(agent.clone(), "Alias Name".into());
        load(&snap, &mut conn).unwrap();
        let (node_json, node_kind, display_name): (String, String, String) = conn
            .query_row(
                "SELECT node, node_kind, display_name FROM aliases",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let decoded: crate::model::NodeId = serde_json::from_str(&node_json).unwrap();
        assert_eq!(decoded, agent);
        assert_eq!(node_kind, "agent_session");
        assert_eq!(display_name, "Alias Name");
    }

    #[test]
    fn loader_is_idempotent() {
        let mut conn = fresh_conn();
        let snap = full_snapshot();
        load(&snap, &mut conn).expect("first load");
        let first_node_count = count(&conn, "SELECT COUNT(*) FROM v_nodes");
        load(&snap, &mut conn).expect("re-load");
        let second_node_count = count(&conn, "SELECT COUNT(*) FROM v_nodes");
        assert_eq!(first_node_count, second_node_count);
        assert_eq!(first_node_count, 9);
    }

    #[test]
    fn reload_replaces_prior_rows() {
        let mut conn = fresh_conn();
        let snap_a = full_snapshot();
        load(&snap_a, &mut conn).unwrap();
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM v_nodes"), 9);

        let mut snap_b = GraphSnapshot::empty();
        snap_b
            .nodes
            .push(GraphNode::Workspace(make_workspace("/other")));
        load(&snap_b, &mut conn).unwrap();
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM v_nodes"), 1);
        let row: String = conn
            .query_row("SELECT root FROM node_workspaces", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row, "/other");
    }
}
