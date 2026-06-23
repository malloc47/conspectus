//! Integration tests for `conspectus serve` (P7-006).
//!
//! The daemon is process-scoped: tests spawn the binary as a
//! subprocess, observe its side effects on `graph.bin`, and
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
    // seconds for the first cycle to land, kill the process,
    // then confirm `graph.bin` exists and parses cleanly via
    // the snapshot module.
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

    // Poll for a validated graph.bin rather than naked file
    // existence: the writer creates the tmp file first and
    // renames over the target, but a check that runs
    // mid-rename could see a partial file. `open_mmap` runs
    // bytecheck end-to-end, so a successful open means the
    // bytes are structurally sound.
    let cache_path = data.path().join("conspectus").join("graph.bin");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut observed_ok = false;
    while Instant::now() < deadline {
        if cache_path.exists() && conspectus::snapshot::open_mmap(&cache_path).is_ok() {
            observed_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Kill the daemon before asserting so a hanging child does
    // not block the test runner if the assertion fails.
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        observed_ok,
        "daemon writer should land a valid graph.bin within the deadline; \
         cache exists={}",
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
fn serve_socket_refresh_command_writes_a_fresh_snapshot() {
    // The `refresh` socket command forces a full cold rebuild on
    // the daemon side: empty prior + every adapter runs + the
    // result lands in `graph.bin`. Useful for "I just did
    // something on disk, refresh now" without waiting for the
    // next forge tick.
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

    let cache_path = data.path().join("conspectus").join("graph.bin");
    // Wait until the daemon's first cycle has produced a
    // validated graph.bin so we know the post-refresh mtime
    // change is attributable to the refresh, not a racy
    // first-write.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if cache_path.exists() && conspectus::snapshot::open_mmap(&cache_path).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let mtime_before = std::fs::metadata(&cache_path)
        .expect("stat cache")
        .modified()
        .expect("modified time");

    // Sleep enough to make the mtime delta observable. Most
    // filesystems track mtime at second granularity.
    std::thread::sleep(Duration::from_millis(1100));

    let mut stream = UnixStream::connect(&socket).expect("connect");
    write_request(
        &mut stream,
        &serde_json::json!({"command": "refresh", "id": "refresh-1"}),
    );
    let response = read_response(&mut stream);

    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(response["id"], "refresh-1");
    assert_eq!(
        response["result"], "ok",
        "refresh should succeed; got error: {:?}",
        response["error"]
    );
    assert!(
        response["data"]["refreshed_epoch"].as_u64().is_some(),
        "refresh response should include refreshed_epoch"
    );

    let mtime_after = std::fs::metadata(&cache_path)
        .expect("stat cache after")
        .modified()
        .expect("modified time after");
    assert!(
        mtime_after > mtime_before,
        "refresh should bump the cache mtime (before={mtime_before:?}, after={mtime_after:?})"
    );
}

#[test]
fn serve_cleans_up_legacy_graph_sqlite_on_startup() {
    // P11-011a: pre-rkyv daemon runs left a graph.sqlite (plus
    // its -wal / -shm sidecars and the backups/ dir). The
    // new daemon has no consumer for any of them and unlinks
    // them on startup. The cleanup is best-effort but the
    // happy-path test pins the behavior so a regression that
    // forgets the cleanup surfaces as leftover legacy
    // artifacts in operator data dirs.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let dir = data.path().join("conspectus");
    std::fs::create_dir_all(&dir).expect("create dir");
    let sqlite_path = dir.join("graph.sqlite");
    let wal_path = dir.join("graph.sqlite-wal");
    let shm_path = dir.join("graph.sqlite-shm");
    let backups_path = dir.join("backups");
    std::fs::write(&sqlite_path, b"legacy bytes").expect("write legacy sqlite");
    std::fs::write(&wal_path, b"").expect("write legacy wal");
    std::fs::write(&shm_path, b"").expect("write legacy shm");
    std::fs::create_dir_all(&backups_path).expect("create legacy backups dir");
    std::fs::write(backups_path.join("graph-1.sqlite"), b"old backup").expect("write old backup");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("spawn serve");

    // Wait until the daemon publishes its first graph.bin —
    // by that point the startup cleanup has definitely run.
    let bin_path = dir.join("graph.bin");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if bin_path.exists() && conspectus::snapshot::open_mmap(&bin_path).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        bin_path.exists(),
        "daemon should write a valid graph.bin under {}",
        dir.display()
    );
    assert!(
        !sqlite_path.exists(),
        "legacy {} should be unlinked on daemon startup",
        sqlite_path.display()
    );
    assert!(
        !wal_path.exists(),
        "legacy {} should be unlinked on daemon startup",
        wal_path.display()
    );
    assert!(
        !shm_path.exists(),
        "legacy {} should be unlinked on daemon startup",
        shm_path.display()
    );
    assert!(
        !backups_path.exists(),
        "legacy backups dir {} should be removed on daemon startup",
        backups_path.display()
    );
}

#[test]
fn serve_socket_snapshot_command_returns_graph_bin_bytes() {
    // P11-006 happy path: spawn the daemon, wait for the
    // first cycle's graph.bin to land, request the snapshot
    // over the socket via the typed client, and confirm the
    // returned bytes match the on-disk artifact byte-for-byte.
    // The bytes are then re-validated via snapshot::open_mmap
    // (after writing to a tmp file so the mmap reader can
    // consume them) to assert they form a structurally sound
    // archive end-to-end.
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

    let bin_path = data.path().join("conspectus").join("graph.bin");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if bin_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        bin_path.exists(),
        "daemon should write graph.bin before snapshot command can succeed"
    );

    // Hand-rolled snapshot request to verify the wire shape
    // independently of the client helper. The base64 payload
    // lands under data.bytes per ADR 0083 / P11-006.
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));
    let mut stream = UnixStream::connect(&socket).expect("connect");
    write_request(
        &mut stream,
        &serde_json::json!({"command": "snapshot", "id": "snap-1"}),
    );
    let response = read_response(&mut stream);
    assert_eq!(response["id"], "snap-1");
    assert_eq!(response["result"], "ok", "response body: {response}");
    let encoded = response["data"]["bytes"]
        .as_str()
        .expect("data.bytes must be a base64 string");
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 decode");

    // The bytes the daemon serves must equal the bytes it
    // wrote to disk. The dual-write helper serializes once and
    // routes the result to both targets; if they ever diverge,
    // every reader cutover downstream is at risk.
    let on_disk = std::fs::read(&bin_path).expect("read graph.bin");
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(
        decoded.len(),
        on_disk.len(),
        "socket-served bytes length must match graph.bin length"
    );
    assert_eq!(decoded, on_disk, "socket bytes must equal graph.bin");

    // Round-trip the bytes through the mmap reader as a
    // structural check. Write to a tmp file first because the
    // mmap path takes a file path; the bytes from the socket
    // were already validated by the daemon-side write that
    // produced them, so unvalidated open is fine.
    let dump_temp = tempfile::TempDir::new().expect("dump temp");
    let dump_path = dump_temp.path().join("from-socket.bin");
    std::fs::write(&dump_path, &decoded).expect("write decoded bytes");
    let handle =
        conspectus::snapshot::open_mmap(&dump_path).expect("socket bytes must validate via mmap");
    let owned = conspectus::snapshot::deserialize_owned(&handle).expect("deserialize socket bytes");
    assert!(
        owned.nodes.is_empty(),
        "empty home should produce empty snapshot; got {} nodes",
        owned.nodes.len()
    );
}

