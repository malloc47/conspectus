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

use std::collections::BTreeMap;
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

pub const HARNESS_KEY: &str = crate::discovery::providers::CLAUDE_CODE;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

/// H-EXT-004 runtime attribution surface for claude-code.
/// The harness ships as either `claude` or `claude-code` on
/// `PATH`; session ids are UUID-shaped; the CLI spawns a few
/// helper daemons whose commands the pre-H-EXT-004
/// `is_claude_background_process` heuristic already recognizes.
static CLAUDE_CODE_RUNTIME_SIGNATURE: super::RuntimeSignature = super::RuntimeSignature {
    harness_key: HARNESS_KEY,
    process_command_basenames: &["claude", "claude-code"],
    command_substrings: &["claude"],
    fd_path_patterns: &["/.claude/tasks/", "/.claude/projects/"],
    extract_session_keys: super::generic_uuid_like_session_keys,
    is_background_process: claude_code_is_background_process,
    is_subagent_process: super::no_match,
};

/// Recognize claude-code's helper daemons. Preserves the
/// pre-H-EXT-004 heuristics from
/// `cross_link::RuntimeProcessRecord::is_claude_background_process`.
fn claude_code_is_background_process(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    command.contains(" daemon run ")
        || command.contains(" --bg-spare")
        || command.contains(" --bg-pty-host")
}

impl HarnessAdapter for ClaudeCodeAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn runtime_signature(&self) -> &'static super::RuntimeSignature {
        &CLAUDE_CODE_RUNTIME_SIGNATURE
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
        Some(&crate::viewer::parser::claude_code::ClaudeCodeParser)
    }

    /// H-TBL-014: the row-label column is tight, so `claude-code`
    /// renders as `claude`. Kept here alongside the adapter
    /// (H-EXT-002) instead of the rows-module match arm that
    /// pre-registry callers used.
    fn display_label(&self) -> &'static str {
        "claude"
    }

    fn launch_options(&self) -> &'static [super::HarnessLaunchOption] {
        super::CLAUDE_CODE_LAUNCH_OPTIONS
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let Some(state_root) = context.harness_state_root(self.harness_key()) else {
            return Ok(GraphFragment::empty());
        };
        let mut fragment = discover_state(state_root)?;
        crate::discovery::stamp_fragment(
            &mut fragment,
            HARNESS_KEY,
            crate::discovery::current_epoch(),
        );
        Ok(fragment)
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("claude")]
    }

    fn resume_argv(
        &self,
        session_id: &str,
        _cwd: &std::path::Path,
    ) -> Option<Vec<std::ffi::OsString>> {
        // Matches the TUI resume-command shape in
        // `src/tui/resume.rs:42`.
        Some(vec![
            std::ffi::OsString::from("claude"),
            std::ffi::OsString::from("--resume"),
            std::ffi::OsString::from(session_id),
        ])
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
                    session_kind: None,
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
        node_provenance: BTreeMap::new(),
    })
}

// H-HYG-004: single production impl. See
// `discovery::harness::aider` for the H-HYG-004 rationale
// (fixture writers stamp mtimes via `File::set_modified`, so
// no cargo-test argv sniff is needed here).
fn file_modified_epoch(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_secs()).ok()
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
            freshness_epoch: None,
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
            freshness_epoch: None,
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
#[path = "claude_code_tests.rs"]
mod tests;
