//! The hook trace (`CONSPECTUS_HOOK_TRACE`) as a harness meets it: the
//! real `conspectus hook write` binary, a payload on stdin, a scratch home
//! and scratch XDG directories, and a stand-in `tmux` on `PATH`.
//!
//! The trace must never change the hook: nothing on stdout or stderr,
//! exit status 0, the same store, no process or socket the hook did not
//! already use. And it keeps no free text: every hook fixture of all four
//! harnesses runs through the command with the trace on, and no sentinel,
//! raw session id, path, or text reaches the trace file.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use conspectus::hook::redact::pseudonym;
use serde_json::{Value, json};

#[path = "support/program.rs"]
mod program;

const TRACE: &str = "recording-1";
const SENTINEL: &str = "SENTINEL";

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
        // Answers like a tmux pane in session `work`, and logs every call
        // so a test can count them. It never reaches a server.
        program::write_program(
            bin.join("tmux"),
            &format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in\n  *session_name*) echo work ;;\n  *pane_id*) echo %7 ;;\nesac\n",
                tmux_log.display()
            ),
        );
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
    /// the host environment, and no trace.
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

    /// The same, tracing to the trace called `name`.
    fn traced(&self, harness: &str, name: &str) -> Command {
        let mut cmd = self.hook(harness);
        cmd.env("CONSPECTUS_HOOK_TRACE", name);
        cmd
    }

    /// The same, inside tmux pane `%7` of a server at a scratch socket.
    fn traced_in_tmux(&self, harness: &str, name: &str) -> Command {
        let mut cmd = self.traced(harness, name);
        cmd.env(
            "TMUX",
            format!("{},4242,0", self.root.join("tmux-sock").display()),
        )
        .env("TMUX_PANE", "%7");
        cmd
    }

    fn conspectus_dir(&self) -> PathBuf {
        self.state.join("conspectus")
    }

    fn trace_dir(&self) -> PathBuf {
        self.conspectus_dir().join("hook-trace")
    }

    fn trace_path(&self, name: &str) -> PathBuf {
        self.trace_dir().join(format!("{name}.jsonl"))
    }

    fn trace_text(&self, name: &str) -> String {
        fs::read_to_string(self.trace_path(name)).unwrap_or_default()
    }

    /// The lines of a trace, each parsed.
    fn trace_lines(&self, name: &str) -> Vec<Value> {
        self.trace_text(name)
            .lines()
            .map(|line| serde_json::from_str(line).expect("a trace line is one JSON value"))
            .collect()
    }

    fn store(&self) -> Value {
        read_json(&self.conspectus_dir().join("live-state.json"))
    }

    fn entry(&self, harness: &str, session: &str) -> Value {
        self.store()["entries"][format!("{harness}:{session}")].clone()
    }

    fn spool_path(&self) -> PathBuf {
        self.conspectus_dir().join("hooks/hooks-latest.json")
    }

    fn tmux_calls(&self) -> usize {
        fs::read_to_string(&self.tmux_log).map_or(0, |log| log.lines().count())
    }

    /// Everything under the state directory, as paths relative to it.
    fn state_files(&self) -> BTreeSet<String> {
        let mut out = Vec::new();
        collect_files(&self.state, &mut out);
        out.iter()
            .map(|path| {
                path.strip_prefix(&self.state)
                    .expect("under the state directory")
                    .display()
                    .to_string()
            })
            .collect()
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

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|err| panic!("parse {}: {err}", path.display()))
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .unwrap_or_else(|err| panic!("stat {}: {err}", path.display()))
        .permissions()
        .mode()
        & 0o777
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

fn fixture_dir(corpus: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hooks")
        .join(corpus)
}

fn fixture(corpus: &str, name: &str) -> Value {
    read_json(&fixture_dir(corpus).join(name))
}

/// The session id of the Claude Code fixtures.
const SESSION: &str = "3f2a9c1e-7b4d-4e8a-9c6f-1a2b3c4d5e6f";

