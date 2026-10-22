//! Command-line splitting shared by previews and history.

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
