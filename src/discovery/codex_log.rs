//! Codex log-derived live-session attribution.
//!
//! Per ADR 0048 v1 slice B, this post-merge pass reads
//! `$CODEX_STATE_ROOT/logs_<N>.sqlite` strictly read-only to answer "which
//! Codex thread is each live Codex process currently writing?" and uses the
//! answer to fix mux↔session attribution when launch argv `--resume <A>` has
//! gone stale because the operator switched sessions in-process.
//!
//! Inputs: a precomputed `(mux → [(harness_key, pid)])` map produced by
//! `cross_link::active_harness_pids_per_mux`, which exposes the same
//! process-tree walk that powers `active_pane_process_match` candidates but
//! without the identity-evidence gating that suppresses publication when
//! fd/command evidence already resolved the mux. Consuming the raw pid set
//! is necessary because this linker's whole purpose is to **correct** stale
//! command/fd evidence — gating on "no identity evidence yet" would defeat
//! that. For each codex pid, the linker queries the `logs.process_uuid`
//! column (encoded as `pid:<os_pid>:<uuid>`) for the freshest `thread_id`
//! within a caller-supplied `ts` floor (see [`DEFAULT_WINDOW_SECONDS`] for
//! the default, dual-purpose query-cost / pid-reuse rationale, and the
//! `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS` env var for production override),
//! and emits a fresh `LinkedToMux` candidate that ranks above command/fd
//! evidence. Stale `active_pane_command_session_match` candidates for the
//! same mux are marked `Overridden` so the resolver no longer prefers them.
//! The linker can be skipped entirely via `CONSPECTUS_DISABLE_CODEX_LOG`.
//!
//! When the log thread id names a Codex session the slice-A state reader has
//! not yet observed, a sparse `AgentSession` node is synthesized in place so
//! the resolver and TUI have a stable target. The state reader replaces the
//! synthesized node naturally on the next discovery pass.
//!
//! The reader never selects `feedback_log_body` (privacy). Connections open
//! read-only with `query_only = ON` and stay short-lived.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use crate::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, Metadata, MuxSessionId, NodeId, Provenance, RelationKind,
    RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SourceMetadata,
};

const ADAPTER_NAME: &str = crate::discovery::providers::CODEX_LOG;
/// Default upper bound on log-row age accepted as evidence. This bound
/// serves two purposes; both justify keeping it nonzero by default and
/// neither matches ADR 0028's hook-sidecar TTL semantics:
///
/// 1. **Query performance.** The `logs` table lacks a `(process_uuid, ts)`
///    compound index, so an unbounded `LIKE 'pid:<pid>:%'` scan is expensive
///    on heavy users. Bounding by `ts` lets the planner use `idx_logs_ts` to
///    narrow the scan.
/// 2. **Pid-reuse defense.** The candidate pid set comes from the
///    process-tree walk and contains currently-live codex pids, but the
///    log row format `pid:<os_pid>:<uuid>` does not match the *current*
///    process uuid — only the prefix. If pid `P` previously ran codex `A`
///    (which wrote log rows), exited, and the kernel reused `P` for a
///    brand-new codex `B` that has not written any log rows yet, our
///    `LIKE 'pid:P:%' ORDER BY ts DESC LIMIT 1` query would return `A`'s
///    latest row and we would attribute the wrong thread. The time bound
///    keeps that misattribution narrow to the bound's window.
///
/// 24 hours is the default: long enough for an idle codex session that has
/// been quiescent between user turns, short enough that pid-reuse risk on
/// typical Linux pid spaces stays low. Overridable via
/// `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS`.
///
/// A proper pid-reuse fix would compare row `ts` against
/// `/proc/<pid>/stat.starttime` and accept only rows written after the
/// current process started. That refinement is deferred (`ProcessSnapshot`
/// does not expose start time today).
pub const DEFAULT_WINDOW_SECONDS: i64 = 24 * 60 * 60;
const PROCESS_UUID_PREFIX: &str = "pid:";

// H-HYG-001: re-export the canonical `current_epoch`. Kept as
// a re-export at this module path because
// `codex_log::current_epoch` is the identifier `apply_mutators`
// invokes.
pub use crate::discovery::current_epoch;

