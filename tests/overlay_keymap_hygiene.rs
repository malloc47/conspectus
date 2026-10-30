//! Overlay keys resolve through key tables (`tuikit::keys` and the
//! host's `tui::keybindings::overlay`), and overlay footers and the
//! status bar's hints are generated from those tables (ADR 0123, ADR 0160,
//! ADR 0191). This guard fails when overlay code matches `KeyCode` by hand
//! or renders a hand-written key hint, the two ways a footer could drift
//! from what its handler accepts.
//!
//! The guard covers the host's overlay code and every file of the
//! framework (`crates/tuikit/src`), because overlay controls move there;
//! the hint scan also covers the code that writes the status line when an
//! overlay opens and the status bar itself. `tests/support/source_tree.rs`
//! explains why and how the scans are guarded against visiting too few
//! files.

use std::fs;
use std::path::{Path, PathBuf};

#[path = "support/source_tree.rs"]
mod source_tree;

use source_tree::{FRAMEWORK_SRC, Tree};
use tuikit::testkit::source::{production_code, string_literals};

/// The trees scanned: the host's overlay code, then the whole framework.
/// A single-file tree must exist, so moving one of those files fails the
/// scan until this list follows it.
const TREES: &[Tree] = &[
    Tree::new("src/tui/widgets", 10),
    Tree::new("src/tui/runtime/overlay_keys.rs", 1),
    Tree::new("src/tui/modal.rs", 1),
    FRAMEWORK_SRC,
];

/// The code that writes the status line: the runtime's handlers, the
/// App's reducer and its parts, and the executor's, which set its text
/// when a key opens an overlay or is refused, the actions that word a
/// refusal or a row's state (`actions.rs`), and the status bar, which
/// draws the hints. They route keys and draw the main view, so they may
/// name key codes, but a key hint they wrote by hand would sit on the
/// status line beside, or instead of, the generated one, and a key a
/// message names by hand can drift from the keymap (they name it with
/// `keybindings::key_name`). The status scan holds them to the stricter
/// [`looks_like_status_hint`].
const STATUS_TREES: &[Tree] = &[
    Tree::new("src/tui/runtime.rs", 1),
    Tree::new("src/tui/runtime/executor.rs", 1),
    Tree::new("src/tui/app.rs", 1),
    Tree::new("src/tui/app", 10),
    Tree::new("src/tui/actions.rs", 1),
    Tree::new("src/tui/ui/header.rs", 1),
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
    (
        "crates/tuikit/src/explorer/keys.rs",
        "the explorer's key table itself: the rows name the keys they bind",
    ),
    (
        "crates/tuikit/src/testkit/keys.rs",
        "the test kit's key-script notation: turns key names into key events",
    ),
    (
        "crates/tuikit/src/testkit/conformance/checks.rs",
        "the conformance suite presses the arrows, Esc, and q to check every modal",
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

fn looks_like_key_hint(literal: &str) -> bool {
    literal.contains(" · ")
        && ["Esc", "Enter", "Tab"].iter().any(|key| {
            literal
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word == *key)
        })
}

/// What the status scan flags: a footer-shaped literal, one whose
/// ` · `-separated pieces include a hint segment ([`is_hint_segment`]),
/// or a message that names a key in its prose ([`names_a_key_in_prose`]).
/// Overlay code explains keys in sentences ("Tab completes the path"),
/// which this would flag, so only the status-line code is held to it.
fn looks_like_status_hint(literal: &str) -> bool {
    looks_like_key_hint(literal)
        || literal.split(" · ").any(is_hint_segment)
        || names_a_key_in_prose(literal)
}

/// Whether a message names a key the way a refusal tells the operator
/// what to press: a piece that leads with keys and a colon
/// (`Backspace: no reveal…`, `i: select…`), a single character in
/// backticks (``try `r` to refresh``), or "press" before a named key
/// (`press Backspace again`). Such a message names the key from its
/// binding (`keybindings::key_name`) instead, through a placeholder.
fn names_a_key_in_prose(literal: &str) -> bool {
    let leads_with_keys = literal.split(" · ").any(|piece| {
        piece
            .split_once(": ")
            .is_some_and(|(keys, _)| is_keys(keys))
    });
    let backticked_character = literal.split('`').enumerate().any(|(index, part)| {
        index % 2 == 1 && part.chars().count() == 1 && !part.starts_with(['{', '}'])
    });
    let words: Vec<&str> = literal.split(' ').collect();
    let pressed = words
        .windows(2)
        .any(|pair| pair[0].eq_ignore_ascii_case("press") && NAMED_KEYS.contains(&pair[1]));
    leads_with_keys || backticked_character || pressed
}

/// Keys that are not a single character, as hints write them.
const NAMED_KEYS: &[&str] = &[
    "Esc",
    "Enter",
    "Tab",
    "S-Tab",
    "Backspace",
    "Space",
    "PgUp",
    "PgDn",
    "Home",
    "End",
    "↑",
    "↓",
    "←",
    "→",
];

/// Whether `token` is keys as a hint writes them, joined with `/`: named
/// keys, chords (`Ctrl-R`, `Alt-b`, `^R`), and single characters other
/// than digits (`1 hit` is a count).
fn is_keys(token: &str) -> bool {
    !token.is_empty()
        && token.split('/').all(|key| {
            NAMED_KEYS.contains(&key)
                || key.starts_with("Ctrl-")
                || key.starts_with("Alt-")
                || (key.starts_with('^') && key.chars().count() == 2)
                || (key.chars().count() == 1 && !key.chars().all(|c| c.is_ascii_digit()))
        })
}

/// Whether `piece` is one hint segment written by hand: keys and a verb
/// (`Tab focus`, `Enter/a attach {label}`, `^R forges`). A piece that
/// leads with a single character needs to be exactly a key and a word
/// (`q quit`), so a sentence that starts with "a" is not one.
fn is_hint_segment(piece: &str) -> bool {
    let words: Vec<&str> = piece.split(' ').collect();
    let [keys, verb, ..] = words.as_slice() else {
        return false;
    };
    let named = keys.split('/').any(|key| key.chars().count() > 1);
    is_keys(keys)
        && verb.chars().next().is_some_and(char::is_lowercase)
        && (named || words.len() == 2)
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
        let lines: Vec<&str> = text.lines().collect();
        // Comments, strings, and test items may name keys freely.
        for (number, code) in production_code(&text).lines().enumerate() {
            if code.contains("KeyCode::") {
                offenders.push(format!(
                    "{relative}:{}: {}",
                    number + 1,
                    lines[number].trim()
                ));
            }
        }
    }
    offenders
}

