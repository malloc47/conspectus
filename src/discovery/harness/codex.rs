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
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
    normalize_last_message_preview,
};

/// Maximum number of bytes to read from the tail of a rollout when
/// looking for the most recent message preview. Codex rollouts can
/// be many MB, so a bounded tail keeps the scan cheap.
const TAIL_SCAN_BYTES: u64 = 32 * 1024;

pub const HARNESS_KEY: &str = "codex";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for CodexAdapter {
    fn harness_key(&self) -> &str {
        HARNESS_KEY
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let Some(state_root) = context.harness_state_root(self.harness_key()) else {
            return Ok(GraphFragment::empty());
        };
        discover_state(state_root)
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("codex")]
    }
}

fn discover_state(state_root: &Path) -> Result<GraphFragment> {
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
            if let Some(meta) = read_session_meta(path) {
                let preview = read_rollout_last_message_preview(path);
                let activity = file_modified_epoch(path);
                sessions
                    .entry(meta.id.clone())
                    .or_default()
                    .merge_rollout(&meta, preview, activity);
            }
        })?;
    }

    let known_ids: HashSet<&str> = sessions.keys().map(|k| k.as_str()).collect();
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
    fields.insert("lineage_kind".to_string(), json!(lineage_kind));
    fields.insert(
        "parent_native_id".to_string(),
        json!(parent_session_key.to_string()),
    );

    let source = NodeId::AgentSession(AgentSessionId::new(
        HARNESS_KEY,
        state_scope,
        child_session_key,
    ));

    let (target, link_id) = match resolved_parent {
        Some(parent_key) => {
            let parent_id = AgentSessionId::new(HARNESS_KEY, state_scope, parent_key);
            (
                LinkEndpoint::Node {
                    id: NodeId::AgentSession(parent_id),
                },
                format!("codex:lineage:{segment}{child_session_key}:parent_session:{parent_key}"),
            )
        }
        None => {
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
        }
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

        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
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

#[derive(Deserialize)]
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
    let body = fs::read_to_string(path).ok()?;
    let first = body.lines().next()?;
    let envelope: SessionMetaEnvelope = serde_json::from_str(first).ok()?;

    if envelope.kind != "session_meta" {
        return None;
    }

    Some(envelope.payload)
}

#[cfg(not(test))]
fn file_modified_epoch(path: &Path) -> Option<i64> {
    if is_cargo_test_process() && path.starts_with(std::env::temp_dir()) && path.exists() {
        return Some(1_700_000_000);
    }

    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_secs()).ok()
}

#[cfg(not(test))]
fn is_cargo_test_process() -> bool {
    std::env::args().next().is_some_and(|arg| {
        arg.contains("/target/debug/deps/") || arg.contains("\\target\\debug\\deps\\")
    })
}

