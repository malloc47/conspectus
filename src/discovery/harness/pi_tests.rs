use std::collections::HashMap;
use std::ffi::OsString;

use tempfile::TempDir;

use super::*;

fn fixture_root(layout: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pi")
        .join(layout)
}

fn discover_at(root: &Path) -> Vec<AgentSessionNode> {
    let context = DiscoveryContext::default().with_harness_state_root(HARNESS_KEY, root);
    PiAdapter::new()
        .discover(&context)
        .expect("discover")
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session),
            _ => None,
        })
        .collect()
}

fn by_key<'a>(sessions: &'a [AgentSessionNode], key: &str) -> &'a AgentSessionNode {
    sessions
        .iter()
        .find(|session| session.id.session_key == key)
        .unwrap_or_else(|| panic!("session `{key}` in {sessions:#?}"))
}

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
        .collect();
    move |key| map.get(key).cloned()
}

#[test]
fn adapter_returns_empty_without_a_state_root() {
    let fragment = PiAdapter::new()
        .discover(&DiscoveryContext::default())
        .expect("discover");
    assert!(fragment.nodes.is_empty());
}

#[test]
fn adapter_returns_empty_when_the_root_is_missing() {
    let temp = TempDir::new().expect("temp");
    assert!(discover_at(&temp.path().join("absent")).is_empty());
}

#[test]
fn grouped_layout_reads_cwd_from_the_header_not_the_directory() {
    let sessions = discover_at(&fixture_root("grouped"));

    // Both sessions live in `--work-my-repo--`, which `/work/my-repo` and
    // `/work/my/repo` both encode to; each keeps its own header cwd.
    let current = by_key(&sessions, "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
    assert_eq!(current.cwd.as_deref(), Some("/work/my-repo"));
    let legacy = by_key(&sessions, "0196f0b1-1111-7222-8333-944455566677");
    assert_eq!(legacy.cwd.as_deref(), Some("/work/my/repo"));

    assert!(sessions.iter().all(|session| session.harness_key == "pi"));
    let scope = fixture_root("grouped").to_string_lossy().to_string();
    assert!(
        sessions
            .iter()
            .all(|session| session.id.state_scope == scope),
        "state scope is the session root, not the cwd directory",
    );
}

#[test]
fn latest_session_info_name_becomes_the_title() {
    let sessions = discover_at(&fixture_root("grouped"));
    let named = by_key(&sessions, "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
    assert_eq!(named.title.as_deref(), Some("Keep latest session name"));
    let unnamed = by_key(&sessions, "0196f0b1-1111-7222-8333-944455566677");
    assert_eq!(unnamed.title, None);
}

#[test]
fn last_active_is_the_newest_entry_timestamp() {
    let sessions = discover_at(&fixture_root("grouped"));
    let named = by_key(&sessions, "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
    // 2026-10-16T10:02:00Z, the trailing `session_info` entry.
    assert_eq!(named.last_active_epoch, Some(1_792_144_920));
}

#[test]
fn preview_is_the_last_user_or_assistant_text() {
    let sessions = discover_at(&fixture_root("grouped"));
    let named = by_key(&sessions, "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
    // The trailing tool result is skipped, and whitespace is collapsed.
    assert_eq!(
        named.last_message_preview.as_deref(),
        Some("Done. The parser now keeps the latest name."),
    );
    let legacy = by_key(&sessions, "0196f0b1-1111-7222-8333-944455566677");
    assert_eq!(legacy.last_message_preview.as_deref(), Some("Legacy reply"));
}

#[test]
fn header_without_an_id_falls_back_to_the_file_name() {
    let sessions = discover_at(&fixture_root("grouped"));
    let session = by_key(&sessions, "0199f0a4-0000-7000-8000-000000000004");
    assert_eq!(session.cwd.as_deref(), Some("/work/other"));
    // A truncated line in the middle doesn't stop the scan.
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("Header without an id")
    );
    assert_eq!(session.last_active_epoch, Some(1_792_152_030));
}

#[test]
fn files_without_a_session_header_are_skipped() {
    let sessions = discover_at(&fixture_root("grouped"));
    let keys: Vec<&str> = sessions
        .iter()
        .map(|session| session.id.session_key.as_str())
        .collect();
    for skipped in [
        // First line is not JSON.
        "0199f0a3-0000-7000-8000-000000000003",
        // Empty file.
        "0199f0a5-0000-7000-8000-000000000005",
        // First line is a message, not a header.
        "0199f0a6-0000-7000-8000-000000000006",
    ] {
        assert!(
            !keys.contains(&skipped),
            "{skipped} should be skipped: {keys:?}"
        );
    }
    assert_eq!(sessions.len(), 3, "{keys:?}");
}

#[test]
fn flat_layout_discovers_sessions_directly_under_the_root() {
    let sessions = discover_at(&fixture_root("flat"));
    let session = by_key(&sessions, "my-custom.id");
    assert_eq!(session.cwd.as_deref(), Some("/work/flat"));
    // The system prompt is not a preview candidate.
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("Flat layout with a custom session id"),
    );
}

#[test]
fn discovery_is_stable_across_runs() {
    let root = fixture_root("grouped");
    assert_eq!(discover_at(&root), discover_at(&root));
}

#[test]
fn rewritten_file_is_rescanned() {
    let temp = TempDir::new().expect("temp");
    let dir = temp.path().join("--work-a--");
    fs::create_dir_all(&dir).expect("dir");
    let file = dir.join("2026-10-16T10-00-00-000Z_s1.jsonl");
    let header = r#"{"type":"session","version":3,"id":"s1","timestamp":"2026-10-16T10:00:00.000Z","cwd":"/work/a"}"#;
    fs::write(&file, format!("{header}\n")).expect("write");

    let context = DiscoveryContext::default().with_harness_state_root(HARNESS_KEY, temp.path());
    let adapter = PiAdapter::new();
    let titles = |context: &DiscoveryContext| -> Vec<Option<String>> {
        adapter
            .discover(context)
            .expect("discover")
            .nodes
            .into_iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(session) => Some(session.title),
                _ => None,
            })
            .collect()
    };
    assert_eq!(titles(&context), vec![None]);

    let named = r#"{"type":"session_info","id":"n1","parentId":null,"timestamp":"2026-10-16T10:05:00.000Z","name":"renamed"}"#;
    fs::write(&file, format!("{header}\n{named}\n")).expect("rewrite");
    assert_eq!(titles(&context), vec![Some("renamed".to_string())]);
}