// ---------------------------------------------------------------------
// Off, and an invalid name
// ---------------------------------------------------------------------

#[test]
fn without_the_variable_no_trace_is_written_and_the_hook_is_as_before() {
    let world = World::new();
    run_silently(
        world.hook("claude-code"),
        fixture("claude-code", "userpromptsubmit.json").to_string(),
    );
    assert!(!world.trace_dir().exists(), "{:?}", world.state_files());
    assert_eq!(
        world.entry("claude-code", SESSION)["observation"]["state"],
        "working"
    );
}

#[test]
fn a_name_that_does_not_match_writes_no_trace() {
    let long = "n".repeat(65);
    for name in [
        "",
        ".hidden",
        "-flag",
        "..",
        "a/b",
        "../escape",
        "has space",
        "line\nbreak",
        "caf\u{e9}",
        long.as_str(),
    ] {
        let world = World::new();
        run_silently(
            world.traced("claude-code", name),
            fixture("claude-code", "userpromptsubmit.json").to_string(),
        );
        assert!(
            !world.trace_dir().exists(),
            "{name:?} made a trace: {:?}",
            world.state_files()
        );
        // The hook itself ran.
        assert_eq!(
            world.entry("claude-code", SESSION)["observation"]["state"],
            "working",
            "{name:?}"
        );
        // Nothing but the store's own files, wherever the name pointed.
        let files = world.state_files();
        assert!(
            files
                .iter()
                .all(|file| file.starts_with("conspectus/live-state")),
            "{name:?}: {files:?}"
        );
        assert!(!world.root.join("escape.jsonl").exists());
    }
}

// ---------------------------------------------------------------------
// What a line says
// ---------------------------------------------------------------------

/// The `after` a line should carry for the store's `entry`.
fn after_of(entry: &Value) -> Value {
    let observation = &entry["observation"];
    let mut after = json!({
        "state": observation["state"],
        "subagents": observation.get("subagents").cloned().unwrap_or_else(|| json!(0)),
        "via_subagent": observation.get("via_subagent").cloned().unwrap_or(json!(false)),
        "since_ms": observation["since_ms"],
        "rev": entry["rev"],
    });
    if let Some(detail) = observation.get("detail") {
        after["detail"] = detail.clone();
    }
    after
}

#[test]
fn each_call_appends_one_line_that_ties_the_event_to_the_state_it_produced() {
    let world = World::new();
    let steps = [
        ("sessionstart-startup.json", vec!["session_started"]),
        ("userpromptsubmit.json", vec!["turn_started"]),
        (
            "permissionrequest-bash.json",
            vec!["wait_started:permission"],
        ),
        ("posttooluse-bash.json", vec!["tool_finished"]),
        (
            "stop.json",
            vec!["turn_ended:finished", "background_work:0"],
        ),
    ];
    let mut states = Vec::new();
    for (index, (file, events)) in steps.iter().enumerate() {
        let payload = fixture("claude-code", file);
        run_silently(
            world.traced_in_tmux("claude-code", TRACE),
            payload.to_string(),
        );
        let lines = world.trace_lines(TRACE);
        assert_eq!(lines.len(), index + 1, "{file}");
        let line = &lines[index];

        assert_eq!(line["v"], 1);
        assert_eq!(line["harness"], "claude-code");
        assert!(line["t_ms"].as_i64().unwrap() > 1_700_000_000_000, "{line}");
        assert_eq!(line["events"], json!(events), "{file}");
        assert_eq!(line["applied"], "written", "{file}");
        assert_eq!(
            line["env"],
            json!({ "TMUX_PANE": true, "TMUX": true, "CLAUDE_PID": false })
        );
        // The line carries the state the store holds right after the event.
        let entry = world.entry("claude-code", SESSION);
        assert_eq!(line["after"], after_of(&entry), "{file}: {line}");
        assert_eq!(line["after"]["rev"], (index + 1) as u64);
        states.push(entry["observation"]["state"].as_str().unwrap().to_string());
        // The payload is there, redacted.
        assert_eq!(
            line["payload"]["hook_event_name"],
            payload["hook_event_name"]
        );
        assert_eq!(line["payload"]["session_id"], pseudonym(TRACE, SESSION));
        assert_eq!(line["payload"]["cwd"], "<path>");
    }
    // The sequence took the session through the states the reducer owes it.
    assert_eq!(states, ["ready", "working", "waiting", "working", "ready"]);
}

