//! opencode harness discovery.
//!
//! Reads modern `$STATE_ROOT/opencode.db` sessions plus legacy
//! `$STATE_ROOT/storage/session/<id>/info.json` records and emits one
//! `AgentSession` per discovered session. Records that fail to parse or are
//! missing the `id` field are skipped silently.
//!
//! `opencode.db` comes in two layouts (ADR 0125). opencode 1.x keeps
//! sessions in `session` and messages in `message` / `part`. opencode 2.x
//! keeps them in `session_v2` and `session_message`; a fresh 2.x database
//! has no 1.x tables, and a migrated one keeps its 1.x tables, stale,
//! beside copies of the same sessions under the same ids. Both layouts are
//! read and a `session_v2` row wins over a `session` row with its id.
//!
//! Per ADR 0018 the SQLite-backed reader also extracts the `parent_id`
//! column when present and emits intra-harness `parent_session` candidate
//! links between two `AgentSession` endpoints. Older databases that lack
//! the column degrade by reading sessions without lineage rather than
//! dropping everything. In 2.x `parent_id` is set only by the `subagent`
//! tool, so it reads as `spawn` lineage and marks the child a subagent,
//! and a fork's `fork_session_id` reads as `fork` lineage.
//!
//! Subagent sessions (openCode `@explore` / `@general` workers) are
//! classified via a schema probe for the `kind` column, with a title-
//! pattern heuristic as fallback when the column is absent. Subagent
//! sessions carry `session_kind: Subagent` on their `AgentSessionNode` so
//! downstream TUI and resolver code can nest, filter, or suppress them.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::memo::{SqliteStamp, StampedMap};
use crate::discovery::{DiscoveryCaches, DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SessionKind, SourceMetadata,
    UnresolvedEndpoint, normalize_last_message_preview,
};

pub const HARNESS_KEY: &str = crate::discovery::providers::OPENCODE;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenCodeAdapter;

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

/// Runtime attribution surface for opencode.
/// Opencode's session ids follow the `ses_<alphanumeric>`
/// grammar with a UUID fallback for legacy sessions;
/// the 1.x CLI spawns subagent processes distinguished by
/// ` subagent` in their argv, and the 2.x shared service
/// runs as `opencode serve --service`.
static OPENCODE_RUNTIME_SIGNATURE: super::RuntimeSignature = super::RuntimeSignature {
    harness_key: HARNESS_KEY,
    process_command_basenames: &["opencode"],
    command_substrings: &["opencode"],
    command_token_basenames: &[],
    fd_path_patterns: &[
        "/.local/share/opencode/",
        "/.config/opencode/",
        "/.opencode/",
    ],
    extract_session_keys: opencode_extract_session_keys,
    is_background_process: opencode_is_background_process,
    is_subagent_process: opencode_is_subagent_process,
};

/// Combine opencode's `ses_<alphanumeric>` grammar with the
/// generic UUID fallback so legacy sessions still resolve.
fn opencode_extract_session_keys(value: &str) -> std::collections::BTreeSet<String> {
    let mut keys = opencode_session_key_values(value);
    keys.extend(super::generic_uuid_like_session_keys(value));
    keys
}

fn opencode_session_key_values(value: &str) -> std::collections::BTreeSet<String> {
    value
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .filter(|part| {
            part.strip_prefix("ses_").is_some_and(|rest| {
                rest.len() >= 8 && rest.chars().all(|ch| ch.is_ascii_alphanumeric())
            })
        })
        .map(str::to_string)
        .collect()
}

/// Headless opencode servers are no one's interactive pane: the 2.x
/// shared service (`opencode serve --service`), which starts as a child
/// of the first TUI and so sits in that TUI's pane, a plain `opencode
/// serve`, and `opencode acp`, which another program drives over stdio.
/// The subcommand is the first argument that isn't a flag.
fn opencode_is_background_process(command: &str) -> bool {
    command
        .split_whitespace()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .is_some_and(|subcommand| matches!(subcommand, "serve" | "acp"))
}

