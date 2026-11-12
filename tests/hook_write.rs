//! `conspectus hook write` run as a harness runs it: a payload on
//! stdin, in a scratch home with scratch XDG directories, a stand-in
//! `tmux` on `PATH`, and no daemon unless a test binds the socket.
//!
//! The command must never disturb the harness that waits on it: nothing
//! on stdout or stderr and exit status 0, whatever the payload or the
//! state of the stores; a state event touches no socket, starts no
//! process once the session has an entry, and never reaches the
//! attribution spool; and no payload text reaches any file. Fixtures are
//! synthetic.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assert_cmd::Command;

const SESSION: &str = "8f3c2a10-5b7e-4c1d-9a2f-0e6d4b8c7a15";

/// A scratch world for one test.
struct World {
    _root: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    state: PathBuf,
    runtime: PathBuf,
    bin: PathBuf,
    tmux_log: PathBuf,
}

impl World {
    fn new() -> Self {
        let root_dir = tempfile::tempdir().expect("scratch root");
        let root = fs::canonicalize(root_dir.path()).expect("canonical root");
        let home = root.join("home");
        let bin = root.join("bin");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&bin).expect("bin");
        let tmux_log = root.join("tmux.log");
        // Answers like a tmux pane in session `work`, and logs every
        // call so a test can count them. It never reaches a server.
        let tmux = bin.join("tmux");
        fs::write(
            &tmux,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in\n  *session_name*) echo work ;;\n  *pane_id*) echo %7 ;;\nesac\n",
                tmux_log.display()
            ),
        )
        .expect("tmux stand-in");
        fs::set_permissions(&tmux, fs::Permissions::from_mode(0o755)).expect("chmod tmux");
        Self {
            state: root.join("xdg/state"),
            runtime: root.join("xdg/run"),
            _root: root_dir,
            root,
            home,
            bin,
            tmux_log,
        }
    }

    /// `conspectus hook write <harness>` outside tmux, with nothing from
    /// the host environment.
    fn hook(&self, harness: &str) -> Command {
        let mut cmd = Command::cargo_bin("conspectus").expect("conspectus binary");
        cmd.env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.root.join("xdg/config"))
            .env("XDG_STATE_HOME", &self.state)
            .env("XDG_DATA_HOME", self.root.join("xdg/data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg/cache"))
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .args(["hook", "write", harness]);
        cmd
    }

    /// The same, inside tmux pane `%7` of a server at a scratch socket.
    fn hook_in_tmux(&self, harness: &str) -> Command {
        let mut cmd = self.hook(harness);
        cmd.env("TMUX", format!("{},4242,0", self.tmux_socket()))
            .env("TMUX_PANE", "%7");
        cmd
    }

    fn tmux_socket(&self) -> String {
        self.root.join("tmux-sock").display().to_string()
    }

    fn store_path(&self) -> PathBuf {
        self.state.join("conspectus/live-state.json")
    }

    fn spool_path(&self) -> PathBuf {
        self.state.join("conspectus/hooks/hooks-latest.json")
    }

    fn store(&self) -> serde_json::Value {
        read_json(&self.store_path())
    }

    fn entry(&self) -> serde_json::Value {
        self.store()["entries"][format!("claude-code:{SESSION}")].clone()
    }

    fn spool_record(&self) -> serde_json::Value {
        let spool = read_json(&self.spool_path());
        let records = spool["records"].as_object().expect("records");
        assert_eq!(records.len(), 1, "{spool}");
        records.values().next().cloned().expect("one record")
    }

    fn tmux_calls(&self) -> usize {
        fs::read_to_string(&self.tmux_log).map_or(0, |log| log.lines().count())
    }

    /// A daemon socket that is bound but never accepts a connection.
    fn stuck_daemon(&self) -> UnixListener {
        let path = self.runtime.join("conspectus/server.sock");
        fs::create_dir_all(path.parent().expect("socket dir")).expect("socket dir");
        UnixListener::bind(&path).expect("bind the daemon socket")
    }
}

fn read_json(path: &Path) -> serde_json::Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn session_start() -> String {
    serde_json::json!({
        "session_id": SESSION,
        "transcript_path": "/home/user/.claude/projects/-work-project/8f3c2a10.jsonl",
        "cwd": "/work/project",
        "hook_event_name": "SessionStart",
        "source": "startup",
        "model": "a-model",
        "permission_mode": "default",
    })
    .to_string()
}

