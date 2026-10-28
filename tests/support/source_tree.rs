//! Shared plumbing for the source-scanning tests: which trees a scan
//! walks, the tripwire that fails a scan which visited too few files, and
//! the check that proves a scan really covers every tree it names.
//!
//! The workspace has two crates with source: the host (`src`, the
//! Conspectus application) and the framework (`crates/tuikit/src`). Code
//! moves from the first into the second over time, so a scan pinned to
//! `src` would keep passing while checking less and less. Every scan names
//! its trees through this module instead, resolved from the workspace
//! root, and fails when a tree is nearly empty.
//!
//! Each test includes this file with `#[path]` rather than `mod support;`,
//! so the hygiene tests do not compile the fixture and replay helpers. The
//! consistency tests inside the library include it the same way.
//!
//! # Which hygiene rule applies to which crate
//!
//! A rule applies to the framework when it guards how code is written. It
//! stays host-only when it guards what the host does or says.
//!
//! | Rule (test) | Host | Framework | Why |
//! | --- | --- | --- | --- |
//! | Inline `mod tests` under 100 lines (`test_module_hygiene`) | `src` | `crates/tuikit/src` | Readable production code is a property of any Rust file, so one limit holds in both crates. |
//! | No backlog IDs in comments (`comment_hygiene`) | `src`, `tests`, `examples` | the same three directories under `crates/tuikit` | Comments explain behavior; IDs belong in commits and the backlog. The framework can be lifted into another repository, where those IDs mean nothing. |
//! | Overlay keys resolve through key tables, footers are generated (`overlay_keymap_hygiene`) | `src/tui/widgets`, `runtime/overlay_keys.rs`, `modal.rs` | all of `crates/tuikit/src` except the files listed in that test | Controls and overlay handlers move into the framework, so the rule follows them. The key-table model has to name key codes itself; it is listed as an exception when it arrives. |
//! | Process working directory read only through the `cwd` helper (`cwd_hygiene`) | `src`, with `src/cwd.rs` and `src/cwd_tests.rs` exempt | `crates/tuikit/src`, no file exempt | The directory can vanish while the program runs, so a read that fails or shifts meaning is a defect in any code. A control that needs a directory (path completion) takes it from its host as an input, so the framework has no helper module to be exempt and a file named `cwd.rs` there is still scanned. |
//! | Terminal taken over and handed back only through the `term` helpers (`terminal_hygiene`) | `src`, with `src/tui/runtime/term.rs` exempt | `crates/tuikit/src`, no file exempt | The helpers keep bracketed paste on exactly while the TUI has the terminal; a direct `ratatui::init` or `ratatui::restore` would leave it off in the TUI or on in the shell. The framework draws into frames its host gives it and never takes the terminal. |
//! | Capability manifest (`capabilities_static`) | `src`, checked against `docs/capabilities.toml` | none: no manifest entries, and the test asserts there is no process, socket, signal, or write path | The framework starts no process and writes no file, so there is nothing to list. The test fails on any such path rather than skipping the tree. The lockfile is shared, so the networking-crate check already covers the framework's dependencies. |
//! | Developer artifacts in string literals: ADR numbers, backlog IDs, design-document paths (`consistency_tests/text.rs`) | `src/tui`, `src/cli` | `crates/tuikit/src` | Text the framework draws reaches the operator through the host, so it obeys the same rule. |
//! | "slot" where the operator says "pin" (`consistency_tests/text.rs`) | `src/tui` | not applied | A pin is a Conspectus word the framework may not name, so a "slot" there is a generic word for something else. |
//! | Rendered screens, help pages, and composed hints; `docs/tui-columns.md` (`consistency_tests/text.rs`, `docs.rs`) | the whole application | not applied | They exercise the application and its left panel. Framework controls reach them through the screens that draw them. |
//!
//! The framework's own guard (`crates/tuikit/tests/independence.rs`)
//! enforces what only the framework needs: no host dependency, no domain
//! vocabulary, no process or write path.
//!
//! The framework's contribution guide describes the same scans in terms the
//! framework may use (its documents cannot name the host), under
//! "Repository-wide hygiene scans" in `crates/tuikit/docs/contributing.md`.
//! Change the two together so they do not drift.
//!
//! # Tripwires
//!
//! Every [`Tree`] states the fewest `.rs` files its scan may visit, and
//! [`rust_files`] fails with the tree, the count, and the minimum when a
//! scan finds fewer. A scan pointed at the wrong directory therefore fails
//! instead of passing empty.
//!
//! * Pick a minimum a little under the current count (about three quarters;
//!   `find <dir> -name '*.rs' | wc -l`), not equal to it, so deleting a few
//!   files does not trip it and a wrong directory still does.
//! * When a story moves files from `src` into `crates/tuikit/src`, raise
//!   `FRAMEWORK_SRC` in the same change and lower `HOST_SRC` if the move
//!   took the host under its minimum. The sum of the two should not shrink.
//! * `FRAMEWORK_SRC` asks for 9 of the 13 files the crate holds today:
//!   `lib.rs` and the `theme` and `glyphs` modules with their sibling test
//!   files.
//!   `FRAMEWORK_EXAMPLES` is 0 until the first example exists; raise it to 1
//!   with that example.
//! * A tree can name a single file (`src/tui/modal.rs`). Its minimum is 1,
//!   so moving or deleting the file fails the scan until the list is
//!   updated.
//!
//! `tests/source_scan_roots.rs` checks this module itself, and each scan has
//! a test built on [`assert_scan_covers_every_tree`] that plants a violation
//! in a scratch copy of every tree it names.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// The workspace root. The root package's manifest directory is the
/// workspace root, so this is where `src`, `crates`, `docs`, and the
/// lockfile live. `tests/source_scan_roots.rs` checks that.
pub fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A directory (or one file) a scan walks, relative to the workspace root,
/// and the fewest `.rs` files the walk may find.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub dir: &'static str,
    pub min_files: usize,
}