/// Recognize opencode subagent processes by command line.
fn opencode_is_subagent_process(command: &str) -> bool {
    command.to_ascii_lowercase().contains(" subagent")
}

impl HarnessAdapter for OpenCodeAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn runtime_signature(&self) -> &'static super::RuntimeSignature {
        &OPENCODE_RUNTIME_SIGNATURE
    }

    fn transcript_source(
        &self,
        session: &crate::model::AgentSessionId,
    ) -> Option<crate::viewer::model::SessionLocator> {
        // Resolve the SQLite database path. A session's
        // `state_scope` may be either the database file itself or
        // its containing directory; both are accepted so existing
        // operator configs keep working.
        let mut db_path = std::path::PathBuf::from(session.state_scope.clone());
        if db_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("db"))
            != Some(true)
        {
            db_path.push("opencode.db");
        }
        Some(crate::viewer::model::SessionLocator {
            harness_key: HARNESS_KEY.to_string(),
            session_key: session.session_key.clone(),
            state_root: db_path,
        })
    }

    fn transcript_parser(&self) -> Option<&'static dyn crate::viewer::parser::HarnessParser> {
        Some(&crate::viewer::parser::opencode::OpenCodeParser)
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        // Delegate to the shared state-root envelope.
        super::discover_with_state_root(context, HARNESS_KEY, |root| {
            discover_state(root, context.caches())
        })
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("opencode")]
    }

    fn resume_argv(
        &self,
        session_id: &str,
        _cwd: &std::path::Path,
    ) -> Option<Vec<std::ffi::OsString>> {
        // `opencode --session <id>` continues a specific session
        // (see `opencode --help`). The positional `[project]`
        // argument is left out so tmux's `-c <cwd>` handles the
        // working directory, mirroring the codex / claude-code
        // pattern.
        Some(vec![
            std::ffi::OsString::from("opencode"),
            std::ffi::OsString::from("--session"),
            std::ffi::OsString::from(session_id),
        ])
    }
}

fn discover_state(state_root: &Path, caches: &DiscoveryCaches) -> Result<GraphFragment> {
    let state_scope = state_root.to_string_lossy().to_string();
    let mut sessions = BTreeMap::new();

    for info in
        read_sqlite_sessions_cached(&state_root.join("opencode.db"), &caches.opencode_sessions)
    {
        sessions.insert(info.id.clone(), info);
    }

    for info in read_legacy_sessions(state_root)? {
        sessions.entry(info.id.clone()).or_insert(info);
    }

    let mut nodes = Vec::with_capacity(sessions.len());
    let mut candidate_links = Vec::new();

    for info in sessions.values() {
        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, &info.id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: info.directory.clone(),
            title: info.title.clone(),
            last_message_preview: info.last_message_preview.clone(),
            last_active_epoch: info.last_active_epoch,
            session_kind: info.session_kind,
        }));
    }

    for info in sessions.values() {
        let parent = info.parent_id.as_deref().map(|parent| {
            let op = match info.layout {
                Layout::V1 => LineageOp::Parent,
                Layout::V2 => LineageOp::Spawn,
            };
            (parent, op)
        });
        let fork = info
            .fork_session_id
            .as_deref()
            .map(|source| (source, LineageOp::Fork));
        for (parent_id, op) in parent.into_iter().chain(fork) {
            if parent_id.is_empty() || parent_id == info.id {
                // Empty parent strings are meaningless; self-parent links would
                // create a cycle that can't represent real lineage.
                continue;
            }

            let resolved_parent = sessions.get(parent_id).map(|_| parent_id);
            candidate_links.push(build_lineage_link(
                &info.id,
                parent_id,
                op,
                &state_scope,
                resolved_parent,
            ));
        }
    }

    Ok(GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    })
}

