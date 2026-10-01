//! Codex harness discovery.
//!
//! Per ADR 0048 v1 the adapter has two reader paths that feed a single merged
//! view of Codex sessions:
//!
//! - The state-database reader opens `$STATE_ROOT/state_<N>.sqlite` (highest
//!   numeric suffix wins), strictly read-only with `query_only` enabled, and
//!   pulls indexed rows from the `threads` and `thread_spawn_edges` tables.
//!   Column probing tolerates future schema additions and missing columns.
//! - The rollout reader walks `$STATE_ROOT/sessions/**/rollout-*.jsonl`,
//!   parses the first `session_meta` line for cwd/fork pointers, and tails
//!   the file body for a `last_message_preview` (ADR 0023).
//!
//! State rows are authoritative for `cwd`, `title`, and `last_active_epoch`
//! (millisecond precision via `updated_at_ms`/`created_at_ms`). The rollout
//! reader still supplies `last_message_preview` because state has no preview
//! column. Sessions present only in one source are emitted from that source
//! alone.
//!
//! Lineage produces two distinct `lineage_kind` operations per ADR 0018:
//! - `fork` from rollout `session_meta.forked_from_id` (user-initiated forks).
//! - `spawn` from `thread_spawn_edges` (subagent spawn relationships).
//!
//! Both can coexist on the same session. Malformed records and rollouts
//! without a `session_meta` envelope are skipped silently so a single bad
//! file cannot poison discovery.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::memo::{FileStamp, StampedMap};
use crate::discovery::{DiscoveryCaches, DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
    normalize_last_message_preview,
};

/// Maximum number of bytes to read from the tail of a rollout when
/// looking for the most recent message preview. Codex rollouts can
/// be many MB, so a bounded tail keeps the scan cheap.
const TAIL_SCAN_BYTES: u64 = 32 * 1024;

pub const HARNESS_KEY: &str = crate::discovery::providers::CODEX;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

/// H-EXT-004 runtime attribution surface for codex. Codex ships
/// as `codex` on `PATH`, session ids are UUID-shaped, and there
/// are no daemon / subagent helper processes to distinguish.
static CODEX_RUNTIME_SIGNATURE: super::RuntimeSignature = super::RuntimeSignature {
    harness_key: HARNESS_KEY,
    // H-EXT-005: the CLI hook-writer pid-resolver
    // (`cli::harness_binaries`) accepts `codex-rs` as an
    // alternate binary name (rust rewrite lineage). Keep both
    // here so the CLI's registry-driven lookup gets the same
    // pair the pre-H-EXT-005 hardcoded match did.
    process_command_basenames: &["codex", "codex-rs"],
    command_substrings: &["codex"],
    fd_path_patterns: &["/.codex/sessions/", "/.codex/tmp/"],
    extract_session_keys: super::generic_uuid_like_session_keys,
    is_background_process: super::no_match,
    is_subagent_process: super::no_match,
};

