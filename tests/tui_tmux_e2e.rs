//! The real TUI in a pane of a private tmux server, driven with
//! `send-keys` and read with `capture-pane`.
//!
//! Two tmux servers keep everything off the real one. The "inner" server
//! holds the muxes under test (the TUI's `TMUX_TMPDIR` points at it). The
//! "outer" server only hosts a pane that stands in for the operator's
//! terminal: either the TUI itself (the outside-tmux flow, where attach is
//! `tmux attach-session` in the TUI's own terminal) or a tmux client
//! attached to the inner server's `ui` session, whose window runs the TUI
//! (the inside-tmux flow, where attach is `switch-client`).
//!
//! Each test asserts the screen converges on the new state with no key
//! press after returning to the TUI, which is what an operator sees.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// How long a screen may take to converge before the test fails. Generous
/// because the suite runs many processes at once; a stale screen that
/// needs a manual refresh never converges, so the bound doesn't hide the
/// bug.
const CONVERGE: Duration = Duration::from_secs(10);

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
}

struct World {
    _temp: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
    inner: PathBuf,
    outer: PathBuf,
    bin_dir: PathBuf,
    daemon: Option<std::process::Child>,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        // tmux socket paths must stay short.
        let root = temp.path().to_path_buf();
        let repo = root.join("home/src/app");
        let inner = root.join("in");
        let outer = root.join("out");
        let bin_dir = root.join("bin");
        let run = root.join("run");
        for dir in [&repo, &inner, &outer, &bin_dir, &run] {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
        // Stub harnesses: alive in a pane, never real agents.
        for harness in ["aider", "claude", "codex"] {
            let path = bin_dir.join(harness);
            fs::write(&path, "#!/bin/sh\nexec sleep 600\n").unwrap();
            fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        }
        for args in [
            &["init", "--quiet", "--initial-branch", "main"][..],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "i",
            ][..],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        Self {
            _temp: temp,
            root,
            repo,
            inner,
            outer,
            bin_dir,
            daemon: None,
        }
    }

    fn env_pairs(&self) -> Vec<(String, String)> {
        let home = self.root.join("home");
        let path = format!(
            "{}:{}",
            self.bin_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        vec![
            ("PATH".into(), path),
            ("HOME".into(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".into(),
                self.root.join("xdg/c").display().to_string(),
            ),
            (
                "XDG_STATE_HOME".into(),
                self.root.join("xdg/s").display().to_string(),
            ),
            (
                "XDG_DATA_HOME".into(),
                self.root.join("xdg/d").display().to_string(),
            ),
            (
                "XDG_CACHE_HOME".into(),
                self.root.join("xdg/k").display().to_string(),
            ),
            (
                "XDG_RUNTIME_DIR".into(),
                self.root.join("run").display().to_string(),
            ),
            ("LC_CTYPE".into(), "C.UTF-8".into()),
            ("CONSPECTUS_DISABLE_FORGE".into(), "1".into()),
            ("CONSPECTUS_DISABLE_ZELLIJ".into(), "1".into()),
            ("TMUX_TMPDIR".into(), self.inner.display().to_string()),
        ]
    }

    /// `tmux` against the inner server (the muxes under test).
    fn inner(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-f", "/dev/null"])
            .args(args)
            .env_remove("TMUX")
            .env("HOME", self.root.join("home"))
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.inner)
            .output()
            .expect("tmux")
    }

    /// `tmux` against the outer server (the operator's terminal).
    fn outer(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-f", "/dev/null"])
            .args(args)
            .env_remove("TMUX")
            .env("HOME", self.root.join("home"))
            .env("LC_CTYPE", "C.UTF-8")
            .env("TMUX_TMPDIR", &self.outer)
            .output()
            .expect("tmux")
    }

    fn conspectus(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_conspectus"));
        cmd.args(args).env_clear().current_dir(&self.repo);
        for (key, value) in self.env_pairs() {
            cmd.env(key, value);
        }
        cmd.output().expect("conspectus")
    }

    fn start_daemon(&mut self) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_conspectus"));
        cmd.arg("serve")
            .env_clear()
            .current_dir(&self.repo)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (key, value) in self.env_pairs() {
            cmd.env(key, value);
        }
        self.daemon = Some(cmd.spawn().expect("serve"));
        let socket = self.root.join("run/conspectus/server.sock");
        wait_until("the daemon socket", Duration::from_secs(30), || {
            socket.exists()
        });
        sleep(Duration::from_secs(2));
    }

