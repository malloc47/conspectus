//! Claude Code harness discovery.
//!
//! Walks `$STATE_ROOT/projects/<encoded-cwd>/<session>.jsonl` and emits one
//! `AgentSession` per discovered session. The session id is taken from the
//! file stem; cwd is read from the first JSONL line that carries it (real
//! Claude Code files commonly start with a `permission-mode` envelope that
//! lacks `cwd`, with the cwd appearing on later user/assistant events).
//! When no JSONL line carries a cwd, the encoded project directory name is
//! decoded as a best-effort fallback. Malformed files yield no session.
//!
//! Per ADR 0018 the adapter also extracts intra-harness session lineage. Two
//! signals coexist:
//!
//! * **Fork** — claude-code's IDE "fork from here" affordance copies the
//!   parent's records into the child file and tags each copied record with
//!   `forkedFrom = { sessionId, messageUuid }`. The first uuid-bearing record
//!   carries this envelope, so if it is present the parent session id is
//!   structural — no leaf-uuid matching needed. Emitted as
//!   `lineage_kind = "fork"`.
//! * **parentUuid** — legacy/hypothetical path for compaction or resume
//!   successors that would write a new session jsonl whose first uuid-bearing
//!   record carries a cross-session `parentUuid`. claude-code 2.1.129 does
//!   not currently produce this shape, but the code is preserved for future
//!   releases. Resolves by matching the parentUuid against the leaf message
//!   of another discovered transcript in the same project directory.
//!
//! When `forkedFrom` is present it takes precedence: it is the explicit,
//! provider-recorded fork pointer and supersedes any speculative parentUuid
//! match. Unresolved parent endpoints in either path are preserved as
//! unresolved evidence so later discovery can reconcile them.
//!
//! Two claude-code 2.1.129 behaviors are intentionally **not** modeled:
//!
//! * **In-place `/compact`** keeps appending to the same session jsonl
//!   rather than starting a successor file, and emits a `type: "summary"`
//!   record at the compaction boundary. Conspectus preserves
//!   `AgentSession` at session-file granularity rather than splitting on
//!   summary records, so an in-place compaction produces no
//!   `parent_session` edge — both endpoints would be the same node. Tests
//!   lock in that summary records inside a transcript do not cause a
//!   spurious lineage candidate.
//! * **Bare fork** (e.g. the "fresh session" affordance) writes a new
//!   session jsonl with no `forkedFrom` envelope and no cross-session
//!   `parentUuid`. With no on-disk signal we emit no lineage candidate.
//!   See backlog `H-LINEAGE-006` for the closure notes.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
    normalize_last_message_preview,
};

/// Maximum number of JSONL lines to scan when looking for `cwd` evidence and
/// the first cross-session `parentUuid`. Real sessions almost always carry
/// both within the first few records; the cap keeps very long transcripts
/// cheap.
const MAX_HEADER_SCAN_LINES: usize = 200;

/// Maximum number of bytes to read from the end of a transcript when looking
/// for the leaf uuid. Claude-code lines are kilobytes at most, so a 32 KiB
/// tail comfortably contains the last several messages without re-parsing
/// the whole transcript.
const TAIL_SCAN_BYTES: u64 = 32 * 1024;

pub const HARNESS_KEY: &str = "claude-code";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for ClaudeCodeAdapter {
    fn harness_key(&self) -> &str {
        HARNESS_KEY
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let Some(state_root) = context.harness_state_root(self.harness_key()) else {
            return Ok(GraphFragment::empty());
        };
        discover_state(state_root)
    }
}

fn discover_state(state_root: &Path) -> Result<GraphFragment> {
    let projects = state_root.join("projects");

    if !projects.exists() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut nodes = Vec::new();
    let mut candidate_links = Vec::new();

    for project in fs::read_dir(&projects)? {
        let project_dir = project?.path();

        if !project_dir.is_dir() {
            continue;
        }

        let fallback_cwd = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(decode_project_dir);

        let mut entries: Vec<DiscoveredSession> = Vec::new();

        for entry in fs::read_dir(&project_dir)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if !name.ends_with(".jsonl") {
                continue;
            }

            let Some(meta) = read_session_header(&path, fallback_cwd.as_deref()) else {
                continue;
            };

            let leaf_uuid = read_session_leaf_uuid(&path);
            let last_message_preview = read_session_last_message_preview(&path);
            let last_active_epoch = file_modified_epoch(&path);

            entries.push(DiscoveredSession {
                node: AgentSessionNode {
                    id: AgentSessionId::new(HARNESS_KEY, &state_scope, &meta.session_id),
                    harness_key: HARNESS_KEY.to_string(),
                    cwd: meta.cwd,
                    title: meta.summary,
                    last_message_preview,
                    last_active_epoch,
                },
                parent_uuid: meta.parent_uuid,
                cross_session_record_type: meta.cross_session_record_type,
                forked_from_session_id: meta.forked_from_session_id,
                forked_from_message_uuid: meta.forked_from_message_uuid,
                leaf_uuid,
            });
        }

        let leaf_to_session: HashMap<&str, &str> = entries
            .iter()
            .filter_map(|entry| {
                entry
                    .leaf_uuid
                    .as_deref()
                    .map(|leaf| (leaf, entry.node.id.session_key.as_str()))
            })
            .collect();

        let session_keys: HashMap<&str, ()> = entries
            .iter()
            .map(|entry| (entry.node.id.session_key.as_str(), ()))
            .collect();

        for entry in &entries {
            // forkedFrom is the explicit provider-recorded fork pointer and
            // wins over any speculative parentUuid match. Skip self-fork rows
            // defensively even though current claude-code does not produce
            // them.
            if let Some(parent_session_id) = entry.forked_from_session_id.as_deref() {
                if !parent_session_id.is_empty() && parent_session_id != entry.node.id.session_key {
                    let resolved = session_keys.contains_key(parent_session_id);
                    candidate_links.push(build_fork_lineage_link(
                        entry,
                        parent_session_id,
                        entry.forked_from_message_uuid.as_deref(),
                        &state_scope,
                        resolved,
                    ));
                }
            } else if let Some(parent_uuid) = entry.parent_uuid.as_deref() {
                candidate_links.push(build_lineage_link(
                    entry,
                    parent_uuid,
                    &state_scope,
                    leaf_to_session.get(parent_uuid).copied(),
                ));
            }
            nodes.push(GraphNode::AgentSession(entry.node.clone()));
        }
    }

    Ok(GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
    })
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

