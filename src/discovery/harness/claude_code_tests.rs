// Extracted from claude_code.rs H-HYG-011 rolling wave via #[path = "claude_code_tests.rs"] mod tests;
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
            &ClaudeCodeSessionRecord::new("session-a", "/work/alpha").with_summary("alpha work"),
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
        decode_project_dir("-home-user--agent-deck"),
        "/home/user/agent/deck"
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
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved parent endpoint, got {other:?}")
        }
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
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved endpoint, got {other:?}")
        }
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
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved parent endpoint, got {other:?}")
        }
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
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved parent endpoint, got {other:?}")
        }
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
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved endpoint, got {other:?}")
        }
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
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved target, got {other:?}")
        }
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
        .find_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
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
