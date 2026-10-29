//! Terminal escape sequences in text that is shown or compared as plain
//! text.

/// `text` without its ANSI escape sequences: control sequences (`ESC [`
/// through the final byte, which colors and cursor moves use), operating
/// system commands (`ESC ]` through BEL or `ESC \`, which titles and
/// clipboard writes use), and two-character escapes. Everything else,
/// line breaks included, is kept.
///
/// Captured terminal output carries these sequences, and a styled frame
/// written as ANSI text carries one at every style change, so a reader
/// that shows such text plainly, or a check that compares it, strips them
/// first.
///
/// ```
/// use tuikit::text::strip_ansi;
///
/// assert_eq!(strip_ansi("\x1b[0m\x1b[1;31mred\x1b[0m plain"), "red plain");
/// assert_eq!(strip_ansi("\x1b]0;title\x07after"), "after");
/// ```
pub fn strip_ansi(text: &str) -> String {
    const ESC: char = '\u{1b}';
    const BEL: char = '\u{7}';
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != ESC {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == BEL {
                        break;
                    }
                    if c == ESC && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn plain_text_and_line_breaks_pass_through() {
        assert_eq!(strip_ansi("one\ntwo  three\n"), "one\ntwo  three\n");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn control_sequences_go_through_their_final_byte() {
        assert_eq!(
            strip_ansi("\x1b[0m\x1b[38;2;1;2;3m\x1b[48;5;8mx\x1b[0m"),
            "x"
        );
        assert_eq!(strip_ansi("a\x1b[2Kb\x1b[10;4Hc"), "abc");
    }

    #[test]
    fn operating_system_commands_end_at_bel_or_string_terminator() {
        assert_eq!(strip_ansi("\x1b]52;c;aGk=\x07copied"), "copied");
        assert_eq!(strip_ansi("\x1b]0;title\x1b\\after"), "after");
        assert_eq!(strip_ansi("\x1b]0;never closed"), "");
    }

    #[test]
    fn a_two_character_escape_drops_both() {
        assert_eq!(strip_ansi("a\x1b7b\x1b8c"), "abc");
        assert_eq!(strip_ansi("end\x1b"), "end");
    }
}
