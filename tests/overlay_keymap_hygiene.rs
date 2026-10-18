//! Overlay keys resolve through `tui::keybindings::overlay` tables, and
//! overlay footers are generated from those tables (ADR 0123). This
//! guard fails when overlay code matches `KeyCode` by hand or renders a
//! hand-written key hint, the two ways a footer could drift from what
//! its handler accepts.

use std::fs;
use std::path::{Path, PathBuf};

/// Help-overlay prose that reads like a key hint but describes the
/// main view, not the overlay it appears in.
const ALLOWED_HINT_TEXT: &[&str] = &["session: one mux, Enter/a attaches · mux: client attached"];

fn overlay_sources(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("src/tui/widgets"), &mut files);
    files.push(root.join("src/tui/runtime/overlay_keys.rs"));
    files.push(root.join("src/tui/modal.rs"));
    files.retain(|path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        !name.ends_with("_tests.rs") && name != "tests.rs"
    });
    files.sort();
    files
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

#[test]
fn overlay_code_resolves_keys_through_keymap_tables() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    for path in overlay_sources(root) {
        let text = fs::read_to_string(&path).expect("read overlay source");
        for (number, line) in production_text(&text).lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("KeyCode::") {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "overlay code matches keys by hand; add rows to a `tui::keybindings::overlay` table instead:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn overlay_footers_are_generated_from_keymap_tables() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    for path in overlay_sources(root) {
        let text = fs::read_to_string(&path).expect("read overlay source");
        for literal in string_literals(production_text(&text)) {
            if looks_like_key_hint(&literal) && !ALLOWED_HINT_TEXT.contains(&literal.as_str()) {
                offenders.push(format!(
                    "{}: {literal:?}",
                    path.strip_prefix(root).unwrap_or(&path).display()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "overlay code renders a hand-written key hint; generate it with `overlay::footer` instead:\n{}",
        offenders.join("\n")
    );
}
