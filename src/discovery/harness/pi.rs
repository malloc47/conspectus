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
use serde_json::json;

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

/// Executable names Pi runs under: its process title is `pi`, or
/// `pi-rpc` in RPC mode.
const PI_BASENAMES: &[&str] = &["pi", "pi-rpc"];

/// Runtime attribution surface for Pi (ADR 0121). `pi` is too short to
/// match as a substring, so pane commands match it as a whole token
/// instead. Pi holds its session file open only while appending, so fd
/// evidence would flicker and no fd patterns are listed. Session ids are
/// UUIDv7.
static PI_RUNTIME_SIGNATURE: super::RuntimeSignature = super::RuntimeSignature {
    harness_key: HARNESS_KEY,
    process_command_basenames: PI_BASENAMES,
    command_substrings: &[],
    command_token_basenames: PI_BASENAMES,
    fd_path_patterns: &[],
    extract_session_keys: super::generic_uuid_like_session_keys,
    is_background_process: pi_is_background_process,
    is_subagent_process: super::no_match,
};

/// RPC mode is driven over stdio by another program, not by someone
/// typing in a pane, so it must not claim pane attribution.
fn pi_is_background_process(command: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .and_then(|program| program.rsplit('/').next())
        .is_some_and(|name| name.eq_ignore_ascii_case("pi-rpc"))
}

impl HarnessAdapter for PiAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn runtime_signature(&self) -> &'static super::RuntimeSignature {
        &PI_RUNTIME_SIGNATURE
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        super::discover_with_state_root(context, HARNESS_KEY, |root| {
            Ok(discover_state(root, context.caches()))
        })
    }

    /// The parser finds the session file under the session root that
    /// discovery recorded as the session's state scope.
    fn transcript_source(
        &self,
        session: &AgentSessionId,
    ) -> Option<crate::viewer::model::SessionLocator> {
        Some(crate::viewer::model::SessionLocator {
            harness_key: HARNESS_KEY.to_string(),
            session_key: session.session_key.clone(),
            state_root: session.state_scope.clone().into(),
        })
    }

    fn transcript_parser(&self) -> Option<&'static dyn crate::viewer::parser::HarnessParser> {
        Some(&crate::viewer::parser::pi::PiParser)
    }

    /// The session root sits above a grouped `--<cwd>--` directory, or is
    /// the file's own directory in the flat layout.
    fn state_scope_for_transcript(&self, path: &Path) -> Option<String> {
        let dir = path.parent()?;
        let root = if is_cwd_dir(dir) { dir.parent()? } else { dir };
        Some(root.to_string_lossy().to_string())
    }

    fn launch_argv(&self) -> Vec<OsString> {
        vec![OsString::from("pi")]
    }

    /// `pi --session <id>` looks the id up in the current project first,
    /// so the caller runs it from the session's cwd.
    fn resume_argv(&self, session_id: &str, _cwd: &Path) -> Option<Vec<OsString>> {
        Some(vec![
            OsString::from("pi"),
            OsString::from("--session"),
            OsString::from(session_id),
        ])
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
    let agent_dir = resolve_agent_dir(&env)?;
    if let Some(session_dir) = settings_session_dir(&agent_dir.join("settings.json")) {
        return Some(session_dir);
    }
    Some(agent_dir.join("sessions"))
}

/// Pi's agent directory: `PI_CODING_AGENT_DIR`, or `~/.pi/agent`. Holds
/// the global settings, the default session root, and user extensions.
pub fn agent_dir_from_env() -> Option<PathBuf> {
    resolve_agent_dir(&|key| std::env::var_os(key))
}