impl HarnessAdapter for CodexAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn runtime_signature(&self) -> &'static super::RuntimeSignature {
        &CODEX_RUNTIME_SIGNATURE
    }

    fn transcript_source(
        &self,
        session: &crate::model::AgentSessionId,
    ) -> Option<crate::viewer::model::SessionLocator> {
        Some(crate::viewer::model::SessionLocator {
            harness_key: HARNESS_KEY.to_string(),
            session_key: session.session_key.clone(),
            state_root: session.state_scope.clone().into(),
        })
    }

    fn transcript_parser(&self) -> Option<&'static dyn crate::viewer::parser::HarnessParser> {
        Some(&crate::viewer::parser::codex::CodexParser)
    }

    /// H-EXT-007: run the codex-log ADR 0048 aux reader as the
    /// codex adapter's aux attribution pass. Reads the
    /// `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS` env var directly so
    /// the pre-H-EXT-007 `LocalDiscoveryConfig.codex_log_window_seconds`
    /// field can retire — the knob is codex-specific and belongs
    /// on the adapter, not on the top-level config.
    fn apply_aux_attribution(
        &self,
        snapshot: &mut crate::model::GraphSnapshot,
        ctx: &super::AuxAttributionContext<'_>,
    ) {
        let window_seconds = std::env::var("CONSPECTUS_CODEX_LOG_WINDOW_SECONDS")
            .ok()
            .and_then(|raw| raw.parse::<i64>().ok())
            .filter(|secs| *secs >= 0)
            .unwrap_or(crate::discovery::codex_log::DEFAULT_WINDOW_SECONDS);
        crate::discovery::codex_log::apply_codex_log_attribution(
            snapshot,
            ctx.state_root,
            ctx.harness_pids_per_mux,
            ctx.now_epoch,
            window_seconds,
            ctx.caches,
        );
    }

    fn launch_options(&self) -> &'static [super::HarnessLaunchOption] {
        super::CODEX_LAUNCH_OPTIONS
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        // H-REF-007: delegate the state-root lookup + fragment
        // stamping to the shared envelope so this adapter only
        // describes its layout.
        super::discover_with_state_root(context, HARNESS_KEY, |root| {
            discover_state(root, context.caches())
        })
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("codex")]
    }

    fn resume_argv(
        &self,
        session_id: &str,
        _cwd: &std::path::Path,
    ) -> Option<Vec<std::ffi::OsString>> {
        // Matches the TUI resume-command shape in
        // `src/tui/resume.rs:46`.
        Some(vec![
            std::ffi::OsString::from("codex"),
            std::ffi::OsString::from("exec"),
            std::ffi::OsString::from("--resume"),
            std::ffi::OsString::from(session_id),
        ])
    }
}

fn discover_state(state_root: &Path, caches: &DiscoveryCaches) -> Result<GraphFragment> {
    let sessions_dir = state_root.join("sessions");
    let state_db = pick_active_state_db(state_root);

    // Both readers may be empty; degrade silently.
    if !sessions_dir.exists() && state_db.is_none() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut sessions: BTreeMap<String, MergedSession> = BTreeMap::new();
    let mut spawn_edges: Vec<SpawnEdge> = Vec::new();

    if let Some(db_path) = state_db.as_deref() {
        let out = read_state_database(db_path);
        for row in out.threads {
            sessions.entry(row.id.clone()).or_default().merge_state(row);
        }
        spawn_edges = out.spawn_edges;
    }

    if sessions_dir.exists() {
        visit_rollouts(&sessions_dir, &mut |path| {
            if let Some(scan) = scan_rollout_cached(path, &caches.codex_rollouts) {
                sessions
                    .entry(scan.meta.id.clone())
                    .or_default()
                    .merge_rollout(&scan.meta, scan.preview, scan.activity);
            }
        })?;
    }

    let known_ids: HashSet<&str> = sessions.keys().map(std::string::String::as_str).collect();
    let mut nodes = Vec::with_capacity(sessions.len());
    let mut candidate_links = Vec::new();

    for (id, session) in &sessions {
        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: session.cwd.clone(),
            title: session.title.clone(),
            last_message_preview: session.last_message_preview.clone(),
            last_active_epoch: session.last_active_epoch,
            session_kind: None,
        }));

        if let Some(parent) = session.forked_from_id.as_deref()
            && parent != id
            && !parent.is_empty()
        {
            let resolved = known_ids.get(parent).map(|_| parent);
            candidate_links.push(build_lineage_link(
                id,
                parent,
                &state_scope,
                resolved,
                LineageOp::Fork,
            ));
        }
    }

    for edge in &spawn_edges {
        if edge.parent.is_empty() || edge.child.is_empty() || edge.parent == edge.child {
            continue;
        }
        let resolved = known_ids
            .get(edge.parent.as_str())
            .map(|_| edge.parent.as_str());
        candidate_links.push(build_lineage_link(
            &edge.child,
            &edge.parent,
            &state_scope,
            resolved,
            LineageOp::Spawn,
        ));
    }

    Ok(GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    })
}