/// Apply Codex log-derived current-session attribution to a snapshot.
///
/// `state_root` is the harness state root for codex (where `logs_*.sqlite`
/// lives alongside `state_*.sqlite`). `codex_pids_per_mux` carries the live
/// codex pid set per mux from `cross_link::active_harness_pids_per_mux`;
/// supply an empty map to skip the linker (production discovery does this
/// when process-tree walking is disabled). `now_epoch` is the wall clock
/// used to compute the `ts` floor. `window_seconds` is the query bound
/// described on [`DEFAULT_WINDOW_SECONDS`]; pass that constant for the
/// default, or a different value when callers want a tighter or wider
/// bound (e.g. test fixtures, or production via the
/// `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS` env var).
pub fn apply_codex_log_attribution(
    snapshot: &mut GraphSnapshot,
    state_root: &Path,
    codex_pids_per_mux: &BTreeMap<MuxSessionId, Vec<(String, i64)>>,
    now_epoch: i64,
    window_seconds: i64,
) {
    let Some(db_path) = pick_active_log_db(state_root) else {
        return;
    };
    let Ok(connection) = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return;
    };
    let _ = connection.execute_batch("PRAGMA query_only = ON;");
    if !logs_table_present(&connection) {
        return;
    }

    let ts_floor = now_epoch.saturating_sub(window_seconds.max(0));
    let candidates = collect_codex_pane_processes(codex_pids_per_mux);
    if candidates.is_empty() {
        return;
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut emitted: Vec<GraphLink> = Vec::new();
    let mut process_links: Vec<GraphLink> = Vec::new();
    let mut synthesized: Vec<AgentSessionNode> = Vec::new();

    // Dedupe so the same (mux, thread) only produces one log-derived link
    // even if several Codex pids under the same mux happen to write to the
    // same thread (rare, but possible during fork transitions).
    let mut seen: BTreeMap<(MuxSessionId, String), bool> = BTreeMap::new();

    for candidate in candidates {
        let Some(observation) = query_freshest_thread(&connection, candidate.pid, ts_floor) else {
            continue;
        };
        let key = (candidate.mux_id.clone(), observation.thread_id.clone());
        if seen.insert(key, true).is_some() {
            continue;
        }

        let session_id =
            AgentSessionId::new(CODEX_HARNESS_KEY, &state_scope, &observation.thread_id);

        let existing_session = snapshot.nodes.iter().any(|node| match node {
            GraphNode::AgentSession(session) => session.id == session_id,
            _ => false,
        });
        if !existing_session {
            synthesized.push(
                AgentSessionNode::new(session_id.clone(), CODEX_HARNESS_KEY.to_string())
                    .with_last_active_epoch(observation.ts),
            );
        }

        emitted.push(build_link(
            &session_id,
            &candidate.mux_id,
            &observation,
            candidate.pid,
        ));
        let process_id = ensure_codex_runtime_process(
            snapshot,
            &candidate.mux_id,
            candidate.pid,
            observation.ts,
        );
        process_links.push(codex_process_link(
            process_id,
            NodeId::AgentSession(session_id.clone()),
            &observation,
            candidate.pid,
        ));
    }

    for synth in synthesized {
        snapshot.nodes.push(GraphNode::AgentSession(synth));
    }

    for link in &emitted {
        demote_stale_codex_command_matches(snapshot, link);
    }

    for link in emitted {
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
    }
    for link in process_links {
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
    }

    // Stamp any nodes/links added above with the codex_log provider;
    // first-write-wins so earlier providers' entries survive.
    crate::discovery::stamp_snapshot_mutations(snapshot, ADAPTER_NAME, now_epoch);
}

/// Pick the highest-suffix `logs_<N>.sqlite`. Mirrors the state-reader's
/// version-selection rule so the two readers agree on which Codex generation
/// is the active one.
fn pick_active_log_db(state_root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(state_root).ok()?;
    let mut best: Option<(u32, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(rest) = name
            .strip_prefix("logs_")
            .and_then(|s| s.strip_suffix(".sqlite"))
        else {
            continue;
        };
        let Ok(n) = rest.parse::<u32>() else {
            continue;
        };
        if best.as_ref().is_none_or(|(prev, _)| n > *prev) {
            best = Some((n, path));
        }
    }
    best.map(|(_, path)| path)
}

fn logs_table_present(connection: &Connection) -> bool {
    let Ok(mut stmt) =
        connection.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='logs'")
    else {
        return false;
    };
    stmt.exists([]).unwrap_or(false)
}

struct CodexPaneProcess {
    mux_id: MuxSessionId,
    pid: i64,
}