#[derive(Clone, Deserialize)]
pub(crate) struct SessionInfo {
    id: String,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    title: Option<String>,
    /// Sourced from `session.parent_id` when the SQLite schema carries it.
    /// Legacy filesystem records do not expose lineage and leave this `None`.
    #[serde(default)]
    parent_id: Option<String>,
    /// Sourced from the most recent `type: "text"` row in the `part`
    /// table when present. Capped/normalized via
    /// [`normalize_last_message_preview`]. Legacy filesystem records
    /// and older schemas without the table leave this `None`.
    #[serde(default)]
    last_message_preview: Option<String>,
    /// Best-effort latest activity timestamp in Unix epoch seconds.
    /// SQLite rows store millisecond timestamps; legacy info.json stores
    /// the same shape under `time.updated` / `time.created`.
    #[serde(default)]
    last_active_epoch: Option<i64>,
    /// Harness-level session classification. Populated from the `kind`
    /// column when the schema carries it, otherwise inferred via a
    /// title-pattern heuristic for subagent detection.
    #[serde(default)]
    session_kind: Option<SessionKind>,
    #[serde(default)]
    time: Option<SessionTime>,
    /// Sourced from `session_v2.fork_session_id`: the session this one
    /// was forked from. 1.x and legacy records leave this `None`.
    #[serde(skip)]
    fork_session_id: Option<String>,
    #[serde(skip)]
    layout: Layout,
}

/// Which `opencode.db` layout a session row came from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Layout {
    #[default]
    V1,
    V2,
}

#[derive(Clone, Deserialize)]
struct SessionTime {
    #[serde(default)]
    created: Option<i64>,
    #[serde(default)]
    updated: Option<i64>,
}

fn read_sqlite_sessions(path: &Path) -> Vec<SessionInfo> {
    if !path.exists() {
        return Vec::new();
    }

    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };

    let mut sessions: BTreeMap<String, SessionInfo> = read_v1_sessions(&connection)
        .into_iter()
        .map(|info| (info.id.clone(), info))
        .collect();
    for info in read_v2_sessions(&connection) {
        sessions.insert(info.id.clone(), info);
    }
    sessions.into_values().collect()
}

fn read_v1_sessions(connection: &Connection) -> Vec<SessionInfo> {
    let Some(mut query) = session_query(connection) else {
        return Vec::new();
    };

    let Ok(rows) = query.statement.query_map([], |row| {
        let id: String = row.get(0)?;
        let directory: Option<String> = row.get(1)?;
        let title: Option<String> = row.get(2)?;
        let mut col: usize = 3;

        let parent_id = if query.has_parent {
            let val = row.get::<_, Option<String>>(col)?;
            col += 1;
            val
        } else {
            None
        };

        let raw_kind = if query.has_kind {
            let val = row.get::<_, Option<String>>(col)?;
            col += 1;
            val
        } else {
            None
        };

        let (time_updated, time_created) = if query.has_time {
            let u = row.get::<_, Option<i64>>(col)?;
            let c = row.get::<_, Option<i64>>(col + 1)?;
            (u, c)
        } else {
            (None, None)
        };

        let last_active_epoch = epoch_ms_to_seconds(time_updated.or(time_created));

        let session_kind =
            classify_session_kind(raw_kind.as_deref(), parent_id.as_deref(), title.as_deref());

        Ok(SessionInfo {
            id,
            directory,
            title,
            parent_id,
            last_active_epoch,
            last_message_preview: None,
            session_kind,
            time: None,
            fork_session_id: None,
            layout: Layout::V1,
        })
    }) else {
        return Vec::new();
    };

    let mut sessions = nonempty_ids(rows);

    // Attach the most recent text-part preview per session. Best-effort:
    // schemas that don't carry the `part` table (or lack JSON1 support
    // in this rusqlite build, which the `bundled` feature ensures we
    // have) degrade to `None`.
    attach_previews(&mut sessions, &read_last_message_previews(connection));
    sessions
}