/// Aggregated per-session data merged across the state DB and rollout files.
/// State rows are authoritative for `cwd`, `title`, and `last_active_epoch`;
/// rollouts contribute the `last_message_preview` and the fork pointer.
#[derive(Default)]
struct MergedSession {
    cwd: Option<String>,
    title: Option<String>,
    last_message_preview: Option<String>,
    last_active_epoch: Option<i64>,
    forked_from_id: Option<String>,
}

impl MergedSession {
    fn merge_state(&mut self, row: ThreadStateRow) {
        if row.cwd.is_some() {
            self.cwd = row.cwd;
        }
        if let Some(title) = row.title.filter(|t| !t.trim().is_empty()) {
            self.title = Some(title);
        } else if self.title.is_none()
            && let Some(message) = row.first_user_message.as_deref()
            && let Some(preview) = normalize_last_message_preview(message)
        {
            self.title = Some(preview);
        }
        if let Some(epoch_ms) = row.updated_at_ms.or(row.created_at_ms) {
            self.last_active_epoch = Some(epoch_ms / 1000);
        }
    }

    fn merge_rollout(
        &mut self,
        meta: &SessionMetaPayload,
        preview: Option<String>,
        activity: Option<i64>,
    ) {
        if self.cwd.is_none() {
            self.cwd = meta.cwd.clone();
        }
        if self.last_message_preview.is_none() {
            self.last_message_preview = preview;
        }
        if self.last_active_epoch.is_none() {
            self.last_active_epoch = activity;
        }
        if self.forked_from_id.is_none() {
            self.forked_from_id = meta.forked_from_id.clone();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LineageOp {
    Fork,
    Spawn,
}

impl LineageOp {
    fn as_str(self) -> &'static str {
        match self {
            LineageOp::Fork => "fork",
            LineageOp::Spawn => "spawn",
        }
    }

    fn evidence(self) -> &'static str {
        match self {
            LineageOp::Fork => "codex session_meta fork",
            LineageOp::Spawn => "codex thread_spawn_edges",
        }
    }

    fn link_segment(self) -> &'static str {
        match self {
            // Preserve the existing fork link-id shape for snapshot stability.
            LineageOp::Fork => "",
            LineageOp::Spawn => "spawn:",
        }
    }
}

