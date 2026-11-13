//! The process working directory can vanish while the program runs, so
//! only `src/cwd.rs` reads it; everything else goes through that
//! module's helpers, which degrade instead of failing (ADR 0111).
//! Setting a child's directory with `Command::current_dir` is fine.
//!
//! The rule covers the host (`src`) and the framework (`crates/tuikit/src`).
//! The framework has no helper module to be exempt: a control that needs a
//! directory, such as path completion, takes it from its host as an input.
//! `tests/support/source_tree.rs` explains which hygiene rules apply to which
//! crate and how the scans are guarded against visiting too few files.

use std::fs;
use std::path::Path;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, HOST_SRC, Tree};

/// The trees whose code must not read the working directory.
const TREES: &[Tree] = &[HOST_SRC, FRAMEWORK_SRC];

/// The only files that may read the working directory, relative to the
/// workspace root. Both belong to the host; no framework file is exempt.
const ALLOWED: &[&str] = &["src/cwd.rs", "src/cwd_tests.rs"];

/// A read of the process working directory, as opposed to a
/// `.current_dir(..)` builder call on a `Command` or the module's own
/// `from_current_dir` constructor.
fn reads_process_cwd(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or(line);
    code.match_indices("current_dir(").any(|(index, _)| {
        code[..index]
            .chars()
            .next_back()
            .is_none_or(|prev| prev != '.' && prev != '_' && !prev.is_alphanumeric())
    })
}

/// Every working-directory read under `trees` outside the exempt files, as
/// `path:line: text` findings with the path relative to `root`.
fn problems(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut problems = Vec::new();
    for path in &source_tree::rust_files(root, trees) {
        let relative = source_tree::relative(root, path);
        if ALLOWED.contains(&relative.as_str()) {
            continue;
        }
        let text = fs::read_to_string(path).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if reads_process_cwd(line) {
                problems.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    problems
}

#[test]
fn only_the_cwd_module_reads_the_working_directory() {
    let problems = problems(source_tree::workspace_root(), TREES);
    assert!(
        problems.is_empty(),
        "read the working directory through `crate::cwd` (`current()` or \
         `for_default_target(flag)`) so a deleted directory can't fail the \
         command; framework code takes the directory from its host instead:\n{}",
        problems.join("\n")
    );
}

#[test]
fn matcher_tells_reads_from_command_builders() {
    assert!(reads_process_cwd("let cwd = std::env::current_dir()?;"));
    assert!(reads_process_cwd("    env::current_dir().ok()"));
    assert!(!reads_process_cwd("Command::new(git).current_dir(root)"));
    assert!(!reads_process_cwd("        .current_dir(cwd)"));
    assert!(!reads_process_cwd("// std::env::current_dir() is banned"));
    assert!(!reads_process_cwd(
        "pub fn from_current_dir() -> Result<Self>"
    ));
    assert!(reads_process_cwd("let here = current_dir()?;"));
}

#[test]
fn a_working_directory_read_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn here() -> std::path::PathBuf {\n    std::env::current_dir().unwrap()\n}\n";
    source_tree::assert_scan_covers_every_tree(TREES, violation, problems);
}

#[test]
fn only_the_hosts_own_cwd_files_are_exempt() {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    let read = "fn here() { let _ = std::env::current_dir(); }\n";
    for exempt in ["src/cwd.rs", "src/cwd_tests.rs"] {
        let path = scratch.path().join(exempt);
        fs::create_dir_all(path.parent().expect("a parent directory")).expect("create tree");
        fs::write(path, read).expect("plant the read");
    }
    let lookalike = scratch.path().join("crates/tuikit/src/cwd.rs");
    fs::create_dir_all(lookalike.parent().expect("a parent directory")).expect("create tree");
    fs::write(lookalike, read).expect("plant the read");

    let lowered = [Tree::new("src", 1), Tree::new("crates/tuikit/src", 1)];
    let found = problems(scratch.path(), &lowered);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("crates/tuikit/src/cwd.rs:1:"),
        "{found:?}"
    );
}