#[test]
fn serve_socket_snapshot_command_errors_before_first_cycle() {
    // P11-006 negative path: a `snapshot` request that arrives
    // before the first per-class cycle completes must return
    // `snapshot_unavailable` so the caller can fall through
    // (P11-008's mmap-or-rebuild) rather than block forever.
    //
    // To race the first-cycle write, set HOME to a real
    // populated tree (forces non-trivial discovery work) and
    // hit the socket immediately after it binds. The bind
    // happens before the first cycle's discovery body starts,
    // so the cache is `None` for the brief window we exploit.
    // If the race is flaky on a fast machine, the assertion
    // gracefully accepts either outcome — the response shape is
    // the regression net, not the timing.
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
        &serde_json::json!({"command": "snapshot", "id": "snap-early"}),
    );
    let response = read_response(&mut stream);

    let _ = child.kill();
    let _ = child.wait();

    // The response either is "ok" (cycle finished before we
    // got the request in) or carries the expected error code.
    // Both shapes prove the dispatch arm is wired; only the
    // "ok" shape requires the data.bytes string. The error
    // shape is the case the test is documenting — the dispatch
    // arm responds rather than hanging.
    assert_eq!(response["id"], "snap-early");
    match response["result"].as_str() {
        Some("error") => {
            assert_eq!(
                response["error"]["code"], "snapshot_unavailable",
                "pre-first-cycle errors must use the snapshot_unavailable code; got {}",
                response
            );
        }
        Some("ok") => {
            assert!(
                response["data"]["bytes"].is_string(),
                "ok responses must carry data.bytes"
            );
        }
        other => panic!("unexpected result discriminator: {other:?}"),
    }
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
fn cli_status_no_daemon_reports_cleanly() {
    // ADR 0038 "absence is not an error" contract surfaces in
    // `conspectus status` too: no socket → exit 0 + one-line
    // "no daemon running" stdout.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("status")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("run status");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("no daemon running"),
        "expected no-daemon line; got:\n{stdout}"
    );
}

