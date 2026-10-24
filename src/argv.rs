//! Command-line splitting and quoting shared by previews, history, and
//! the rebuild plan.

use std::ffi::{OsStr, OsString};

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

/// Split a command line into arguments the way a POSIX shell would for
/// plain words, single and double quotes, and backslash escapes. No
/// expansion happens; the preview only shows where arguments break.
pub fn split_command_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                in_word = true;
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    current.push(quoted);
                }
            }
            '"' => {
                in_word = true;
                while let Some(quoted) = chars.next() {
                    match quoted {
                        '"' => break,
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                current.push(escaped);
                            }
                        }
                        other => current.push(other),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
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
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(tokens: &[&str]) -> Vec<OsString> {
        tokens.iter().map(OsString::from).collect()
    }

    #[test]
    fn quoting_keeps_plain_words_bare_and_wraps_the_rest() {
        assert_eq!(
            shell_join(&os(&["tmux", "new-session", "-d", "-s", "work"])),
            "tmux new-session -d -s work"
        );
        assert_eq!(shell_quote(OsStr::new("")), "''");
        assert_eq!(shell_quote(OsStr::new("a b")), "'a b'");
        assert_eq!(shell_quote(OsStr::new("it's")), r"'it'\''s'");
        assert_eq!(shell_quote(OsStr::new(";")), r"\;");
        assert_eq!(shell_quote(OsStr::new("$HOME")), "'$HOME'");
    }

    #[test]
    fn quoted_commands_split_back_into_the_same_argv() {
        let argv = os(&[
            "tmux",
            "new-session",
            "-c",
            "/a b/c",
            "claude",
            "--msg",
            "it's \"q\" $x `y`",
            "",
            ";",
            "set-option",
            "é ü",
        ]);
        assert_eq!(
            split_command_line(&shell_join(&argv)),
            argv.iter()
                .map(|t| t.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn splits_tmux_start_commands() {
        assert_eq!(
            split_command_line(r#"sh -c "sleep 60" "extra arg""#),
            ["sh", "-c", "sleep 60", "extra arg"]
        );
        assert_eq!(
            split_command_line(r#""sleep 62 ; echo hi""#),
            ["sleep 62 ; echo hi"]
        );
        assert!(split_command_line("   ").is_empty());
    }
}