    fn shell_env_prefix(&self, keep_tmux: bool) -> String {
        let mut parts = Vec::new();
        if !keep_tmux {
            parts.push("-u TMUX".to_string());
        }
        for (key, value) in self.env_pairs() {
            parts.push(format!("{key}='{value}'"));
        }
        format!("env {}", parts.join(" "))
    }

    fn tui_command(&self, keep_tmux: bool) -> String {
        format!(
            "{} {} tui --view mux --refresh-interval 30s 2>{}; sleep 120",
            self.shell_env_prefix(keep_tmux),
            env!("CARGO_BIN_EXE_conspectus"),
            self.root.join("tui.err").display()
        )
    }

    /// Outside-tmux flow: the TUI runs directly in the operator's terminal.
    fn start_tui_outside(&self) {
        let command = self.tui_command(false);
        assert!(
            self.outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
        );
        wait_until("the TUI to list muxes", CONVERGE, || {
            self.screen().contains('▣')
        });
    }

    /// Inside-tmux flow: the TUI runs in session `ui` of the inner server
    /// and the operator's terminal is a client attached to it.
    fn start_tui_inside(&self) {
        let command = self.tui_command(true);
        assert!(
            self.inner(&[
                "new-session",
                "-d",
                "-s",
                "ui",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
        );
        let attach = format!(
            "env -u TMUX TMUX_TMPDIR='{}' tmux attach -t ui; echo CLIENT-EXITED; sleep 120",
            self.inner.display()
        );
        assert!(
            self.outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &attach
            ])
            .status
            .success()
        );
        wait_until("the TUI to list muxes", CONVERGE, || {
            self.tui_pane().contains('▣')
        });
    }