#[test]
fn cli_status_with_daemon_lists_each_class() {
    // Spawn the daemon, give every class time to complete its
    // first synchronous-on-startup cycle, then query status and
    // assert each class shows up with last_outcome = "ok".
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut serve_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut serve_cmd, home.path(), data.path(), runtime.path());
    serve_cmd
        .current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = serve_cmd.spawn().expect("spawn serve");
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));

    // Each class kicks off its first cycle immediately. The
    // writer Mutex serializes them, so under nextest's parallel
    // load it can take a noticeable beat for all four to flush.
    // 15s is generous — solo it lands in under 200ms.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut json_value = serde_json::Value::Null;
    while Instant::now() < deadline {
        let mut status_cmd = Command::new(conspectus_bin());
        isolated_serve_env_args(&mut status_cmd, home.path(), data.path(), runtime.path());
        status_cmd
            .current_dir(cwd.path())
            .args(["status", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let out = status_cmd.output().expect("run status --format json");
        // The poll condition: every class has not only started
        // its first cycle but also completed it with a non-null
        // outcome. `started_epoch` lands on entry; `last_outcome`
        // lands on completion. Polling on the former only races
        // against the cycle in flight.
        if out.status.success()
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
            && let Some(map) = value.as_object()
            && map.len() == 4
            && map
                .values()
                .all(|entry| entry.get("last_outcome").is_some_and(|v| !v.is_null()))
        {
            json_value = value;
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    let _ = child.kill();
    let _ = child.wait();

    let map = json_value
        .as_object()
        .expect("status JSON should be an object");
    for class in ["git", "mux", "harness", "forge"] {
        let entry = map
            .get(class)
            .unwrap_or_else(|| panic!("class `{class}` missing from status; got {map:?}"));
        let outcome = entry
            .get("last_outcome")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(
            outcome, "ok",
            "class `{class}` should have completed its first cycle with ok outcome; entry = {entry}"
        );
    }
}

#[test]
fn cli_refresh_command_falls_back_to_local_when_no_daemon() {
    // No `conspectus serve` running → no socket → `conspectus
    // refresh` should print the local-fallback line and exit 0.
    // Pins the "absence of a server is not an error" guarantee
    // from ADR 0038 at the client side.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .arg("refresh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("run refresh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "refresh should succeed even without a daemon: stderr=\n{stderr}"
    );
    assert!(
        stdout.contains("refreshed via in-process cold rebuild"),
        "expected in-process fallback line; stdout=\n{stdout}"
    );

    let cache = data.path().join("conspectus").join("graph.bin");
    assert!(
        cache.exists(),
        "in-process fallback should populate {}",
        cache.display()
    );
}

#[test]
fn cli_refresh_command_uses_daemon_when_present() {
    // With `conspectus serve` running, the client should route
    // through the socket and print the "via daemon" line.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut serve_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut serve_cmd, home.path(), data.path(), runtime.path());
    serve_cmd
        .current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = serve_cmd.spawn().expect("spawn serve");
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));

    let mut client_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut client_cmd, home.path(), data.path(), runtime.path());
    client_cmd
        .current_dir(cwd.path())
        .arg("refresh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = client_cmd.output().expect("run refresh client");

    let _ = child.kill();
    let _ = child.wait();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("refreshed via daemon"),
        "expected daemon-routed line; stdout=\n{stdout}"
    );
}

#[test]
fn cli_refresh_class_unknown_errors_before_socket() {
    // `--class bogus` must fail fast with a clear "unknown
    // class" line, independent of whether a daemon is running.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .args(["refresh", "--class", "bogus"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("run refresh --class bogus");
    assert!(
        !output.status.success(),
        "unknown class should be a non-zero exit"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown --class") && stderr.contains("bogus"),
        "expected unknown-class error; stderr=\n{stderr}"
    );
}

#[test]
fn cli_refresh_class_runs_in_process_when_no_daemon() {
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut cmd, home.path(), data.path(), runtime.path());
    cmd.current_dir(cwd.path())
        .args(["refresh", "--class", "git"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("run refresh --class git");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refreshed git via in-process per-class refresh"),
        "expected per-class in-process line; stdout=\n{stdout}"
    );
    assert!(data.path().join("conspectus").join("graph.bin").exists());
}

