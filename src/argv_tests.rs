use super::*;

fn os(tokens: &[&str]) -> Vec<OsString> {
    tokens.iter().map(OsString::from).collect()
}

/// What `line` splits into through both readers, which agree on finished
/// text.
fn split(line: &str) -> Vec<String> {
    let lenient = split_command_line(line);
    assert_eq!(
        parse_command_line(line).as_ref(),
        Ok(&lenient),
        "the readers disagree on {line:?}"
    );
    lenient
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
        r"a\b",
    ]);
    assert_eq!(
        split(&shell_join(&argv)),
        argv.iter()
            .map(|t| t.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn splits_tmux_start_commands() {
    assert_eq!(
        split(r#"sh -c "sleep 60" "extra arg""#),
        ["sh", "-c", "sleep 60", "extra arg"]
    );
    assert_eq!(split(r#""sleep 62 ; echo hi""#), ["sleep 62 ; echo hi"]);
    assert!(split("   ").is_empty());
    assert!(split("").is_empty());
}

#[test]
fn whitespace_separates_and_counts_once() {
    assert_eq!(
        split("  claude\t--model   opus  "),
        ["claude", "--model", "opus"]
    );
}

#[test]
fn single_quotes_keep_everything_as_written() {
    assert_eq!(
        split(r#"claude --model 'opus 4' 'a\b "c" $d'"#),
        ["claude", "--model", "opus 4", r#"a\b "c" $d"#]
    );
    assert_eq!(split(r"'it'\''s'"), ["it's"]);
}

#[test]
fn double_quotes_take_a_backslash_only_before_the_four_specials() {
    assert_eq!(
        split(r#""say \"hi\"" "\\" "\$HOME" "\`x\`""#),
        [r#"say "hi""#, r"\", "$HOME", "`x`"]
    );
    // The edge case the launch and pin edit forms used to read two ways:
    // before any other character the backslash stays.
    assert_eq!(split(r#""a\b""#), [r"a\b"]);
    assert_eq!(split(r#""\n\t""#), [r"\n\t"]);
}

#[test]
fn outside_quotes_a_backslash_makes_the_next_character_plain() {
    assert_eq!(split(r"x\ y \'q\' \\ \a"), ["x y", "'q'", r"\", "a"]);
}

#[test]
fn pieces_with_nothing_between_them_are_one_argument() {
    assert_eq!(
        split(r#"--name="two words" a'b'"c"d"#),
        ["--name=two words", "abcd"]
    );
    assert_eq!(split(r#"a '' "" b"#), ["a", "", "", "b"]);
}

#[test]
fn nothing_is_expanded_or_run() {
    assert_eq!(
        split("echo $HOME ~ *.rs ; ls | wc && `id`"),
        [
            "echo", "$HOME", "~", "*.rs", ";", "ls", "|", "wc", "&&", "`id`"
        ]
    );
}

#[test]
fn unfinished_text_is_refused_by_the_parser_and_read_by_the_splitter() {
    assert_eq!(
        parse_command_line("claude 'oops"),
        Err(Unfinished::Quote('\''))
    );
    assert_eq!(split_command_line("claude 'oops"), ["claude", "oops"]);
    assert_eq!(
        parse_command_line(r#"claude "oops \""#),
        Err(Unfinished::Quote('"'))
    );
    assert_eq!(
        split_command_line(r#"claude "oops \""#),
        ["claude", r#"oops ""#]
    );
    assert_eq!(parse_command_line(r"claude \"), Err(Unfinished::Escape));
    assert_eq!(split_command_line(r"claude \"), ["claude", r"\"]);
    assert_eq!(
        parse_command_line(r#""a\"#),
        Err(Unfinished::Quote('"')),
        "an open quote is what is unfinished"
    );
    assert_eq!(split_command_line(r#""a\"#), [r"a\"]);
}

#[test]
fn the_reasons_read_as_the_end_of_a_sentence() {
    assert_eq!(Unfinished::Quote('\'').to_string(), "an unclosed `'` quote");
    assert_eq!(Unfinished::Escape.to_string(), "a trailing escape");
}
