//! A test that writes a program it runs (a stand-in `gh`, `tmux`, or
//! harness) writes it with the shared helper, `tests/support/program.rs`
//! (ADR 0241). Written any other way, the program fails to start now and
//! then under `cargo test` with "Text file busy"; the helper's module
//! comment explains the race.
//!
//! The scan here fails on test code that gives a file an execute bit by
//! hand: a mode literal with an execute bit passed to `from_mode`, `mode`,
//! or `set_mode`, anywhere but the helper. Test code is every file under
//! `tests/`, the library's unit-test files (`tests.rs`, `*_tests.rs`), and
//! the test module that ends a source file, from its `#[cfg(test)]` and
//! `mod` lines on. A test that changes a directory's permissions keeps the
//! permissions it found and gives them back (or masks them) instead of
//! writing a mode with execute bits, so it never needs an exception. The
//! framework writes no file at all, which its own guard enforces, so the
//! scan covers the host only.
//!
//! The last test checks the helper's promise itself: a program it writes
//! starts at once while other threads of the same process keep forking.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "support/source_tree.rs"]
mod source_tree;

#[path = "support/program.rs"]
mod program;

use source_tree::{HOST_SRC, HOST_TESTS, Tree};

/// The trees whose test code is scanned.
const TREES: &[Tree] = &[HOST_SRC, HOST_TESTS];

/// The one file allowed to set an execute bit.
const HELPER: &str = "tests/support/program.rs";

/// The calls that take a mode.
const MODE_CALLS: &[&str] = &["from_mode(", ".mode(", "set_mode("];

/// The mode literal at the start of `args` (`0o755`, `0o7_55`), if the
/// argument is one.
fn mode_literal(args: &str) -> Option<u32> {
    let digits = args.trim_start().strip_prefix("0o")?;
    let digits: String = digits
        .chars()
        .take_while(|c| c.is_digit(8) || *c == '_')
        .filter(|c| *c != '_')
        .collect();
    u32::from_str_radix(&digits, 8).ok()
}

/// Where test code starts in a file at `relative`: the first line, or the
/// line of the trailing `#[cfg(test)]` module of a production source file.
/// `None` for a file with no test code.
fn test_code_start(relative: &str, lines: &[&str]) -> Option<usize> {
    if relative.starts_with("tests/") || source_tree::is_test_file(Path::new(relative)) {
        return Some(0);
    }
    lines.windows(2).position(|pair| {
        pair[0].trim() == "#[cfg(test)]" && pair[1].trim_start().starts_with("mod ")
    })
}

/// Each mode literal with an execute bit in the test code of `text`, the
/// file at `relative`, as `path:line: ...` findings.
fn findings_in(relative: &str, text: &str) -> Vec<String> {
    if relative == HELPER {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = test_code_start(relative, &lines) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start) {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for call in MODE_CALLS {
            for (at, _) in line.match_indices(call) {
                let Some(mode) = mode_literal(&line[at + call.len()..]) else {
                    continue;
                };
                if mode & 0o111 != 0 {
                    findings.push(format!(
                        "{relative}:{}: `{call}` is given 0o{mode:o}, an execute bit by hand",
                        index + 1
                    ));
                }
            }
        }
    }
    findings
}

/// Every finding under `trees`.
fn offenders(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut offenders = Vec::new();
    for path in source_tree::rust_files(root, trees) {
        let text = fs::read_to_string(&path).expect("read source file");
        offenders.extend(findings_in(&source_tree::relative(root, &path), &text));
    }
    offenders
}

#[test]
fn tests_write_the_programs_they_run_through_the_helper() {
    let offenders = offenders(source_tree::workspace_root(), TREES);
    assert!(
        offenders.is_empty(),
        "write these programs with `write_program` or `copy_program` from \
         `tests/support/program.rs`, which explains why (ADR 0241):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn a_program_made_executable_by_hand_is_caught_in_every_tree_the_scan_covers() {
    // Split so this file does not hold the call it plants.
    let call = concat!("from_", "mode(0o755)");
    let violation = format!(
        "#[cfg(test)]\nmod tests {{\n    fn f(p: &Path) {{\n        \
         fs::set_permissions(p, fs::Permissions::{call}).unwrap();\n    }}\n}}\n"
    );
    source_tree::assert_scan_covers_every_tree(TREES, &violation, offenders);
}

#[test]
fn the_scan_reads_mode_literals_in_test_code_only() {
    let by_hand = concat!("    let _ = Permissions::from_", "mode(0o7_00);\n");
    let created = concat!("    options.mode(0o", "755);\n");
    let harmless = "    let _ = Permissions::from_mode(0o644);\n\
                    // Permissions::from_mode(0o755) in a comment\n\
                    perms.set_mode(perms.mode() & 0o700);\n";
    let production = format!("fn shipped() {{\n{by_hand}}}\n");
    let with_tests = format!("{production}\n#[cfg(test)]\nmod tests {{\n{harmless}{created}}}\n");

    assert_eq!(findings_in("src/a.rs", &production), Vec::<String>::new());
    assert_eq!(
        findings_in("src/a.rs", &with_tests),
        ["src/a.rs:10: `.mode(` is given 0o755, an execute bit by hand"]
    );
    assert_eq!(
        findings_in("src/a_tests.rs", &production),
        ["src/a_tests.rs:2: `from_mode(` is given 0o700, an execute bit by hand"]
    );
    assert_eq!(
        findings_in("tests/cli.rs", &production).len(),
        1,
        "every file under tests/ is test code"
    );
    assert_eq!(findings_in(HELPER, &production), Vec::<String>::new());
}

#[test]
fn a_program_written_by_the_helper_starts_while_other_threads_fork() {
    let scratch = tempfile::tempdir().expect("scratch");
    let stop = Arc::new(AtomicBool::new(false));
    let forkers: Vec<_> = (0..4)
        .map(|_| {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let _ = Command::new("/bin/sh").args(["-c", ":"]).status();
                }
            })
        })
        .collect();

    let failures: Vec<String> = (0..100)
        .filter_map(|n| {
            let path = scratch.path().join(format!("program-{n}"));
            program::write_program(&path, "#!/bin/sh\nexit 0\n");
            match Command::new(&path).status() {
                Ok(status) if status.success() => None,
                outcome => Some(format!("program-{n}: {outcome:?}")),
            }
        })
        .collect();

    stop.store(true, Ordering::Relaxed);
    for forker in forkers {
        forker.join().expect("forking thread");
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
