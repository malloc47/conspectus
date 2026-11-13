//! `claude agents --json` as a live-state repair (ADR 0218), end to end:
//! `conspectus serve` in an isolated home, with a stand-in `claude` (a
//! shell script, never the real harness) first on a `PATH` of system
//! directories, a Claude Code session from the fixture writer, and a
//! scratch live-state store whose entry the hooks left stuck. The
//! stand-in records how it was run and prints a synthetic listing.
//! Clients read the daemon's snapshot as the operator's commands do.

use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::{Duration, Instant};

use conspectus::discovery::harness::fixtures::{ClaudeCodeSessionRecord, HarnessFixture};
use conspectus::live_store::{LiveContext, LiveEvent, LiveEventKind, LiveStore, WaitKind};
use conspectus::model::live::{LiveSource, LiveState};
use serde_json::{Value, json};

/// Planted in every text field of the listing: it must reach no output.
const SENTINEL: &str = "sentinel-agents-58e0c4";

const SESSION: &str = "8c1d2f0e-5b7a-4c39-9e21-3a6f0b9d4e17";

const MINUTE_MS: i64 = 60_000;

struct World {
    _temp: tempfile::TempDir,
    home: PathBuf,
    bin: PathBuf,
    log: PathBuf,
    daemon: Option<Child>,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let bin = temp.path().join("bin");
        let log = temp.path().join("log");
        for dir in [&home, &bin, &log] {
            std::fs::create_dir_all(dir).unwrap();
        }
        HarnessFixture::at(&home)
            .write_claude_code_session(&ClaudeCodeSessionRecord::new(
                SESSION,
                home.join("work").display().to_string(),
            ))
            .expect("write the session");
        Self {
            _temp: temp,
            home,
            bin,
            log,
            daemon: None,
        }
    }

    /// A stand-in `claude` that appends its arguments and whether
    /// nonessential traffic is off to the log, then prints `listing`.
    fn fake_claude(&self, listing: &Value) {
        use std::os::unix::fs::PermissionsExt;
        let listing_path = self.log.join("listing.json");
        std::fs::write(&listing_path, serde_json::to_vec(listing).unwrap()).unwrap();
        let script = format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> '{calls}'\n\
             printf '%s\\n' \"${{CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC:-unset}}\" >> '{env}'\n\
             echo 'stderr is never read {SENTINEL}' >&2\n\
             cat '{listing}'\n",
            calls = self.log.join("calls").display(),
            env = self.log.join("env").display(),
            listing = listing_path.display(),
        );
        let path = self.bin.join("claude");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// How the stand-in was run, one line per run.
    fn calls(&self) -> Vec<String> {
        lines(&self.log.join("calls"))
    }

    fn state_dir(&self) -> PathBuf {
        self.home.join(".local/state/conspectus")
    }

    /// `kinds` for the session, as this process's hooks would have
    /// written them, the first `minutes_ago` and each a minute later.
    fn hook_events(&self, minutes_ago: i64, kinds: Vec<LiveEventKind>) {
        let pid = i64::from(std::process::id());
        let store = LiveStore::at(self.state_dir());
        let now = conspectus::live_store::now_ms();
        for (index, kind) in (0..).zip(kinds) {
            let event = LiveEvent::new(
                "claude-code",
                SESSION,
                now - (minutes_ago - index) * MINUTE_MS,
                kind,
            )
            .with_context(LiveContext {
                pid: Some(pid),
                pid_start: conspectus::live_store::process_start_time(pid),
                ..Default::default()
            });
            store.apply_event(&event).expect("write the store");
        }
    }

    fn env(&self, command: &mut StdCommand) {
        let home = &self.home;
        command
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_STATE_HOME", home.join(".local/state"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_RUNTIME_DIR", home.join("run"))
            .env(
                "CONSPECTUS_CLAUDE_CODE_STATE",
                HarnessFixture::at(home).claude_code_state_root(),
            )
            .env("CONSPECTUS_DISABLE_TMUX", "1")
            .env("CONSPECTUS_DISABLE_ZELLIJ", "1")
            .env("CONSPECTUS_DISABLE_FORGE", "1")
            .current_dir(home);
    }

    fn serve(&mut self, extra: &[(&str, &str)]) {
        let mut command = StdCommand::new(env!("CARGO_BIN_EXE_conspectus"));
        self.env(&mut command);
        command
            .envs(extra.iter().copied())
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        self.daemon = Some(command.spawn().expect("spawn conspectus serve"));
        let socket = self.home.join("run/conspectus/server.sock");
        let deadline = Instant::now() + Duration::from_secs(60);
        while !socket.exists() {
            assert!(Instant::now() < deadline, "the daemon never listened");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Stop the daemon and return what it printed on stderr.
    fn stop(&mut self) -> String {
        let Some(mut daemon) = self.daemon.take() else {
            return String::new();
        };
        let _ = daemon.kill();
        let output = daemon.wait_with_output().expect("wait for the daemon");
        String::from_utf8_lossy(&output.stderr).into_owned()
    }

    /// `conspectus <args>` as a client of the daemon; its stdout.
    fn client(&self, args: &[&str]) -> String {
        let mut command = StdCommand::new(env!("CARGO_BIN_EXE_conspectus"));
        self.env(&mut command);
        let output = command.args(args).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{args:?}: {stderr}");
        assert!(!stderr.contains(SENTINEL), "the listing reached stderr");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(!stdout.contains(SENTINEL), "the listing reached {args:?}");
        stdout
    }

    /// The session's `node_live` entry once `done` holds for it, waiting
    /// for the daemon's cycles.
    fn entry_when(&self, done: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let graph: Value =
                serde_json::from_str(&self.client(&["graph", "--format", "json"])).unwrap();
            let entry = graph["node_live"].as_array().and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry["node_id"]["session_key"] == SESSION)
                    .cloned()
            });
            if let Some(entry) = entry.filter(|entry| done(entry)) {
                assert!(
                    graph.get("live_polls").is_none(),
                    "the poll stays out of JSON"
                );
                return entry;
            }
            assert!(
                Instant::now() < deadline,
                "the entry never read as expected: {graph:#}"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Fail when the file at `path` holds the sentinel anywhere.
    fn no_sentinel_in(&self, path: &Path) {
        let bytes = std::fs::read(path).unwrap_or_default();
        assert!(
            !bytes
                .windows(SENTINEL.len())
                .any(|window| window == SENTINEL.as_bytes()),
            "the listing reached {}",
            path.display()
        );
    }
}