#[test]
fn the_environment_flags_say_what_was_set_and_nothing_else() {
    let world = World::new();
    let payload = fixture("claude-code", "userpromptsubmit.json").to_string();
    run_silently(world.traced("claude-code", TRACE), payload.clone());
    let mut with_pid = world.traced("claude-code", TRACE);
    // The pid names no running process, so the writer ignores it; the flag
    // only says the variable was set.
    with_pid.env("CLAUDE_PID", "2147483646");
    run_silently(with_pid, payload);
    let lines = world.trace_lines(TRACE);
    assert_eq!(
        lines[0]["env"],
        json!({ "TMUX_PANE": false, "TMUX": false, "CLAUDE_PID": false })
    );
    assert_eq!(
        lines[1]["env"],
        json!({ "TMUX_PANE": false, "TMUX": false, "CLAUDE_PID": true })
    );
    assert!(!world.trace_text(TRACE).contains("2147483646"));
}

#[test]
fn a_plugin_snapshot_is_traced_as_a_snapshot() {
    let world = World::new();
    let pi = json!({
        "session_id": "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d",
        "cwd": "/home/user/projects/my-project",
        "transcript_path": "/home/user/.pi/agent/sessions/x.jsonl",
        "hook_event_name": "live:agent_start",
        "live": {
            "instance": "mhv8k2a0-3f9c1e7a5b2d4c60", "seq": 2, "observed_ms": 2000,
            "heartbeat_s": 60, "pid": 48213, "pid_start": 771_234_567,
            "state": "working", "detail": "generating", "subagents": 0, "since_ms": 2000,
        },
    });
    run_silently(world.traced("pi", TRACE), pi.to_string());
    let opencode = json!({
        "session_id": "ses_7Kq2mX9vB4nL1pZ8",
        "cwd": "/home/user/projects/my-project",
        "hook_event_name": "session.status",
        "attribution": false,
        "live": {
            "instance": "lq2c2a00-6c0a4e8d2b7f1c53", "seq": 3, "observed_ms": 3000,
            "heartbeat_s": 60, "pid": 52817, "pid_start": 918_273_645,
            "state": "waiting", "detail": "permission", "subagents": 1,
            "via_subagent": true, "since_ms": 2900,
        },
    });
    run_silently(world.traced("opencode", TRACE), opencode.to_string());

    let lines = world.trace_lines(TRACE);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["harness"], "pi");
    assert_eq!(lines[0]["events"], json!(["snapshot"]));
    assert_eq!(lines[0]["applied"], "written");
    assert_eq!(lines[0]["after"]["state"], "working");
    assert_eq!(lines[0]["after"]["detail"], "generating");
    assert_eq!(lines[0]["payload"]["live"]["state"], "working");
    assert_eq!(
        lines[0]["payload"]["live"]["instance"],
        pseudonym(TRACE, "mhv8k2a0-3f9c1e7a5b2d4c60")
    );
    assert_eq!(lines[0]["payload"]["live"]["seq"], 2);
    assert_eq!(lines[1]["harness"], "opencode");
    assert_eq!(lines[1]["events"], json!(["snapshot"]));
    assert_eq!(lines[1]["after"]["state"], "waiting");
    assert_eq!(lines[1]["after"]["detail"], "permission");
    assert_eq!(lines[1]["after"]["subagents"], 1);
    assert_eq!(lines[1]["after"]["via_subagent"], true);
}

