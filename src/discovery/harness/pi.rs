//! Pi harness discovery (ADR 0121).
//!
//! Pi writes one JSONL file per session under a single session root,
//! either grouped by cwd (`<root>/--<encoded-cwd>--/<ts>_<id>.jsonl`, the
//! default) or flat (`<root>/<ts>_<id>.jsonl`, when the operator points
//! Pi at its own directory). Both layouts are scanned. The cwd encoding
//! in directory names is lossy (`/`, `\`, and `:` all become `-`), so cwd
//! always comes from the header line and directory names are never
//! decoded.
//!
//! Reads are tiered per ADR 0086. The header, `session_info` names, and
//! entry timestamps are Tier 1: only those fields are deserialized, and
//! lines that can't carry a name are skipped before parsing. The
//! last-message preview is the Tier 2 read, capped and normalized by
//! [`normalize_last_message_preview`] (ADR 0023).
//!
//! `/fork`, `/clone`, and `pi --fork` record the parent's file path as the
//! header's `parentSession`. Each one becomes an ADR 0018
//! `parent_session` candidate from the child, with `lineage_kind = fork`
//! (the header doesn't say which command ran). In-file `/tree` branches
//! are one session and produce no lineage.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::memo::{FileStamp, StampedMap};
use crate::discovery::{DiscoveryCaches, DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
    normalize_last_message_preview, source_field,
};

pub const HARNESS_KEY: &str = crate::discovery::providers::PI;

/// Bytes read from the end of a session file for the newest entry
/// timestamp and the last-message preview. Pi entries are a few
/// kilobytes at most, so the tail holds several recent messages.
const TAIL_SCAN_BYTES: u64 = 32 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PiAdapter;

impl PiAdapter {
    pub fn new() -> Self {
        Self
    }
}

/// Runtime attribution surface for Pi. Discovery ships first with no
/// process, pane-command, or fd patterns, so Pi sessions take no part
/// in pane attribution until the exact-basename matching ADR 0121
/// specifies lands (backlog `CSP-612`).
static PI_RUNTIME_SIGNATURE: super::RuntimeSignature = super::RuntimeSignature {
    harness_key: HARNESS_KEY,
    process_command_basenames: &[],
    command_substrings: &[],
    fd_path_patterns: &[],
    extract_session_keys: super::generic_uuid_like_session_keys,
    is_background_process: super::no_match,
    is_subagent_process: super::no_match,
};

impl HarnessAdapter for PiAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn runtime_signature(&self) -> &'static super::RuntimeSignature {
        &PI_RUNTIME_SIGNATURE
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        super::discover_with_state_root(context, HARNESS_KEY, |root| {
            discover_state(root, context.caches())
        })
    }

    fn launch_argv(&self) -> Vec<OsString> {
        vec![OsString::from("pi")]
    }
}

/// Resolve Pi's session root from the process environment, in ADR 0121
/// order: `CONSPECTUS_PI_STATE`, `PI_CODING_AGENT_SESSION_DIR`, an
/// absolute `sessionDir` in the global settings, then
/// `<agent-dir>/sessions`.
pub fn state_root_from_env() -> Option<PathBuf> {
    resolve_state_root(|key| std::env::var_os(key))
}

/// [`state_root_from_env`] with the environment lookup injected, so
/// tests can exercise the precedence without touching process state.
pub(crate) fn resolve_state_root(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let non_empty = |key: &str| env(key).filter(|value| !value.is_empty());

    if let Some(root) = non_empty("CONSPECTUS_PI_STATE") {
        return Some(PathBuf::from(root));
    }
    if let Some(root) = non_empty("PI_CODING_AGENT_SESSION_DIR") {
        return Some(PathBuf::from(root));
    }
    let agent_dir = match non_empty("PI_CODING_AGENT_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(non_empty("HOME")?).join(".pi/agent"),
    };
    if let Some(session_dir) = settings_session_dir(&agent_dir.join("settings.json")) {
        return Some(session_dir);
    }
    Some(agent_dir.join("sessions"))
}

/// The global settings' `sessionDir`, when it is an absolute path. Pi
/// resolves a relative value against each run's cwd, which discovery
/// can't know.
fn settings_session_dir(settings: &Path) -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct Settings {
        #[serde(rename = "sessionDir", default)]
        session_dir: Option<String>,
    }

    let text = fs::read_to_string(settings).ok()?;
    let parsed: Settings = serde_json::from_str(&text).ok()?;
    let dir = PathBuf::from(parsed.session_dir?);
    dir.is_absolute().then_some(dir)
}

