//! Codex harness discovery.
//!
//! Reads the first JSONL line (`session_meta`) of each rollout file under
//! `$STATE_ROOT/sessions/**/rollout-*.jsonl` and emits one `AgentSession` per
//! discovered session. Real Codex stores rollouts under
//! `sessions/YYYY/MM/DD/`, so the scanner walks the tree recursively rather
//! than only looking at the top-level directory. Malformed records and
//! rollouts without a `session_meta` envelope are skipped silently so a single
//! bad file cannot poison discovery.
//!
//! Per ADR 0018 the adapter also extracts the `forked_from_id` field from
//! `session_meta.payload` when present and emits an intra-harness
//! `parent_session` candidate. Codex does not currently expose a separate
//! resume pointer; the rollout format only carries the fork ancestry, so
//! resume-only lineage is left for a follow-up if and when codex publishes
//! a distinguishable field.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
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
}

fn discover_state(state_root: &Path) -> Result<GraphFragment> {
    let sessions_dir = state_root.join("sessions");

    if !sessions_dir.exists() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut metas: Vec<SessionMetaPayload> = Vec::new();

    let mut previews: HashMap<String, String> = HashMap::new();

    visit_rollouts(&sessions_dir, &mut |path| {
        if let Some(meta) = read_session_meta(path) {
            if let Some(preview) = read_rollout_last_message_preview(path) {
                previews.insert(meta.id.clone(), preview);
            }
            metas.push(meta);
        }
    })?;

    let known_ids: HashMap<&str, ()> = metas.iter().map(|m| (m.id.as_str(), ())).collect();
    let mut nodes = Vec::with_capacity(metas.len());
    let mut candidate_links = Vec::new();

    for meta in &metas {
        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, &meta.id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: meta.cwd.clone(),
            title: None,
            last_message_preview: previews.get(&meta.id).cloned(),
        }));

        let Some(parent_id) = meta.forked_from_id.as_deref() else {
            continue;
        };

        if parent_id.is_empty() || parent_id == meta.id {
            // Empty pointers carry no information; self-fork pointers would
            // create a degenerate cycle.
            continue;
        }

        let resolved_parent = known_ids.get(parent_id).map(|_| parent_id);
        candidate_links.push(build_lineage_link(
            &meta.id,
            parent_id,
            &state_scope,
            resolved_parent,
        ));
    }

    Ok(GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
    })
}

fn build_lineage_link(
    child_session_key: &str,
    parent_session_key: &str,
    state_scope: &str,
    resolved_parent: Option<&str>,
) -> GraphLink {
    // Codex `forked_from_id` is a true fork pointer (multiple children can
    // share one parent), so `fork` is the correct ADR 0018 operation type.
    let lineage_kind = "fork";

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
                format!("codex:lineage:{child_session_key}:parent_session:{parent_key}"),
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
                    "codex:lineage:{child_session_key}:parent_session:unresolved:{parent_session_key}"
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
            evidence: Some(format!("codex session_meta {lineage_kind}")),
            fields,
        },
        state: LinkState::Active,
    }
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
        return Some(text.clone());
    }
    None
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
}
