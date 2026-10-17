use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;

use tempfile::TempDir;

use super::*;
use crate::discovery::harness::fixtures::{HarnessFixture, PiSessionRecord, write_malformed};

fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
    let fixture = HarnessFixture::at(temp.path());
    let context =
        DiscoveryContext::default().with_harness_state_root(HARNESS_KEY, fixture.pi_state_root());
    (context, fixture)
}

fn sessions(context: &DiscoveryContext) -> Vec<AgentSessionNode> {
    PiAdapter::new()
        .discover(context)
        .expect("discover")
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session),
            _ => None,
        })
        .collect()
}

fn discover_session(context: &DiscoveryContext, id: &str) -> AgentSessionNode {
    sessions(context)
        .into_iter()
        .find(|session| session.id.session_key == id)
        .unwrap_or_else(|| panic!("session `{id}`"))
}

fn append_session_lines(path: &Path, lines: &[&str]) {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(path)
        .expect("open session");
    for line in lines {
        writeln!(file, "{line}").expect("append line");
    }
}

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
        .collect();
    move |key| map.get(key).cloned()
}

#[test]
fn adapter_returns_empty_when_no_state_root_configured() {
    let fragment = PiAdapter::new()
        .discover(&DiscoveryContext::default())
        .expect("discover");
    assert!(fragment.nodes.is_empty());
}

#[test]
fn adapter_returns_empty_when_sessions_dir_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, _) = context_with_state(&temp);
    assert!(sessions(&context).is_empty());
}

#[test]
fn discovers_pi_sessions_with_cwd_name_and_stable_ids() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(
            &PiSessionRecord::new("session-a", "/work/alpha")
                .with_name("alpha work")
                .with_timestamp("2026-01-02T03:04:05.000Z"),
        )
        .expect("write a");
    fixture
        .write_pi_session(&PiSessionRecord::new("session-b", "/work/beta"))
        .expect("write b");

    let first = PiAdapter::new().discover(&context).expect("first");
    let second = PiAdapter::new().discover(&context).expect("second");
    assert_eq!(first, second);

    let a = discover_session(&context, "session-a");
    assert_eq!(a.harness_key, "pi");
    assert_eq!(a.cwd.as_deref(), Some("/work/alpha"));
    assert_eq!(a.title.as_deref(), Some("alpha work"));
    assert_eq!(
        a.id.state_scope,
        fixture.pi_state_root().to_string_lossy(),
        "the state scope is the session root, not the cwd directory",
    );
    assert!(discover_session(&context, "session-b").title.is_none());
}

#[test]
fn discovers_pi_sessions_in_flat_layout() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(&PiSessionRecord::new("my-custom.id", "/work/flat").flat())
        .expect("write flat");

    assert_eq!(
        discover_session(&context, "my-custom.id").cwd.as_deref(),
        Some("/work/flat")
    );
}

#[test]
fn cwd_comes_from_the_header_when_encoded_directories_collide() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    // Both cwds encode to `--work-my-repo--`.
    let dashed = fixture
        .write_pi_session(&PiSessionRecord::new("dashed", "/work/my-repo"))
        .expect("write dashed");
    let nested = fixture
        .write_pi_session(&PiSessionRecord::new("nested", "/work/my/repo"))
        .expect("write nested");
    assert_eq!(dashed.parent(), nested.parent());

    assert_eq!(
        discover_session(&context, "dashed").cwd.as_deref(),
        Some("/work/my-repo")
    );
    assert_eq!(
        discover_session(&context, "nested").cwd.as_deref(),
        Some("/work/my/repo")
    );
}

#[test]
fn latest_session_info_name_wins() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_pi_session(&PiSessionRecord::new("s", "/work").with_name("first name"))
        .expect("write");
    append_session_lines(
        &path,
        &[r#"{"type":"session_info","id":"n2","parentId":null,"name":"second name"}"#],
    );

    assert_eq!(
        discover_session(&context, "s").title.as_deref(),
        Some("second name")
    );
}

#[test]
fn name_scan_continues_from_where_the_last_scan_stopped() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_pi_session(&PiSessionRecord::new("s", "/work").with_exchange("q", "a"))
        .expect("write");
    assert!(discover_session(&context, "s").title.is_none());

    // A line still being written isn't counted until it is complete.
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open");
    write!(
        file,
        r#"{{"type":"session_info","id":"n1","parentId":null,"name":"ren"#
    )
    .expect("partial");
    assert!(discover_session(&context, "s").title.is_none());
    writeln!(file, r#"amed"}}"#).expect("rest of line");
    assert_eq!(
        discover_session(&context, "s").title.as_deref(),
        Some("renamed")
    );

    // A file that shrank was rewritten, so it is scanned from the start.
    fs::write(
        &path,
        concat!(
            r#"{"type":"session","version":3,"id":"s","cwd":"/work"}"#,
            "\n",
            r#"{"type":"session_info","id":"n1","parentId":null,"name":"short"}"#,
            "\n",
        ),
    )
    .expect("rewrite");
    assert_eq!(
        discover_session(&context, "s").title.as_deref(),
        Some("short")
    );
}