    /// What the operator's terminal shows.
    fn screen(&self) -> String {
        let out = self.outer(&["capture-pane", "-p", "-t", "host"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// The TUI's own pane in the inside-tmux flow, whether or not a
    /// client is looking at it.
    fn tui_pane(&self) -> String {
        let out = self.inner(&["capture-pane", "-p", "-t", "ui"]);
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn keys(&self, keys: &[&str]) {
        let mut args = vec!["send-keys", "-t", "host"];
        args.extend_from_slice(keys);
        assert!(self.outer(&args).status.success());
        sleep(Duration::from_millis(250));
    }

    fn new_mux(&self, name: &str, shell: bool) {
        let mut args = vec!["new-session", "-d", "-s", name, "-c"];
        let cwd = self.repo.display().to_string();
        args.push(&cwd);
        if !shell {
            args.extend(["sleep", "600"]);
        }
        assert!(self.inner(&args).status.success());
    }

    fn pin(&self, name: &str) {
        let out = self.conspectus(&["pin", "add", name, "--name", &format!("pin-{name}")]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn mux_alive(&self, name: &str) -> bool {
        self.inner(&["has-session", "-t", &format!("={name}")])
            .status
            .success()
    }

    /// Move the TUI's cursor onto the mux row for `name` (Mux view, first
    /// row selected).
    fn select_mux(&self, text: impl Fn() -> String, name: &str) {
        let screen = text();
        let rows: Vec<&str> = screen
            .lines()
            .map(left_pane)
            .filter(|line| line.contains('▣'))
            .collect();
        let index = rows
            .iter()
            .position(|line| line.contains(name))
            .unwrap_or_else(|| panic!("no row for {name}:\n{screen}"));
        for _ in 0..index {
            self.keys(&["j"]);
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if let Some(mut daemon) = self.daemon.take() {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
        let _ = self.inner(&["kill-server"]);
        let _ = self.outer(&["kill-server"]);
    }
}

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}");
}

/// Wait for the operator's terminal to satisfy `done`, showing the
/// screen if it never does.
fn wait_screen(world: &World, what: &str, done: impl Fn(&str) -> bool) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = world.screen();
        if done(&last) {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}:\n{last}");
}

/// The row text for `name` in a captured screen, if listed.
fn row<'a>(screen: &'a str, name: &str) -> Option<&'a str> {
    screen
        .lines()
        .map(left_pane)
        .find(|line| line.contains('▣') && line.contains(name))
}

/// The left pane's part of a screen line.
fn left_pane(line: &str) -> &str {
    line.split("││").next().unwrap_or(line)
}

/// A ghost row: the pin marker is hollow and the state is the stopped
/// glyph, with nothing saying it is running.
fn is_stopped_row(line: &str) -> bool {
    use conspectus::tui::icons::state::{PIN_STOPPED, STOPPED};
    line.contains(PIN_STOPPED) && line.contains(STOPPED)
}

fn assert_converges_to_stopped(world: &World, read: impl Fn(&World) -> String, name: &str) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = read(world);
        if row(&last, name).is_some_and(is_stopped_row) {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("`{name}` never showed as stopped without a refresh:\n{last}");
}

fn assert_converges_to_absent(world: &World, read: impl Fn(&World) -> String, name: &str) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < CONVERGE {
        last = read(world);
        if row(&last, name).is_none() && last.contains("stopped") {
            return;
        }
        sleep(Duration::from_millis(100));
    }
    panic!("`{name}` never left the Mux view without a refresh:\n{last}");
}

/// Attach to `victim` from the TUI (Enter on its row).
fn attach(world: &World, read: impl Fn() -> String) {
    world.select_mux(read, "victim");
    world.keys(&["Enter"]);
    sleep(Duration::from_millis(1500));
}

macro_rules! needs_tmux {
    () => {
        if !tmux_available() {
            eprintln!("tmux not installed; skipping");
            return;
        }
    };
}

// ---- Outside tmux: attach-session in the TUI's own terminal -----------

fn outside_exit(pinned: bool, daemon: bool) {
    let mut world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    if pinned {
        world.pin("victim");
    }
    if daemon {
        world.start_daemon();
    }
    world.start_tui_outside();
    attach(&world, || world.screen());
    world.keys(&["exit", "Enter"]);
    if pinned {
        assert_converges_to_stopped(&world, World::screen, "victim");
    } else {
        assert_converges_to_absent(&world, World::screen, "victim");
    }
    assert!(!world.mux_alive("victim"));
}

#[test]
fn outside_tmux_exiting_a_pinned_mux_shows_its_ghost_without_a_refresh() {
    needs_tmux!();
    outside_exit(true, false);
}

#[test]
fn outside_tmux_exiting_an_unpinned_mux_removes_it_from_the_mux_view() {
    needs_tmux!();
    outside_exit(false, false);
}

#[test]
fn outside_tmux_exiting_a_pinned_mux_with_a_daemon_shows_its_ghost() {
    needs_tmux!();
    outside_exit(true, true);
}

#[test]
fn outside_tmux_detaching_leaves_the_mux_running() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    world.start_tui_outside();
    attach(&world, || world.screen());
    world.keys(&["C-b", "d"]);
    wait_screen(&world, "the TUI to return", |screen| screen.contains('▣'));
    sleep(Duration::from_secs(2));
    let screen = world.screen();
    let line = row(&screen, "victim").unwrap_or_else(|| panic!("victim vanished:\n{screen}"));
    assert!(!is_stopped_row(line), "{screen}");
    assert!(world.mux_alive("victim"));
}

#[test]
fn outside_tmux_exiting_the_only_mux_converges_when_the_server_dies() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("victim", true);
    world.pin("victim");
    world.start_tui_outside();
    world.keys(&["Enter"]);
    sleep(Duration::from_millis(1500));
    world.keys(&["exit", "Enter"]);
    assert_converges_to_stopped(&world, World::screen, "victim");
}

// ---- Inside tmux: attach is switch-client ------------------------------