fn read_v2_sessions(connection: &Connection) -> Vec<SessionInfo> {
    let Ok(mut statement) = connection.prepare(
        "SELECT id, directory, title, parent_id, fork_session_id, time_updated, time_created \
         FROM session_v2 ORDER BY id",
    ) else {
        return Vec::new();
    };

    let Ok(rows) = statement.query_map([], |row| {
        let parent_id: Option<String> = row.get(3)?;
        let time_updated: Option<i64> = row.get(5)?;
        let time_created: Option<i64> = row.get(6)?;
        // 2.x sets `parent_id` only on sessions the `subagent` tool spawns.
        let session_kind = parent_id.as_ref().map(|_| SessionKind::Subagent);
        Ok(SessionInfo {
            id: row.get(0)?,
            directory: row.get(1)?,
            title: row.get(2)?,
            parent_id,
            last_active_epoch: epoch_ms_to_seconds(time_updated.or(time_created)),
            last_message_preview: None,
            session_kind,
            time: None,
            fork_session_id: row.get(4)?,
            layout: Layout::V2,
        })
    }) else {
        return Vec::new();
    };

    let mut sessions = nonempty_ids(rows);
    attach_previews(&mut sessions, &read_v2_last_message_previews(connection));
    sessions
}

fn nonempty_ids(rows: impl Iterator<Item = rusqlite::Result<SessionInfo>>) -> Vec<SessionInfo> {
    rows.filter_map(|row| {
        let info = row.ok()?;
        if info.id.trim().is_empty() {
            None
        } else {
            Some(info)
        }
    })
    .collect()
}

fn attach_previews(sessions: &mut [SessionInfo], previews: &BTreeMap<String, String>) {
    for session in sessions {
        if let Some(text) = previews.get(&session.id) {
            session.last_message_preview = normalize_last_message_preview(text);
        }
    }
}

struct SessionQuery<'conn> {
    statement: rusqlite::Statement<'conn>,
    has_parent: bool,
    has_time: bool,
    has_kind: bool,
}

fn session_query(connection: &Connection) -> Option<SessionQuery<'_>> {
    // Probe for the `kind` column first (openCode ≥ some-future-version that
    // tags subagent sessions with a dedicated field). If the column exists the
    // adapter reads it directly; otherwise it falls back to a title-pattern
    // heuristic keyed on `parent_id` presence.
    let candidates = [
        (
            "SELECT id, directory, title, parent_id, kind, time_updated, time_created FROM session ORDER BY id",
            true,
            true,
            true,
        ),
        (
            "SELECT id, directory, title, parent_id, kind FROM session ORDER BY id",
            true,
            false,
            true,
        ),
        (
            "SELECT id, directory, title, parent_id, time_updated, time_created FROM session ORDER BY id",
            true,
            true,
            false,
        ),
        (
            "SELECT id, directory, title, parent_id FROM session ORDER BY id",
            true,
            false,
            false,
        ),
        (
            "SELECT id, directory, title, time_updated, time_created FROM session ORDER BY id",
            false,
            true,
            false,
        ),
        (
            "SELECT id, directory, title FROM session ORDER BY id",
            false,
            false,
            false,
        ),
    ];

    for (sql, has_parent, has_time, has_kind) in candidates {
        if let Ok(statement) = connection.prepare(sql) {
            return Some(SessionQuery {
                statement,
                has_parent,
                has_time,
                has_kind,
            });
        }
    }

    None
}

fn epoch_ms_to_seconds(epoch_ms: Option<i64>) -> Option<i64> {
    epoch_ms.map(|value| value / 1000)
}

