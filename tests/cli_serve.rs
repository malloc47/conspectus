//! Integration tests for `conspectus serve` (P7-006).
//!
//! The daemon is process-scoped: tests spawn the binary as a
//! subprocess, observe its side effects on `graph.sqlite`, and
//! signal-kill it before asserting. Each test uses a fresh
//! `$XDG_DATA_HOME` so the cache it inspects is unambiguously
//! the one this run produced.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn isolated_env_args(cmd: &mut Command, home: &Path, data_home: &Path) {
    cmd.env("HOME", home);
    cmd.env("XDG_DATA_HOME", data_home);
    cmd.env("CONSPECTUS_DISABLE_TMUX", "1");
    cmd.env("CONSPECTUS_DISABLE_FORGE", "1");
    cmd.env_remove("CONSPECTUS_CODEX_STATE");
    cmd.env_remove("CONSPECTUS_CLAUDE_CODE_STATE");
    cmd.env_remove("CONSPECTUS_OPENCODE_STATE");
}

/// Apply env-var isolation including a fresh `XDG_RUNTIME_DIR`
/// so each test owns its own socket path. Without this, two
/// parallel daemon tests fight over the same socket file.
fn isolated_serve_env_args(cmd: &mut Command, home: &Path, data_home: &Path, runtime_dir: &Path) {
    isolated_env_args(cmd, home, data_home);
    cmd.env("XDG_RUNTIME_DIR", runtime_dir);
}

/// Read or write a length-prefixed JSON frame per ADR 0038.
fn write_request(stream: &mut UnixStream, payload: &serde_json::Value) {
    let bytes = serde_json::to_vec(payload).expect("serialize request");
    let len = u32::try_from(bytes.len()).expect("request fits in u32");
    stream
        .write_all(&len.to_be_bytes())
        .expect("write length prefix");
    stream.write_all(&bytes).expect("write request body");
}

fn read_response(stream: &mut UnixStream) -> serde_json::Value {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).expect("read length prefix");
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).expect("read response body");
    serde_json::from_slice(&body).expect("parse response JSON")
}

fn socket_path_under(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("conspectus").join("server.sock")
}