#[test]
fn a_call_the_store_does_not_take_is_still_traced() {
    let world = World::new();
    // Codex events that map to nothing, an unregistered harness, and
    // input that is not JSON.
    run_silently(
        world.traced("codex", TRACE),
        fixture("codex", "pretooluse-request-user-input.json").to_string(),
    );
    run_silently(
        world.traced("no-such-harness", TRACE),
        json!({ "session_id": "s1", "prompt": "a secret prompt" }).to_string(),
    );
    run_silently(
        world.traced("claude-code", TRACE),
        "this is not json, a secret",
    );
    run_silently(world.traced("claude code; echo hi", TRACE), "{}");

    let lines = world.trace_lines(TRACE);
    assert_eq!(lines.len(), 4);
    for line in &lines {
        assert_eq!(line["events"], json!([]), "{line}");
        assert_eq!(line["applied"], "none", "{line}");
        assert!(line.get("after").is_none(), "{line}");
    }
    assert_eq!(lines[0]["harness"], "codex");
    assert_eq!(lines[0]["payload"]["tool_name"], "request_user_input");
    assert_eq!(lines[1]["harness"], "no-such-harness");
    assert_eq!(lines[1]["payload"]["prompt"], "<str:15>");
    assert_eq!(lines[2]["payload"], "<unparsed:26 bytes>");
    assert_eq!(lines[3]["harness"], "<invalid>");
    let text = world.trace_text(TRACE);
    assert!(
        !text.contains("secret") && !text.contains("echo hi"),
        "{text}"
    );
}

#[test]
fn a_trace_is_private_and_lines_append() {
    let world = World::new();
    for _ in 0..3 {
        run_silently(
            world.traced("claude-code", TRACE),
            fixture("claude-code", "stop.json").to_string(),
        );
    }
    assert_eq!(mode(&world.trace_dir()), 0o700);
    assert_eq!(mode(&world.trace_path(TRACE)), 0o600);
    assert_eq!(world.trace_lines(TRACE).len(), 3);
    // Another trace is another file, and the first is left alone.
    let first = world.trace_text(TRACE);
    run_silently(
        world.traced("claude-code", "recording-2"),
        fixture("claude-code", "stop.json").to_string(),
    );
    assert_eq!(world.trace_text(TRACE), first);
    assert_eq!(world.trace_lines("recording-2").len(), 1);
    assert_eq!(
        world.trace_dir().read_dir().unwrap().count(),
        2,
        "one file per trace"
    );
}

#[test]
fn a_trace_dir_given_by_state_root_follows_the_store() {
    let world = World::new();
    let root = world.root.join("custom-root");
    let mut cmd = world.traced("claude-code", TRACE);
    cmd.args(["--state-root"]).arg(&root);
    run_silently(cmd, fixture("claude-code", "stop.json").to_string());
    assert!(root.join("live-state.json").is_file());
    assert!(
        root.join("hook-trace")
            .join(format!("{TRACE}.jsonl"))
            .is_file()
    );
    assert!(!world.trace_dir().exists());
}

#[test]
fn two_traces_name_one_session_by_two_pseudonyms() {
    let world = World::new();
    let payload = fixture("claude-code", "stop.json").to_string();
    run_silently(world.traced("claude-code", "recording-1"), payload.clone());
    run_silently(world.traced("claude-code", "recording-2"), payload.clone());
    run_silently(world.traced("claude-code", "recording-1"), payload);
    let first = world.trace_lines("recording-1");
    let second = world.trace_lines("recording-2");
    assert_eq!(
        first[0]["payload"]["session_id"],
        first[1]["payload"]["session_id"]
    );
    assert_ne!(
        first[0]["payload"]["session_id"],
        second[0]["payload"]["session_id"]
    );
    assert_eq!(
        first[0]["payload"]["session_id"],
        pseudonym("recording-1", SESSION)
    );
    assert_eq!(
        second[0]["payload"]["session_id"],
        pseudonym("recording-2", SESSION)
    );
}

// ---------------------------------------------------------------------
// The hook is not disturbed
// ---------------------------------------------------------------------