#[test]
fn session_key_from_file_name_splits_at_the_first_underscore() {
    let key = |name: &str| session_key_from_file_name(Path::new(name));
    assert_eq!(
        key("2026-10-16T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl").as_deref(),
        Some("0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d"),
    );
    assert_eq!(
        key("2026-10-16T10-00-00-000Z_custom_id.jsonl").as_deref(),
        Some("custom_id")
    );
    assert_eq!(key("no-separator.jsonl"), None);
    assert_eq!(key("_missing-timestamp.jsonl"), None);
    assert_eq!(key("2026-10-16T10-00-00-000Z_.jsonl"), None);
}

#[test]
fn state_root_prefers_the_conspectus_override() {
    let env = env_of(&[
        ("CONSPECTUS_PI_STATE", "/override"),
        ("PI_CODING_AGENT_SESSION_DIR", "/pi-sessions"),
        ("HOME", "/home/user"),
    ]);
    assert_eq!(resolve_state_root(env), Some(PathBuf::from("/override")));
}

#[test]
fn state_root_honors_pi_session_dir() {
    let env = env_of(&[
        ("PI_CODING_AGENT_SESSION_DIR", "/pi-sessions"),
        ("PI_CODING_AGENT_DIR", "/agent"),
        ("HOME", "/home/user"),
    ]);
    assert_eq!(resolve_state_root(env), Some(PathBuf::from("/pi-sessions")));
}

#[test]
fn state_root_reads_an_absolute_global_session_dir_setting() {
    let temp = TempDir::new().expect("temp");
    let agent = temp.path().join("agent");
    fs::create_dir_all(&agent).expect("agent dir");
    fs::write(
        agent.join("settings.json"),
        r#"{"sessionDir":"/elsewhere/sessions","theme":"dark"}"#,
    )
    .expect("settings");
    let env = env_of(&[("PI_CODING_AGENT_DIR", agent.to_str().expect("utf8"))]);
    assert_eq!(
        resolve_state_root(env),
        Some(PathBuf::from("/elsewhere/sessions"))
    );
}

#[test]
fn state_root_ignores_a_relative_or_unreadable_session_dir_setting() {
    let temp = TempDir::new().expect("temp");
    let agent = temp.path().join("agent");
    fs::create_dir_all(&agent).expect("agent dir");
    let env = || env_of(&[("PI_CODING_AGENT_DIR", agent.to_str().expect("utf8"))]);

    fs::write(
        agent.join("settings.json"),
        r#"{"sessionDir":"sessions-here"}"#,
    )
    .expect("relative");
    assert_eq!(resolve_state_root(env()), Some(agent.join("sessions")));

    fs::write(agent.join("settings.json"), "{ not json").expect("malformed");
    assert_eq!(resolve_state_root(env()), Some(agent.join("sessions")));
}

#[test]
fn state_root_defaults_under_home() {
    let env = env_of(&[("HOME", "/home/user")]);
    assert_eq!(
        resolve_state_root(env),
        Some(PathBuf::from("/home/user/.pi/agent/sessions"))
    );
    assert_eq!(resolve_state_root(env_of(&[])), None);
}

#[test]
fn empty_environment_values_are_ignored() {
    let env = env_of(&[
        ("CONSPECTUS_PI_STATE", ""),
        ("PI_CODING_AGENT_SESSION_DIR", ""),
        ("HOME", "/home/user"),
    ]);
    assert_eq!(
        resolve_state_root(env),
        Some(PathBuf::from("/home/user/.pi/agent/sessions"))
    );
}

#[test]
fn unreadable_session_file_is_skipped() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().expect("temp");
    let readable = temp.path().join("2026-10-16T10-00-00-000Z_ok.jsonl");
    let locked = temp.path().join("2026-10-16T10-00-00-000Z_locked.jsonl");
    let header = |id: &str| {
        format!(r#"{{"type":"session","version":3,"id":"{id}","cwd":"/work/a"}}"#) + "\n"
    };
    fs::write(&readable, header("ok")).expect("write readable");
    fs::write(&locked, header("locked")).expect("write locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read(&locked).is_ok() {
        // Running as root: permissions don't stop the read.
        return;
    }

    let sessions = discover_at(temp.path());
    let keys: Vec<&str> = sessions
        .iter()
        .map(|session| session.id.session_key.as_str())
        .collect();
    assert_eq!(keys, vec!["ok"]);
}