/// Project the precomputed `(harness_key, pid)` pairs into the codex-only
/// subset this linker needs.
fn collect_codex_pane_processes(
    codex_pids_per_mux: &BTreeMap<MuxSessionId, Vec<(String, i64)>>,
) -> Vec<CodexPaneProcess> {
    let mut output: Vec<CodexPaneProcess> = Vec::new();
    let mut seen: BTreeMap<(MuxSessionId, i64), ()> = BTreeMap::new();

    for (mux_id, entries) in codex_pids_per_mux {
        for (harness_key, pid) in entries {
            if harness_key != CODEX_HARNESS_KEY {
                continue;
            }
            if seen.insert((mux_id.clone(), *pid), ()).is_some() {
                continue;
            }
            output.push(CodexPaneProcess {
                mux_id: mux_id.clone(),
                pid: *pid,
            });
        }
    }

    output
}

fn mux_target(link: &GraphLink) -> Option<&MuxSessionId> {
    match &link.target {
        LinkEndpoint::Node {
            id: NodeId::MuxSession(id),
        } => Some(id),
        _ => None,
    }
}

struct ThreadObservation {
    thread_id: String,
    process_uuid: String,
    process_uuid_suffix: Option<String>,
    ts: i64,
}

/// Query the logs DB for the freshest `thread_id` written by the process
/// whose `process_uuid` starts with `pid:<pid>:`. Bounded by `ts_floor` so
/// the query never scans the whole table on heavy log volumes.
fn query_freshest_thread(
    connection: &Connection,
    pid: i64,
    ts_floor: i64,
) -> Option<ThreadObservation> {
    let prefix = format!("{PROCESS_UUID_PREFIX}{pid}:");
    let like_pattern = format!("{prefix}%");
    let mut stmt = connection
        .prepare(
            "SELECT thread_id, process_uuid, ts \
             FROM logs \
             WHERE process_uuid LIKE ?1 \
               AND thread_id IS NOT NULL \
               AND ts >= ?2 \
             ORDER BY ts DESC, ts_nanos DESC, id DESC \
             LIMIT 1",
        )
        .ok()?;
    stmt.query_row(params![like_pattern, ts_floor], |row| {
        let thread_id: String = row.get(0)?;
        let process_uuid: String = row.get(1)?;
        let ts: i64 = row.get(2)?;
        Ok(ThreadObservation {
            process_uuid_suffix: process_uuid.strip_prefix(&prefix).map(|s| s.to_string()),
            thread_id,
            process_uuid,
            ts,
        })
    })
    .ok()
}

fn build_link(
    session_id: &AgentSessionId,
    mux_id: &MuxSessionId,
    observation: &ThreadObservation,
    pid: i64,
) -> GraphLink {
    let source = NodeId::AgentSession(session_id.clone());
    let target = NodeId::MuxSession(mux_id.clone());

    let mut fields = Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String("codex_log_current_thread_match".to_string()),
    );
    fields.insert(
        "observed_epoch".to_string(),
        serde_json::Value::Number(observation.ts.into()),
    );
    fields.insert(
        "process_pid".to_string(),
        serde_json::Value::Number(pid.into()),
    );
    fields.insert(
        "process_uuid".to_string(),
        serde_json::Value::String(observation.process_uuid.clone()),
    );
    if let Some(uuid_suffix) = &observation.process_uuid_suffix {
        fields.insert(
            "process_uuid_suffix".to_string(),
            serde_json::Value::String(uuid_suffix.clone()),
        );
    }

    GraphLink {
        id: format!(
            "codex_log:{source}:linked_to_mux:{target}:{}",
            observation.ts
        ),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some("codex_log_current_thread_match".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn ensure_codex_runtime_process(
    snapshot: &mut GraphSnapshot,
    mux_id: &MuxSessionId,
    pid: i64,
    observed_epoch: i64,
) -> NodeId {
    if let Some(existing) = runtime_process_for_mux_pid(snapshot, mux_id, pid) {
        return existing;
    }

    let mux_node_id = NodeId::MuxSession(mux_id.clone());
    let observation_key = format!("codex_log:{mux_node_id}:pid:{pid}");
    let process_id = RuntimeProcessId::new(&observation_key);
    let process_node_id = NodeId::RuntimeProcess(process_id.clone());
    snapshot
        .nodes
        .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: process_id,
            observation_key,
            pid: Some(pid),
            parent_pid: None,
            root_pane_pid: None,
            command: Some("codex".to_string()),
            cwd: None,
            harness_key: Some(CODEX_HARNESS_KEY.to_string()),
            role: Some(RuntimeProcessRole::HumanAgent),
            depth: None,
            observed_epoch: Some(observed_epoch),
        }));
    let containment = GraphLink {
        id: format!("codex_log:{mux_node_id}:mux_contains_process:{process_node_id}"),
        source: mux_node_id,
        target: LinkEndpoint::Node {
            id: process_node_id.clone(),
        },
        relation: RelationKind::MuxContainsProcess,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some("codex_log_process_observation".to_string()),
            fields: Metadata::new(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    };
    if !snapshot
        .candidate_links
        .iter()
        .any(|existing| existing.id == containment.id)
    {
        snapshot.candidate_links.push(containment);
    }
    process_node_id
}

