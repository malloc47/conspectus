//! Command-line splitting and quoting shared by the forms, previews,
//! history, the pin commands, and the rebuild plan.
//!
//! # How a command line splits
//!
//! Every argv Conspectus reads from text goes through one grammar: a POSIX
//! shell's word splitting with nothing expanded.
//!
//! - Unquoted whitespace separates arguments. A run of it counts once, and
//!   whitespace at either end makes no argument.
//! - Single quotes keep everything between them as written, backslashes
//!   included: `'a\b c'` is `a\b c`.
//! - Double quotes keep everything between them, except that a backslash
//!   before `"`, `\`, `$`, or a backtick stands for that character. Before
//!   anything else the backslash stays: `"say \"hi\""` is `say "hi"`, and
//!   `"a\b"` is `a\b`.
//! - Outside quotes a backslash makes the next character plain: `x\ y` is
//!   `x y`, `\'` is `'`, and `\\` is `\`.
//! - Pieces with nothing between them are one argument (`--name="two
//!   words"` is `--name=two words`), and `''` or `""` alone is an empty
//!   argument.
//! - Nothing is expanded or run: `$HOME`, backticks, `~`, globs, `;`, `|`,
//!   and the other shell operators are ordinary characters.
//!
//! Text can end unfinished, inside a quote or on a backslash with nothing
//! after it. [`parse_command_line`] refuses such text and says why; it
//! reads an argv someone typed (the launch and pin edit forms, `pin edit
//! --set argv=…`), so the form or the command can say what to fix.
//! [`split_command_line`] reads it as far as it goes, the open quote
//! running to the end and a last lone backslash standing for itself; it
//! reads text Conspectus did not type, such as a tmux pane's start
//! command. Finished text splits the same way through either.
//!
//! [`shell_quote`] writes one argument so that it splits back to itself.

use std::ffi::{OsStr, OsString};
use std::fmt;

/// Quote one argument for a POSIX shell so it round-trips through
/// [`split_command_line`] and through `sh` itself. Words made only of
/// characters a shell leaves alone stay bare; the lone `;` that tmux
/// uses to chain commands becomes `\;`; everything else is wrapped in
/// single quotes.
pub fn shell_quote(token: &OsStr) -> String {
    let token = token.to_string_lossy();
    if token == ";" {
        return "\\;".to_string();
    }
    let plain = !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if plain {
        return token.into_owned();
    }
    format!("'{}'", token.replace('\'', "'\\''"))
}

/// Quote and join an argv into one pasteable command line.
pub fn shell_join(argv: &[OsString]) -> String {
    argv.iter()
        .map(|token| shell_quote(token))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Why a command line cannot be split as it stands: it ends unfinished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfinished {
    /// A quote (`'` or `"`) opened and never closed.
    Quote(char),
    /// A backslash with nothing after it.
    Escape,
}

impl fmt::Display for Unfinished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quote(quote) => write!(f, "an unclosed `{quote}` quote"),
            Self::Escape => f.write_str("a trailing escape"),
        }
    }
}

impl std::error::Error for Unfinished {}

/// Split a command line someone typed into arguments, as the module docs
/// describe, or say why it is unfinished: an unclosed quote or a last
/// backslash with nothing after it.
pub fn parse_command_line(line: &str) -> Result<Vec<String>, Unfinished> {
    match tokenize(line) {
        (args, None) => Ok(args),
        (_, Some(unfinished)) => Err(unfinished),
    }
}

/// Split a command line into arguments, as the module docs describe,
/// reading unfinished text as far as it goes: an open quote runs to the
/// end, and a last lone backslash stands for itself.
pub fn split_command_line(line: &str) -> Vec<String> {
    tokenize(line).0
}

/// The arguments `line` splits into, and how it is unfinished if it is.
fn tokenize(line: &str) -> (Vec<String>, Option<Unfinished>) {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut unfinished = None;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                in_word = true;
                let mut closed = false;
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        closed = true;
                        break;
                    }
                    current.push(quoted);
                }
                if !closed {
                    unfinished = Some(Unfinished::Quote('\''));
                }
            }
            '"' => {
                in_word = true;
                let mut closed = false;
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => match chars.next() {
                            Some(escaped @ ('"' | '\\' | '$' | '`')) => current.push(escaped),
                            Some(other) => {
                                current.push('\\');
                                current.push(other);
                            }
                            None => current.push('\\'),
                        },
                        other => current.push(other),
                    }
                }
                if !closed {
                    unfinished = Some(Unfinished::Quote('"'));
                }
            }
            '\\' => {
                in_word = true;
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                } else {
                    current.push('\\');
                    unfinished = Some(Unfinished::Escape);
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    args.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            other => {
                in_word = true;
                current.push(other);
            }
        }
    }
    if in_word {
        args.push(current);
    }
    (args, unfinished)
}

#[cfg(test)]
#[path = "argv_tests.rs"]
mod tests;