fn build_lineage_link(
    child_session_key: &str,
    parent_session_key: &str,
    state_scope: &str,
    resolved_parent: Option<&str>,
    op: LineageOp,
) -> GraphLink {
    let lineage_kind = op.as_str();
    let segment = op.link_segment();

    let mut fields: Metadata = Metadata::new();
    fields.insert("harness_key".to_string(), json!(HARNESS_KEY));
    fields.insert(
        crate::model::source_field::LINEAGE_KIND.to_string(),
        json!(lineage_kind),
    );
    fields.insert(
        "parent_native_id".to_string(),
        json!(parent_session_key.to_string()),
    );

    let source = NodeId::AgentSession(AgentSessionId::new(
        HARNESS_KEY,
        state_scope,
        child_session_key,
    ));

    let (target, link_id) = if let Some(parent_key) = resolved_parent {
        let parent_id = AgentSessionId::new(HARNESS_KEY, state_scope, parent_key);
        (
            LinkEndpoint::Node {
                id: NodeId::AgentSession(parent_id),
            },
            format!("codex:lineage:{segment}{child_session_key}:parent_session:{parent_key}"),
        )
    } else {
        let evidence = UnresolvedEndpoint {
            node_type: "agent_session".to_string(),
            harness_key: Some(HARNESS_KEY.to_string()),
            native_id: Some(parent_session_key.to_string()),
            state_scope: Some(state_scope.to_string()),
            path: None,
            metadata: fields.clone(),
        };
        (
            LinkEndpoint::Unresolved { evidence },
            format!(
                "codex:lineage:{segment}{child_session_key}:parent_session:unresolved:{parent_session_key}"
            ),
        )
    };

    GraphLink {
        id: link_id,
        source,
        target,
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: HARNESS_KEY.to_string(),
            evidence: Some(op.evidence().to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

/// Pick the `state_<N>.sqlite` with the highest numeric suffix. Codex bumps
/// the suffix on breaking schema changes, so the highest version is the file
/// the live codex process is reading and writing.
fn pick_active_state_db(state_root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(state_root).ok()?;
    let mut best: Option<(u32, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(rest) = name
            .strip_prefix("state_")
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

#[derive(Default)]
struct StateReadOutput {
    threads: Vec<ThreadStateRow>,
    spawn_edges: Vec<SpawnEdge>,
}

#[derive(Clone, Debug)]
struct ThreadStateRow {
    id: String,
    cwd: Option<String>,
    title: Option<String>,
    first_user_message: Option<String>,
    updated_at_ms: Option<i64>,
    created_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
struct SpawnEdge {
    parent: String,
    child: String,
}

fn read_state_database(db_path: &Path) -> StateReadOutput {
    let Ok(connection) = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return StateReadOutput::default();
    };

    // Defense in depth: read_only flags already block writes, but
    // `query_only` blocks any attached database or future allowance from
    // mutating either. Errors here are non-fatal — the worst case is the
    // pragma silently no-ops on an old SQLite, and we still cannot write.
    let _ = connection.execute_batch("PRAGMA query_only = ON;");

    let threads = read_threads(&connection);
    let spawn_edges = read_spawn_edges(&connection);
    StateReadOutput {
        threads,
        spawn_edges,
    }
}

fn read_threads(connection: &Connection) -> Vec<ThreadStateRow> {
    let columns = table_columns(connection, "threads");
    if !columns.iter().any(|c| c == "id") {
        return Vec::new();
    }
    let has_cwd = columns.iter().any(|c| c == "cwd");
    let has_title = columns.iter().any(|c| c == "title");
    let has_first_user_message = columns.iter().any(|c| c == "first_user_message");
    let has_updated_ms = columns.iter().any(|c| c == "updated_at_ms");
    let has_created_ms = columns.iter().any(|c| c == "created_at_ms");

    let mut select = String::from("SELECT id");
    select.push_str(if has_cwd { ", cwd" } else { ", NULL" });
    select.push_str(if has_title { ", title" } else { ", NULL" });
    select.push_str(if has_first_user_message {
        ", first_user_message"
    } else {
        ", NULL"
    });
    select.push_str(if has_updated_ms {
        ", updated_at_ms"
    } else {
        ", NULL"
    });
    select.push_str(if has_created_ms {
        ", created_at_ms"
    } else {
        ", NULL"
    });
    select.push_str(" FROM threads");

    let Ok(mut stmt) = connection.prepare(&select) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(ThreadStateRow {
            id: row.get::<_, String>(0)?,
            cwd: row.get::<_, Option<String>>(1)?,
            title: row.get::<_, Option<String>>(2)?,
            first_user_message: row.get::<_, Option<String>>(3)?,
            updated_at_ms: row.get::<_, Option<i64>>(4)?,
            created_at_ms: row.get::<_, Option<i64>>(5)?,
        })
    }) else {
        return Vec::new();
    };

    rows.filter_map(Result::ok)
        .filter(|r| !r.id.trim().is_empty())
        .collect()
}

fn read_spawn_edges(connection: &Connection) -> Vec<SpawnEdge> {
    let columns = table_columns(connection, "thread_spawn_edges");
    if !columns.iter().any(|c| c == "parent_thread_id")
        || !columns.iter().any(|c| c == "child_thread_id")
    {
        return Vec::new();
    }
    let Ok(mut stmt) =
        connection.prepare("SELECT parent_thread_id, child_thread_id FROM thread_spawn_edges")
    else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(SpawnEdge {
            parent: row.get::<_, String>(0)?,
            child: row.get::<_, String>(1)?,
        })
    }) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info(\"{table}\")");
    let Ok(mut stmt) = connection.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn visit_rollouts(dir: &Path, on_rollout: &mut dyn FnMut(&Path)) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            visit_rollouts(&path, on_rollout)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if name.starts_with("rollout-") && path.extension().is_some_and(|ext| ext == "jsonl") {
            on_rollout(&path);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct SessionMetaEnvelope {
    #[serde(rename = "type")]
    kind: String,
    payload: SessionMetaPayload,
}

#[derive(Clone, Deserialize)]
struct SessionMetaPayload {
    id: String,
    #[serde(default)]
    cwd: Option<String>,
    /// Present when codex forked this rollout from another. Resume continues
    /// to be written into the same rollout file rather than creating a new
    /// one, so this is the only cross-rollout lineage pointer the format
    /// exposes today.
    #[serde(default)]
    forked_from_id: Option<String>,
}

fn read_session_meta(path: &Path) -> Option<SessionMetaPayload> {
    // H-SERVE-PERF-006: only the first line matters (the
    // `session_meta` envelope), so read a single line rather
    // than the whole file. The pre-fix path used
    // `fs::read_to_string` which loaded the entire rollout —
    // up to 24 MB per file on active operator boxes — and
    // discarded everything past the first newline, dominating
    // serve idle CPU / tmpfs read volume once earlier caches
    // eliminated the SQLite, /proc walk, gh, and GitProbe
    // spawn costs (ADR 0091).
    let file = fs::File::open(path).ok()?;
    let mut first = String::new();
    BufReader::new(file).read_line(&mut first).ok()?;
    if first.is_empty() {
        return None;
    }
    let envelope: SessionMetaEnvelope = serde_json::from_str(first.trim_end()).ok()?;

    if envelope.kind != "session_meta" {
        return None;
    }

    Some(envelope.payload)
}

// ---------------------------------------------------------------------------
// H-SERVE-PERF-007: per-rollout scan cache.
// ---------------------------------------------------------------------------
//
// `discover_state`'s `visit_rollouts` loop opens every `rollout-*.jsonl`
// under `~/.codex/sessions/**/` on every harness cycle, calling
// `read_session_meta` (first line) + `read_rollout_last_message_preview`
// (32 KB tail). Almost every rollout is dormant, yet each cycle re-opens
// them all. Cache the extracted (meta, preview, activity) triple per
// file, fingerprinted on `(mtime_ns, size)` — a warm call stats only.

#[derive(Clone)]
pub(crate) struct RolloutScan {
    meta: SessionMetaPayload,
    preview: Option<String>,
    activity: Option<i64>,
}

fn scan_rollout_cached(
    path: &Path,
    cache: &StampedMap<FileStamp, RolloutScan>,
) -> Option<RolloutScan> {
    let stamp = FileStamp::of(path)?;
    if let Some(scan) = cache.get(path, &stamp) {
        return Some(scan);
    }
    let scan = RolloutScan {
        meta: read_session_meta(path)?,
        preview: read_rollout_last_message_preview(path),
        activity: stamp.modified_epoch(),
    };
    cache.insert(path.to_path_buf(), stamp, scan.clone());
    Some(scan)
}

/// Extract the rollout's most recent user/assistant text content as a
/// preview (ADR 0023). Walks the trailing [`TAIL_SCAN_BYTES`] of the
/// JSONL file backward, dropping the partial first line when the
/// seek lands mid-file. Only `response_item` records with
/// `payload.type == "message"` and a `user`/`assistant` role
/// contribute; `reasoning`, `function_call`, `function_call_output`,
/// and `event_msg` records are skipped. Within a message the
/// extractor returns the last `input_text`/`output_text` block with
/// non-empty content.
///
/// The result is normalized via [`normalize_last_message_preview`]
/// (whitespace collapsed, capped at 200 chars with `…`). Discovery
/// stays best-effort: corrupt JSON, empty rollouts, and rollouts
/// whose tail contains only tool / reasoning payloads yield `None`.
fn read_rollout_last_message_preview(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len == 0 {
        return None;
    }

    let start = len.saturating_sub(TAIL_SCAN_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((len - start) as usize);
    file.read_to_end(&mut buf).ok()?;

    // Drop the first partial line if we started mid-file.
    let scan_start = if start > 0 {
        match buf.iter().position(|&b| b == b'\n') {
            Some(idx) => idx + 1,
            None => return None,
        }
    } else {
        0
    };

    let lines: Vec<&[u8]> = buf[scan_start..]
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    for line in lines.iter().rev() {
        let Ok(parsed) = serde_json::from_slice::<RolloutLine>(line) else {
            continue;
        };
        if let Some(text) = extract_rollout_preview_text(&parsed)
            && let Some(preview) = normalize_last_message_preview(&text)
        {
            return Some(preview);
        }
    }
    None
}

#[derive(Deserialize)]
struct RolloutLine {
    #[serde(rename = "type", default)]
    record_type: Option<String>,
    #[serde(default)]
    payload: Option<RolloutPayload>,
}

#[derive(Deserialize)]
struct RolloutPayload {
    #[serde(rename = "type", default)]
    payload_type: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Vec<RolloutContent>>,
}

#[derive(Deserialize)]
struct RolloutContent {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

fn extract_rollout_preview_text(line: &RolloutLine) -> Option<String> {
    if line.record_type.as_deref() != Some("response_item") {
        return None;
    }
    let payload = line.payload.as_ref()?;
    if payload.payload_type.as_deref() != Some("message") {
        return None;
    }
    let role = payload.role.as_deref()?;
    if role != "user" && role != "assistant" {
        return None;
    }
    let blocks = payload.content.as_ref()?;
    for block in blocks.iter().rev() {
        let block_type = block.block_type.as_deref()?;
        if block_type != "input_text" && block_type != "output_text" {
            continue;
        }
        let Some(text) = block.text.as_ref() else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        // Codex wraps a handful of system-flavor turns in XML-style
        // channel markers (`<turn_aborted>`, `<proposed_plan>`, …).
        // Drop the marker prefix when present so the preview shows
        // the actual body; when the body is empty after stripping,
        // return None and let the outer backward walk pick the
        // previous message instead.
        return apply_codex_channel_marker_filter(text);
    }
    None
}

/// Channel-marker tag names codex wraps system-flavor turns in.
/// Conservative on purpose: only listed tags are stripped, so a
/// legitimate `<html>` or `<foo>` in user content is left alone.
const CODEX_CHANNEL_MARKERS: &[&str] = &["turn_aborted", "proposed_plan"];

/// If `text` opens with a known codex channel marker (`<marker>` or
/// `<marker> body`), drop the marker (and a matching `</marker>`
/// closing tag if present), trim, and return the body when
/// non-empty. A bare `<marker>` with nothing after returns `None`
/// so the caller's backward walk skips this message and tries an
/// earlier one. Text without a known marker is returned verbatim.
fn apply_codex_channel_marker_filter(text: &str) -> Option<String> {
    let trimmed = text.trim_start();
    for marker in CODEX_CHANNEL_MARKERS {
        let open_tag = format!("<{marker}>");
        if let Some(after_open) = trimmed.strip_prefix(&open_tag) {
            let body = after_open.trim_start();
            let close_tag = format!("</{marker}>");
            let body = body.strip_suffix(&close_tag).unwrap_or(body).trim();
            if body.is_empty() {
                return None;
            }
            return Some(body.to_string());
        }
    }
    Some(text.to_string())
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