fn resolve_agent_dir(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let non_empty = |key: &str| env(key).filter(|value| !value.is_empty());
    match non_empty("PI_CODING_AGENT_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(PathBuf::from(non_empty("HOME")?).join(".pi").join("agent")),
    }
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

fn discover_state(state_root: &Path, caches: &DiscoveryCaches) -> GraphFragment {
    if !state_root.is_dir() {
        return GraphFragment::empty();
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut sessions: BTreeMap<String, ScannedSession> = BTreeMap::new();

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
            Some(build_lineage_link(
                &session.node,
                parent,
                &state_scope,
                resolved,
            ))
        })
        .collect();

    GraphFragment {
        nodes: sessions
            .into_values()
            .map(|session| GraphNode::AgentSession(session.node))
            .collect(),
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    }
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

/// The `session` header in `line`, or `None` when the line is malformed
/// or another entry type.
fn parse_header(line: &str) -> Option<Header> {
    let header: Header = serde_json::from_str(line).ok()?;
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
        let line = BufReader::new(file).lines().next()?.ok()?;
        parse_header(&line)?.id.filter(|id| !id.is_empty())
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
/// `session_info` name and how far into the file the name scan got.
#[derive(Clone, Debug)]
pub(crate) struct Scanned {
    session_key: String,
    cwd: Option<String>,
    title: Option<String>,
    header_epoch: Option<i64>,
    parent_session: Option<String>,
    /// Bytes of complete lines already scanned for names.
    names_scanned_to: u64,
}

/// Read the header and scan the whole file for names. `None` when the
/// first line is not a `session` header or no session key can be found.
fn read_header_and_name(path: &Path) -> Option<Scanned> {
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut first = String::new();
    let header_len = reader.read_line(&mut first).ok()?;
    let header = parse_header(first.trim_end())?;
    let session_key = header
        .id
        .filter(|id| !id.is_empty())
        .or_else(|| session_key_from_file_name(path))?;

    let mut scanned = Scanned {
        session_key,
        cwd: header.cwd.filter(|cwd| !cwd.is_empty()),
        title: None,
        header_epoch: header.timestamp.as_deref().and_then(parse_epoch),
        parent_session: header.parent_session.filter(|path| !path.is_empty()),
        names_scanned_to: header_len as u64,
    };
    scan_names(&mut reader, &mut scanned);
    Some(scanned)
}

/// Pi only appends to a session file, so a file that grew is scanned
/// for names from where the previous scan stopped.
fn continue_name_scan(path: &Path, mut previous: Scanned) -> Option<Scanned> {
    let mut file = fs::File::open(path).ok()?;
    file.seek(SeekFrom::Start(previous.names_scanned_to)).ok()?;
    scan_names(&mut BufReader::new(file), &mut previous);
    Some(previous)
}

/// Fold every complete `session_info` line from `reader` into
/// `scanned.title`. A last line without its newline is still being
/// written; it is left for the next scan.
fn scan_names(reader: &mut impl BufRead, scanned: &mut Scanned) {
    let mut line = String::new();
    loop {
        line.clear();
        let Ok(read) = reader.read_line(&mut line) else {
            break;
        };
        if read == 0 || !line.ends_with('\n') {
            break;
        }
        scanned.names_scanned_to += read as u64;
        // Only `session_info` entries carry a name; skip every other line
        // before parsing so message bodies are never deserialized.
        if !line.contains("\"session_info\"") {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<NameEntry>(&line)
            && entry.entry_type.as_deref() == Some("session_info")
        {
            // Latest wins; an empty name clears an earlier one.
            scanned.title = entry.name.filter(|name| !name.trim().is_empty());
        }
    }
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
pub(crate) struct ScannedSession {
    node: AgentSessionNode,
    parent: Option<ParentRef>,
    scan: Scanned,
}

/// Scan `path` into a session, reusing the cached result while the file's
/// `(mtime, size)` is unchanged.
fn scan_session_cached(
    path: &Path,
    state_scope: &str,
    cache: &StampedMap<FileStamp, ScannedSession>,
) -> Option<ScannedSession> {
    let stamp = FileStamp::of(path)?;
    if let Some(session) = cache.get(path, &stamp) {
        return Some(session);
    }

    let scanned = match cache.get_if(path, |_, _| true) {
        Some(previous) if previous.scan.names_scanned_to <= stamp.len => {
            continue_name_scan(path, previous.scan)?
        }
        _ => read_header_and_name(path)?,
    };
    let parent = scanned
        .parent_session
        .as_deref()
        .and_then(parent_ref)
        .filter(|parent| parent.session_key != scanned.session_key);
    let (newest_epoch, last_message_preview) = read_tail(path);
    let node = AgentSessionNode {
        id: AgentSessionId::new(HARNESS_KEY, state_scope, &scanned.session_key),
        harness_key: HARNESS_KEY.to_string(),
        cwd: scanned.cwd.clone(),
        title: scanned.title.clone(),
        last_message_preview,
        last_active_epoch: newest_epoch
            .or(scanned.header_epoch)
            .or_else(|| stamp.modified_epoch()),
        session_kind: None,
    };

    let session = ScannedSession {
        node,
        parent,
        scan: scanned,
    };
    cache.insert(path.to_path_buf(), stamp, session.clone());
    Some(session)
}

/// The ADR 0018 `parent_session` candidate from `child` to `parent`: a
/// node endpoint when the parent was discovered in this root, otherwise
/// unresolved evidence carrying the recovered key and the parent's path.
fn build_lineage_link(
    child: &AgentSessionNode,
    parent: &ParentRef,
    state_scope: &str,
    resolved: bool,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert("harness_key".to_string(), json!(HARNESS_KEY));
    fields.insert(source_field::LINEAGE_KIND.to_string(), json!("fork"));
    fields.insert("parent_session_id".to_string(), json!(parent.session_key));
    fields.insert("parent_session_path".to_string(), json!(parent.path));

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
