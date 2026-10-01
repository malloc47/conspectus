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

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use rusqlite::{Connection, OpenFlags, params};

use crate::discovery::DiscoveryCaches;
use crate::discovery::harness::codex::HARNESS_KEY as CODEX_HARNESS_KEY;
use crate::discovery::memo::FileStamp;
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MatchKind, Metadata, MuxSessionId, NodeId, Provenance, RelationKind,
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
    caches: &DiscoveryCaches,
) {
    let Some(db_path) = pick_active_log_db(state_root) else {
        return;
    };
    let ts_floor = now_epoch.saturating_sub(window_seconds.max(0));
    let candidates = collect_codex_pane_processes(codex_pids_per_mux);
    if candidates.is_empty() {
        return;
    }

    // H-SERVE-PERF-002: consult the query cache before opening the DB.
    // On a cache hit no SQLite connection is opened; on a miss we
    // open, verify the schema, re-query, and refresh the cache.
    // See `CachedQuery` docs for the fingerprint invariants.
    let observations =
        observations_for_candidates(&db_path, &candidates, ts_floor, &caches.codex_log);

    let state_scope = state_root.to_string_lossy().to_string();
    let mut emitted: Vec<GraphLink> = Vec::new();
    let mut process_links: Vec<GraphLink> = Vec::new();
    let mut synthesized: Vec<AgentSessionNode> = Vec::new();

    // Dedupe so the same (mux, thread) only produces one log-derived link
    // even if several Codex pids under the same mux happen to write to the
    // same thread (rare, but possible during fork transitions).
    let mut seen: BTreeSet<(MuxSessionId, String)> = BTreeSet::new();

    for candidate in candidates {
        let Some(observation) = observations.get(&candidate.pid).cloned() else {
            continue;
        };
        let key = (candidate.mux_id.clone(), observation.thread_id.clone());
        if !seen.insert(key) {
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
    let mut seen: BTreeSet<(MuxSessionId, i64)> = BTreeSet::new();

    for (mux_id, entries) in codex_pids_per_mux {
        for (harness_key, pid) in entries {
            if harness_key != CODEX_HARNESS_KEY {
                continue;
            }
            if !seen.insert((mux_id.clone(), *pid)) {
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

#[derive(Clone)]
struct ThreadObservation {
    thread_id: String,
    process_uuid: String,
    process_uuid_suffix: Option<String>,
    ts: i64,
}

/// Cross-cycle cache for [`apply_codex_log_attribution`] (H-SERVE-PERF-002).
///
/// The Codex logs SQLite DB is on the harness-class hot path: every
/// mux/harness re-run cycle re-opens `logs_<N>.sqlite` and runs one
/// `LIKE 'pid:<pid>:%'` scan per live Codex pid. On busy operator
/// boxes that DB is tens of megabytes and the `logs` table lacks a
/// `(process_uuid, ts)` index, so those scans dominate serve idle
/// CPU / tmpfs read volume (see this file's `DEFAULT_WINDOW_SECONDS`
/// docstring and ADR 0091 for the diagnosis).
///
/// This cache short-circuits the query when the DB file hasn't
/// advanced since the last cycle for the same candidate pid set —
/// the query result is deterministic in that case. The cache stores
/// only the pids that returned an observation; absent keys mean
/// "queried and got nothing," mirroring `query_freshest_thread`
/// returning `None`. When the DB advances (mtime or size changes)
/// or a new candidate pid appears, the cache is invalidated and
/// the query runs normally.
///
/// The `ts_floor` moves forward every cycle but the cache is safe
/// across advances: `query_freshest_thread` returns the freshest row
/// per pid (`ORDER BY ts DESC LIMIT 1`). If the freshest cached
/// observation ages out under a later floor, all older rows have
/// too — so filtering cached observations by the current floor
/// yields the same result as re-querying against the same DB.
struct CachedQuery {
    db_path: PathBuf,
    db_stamp: FileStamp,
    candidate_pids: Vec<i64>,
    /// `ts_floor` that was in force when the cache was populated.
    /// A later call whose `ts_floor` is >= this value can safely
    /// reuse the cache (freshest-per-pid is unchanged; a filter
    /// drops observations that have aged out). A later call whose
    /// `ts_floor` is *lower* (test widens the window, or clock
    /// skew) is a cache-miss because there may be older rows that
    /// the original narrower query never returned.
    cached_ts_floor: i64,
    /// pid → freshest observation returned by `query_freshest_thread`
    /// on the last run. Pids that returned `None` are absent from the
    /// map so a lookup miss stays cheap.
    observations: HashMap<i64, ThreadObservation>,
}

/// The last [`CachedQuery`], kept in [`DiscoveryCaches`].
#[derive(Default)]
pub(crate) struct QueryCache {
    last: Mutex<Option<CachedQuery>>,
    /// SQLite queries issued, so tests can check that a hit skipped them.
    #[cfg(test)]
    queries: std::sync::atomic::AtomicUsize,
}

impl QueryCache {
    /// Queries issued since the last call.
    #[cfg(test)]
    pub(crate) fn take_queries(&self) -> usize {
        self.queries.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

/// Compute observations for `candidates`, reusing the last query's
/// results when the underlying DB and candidate pid set are unchanged
/// since the last call. On cache hit no SQLite connection is opened.
/// On cache miss (or first call, or metadata unreadable) opens the
/// DB read-only, verifies the schema, runs the per-pid query, and
/// refreshes the cache.
///
/// The returned map is filtered by `ts_floor` so an aged-out
/// observation is dropped just as `query_freshest_thread` would have
/// dropped it under the newer floor.
fn observations_for_candidates(
    db_path: &Path,
    candidates: &[CodexPaneProcess],
    ts_floor: i64,
    cache: &QueryCache,
) -> HashMap<i64, ThreadObservation> {
    let stamp = FileStamp::of(db_path);
    let mut candidate_pids: Vec<i64> = candidates.iter().map(|c| c.pid).collect();
    candidate_pids.sort_unstable();
    candidate_pids.dedup();

    let mut cache_guard = cache.last.lock().unwrap_or_else(PoisonError::into_inner);
    if let (Some(stamp), Some(cached)) = (stamp, cache_guard.as_ref())
        && cached.db_path == db_path
        && cached.db_stamp == stamp
        && cached.candidate_pids == candidate_pids
        && ts_floor >= cached.cached_ts_floor
    {
        // Cache hit: replay observations, dropping any that have aged
        // out under the new floor. See CachedQuery docs for why this is
        // equivalent to re-querying the unchanged DB.
        return cached
            .observations
            .iter()
            .filter(|(_, obs)| obs.ts >= ts_floor)
            .map(|(pid, obs)| (*pid, obs.clone()))
            .collect();
    }

    // Cache miss — open the DB, verify the schema, and query. Any
    // failure between here and the successful query flushes the
    // cache so the next call sees a clean state.
    let Ok(connection) = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        *cache_guard = None;
        return HashMap::new();
    };
    let _ = connection.execute_batch("PRAGMA query_only = ON;");
    if !logs_table_present(&connection) {
        *cache_guard = None;
        return HashMap::new();
    }

    let mut observations: HashMap<i64, ThreadObservation> = HashMap::new();
    for candidate in candidates {
        if observations.contains_key(&candidate.pid) {
            continue;
        }
        #[cfg(test)]
        cache
            .queries
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(observation) = query_freshest_thread(&connection, candidate.pid, ts_floor) {
            observations.insert(candidate.pid, observation);
        }
    }

    if let Some(stamp) = stamp {
        *cache_guard = Some(CachedQuery {
            db_path: db_path.to_path_buf(),
            db_stamp: stamp,
            candidate_pids,
            cached_ts_floor: ts_floor,
            observations: observations.clone(),
        });
    } else {
        // Metadata unreadable — flush the cache so the next successful
        // stat re-populates it rather than serving stale entries.
        *cache_guard = None;
    }

    observations
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
            process_uuid_suffix: process_uuid
                .strip_prefix(&prefix)
                .map(std::string::ToString::to_string),
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
        serde_json::Value::String(MatchKind::CodexLogCurrentThreadMatch.to_string()),
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
            evidence: Some(MatchKind::CodexLogCurrentThreadMatch.to_string()),
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
            evidence: Some(MatchKind::CodexLogProcessObservation.to_string()),
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
    let process_ids: BTreeSet<NodeId> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::RuntimeProcess(process) if process.pid == Some(pid) => {
                Some(NodeId::RuntimeProcess(process.id.clone()))
            }
            _ => None,
        })
        .collect();
    let mux_node_id = NodeId::MuxSession(mux_id.clone());
    snapshot.candidate_links.iter().find_map(|link| {
        if link.relation == RelationKind::MuxContainsProcess
            && link.source == mux_node_id
            && let Some(target) = link.target_node_id()
            && process_ids.contains(target)
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
        serde_json::Value::String(MatchKind::CodexLogProcessThreadMatch.to_string()),
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
            evidence: Some(MatchKind::CodexLogProcessThreadMatch.to_string()),
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
        if link.source_metadata.match_kind() != Some(MatchKind::ActivePaneCommandSessionMatch) {
            continue;
        }

        let Some(stale_target) = mux_target(link) else {
            continue;
        };
        if stale_target != &fresh_target {
            continue;
        }
        let NodeId::AgentSession(stale_source) = &link.source else {
            continue;
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