/// Build a `session_id → most-recent-text` map by walking the `part`
/// table backward. Skips non-text parts (`step-start`, `step-finish`,
/// `reasoning`, `tool-call`, etc.) and empty text fields. The whole
/// query is best-effort — returns an empty map if the table is
/// missing, JSON1 isn't compiled in (it is under `bundled`), or any
/// row fails to parse.
fn read_last_message_previews(connection: &Connection) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let sql = "WITH ranked AS ( \
        SELECT session_id, json_extract(data, '$.text') AS text, \
               ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY time_created DESC, id DESC) AS rn \
        FROM part \
        WHERE json_extract(data, '$.type') = 'text' \
          AND json_extract(data, '$.text') IS NOT NULL \
          AND length(json_extract(data, '$.text')) > 0 \
    ) \
    SELECT session_id, text FROM ranked WHERE rn = 1";
    let Ok(mut statement) = connection.prepare(sql) else {
        return out;
    };
    let Ok(rows) = statement.query_map([], |row| {
        let session_id: String = row.get(0)?;
        let text: String = row.get(1)?;
        Ok((session_id, text))
    }) else {
        return out;
    };
    for row in rows.flatten() {
        out.insert(row.0, row.1);
    }
    out
}

/// Build a `session_id → most-recent-text` map from 2.x `session_message`
/// rows: the newest non-empty user prompt or assistant `text` content
/// item by `seq`. Reasoning and tool content are skipped, matching the
/// 1.x preview. Best-effort like [`read_last_message_previews`]; a row
/// whose `data` is not JSON is treated as empty rather than failing the
/// query.
fn read_v2_last_message_previews(connection: &Connection) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let sql = "WITH texts AS ( \
        SELECT m.session_id AS session_id, m.seq AS seq, CAST(c.key AS INTEGER) AS ord, \
               json_extract(c.value, '$.text') AS text \
        FROM session_message m, \
             json_each(CASE WHEN json_valid(m.data) THEN m.data ELSE '{}' END, '$.content') c \
        WHERE m.type = 'assistant' AND json_extract(c.value, '$.type') = 'text' \
        UNION ALL \
        SELECT session_id, seq, -1, \
               CASE WHEN json_valid(data) THEN json_extract(data, '$.text') END \
        FROM session_message WHERE type = 'user' \
    ), ranked AS ( \
        SELECT session_id, text, \
               ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY seq DESC, ord DESC) AS rn \
        FROM texts \
        WHERE typeof(text) = 'text' AND length(trim(text)) > 0 \
    ) \
    SELECT session_id, text FROM ranked WHERE rn = 1";
    let Ok(mut statement) = connection.prepare(sql) else {
        return out;
    };
    let Ok(rows) = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return out;
    };
    for (session_id, text) in rows.flatten() {
        out.insert(session_id, text);
    }
    out
}

/// True when the title contains an openCode-convention subagent marker:
/// `(@<name> subagent)`.  Known variants are `@explore` and `@general`;
/// the pattern matches any `(@`-prefixed name followed by ` subagent)` to
/// survive subagent-type additions without code changes.
fn title_contains_subagent_pattern(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    // Fast path: known fixed patterns.
    if lower.contains("(@explore subagent)") || lower.contains("(@general subagent)") {
        return true;
    }
    // Catch any future `(@<name> subagent)` variant.
    lower.contains("(@") && lower.contains(" subagent)")
}

/// Classify an openCode session as human-driven or a subagent worker.
///
/// Prefers the dedicated `kind` column when the schema carries it.
/// Falls back to a title-pattern heuristic when only `parent_id` is
/// available (e.g. `(@explore subagent)` / `(@general subagent)`).
fn classify_session_kind(
    raw_kind: Option<&str>,
    parent_id: Option<&str>,
    title: Option<&str>,
) -> Option<SessionKind> {
    if let Some(kind) = raw_kind {
        return match kind {
            "subagent" => Some(SessionKind::Subagent),
            _ => Some(SessionKind::Human),
        };
    }

    if parent_id.is_some()
        && let Some(title) = title
        && title_contains_subagent_pattern(title)
    {
        return Some(SessionKind::Subagent);
    }

    None
}

