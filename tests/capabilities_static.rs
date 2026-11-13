//! Source-level checks of the capability manifest (ADR 0117):
//! `docs/capabilities.toml` names everything Conspectus may run,
//! connect to, signal, and write. These tests fail when shipped code
//! starts a process outside `src/exec.rs`, reaches for a network or
//! process API the manifest doesn't allow, or depends on a networking
//! crate. `tests/capabilities.rs` checks the built binary's behavior.
//!
//! The framework (`crates/tuikit`) ships inside the same binary but has no
//! manifest entries: it starts no process and opens no socket, signal, or
//! write path. Instead of skipping its tree, the checks here fail on any
//! such path in it, with no per-file allowance. The lockfile is shared, so
//! the networking-crate check already covers the framework's dependencies.
//! `tests/support/source_tree.rs` explains which hygiene rules apply to
//! which crate and how the scans are guarded against visiting too few files.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use conspectus::exec::Program;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, HOST_SRC, Tree, workspace_root};

fn manifest() -> toml::Table {
    let text =
        fs::read_to_string(workspace_root().join("docs/capabilities.toml")).expect("read manifest");
    text.parse().expect("parse docs/capabilities.toml")
}

/// The shipped lines of a source file, numbered from 1: test-only files
/// are skipped, a trailing `#[cfg(test)] mod` is cut off, and comments
/// are dropped.
fn shipped_lines(path: &Path) -> Vec<(usize, String)> {
    if source_tree::is_test_file(path) {
        return Vec::new();
    }
    let text = fs::read_to_string(path).expect("read source");
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let next = lines.get(index + 1).map_or("", |next| next.trim_start());
        if line.trim() == "#[cfg(test)]" && next.starts_with("mod ") {
            break;
        }
        let code = match line.trim_start() {
            trimmed if trimmed.starts_with("//") => "",
            _ => line.find(" // ").map_or(*line, |at| &line[..at]),
        };
        if !code.trim().is_empty() {
            out.push((index + 1, code.to_string()));
        }
    }
    out
}

/// A pattern shipped code may only use in `allowed_in` (paths relative
/// to the workspace root), or nowhere when `allowed_in` is empty.
#[derive(Clone, Copy)]
struct Rule {
    pattern: &'static str,
    allowed_in: &'static [&'static str],
    why: &'static str,
}

const RULES: &[Rule] = &[
    Rule {
        pattern: "Command::new",
        allowed_in: &["src/exec.rs"],
        why: "start processes through `exec::command` with a `Program` from the manifest",
    },
    Rule {
        pattern: "CommandExt",
        allowed_in: &[],
        why: "`exec`, `pre_exec`, and friends bypass the manifest's process model",
    },
    Rule {
        pattern: "std::net",
        allowed_in: &[],
        why: "Conspectus opens no network sockets; `gh` is its only network path",
    },
    Rule {
        pattern: "TcpStream",
        allowed_in: &[],
        why: "Conspectus opens no network sockets",
    },
    Rule {
        pattern: "TcpListener",
        allowed_in: &[],
        why: "Conspectus opens no network sockets",
    },
    Rule {
        pattern: "UdpSocket",
        allowed_in: &[],
        why: "Conspectus opens no network sockets",
    },
    Rule {
        pattern: "UnixStream",
        allowed_in: &["src/server/mod.rs"],
        why: "the daemon socket is the only Unix socket in the manifest",
    },
    Rule {
        pattern: "UnixListener",
        allowed_in: &["src/server/mod.rs"],
        why: "the daemon socket is the only Unix socket in the manifest",
    },
    Rule {
        pattern: "libc::kill",
        allowed_in: &["src/discovery/tmux/teardown.rs"],
        why: "signals are sent only by the operator-initiated teardown",
    },
    Rule {
        pattern: "libc::fork",
        allowed_in: &[],
        why: "start processes through `exec::command`",
    },
    Rule {
        pattern: "libc::vfork",
        allowed_in: &[],
        why: "start processes through `exec::command`",
    },
    Rule {
        pattern: "libc::exec",
        allowed_in: &[],
        why: "start processes through `exec::command`",
    },
    Rule {
        pattern: "libc::posix_spawn",
        allowed_in: &[],
        why: "start processes through `exec::command`",
    },
    Rule {
        pattern: "libc::system",
        allowed_in: &[],
        why: "start processes through `exec::command`",
    },
    Rule {
        pattern: "libc::socket",
        allowed_in: &[],
        why: "sockets go through the daemon's `UnixListener`/`UnixStream`",
    },
    Rule {
        pattern: "libc::connect",
        allowed_in: &[],
        why: "sockets go through the daemon's `UnixListener`/`UnixStream`",
    },
    Rule {
        pattern: "libc::ptrace",
        allowed_in: &[],
        why: "Conspectus never inspects other processes' memory",
    },
];

