use super::*;

fn texts(source: &str) -> Vec<String> {
    string_literals(source)
        .into_iter()
        .map(|literal| literal.text)
        .collect()
}

#[test]
fn code_blanks_comments_and_literals_and_keeps_lines() {
    let source = "let a = \"Color::Red\"; // Color::Blue\n/* Color::Green\n */ let b = 'x';\nlet c = Color::Cyan;\n";
    let blanked = code(source);
    assert_eq!(blanked.lines().count(), source.lines().count());
    assert_eq!(blanked.matches("Color::").count(), 1);
    assert!(blanked.lines().nth(3).unwrap().contains("Color::Cyan"));
    assert_eq!(blanked.chars().count(), source.chars().count());
}

#[test]
fn without_comments_keeps_the_literals() {
    let source = "let a = \"kept\"; // dropped\n";
    let kept = without_comments(source);
    assert!(kept.contains("\"kept\"") && !kept.contains("dropped"));
}

#[test]
fn test_items_are_blanked_and_the_code_after_them_is_not() {
    let source =
        "fn live() {}\n#[cfg(test)]\nmod tests {\n    fn f() { \"}\"; }\n}\nfn after() {}\n";
    let stripped = strip_test_items(source);
    assert!(stripped.contains("fn live") && stripped.contains("fn after"));
    assert!(!stripped.contains("mod tests") && !stripped.contains("fn f"));
    assert_eq!(stripped.lines().count(), source.lines().count());

    let production = production_code("#[cfg(test)]\nmod tests;\nfn kept() { Command::new(); }\n");
    assert!(production.contains("Command::new") && !production.contains("mod tests"));
}

#[test]
fn literals_skip_comments_tests_and_character_literals() {
    let source = "let a = \"keep\"; // \"not this\"\n/* \"nor this\" */\nlet b = r#\"raw \"q\" \"#; let c = '\"'; let d = \"e\\\"s\";\n#[cfg(test)]\nlet e = \"test only\";\nlet f = b\"bytes\";";
    assert_eq!(texts(source), ["keep", "raw \"q\" ", "e\"s", "bytes"]);
}

#[test]
fn literals_carry_the_line_they_start_on() {
    let source = "// \"one\"\nlet a = \"two\nlines\";\nlet b = \"three\";\n";
    let found = string_literals(source);
    let lines: Vec<(usize, &str)> = found
        .iter()
        .map(|literal| (literal.line, literal.text.as_str()))
        .collect();
    assert_eq!(lines, [(2, "two\nlines"), (4, "three")]);
}

#[test]
fn escapes_are_resolved() {
    let source = r#"let a = "tab\there\nnew \\ \x41 \u{2026} \'q\'"; let b = "joined \
                    line";"#;
    assert_eq!(
        texts(source),
        ["tab\there\nnew \\ A \u{2026} 'q'", "joined line"]
    );
}

#[test]
fn a_raw_identifier_and_a_lifetime_are_not_literals() {
    let source = "fn r#match<'a>(x: &'a str) -> &'a str { x }\nlet s = \"real\";";
    assert_eq!(texts(source), ["real"]);
}

#[test]
fn a_needle_matches_only_where_a_word_starts() {
    assert!(contains_at_word_start("Command::new()", "Command::new"));
    assert!(contains_at_word_start(
        "std::process::Command::new()",
        "Command::new"
    ));
    assert!(!contains_at_word_start("MyCommand::new()", "Command::new"));
    assert!(!contains_at_word_start("my_Command::new()", "Command::new"));
    assert!(is_identifier_char('_') && !is_identifier_char(':'));
}

#[test]
fn the_walk_lists_files_sorted_and_skips_build_and_hidden_directories() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let found = rust_files(&root.join("src/testkit"));
    assert!(found.iter().any(|path| path.ends_with("source.rs")));
    assert!(found.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        found
            .iter()
            .all(|path| path.extension().is_some_and(|e| e == "rs"))
    );
    let every = files(&root.join("src/testkit"));
    assert!(
        every
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "golden"))
    );
    assert_eq!(files(&root.join("src/lib.rs")), [root.join("src/lib.rs")]);
    assert!(files(&root.join("no-such-directory")).is_empty());
    let crate_files = files(root);
    assert!(
        crate_files
            .iter()
            .all(|path| !path.components().any(|part| part.as_os_str() == "target"))
    );
}
