//! Source-level checks of the capability manifest (ADR 0117):
//! `docs/capabilities.toml` names everything Conspectus may run,
//! connect to, signal, and write. These tests fail when shipped code
//! starts a process outside `src/exec.rs`, reaches for a network or
//! process API the manifest doesn't allow, or depends on a networking
//! crate. `tests/capabilities.rs` checks the built binary's behavior.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use conspectus::exec::Program;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn manifest() -> toml::Table {
    let text = fs::read_to_string(root().join("docs/capabilities.toml")).expect("read manifest");
    text.parse().expect("parse docs/capabilities.toml")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The shipped lines of a source file, numbered from 1: test-only files
/// are skipped, a trailing `#[cfg(test)] mod` is cut off, and comments
/// are dropped.
fn shipped_lines(path: &Path) -> Vec<(usize, String)> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if name.ends_with("_tests.rs") || name == "tests.rs" {
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
/// to the crate root), or nowhere when `allowed_in` is empty.
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

#[test]
fn shipped_code_uses_only_the_process_and_network_apis_the_manifest_allows() {
    let mut files = Vec::new();
    rust_files(&root().join("src"), &mut files);
    files.sort();
    let mut violations = Vec::new();
    for path in &files {
        let relative = path
            .strip_prefix(root())
            .expect("under the crate root")
            .to_string_lossy()
            .replace('\\', "/");
        for (number, line) in shipped_lines(path) {
            for rule in RULES {
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
    assert!(
        violations.is_empty(),
        "shipped code outside the capability manifest (docs/capabilities.toml, ADR 0117):\n{}",
        violations.join("\n")
    );
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
    let adr_dir = root().join("docs/adr");
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
    let lock: toml::Table = fs::read_to_string(root().join("Cargo.lock"))
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