fn discover_state(state_root: &Path, caches: &DiscoveryCaches) -> Result<GraphFragment> {
    if !state_root.is_dir() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut sessions: BTreeMap<String, PiSession> = BTreeMap::new();

    for path in session_files(state_root) {
        if let Some(session) = scan_session_cached(&path, &state_scope, &caches.pi_sessions) {
            // One node per session key. Two files claiming the same id
            // (a copied file, say) keep the most recently active one.
            let keep = sessions
                .get(&session.node.id.session_key)
                .is_none_or(|existing| {
                    existing.node.last_active_epoch < session.node.last_active_epoch
                });
            if keep {
                sessions.insert(session.node.id.session_key.clone(), session);
            }
        }
    }

    let candidate_links = sessions
        .values()
        .filter_map(|session| {
            let parent = session.parent.as_ref()?;
            let resolved = sessions.contains_key(&parent.session_key);
            Some(lineage_link(&session.node, parent, &state_scope, resolved))
        })
        .collect();

    Ok(GraphFragment {
        nodes: sessions
            .into_values()
            .map(|session| GraphNode::AgentSession(session.node))
            .collect(),
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    })
}

/// Every `*.jsonl` directly under the root (flat layout) and one level
/// down in `--<encoded-cwd>--` directories (grouped layout), in a stable
/// order. Unreadable directories are skipped.
fn session_files(state_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(state_root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_session_file(&path) {
            files.push(path);
        } else if path.is_dir() && is_cwd_dir(&path) {
            let Ok(children) = fs::read_dir(&path) else {
                continue;
            };
            files.extend(
                children
                    .flatten()
                    .map(|child| child.path())
                    .filter(|child| is_session_file(child)),
            );
        }
    }
    files.sort();
    files
}

fn is_session_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file()
}

fn is_cwd_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.len() > 4 && name.starts_with("--") && name.ends_with("--"))
}

/// The session id in a `<timestamp>_<session-id>.jsonl` file name. Pi's
/// timestamps contain no `_`, and custom ids may, so the split is at the
/// first one.
pub(crate) fn session_key_from_file_name(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let (timestamp, id) = stem.split_once('_')?;
    (!timestamp.is_empty() && !id.is_empty()).then(|| id.to_string())
}

#[derive(Deserialize)]
struct Header {
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "parentSession", default)]
    parent_session: Option<String>,
}

/// The `session` header on the first line of `lines`, or `None` when that
/// line is missing, malformed, or another entry type.
fn parse_header(lines: &mut impl Iterator<Item = std::io::Result<String>>) -> Option<Header> {
    let header: Header = serde_json::from_str(&lines.next()?.ok()?).ok()?;
    (header.entry_type == "session").then_some(header)
}

/// A session's parent, recovered from the header's `parentSession` path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParentRef {
    session_key: String,
    path: String,
}

/// Recover the parent's session key from its path. The path must name a
/// Pi session file; the parent's own header id wins when the file is
/// readable, and the file name's id stands in when it isn't (deleted, or
/// outside this machine's scan).
fn parent_ref(parent_path: &str) -> Option<ParentRef> {
    let path = Path::new(parent_path);
    if path.extension().is_none_or(|ext| ext != "jsonl") {
        return None;
    }
    let from_name = session_key_from_file_name(path)?;
    let from_header = fs::File::open(path).ok().and_then(|file| {
        parse_header(&mut BufReader::new(file).lines())?
            .id
            .filter(|id| !id.is_empty())
    });
    Some(ParentRef {
        session_key: from_header.unwrap_or(from_name),
        path: parent_path.to_string(),
    })
}

