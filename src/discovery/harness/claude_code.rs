//! Claude Code harness discovery.
//!
//! Walks `$STATE_ROOT/projects/<encoded-cwd>/<session>.jsonl` and emits one
//! `AgentSession` per discovered session. The session id is taken from the
//! file stem; cwd is read from the first JSONL line that carries it (real
//! Claude Code files commonly start with a `permission-mode` envelope that
//! lacks `cwd`, with the cwd appearing on later user/assistant events).
//! When no JSONL line carries a cwd, the encoded project directory name is
//! decoded as a best-effort fallback. Malformed files yield no session.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

/// Maximum number of JSONL lines to scan when looking for `cwd` evidence.
/// Real sessions almost always carry cwd within the first few user/assistant
/// events; the cap keeps very long transcripts cheap.
const MAX_HEADER_SCAN_LINES: usize = 200;

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

    for project in fs::read_dir(&projects)? {
        let project_dir = project?.path();

        if !project_dir.is_dir() {
            continue;
        }

        let fallback_cwd = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(decode_project_dir);

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

            nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(HARNESS_KEY, &state_scope, &meta.session_id),
                harness_key: HARNESS_KEY.to_string(),
                cwd: meta.cwd,
                title: meta.summary,
            }));
        }
    }

    Ok(GraphFragment {
        nodes,
        candidate_links: Vec::new(),
        diagnostics: Vec::new(),
    })
}

struct SessionHeader {
    session_id: String,
    cwd: Option<String>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct ScannedLine {
    #[serde(rename = "sessionId", default)]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

fn read_session_header(path: &Path, fallback_cwd: Option<&str>) -> Option<SessionHeader> {
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)?;
    let file = fs::File::open(path).ok()?;
    let mut cwd: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut saw_any_record = false;

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
        if cwd.is_some() && summary.is_some() {
            break;
        }
        // sessionId from JSONL is informational; the filename is authoritative.
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
    })
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
}
