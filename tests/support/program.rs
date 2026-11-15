//! Stand-in programs for tests: a fake `gh`, `tmux`, or harness that a
//! test writes and then runs, or puts where the code under test runs it.
//!
//! A test that writes a program it runs writes it with [`write_program`],
//! or copies an existing binary with [`copy_program`] (ADR 0241).
//! `tests/program_hygiene.rs` fails on test code that sets an execute bit
//! anywhere else. The library's unit tests reach this file as
//! `crate::test_program`, and an integration test includes it with
//! `#[path = "support/program.rs"] mod program;`.
//!
//! # Why a helper
//!
//! Under `cargo test` the tests of one binary run as threads of one
//! process. Starting a child forks that process, and the child holds a copy
//! of every descriptor open at that instant until its own `exec` closes the
//! close-on-exec ones. The kernel refuses to execute a file that any
//! process holds open for writing (`ETXTBSY`, "Text file busy"). So a test
//! that writes its program with `fs::write` and then runs it fails now and
//! then: when another test forks while the write descriptor is open, the
//! forked child can still hold that descriptor after `fs::write` returns,
//! and an `exec` in that window fails. Syncing the file, renaming it into
//! place, or retrying the test does not remove the window, since the
//! inherited descriptor still names the file. Nextest runs each test in a
//! process of its own, so it never shows the failure; `cargo test` does.
//!
//! [`write_program`] writes the file from a short-lived `/bin/sh` child
//! instead, so the write descriptor exists only in that child, and no test
//! thread forks from it. A process's descriptors are closed before its
//! parent can reap it, so when the call returns nothing holds the file
//! open for writing. The execute bit is set afterwards by path (`chmod`),
//! which opens no descriptor.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Writes `content` to `path` as an executable program (mode 0755) and
/// returns the path. The parent directory must exist. Give each program
/// its own path: a test that needs a second stand-in writes it beside the
/// first, not over it.
///
/// `content` travels as one argument to the writing shell, so it is text
/// of at most 128 KiB, which every stand-in script is.
pub fn write_program(path: impl AsRef<Path>, content: &str) -> PathBuf {
    let path = path.as_ref();
    // `printf` is a shell builtin, so this needs no `PATH`.
    let status = Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$2" > "$1""#, "sh"])
        .arg(path)
        .arg(content)
        .stdin(Stdio::null())
        .status()
        .unwrap_or_else(|err| panic!("start the shell that writes {}: {err}", path.display()));
    assert!(status.success(), "write {}: {status}", path.display());
    make_executable(path);
    path.to_path_buf()
}

/// Copies the program at `from` to `to` (mode 0755) and returns `to`, with
/// the copy made by a `cp` child for the same reason [`write_program`]
/// writes from one.
pub fn copy_program(from: impl AsRef<Path>, to: impl AsRef<Path>) -> PathBuf {
    let (from, to) = (from.as_ref(), to.as_ref());
    let status = Command::new("cp")
        .arg("--")
        .arg(from)
        .arg(to)
        .stdin(Stdio::null())
        .status()
        .unwrap_or_else(|err| panic!("start cp for {}: {err}", to.display()));
    assert!(
        status.success(),
        "copy {} to {}: {status}",
        from.display(),
        to.display()
    );
    make_executable(to);
    to.to_path_buf()
}

fn make_executable(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|err| panic!("make {} executable: {err}", path.display()));
}