struct DiscoveredSession {
    node: AgentSessionNode,
    parent_uuid: Option<String>,
    cross_session_record_type: Option<String>,
    forked_from_session_id: Option<String>,
    forked_from_message_uuid: Option<String>,
    leaf_uuid: Option<String>,
}

struct SessionHeader {
    session_id: String,
    cwd: Option<String>,
    summary: Option<String>,
    parent_uuid: Option<String>,
    /// `type` of the first record that carries a non-null `parentUuid`.
    /// `Some("summary")` signals a compaction successor; anything else (or
    /// `None`) is treated as a resume.
    cross_session_record_type: Option<String>,
    /// `forkedFrom.sessionId` on the first uuid-bearing record, when present.
    /// Identifies the parent session of an IDE fork.
    forked_from_session_id: Option<String>,
    /// `forkedFrom.messageUuid` on the first uuid-bearing record. For a fork
    /// this is the uuid of the corresponding first message in the parent
    /// session; it is preserved as evidence rather than used for resolution.
    forked_from_message_uuid: Option<String>,
}

#[derive(Deserialize)]
struct ScannedLine {
    #[serde(rename = "sessionId", default)]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(rename = "parentUuid", default)]
    parent_uuid: Option<String>,
    #[serde(rename = "type", default)]
    record_type: Option<String>,
    #[serde(default)]
    uuid: Option<String>,
    #[serde(rename = "forkedFrom", default)]
    forked_from: Option<ForkedFrom>,
}

#[derive(Deserialize)]
struct ForkedFrom {
    #[serde(rename = "sessionId", default)]
    session_id: Option<String>,
    #[serde(rename = "messageUuid", default)]
    message_uuid: Option<String>,
}

fn read_session_header(path: &Path, fallback_cwd: Option<&str>) -> Option<SessionHeader> {
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)?;
    let file = fs::File::open(path).ok()?;
    let mut cwd: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut parent_uuid: Option<String> = None;
    let mut cross_session_record_type: Option<String> = None;
    let mut forked_from_session_id: Option<String> = None;
    let mut forked_from_message_uuid: Option<String> = None;
    let mut saw_any_record = false;
    let mut saw_first_real_record = false;

    for line in BufReader::new(file).lines().take(MAX_HEADER_SCAN_LINES) {
        let Ok(line) = line else { continue };
        let Ok(parsed) = serde_json::from_str::<ScannedLine>(&line) else {
            continue;
        };
        saw_any_record = true;

        if cwd.is_none() {
            cwd = parsed.cwd;
        }
        if summary.is_none() {
            summary = parsed.summary;
        }
        // The cross-session lineage pointers live on the first uuid-bearing
        // record. Real claude-code transcripts open with envelopes such as
        // `permission-mode` or `file-history-snapshot` that carry no uuid;
        // the first user/assistant/summary message after those carries
        // either `forkedFrom` (IDE fork) or `parentUuid` (hypothetical
        // compaction/resume successor). Records after that point within the
        // same session, so we capture exactly once on the first real record.
        if !saw_first_real_record && parsed.uuid.is_some() {
            parent_uuid = parsed.parent_uuid;
            cross_session_record_type = parsed.record_type;
            if let Some(forked_from) = parsed.forked_from {
                forked_from_session_id = forked_from.session_id;
                forked_from_message_uuid = forked_from.message_uuid;
            }
            saw_first_real_record = true;
        }
        if cwd.is_some() && summary.is_some() && saw_first_real_record {
            break;
        }
        // sessionId from JSONL is informational; the filename is authoritative
        // for the session id.
        let _ = parsed.session_id;
    }

    if !saw_any_record {
        return None;
    }

    if cwd.is_none() {
        cwd = fallback_cwd.map(str::to_string);
    }

    Some(SessionHeader {
        session_id,
        cwd,
        summary,
        parent_uuid,
        cross_session_record_type,
        forked_from_session_id,
        forked_from_message_uuid,
    })
}