impl Tree {
    pub const fn new(dir: &'static str, min_files: usize) -> Self {
        Self { dir, min_files }
    }

    /// Whether this tree names one file rather than a directory.
    fn is_file(&self) -> bool {
        Path::new(self.dir)
            .extension()
            .is_some_and(|ext| ext == "rs")
    }
}

/// The host's `src` (395 files today).
pub const HOST_SRC: Tree = Tree::new("src", 300);
/// The host's integration tests (38 files today).
pub const HOST_TESTS: Tree = Tree::new("tests", 30);
/// The host's examples (1 file today).
pub const HOST_EXAMPLES: Tree = Tree::new("examples", 1);
/// The framework's `src` (33 files today). Raise this as code moves in.
pub const FRAMEWORK_SRC: Tree = Tree::new("crates/tuikit/src", 25);
/// The framework's integration tests (6 files today, one of them shared
/// scan plumbing under `support/`). The independence guard alone keeps the
/// minimum of 1.
pub const FRAMEWORK_TESTS: Tree = Tree::new("crates/tuikit/tests", 1);
/// The framework's examples (none today).
pub const FRAMEWORK_EXAMPLES: Tree = Tree::new("crates/tuikit/examples", 0);

/// Every Rust directory of both crates, for rules that apply to all code.
pub const EVERY_RUST_TREE: &[Tree] = &[
    HOST_SRC,
    HOST_TESTS,
    HOST_EXAMPLES,
    FRAMEWORK_SRC,
    FRAMEWORK_TESTS,
    FRAMEWORK_EXAMPLES,
];

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            collect(&entry.path(), out);
        }
    } else if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
        out.push(path.to_path_buf());
    }
}

/// The `.rs` files under `trees`, sorted, or a description of every tree
/// that has fewer files than its minimum.
pub fn try_rust_files(root: &Path, trees: &[Tree]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut short = Vec::new();
    for tree in trees {
        let before = files.len();
        collect(&root.join(tree.dir), &mut files);
        let found = files.len() - before;
        if found < tree.min_files {
            short.push(format!(
                "  `{}`: {found} Rust files, at least {} expected",
                tree.dir, tree.min_files
            ));
        }
    }
    if !short.is_empty() {
        return Err(format!(
            "a source scan visited too few files, so its root is probably wrong \
             (scanning from {}):\n{}\nIf code moved on purpose, change the minimums in \
             tests/support/source_tree.rs.",
            root.display(),
            short.join("\n")
        ));
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// [`try_rust_files`], panicking with its description when a tripwire
/// fires.
pub fn rust_files(root: &Path, trees: &[Tree]) -> Vec<PathBuf> {
    try_rust_files(root, trees).unwrap_or_else(|message| panic!("{message}"))
}

/// Whether `path` is a unit-test file: `tests.rs` or `*_tests.rs`.
pub fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.ends_with("_tests.rs") || name == "tests.rs"
}

/// `path` relative to `root`, with forward slashes, as findings print it.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Proves that `scan` covers every tree in `trees`. It builds a scratch
/// workspace root, writes `violation` into each tree (as `violation.rs`
/// inside a directory, or as the file itself for a single-file tree), runs
/// `scan` from there with every minimum lowered to 1, and requires a
/// finding that starts with each tree's path.
///
/// A finding must therefore begin with the offending file's path relative
/// to the root, as `relative` prints it.
pub fn assert_scan_covers_every_tree(
    trees: &[Tree],
    violation: &str,
    scan: impl Fn(&Path, &[Tree]) -> Vec<String>,
) {
    let scratch = tempfile::tempdir().expect("scratch workspace root");
    for tree in trees {
        let target = if tree.is_file() {
            scratch.path().join(tree.dir)
        } else {
            scratch.path().join(tree.dir).join("violation.rs")
        };
        fs::create_dir_all(target.parent().expect("a parent directory")).expect("create tree");
        fs::write(&target, violation).expect("plant the violation");
    }
    let lowered: Vec<Tree> = trees.iter().map(|tree| Tree::new(tree.dir, 1)).collect();
    let findings = scan(scratch.path(), &lowered);
    for tree in trees {
        let prefix = if tree.is_file() {
            tree.dir.to_string()
        } else {
            format!("{}/", tree.dir)
        };
        assert!(
            findings.iter().any(|finding| finding.starts_with(&prefix)),
            "the scan did not report the violation planted under `{}`; it reported:\n{}",
            tree.dir,
            findings.join("\n")
        );
    }
}
