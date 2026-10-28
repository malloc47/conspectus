//! Overlay keys resolve through key tables (`tuikit::keys` and the
//! host's `tui::keybindings::overlay`), and overlay footers are generated
//! from those tables (ADR 0123, ADR 0160). This
//! guard fails when overlay code matches `KeyCode` by hand or renders a
//! hand-written key hint, the two ways a footer could drift from what
//! its handler accepts.
//!
//! The guard covers the host's overlay code and every file of the
//! framework (`crates/tuikit/src`), because overlay controls move there;
//! `tests/support/source_tree.rs` explains why and how the scans are
//! guarded against visiting too few files.

use std::fs;
use std::path::{Path, PathBuf};

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, Tree};

/// The trees scanned: the host's overlay code, then the whole framework.
/// A single-file tree must exist, so moving one of those files fails the
/// scan until this list follows it.
const TREES: &[Tree] = &[
    Tree::new("src/tui/widgets", 10),
    Tree::new("src/tui/runtime/overlay_keys.rs", 1),
    Tree::new("src/tui/modal.rs", 1),
    FRAMEWORK_SRC,
];

/// Framework files that may name `KeyCode` outside a test module because
/// they define the key-table model itself, each with its reason. A listed
/// file is exempt from the hand-matched `KeyCode` check only; its string
/// literals are still checked. Paths are relative to the workspace root.
/// Add an entry only for a file whose job is to turn raw key events into
/// table lookups; overlay and control code resolves through the tables and
/// stays covered.
const KEY_MODEL_FILES: &[(&str, &str)] = &[
    (
        "crates/tuikit/src/keys/matcher.rs",
        "the key matcher: matches key events, writes key labels, and builds matchers",
    ),
    (
        "crates/tuikit/src/keys/table.rs",
        "resolves key events against tables and tells a typed character from a key",
    ),
    (
        "crates/tuikit/src/keys/role.rs",
        "presses Esc, Enter, and Space to check a modal's tables against the convention",
    ),
    (
        "crates/tuikit/src/keys/standard.rs",
        "the standard key tables themselves: the rows name the keys they bind",
    ),
];

/// Help-overlay prose that reads like a key hint but describes the
/// main view, not the overlay it appears in.
const ALLOWED_HINT_TEXT: &[&str] = &["session: one mux, Enter/a attaches · mux: client attached"];

/// Overlay and control sources under `trees`, unit-test files excluded.
fn overlay_sources(root: &Path, trees: &[Tree]) -> Vec<PathBuf> {
    source_tree::rust_files(root, trees)
        .into_iter()
        .filter(|path| !source_tree::is_test_file(path))
        .collect()
}

/// The file's text up to its test module, which may match keys freely.
fn production_text(text: &str) -> &str {
    let lines: Vec<&str> = text.lines().collect();
    let mut offset = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.trim() == "#[cfg(test)]"
            && lines[index + 1..]
                .iter()
                .find(|next| !next.trim().starts_with("#["))
                .is_some_and(|next| next.trim_start().starts_with("mod tests"))
        {
            return &text[..offset];
        }
        offset += line.len() + 1;
    }
    text
}

fn string_literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut literal = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        literal.push(escaped);
                    }
                }
                '"' => break,
                other => literal.push(other),
            }
        }
        out.push(literal);
    }
    out
}

fn looks_like_key_hint(literal: &str) -> bool {
    literal.contains(" · ")
        && ["Esc", "Enter", "Tab"].iter().any(|key| {
            literal
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word == *key)
        })
}

/// Lines of overlay code under `trees` that match a `KeyCode` by hand, as
/// `path:line: text` findings.
fn hand_matched_keys(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut offenders = Vec::new();
    for path in overlay_sources(root, trees) {
        let relative = source_tree::relative(root, &path);
        if KEY_MODEL_FILES.iter().any(|(file, _)| *file == relative) {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read overlay source");
        for (number, line) in production_text(&text).lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("KeyCode::") {
                offenders.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    offenders
}

/// Hand-written key hints in overlay code under `trees`, as
/// `path: "literal"` findings.
fn hand_written_hints(root: &Path, trees: &[Tree]) -> Vec<String> {
    let mut offenders = Vec::new();
    for path in overlay_sources(root, trees) {
        let text = fs::read_to_string(&path).expect("read overlay source");
        for literal in string_literals(production_text(&text)) {
            if looks_like_key_hint(&literal) && !ALLOWED_HINT_TEXT.contains(&literal.as_str()) {
                offenders.push(format!(
                    "{}: {literal:?}",
                    source_tree::relative(root, &path)
                ));
            }
        }
    }
    offenders
}

#[test]
fn overlay_code_resolves_keys_through_keymap_tables() {
    let offenders = hand_matched_keys(source_tree::workspace_root(), TREES);
    assert!(
        offenders.is_empty(),
        "overlay code matches keys by hand; add rows to a key table (`tuikit::keys` or \
         `tui::keybindings::overlay`) instead (a framework file that defines the key-table \
         model itself belongs in `KEY_MODEL_FILES`):\n{}",
        offenders.join("\n")
    );
}

/// A key-model entry that no longer names `KeyCode` (or whose file is
/// gone) would exempt nothing, so it fails until it is removed.
#[test]
fn every_key_model_file_still_names_key_codes() {
    let root = source_tree::workspace_root();
    for (file, reason) in KEY_MODEL_FILES {
        let text = fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| panic!("{file} ({reason}): {error}"));
        assert!(
            production_text(&text).lines().any(|line| line
                .split("//")
                .next()
                .unwrap_or("")
                .contains("KeyCode::")),
            "{file} no longer names `KeyCode`; remove it from KEY_MODEL_FILES"
        );
    }
}

#[test]
fn overlay_footers_are_generated_from_keymap_tables() {
    let offenders = hand_written_hints(source_tree::workspace_root(), TREES);
    assert!(
        offenders.is_empty(),
        "overlay code renders a hand-written key hint; generate it with `tuikit::keys::footer` instead:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn a_hand_matched_key_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn f() { let _ = KeyCode::Esc; }\n";
    source_tree::assert_scan_covers_every_tree(TREES, violation, hand_matched_keys);
}

#[test]
fn a_hand_written_hint_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn f() { let _ = \"Esc · Enter close\"; }\n";
    source_tree::assert_scan_covers_every_tree(TREES, violation, hand_written_hints);
}
