//! Only `src/tui/runtime/term.rs` takes the terminal over and hands it
//! back. Its helpers turn bracketed paste on with raw mode and the
//! alternate screen and off again before the shell or a subprocess (an
//! attach, the external viewer) gets the terminal, so a paste is one
//! event while the TUI runs and plain text everywhere else. A direct
//! `ratatui::init` or `ratatui::restore` elsewhere would skip that.
//!
//! The rule covers the host (`src`) and the framework (`crates/tuikit/src`).
//! The framework takes no terminal: its host hands it a frame to draw.
//! `tests/support/source_tree.rs` explains which hygiene rules apply to which
//! crate and how the scans are guarded against visiting too few files.

use std::fs;
use std::path::Path;

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, HOST_SRC, Tree};

/// The trees whose code must not take the terminal directly.
const TREES: &[Tree] = &[HOST_SRC, FRAMEWORK_SRC];

/// The only file that may, relative to the workspace root.
const ALLOWED: &[&str] = &["src/tui/runtime/term.rs"];

/// The `ratatui` calls that take the terminal over or hand it back.
const CALLS: &[&str] = &[
    "ratatui::init(",
    "ratatui::try_init(",
    "ratatui::init_with_options(",
    "ratatui::try_init_with_options(",
    "ratatui::restore(",
    "ratatui::try_restore(",
    "ratatui::run(",
];

/// Whether `line`'s code (not a comment) takes or returns the terminal.
fn takes_the_terminal(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or(line);
    CALLS.iter().any(|call| code.contains(call))
}

/// Every such call under `trees` outside the exempt file, as
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
            if takes_the_terminal(line) {
                problems.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    problems
}

#[test]
fn only_the_term_helpers_take_the_terminal() {
    let problems = problems(source_tree::workspace_root(), TREES);
    assert!(
        problems.is_empty(),
        "take the terminal with `term::enter` and hand it back with \
         `term::leave`, which keep bracketed paste on only while the TUI \
         has it:\n{}",
        problems.join("\n")
    );
}

#[test]
fn matcher_tells_calls_from_comments() {
    assert!(takes_the_terminal(
        "    let mut terminal = ratatui::init();"
    ));
    assert!(takes_the_terminal("    ratatui::restore();"));
    assert!(takes_the_terminal("let t = ratatui::try_init()?;"));
    assert!(!takes_the_terminal(
        "/// Plain `ratatui::restore()` is enough"
    ));
    assert!(!takes_the_terminal(
        "    // ratatui::init() installs a hook"
    ));
    assert!(!takes_the_terminal("    let mut terminal = term::enter();"));
}

#[test]
fn a_direct_terminal_take_over_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn take() {\n    let _ = ratatui::init();\n}\n";
    source_tree::assert_scan_covers_every_tree(TREES, violation, problems);
}
