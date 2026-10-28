//! The source scanner: which non-ASCII characters a piece of Rust source
//! draws.
//!
//! A glyph the terminal's font lacks is drawn from a fallback font and can
//! spill over its neighbor, so every character a program draws has to be
//! vetted (see [`super::VettedGlyphs`]). The scanner is the half of that
//! check that reads source: it reports each non-ASCII character in code,
//! which in Rust means string and character literals, and ignores the
//! places a character is prose, comments of every kind and `#[cfg(test)]`
//! items.
//!
//! It is a pure function over text, so a host walks its own trees (and
//! decides which files are test code) and hands each file's text here. The
//! lexing is deliberately small: it understands line and nested block
//! comments, string literals with escapes, raw and byte strings, and
//! character literals versus lifetimes. It assumes `rustfmt`-formatted
//! source, and it is a tripwire for a forgotten glyph, not a parser.

/// One non-ASCII character found in code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// The 1-based line it is on.
    pub line: usize,
    /// The character.
    pub glyph: char,
}

/// The attribute that gates an item to test builds.
const TEST_GATE: &str = "#[cfg(test)]";

/// Every non-ASCII character `source` draws, in order, with its line.
///
/// Characters in comments (line, doc, and block) are skipped, and so is
/// every item behind a `#[cfg(test)]` attribute. Characters in string and
/// character literals count.
pub fn drawn_glyphs(source: &str) -> Vec<Drawn> {
    let chars: Vec<char> = source.chars().collect();
    let (mut visible, skeleton) = lex(&chars);
    blank_test_items(&skeleton, &mut visible);
    let mut out = Vec::new();
    let mut line = 1;
    for c in visible {
        match c {
            '\n' => line += 1,
            c if !c.is_ascii() => out.push(Drawn { line, glyph: c }),
            _ => {}
        }
    }
    out
}

fn blank(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Splits `chars` into two same-length views. The first has comments
/// blanked and literals kept. The second also has literals blanked, so
/// braces and brackets inside text cannot confuse item matching.
fn lex(chars: &[char]) -> (Vec<char>, Vec<char>) {
    let mut visible = Vec::with_capacity(chars.len());
    let mut skeleton = Vec::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        let Some((end, comment)) = token_end(chars, index) else {
            visible.push(chars[index]);
            skeleton.push(chars[index]);
            index += 1;
            continue;
        };
        for &c in &chars[index..end] {
            visible.push(if comment { blank(c) } else { c });
            skeleton.push(blank(c));
        }
        index = end;
    }
    (visible, skeleton)
}

/// The end of the comment or literal starting at `index`, and whether it is
/// a comment, or `None` when plain code starts there.
fn token_end(chars: &[char], index: usize) -> Option<(usize, bool)> {
    let second = chars.get(index + 1).copied();
    match chars[index] {
        '/' if second == Some('/') => {
            let end = chars[index..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(chars.len(), |offset| index + offset);
            Some((end, true))
        }
        '/' if second == Some('*') => Some((block_comment_end(chars, index), true)),
        '"' => Some((string_end(chars, index + 1), false)),
        'r' => raw_string_end(chars, index).map(|end| (end, false)),
        '\'' => char_literal_end(chars, index).map(|end| (end, false)),
        _ => None,
    }
}

/// Index just past the string literal whose body starts at `from`.
fn string_end(chars: &[char], from: usize) -> usize {
    let mut index = from;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '"' => return index + 1,
            _ => index += 1,
        }
    }
    chars.len()
}

/// Index just past the (possibly nested) block comment starting at `start`.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        if chars[index..].starts_with(&['/', '*']) {
            depth += 1;
            index += 2;
        } else if chars[index..].starts_with(&['*', '/']) {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    chars.len()
}

/// Index just past a raw string literal starting at the `r` at `start`, or
/// `None` when the `r` begins something else (an identifier, `r#ident`).
fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    let after_prefix = start == 0
        || !is_identifier_char(chars[start - 1])
        || (chars[start - 1] == 'b' && (start == 1 || !is_identifier_char(chars[start - 2])));
    if !after_prefix {
        return None;
    }
    let hashes = chars[start + 1..].iter().take_while(|c| **c == '#').count();
    if chars.get(start + 1 + hashes) != Some(&'"') {
        return None;
    }
    let closing: Vec<char> = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    let end = (start + 2 + hashes..chars.len())
        .find(|&index| chars[index..].starts_with(&closing))
        .map_or(chars.len(), |index| index + closing.len());
    Some(end)
}

/// Index just past the character literal starting at `start`, or `None`
/// for a lifetime.
fn char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start + 1) == Some(&'\\') {
        let tail = chars.get(start + 3..)?;
        let close = tail.iter().position(|c| *c == '\'')?;
        Some(start + 3 + close + 1)
    } else if chars.get(start + 2) == Some(&'\'') {
        Some(start + 3)
    } else {
        None
    }
}

/// Index just past the item that begins at `from`: through its closing
/// brace, or through the `;` of a body-less item such as `mod tests;`.
fn item_end(skeleton: &[char], from: usize) -> usize {
    let mut nesting = 0i32;
    let mut index = from;
    while index < skeleton.len() {
        match skeleton[index] {
            '(' | '[' => nesting += 1,
            ')' | ']' => nesting -= 1,
            ';' if nesting == 0 => return index + 1,
            '{' if nesting == 0 => return block_end(skeleton, index),
            _ => {}
        }
        index += 1;
    }
    skeleton.len()
}

/// Index just past the brace block that opens at `open`.
fn block_end(skeleton: &[char], open: usize) -> usize {
    let mut depth = 0i32;
    for (offset, c) in skeleton[open..].iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return open + offset + 1;
        }
    }
    skeleton.len()
}

/// Blanks every `#[cfg(test)]` item in `visible`, found through the
/// literal-free `skeleton`.
fn blank_test_items(skeleton: &[char], visible: &mut [char]) {
    let gate: Vec<char> = TEST_GATE.chars().collect();
    let mut index = 0;
    while index < skeleton.len() {
        if skeleton[index..].starts_with(&gate) {
            let end = item_end(skeleton, index + gate.len());
            for c in &mut visible[index..end] {
                *c = blank(*c);
            }
            index = end;
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