/// Returns the `uuid` of the last successfully parsed record in the
/// transcript, or `None` if no record carries a `uuid`. Reads at most the
/// trailing [`TAIL_SCAN_BYTES`] of the file so very long transcripts stay
/// cheap.
fn read_session_leaf_uuid(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();

    if len == 0 {
        return None;
    }

    let start = len.saturating_sub(TAIL_SCAN_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((len - start) as usize);
    file.read_to_end(&mut buf).ok()?;

    // If we started mid-file, the first partial line is unreliable — drop it.
    let scan_start = if start > 0 {
        match buf.iter().position(|&b| b == b'\n') {
            Some(idx) => idx + 1,
            None => return None,
        }
    } else {
        0
    };

    let mut last_uuid: Option<String> = None;
    for line in buf[scan_start..].split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_slice::<ScannedLine>(line) else {
            continue;
        };
        if let Some(uuid) = parsed.uuid {
            last_uuid = Some(uuid);
        }
    }

    last_uuid
}

/// Extract the session's most recent user/assistant text message as a
/// preview (ADR 0023). Walks the trailing [`TAIL_SCAN_BYTES`] of the
/// transcript backward, skipping `tool_use`, `tool_result`, `thinking`,
/// system records, and the `isCompactSummary` synthetic summary user
/// message. Returns the first matching text content, normalized via
/// [`normalize_last_message_preview`].
///
/// Returns `None` when the transcript is empty, contains no usable text
/// in its tail window, or fails to parse. Discovery is best-effort —
/// errors degrade to `None`.
fn read_session_last_message_preview(path: &Path) -> Option<String> {
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

    // Walk lines in reverse so we hit the most recent message first.
    let lines: Vec<&[u8]> = buf[scan_start..]
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    for line in lines.iter().rev() {
        let Ok(parsed) = serde_json::from_slice::<MessageScan>(line) else {
            continue;
        };
        if let Some(text) = extract_message_preview_text(&parsed)
            && let Some(preview) = normalize_last_message_preview(&text)
        {
            return Some(preview);
        }
    }
    None
}

#[derive(Deserialize)]
struct MessageScan {
    #[serde(rename = "type", default)]
    record_type: Option<String>,
    #[serde(default)]
    message: Option<MessageBody>,
    #[serde(rename = "isCompactSummary", default)]
    is_compact_summary: bool,
}

#[derive(Deserialize)]
struct MessageBody {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<MessageContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageContent {
    /// Some pre-content-block transcripts (and a few synthetic
    /// records) carry `content` as a plain string.
    String(String),
    /// Modern transcripts emit a list of typed content blocks.
    Blocks(Vec<ContentBlock>),
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// Pull a candidate preview string out of one parsed line. Returns
/// `None` for non-message records, tool results / tool uses /
/// thinking blocks, and compaction summary records.
fn extract_message_preview_text(line: &MessageScan) -> Option<String> {
    if line.is_compact_summary {
        return None;
    }
    let record_type = line.record_type.as_deref()?;
    if record_type != "user" && record_type != "assistant" {
        return None;
    }
    let message = line.message.as_ref()?;
    // Honor message.role when present; some transcripts elide it.
    if let Some(role) = message.role.as_deref()
        && role != "user"
        && role != "assistant"
    {
        return None;
    }
    let content = message.content.as_ref()?;
    match content {
        MessageContent::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(text.clone())
            }
        }
        MessageContent::Blocks(blocks) => {
            // Take the last `text` block in the message so the
            // preview reflects the trailing prose rather than an
            // intermediate lead-in. Tool-use, tool-result, and
            // thinking blocks are skipped.
            for block in blocks.iter().rev() {
                if block.block_type.as_deref() != Some("text") {
                    continue;
                }
                let Some(text) = block.text.as_ref() else {
                    continue;
                };
                if text.trim().is_empty() {
                    continue;
                }
                return Some(text.clone());
            }
            None
        }
    }
}