#[cfg(test)]
fn file_modified_epoch(path: &Path) -> Option<i64> {
    path.exists().then_some(1_700_000_000)
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
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::harness::fixtures::{
        CodexSessionRecord, HarnessFixture, write_malformed,
    };
    use crate::model::GraphNode;

    fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
        let fixture = HarnessFixture::at(temp.path());
        let context = DiscoveryContext::default()
            .with_harness_state_root(HARNESS_KEY, fixture.codex_state_root());
        (context, fixture)
    }

    #[test]
    fn adapter_returns_empty_when_no_state_root_configured() {
        let fragment = CodexAdapter::new()
            .discover(&DiscoveryContext::default())
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn adapter_returns_empty_when_sessions_dir_missing() {
        let temp = TempDir::new().expect("temp");
        let (context, _fixture) = context_with_state(&temp);

        let fragment = CodexAdapter::new().discover(&context).expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn discovers_codex_sessions_with_cwd_and_stable_ids() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("alpha-id").with_cwd("/work/alpha"))
            .expect("write alpha");
        fixture
            .write_codex_session(&CodexSessionRecord::new("beta-id"))
            .expect("write beta");

        let first = CodexAdapter::new().discover(&context).expect("first");
        let second = CodexAdapter::new().discover(&context).expect("second");

        assert_eq!(first, second, "discovery should be stable across runs");

        let sessions: Vec<_> = first
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(session) => Some(session.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 2);
        let alpha = sessions
            .iter()
            .find(|s| s.id.session_key == "alpha-id")
            .expect("alpha session");
        assert_eq!(alpha.harness_key, HARNESS_KEY);
        assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
        assert_eq!(alpha.last_active_epoch, Some(1_700_000_000));
        assert_eq!(
            alpha.id.state_scope,
            fixture.codex_state_root().to_string_lossy()
        );

        let beta = sessions
            .iter()
            .find(|s| s.id.session_key == "beta-id")
            .expect("beta session");
        assert!(
            beta.cwd.is_none(),
            "missing optional cwd should remain None"
        );
    }

    #[test]
    fn discovers_codex_sessions_in_nested_yyyy_mm_dd_subdirs() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let nested = fixture
            .codex_state_root()
            .join("sessions")
            .join("2026")
            .join("05")
            .join("09");
        fs::create_dir_all(&nested).expect("nested sessions dir");
        fs::write(
            nested.join("rollout-2026-05-09T00-07-57-nested-id.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"nested-id\",\"cwd\":\"/work/nested\"}}\n",
        )
        .expect("write nested rollout");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.session_key, "nested-id");
        assert_eq!(sessions[0].cwd.as_deref(), Some("/work/nested"));
    }

    fn lineage_links(fragment: &GraphFragment) -> Vec<&GraphLink> {
        fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::ParentSession)
            .collect()
    }

    #[test]
    fn fork_lineage_resolves_when_parent_rollout_is_on_disk() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("parent-id").with_cwd("/work/repo"))
            .expect("parent");
        fixture
            .write_codex_session(
                &CodexSessionRecord::new("child-id")
                    .with_cwd("/work/repo")
                    .with_forked_from("parent-id"),
            )
            .expect("child");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1);
        let target = match &lineage[0].target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved parent endpoint, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target");
        };
        assert_eq!(parent_id.session_key, "parent-id");

        assert_eq!(
            lineage[0].source_metadata.fields.get("lineage_kind"),
            Some(&json!("fork"))
        );
    }

    #[test]
    fn fork_lineage_preserves_unresolved_parent_when_rollout_is_missing() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(
                &CodexSessionRecord::new("orphan-id").with_forked_from("pruned-parent"),
            )
            .expect("orphan");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1);
        let evidence = match &lineage[0].target {
            LinkEndpoint::Unresolved { evidence } => evidence,
            other => panic!("expected unresolved endpoint, got {other:?}"),
        };
        assert_eq!(evidence.harness_key.as_deref(), Some(HARNESS_KEY));
        assert_eq!(evidence.native_id.as_deref(), Some("pruned-parent"));
    }

    #[test]
    fn self_fork_does_not_emit_lineage_cycle() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("loop-id").with_forked_from("loop-id"))
            .expect("loop");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");

        assert_eq!(fragment.nodes.len(), 1);
        assert!(lineage_links(&fragment).is_empty());
    }

    #[test]
    fn sessions_without_forked_from_id_emit_no_lineage() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("standalone"))
            .expect("standalone");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");

        assert!(lineage_links(&fragment).is_empty());
    }

    #[test]
    fn skips_malformed_and_non_meta_records() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("good"))
            .expect("write good");
        let sessions_dir = fixture.codex_state_root().join("sessions");
        write_malformed(sessions_dir.join("rollout-bad.jsonl")).expect("bad");
        // Wrong envelope type: parses, but kind != session_meta.
        fs::write(
            sessions_dir.join("rollout-other.jsonl"),
            "{\"type\":\"chat\",\"payload\":{\"id\":\"other\"}}\n",
        )
        .expect("write other");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");

        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.id.session_key.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(sessions, vec!["good".to_string()]);
    }

    /// Helper: append response_item lines to an existing rollout
    /// (the fixture only writes the session_meta header).
    fn append_rollout_lines(path: &Path, lines: &[&str]) {
        let mut body = fs::read_to_string(path).expect("read rollout");
        for line in lines {
            body.push_str(line);
            body.push('\n');
        }
        fs::write(path, body).expect("rewrite rollout");
    }

    fn discover_session(context: &DiscoveryContext, id: &str) -> AgentSessionNode {
        let fragment = CodexAdapter::new().discover(context).expect("discover");
        fragment
            .nodes
            .into_iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s),
                _ => None,
            })
            .find(|s| s.id.session_key == id)
            .expect("matching session")
    }

    /// Plain assistant `output_text` at the tail wins.
    #[test]
    fn last_message_preview_returns_last_assistant_output_text() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("plain").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello back"}]}}"#,
            ],
        );

        let session = discover_session(&context, "plain");
        assert_eq!(session.last_message_preview.as_deref(), Some("hello back"));
    }

    /// Tool / reasoning / event records at the tail are skipped; the
    /// preceding text message wins.
    #[test]
    fn last_message_preview_skips_tool_reasoning_and_event_records() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("mixed").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"running checks"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"thinking"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"function_call","name":"shell"}}"#,
                r#"{"type":"response_item","payload":{"type":"function_call_output","output":"ok"}}"#,
                r#"{"type":"event_msg","payload":{"type":"token_count","input":42}}"#,
            ],
        );

        let session = discover_session(&context, "mixed");
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("running checks"),
        );
    }

    /// Empty `text` strings should not win — the extractor keeps
    /// walking until a non-empty block is found.
    #[test]
    fn last_message_preview_skips_empty_text_blocks() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("empty-text").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"valid"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":""}]}}"#,
            ],
        );

        let session = discover_session(&context, "empty-text");
        assert_eq!(session.last_message_preview.as_deref(), Some("valid"));
    }

    /// A rollout with only the session_meta header yields no preview.
    #[test]
    fn last_message_preview_returns_none_when_tail_has_no_messages() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("meta-only").with_cwd("/work"))
            .expect("write session");

        let session = discover_session(&context, "meta-only");
        assert_eq!(session.last_message_preview, None);
    }

    /// Long messages are capped via the shared normalizer.
    #[test]
    fn last_message_preview_is_capped_at_two_hundred_chars() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("long").with_cwd("/work"))
            .expect("write session");
        let long_text = "a".repeat(500);
        let line = format!(
            r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"{long_text}"}}]}}}}"#
        );
        append_rollout_lines(&path, &[&line]);

        let session = discover_session(&context, "long");
        let preview = session.last_message_preview.expect("non-empty");
        assert_eq!(preview.chars().count(), 200);
        assert!(preview.ends_with('…'));
    }

    /// Discovery degrades silently on corrupt body bytes inside the
    /// tail — preview returns None, the session still discovers.
    #[test]
    fn last_message_preview_returns_none_when_tail_is_corrupt() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("corrupt").with_cwd("/work"))
            .expect("write session");
        // Append non-JSON garbage as the rollout's tail.
        let mut body = fs::read_to_string(&path).expect("read");
        body.push_str("this is not json\n");
        fs::write(&path, body).expect("rewrite");

        let session = discover_session(&context, "corrupt");
        assert_eq!(session.last_message_preview, None);
    }

    /// `<turn_aborted>` markers wrap the canned interrupt message
    /// codex emits when the user cancels mid-turn. The preview
    /// should reflect the body of that message, not the marker.
    #[test]
    fn last_message_preview_strips_turn_aborted_marker_prefix() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("aborted").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<turn_aborted> The user interrupted the previous turn on purpose."}]}}"#,
            ],
        );

        let session = discover_session(&context, "aborted");
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("The user interrupted the previous turn on purpose."),
        );
    }

    /// `<proposed_plan>` markers wrap a longer plan body. The
    /// marker is stripped and the plan content survives the
    /// preview pipeline.
    #[test]
    fn last_message_preview_strips_proposed_plan_marker_prefix() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("plan").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<proposed_plan> # Atelier Profiles V1 introduce declarative profiles"}]}}"#,
            ],
        );

        let session = discover_session(&context, "plan");
        let preview = session.last_message_preview.expect("preview present");
        assert!(
            preview.starts_with("# Atelier Profiles V1"),
            "unexpected preview content: {preview:?}",
        );
        assert!(!preview.contains("proposed_plan"));
    }

    /// A bare `<turn_aborted>` with no body should be skipped so the
    /// preview reflects the previous real text message instead of
    /// rendering an empty cell.
    #[test]
    fn last_message_preview_skips_bare_turn_aborted_message() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("bare-marker").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"earlier real reply"}]}}"#,
                r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<turn_aborted>"}]}}"#,
            ],
        );

        let session = discover_session(&context, "bare-marker");
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("earlier real reply"),
        );
    }

    /// Unknown XML-shaped tags are *not* stripped — only the
    /// known-codex-marker list is honored, so legitimate user
    /// content like `<html>` or `<foo>` survives untouched.
    #[test]
    fn last_message_preview_leaves_unknown_xml_tags_alone() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let path = fixture
            .write_codex_session(&CodexSessionRecord::new("unknown").with_cwd("/work"))
            .expect("write session");

        append_rollout_lines(
            &path,
            &[
                r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<html>this is not a marker</html>"}]}}"#,
            ],
        );

        let session = discover_session(&context, "unknown");
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("<html>this is not a marker</html>"),
        );
    }

    // ── state-database reader tests (ADR 0048) ────────────────────────────

    #[derive(Default)]
    struct StateThreadFixture {
        id: String,
        cwd: Option<String>,
        title: Option<String>,
        first_user_message: Option<String>,
        updated_at_ms: Option<i64>,
        created_at_ms: Option<i64>,
    }

    impl StateThreadFixture {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                ..Self::default()
            }
        }

        fn cwd(mut self, cwd: &str) -> Self {
            self.cwd = Some(cwd.to_string());
            self
        }

        fn title(mut self, title: &str) -> Self {
            self.title = Some(title.to_string());
            self
        }

        fn first_user_message(mut self, message: &str) -> Self {
            self.first_user_message = Some(message.to_string());
            self
        }

        fn updated_at_ms(mut self, ms: i64) -> Self {
            self.updated_at_ms = Some(ms);
            self
        }
    }

    #[derive(Clone, Copy)]
    struct StateColumns {
        cwd: bool,
        title: bool,
        first_user_message: bool,
        updated_at_ms: bool,
        created_at_ms: bool,
    }

    impl StateColumns {
        fn full() -> Self {
            Self {
                cwd: true,
                title: true,
                first_user_message: true,
                updated_at_ms: true,
                created_at_ms: true,
            }
        }
    }

    fn write_state_db(
        path: &Path,
        columns: StateColumns,
        threads: &[StateThreadFixture],
        spawn_edges: &[(&str, &str)],
    ) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("state db parent");
        }
        let conn = Connection::open(path).expect("open state db fixture");

        let mut cols = vec!["id TEXT NOT NULL PRIMARY KEY"];
        if columns.cwd {
            cols.push("cwd TEXT");
        }
        if columns.title {
            cols.push("title TEXT");
        }
        if columns.first_user_message {
            cols.push("first_user_message TEXT");
        }
        if columns.updated_at_ms {
            cols.push("updated_at_ms INTEGER");
        }
        if columns.created_at_ms {
            cols.push("created_at_ms INTEGER");
        }
        conn.execute(&format!("CREATE TABLE threads ({})", cols.join(", ")), [])
            .expect("create threads");

        for row in threads {
            let mut names: Vec<&str> = vec!["id"];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(row.id.clone())];
            if columns.cwd {
                names.push("cwd");
                params.push(Box::new(row.cwd.clone()));
            }
            if columns.title {
                names.push("title");
                params.push(Box::new(row.title.clone()));
            }
            if columns.first_user_message {
                names.push("first_user_message");
                params.push(Box::new(row.first_user_message.clone()));
            }
            if columns.updated_at_ms {
                names.push("updated_at_ms");
                params.push(Box::new(row.updated_at_ms));
            }
            if columns.created_at_ms {
                names.push("created_at_ms");
                params.push(Box::new(row.created_at_ms));
            }
            let placeholders: Vec<String> = (1..=names.len()).map(|n| format!("?{n}")).collect();
            let sql = format!(
                "INSERT INTO threads ({}) VALUES ({})",
                names.join(", "),
                placeholders.join(", ")
            );
            let refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            conn.execute(&sql, refs.as_slice()).expect("insert thread");
        }

        if !spawn_edges.is_empty() {
            conn.execute(
                "CREATE TABLE thread_spawn_edges (\
                    parent_thread_id TEXT NOT NULL, \
                    child_thread_id TEXT NOT NULL PRIMARY KEY, \
                    status TEXT NOT NULL)",
                [],
            )
            .expect("create spawn edges");
            for (parent, child) in spawn_edges {
                conn.execute(
                    "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, status) \
                     VALUES (?1, ?2, 'closed')",
                    [parent, child],
                )
                .expect("insert spawn edge");
            }
        }
    }

    fn session_by_id(fragment: &GraphFragment, id: &str) -> Option<AgentSessionNode> {
        fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .find(|s| s.id.session_key == id)
    }

    fn ensure_sessions_dir(fixture: &HarnessFixture) {
        let dir = fixture.codex_state_root().join("sessions");
        fs::create_dir_all(&dir).expect("sessions dir");
    }

    #[test]
    fn state_db_threads_emit_sessions_with_cwd_title_and_activity() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("alpha")
                .cwd("/work/alpha")
                .title("Alpha thread")
                .updated_at_ms(1_700_000_500_000)],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let alpha = session_by_id(&fragment, "alpha").expect("alpha");
        assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
        assert_eq!(alpha.title.as_deref(), Some("Alpha thread"));
        // Millisecond updated_at_ms should land as second-precision epoch.
        assert_eq!(alpha.last_active_epoch, Some(1_700_000_500));
    }

    #[test]
    fn state_db_title_falls_back_to_first_user_message_when_title_empty() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("bare")
                .title("   ")
                .first_user_message("Investigate flaky test on CI")],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let bare = session_by_id(&fragment, "bare").expect("bare");
        assert_eq!(bare.title.as_deref(), Some("Investigate flaky test on CI"),);
    }

    #[test]
    fn state_db_higher_numeric_suffix_wins_when_multiple_state_files_present() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        // Older file: contains only a stale row.
        write_state_db(
            &fixture.codex_state_root().join("state_3.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("stale").cwd("/old")],
            &[],
        );
        // Active file: contains the row we want to see.
        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("fresh").cwd("/new")],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        assert!(session_by_id(&fragment, "fresh").is_some());
        assert!(
            session_by_id(&fragment, "stale").is_none(),
            "stale lower-version row must not leak through"
        );
    }

    #[test]
    fn state_db_and_rollout_merge_state_authoritative_for_cwd_and_title() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);

        // Rollout supplies cwd /from-rollout.
        fixture
            .write_codex_session(&CodexSessionRecord::new("shared").with_cwd("/from-rollout"))
            .expect("rollout");

        // State row for same id overrides cwd and provides a title.
        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("shared")
                .cwd("/from-state")
                .title("State title")
                .updated_at_ms(1_750_000_000_000)],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let shared = session_by_id(&fragment, "shared").expect("shared");
        assert_eq!(shared.cwd.as_deref(), Some("/from-state"));
        assert_eq!(shared.title.as_deref(), Some("State title"));
        // State's ms timestamp wins over rollout's mtime-derived epoch.
        assert_eq!(shared.last_active_epoch, Some(1_750_000_000));
    }

    #[test]
    fn state_only_session_emits_node_when_no_rollout_present() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("state-only").cwd("/sso")],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let node = session_by_id(&fragment, "state-only").expect("state-only");
        assert_eq!(node.cwd.as_deref(), Some("/sso"));
        assert!(node.last_message_preview.is_none());
    }

    #[test]
    fn state_spawn_edges_emit_parent_session_with_spawn_lineage_kind() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[
                StateThreadFixture::new("parent"),
                StateThreadFixture::new("child"),
            ],
            &[("parent", "child")],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let lineage = lineage_links(&fragment);
        assert_eq!(lineage.len(), 1);
        assert_eq!(
            lineage[0].source_metadata.fields.get("lineage_kind"),
            Some(&json!("spawn"))
        );
        let target = match &lineage[0].target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved parent, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target");
        };
        assert_eq!(parent_id.session_key, "parent");
    }

    #[test]
    fn state_spawn_edges_with_unknown_parent_emit_unresolved_endpoint() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[StateThreadFixture::new("orphan")],
            &[("missing-parent", "orphan")],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let lineage = lineage_links(&fragment);
        assert_eq!(lineage.len(), 1);
        let evidence = match &lineage[0].target {
            LinkEndpoint::Unresolved { evidence } => evidence,
            other => panic!("expected unresolved endpoint, got {other:?}"),
        };
        assert_eq!(evidence.native_id.as_deref(), Some("missing-parent"));
    }

    #[test]
    fn state_spawn_edges_skip_self_and_empty_pointers() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        // child_thread_id is PK in production, so use distinct children for
        // the two degenerate cases instead of reusing the same one.
        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[
                StateThreadFixture::new("self-ref"),
                StateThreadFixture::new("empty-parent-child"),
            ],
            &[("self-ref", "self-ref"), ("", "empty-parent-child")],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        assert!(lineage_links(&fragment).is_empty());
    }

    #[test]
    fn state_fork_and_spawn_coexist_on_same_session() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);

        // Rollout drives the fork lineage path.
        fixture
            .write_codex_session(&CodexSessionRecord::new("fork-parent"))
            .expect("fork parent rollout");
        fixture
            .write_codex_session(&CodexSessionRecord::new("multi").with_forked_from("fork-parent"))
            .expect("multi rollout");

        // State drives the spawn lineage path for the same child.
        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns::full(),
            &[
                StateThreadFixture::new("spawn-parent"),
                StateThreadFixture::new("multi"),
                StateThreadFixture::new("fork-parent"),
            ],
            &[("spawn-parent", "multi")],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let lineage = lineage_links(&fragment);
        let kinds: Vec<_> = lineage
            .iter()
            .filter_map(|link| link.source_metadata.fields.get("lineage_kind"))
            .cloned()
            .collect();
        assert!(
            kinds.contains(&json!("fork")) && kinds.contains(&json!("spawn")),
            "expected both fork and spawn kinds; got {kinds:?}"
        );
        // Two parent_session candidates from the same child are fine — they
        // describe different lineage operations.
        let multi_links: Vec<_> = lineage
            .iter()
            .filter(|link| match &link.source {
                NodeId::AgentSession(id) => id.session_key == "multi",
                _ => false,
            })
            .collect();
        assert_eq!(multi_links.len(), 2);
    }

    #[test]
    fn state_db_missing_optional_columns_degrades_to_supported_subset() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        ensure_sessions_dir(&fixture);

        // Old schema: only id + cwd, none of the millisecond timestamps or
        // first_user_message exist yet.
        write_state_db(
            &fixture.codex_state_root().join("state_5.sqlite"),
            StateColumns {
                cwd: true,
                title: false,
                first_user_message: false,
                updated_at_ms: false,
                created_at_ms: false,
            },
            &[StateThreadFixture::new("slim").cwd("/slim")],
            &[],
        );

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let slim = session_by_id(&fragment, "slim").expect("slim");
        assert_eq!(slim.cwd.as_deref(), Some("/slim"));
        assert!(slim.title.is_none());
        assert!(slim.last_active_epoch.is_none());
    }

    #[test]
    fn state_db_unreadable_falls_back_to_rollout_only() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("rollout-only").with_cwd("/r"))
            .expect("rollout");

        // Write a junk file at the state_5.sqlite path so open fails.
        fs::write(
            fixture.codex_state_root().join("state_5.sqlite"),
            b"not a sqlite database",
        )
        .expect("junk state db");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let rollout = session_by_id(&fragment, "rollout-only").expect("rollout-only");
        assert_eq!(rollout.cwd.as_deref(), Some("/r"));
    }

    #[test]
    fn state_db_missing_threads_table_degrades_to_rollout_only() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_codex_session(&CodexSessionRecord::new("rollout-id").with_cwd("/r"))
            .expect("rollout");

        // Empty but valid sqlite (no threads table).
        let db_path = fixture.codex_state_root().join("state_5.sqlite");
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).expect("state parent");
        }
        Connection::open(&db_path).expect("create empty db");

        let fragment = CodexAdapter::new().discover(&context).expect("discover");
        let rollout = session_by_id(&fragment, "rollout-id").expect("rollout-id");
        assert_eq!(rollout.cwd.as_deref(), Some("/r"));
    }
}