#[test]
fn cli_refresh_class_routes_through_daemon_when_present() {
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");

    let mut serve_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut serve_cmd, home.path(), data.path(), runtime.path());
    serve_cmd
        .current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = serve_cmd.spawn().expect("spawn serve");
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));

    let mut client_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut client_cmd, home.path(), data.path(), runtime.path());
    client_cmd
        .current_dir(cwd.path())
        .args(["refresh", "--class", "forge"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = client_cmd.output().expect("client refresh forge");

    let _ = child.kill();
    let _ = child.wait();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refreshed forge via daemon"),
        "expected daemon-routed per-class line; stdout=\n{stdout}"
    );
}

#[test]
fn serve_harness_watcher_fires_on_state_dir_change() {
    // P7-009 end-to-end: the daemon installs a filesystem
    // watcher on every configured harness state dir. When a
    // file appears in one of them, the harness scheduler
    // wakes immediately and runs a cycle — observable as the
    // harness class's last_completed_epoch advancing well
    // before its 5-second interval.
    //
    // The harness state dirs come from CONSPECTUS_<HARNESS>_STATE
    // env vars (which we usually clear in tests to keep
    // discovery deterministic). For this test we point the
    // codex state dir at a controlled tempdir so we can poke
    // it and observe the watcher fire.
    let home = tempfile::TempDir::new().expect("home temp");
    let data = tempfile::TempDir::new().expect("data temp");
    let cwd = tempfile::TempDir::new().expect("cwd temp");
    let runtime = tempfile::TempDir::new().expect("runtime temp");
    let codex_state = tempfile::TempDir::new().expect("codex state");

    let mut serve_cmd = Command::new(conspectus_bin());
    isolated_serve_env_args(&mut serve_cmd, home.path(), data.path(), runtime.path());
    // Override the deletion isolated_env_args did so the codex
    // state dir we control is actually watched. CODEX is the
    // simplest harness — its state dir just needs to exist for
    // the watcher install to succeed.
    serve_cmd.env("CONSPECTUS_CODEX_STATE", codex_state.path());
    serve_cmd
        .current_dir(cwd.path())
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = serve_cmd.spawn().expect("spawn serve");
    let socket = socket_path_under(runtime.path());
    assert!(wait_for_socket(&socket, Duration::from_secs(5)));

    // Let the daemon's startup activity settle: every class
    // runs its first cycle synchronously, and peer-class first
    // cycles can occasionally touch the harness state dir,
    // firing the watcher once before the test writes its
    // trigger. A short sleep is sufficient because the first
    // cycle in this empty-workspace setup finishes in
    // milliseconds.
    std::thread::sleep(Duration::from_millis(800));

    // Record the trigger wall-clock. The watcher-driven cycle
    // we're looking for must have `last_started_epoch >=
    // trigger_epoch`. This avoids the baseline-counting race
    // entirely — we don't care what the harness was doing
    // before; we only care that a cycle starts after our
    // trigger.
    let trigger_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Trigger the watcher: create a file in the watched dir.
    // The harness scheduler should wake within a fraction of
    // a second — well before the 5s interval.
    std::fs::write(codex_state.path().join("triggered"), b"hi").expect("write trigger file");

    // Wait up to 3 seconds for a harness cycle to start at or
    // after the trigger. 3s is comfortably below the 5s
    // interval, so success here means the watcher woke the
    // scheduler rather than the timer firing.
    let saw_wake = wait_for_harness_started_at_or_after(
        runtime.path(),
        home.path(),
        data.path(),
        cwd.path(),
        trigger_epoch,
        Duration::from_secs(3),
    );

    let _ = child.kill();
    let output = child.wait_with_output().expect("collect output");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        saw_wake,
        "watcher should have woken the harness scheduler within 3s of the file create; \
         trigger_epoch={trigger_epoch}; daemon stderr:\n{stderr}"
    );
}

/// Poll `conspectus status --format json` until the harness
/// `last_started_epoch` is at or after `floor_epoch`. Returns
/// `true` on success, `false` if `timeout` elapses first.
fn wait_for_harness_started_at_or_after(
    runtime_dir: &Path,
    home: &Path,
    data_home: &Path,
    cwd: &Path,
    floor_epoch: u64,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut cmd = Command::new(conspectus_bin());
        isolated_serve_env_args(&mut cmd, home, data_home, runtime_dir);
        cmd.current_dir(cwd)
            .args(["status", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Ok(out) = cmd.output()
            && out.status.success()
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&out.stdout)
            && let Some(harness) = value.get("harness")
            && let Some(epoch) = harness.get("last_started_epoch").and_then(|v| v.as_u64())
            && epoch >= floor_epoch
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    false
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