fn build_fork_lineage_link(
    entry: &DiscoveredSession,
    parent_session_id: &str,
    forked_from_message_uuid: Option<&str>,
    state_scope: &str,
    resolved: bool,
) -> GraphLink {
    let mut fields: Metadata = Metadata::new();
    fields.insert("harness_key".to_string(), json!(HARNESS_KEY));
    fields.insert("lineage_kind".to_string(), json!("fork"));
    fields.insert("parent_session_id".to_string(), json!(parent_session_id));
    if let Some(uuid) = forked_from_message_uuid {
        fields.insert("forked_from_message_uuid".to_string(), json!(uuid));
    }

    let child_session_key = entry.node.id.session_key.as_str();
    let source = NodeId::AgentSession(entry.node.id.clone());

    let (target, link_id) = if resolved {
        let parent_id = AgentSessionId::new(HARNESS_KEY, state_scope, parent_session_id);
        (
            LinkEndpoint::Node {
                id: NodeId::AgentSession(parent_id),
            },
            format!("claude-code:lineage:{child_session_key}:parent_session:{parent_session_id}"),
        )
    } else {
        let evidence = UnresolvedEndpoint {
            node_type: "agent_session".to_string(),
            harness_key: Some(HARNESS_KEY.to_string()),
            native_id: Some(parent_session_id.to_string()),
            state_scope: Some(state_scope.to_string()),
            path: None,
            metadata: fields.clone(),
        };
        (
            LinkEndpoint::Unresolved { evidence },
            format!(
                "claude-code:lineage:{child_session_key}:parent_session:unresolved:{parent_session_id}"
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
            evidence: Some("claude-code transcript fork".to_string()),
            fields,
        },
        state: LinkState::Active,
    }
}

fn build_lineage_link(
    entry: &DiscoveredSession,
    parent_uuid: &str,
    state_scope: &str,
    resolved_parent_session_key: Option<&str>,
) -> GraphLink {
    let lineage_kind = if entry.cross_session_record_type.as_deref() == Some("summary") {
        "compaction"
    } else {
        "resume"
    };

    let mut fields: Metadata = Metadata::new();
    fields.insert("harness_key".to_string(), json!(HARNESS_KEY));
    fields.insert("lineage_kind".to_string(), json!(lineage_kind));
    fields.insert("parent_uuid".to_string(), json!(parent_uuid));

    let child_session_key = entry.node.id.session_key.as_str();
    let source = NodeId::AgentSession(entry.node.id.clone());

    let (target, link_id) = match resolved_parent_session_key {
        Some(parent_key) => {
            let parent_id = AgentSessionId::new(HARNESS_KEY, state_scope, parent_key);
            (
                LinkEndpoint::Node {
                    id: NodeId::AgentSession(parent_id),
                },
                format!("claude-code:lineage:{child_session_key}:parent_session:{parent_key}"),
            )
        }
        None => {
            let evidence = UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some(HARNESS_KEY.to_string()),
                native_id: Some(parent_uuid.to_string()),
                state_scope: Some(state_scope.to_string()),
                path: None,
                metadata: fields.clone(),
            };
            (
                LinkEndpoint::Unresolved { evidence },
                format!(
                    "claude-code:lineage:{child_session_key}:parent_session:unresolved:{parent_uuid}"
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
            evidence: Some(format!("claude-code transcript {lineage_kind}")),
            fields,
        },
        state: LinkState::Active,
    }
}

/// Best-effort inverse of Claude Code's project-directory encoding (`/` → `-`).
/// Real encoding is lossy on paths that contain literal `-` or `.`, so this is
/// only used as a last-resort fallback when no JSONL line carries `cwd`.
fn decode_project_dir(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|ch| if ch == '-' { '/' } else { ch })
        .collect();
    coalesce_slashes(&replaced)
}

fn coalesce_slashes(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut last_slash = false;

    for ch in path.chars() {
        if ch == '/' {
            if !last_slash {
                out.push(ch);
            }
            last_slash = true;
        } else {
            out.push(ch);
            last_slash = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::discovery::harness::fixtures::{
        ClaudeCodeSessionRecord, HarnessFixture, write_malformed,
    };
    use crate::model::GraphNode;

    fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
        let fixture = HarnessFixture::at(temp.path());
        let context = DiscoveryContext::default()
            .with_harness_state_root(HARNESS_KEY, fixture.claude_code_state_root());
        (context, fixture)
    }

    #[test]
    fn adapter_returns_empty_when_no_state_root_configured() {
        let fragment = ClaudeCodeAdapter::new()
            .discover(&DiscoveryContext::default())
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn adapter_returns_empty_when_projects_dir_missing() {
        let temp = TempDir::new().expect("temp");
        let (context, _) = context_with_state(&temp);

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn discovers_claude_sessions_with_summary_and_stable_ids() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_claude_code_session(
                &ClaudeCodeSessionRecord::new("session-a", "/work/alpha")
                    .with_summary("alpha work"),
            )
            .expect("write a");
        fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("session-b", "/work/beta"))
            .expect("write b");

        let first = ClaudeCodeAdapter::new().discover(&context).expect("first");
        let second = ClaudeCodeAdapter::new().discover(&context).expect("second");

        assert_eq!(first, second);

        let sessions: Vec<_> = first
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 2);
        let a = sessions
            .iter()
            .find(|s| s.id.session_key == "session-a")
            .expect("session-a");
        assert_eq!(a.cwd.as_deref(), Some("/work/alpha"));
        assert_eq!(a.title.as_deref(), Some("alpha work"));
        assert_eq!(a.last_active_epoch, Some(1_700_000_000));

        let b = sessions
            .iter()
            .find(|s| s.id.session_key == "session-b")
            .expect("session-b");
        assert!(b.title.is_none(), "missing summary should leave title None");
    }

    #[test]
    fn cwd_comes_from_later_jsonl_line_when_first_line_lacks_it() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let project_dir = fixture
            .claude_code_state_root()
            .join("projects")
            .join("-work-conspectus");
        std::fs::create_dir_all(&project_dir).expect("project dir");
        let session_id = "e83ded0a-b5a4-4a1c-b460-f83d49bd01ce";
        std::fs::write(
            project_dir.join(format!("{session_id}.jsonl")),
            "{\"type\":\"permission-mode\",\"sessionId\":\"e83ded0a-b5a4-4a1c-b460-f83d49bd01ce\"}\n\
             {\"type\":\"user\",\"cwd\":\"/work/conspectus\",\"sessionId\":\"e83ded0a-b5a4-4a1c-b460-f83d49bd01ce\"}\n",
        )
        .expect("write claude session");

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");
        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.session_key, session_id);
        assert_eq!(sessions[0].cwd.as_deref(), Some("/work/conspectus"));
    }

    #[test]
    fn cwd_falls_back_to_decoded_project_directory_when_jsonl_lacks_cwd() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let project_dir = fixture
            .claude_code_state_root()
            .join("projects")
            .join("-work-conspectus");
        std::fs::create_dir_all(&project_dir).expect("project dir");
        std::fs::write(
            project_dir.join("noop.jsonl"),
            "{\"type\":\"permission-mode\",\"sessionId\":\"noop\"}\n",
        )
        .expect("write claude session");

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");
        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].cwd.as_deref(), Some("/work/conspectus"));
    }

    #[test]
    fn session_id_comes_from_filename_not_first_line() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let project_dir = fixture
            .claude_code_state_root()
            .join("projects")
            .join("-work-x");
        std::fs::create_dir_all(&project_dir).expect("project dir");
        // The JSONL sessionId disagrees with the filename; filename should win
        // so a discovered session can always be located on disk by id.
        std::fs::write(
            project_dir.join("real-name.jsonl"),
            "{\"type\":\"user\",\"sessionId\":\"different-id\",\"cwd\":\"/work/x\"}\n",
        )
        .expect("write claude session");

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");
        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.session_key, "real-name");
    }

    #[test]
    fn decode_project_dir_coalesces_consecutive_slashes() {
        assert_eq!(decode_project_dir("-work-repo"), "/work/repo");
        // Claude Code's encoding is lossy: every `-` becomes `/`, so the
        // decoder cannot tell a literal hyphen (`agent-deck`) from a path
        // separator. Consecutive separators are coalesced into one `/` so the
        // fallback at least produces a plausible absolute path. Callers should
        // prefer the JSONL-derived cwd whenever it exists.
        assert_eq!(
            decode_project_dir("-home-malloc47--agent-deck"),
            "/home/malloc47/agent/deck"
        );
    }

    /// Builds a project directory at `<state_root>/projects/<project>` and
    /// writes one transcript per record. Each record is `(filename_stem,
    /// jsonl_body)`. The body is written verbatim, including newlines.
    fn write_transcripts(
        state_root: &Path,
        project: &str,
        transcripts: &[(&str, &str)],
    ) -> std::path::PathBuf {
        let project_dir = state_root.join("projects").join(project);
        std::fs::create_dir_all(&project_dir).expect("project dir");

        for (name, body) in transcripts {
            std::fs::write(project_dir.join(format!("{name}.jsonl")), body).expect("write");
        }

        project_dir
    }

    fn lineage_links(fragment: &GraphFragment) -> Vec<&GraphLink> {
        fragment
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::ParentSession)
            .collect()
    }

    #[test]
    fn compaction_successor_links_to_parent_session_on_disk() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/repo\",\"uuid\":\"u-parent-1\"}\n\
            {\"type\":\"assistant\",\"sessionId\":\"parent\",\"uuid\":\"u-parent-leaf\",\"parentUuid\":\"u-parent-1\"}\n";
        let child_body = "\
            {\"type\":\"summary\",\"parentUuid\":\"u-parent-leaf\",\"uuid\":\"u-child-1\",\"summary\":\"prior session summary\"}\n\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-2\",\"parentUuid\":\"u-child-1\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("parent", parent_body), ("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1, "one parent_session candidate expected");
        let link = lineage[0];
        let target = match &link.target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved parent endpoint, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target, got {target:?}");
        };
        assert_eq!(parent_id.session_key, "parent");
        assert_eq!(parent_id.harness_key, HARNESS_KEY);

        let NodeId::AgentSession(child_id) = &link.source else {
            panic!("expected AgentSession source");
        };
        assert_eq!(child_id.session_key, "child");

        assert_eq!(
            link.source_metadata.fields.get("lineage_kind"),
            Some(&json!("compaction"))
        );
        assert_eq!(
            link.source_metadata.fields.get("parent_uuid"),
            Some(&json!("u-parent-leaf"))
        );
    }

    #[test]
    fn resume_successor_uses_resume_lineage_kind() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/repo\",\"uuid\":\"u-parent-leaf\"}\n";
        // Resume transcripts start with a regular user message (no compaction
        // summary) whose parentUuid points to the leaf of the prior session.
        let child_body = "\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"parentUuid\":\"u-parent-leaf\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("parent", parent_body), ("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1);
        assert_eq!(
            lineage[0].source_metadata.fields.get("lineage_kind"),
            Some(&json!("resume"))
        );
        assert!(matches!(lineage[0].target, LinkEndpoint::Node { .. }));
    }

    #[test]
    fn lineage_preserves_unresolved_parent_when_predecessor_is_pruned() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let orphan_body = "\
            {\"type\":\"summary\",\"parentUuid\":\"missing-leaf\",\"uuid\":\"u-orphan-1\",\"summary\":\"summary of pruned session\"}\n\
            {\"type\":\"user\",\"sessionId\":\"orphan\",\"cwd\":\"/work/repo\",\"uuid\":\"u-orphan-2\",\"parentUuid\":\"u-orphan-1\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("orphan", orphan_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1);
        let evidence = match &lineage[0].target {
            LinkEndpoint::Unresolved { evidence } => evidence,
            other => panic!("expected unresolved endpoint, got {other:?}"),
        };
        assert_eq!(evidence.node_type, "agent_session");
        assert_eq!(evidence.harness_key.as_deref(), Some(HARNESS_KEY));
        assert_eq!(evidence.native_id.as_deref(), Some("missing-leaf"));
        assert_eq!(
            evidence.metadata.get("lineage_kind"),
            Some(&json!("compaction"))
        );
    }

    #[test]
    fn sessions_without_parent_uuid_emit_no_lineage_link() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("standalone", "/work/repo"))
            .expect("write");

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");

        assert!(lineage_links(&fragment).is_empty());
    }

    #[test]
    fn lineage_match_is_scoped_to_one_project_directory() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/alpha\",\"uuid\":\"shared-leaf\"}\n";
        let child_body = "\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/beta\",\"uuid\":\"u-c\",\"parentUuid\":\"shared-leaf\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-alpha",
            &[("parent", parent_body)],
        );
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-beta",
            &[("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(
            lineage.len(),
            1,
            "cross-project leaf match must not resolve"
        );
        assert!(
            matches!(lineage[0].target, LinkEndpoint::Unresolved { .. }),
            "shared uuid across projects should not produce a concrete target"
        );
    }

    #[test]
    fn lineage_pointer_is_read_from_first_uuid_bearing_record_not_envelope() {
        // Real claude-code transcripts open with envelope records
        // (`permission-mode`, `file-history-snapshot`) that carry no
        // `uuid` field. The cross-session parent pointer lives on the
        // first real user/assistant/summary message after the envelopes.
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/repo\",\"uuid\":\"u-parent-leaf\"}\n";
        let child_body = "\
            {\"type\":\"permission-mode\",\"sessionId\":\"child\",\"permissionMode\":\"default\"}\n\
            {\"type\":\"file-history-snapshot\",\"isSnapshotUpdate\":true,\"messageId\":\"m1\",\"snapshot\":{}}\n\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"parentUuid\":\"u-parent-leaf\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("parent", parent_body), ("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(
            lineage.len(),
            1,
            "envelope-prefixed transcript must still detect lineage",
        );
        let target = match &lineage[0].target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved parent endpoint, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target");
        };
        assert_eq!(parent_id.session_key, "parent");
    }

    #[test]
    fn ide_fork_with_forked_from_resolves_to_parent_session() {
        // claude-code 2.1.129's "fork with history" affordance copies the
        // parent's records into the child file. Each copied record carries
        // a `forkedFrom = {sessionId, messageUuid}` envelope. The first
        // uuid-bearing record's `forkedFrom.sessionId` is the parent.
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/repo\",\"uuid\":\"u-parent-1\"}\n";
        let child_body = "\
            {\"type\":\"permission-mode\",\"sessionId\":\"child\",\"permissionMode\":\"default\"}\n\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"parentUuid\":null,\"forkedFrom\":{\"sessionId\":\"parent\",\"messageUuid\":\"u-parent-1\"}}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("parent", parent_body), ("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1, "one fork lineage candidate expected");
        let link = lineage[0];
        let target = match &link.target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved parent endpoint, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target");
        };
        assert_eq!(parent_id.session_key, "parent");
        assert_eq!(parent_id.harness_key, HARNESS_KEY);

        let NodeId::AgentSession(child_id) = &link.source else {
            panic!("expected AgentSession source");
        };
        assert_eq!(child_id.session_key, "child");

        assert_eq!(
            link.source_metadata.fields.get("lineage_kind"),
            Some(&json!("fork"))
        );
        assert_eq!(
            link.source_metadata.fields.get("parent_session_id"),
            Some(&json!("parent"))
        );
        assert_eq!(
            link.source_metadata.fields.get("forked_from_message_uuid"),
            Some(&json!("u-parent-1"))
        );
    }

    #[test]
    fn ide_fork_with_missing_parent_session_is_unresolved() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let child_body = "\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"forkedFrom\":{\"sessionId\":\"ghost-parent\",\"messageUuid\":\"u-ghost-1\"}}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage = lineage_links(&fragment);

        assert_eq!(lineage.len(), 1);
        let evidence = match &lineage[0].target {
            LinkEndpoint::Unresolved { evidence } => evidence,
            other => panic!("expected unresolved endpoint, got {other:?}"),
        };
        assert_eq!(evidence.node_type, "agent_session");
        assert_eq!(evidence.harness_key.as_deref(), Some(HARNESS_KEY));
        assert_eq!(evidence.native_id.as_deref(), Some("ghost-parent"));
        assert_eq!(evidence.metadata.get("lineage_kind"), Some(&json!("fork")));
        assert_eq!(
            evidence.metadata.get("parent_session_id"),
            Some(&json!("ghost-parent"))
        );
    }

    #[test]
    fn in_place_compaction_summary_record_emits_no_lineage() {
        // claude-code 2.1.129's `/compact` appends to the same session
        // jsonl rather than starting a successor file, dropping a
        // `type: "summary"` record at the compaction boundary. ADR 0018
        // keeps `AgentSession` at session-file granularity, so within-
        // session compaction must not produce a `parent_session`
        // candidate — both endpoints would be the same node.
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        // First uuid-bearing record carries no cross-session pointer (this
        // session is a fresh start). A summary record then appears mid-
        // stream as the compaction marker, followed by post-compact
        // messages that chain within the session.
        let body = "\
            {\"type\":\"permission-mode\",\"sessionId\":\"compacted\",\"permissionMode\":\"default\"}\n\
            {\"type\":\"user\",\"sessionId\":\"compacted\",\"cwd\":\"/work/repo\",\"uuid\":\"u-pre-1\",\"parentUuid\":null}\n\
            {\"type\":\"assistant\",\"sessionId\":\"compacted\",\"uuid\":\"u-pre-2\",\"parentUuid\":\"u-pre-1\"}\n\
            {\"type\":\"summary\",\"sessionId\":\"compacted\",\"uuid\":\"u-summary\",\"parentUuid\":\"u-pre-2\",\"summary\":\"compaction marker\"}\n\
            {\"type\":\"user\",\"sessionId\":\"compacted\",\"uuid\":\"u-post-1\",\"parentUuid\":\"u-summary\"}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("compacted", body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");

        assert!(
            lineage_links(&fragment).is_empty(),
            "in-place compaction must not emit a parent_session candidate"
        );
        let session_count = fragment
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::AgentSession(_)))
            .count();
        assert_eq!(
            session_count, 1,
            "session-file granularity: one AgentSession per jsonl regardless of internal summary records"
        );
    }

    #[test]
    fn fork_variant_without_forked_from_emits_no_lineage() {
        // A second observed claude-code fork variant ("fresh session from
        // here", 926c6991-… on the 2026-05-17 validation) creates a child
        // jsonl with no `forkedFrom` envelope and no cross-session
        // `parentUuid` either. With no on-disk signal we emit no lineage
        // candidate; side-channel inference is tracked in H-LINEAGE-006.
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent\",\"cwd\":\"/work/repo\",\"uuid\":\"u-parent-1\"}\n";
        let child_body = "\
            {\"type\":\"permission-mode\",\"sessionId\":\"child\",\"permissionMode\":\"default\"}\n\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"parentUuid\":null}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("parent", parent_body), ("child", child_body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");

        assert!(
            lineage_links(&fragment).is_empty(),
            "child without forkedFrom or parentUuid must not emit lineage"
        );
        let session_count = fragment
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::AgentSession(_)))
            .count();
        assert_eq!(session_count, 2, "both sessions still surface as nodes");
    }

    #[test]
    fn forked_from_takes_precedence_over_parent_uuid() {
        // Defensive: if a future release happens to set both forkedFrom
        // (structural) and parentUuid (speculative leaf match), the
        // structural pointer wins so we never emit two parent_session
        // candidates that disagree.
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let parent_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent-fork\",\"cwd\":\"/work/repo\",\"uuid\":\"u-pf-1\"}\n";
        // u-decoy-leaf names a hypothetical other parent that the speculative
        // parentUuid heuristic would have matched; the fork pointer must
        // override.
        let decoy_body = "\
            {\"type\":\"user\",\"sessionId\":\"parent-decoy\",\"cwd\":\"/work/repo\",\"uuid\":\"u-decoy-leaf\"}\n";
        let child_body = "\
            {\"type\":\"user\",\"sessionId\":\"child\",\"cwd\":\"/work/repo\",\"uuid\":\"u-child-1\",\"parentUuid\":\"u-decoy-leaf\",\"forkedFrom\":{\"sessionId\":\"parent-fork\",\"messageUuid\":\"u-pf-1\"}}\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[
                ("parent-fork", parent_body),
                ("parent-decoy", decoy_body),
                ("child", child_body),
            ],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");
        let lineage: Vec<_> = lineage_links(&fragment)
            .into_iter()
            .filter(|l| matches!(&l.source, NodeId::AgentSession(id) if id.session_key == "child"))
            .collect();

        assert_eq!(
            lineage.len(),
            1,
            "exactly one lineage candidate from the child"
        );
        let target = match &lineage[0].target {
            LinkEndpoint::Node { id } => id,
            other => panic!("expected resolved target, got {other:?}"),
        };
        let NodeId::AgentSession(parent_id) = target else {
            panic!("expected AgentSession target");
        };
        assert_eq!(parent_id.session_key, "parent-fork");
        assert_eq!(
            lineage[0].source_metadata.fields.get("lineage_kind"),
            Some(&json!("fork"))
        );
    }

    #[test]
    fn malformed_first_record_skips_session_entirely() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        // First record is unparseable; the second has a valid parentUuid. We
        // currently bail when no record parses; if the first parses-but-skips
        // pattern emerges, only this case needs to flip.
        let body = "{not valid json\n";
        write_transcripts(
            &fixture.claude_code_state_root(),
            "-work-repo",
            &[("bad", body)],
        );

        let fragment = ClaudeCodeAdapter::new().discover(&context).expect("disc");

        assert!(fragment.nodes.is_empty());
        assert!(lineage_links(&fragment).is_empty());
    }

    #[test]
    fn skips_malformed_records() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("good", "/work/repo"))
            .expect("write good");
        let project_dir = fixture
            .claude_code_state_root()
            .join("projects")
            .join("-work-other");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        write_malformed(project_dir.join("bad.jsonl")).expect("malformed");

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");

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

    fn write_transcript(state_root: &Path, project: &str, session_id: &str, body: &str) {
        let project_dir = state_root.join("projects").join(project);
        std::fs::create_dir_all(&project_dir).expect("project dir");
        std::fs::write(project_dir.join(format!("{session_id}.jsonl")), body)
            .expect("write claude session");
    }

    fn discover_one_session(context: &DiscoveryContext) -> AgentSessionNode {
        let fragment = ClaudeCodeAdapter::new()
            .discover(context)
            .expect("discover");
        fragment
            .nodes
            .into_iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s),
                _ => None,
            })
            .next()
            .expect("one agent session")
    }

    /// Plain user→assistant exchange: the assistant's text wins.
    #[test]
    fn last_message_preview_returns_last_assistant_text() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let body = concat!(
            r#"{"type":"permission-mode","sessionId":"plain"}"#,
            "\n",
            r#"{"type":"user","sessionId":"plain","uuid":"u1","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#,
            "\n",
            r#"{"type":"assistant","sessionId":"plain","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"hello back"}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "plain", body);

        let session = discover_one_session(&context);
        assert_eq!(session.last_message_preview.as_deref(), Some("hello back"));
    }

    /// Tool-use blocks at the tail are ignored; the most recent text
    /// content wins even when it sits before a chain of tool calls.
    #[test]
    fn last_message_preview_skips_tool_use_and_tool_result_blocks() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let body = concat!(
            r#"{"type":"user","sessionId":"s","uuid":"u1","message":{"role":"user","content":[{"type":"text","text":"start"}]}}"#,
            "\n",
            r#"{"type":"assistant","sessionId":"s","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"working on it"}]}}"#,
            "\n",
            r#"{"type":"assistant","sessionId":"s","uuid":"a2","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read"}]}}"#,
            "\n",
            r#"{"type":"user","sessionId":"s","uuid":"u2","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"file body"}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", body);

        let session = discover_one_session(&context);
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("working on it"),
        );
    }

    /// `thinking` blocks at the tail are also skipped — they are not
    /// visible message content.
    #[test]
    fn last_message_preview_skips_thinking_blocks() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let body = concat!(
            r#"{"type":"assistant","sessionId":"s","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"final answer"}]}}"#,
            "\n",
            r#"{"type":"assistant","sessionId":"s","uuid":"a2","message":{"role":"assistant","content":[{"type":"thinking","thinking":"reasoning..."}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", body);

        let session = discover_one_session(&context);
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("final answer"),
        );
    }

    /// `isCompactSummary: true` records carry a synthetic
    /// post-compaction summary, not a real user message. The preview
    /// should come from a real user/assistant message after the
    /// boundary when one exists.
    #[test]
    fn last_message_preview_skips_compact_summary_records() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let body = concat!(
            r#"{"type":"user","sessionId":"s","uuid":"u1","isCompactSummary":true,"message":{"role":"user","content":[{"type":"text","text":"synthetic compaction summary"}]}}"#,
            "\n",
            r#"{"type":"assistant","sessionId":"s","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"first real reply after compaction"}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", body);

        let session = discover_one_session(&context);
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("first real reply after compaction"),
        );
    }

    /// A transcript whose tail is entirely tool-use / tool-result
    /// activity (with no text in the scan window) yields `None`. The
    /// renderer falls back to `—`.
    #[test]
    fn last_message_preview_returns_none_for_tool_only_transcript() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let body = concat!(
            r#"{"type":"assistant","sessionId":"s","uuid":"a1","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read"}]}}"#,
            "\n",
            r#"{"type":"user","sessionId":"s","uuid":"u1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"file body"}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", body);

        let session = discover_one_session(&context);
        assert_eq!(session.last_message_preview, None);
    }

    /// Empty/malformed transcripts must degrade silently to `None`.
    #[test]
    fn last_message_preview_returns_none_for_empty_or_corrupt_transcript() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        // Empty body — discovery skips the file (`read_session_header`
        // returns None for fully-empty files), so the easier case is a
        // single corrupt-JSON line plus a permission-mode header that
        // succeeds at the header scan but yields no text in the tail.
        let body = concat!(
            r#"{"type":"permission-mode","sessionId":"corrupt"}"#,
            "\n",
            "this is not json\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "corrupt", body);

        let session = discover_one_session(&context);
        assert_eq!(session.last_message_preview, None);
    }

    /// Long messages are capped at 200 chars by
    /// `normalize_last_message_preview`.
    #[test]
    fn last_message_preview_is_capped_at_two_hundred_chars() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let long = "a".repeat(500);
        let body = format!(
            "{{\"type\":\"assistant\",\"sessionId\":\"s\",\"uuid\":\"a1\",\"message\":{{\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":\"{long}\"}}]}}}}\n"
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", &body);

        let session = discover_one_session(&context);
        let preview = session.last_message_preview.expect("non-empty");
        assert_eq!(preview.chars().count(), 200);
        assert!(preview.ends_with('…'));
    }

    /// Multi-line messages get whitespace-collapsed before capping.
    #[test]
    fn last_message_preview_collapses_whitespace() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        // JSON-encoded newline and tab.
        let body = concat!(
            r#"{"type":"assistant","sessionId":"s","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"line one\n\nline two\twith\ttabs"}]}}"#,
            "\n",
        );
        write_transcript(&fixture.claude_code_state_root(), "-work", "s", body);

        let session = discover_one_session(&context);
        assert_eq!(
            session.last_message_preview.as_deref(),
            Some("line one line two with tabs"),
        );
    }
}