impl Drop for World {
    fn drop(&mut self) {
        self.stop();
    }
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// A listing of this process's session in `status`, with the sentinel in
/// every text field Claude Code documents.
fn listing(status: &str) -> Value {
    json!([{
        "pid": std::process::id(),
        "sessionId": SESSION,
        "name": format!("{SENTINEL} fix the login bug"),
        "cwd": format!("/home/user/{SENTINEL}"),
        "kind": "interactive",
        "startedAt": 1_794_546_000_000_i64,
        "status": status,
        "unknownField": SENTINEL,
    }])
}

fn state_and_source(entry: &Value) -> (&str, &str) {
    (
        entry["state"].as_str().unwrap_or_default(),
        entry["source"].as_str().unwrap_or_default(),
    )
}

#[test]
fn the_daemon_reads_an_interrupted_turn_as_ready_from_claude_agents() {
    let mut world = World::new();
    world.fake_claude(&listing("idle"));
    // An Esc interrupt: the turn started, and no stop event came.
    world.hook_events(2, vec![LiveEventKind::TurnStarted]);
    world.serve(&[]);

    let entry = world.entry_when(|entry| state_and_source(entry) == ("ready", "harness_poll"));
    assert_eq!(entry["pid"], i64::from(std::process::id()));
    assert_eq!(entry["liveness"], "alive");
    assert!(entry.get("detail").is_none(), "{entry}");

    // The table reads the repaired state like any other.
    let table = world.client(&["table", "sessions", "--columns", "+live"]);
    let row = table
        .lines()
        .find(|line| line.contains(&SESSION[SESSION.len() - 8..]))
        .unwrap_or_else(|| panic!("no row for the session: {table}"));
    assert!(row.contains("ready"), "{table}");

    let calls = world.calls();
    assert!(!calls.is_empty(), "the stand-in never ran");
    assert!(
        calls.iter().all(|call| call == "agents --json"),
        "{calls:?}"
    );
    assert!(
        lines(&world.log.join("env"))
            .iter()
            .all(|value| value == "1")
    );

    let stderr = world.stop();
    assert!(
        !stderr.contains(SENTINEL),
        "the listing reached the daemon's stderr"
    );
    world.no_sentinel_in(&world.home.join(".local/share/conspectus/graph.bin"));
    world.no_sentinel_in(&world.state_dir().join("live-state.json"));
    // The store's entry is as the hooks left it: a poll writes nothing.
    let file = LiveStore::at(world.state_dir()).read();
    let states: Vec<_> = file
        .entries
        .values()
        .map(|entry| (entry.observation.state, entry.observation.source))
        .collect();
    assert_eq!(states, [(LiveState::Working, LiveSource::Hook)]);
}

#[test]
fn the_daemon_reads_an_answered_approval_as_working_from_claude_agents() {
    let mut world = World::new();
    world.fake_claude(&listing("busy"));
    world.hook_events(
        3,
        vec![
            LiveEventKind::TurnStarted,
            LiveEventKind::WaitStarted {
                agent: None,
                kind: WaitKind::Permission,
            },
        ],
    );
    world.serve(&[]);
    let entry = world.entry_when(|entry| state_and_source(entry) == ("working", "harness_poll"));
    assert_eq!(entry["detail"], "generating");
}

#[test]
fn the_switch_keeps_claude_from_being_run() {
    let mut world = World::new();
    world.fake_claude(&listing("idle"));
    world.hook_events(2, vec![LiveEventKind::TurnStarted]);
    world.serve(&[("CONSPECTUS_DISABLE_LIVE_POLL", "1")]);
    // The pass that binds the entry is the one that would have polled.
    let entry = world.entry_when(|entry| entry["state"] == "working");
    assert_eq!(entry["source"], "hook");
    assert!(world.calls().is_empty(), "claude ran: {:?}", world.calls());
}

#[test]
fn a_one_shot_command_never_runs_claude() {
    let world = World::new();
    world.fake_claude(&listing("idle"));
    world.hook_events(2, vec![LiveEventKind::TurnStarted]);
    let graph: Value =
        serde_json::from_str(&world.client(&["graph", "--format", "json", "--refresh"])).unwrap();
    let entry = graph["node_live"]
        .as_array()
        .and_then(|entries| entries.first())
        .cloned()
        .unwrap_or_else(|| panic!("no live state: {graph:#}"));
    assert_eq!(state_and_source(&entry), ("working", "hook"));
    assert!(world.calls().is_empty(), "claude ran: {:?}", world.calls());
}