fn inside_exit(pinned: bool, detach_on_destroy: &str) {
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", true);
    if pinned {
        world.pin("victim");
    }
    world.start_tui_inside();
    world.inner(&["set", "-g", "detach-on-destroy", detach_on_destroy]);
    attach(&world, || world.tui_pane());
    // The client is now looking at the victim's shell.
    wait_until("the client to show the victim", CONVERGE, || {
        !world.screen().contains("pinned · history")
    });
    world.keys(&["exit", "Enter"]);
    if detach_on_destroy == "on" {
        // The client detached when its session ended: the operator
        // reattaches later and finds the TUI.
        wait_until("the client to exit", CONVERGE, || {
            world.screen().contains("CLIENT-EXITED")
        });
        let attach = format!(
            "env -u TMUX TMUX_TMPDIR='{}' tmux attach -t ui; sleep 120",
            world.inner.display()
        );
        world.outer(&["respawn-window", "-k", "-t", "host", &attach]);
    }
    if pinned {
        assert_converges_to_stopped(&world, World::tui_pane, "victim");
    } else {
        assert_converges_to_absent(&world, World::tui_pane, "victim");
    }
}

#[test]
fn inside_tmux_returning_after_exiting_a_pinned_mux_shows_its_ghost() {
    needs_tmux!();
    inside_exit(true, "off");
}

#[test]
fn inside_tmux_returning_after_exiting_an_unpinned_mux_removes_it() {
    needs_tmux!();
    inside_exit(false, "off");
}

#[test]
fn inside_tmux_reattaching_after_the_client_was_detached_shows_the_ghost() {
    needs_tmux!();
    inside_exit(true, "on");
}

#[test]
fn inside_tmux_a_kill_from_another_client_is_seen_on_return() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", false);
    world.pin("victim");
    world.start_tui_inside();
    world.inner(&["set", "-g", "detach-on-destroy", "off"]);
    attach(&world, || world.tui_pane());
    // Killed by someone else while the operator is in another session.
    world.inner(&["kill-session", "-t", "=victim"]);
    assert_converges_to_stopped(&world, World::tui_pane, "victim");
}

// ---- Other triggers -----------------------------------------------------

#[test]
fn an_idle_tui_notices_a_kill_on_its_timer() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.new_mux("victim", false);
    world.pin("victim");
    // A short interval stands in for the operator waiting.
    let command = world
        .tui_command(false)
        .replace("--refresh-interval 30s", "--refresh-interval 1s");
    assert!(
        world
            .outer(&[
                "new-session",
                "-d",
                "-s",
                "host",
                "-x",
                "150",
                "-y",
                "40",
                &command
            ])
            .status
            .success()
    );
    wait_until("the TUI to list muxes", CONVERGE, || {
        world.screen().contains("victim")
    });
    world.inner(&["kill-session", "-t", "=victim"]);
    assert_converges_to_stopped(&world, World::screen, "victim");
}

#[test]
fn launching_from_the_form_and_detaching_at_once_shows_the_new_mux_running() {
    needs_tmux!();
    let world = World::new();
    world.new_mux("other", false);
    world.start_tui_outside();
    world.keys(&["N"]);
    sleep(Duration::from_millis(500));
    assert!(
        world.screen().contains("Launch harness"),
        "{}",
        world.screen()
    );
    world.keys(&["Enter"]);
    // Launching attaches; detach straight away.
    sleep(Duration::from_millis(1500));
    world.keys(&["C-b", "d"]);
    wait_until("the TUI to return", CONVERGE, || {
        world.screen().contains("pinned · history")
    });
    let start = Instant::now();
    loop {
        let screen = world.screen();
        let lines: Vec<&str> = screen
            .lines()
            .map(left_pane)
            .filter(|line| line.contains('▣') && !line.contains("other"))
            .collect();
        if lines.len() == 1 && !is_stopped_row(lines[0]) {
            break;
        }
        assert!(
            start.elapsed() < CONVERGE,
            "the new mux never showed as running without a refresh:\n{screen}"
        );
        sleep(Duration::from_millis(100));
    }
}
