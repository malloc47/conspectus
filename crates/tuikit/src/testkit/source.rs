//! Source-scan helpers: what a hygiene test needs to read Rust source the
//! way the compiler would, without a parser.
//!
//! A scan that looks for a forbidden call should not trip on the word in a
//! comment or a string, and one that reads the text a program shows should
//! read its string literals and nothing else; both usually exempt test
//! code. These helpers go through the crate's one small lexer (the glyph
//! scanner uses it too), which understands line and nested block comments,
//! string literals with escapes, raw and byte strings, character literals
//! versus lifetimes, and `#[cfg(test)]` items. Every helper keeps line
//! breaks where they were, so a finding's line number is the file's. It
//! assumes `rustfmt`-formatted source: it is a tripwire for review, not a
//! defense against code written to evade it.
//!
//! - [`code`]: comments and literals blanked; what to search for calls,
//!   paths, and identifiers.
//! - [`without_comments`]: comments blanked, literals kept.
//! - [`strip_test_items`]: every `#[cfg(test)]` item blanked.
//! - [`production_code`]: [`code`] with the test items blanked too.
//! - [`string_literals`]: the text of each string literal outside comments
//!   and test items, escapes resolved, with its line.
//! - [`contains_at_word_start`] and [`is_identifier_char`]: matching a
//!   needle where a word starts, so `Command::new` does not match
//!   `MyCommand::new`.
//! - [`files`] and [`rust_files`]: a sorted, read-only walk of a tree.
//!
//! The rules a host enforces with them (which words, which trees, which
//! exceptions) are its own; only the mechanism is here.
//!
//! ```
//! use tuikit::testkit::source;
//!
//! let text = "let a = \"Esc close\"; // KeyCode::Esc\n#[cfg(test)]\nmod tests { fn t() { let _ = \"x\"; } }\n";
//! assert!(!source::code(text).contains("KeyCode"));
//! let literals = source::string_literals(text);
//! assert_eq!(literals.len(), 1);
//! assert_eq!((literals[0].line, literals[0].text.as_str()), (1, "Esc close"));
//! assert!(source::contains_at_word_start("x = Command::new()", "Command::new"));
//! assert!(!source::contains_at_word_start("x = MyCommand::new()", "Command::new"));
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use crate::lex::{self, Kind, Lexed};

/// `source` with comments and string and character literals blanked, line
/// breaks kept.
pub fn code(source: &str) -> String {
    Lexed::new(source).skeleton().into_iter().collect()
}

/// `source` with comments blanked and literals kept, line breaks kept.
pub fn without_comments(source: &str) -> String {
    Lexed::new(source)
        .blanked(|kind| kind == Kind::Comment)
        .into_iter()
        .collect()
}

/// `source` with every item behind a `#[cfg(test)]` attribute blanked
/// (attribute included), line breaks kept. Comments and literals stay,
/// but braces inside them do not confuse where an item ends.
pub fn strip_test_items(source: &str) -> String {
    let lexed = Lexed::new(source);
    let mut out = lexed.chars.clone();
    for item in lexed.test_items() {
        lex::blank_range(&mut out, item);
    }
    out.into_iter().collect()
}

/// `source` with comments, literals, and `#[cfg(test)]` items blanked:
/// the code a production build compiles.
pub fn production_code(source: &str) -> String {
    let lexed = Lexed::new(source);
    let mut out = lexed.skeleton();
    for item in lexed.test_items() {
        lex::blank_range(&mut out, item);
    }
    out.into_iter().collect()
}

/// One string literal of a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Literal {
    /// The 1-based line the literal starts on.
    pub line: usize,
    /// Its text, escapes resolved (a raw string's text as written).
    pub text: String,
}

/// Every string literal of `source` outside comments and `#[cfg(test)]`
/// items, in order: plain, byte, and raw strings. Character literals are
/// not strings.
pub fn string_literals(source: &str) -> Vec<Literal> {
    let lexed = Lexed::new(source);
    let tests = lexed.test_items();
    let chars = &lexed.chars;
    let mut out = Vec::new();
    let mut line = 1;
    let mut counted = 0;
    for token in &lexed.tokens {
        if !matches!(token.kind, Kind::Text | Kind::RawText)
            || tests.iter().any(|item| item.contains(&token.start))
        {
            continue;
        }
        line += chars[counted..token.start]
            .iter()
            .filter(|c| **c == '\n')
            .count();
        counted = token.start;
        let body = if token.kind == Kind::RawText {
            let hashes = lex::raw_hashes(chars, token.start).unwrap_or(0);
            let start = token.start + hashes + 2;
            let end = token.end.saturating_sub(hashes + 1).max(start);
            chars[start..end].iter().collect()
        } else {
            let end = token.end.saturating_sub(1).max(token.start + 1);
            unescape(&chars[token.start + 1..end])
        };
        out.push(Literal { line, text: body });
    }
    out
}

/// The text a string literal's body (between its quotes) stands for.
fn unescape(body: &[char]) -> String {
    let mut out = String::new();
    let mut chars = body.iter().copied().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('x') => {
                let digits: String = chars.by_ref().take(2).collect();
                if let Some(c) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(c);
                }
            }
            Some('u') => {
                let digits: String = chars
                    .by_ref()
                    .skip_while(|c| *c == '{')
                    .take_while(|c| *c != '}')
                    .collect();
                if let Some(c) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(c);
                }
            }
            // A line continuation: the break and the next line's indent go.
            Some('\n') => while chars.next_if(|c| c.is_whitespace()).is_some() {},
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Whether `c` can be part of an identifier.
pub fn is_identifier_char(c: char) -> bool {
    lex::is_identifier_char(c)
}

/// Whether `needle` occurs in `line` starting where a word starts: at the
/// line's start or after a character that cannot be part of an identifier.
pub fn contains_at_word_start(line: &str, needle: &str) -> bool {
    line.match_indices(needle).any(|(index, _)| {
        line[..index]
            .chars()
            .next_back()
            .is_none_or(|c| !is_identifier_char(c))
    })
}

/// Every file under `root` (or `root` itself, when it is a file), sorted.
/// Directories named `target` and hidden directories below `root` are
/// skipped, and so is anything that cannot be read. Nothing is written.
pub fn files(root: &Path) -> Vec<PathBuf> {
    fn walk(path: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name != "target" && !name.starts_with('.') {
                    walk(&path, out);
                }
            } else if kind.is_file() {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
    } else {
        walk(root, &mut out);
    }
    out.sort();
    out
}

/// The `.rs` files [`files`] finds under `root`.
pub fn rust_files(root: &Path) -> Vec<PathBuf> {
    files(root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect()
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
