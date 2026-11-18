//! The interactive showcase demo (`just demo`): `tui --fixture` with the
//! showcase's anchor time shows the agents' live states and which stops
//! are unread, takes the reads in memory only, and picks up a stop staged
//! by editing the fixture when `r` reloads it.
//!
//! The TUI runs in a pane of a private tmux server (the operator's own is
//! never reached), with a scratch home, so its state file is the test's.
#![cfg(all(unix, feature = "snapshot"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[path = "support/tmux.rs"]
mod tmux;

/// The showcase's anchor time (`SHOWCASE_NOW_EPOCH`, the justfile's
/// `showcase_now`).
const ANCHOR: i64 = 1_790_769_600;

/// How long a screen may take to show what it should.
const WAIT: Duration = Duration::from_secs(15);

/// The keys that put the cursor on the showcase's Pi session, whose agent
/// failed its turn two hours before the anchor and is in no mux.
const TO_PI: &str = "ljljlj";

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A scratch home, a private tmux server, and a copy of the showcase
/// fixture to edit.
struct Demo {
    _temp: tempfile::TempDir,
    root: PathBuf,
    tmpdir: PathBuf,
    fixture: PathBuf,
}

impl Demo {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        // tmux socket paths must stay short.
        let root = temp.path().to_path_buf();
        let tmpdir = root.join("t");
        fs::create_dir_all(&tmpdir).expect("mkdir");
        let fixture = root.join("showcase.json");
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/showcase.json"),
            &fixture,
        )
        .expect("copy the fixture");
        Self {
            _temp: temp,
            root,
            tmpdir,
            fixture,
        }
    }

    fn state_file(&self) -> PathBuf {
        self.root.join("state/conspectus/tui-state.json")
    }

    fn tmux(&self, args: &[&str]) -> std::process::Output {
        tmux::run(&self.tmpdir, args, || {
            let mut command = Command::new("tmux");
            command
                .args(["-f", "/dev/null"])
                .args(args)
                .env_remove("TMUX")
                .env("HOME", &self.root)
                .env("LC_CTYPE", "C.UTF-8")
                .env("TMUX_TMPDIR", &self.tmpdir);
            command
        })
    }

    fn tmux_ok(&self, args: &[&str]) {
        tmux::assert_ok(&self.tmpdir, args, &self.tmux(args));
    }

    /// Start the demo in session `name`: the fixture under the anchor
    /// time, as `just demo` runs it, in a scratch home with no tmux or
    /// forge discovery of its own.
    fn start(&self, name: &str, pinned: bool) {
        let now = if pinned {
            format!("--snapshot-now {ANCHOR}")
        } else {
            String::new()
        };
        let command = format!(
            "env -i PATH='{path}' HOME='{home}' TERM=xterm-256color \
             XDG_CONFIG_HOME='{root}/config' XDG_STATE_HOME='{root}/state' \
             XDG_DATA_HOME='{root}/data' XDG_CACHE_HOME='{root}/cache' \
             XDG_RUNTIME_DIR='{root}/run' CONSPECTUS_DISABLE_TMUX=1 CONSPECTUS_DISABLE_FORGE=1 \
             '{bin}' tui --fixture '{fixture}' {now} 2>'{root}/tui.err'; sleep 60",
            path = std::env::var("PATH").unwrap_or_default(),
            home = self.root.display(),
            root = self.root.display(),
            bin = env!("CARGO_BIN_EXE_conspectus"),
            fixture = self.fixture.display(),
        );
        self.tmux_ok(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            "150",
            "-y",
            "40",
            &command,
        ]);
        self.wait(name, "the tab strip", |screen| {
            screen.contains("sessions · mux")
        });
    }

    fn screen(&self, name: &str) -> String {
        let out = self.tmux(&["capture-pane", "-p", "-t", name]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn keys(&self, name: &str, keys: &str) {
        self.tmux_ok(&["send-keys", "-t", name, "-l", keys]);
        sleep(Duration::from_millis(250));
    }

    /// Wait for `name`'s screen to satisfy `done`; show it if it never does.
    fn wait(&self, name: &str, what: &str, done: impl Fn(&str) -> bool) -> String {
        let start = Instant::now();
        let mut last = String::new();
        while start.elapsed() < WAIT {
            last = self.screen(name);
            if done(&last) {
                return last;
            }
            sleep(Duration::from_millis(100));
        }
        let err = fs::read_to_string(self.root.join("tui.err")).unwrap_or_default();
        panic!("timed out waiting for {what}:\n{last}\nstderr: {err}");
    }

    /// The line of the detail pane that says the selected agent's state.
    fn state_field(screen: &str) -> Option<String> {
        screen
            .lines()
            .find_map(|line| line.split_once("state ").map(|(_, rest)| rest))
            .map(|rest| rest.trim_end_matches(['│', ' ']).to_string())
    }
}

impl Drop for Demo {
    fn drop(&mut self) {
        let _ = self.tmux(&["kill-server"]);
    }
}

/// `fixture` with the hook session starting to wait a few seconds before
/// the anchor time: the edit `just demo-stop` makes.
fn stage_a_stop(fixture: &Path) {
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(fixture).expect("read")).expect("json");
    let live = value["node_live"].as_array_mut().expect("node_live");
    let entry = live
        .iter_mut()
        .find(|entry| entry["node_id"]["session_key"] == "showcase-hook-current")
        .expect("the hook session's live entry");
    assert_eq!(entry["state"], "working", "it works before the edit");
    entry["state"] = "waiting".into();
    entry["detail"] = "input".into();
    entry["since_ms"] = ((ANCHOR - 5) * 1000).into();
    entry["observed_ms"] = (ANCHOR * 1000).into();
    fs::write(fixture, serde_json::to_string(&value).expect("json")).expect("write");
}

