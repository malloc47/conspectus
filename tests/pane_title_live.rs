//! A Codex pane's title as live state (ADR 0217), end to end: a private
//! tmux server whose pane runs a stand-in named `codex` (a shell, never
//! the real harness) under titles the test sets, observed by the CLI in
//! an isolated home with no hook installed. Every title is synthetic.
//!
//! The server's socket sits in the test's scratch directory
//! (`TMUX_TMPDIR`), every tmux call drops `$TMUX`, and the server is
//! killed when the test ends, so nothing reaches the host's tmux.

use std::path::PathBuf;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use serde_json::Value;

/// Planted in every title: it must never reach any output.
const SENTINEL: &str = "sentinel-task-4c19e2";

struct World {
    _temp: tempfile::TempDir,
    home: PathBuf,
    tmux_tmpdir: PathBuf,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        // Short: a socket path must fit in a `sockaddr_un`.
        let tmux_tmpdir = temp.path().join("t");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&tmux_tmpdir).unwrap();
        Self {
            _temp: temp,
            home,
            tmux_tmpdir,
        }
    }

    fn conspectus(&self) -> Command {
        let mut cmd = Command::cargo_bin("conspectus").expect("binary");
        // tmux prints tabs in `-F` formats as `_` without a UTF-8 locale.
        cmd.env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LC_CTYPE", "C.UTF-8")
            .env("HOME", &self.home)
            .env("CODEX_HOME", self.home.join(".codex"))
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_STATE_HOME", self.home.join(".local/state"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .env("XDG_RUNTIME_DIR", self.home.join("no-daemon"))
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env("CONSPECTUS_DISABLE_FORGE", "1")
            .env("CONSPECTUS_DISABLE_ZELLIJ", "1")
            .current_dir(&self.home);
        cmd
    }

    fn tmux(&self, args: &[&str]) -> bool {
        StdCommand::new("tmux")
            .args(args)
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .status()
            .is_ok_and(|status| status.success())
    }

    /// A session `name` whose pane runs a shell that calls itself
    /// `codex`, as tmux reports a pane's program.
    fn start_codex_stand_in(&self, name: &str) {
        assert!(self.tmux(&[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            "120",
            "-y",
            "40",
            "bash",
            "-c",
            "exec -a codex bash -c 'sleep 600; true'",
        ]));
        // The pane reports `bash` until the stand-in has replaced it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.pane_command(name).as_deref() != Some("codex") {
            assert!(
                std::time::Instant::now() < deadline,
                "the stand-in never ran"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The program tmux reports in session `name`'s active pane.
    fn pane_command(&self, name: &str) -> Option<String> {
        let output = StdCommand::new("tmux")
            .args([
                "display-message",
                "-p",
                "-t",
                name,
                "#{pane_current_command}",
            ])
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.tmux_tmpdir)
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn set_title(&self, name: &str, title: &str) {
        assert!(self.tmux(&["select-pane", "-t", name, "-T", title]));
    }

    fn stdout(&self, cmd: &mut Command) -> String {
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains(SENTINEL),
            "the title reached stderr"
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn graph(&self, cmd: &mut Command) -> Value {
        let text = self.stdout(cmd.args(["graph", "--format", "json"]));
        assert!(!text.contains(SENTINEL), "the title reached the graph JSON");
        serde_json::from_str(&text).unwrap()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = self.tmux(&["kill-server"]);
    }
}

fn tools_available() -> bool {
    [("tmux", "-V"), ("bash", "--version")]
        .iter()
        .all(|(program, flag)| {
            StdCommand::new(program)
                .arg(flag)
                .output()
                .is_ok_and(|output| output.status.success())
        })
}

/// The `pane_live` entry of mux `name`'s pane `%0`, if any.
fn pane_entry(graph: &Value, name: &str) -> Option<Value> {
    graph["pane_live"].as_array()?.iter().find_map(|entry| {
        (entry["node_id"]["native_id"] == format!("tmux:{name}") && entry["pane"] == "%0")
            .then(|| entry.clone())
    })
}

#[test]
fn a_codex_panes_title_shows_its_wait_and_its_work_without_hooks() {
    if !tools_available() {
        eprintln!("tmux or bash not installed; skipping");
        return;
    }
    let world = World::new();
    world.start_codex_stand_in("cx");

    world.set_title(
        "cx",
        &format!("[ ! ] Action Required {SENTINEL} | conspectus"),
    );
    let graph = world.graph(&mut world.conspectus());
    let entry = pane_entry(&graph, "cx").unwrap_or_else(|| panic!("no pane entry: {graph:#}"));
    assert_eq!(entry["state"], "waiting");
    assert_eq!(entry["detail"], "input");
    assert_eq!(entry["source"], "pane_title");
    assert!(entry.get("pid").is_none(), "{entry}");
    assert!(graph.get("node_live").is_none(), "{graph:#}");

    // The health line says where Codex's state comes from meanwhile.
    let status = world.stdout(world.conspectus().args(["hook", "status", "codex"]));
    let health = status.lines().last().unwrap_or_default();
    assert!(health.starts_with("off\tcodex\t"), "{status}");
    assert!(
        health.ends_with("; 1 agent shown from its pane title"),
        "{status}"
    );

    // A spinner frame Codex draws while it works.
    world.set_title("cx", &format!("⠙ {SENTINEL} | conspectus"));
    let graph = world.graph(&mut world.conspectus());
    let entry = pane_entry(&graph, "cx").expect("pane entry");
    assert_eq!(
        (&entry["state"], &entry["source"]),
        (&Value::from("working"), &Value::from("pane_title"))
    );

    // An idle title says nothing.
    world.set_title("cx", &format!("{SENTINEL} | conspectus"));
    let graph = world.graph(&mut world.conspectus());
    assert_eq!(pane_entry(&graph, "cx"), None, "{graph:#}");
}

#[test]
fn the_switch_keeps_pane_titles_unread() {
    if !tools_available() {
        eprintln!("tmux or bash not installed; skipping");
        return;
    }
    let world = World::new();
    world.start_codex_stand_in("cx");
    world.set_title("cx", &format!("[ ! ] Action Required {SENTINEL}"));
    let graph = world.graph(
        world
            .conspectus()
            .env("CONSPECTUS_DISABLE_PANE_TITLES", "1"),
    );
    assert_eq!(pane_entry(&graph, "cx"), None, "{graph:#}");
    // The mux is still there: only its title went unread.
    let listed = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["id"]["native_id"] == "tmux:cx");
    assert!(listed, "{graph:#}");
}