fn wait_for_socket(path: &Path, deadline: Duration) -> bool {
    let stop = Instant::now() + deadline;
    while Instant::now() < stop {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn conspectus_bin() -> std::path::PathBuf {
    // assert_cmd::cargo crate not available without the dep; use
    // the CARGO_BIN_EXE_<name> env var Cargo exposes to integration
    // tests instead.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_conspectus"))
}

#[test]
fn serve_populates_graph_cache_within_first_tick() {
    // Layer A daemon: spawn `conspectus serve`, wait a few
    // seconds for the first warm-start cycle to land, kill the
    // process, then confirm the cache file exists and carries a
    // valid `user_version` set by the writer.
    //
    // The default shortest interval is 5s (harness/mux), but the
    // *first* cycle runs immediately on startup before the first
    // sleep — so a 2-3s wait is enough to observe the persist.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn conspectus serve");

    // Poll for a valid schema, not just file existence: the
    // writer creates the SQLite file inside open() and only
    // *then* applies the schema, so a naive existence check
    // races against the in-flight first persist. Reading
    // `user_version > 0` is the cheap "schema has been applied"
    // signal that callers (other daemon ticks, peer one-shot
    // CLIs) use too.
    let cache_path = data.path().join("conspectus").join("graph.sqlite");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut observed_version: u32 = 0;
    while Instant::now() < deadline {
        if cache_path.exists()
            && let Ok(conn) = rusqlite::Connection::open_with_flags(
                &cache_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            && let Ok(v) = conn.query_row::<u32, _, _>("PRAGMA user_version", [], |row| row.get(0))
            && v > 0
        {
            observed_version = v;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Kill the daemon before asserting so a hanging child does
    // not block the test runner if the assertion fails.
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        observed_version > 0,
        "daemon writer should set user_version within the deadline; \
         cache exists={}, observed={observed_version}",
        cache_path.exists()
    );
}

#[test]
fn serve_socket_echoes_a_ping_request() {
    // ADR 0038 wire shape: 4-byte big-endian length prefix +
    // UTF-8 JSON. The v1 dispatch table only knows `ping` (the
    // mutation commands land in the next commit); pinging
    // confirms the socket binds, framing round-trips, and the
    // dispatcher correlates the request id into the response.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn serve");

    let socket = socket_path_under(runtime.path());
    let connected = wait_for_socket(&socket, Duration::from_secs(5)) || {
        // Bind happens before the listener loop, so if 5s
        // passed we have a deeper problem. Don't hang.
        let _ = child.kill();
        let _ = child.wait();
        false
    };
    assert!(
        connected,
        "daemon should bind {} within 5s",
        socket.display()
    );

    let mut stream = UnixStream::connect(&socket).expect("connect to daemon socket");
    write_request(
        &mut stream,
        &serde_json::json!({
            "command": "ping",
            "args": {"hello": "world"},
            "id": "req-1",
        }),
    );
    let response = read_response(&mut stream);

    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(response["id"], "req-1", "id must round-trip");
    assert_eq!(response["result"], "ok");
    assert_eq!(
        response["data"]["echo"]["hello"], "world",
        "ping should echo the args payload"
    );
}

#[test]
fn serve_socket_rejects_unknown_command() {
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn serve");
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));

    let mut stream = UnixStream::connect(&socket).expect("connect");
    write_request(
        &mut stream,
        &serde_json::json!({"command": "do-the-thing", "id": "rq-9"}),
    );
    let response = read_response(&mut stream);

    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(response["id"], "rq-9");
    assert_eq!(response["result"], "error");
    assert_eq!(response["error"]["code"], "unknown_command");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("do-the-thing")
    );
}

#[test]
fn serve_shuts_down_cleanly_on_sigterm() {
    // ADR 0080: SIGTERM (and SIGINT) flip a shared shutdown
    // flag that every scheduler thread polls between sleeps.
    // The daemon should exit with status 0 within a few seconds
    // of the signal and emit the "stopped" line on stderr.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");

    let runtime = tempfile::TempDir::new().expect("runtime temp");
    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn serve");

    // Let the daemon finish its synchronous first-tick batch so
    // signal arrival lands during the inter-tick sleep — the
    // exact path the shutdown latch is designed to cover.
    std::thread::sleep(Duration::from_millis(800));

    // SIGTERM the child via libc (the std Child API only knows
    // SIGKILL on Unix). `kill -TERM <pid>` is the daemon-style
    // shutdown the operator (or systemd) would actually send.
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }

    // Give the daemon up to 5s to observe the signal and exit
    // cleanly; the inter-tick poll cadence is 200ms so this is
    // generous. Hang detection: if exit takes longer, fail.
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => break None,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };

    if status.is_none() {
        // Don't hang the test runner if shutdown is broken.
        let _ = child.kill();
        let _ = child.wait();
        panic!("daemon did not exit within 5s of SIGTERM");
    }
    let output = child.wait_with_output().expect("collect output");
    assert!(
        output.status.success(),
        "daemon should exit 0 on SIGTERM; got {:?}",
        output.status
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stopped"),
        "expected `stopped` line in stderr after graceful shutdown; got:\n{stderr}"
    );
}

#[test]
fn serve_logs_startup_line_to_stderr() {
    // Operators (and future P7-008 status checks) need to see
    // when the daemon actually started; this pins the startup
    // log so a refactor that loses it surfaces in CI.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn serve");

    // Give the daemon a moment to log and write before we kill
    // it. The startup line lands before the first sleep.
    std::thread::sleep(Duration::from_millis(500));
    let _ = child.kill();
    let output = child.wait_with_output().expect("collect output");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conspectus serve: starting"),
        "expected startup line in stderr; got:\n{stderr}"
    );
}