#[derive(Deserialize)]
struct NameEntry {
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// Session identity and cwd from the header line, plus the latest
/// `session_info` name. `None` when the first line is not a `session`
/// header or no session key can be found.
struct Scanned {
    session_key: String,
    cwd: Option<String>,
    title: Option<String>,
    header_epoch: Option<i64>,
    parent_session: Option<String>,
}

fn read_header_and_name(path: &Path) -> Option<Scanned> {
    let file = fs::File::open(path).ok()?;
    let mut lines = BufReader::new(file).lines();

    let header = parse_header(&mut lines)?;
    let session_key = header
        .id
        .filter(|id| !id.is_empty())
        .or_else(|| session_key_from_file_name(path))?;

    let mut title = None;
    for line in lines {
        let Ok(line) = line else { break };
        // Only `session_info` entries carry a name; skip every other line
        // before parsing so message bodies are never deserialized.
        if !line.contains("\"session_info\"") {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<NameEntry>(&line)
            && entry.entry_type.as_deref() == Some("session_info")
        {
            // Latest wins; an empty name clears an earlier one.
            title = entry.name.filter(|name| !name.trim().is_empty());
        }
    }

    Some(Scanned {
        session_key,
        cwd: header.cwd.filter(|cwd| !cwd.is_empty()),
        title,
        header_epoch: header.timestamp.as_deref().and_then(parse_epoch),
        parent_session: header.parent_session.filter(|path| !path.is_empty()),
    })
}

fn parse_epoch(timestamp: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|time| time.timestamp())
}

#[derive(Deserialize)]
struct TailEntry {
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type", default)]
    block_type: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// The newest entry timestamp and the last-message preview, read from
/// the tail of the file. Entries are appended in time order, so the
/// last timestamped line is the newest.
fn read_tail(path: &Path) -> (Option<i64>, Option<String>) {
    let Some(buf) = read_tail_bytes(path) else {
        return (None, None);
    };
    let mut newest = None;
    let mut preview = None;
    for line in buf.split(|&byte| byte == b'\n').rev() {
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_slice::<TailEntry>(line) else {
            continue;
        };
        if newest.is_none() {
            newest = entry.timestamp.as_deref().and_then(parse_epoch);
        }
        if preview.is_none() {
            preview = preview_text(&entry).and_then(|text| normalize_last_message_preview(&text));
        }
        if newest.is_some() && preview.is_some() {
            break;
        }
    }
    (newest, preview)
}

fn read_tail_bytes(path: &Path) -> Option<Vec<u8>> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_SCAN_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    // Starting mid-file leaves a partial first line; drop it.
    if start > 0 {
        let first_newline = buf.iter().position(|&byte| byte == b'\n')?;
        buf.drain(..=first_newline);
    }
    Some(buf)
}

/// User or assistant text from a `message` entry, per ADR 0023: the
/// last non-empty `text` block, skipping thinking, tool calls, tool
/// results, and system prompts.
fn preview_text(entry: &TailEntry) -> Option<String> {
    if entry.entry_type.as_deref() != Some("message") {
        return None;
    }
    let message = entry.message.as_ref()?;
    if !matches!(message.role.as_deref(), Some("user" | "assistant")) {
        return None;
    }
    match message.content.as_ref()? {
        Content::Text(text) => (!text.trim().is_empty()).then(|| text.clone()),
        Content::Blocks(blocks) => blocks.iter().rev().find_map(|block| {
            let text = block.text.as_ref()?;
            (block.block_type.as_deref() == Some("text") && !text.trim().is_empty())
                .then(|| text.clone())
        }),
    }
}

/// One scanned session file: its node and, for a fork or clone, the
/// parent it points at.
#[derive(Clone, Debug)]
pub(crate) struct PiSession {
    node: AgentSessionNode,
    parent: Option<ParentRef>,
}

/// Scan `path` into a session, reusing the cached result while the file's
/// `(mtime, size)` is unchanged.
fn scan_session_cached(
    path: &Path,
    state_scope: &str,
    cache: &StampedMap<FileStamp, PiSession>,
) -> Option<PiSession> {
    let stamp = FileStamp::of(path)?;
    if let Some(session) = cache.get(path, &stamp) {
        return Some(session);
    }

    let scanned = read_header_and_name(path)?;
    let parent = scanned
        .parent_session
        .as_deref()
        .and_then(parent_ref)
        .filter(|parent| parent.session_key != scanned.session_key);
    let (newest_epoch, last_message_preview) = read_tail(path);
    let node = AgentSessionNode {
        id: AgentSessionId::new(HARNESS_KEY, state_scope, &scanned.session_key),
        harness_key: HARNESS_KEY.to_string(),
        cwd: scanned.cwd,
        title: scanned.title,
        last_message_preview,
        last_active_epoch: newest_epoch
            .or(scanned.header_epoch)
            .or_else(|| stamp.modified_epoch()),
        session_kind: None,
    };

    let session = PiSession { node, parent };
    cache.insert(path.to_path_buf(), stamp, session.clone());
    Some(session)
}

/// The ADR 0018 `parent_session` candidate from `child` to `parent`: a
/// node endpoint when the parent was discovered in this root, otherwise
/// unresolved evidence carrying the recovered key and the parent's path.
fn lineage_link(
    child: &AgentSessionNode,
    parent: &ParentRef,
    state_scope: &str,
    resolved: bool,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert("harness_key".to_string(), HARNESS_KEY.into());
    fields.insert(source_field::LINEAGE_KIND.to_string(), "fork".into());
    fields.insert("lineage_fidelity".to_string(), "native".into());
    fields.insert(
        "parent_session_id".to_string(),
        parent.session_key.as_str().into(),
    );
    fields.insert(
        "parent_session_path".to_string(),
        parent.path.as_str().into(),
    );

    let child_key = &child.id.session_key;
    let parent_key = &parent.session_key;
    let (target, id) = if resolved {
        (
            LinkEndpoint::Node {
                id: NodeId::AgentSession(AgentSessionId::new(HARNESS_KEY, state_scope, parent_key)),
            },
            format!("pi:lineage:{child_key}:parent_session:{parent_key}"),
        )
    } else {
        (
            LinkEndpoint::Unresolved {
                evidence: UnresolvedEndpoint {
                    node_type: "agent_session".to_string(),
                    harness_key: Some(HARNESS_KEY.to_string()),
                    native_id: Some(parent_key.clone()),
                    state_scope: Some(state_scope.to_string()),
                    path: Some(parent.path.clone()),
                    metadata: fields.clone(),
                },
            },
            format!("pi:lineage:{child_key}:parent_session:unresolved:{parent_key}"),
        )
    };

    GraphLink {
        id,
        source: NodeId::AgentSession(child.id.clone()),
        target,
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: HARNESS_KEY.to_string(),
            evidence: Some("pi session header parentSession".to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

#[cfg(test)]
#[path = "pi_tests.rs"]
mod tests;