/// The store's entries without the times a second run would change.
fn entries_without_times(world: &World) -> Value {
    let mut store = world.store();
    store["rev"] = json!(0);
    for entry in store["entries"]
        .as_object_mut()
        .expect("entries")
        .values_mut()
    {
        entry["changed_ms"] = json!(0);
        for key in ["since_ms", "observed_ms"] {
            entry["observation"][key] = json!(0);
        }
        if let Some(waits) = entry.get_mut("reducer") {
            *waits = json!({});
        }
    }
    store
}

#[test]
fn an_unwritable_trace_leaves_the_hook_as_it_was() {
    let control = World::new();
    let payload = fixture("claude-code", "userpromptsubmit.json").to_string();
    run_silently(control.hook("claude-code"), payload.clone());

    // A file stands where the directory belongs.
    let blocked_dir = World::new();
    fs::create_dir_all(blocked_dir.conspectus_dir()).unwrap();
    fs::write(blocked_dir.trace_dir(), "in the way").unwrap();
    run_silently(blocked_dir.traced("claude-code", TRACE), payload.clone());
    assert_eq!(
        fs::read_to_string(blocked_dir.trace_dir()).unwrap(),
        "in the way"
    );

    // A directory stands where the file belongs.
    let blocked_file = World::new();
    fs::create_dir_all(blocked_file.trace_path(TRACE)).unwrap();
    run_silently(blocked_file.traced("claude-code", TRACE), payload.clone());

    // The state directory itself is a file: neither the store nor the
    // trace can be written, and the hook still says nothing.
    let no_state = World::new();
    fs::create_dir_all(no_state.state.clone()).unwrap();
    fs::write(no_state.conspectus_dir(), "not a directory").unwrap();
    run_silently(no_state.traced("claude-code", TRACE), payload);

    let expected = entries_without_times(&control);
    assert_eq!(entries_without_times(&blocked_dir), expected);
    assert_eq!(entries_without_times(&blocked_file), expected);
    assert_eq!(
        world_state_state(&control),
        "working",
        "the control wrote its store"
    );
    assert_eq!(
        fs::read_to_string(no_state.conspectus_dir()).unwrap(),
        "not a directory"
    );
}