#[test]
fn last_active_is_the_newest_entry_timestamp() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_pi_session(
            &PiSessionRecord::new("s", "/work").with_timestamp("2026-01-02T03:04:05.000Z"),
        )
        .expect("write");
    assert_eq!(
        discover_session(&context, "s").last_active_epoch,
        Some(1_767_323_045)
    );

    append_session_lines(
        &path,
        &[
            r#"{"type":"label","id":"l1","parentId":null,"timestamp":"2026-01-02T04:00:00.000Z","targetId":"x","label":"done"}"#,
        ],
    );
    assert_eq!(
        discover_session(&context, "s").last_active_epoch,
        Some(1_767_326_400)
    );
}

#[test]
fn header_without_an_id_falls_back_to_the_file_name() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let dir = fixture.pi_state_root().join("--work--");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        dir.join("2026-01-02T03-04-05-000Z_from-name.jsonl"),
        "{\"type\":\"session\",\"version\":3,\"cwd\":\"/work\"}\n",
    )
    .expect("write");

    assert_eq!(
        discover_session(&context, "from-name").cwd.as_deref(),
        Some("/work")
    );
}

#[test]
fn skips_malformed_empty_and_headerless_files() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(&PiSessionRecord::new("good", "/work"))
        .expect("write good");
    let dir = fixture.pi_state_root().join("--work--");
    write_malformed(dir.join("2026-01-02T03-04-05-000Z_malformed.jsonl")).expect("malformed");
    fs::write(dir.join("2026-01-02T03-04-05-000Z_empty.jsonl"), "").expect("empty");
    fs::write(
        dir.join("2026-01-02T03-04-05-000Z_headerless.jsonl"),
        "{\"type\":\"message\",\"id\":\"m\",\"parentId\":null}\n",
    )
    .expect("headerless");
    fs::write(dir.join("notes.txt"), "not a session").expect("notes");

    let keys: Vec<String> = sessions(&context)
        .into_iter()
        .map(|session| session.id.session_key)
        .collect();
    assert_eq!(keys, ["good"]);
}

#[test]
fn skips_unreadable_session_files() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(&PiSessionRecord::new("ok", "/work"))
        .expect("write ok");
    let locked = fixture
        .write_pi_session(&PiSessionRecord::new("locked", "/work"))
        .expect("write locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read(&locked).is_ok() {
        // Running as root: permissions don't stop the read.
        return;
    }

    let keys: Vec<String> = sessions(&context)
        .into_iter()
        .map(|session| session.id.session_key)
        .collect();
    assert_eq!(keys, ["ok"]);
}

#[test]
fn last_message_preview_returns_the_most_recent_text() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(
            &PiSessionRecord::new("s", "/work")
                .with_exchange("first question", "first answer")
                .with_assistant_message("Done.   The parser\nkeeps the name."),
        )
        .expect("write");

    assert_eq!(
        discover_session(&context, "s")
            .last_message_preview
            .as_deref(),
        Some("Done. The parser keeps the name.")
    );
}

#[test]
fn last_message_preview_skips_tool_results_thinking_and_system_messages() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_pi_session(&PiSessionRecord::new("s", "/work").with_exchange("ask", "answer"))
        .expect("write");
    append_session_lines(
        &path,
        &[
            r#"{"type":"message","id":"t1","parentId":null,"message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"toolCall","id":"c","name":"bash","arguments":{}}]}}"#,
            r#"{"type":"message","id":"t2","parentId":"t1","message":{"role":"toolResult","toolCallId":"c","content":[{"type":"text","text":"tool output"}]}}"#,
            r#"{"type":"message","id":"t3","parentId":"t2","message":{"role":"system","content":"You are Pi."}}"#,
        ],
    );

    assert_eq!(
        discover_session(&context, "s")
            .last_message_preview
            .as_deref(),
        Some("answer")
    );
}

#[test]
fn last_message_preview_is_capped_at_two_hundred_chars() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(
            &PiSessionRecord::new("s", "/work").with_assistant_message("x".repeat(500)),
        )
        .expect("write");

    let preview = discover_session(&context, "s")
        .last_message_preview
        .expect("preview");
    assert_eq!(preview.chars().count(), 200);
    assert!(preview.ends_with('…'));
}

#[test]
fn last_message_preview_returns_none_without_messages() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(&PiSessionRecord::new("s", "/work").with_name("named"))
        .expect("write");

    assert!(
        discover_session(&context, "s")
            .last_message_preview
            .is_none()
    );
}

fn lineage_links(fragment: &GraphFragment) -> Vec<&GraphLink> {
    fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::ParentSession)
        .collect()
}