/// What the framework may not do on top of `RULES`: start a process by any
/// route or write to the filesystem. The host's writes are bounded by the
/// manifest and traced in `tests/capabilities.rs`; the framework has none.
const FRAMEWORK_EXTRA_RULES: &[Rule] = &[
    Rule {
        pattern: "std::process",
        allowed_in: &[],
        why: "the framework starts no process; hosts supply what it needs",
    },
    Rule {
        pattern: "File::create",
        allowed_in: &[],
        why: "the framework writes no file; hosts supply a sink",
    },
    Rule {
        pattern: "File::options",
        allowed_in: &[],
        why: "the framework writes no file; hosts supply a sink",
    },
    Rule {
        pattern: "OpenOptions",
        allowed_in: &[],
        why: "the framework writes no file; hosts supply a sink",
    },
    Rule {
        pattern: "fs::write",
        allowed_in: &[],
        why: "the framework writes no file; hosts supply a sink",
    },
    Rule {
        pattern: "fs::copy",
        allowed_in: &[],
        why: "the framework writes no file; hosts supply a sink",
    },
    Rule {
        pattern: "fs::rename",
        allowed_in: &[],
        why: "the framework moves no file",
    },
    Rule {
        pattern: "hard_link",
        allowed_in: &[],
        why: "the framework creates no file link",
    },
    Rule {
        pattern: "symlink",
        allowed_in: &[],
        why: "the framework creates no file link",
    },
    Rule {
        pattern: "set_permissions",
        allowed_in: &[],
        why: "the framework changes no file permissions",
    },
    Rule {
        pattern: "remove_file",
        allowed_in: &[],
        why: "the framework deletes no file",
    },
    Rule {
        pattern: "remove_dir",
        allowed_in: &[],
        why: "the framework deletes no directory",
    },
    Rule {
        pattern: "create_dir",
        allowed_in: &[],
        why: "the framework creates no directory",
    },
];

/// The rules for the framework tree: every rule of `RULES` with its
/// allowance removed (the files it names are host files), plus
/// `FRAMEWORK_EXTRA_RULES`.
fn framework_rules() -> Vec<Rule> {
    RULES
        .iter()
        .map(|rule| Rule {
            allowed_in: &[],
            ..*rule
        })
        .chain(FRAMEWORK_EXTRA_RULES.iter().copied())
        .collect()
}

/// Shipped lines under `trees` that match one of `rules` outside the
/// files the rule allows, as `path:line: ...` findings.
fn violations(root: &Path, trees: &[Tree], rules: &[Rule]) -> Vec<String> {
    let mut violations = Vec::new();
    for path in source_tree::rust_files(root, trees) {
        let relative = source_tree::relative(root, &path);
        for (number, line) in shipped_lines(&path) {
            for rule in rules {
                if line.contains(rule.pattern) && !rule.allowed_in.contains(&relative.as_str()) {
                    violations.push(format!(
                        "{relative}:{number}: `{}`: {}\n    {}",
                        rule.pattern,
                        rule.why,
                        line.trim()
                    ));
                }
            }
        }
    }
    violations
}