#[test]
fn the_demo_shows_unread_stops_keeps_reads_in_memory_and_takes_a_staged_stop() {
    if !tmux_available() {
        eprintln!("skipping: no tmux");
        return;
    }
    let demo = Demo::new();
    // A state file left by an earlier run of the operator's TUI reads
    // every stop it names: none of it reaches a fixture's agents.
    let remembered = r#"{"schema_version":1,"read_marks":{
        "agent_session:pi:/fixture/harness/pi:showcase-pi": 9999999999999}}"#;
    fs::create_dir_all(demo.state_file().parent().expect("dir")).expect("mkdir");
    fs::write(demo.state_file(), remembered).expect("seed");

    demo.start("a", true);
    // Pinned to the anchor time, the agents' live states show (waiting
    // ones count in the header), where they would read stale today.
    demo.wait("a", "the waiting count", |screen| screen.contains("! 4"));
    demo.keys("a", TO_PI);
    let screen = demo.wait("a", "the Pi session's state", |screen| {
        Demo::state_field(screen).is_some_and(|state| state.contains("error"))
    });
    assert!(
        Demo::state_field(&screen).is_some_and(|state| state.ends_with("unread")),
        "its stop is unread, whatever the state file remembers:\n{screen}"
    );

    // `c` reads it.
    demo.keys("a", "c");
    demo.wait("a", "the stop read", |screen| {
        Demo::state_field(screen).is_some_and(|state| state.ends_with("· read"))
    });

    // A stop staged in the fixture shows when `r` reloads it, unread.
    stage_a_stop(&demo.fixture);
    demo.keys("a", "r");
    demo.wait("a", "one more waiting", |screen| screen.contains("! 5"));

    // Quitting writes nothing of what was read, and the next run starts
    // with every stop unread again.
    demo.keys("a", "q");
    let state = fs::read_to_string(demo.state_file()).unwrap_or_default();
    assert!(
        !state.contains("showcase-hook") && state.contains("showcase-pi"),
        "the file keeps what it had and gains no mark: {state}"
    );
    // (The rows it left open are its own: start the next from the top.)
    fs::remove_file(demo.state_file()).expect("remove the state file");
    demo.start("b", true);
    demo.keys("b", TO_PI);
    let screen = demo.wait("b", "the Pi session's state", |screen| {
        Demo::state_field(screen).is_some_and(|state| state.contains("error"))
    });
    assert!(
        Demo::state_field(&screen).is_some_and(|state| state.ends_with("unread")),
        "unread again in the next run:\n{screen}"
    );
}

#[test]
fn without_the_anchor_time_the_demo_reads_every_state_stale() {
    if !tmux_available() {
        eprintln!("skipping: no tmux");
        return;
    }
    let demo = Demo::new();
    demo.start("c", false);
    sleep(Duration::from_millis(500));
    let screen = demo.screen("c");
    assert!(
        !screen.contains("! 4") && !screen.contains("next unread"),
        "no agent's evidence is fresh against today's clock:\n{screen}"
    );
}