/// The operation a parent pointer records, in ADR 0018 / ADR 0048
/// lineage vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LineageOp {
    /// 1.x `session.parent_id`: a tree-shape pointer without a documented
    /// operation type, so `unknown` is the honest label.
    Parent,
    /// 2.x `session_v2.parent_id`, set only by the `subagent` tool.
    Spawn,
    /// 2.x `session_v2.fork_session_id`.
    Fork,
}

impl LineageOp {
    fn kind(self) -> &'static str {
        match self {
            LineageOp::Parent => "unknown",
            LineageOp::Spawn => "spawn",
            LineageOp::Fork => "fork",
        }
    }

    fn evidence(self) -> &'static str {
        match self {
            LineageOp::Parent => "opencode session.parent_id unknown",
            LineageOp::Spawn => "opencode session_v2.parent_id spawn",
            LineageOp::Fork => "opencode session_v2.fork_session_id fork",
        }
    }

    fn link_segment(self) -> &'static str {
        match self {
            LineageOp::Parent | LineageOp::Spawn => "parent_session",
            LineageOp::Fork => "fork_session",
        }
    }
}

fn build_lineage_link(
    child_session_key: &str,
    parent_session_key: &str,
    op: LineageOp,
    state_scope: &str,
    resolved_parent: Option<&str>,
) -> GraphLink {
    let lineage_kind = op.kind();
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
            format!("opencode:lineage:{child_session_key}:{segment}:{parent_key}"),
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
                "opencode:lineage:{child_session_key}:{segment}:unresolved:{parent_session_key}"
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

fn read_legacy_sessions(state_root: &Path) -> Result<Vec<SessionInfo>> {
    let sessions = state_root.join("storage").join("session");

    if !sessions.exists() {
        return Ok(Vec::new());
    }

    let mut infos = Vec::new();

    for entry in fs::read_dir(&sessions)? {
        let session_dir = entry?.path();

        if !session_dir.is_dir() {
            continue;
        }

        let info_path = session_dir.join("info.json");

        if let Some(info) = read_info(&info_path) {
            infos.push(info);
        }
    }

    Ok(infos)
}

fn read_info(path: &Path) -> Option<SessionInfo> {
    let body = fs::read_to_string(path).ok()?;
    let mut info: SessionInfo = serde_json::from_str(&body).ok()?;
    if info.last_active_epoch.is_none() {
        info.last_active_epoch = info
            .time
            .as_ref()
            .and_then(|time| epoch_ms_to_seconds(time.updated.or(time.created)));
    }
    Some(info)
}

// ---------------------------------------------------------------------------
// opencode.db scan cache.
// ---------------------------------------------------------------------------
//
// `read_sqlite_sessions` opens `~/.local/share/opencode/opencode.db`
// (25 MB on the operator's box) and issues a full-table `SELECT`
// scan of the session tables (plus a preview lookup per session).
// The DB uses WAL mode, so commits land in `opencode.db-wal` until a
// checkpoint; the cache key stamps both files. On a warm daemon with
// an idle opencode neither changes and the cache hit rate is ~100%,
// saving the ~5,000 pread64 pages per full-table scan every harness
// cycle.

fn read_sqlite_sessions_cached(
    path: &Path,
    cache: &StampedMap<SqliteStamp, Vec<SessionInfo>>,
) -> Vec<SessionInfo> {
    let Some(stamp) = SqliteStamp::of(path) else {
        return Vec::new();
    };
    if let Some(sessions) = cache.get(path, &stamp) {
        return sessions;
    }
    let sessions = read_sqlite_sessions(path);
    cache.insert(path.to_path_buf(), stamp, sessions.clone());
    sessions
}

#[cfg(test)]
#[path = "opencode_tests.rs"]
mod tests;