/// Hand-written key hints in overlay code under `trees`, as
/// `path: "literal"` findings.
fn hand_written_hints(root: &Path, trees: &[Tree]) -> Vec<String> {
    hints_by(root, trees, looks_like_key_hint)
}

/// Hand-written key hints in the status-line code under `trees`.
fn hand_written_status_hints(root: &Path, trees: &[Tree]) -> Vec<String> {
    hints_by(root, trees, looks_like_status_hint)
}

/// The string literals under `trees` that `looks` takes for a key hint.
fn hints_by(root: &Path, trees: &[Tree], looks: fn(&str) -> bool) -> Vec<String> {
    let mut offenders = Vec::new();
    for path in overlay_sources(root, trees) {
        let text = fs::read_to_string(&path).expect("read overlay source");
        for literal in string_literals(&text) {
            let literal = literal.text;
            if looks(&literal) && !ALLOWED_HINT_TEXT.contains(&literal.as_str()) {
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
            production_code(&text).contains("KeyCode::"),
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
fn the_status_line_names_no_key_hint_written_by_hand() {
    let offenders = hand_written_status_hints(source_tree::workspace_root(), STATUS_TREES);
    assert!(
        offenders.is_empty(),
        "status-line code writes a key hint by hand; the status bar generates hints from the \
         key tables (`tuikit::status_bar::KeyHintBar`), and an overlay names its own keys \
         (`Overlay::hints`):\n{}",
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

#[test]
fn a_hand_written_status_hint_is_caught_in_every_file_the_status_scan_covers() {
    let violation = "fn f() { let _ = \"Tab focus\"; }\n";
    source_tree::assert_scan_covers_every_tree(STATUS_TREES, violation, hand_written_status_hints);
}

/// The status line's hand-written hints this scan once missed, each of
/// the shapes it now catches, and text it must leave alone.
#[test]
fn a_hint_segment_written_by_hand_is_caught_and_prose_is_not() {
    for hint in [
        "worktree: ↑/↓ move · Enter pick · Esc close",
        "Tab focus",
        "q quit",
        "r refresh · ^R forges",
        "Enter/a attach {label}",
        "Enter/v view {} · S resume",
        "Backspace back",
        "Ctrl-R hard refresh",
        "←/→ fold",
        // Keys named in a message's prose, as refusals named them.
        "i: select an agent or mux session row to copy its id",
        "Backspace: no reveal to return from",
        "mux node missing from snapshot — try `r` to refresh",
        "explorer: no drill history — press Backspace again to return to the left pane",
    ] {
        assert!(looks_like_status_hint(hint), "{hint:?} is a key hint");
    }
    for prose in [
        "a status message long enough to wrap",
        "attach",
        "worktree: nothing selected",
        "{}: select an agent or mux session row to copy its id",
        "attach: 2 ambiguous muxes here — select a session to see its candidates",
        "pin `{pin_id}` is {} · {enter} runs `{rebuild}` to bring it back",
        "select the `Other` header",
        "tmux:off",
        "(pin, forget, plan, clean up)",
        "at the match",
        "1 hit",
        "",
    ] {
        assert!(
            !looks_like_status_hint(prose),
            "{prose:?} is not a key hint"
        );
    }
}