#[test]
fn shipped_code_uses_only_the_process_and_network_apis_the_manifest_allows() {
    let violations = violations(workspace_root(), &[HOST_SRC], RULES);
    assert!(
        violations.is_empty(),
        "shipped code outside the capability manifest (docs/capabilities.toml, ADR 0117):\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_framework_has_no_process_socket_signal_or_write_path() {
    let violations = violations(workspace_root(), &[FRAMEWORK_SRC], &framework_rules());
    assert!(
        violations.is_empty(),
        "the framework crate must start no process and open no socket, signal, or write path, \
         so it has no entry in the capability manifest (ADR 0117, ADR 0150). Pass a sink or a \
         source in from the host instead:\n{}",
        violations.join("\n")
    );
}

#[test]
fn a_process_call_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn f() { let _ = std::process::Command::new(\"x\"); }\n";
    source_tree::assert_scan_covers_every_tree(
        &[HOST_SRC, FRAMEWORK_SRC],
        violation,
        |root, lowered| {
            let (host, framework) = lowered.split_at(1);
            let mut found = violations(root, host, RULES);
            found.extend(violations(root, framework, &framework_rules()));
            found
        },
    );
}

#[test]
fn a_write_path_is_caught_in_the_framework_tree() {
    let violation = "fn f() { let _ = std::fs::write(\"x\", \"y\"); }\n";
    source_tree::assert_scan_covers_every_tree(&[FRAMEWORK_SRC], violation, |root, lowered| {
        violations(root, lowered, &framework_rules())
    });
}

#[test]
fn every_program_has_a_manifest_entry_and_every_entry_a_program() {
    let manifest = manifest();
    let listed: BTreeSet<String> = manifest["programs"]
        .as_table()
        .expect("[programs]")
        .keys()
        .cloned()
        .collect();
    let known: BTreeSet<String> = Program::ALL
        .iter()
        .map(|program| program.key().to_string())
        .collect();
    assert_eq!(
        listed, known,
        "`exec::Program` and docs/capabilities.toml [programs] must list the same programs"
    );
}

#[test]
fn every_manifest_section_cites_adrs_that_exist() {
    let manifest = manifest();
    let mut sections: Vec<(String, &toml::Table)> = Vec::new();
    for (name, program) in manifest["programs"].as_table().expect("[programs]") {
        sections.push((
            format!("programs.{name}"),
            program.as_table().expect("program table"),
        ));
    }
    for name in ["sockets", "signals", "writes", "dependencies"] {
        sections.push((
            name.to_string(),
            manifest[name].as_table().expect("section table"),
        ));
    }
    let adr_dir = workspace_root().join("docs/adr");
    let adrs: Vec<String> = fs::read_dir(&adr_dir)
        .expect("read docs/adr")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    for (name, section) in sections {
        let cited = section
            .get("adr")
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("[{name}] cites no ADR"));
        assert!(!cited.is_empty(), "[{name}] cites no ADR");
        for adr in cited {
            let number = adr.as_str().expect("ADR number as a string");
            assert!(
                adrs.iter()
                    .any(|file| file.starts_with(&format!("{number}-"))),
                "[{name}] cites ADR {number}, which doesn't exist"
            );
        }
    }
}

#[test]
fn no_networking_crate_is_a_dependency() {
    let manifest = manifest();
    let denied: BTreeSet<&str> = manifest["dependencies"]["deny"]
        .as_array()
        .expect("[dependencies] deny")
        .iter()
        .map(|name| name.as_str().expect("crate name"))
        .collect();
    let lock: toml::Table = fs::read_to_string(workspace_root().join("Cargo.lock"))
        .expect("read Cargo.lock")
        .parse()
        .expect("parse Cargo.lock");
    let present: Vec<&str> = lock["package"]
        .as_array()
        .expect("[[package]]")
        .iter()
        .filter_map(|package| package["name"].as_str())
        .filter(|name| denied.contains(name))
        .collect();
    assert!(
        present.is_empty(),
        "networking crates in Cargo.lock (docs/capabilities.toml [dependencies], ADR 0117): {present:?}"
    );
}