fn runtime_process_for_mux_pid(
    snapshot: &GraphSnapshot,
    mux_id: &MuxSessionId,
    pid: i64,
) -> Option<NodeId> {
    let process_ids: BTreeMap<_, _> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::RuntimeProcess(process) if process.pid == Some(pid) => {
                Some((NodeId::RuntimeProcess(process.id.clone()), ()))
            }
            _ => None,
        })
        .collect();
    let mux_node_id = NodeId::MuxSession(mux_id.clone());
    snapshot.candidate_links.iter().find_map(|link| {
        if link.relation == RelationKind::MuxContainsProcess
            && link.source == mux_node_id
            && let Some(target) = link.target_node_id()
            && process_ids.contains_key(target)
        {
            return Some(target.clone());
        }
        None
    })
}

fn codex_process_link(
    process_id: NodeId,
    session_id: NodeId,
    observation: &ThreadObservation,
    pid: i64,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(
            crate::resolve::evidence::CODEX_LOG_PROCESS_THREAD_MATCH.to_string(),
        ),
    );
    fields.insert(
        "observed_epoch".to_string(),
        serde_json::Value::Number(observation.ts.into()),
    );
    fields.insert(
        "process_pid".to_string(),
        serde_json::Value::Number(pid.into()),
    );
    fields.insert(
        "process_uuid".to_string(),
        serde_json::Value::String(observation.process_uuid.clone()),
    );
    GraphLink {
        id: format!("codex_log:{process_id}:process_identifies_session:{session_id}"),
        source: process_id,
        target: LinkEndpoint::Node { id: session_id },
        relation: RelationKind::ProcessIdentifiesSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(crate::resolve::evidence::CODEX_LOG_PROCESS_THREAD_MATCH.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

/// Mark `active_pane_command_session_match` candidates for the same mux as
/// `Overridden`, matching the demotion shape established for hook sidecars
/// in ADR 0028. The override only triggers when the stale link points at a
/// different Codex agent session than the freshly-resolved one — a
/// command-match candidate for the same session as the log-derived match is
/// corroborating evidence, not stale, and stays active.
fn demote_stale_codex_command_matches(snapshot: &mut GraphSnapshot, fresh: &GraphLink) {
    let Some(fresh_target) = mux_target(fresh).cloned() else {
        return;
    };
    let fresh_source_session = match &fresh.source {
        NodeId::AgentSession(id) => id.clone(),
        _ => return,
    };

    for link in &mut snapshot.candidate_links {
        if link.relation != RelationKind::LinkedToMux {
            continue;
        }
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        let match_kind = link
            .source_metadata
            .fields
            .get(crate::model::source_field::MATCH_KIND)
            .and_then(serde_json::Value::as_str)
            .or(link.source_metadata.evidence.as_deref());
        if match_kind != Some(crate::resolve::evidence::ACTIVE_PANE_COMMAND_SESSION_MATCH) {
            continue;
        }

        let Some(stale_target) = mux_target(link) else {
            continue;
        };
        if stale_target != &fresh_target {
            continue;
        }
        let stale_source = match &link.source {
            NodeId::AgentSession(id) => id,
            _ => continue,
        };
        if stale_source.harness_key != CODEX_HARNESS_KEY {
            continue;
        }
        if stale_source == &fresh_source_session {
            continue;
        }

        link.state = LinkState::Overridden {
            by: fresh.id.clone(),
            reason: Some(
                "fresh codex log current-thread evidence supersedes stale launch argv match"
                    .to_string(),
            ),
        };
    }
}

#[cfg(test)]
#[path = "codex_log_tests.rs"]
mod tests;