fn world_state_state(world: &World) -> String {
    world.entry("claude-code", SESSION)["observation"]["state"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_traced_state_event_starts_no_process_and_opens_no_socket() {
    let world = World::new();
    run_silently(
        world.traced_in_tmux("claude-code", TRACE),
        fixture("claude-code", "sessionstart-startup.json").to_string(),
    );
    let spool = fs::read(world.spool_path()).expect("spool");
    let tmux_calls = world.tmux_calls();
    assert!(tmux_calls > 0, "the session start asked tmux");
    let socket = world.runtime.join("conspectus/server.sock");
    fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(&socket).expect("bind the daemon socket");
    listener.set_nonblocking(true).unwrap();

    run_silently(
        world.traced_in_tmux("claude-code", TRACE),
        fixture("claude-code", "stop.json").to_string(),
    );

    assert_eq!(world.tmux_calls(), tmux_calls, "tmux was not started");
    assert_eq!(fs::read(world.spool_path()).expect("spool"), spool);
    assert!(
        matches!(listener.accept(), Err(err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "nothing connected to the daemon socket"
    );
    assert_eq!(world.trace_lines(TRACE).len(), 2);
}

/// The median wall time of `runs` runs of `payload` with `name` as the
/// trace, or none.
fn median_run(world: &World, payload: &str, name: Option<&str>, runs: usize) -> Duration {
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let cmd = match name {
                Some(name) => world.traced("claude-code", name),
                None => world.hook("claude-code"),
            };
            let started = Instant::now();
            let output = { cmd }
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
fn a_trace_adds_little_to_a_hook_call() {
    let world = World::new();
    let big = {
        let mut payload = fixture("claude-code", "posttooluse-bash.json");
        payload["tool_response"]["stdout"] = json!("x".repeat(200_000));
        payload.to_string()
    };
    for (what, payload) in [
        (
            "state event",
            fixture("claude-code", "stop.json").to_string(),
        ),
        ("large tool response", big),
    ] {
        let off = median_run(&world, &payload, None, 30);
        let on = median_run(&world, &payload, Some(TRACE), 30);
        eprintln!("hook write median over 30 runs, {what}: off {off:?}, on {on:?}");
        // Loose, so a loaded machine does not fail the suite.
        assert!(
            on < off + Duration::from_millis(100),
            "{what}: {off:?} then {on:?}"
        );
    }
}

// ---------------------------------------------------------------------
// No free text
// ---------------------------------------------------------------------

#[test]
fn an_unknown_future_field_is_redacted() {
    let world = World::new();
    let mut payload = fixture("claude-code", "userpromptsubmit.json");
    payload["future_field"] = json!("SENTINEL-future-text");
    payload["future_object"] = json!({
        "inner": "SENTINEL-inner-text",
        "list": ["SENTINEL-in-list", { "deep": "SENTINEL-deep-text" }],
        "SENTINEL odd key": "SENTINEL-odd-value",
        "a": { "b": { "c": { "d": "SENTINEL-too-deep" } } },
    });
    payload["SENTINEL future key"] = json!("SENTINEL-value");
    payload["many"] = json!((0..20).map(|n| format!("SENTINEL-{n}")).collect::<Vec<_>>());
    run_silently(world.traced("claude-code", TRACE), payload.to_string());

    let text = world.trace_text(TRACE);
    assert!(!text.contains(SENTINEL), "{text}");
    let line = &world.trace_lines(TRACE)[0];
    assert_eq!(line["payload"]["future_field"], "<str:20>");
    assert_eq!(
        line["payload"]["future_object"]["list"][1]["deep"],
        "<str:18>"
    );
    assert_eq!(line["payload"]["many"].as_array().unwrap().len(), 9);
    assert_eq!(line["payload"]["many"][8], "<+12 more>");
    // The prompt of the fixture is a size, and no word of it is there.
    assert_eq!(line["payload"]["prompt"], "<str:41>");
    assert!(!text.contains("Refactor"), "{text}");
}

/// Keys whose strings the trace keeps when they look like words, so a
/// payload may hold them as they are. Written out here on purpose: the
/// test names the vocabulary independently of the code under test.
const VOCABULARY_KEYS: &[&str] = &[
    "hook_event_name",
    "source",
    "trigger",
    "reason",
    "notification_type",
    "permission_mode",
    "error",
    "status",
    "state",
    "detail",
    "kind",
    "type",
    "stop_reason",
    "error_class",
    "mode",
    "level",
    "behavior",
    "tool_name",
];

fn is_vocabulary_key(key: &str) -> bool {
    VOCABULARY_KEYS.contains(&key)
        || key == "version"
        || key.ends_with("_version")
        || key.ends_with("Version")
}

fn word_shaped(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 48
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}

/// Whether a string at `key` may reach the trace as it is.
fn may_stay(key: Option<&str>, text: &str) -> bool {
    key.is_some_and(|key| {
        is_vocabulary_key(key)
            && word_shaped(text)
            && !(key == "tool_name" && text.starts_with("mcp__"))
    })
}

/// Every string in `value` that must not reach the trace: each string
/// value that may not stay, and each key that is not shaped like one.
fn forbidden(value: &Value, key: Option<&str>, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if !may_stay(key, text) {
                out.insert(text.clone());
            }
        }
        Value::Array(items) => items.iter().for_each(|item| forbidden(item, key, out)),
        Value::Object(map) => {
            for (name, item) in map {
                if name.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                    out.insert(name.clone());
                }
                forbidden(item, Some(name), out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Every string in `value`, keys included.
fn every_string(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            out.insert(text.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| every_string(item, out)),
        Value::Object(map) => {
            for (name, item) in map {
                out.insert(name.clone());
                every_string(item, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// The strings of `value` that may stay, with the keys they sit under.
fn allowed(value: &Value, key: Option<&str>, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if may_stay(key, text) {
                out.insert(text.clone());
            }
        }
        Value::Array(items) => items.iter().for_each(|item| allowed(item, key, out)),
        Value::Object(map) => map
            .iter()
            .for_each(|(name, item)| allowed(item, Some(name), out)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// A copy of `payload` with a sentinel in every string that may not stay
/// (ids, paths, text, odd keys), and fields no one has seen added.
fn plant(payload: &Value, counter: &mut usize) -> Value {
    fn walk(value: &Value, key: Option<&str>, counter: &mut usize) -> Value {
        match value {
            Value::String(text) if may_stay(key, text) => value.clone(),
            Value::String(text) if key == Some("tool_name") && text.starts_with("mcp__") => {
                *counter += 1;
                json!(format!("mcp__SENTINEL-planted-{counter}__tool"))
            }
            Value::String(_) => {
                *counter += 1;
                json!(format!("SENTINEL-planted-{counter}"))
            }
            Value::Array(items) => {
                Value::Array(items.iter().map(|item| walk(item, key, counter)).collect())
            }
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(name, item)| (name.clone(), walk(item, Some(name), counter)))
                    .collect(),
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        }
    }
    let mut planted = walk(payload, None, counter);
    if let Some(fields) = planted.as_object_mut() {
        fields.insert("future_field".to_string(), json!("SENTINEL-future"));
        fields.insert(
            "future_object".to_string(),
            json!({
                "inner": "SENTINEL-inner",
                "list": ["SENTINEL-in-list", { "SENTINEL odd key": "SENTINEL-odd" }],
                "a": { "b": { "c": { "d": "SENTINEL-too-deep" } } },
            }),
        );
        fields.insert("SENTINEL top level key".to_string(), json!("SENTINEL-top"));
    }
    planted
}

/// Every fixture payload of `corpus` in file order: single payloads, then
/// the payload of each sequence step. A corpus of event scripts gives the
/// payloads its plugin emits and the events it saw.
fn corpus_payloads(corpus: &str) -> Vec<(String, Value)> {
    let mut names: Vec<PathBuf> = fs::read_dir(fixture_dir(corpus))
        .expect("fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.is_file())
        .collect();
    names.sort();
    let mut payloads = Vec::new();
    for path in &names {
        let place = path.file_name().unwrap().to_string_lossy().to_string();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => payloads.push((place, read_json(path))),
            Some("jsonl") => {
                let text = fs::read_to_string(path).expect("sequence");
                for (index, line) in text.lines().enumerate().skip(1) {
                    let step: Value = serde_json::from_str(line).expect("step");
                    let place = format!("{place}:{}", index + 1);
                    if let Some(payload) = step.get("payload") {
                        payloads.push((place.clone(), payload.clone()));
                    }
                    for payload in step["emit"].as_array().into_iter().flatten() {
                        payloads.push((place.clone(), payload.clone()));
                    }
                    // The events a plugin sees, as one that forwarded them
                    // whole would send them.
                    if let Some(event) = step.get("event") {
                        let mut forwarded = event.clone();
                        if corpus == "pi-live" {
                            forwarded["session_id"] = json!("0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
                            forwarded["hook_event_name"] = json!("live:forwarded");
                        }
                        payloads.push((format!("{place} event"), forwarded));
                    }
                }
            }
            _ => {}
        }
    }
    payloads
}

/// Run `payloads` through the command with the trace on and check each
/// line as it lands. `plant_text` runs each a second time with a sentinel
/// in everything that may not stay.
fn run_corpus(world: &World, harness: &str, corpus: &str, name: &str) -> usize {
    let mut inputs: Vec<(String, Value)> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut counter = 0;
    for (place, payload) in corpus_payloads(corpus) {
        let planted = plant(&payload, &mut counter);
        for (place, payload) in [
            (place.clone(), payload),
            (format!("{place} planted"), planted),
        ] {
            if seen.insert(payload.to_string()) {
                inputs.push((place, payload));
            }
        }
    }
    assert!(
        inputs.len() > 40,
        "{corpus}: only {} payloads",
        inputs.len()
    );
    let mut done = 0;
    for (place, payload) in &inputs {
        // The payload as the harness sends it, with its text-bearing fields.
        run_silently(world.traced_in_tmux(harness, name), payload.to_string());
        done += 1;
        let lines = world.trace_lines(name);
        let line = lines.last().expect("a line for each call");
        let text = line.to_string();
        assert_eq!(lines.len(), done, "{corpus} {place}: one line per call");
        assert_eq!(line["harness"], harness);

        // What may be there: the line's own words, and the payload's
        // vocabulary. Nothing else of the payload's strings.
        let mut own = BTreeSet::new();
        let mut shell = line.clone();
        shell.as_object_mut().unwrap().remove("payload");
        every_string(&shell, &mut own);
        let mut kept = BTreeSet::new();
        allowed(payload, None, &mut kept);
        let mut bad = BTreeSet::new();
        forbidden(payload, None, &mut bad);
        for text_in_payload in &bad {
            if kept.contains(text_in_payload)
                || own.contains(text_in_payload)
                || text_in_payload.is_empty()
            {
                continue;
            }
            let quoted = Value::String(text_in_payload.clone()).to_string();
            assert!(
                !text.contains(&quoted),
                "{corpus} {place}: {quoted} reached the trace: {text}"
            );
        }
        assert!(!text.contains(SENTINEL), "{corpus} {place}: {text}");
        if let Some(session) = payload["session_id"].as_str() {
            assert_eq!(
                line["payload"]["session_id"],
                pseudonym(name, session),
                "{corpus} {place}"
            );
        }
    }
    done
}

/// Run every fixture payload of `corpus` through `harness`'s command with
/// the trace on, and check the whole trace file once more at the end.
fn no_free_text_of_a_corpus_reaches_the_trace(harness: &str, corpus: &str, raw: &[&str]) {
    let world = World::new();
    let name = format!("fixtures-{corpus}");
    let total = run_corpus(&world, harness, corpus, &name);
    eprintln!("{total} {corpus} payloads ran through hook write with the trace on");
    // The whole file, once more: nothing planted, nothing raw.
    let text = world.trace_text(&name);
    assert!(!text.contains(SENTINEL), "{corpus}");
    for raw in raw.iter().chain(&["/home/user", "SENTINEL"]) {
        assert!(!text.contains(raw), "{corpus}: the trace holds {raw}");
    }
    assert!(text.contains("\"id:"), "{corpus}: ids became pseudonyms");
    assert!(text.contains("<path>"), "{corpus}: paths became markers");
    assert_eq!(mode(&world.trace_path(&name)), 0o600);
    // The traces are all that was added to the state besides the store.
    let files = world.state_files();
    assert!(
        files.iter().all(|file| {
            file.starts_with("conspectus/hook-trace/")
                || file.starts_with("conspectus/live-state")
                || file.starts_with("conspectus/hooks/")
        }),
        "{files:?}"
    );
}

#[test]
fn no_free_text_of_any_claude_code_fixture_reaches_the_trace() {
    no_free_text_of_a_corpus_reaches_the_trace("claude-code", "claude-code", &["3f2a9c1e-7b4d"]);
}

#[test]
fn no_free_text_of_any_codex_fixture_reaches_the_trace() {
    no_free_text_of_a_corpus_reaches_the_trace("codex", "codex", &["019a7c3e-5b2d"]);
}

#[test]
fn no_free_text_of_any_opencode_fixture_reaches_the_trace() {
    no_free_text_of_a_corpus_reaches_the_trace(
        "opencode",
        "opencode-live",
        &["ses_7Kq2mX9vB4nL1pZ8"],
    );
}

#[test]
fn no_free_text_of_any_pi_fixture_reaches_the_trace() {
    no_free_text_of_a_corpus_reaches_the_trace("pi", "pi-live", &["0199f0a2-7c1e"]);
}