fn stop() -> String {
    serde_json::json!({
        "session_id": SESSION,
        "transcript_path": "/home/user/.claude/projects/-work-project/8f3c2a10.jsonl",
        "cwd": "/work/project",
        "hook_event_name": "Stop",
        "stop_hook_active": false,
    })
    .to_string()
}

/// Run `cmd` with `stdin` and require the silent success the harness
/// needs.
fn run_silently(mut cmd: Command, stdin: impl Into<Vec<u8>>) {
    let output = cmd.write_stdin(stdin).output().expect("run hook write");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_session_start_records_live_state_and_attribution_without_a_word() {
    let world = World::new();
    run_silently(world.hook_in_tmux("claude-code"), session_start());

    let entry = world.entry();
    assert_eq!(entry["observation"]["state"], "ready", "{entry}");
    assert_eq!(entry["cwd"], "/work/project");
    assert_eq!(
        entry["transcript_path"],
        "/home/user/.claude/projects/-work-project/8f3c2a10.jsonl"
    );
    assert_eq!(
        entry["tmux"],
        serde_json::json!({
            "socket": world.tmux_socket(),
            "session": "work",
            "pane": "%7",
        })
    );

    let record = world.spool_record();
    assert_eq!(record["harness_key"], "claude-code");
    assert_eq!(record["session_key"], SESSION);
    assert_eq!(record["hook_event_name"], "SessionStart");
    assert_eq!(record["tmux"]["session_name"], "work");
    assert_eq!(record["tmux"]["pane_id"], "%7");
    // The attribution path asked tmux, as it does for every session
    // start; the live-state entry reused that answer.
    assert_eq!(world.tmux_calls(), 2);
}

#[test]
fn a_state_event_touches_no_socket_no_process_and_no_spool() {
    let world = World::new();
    run_silently(world.hook_in_tmux("claude-code"), session_start());
    let spool = fs::read(world.spool_path()).expect("spool");
    let tmux_calls = world.tmux_calls();
    let listener = world.stuck_daemon();
    listener.set_nonblocking(true).expect("nonblocking");

    run_silently(world.hook_in_tmux("claude-code"), stop());

    assert_eq!(world.tmux_calls(), tmux_calls, "tmux was not started");
    assert_eq!(fs::read(world.spool_path()).expect("spool"), spool);
    assert!(
        matches!(listener.accept(), Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "nothing connected to the daemon socket"
    );
    assert_eq!(world.entry()["tmux"]["session"], "work");
}

#[test]
fn a_bad_payload_is_dropped_without_a_word() {
    for (label, harness, payload) in [
        ("empty", "claude-code", String::new()),
        ("blank", "claude-code", " \n".to_string()),
        (
            "invalid JSON",
            "claude-code",
            "{\"session_id\": ".to_string(),
        ),
        (
            "no session id",
            "claude-code",
            r#"{"hook_event_name":"SessionStart"}"#.to_string(),
        ),
        (
            "a number for a session id",
            "claude-code",
            r#"{"hook_event_name":"SessionStart","session_id":7}"#.to_string(),
        ),
        ("an array", "claude-code", "[1, 2, 3]".to_string()),
        ("an unknown harness", "not-a-harness", session_start()),
    ] {
        let world = World::new();
        run_silently(world.hook(harness), payload);
        assert!(!world.store_path().exists(), "{label}: no live state");
        assert!(!world.spool_path().exists(), "{label}: no spool");
    }
}

#[test]
fn payload_bytes_that_are_not_utf8_are_dropped_without_a_word() {
    let world = World::new();
    run_silently(world.hook("claude-code"), vec![0xff, 0xfe, b'{', 0x80]);
    assert!(!world.store_path().exists());
}

#[test]
fn a_corrupt_store_is_replaced_without_a_word() {
    let world = World::new();
    fs::create_dir_all(world.store_path().parent().expect("dir")).expect("state dir");
    fs::write(world.store_path(), "{ this is not a store").expect("corrupt store");

    run_silently(world.hook("claude-code"), session_start());
    assert_eq!(world.entry()["observation"]["state"], "ready");
}

#[test]
fn a_locked_store_drops_the_event_and_still_records_the_session_start() {
    let world = World::new();
    let dir = world.store_path().parent().expect("dir").to_path_buf();
    fs::create_dir_all(&dir).expect("state dir");
    let lock = fs::File::create(dir.join("live-state.lock")).expect("lock file");
    lock.lock().expect("hold the store lock");

    let started = Instant::now();
    run_silently(world.hook("claude-code"), session_start());
    let waited = started.elapsed();
    drop(lock);

    assert!(!world.store_path().exists(), "the event was dropped");
    assert_eq!(world.spool_record()["session_key"], SESSION);
    assert!(waited < Duration::from_secs(3), "{waited:?}");
}

#[test]
fn an_unwritable_state_directory_is_no_error() {
    let world = World::new();
    let dir = world.state.join("conspectus");
    fs::create_dir_all(&dir).expect("state dir");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).expect("read-only");

    run_silently(world.hook("claude-code"), session_start());
    run_silently(world.hook("claude-code"), stop());
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("writable again");
}

#[test]
fn a_missing_tmux_is_no_error_and_the_pane_still_comes_from_tmux_pane() {
    let world = World::new();
    fs::remove_file(world.bin.join("tmux")).expect("remove the stand-in");
    let mut cmd = world.hook_in_tmux("claude-code");
    cmd.env("PATH", world.bin.display().to_string());

    run_silently(cmd, session_start());
    assert_eq!(
        world.entry()["tmux"],
        serde_json::json!({ "socket": world.tmux_socket(), "pane": "%7" })
    );
}

#[test]
fn a_stuck_daemon_costs_a_session_start_its_bound_and_the_spool_still_gets_the_record() {
    let world = World::new();
    let _listener = world.stuck_daemon();

    let started = Instant::now();
    run_silently(world.hook("claude-code"), session_start());
    let waited = started.elapsed();

    // The ingest bound is 2 s; an installed hook entry allows 5 s.
    assert!(waited >= Duration::from_millis(1500), "{waited:?}");
    assert!(waited < Duration::from_secs(4), "{waited:?}");
    assert_eq!(world.spool_record()["session_key"], SESSION);
    assert_eq!(world.entry()["observation"]["state"], "ready");
}

#[test]
fn no_payload_text_reaches_a_file_or_an_output() {
    let sentinel = "SENTINEL-hook-write-7d41c9";
    let world = World::new();
    let payloads = [
        serde_json::json!({
            "session_id": SESSION,
            "cwd": "/work/project",
            "transcript_path": "/work/project/t.jsonl",
            "hook_event_name": "SessionStart",
            "source": "startup",
            "session_title": sentinel,
            "prompt": sentinel,
            "message": sentinel,
            "tool_input": {"command": sentinel},
            "tool_response": {"stdout": sentinel, "status": sentinel},
        }),
        serde_json::json!({
            "session_id": SESSION,
            "hook_event_name": "UserPromptSubmit",
            "prompt": sentinel,
        }),
        serde_json::json!({
            "session_id": SESSION,
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_use_id": "toolu_01",
            "tool_input": {"command": sentinel},
            "tool_response": {"stdout": sentinel, "stderr": sentinel},
        }),
        serde_json::json!({
            "session_id": SESSION,
            "hook_event_name": "Notification",
            "notification_type": "permission_prompt",
            "message": sentinel,
            "title": sentinel,
        }),
        serde_json::json!({
            "session_id": SESSION,
            "hook_event_name": "Stop",
            "last_assistant_message": sentinel,
        }),
    ];
    for payload in payloads {
        let output = world
            .hook_in_tmux("claude-code")
            .write_stdin(payload.to_string())
            .output()
            .expect("run hook write");
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }

    assert!(world.store_path().is_file());
    assert!(world.spool_path().is_file());
    let mut files = Vec::new();
    collect_files(&world.state, &mut files);
    collect_files(&world.runtime, &mut files);
    for file in files {
        let bytes = fs::read(&file).expect("read state file");
        assert!(
            !String::from_utf8_lossy(&bytes).contains(sentinel),
            "{} holds payload text",
            file.display()
        );
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// A sentinel for payload text.
const TEXT_SENTINEL: &str = "SENTINEL-claude-fixture-text-5c1e9a";
/// A sentinel shaped like an id or a name, so the store's own id check
/// would let it through if a mapping routed it there.
const NAME_SENTINEL: &str = "SENTINELclaudename5c1e9a";

/// Fields the Claude Code mapping reads to identify the session or
/// classify the event; they keep their fixture value.
const READ_FIELDS: &[&str] = &[
    "session_id",
    "cwd",
    "transcript_path",
    "agent_transcript_path",
    "hook_event_name",
    "agent_id",
    "source",
    "notification_type",
    "trigger",
    "reason",
    "permission_mode",
    "stop_hook_active",
    "is_interrupt",
];

/// Tool names that classify an event: kept in the payload, never in a file.
const CLASSIFYING_TOOLS: &[&str] = &["AskUserQuestion", "ExitPlanMode", "Agent", "Task"];

/// Fields holding a name or an id rather than prose.
const NAME_FIELDS: &[&str] = &[
    "agent_type",
    "model",
    "prompt_id",
    "tool_use_id",
    "name",
    "subagent_type",
    "id",
    "filePath",
    "file_path",
];

/// The payload with a sentinel in every field that carries payload
/// rather than an id the mapping reads, every nested key included, and
/// the documented payload-bearing fields added where the fixture lacks
/// them.
fn plant_sentinels(payload: &mut serde_json::Value) {
    let event = payload["hook_event_name"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let fields = payload.as_object_mut().expect("payload object");
    let tool = fields
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    for (key, value) in fields.iter_mut() {
        match key.as_str() {
            key if READ_FIELDS.contains(&key) => {}
            "tool_name" if CLASSIFYING_TOOLS.contains(&tool.as_str()) => {}
            "tool_name" => *value = serde_json::json!(format!("mcp__{NAME_SENTINEL}__tool")),
            // Read to classify a failure, then dropped.
            "error" if event == "StopFailure" => {}
            "tool_response" if tool == "Agent" || tool == "Task" => {
                plant_except(value, &["status", "agentId"]);
            }
            "background_tasks" => {
                for task in value.as_array_mut().into_iter().flatten() {
                    plant_except(task, &["id", "type", "status"]);
                }
            }
            key => plant_all(value, key),
        }
    }
    for key in [
        "prompt",
        "message",
        "title",
        "last_assistant_message",
        "session_title",
        "error_details",
        "compact_summary",
        "custom_instructions",
    ] {
        fields
            .entry(key)
            .or_insert_with(|| serde_json::json!(TEXT_SENTINEL));
    }
    for key in ["agent_type", "model", "prompt_id", "tool_use_id"] {
        fields
            .entry(key)
            .or_insert_with(|| serde_json::json!(NAME_SENTINEL));
    }
    fields
        .entry("tool_input")
        .or_insert_with(|| serde_json::json!({ "command": TEXT_SENTINEL }));
    fields.entry("permission_suggestions").or_insert_with(|| {
        serde_json::json!([{ "type": "addRules", "rules": [{ "toolName": NAME_SENTINEL, "ruleContent": TEXT_SENTINEL }] }])
    });
    if event != "StopFailure" {
        fields
            .entry("error")
            .or_insert_with(|| serde_json::json!(TEXT_SENTINEL));
    }
}

fn plant_except(value: &mut serde_json::Value, keep: &[&str]) {
    if let Some(map) = value.as_object_mut() {
        for (key, item) in map.iter_mut() {
            if !keep.contains(&key.as_str()) {
                plant_all(item, key);
            }
        }
    }
}

fn plant_all(value: &mut serde_json::Value, key: &str) {
    match value {
        serde_json::Value::String(text) => {
            let sentinel = if NAME_FIELDS.contains(&key) {
                NAME_SENTINEL
            } else {
                TEXT_SENTINEL
            };
            *text = sentinel.to_string();
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|item| plant_all(item, key)),
        serde_json::Value::Object(map) => {
            // A nested key can be text too (an answer keyed by its
            // question).
            let planted = std::mem::take(map)
                .into_iter()
                .enumerate()
                .map(|(index, (key, mut item))| {
                    plant_all(&mut item, &key);
                    let key = if NAME_FIELDS.contains(&key.as_str()) || key.len() < 24 {
                        key
                    } else {
                        format!("{TEXT_SENTINEL}-{index}")
                    };
                    (key, item)
                })
                .collect();
            *map = planted;
        }
        _ => {}
    }
}

fn claude_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hooks/claude-code")
}

/// Every Claude Code fixture payload in file order: the single payloads,
/// then each sequence's steps.
fn claude_fixture_payloads() -> Vec<(String, serde_json::Value)> {
    let mut names: Vec<PathBuf> = fs::read_dir(claude_fixture_dir())
        .expect("fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .collect();
    names.sort();
    let mut payloads = Vec::new();
    for path in names
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
    {
        payloads.push((path.display().to_string(), read_json(path)));
    }
    for path in names
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
    {
        let text = fs::read_to_string(path).expect("sequence");
        for (index, line) in text.lines().enumerate().skip(1) {
            let step: serde_json::Value = serde_json::from_str(line).expect("step");
            if let Some(payload) = step.get("payload") {
                payloads.push((format!("{}:{}", path.display(), index + 1), payload.clone()));
            }
        }
    }
    payloads
}

/// The first sentinel or classifying name in `text`, if any.
fn leaked(text: &str) -> Option<String> {
    let quoted = CLASSIFYING_TOOLS
        .iter()
        .chain(&["rate_limit", "authentication_failed"])
        .map(|name| format!("\"{name}\""));
    [TEXT_SENTINEL.to_string(), NAME_SENTINEL.to_string()]
        .into_iter()
        .chain(quoted)
        .find(|needle| text.contains(needle.as_str()))
}

#[test]
fn no_payload_text_of_any_claude_code_fixture_reaches_a_file_or_an_output() {
    let world = World::new();
    let payloads = claude_fixture_payloads();
    assert!(payloads.len() > 100, "{}", payloads.len());
    for (place, mut payload) in payloads {
        plant_sentinels(&mut payload);
        let planted = payload.to_string();
        assert!(planted.contains(TEXT_SENTINEL), "{place}: nothing planted");
        let output = world
            .hook_in_tmux("claude-code")
            .write_stdin(planted)
            .output()
            .expect("run hook write");
        assert_eq!(output.status.code(), Some(0), "{place}");
        assert!(
            output.stdout.is_empty() && output.stderr.is_empty(),
            "{place} was not silent"
        );
        // Every file after every event, since a later event can drop
        // what an earlier one leaked.
        let mut files = Vec::new();
        collect_files(&world.state, &mut files);
        collect_files(&world.runtime, &mut files);
        for file in files {
            let text =
                String::from_utf8_lossy(&fs::read(&file).expect("read state file")).into_owned();
            if let Some(needle) = leaked(&text) {
                panic!("after {place}, {} holds {needle}", file.display());
            }
        }
    }

    // The writes happened: the fixtures' session has an entry, and its
    // session starts reached the spool.
    let store = world.store();
    let entry = &store["entries"]["claude-code:3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f"];
    assert!(entry["observation"]["state"].is_string(), "{store}");
    assert!(world.spool_path().is_file());
}

/// The median wall time of `runs` runs of `payload`, outside tmux and
/// with no daemon.
fn median_run(world: &World, payload: &str, runs: usize) -> Duration {
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let started = Instant::now();
            let output = world
                .hook("claude-code")
                .write_stdin(payload)
                .output()
                .expect("run hook write");
            let elapsed = started.elapsed();
            assert_eq!(output.status.code(), Some(0));
            elapsed
        })
        .collect();
    times.sort();
    times[times.len() / 2]
}

#[test]
fn an_event_costs_a_few_milliseconds() {
    let world = World::new();
    let start = median_run(&world, &session_start(), 50);
    let state = median_run(&world, &stop(), 50);
    eprintln!("hook write median over 50 runs: session start {start:?}, state event {state:?}");
    // Loose, so a loaded machine does not fail the suite: the measured
    // medians are a few milliseconds.
    assert!(start < Duration::from_millis(100), "{start:?}");
    assert!(state < Duration::from_millis(100), "{state:?}");
}