#[test]
fn fork_lineage_resolves_when_parent_session_is_on_disk() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let parent = fixture
        .write_pi_session(&PiSessionRecord::new("parent", "/work/a"))
        .expect("parent");
    // The child forked into another cwd, so it sits in another directory.
    fixture
        .write_pi_session(&PiSessionRecord::new("child", "/work/b").with_parent_session(&parent))
        .expect("child");

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    let links = lineage_links(&fragment);
    assert_eq!(links.len(), 1);
    let link = links[0];
    let scope = fixture.pi_state_root().to_string_lossy().to_string();
    assert_eq!(
        link.source,
        NodeId::AgentSession(AgentSessionId::new(HARNESS_KEY, &scope, "child"))
    );
    assert_eq!(
        link.target,
        LinkEndpoint::Node {
            id: NodeId::AgentSession(AgentSessionId::new(HARNESS_KEY, &scope, "parent")),
        }
    );
    let fields = &link.source_metadata.fields;
    assert_eq!(fields.get("harness_key"), Some(&json!("pi")));
    assert_eq!(fields.get(source_field::LINEAGE_KIND), Some(&json!("fork")));
    assert_eq!(fields.get("parent_session_id"), Some(&json!("parent")));
}

#[test]
fn fork_lineage_preserves_unresolved_parent_when_file_is_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let missing = "/home/user/.pi/agent/sessions/--gone--/2026-01-01T00-00-00-000Z_gone.jsonl";
    fixture
        .write_pi_session(&PiSessionRecord::new("child", "/work").with_parent_session(missing))
        .expect("child");

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    let links = lineage_links(&fragment);
    assert_eq!(links.len(), 1);
    let LinkEndpoint::Unresolved { evidence } = &links[0].target else {
        panic!("expected an unresolved parent: {links:#?}");
    };
    assert_eq!(evidence.harness_key.as_deref(), Some("pi"));
    assert_eq!(evidence.native_id.as_deref(), Some("gone"));
    assert_eq!(evidence.path.as_deref(), Some(missing));

    let output = crate::resolve::resolve_links(&fragment.candidate_links);
    assert!(output.resolved_relationships.is_empty());
    assert!(matches!(
        output.diagnostics.as_slice(),
        [crate::model::Diagnostic::UnresolvedEndpoint {
            relation: RelationKind::ParentSession,
            ..
        }]
    ));
}

#[test]
fn fork_lineage_keys_a_readable_parent_by_its_header_id() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    // The parent lives outside the scan, and its file name disagrees with
    // its header.
    let elsewhere = temp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("elsewhere");
    let parent = elsewhere.join("2026-01-02T03-04-05-000Z_renamed-file.jsonl");
    fs::write(
        &parent,
        "{\"type\":\"session\",\"version\":3,\"id\":\"header-id\",\"cwd\":\"/work\"}\n",
    )
    .expect("parent");
    fixture
        .write_pi_session(&PiSessionRecord::new("child", "/work").with_parent_session(&parent))
        .expect("child");

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    let links = lineage_links(&fragment);
    let LinkEndpoint::Unresolved { evidence } = &links[0].target else {
        panic!("parent is outside the scan: {links:#?}");
    };
    assert_eq!(evidence.native_id.as_deref(), Some("header-id"));
}

#[test]
fn self_fork_does_not_emit_lineage_cycle() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let own_path = fixture
        .pi_state_root()
        .join("--work--")
        .join("2026-01-02T03-04-05-000Z_self.jsonl");
    fixture
        .write_pi_session(&PiSessionRecord::new("self", "/work").with_parent_session(&own_path))
        .expect("self");

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    assert!(lineage_links(&fragment).is_empty());
}

#[test]
fn parent_paths_that_are_not_session_files_emit_no_lineage() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    for (id, parent) in [
        ("empty", ""),
        ("not-jsonl", "/tmp/notes.txt"),
        (
            "no-id-in-name",
            "/home/user/.pi/agent/sessions/--work--/notes.jsonl",
        ),
    ] {
        fixture
            .write_pi_session(&PiSessionRecord::new(id, "/work").with_parent_session(parent))
            .expect("write");
    }

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    assert!(lineage_links(&fragment).is_empty(), "{fragment:#?}");
}

#[test]
fn sessions_without_parent_session_emit_no_lineage() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_pi_session(&PiSessionRecord::new("s", "/work").with_exchange("q", "a"))
        .expect("write");

    let fragment = PiAdapter::new().discover(&context).expect("discover");
    assert!(lineage_links(&fragment).is_empty());
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
fn session_keys_are_extracted_from_pi_commands() {
    let extract = PI_RUNTIME_SIGNATURE.extract_session_keys;
    let id = "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d";
    for command in [
        format!("pi --session {id}"),
        format!("pi --fork {id}"),
        format!(
            "pi --session /home/user/.pi/agent/sessions/--work--/2026-10-16T10-00-00-000Z_{id}.jsonl"
        ),
    ] {
        assert_eq!(
            extract(&command).into_iter().collect::<Vec<_>>(),
            vec![id.to_string()],
            "{command}",
        );
    }
    // Custom `--session-id` values outside the UUID grammar are a
    // documented limitation.
    assert!(extract("pi --session my-custom.id").is_empty());
}

#[test]
fn rpc_mode_is_a_background_process() {
    assert!(pi_is_background_process("pi-rpc"));
    assert!(pi_is_background_process("/usr/local/bin/pi-rpc --mode rpc"));
    assert!(!pi_is_background_process("pi"));
    assert!(!pi_is_background_process("pi --session abc"));
    assert!(!pi_is_background_process("nvim pi-rpc.md"));
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
