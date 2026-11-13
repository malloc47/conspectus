use super::*;

fn rows(value: &FieldValue, now: Option<i64>) -> Vec<String> {
    value_lines(value, now, crate::tui::Theme::default().muted)
        .expect("lines")
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn command_lines_split_like_a_shell() {
    assert_eq!(
        split_command_line(r#"claude --resume abc -p 'two words' "a \"q\"" x\ y"#),
        [
            "claude",
            "--resume",
            "abc",
            "-p",
            "two words",
            r#"a "q""#,
            "x y"
        ]
    );
    assert_eq!(
        rows(
            &FieldValue::Command(vec!["claude".into(), "--resume".into(), "abc".into()]),
            None
        ),
        ["claude", "  --resume", "  abc"]
    );
}

#[test]
fn a_timestamp_shows_utc_and_its_age() {
    let lines = rows(&FieldValue::Timestamp(1_700_000_000), Some(1_700_003_600));
    assert!(
        lines.contains(&"UTC    2023-11-14 22:13:20".to_string()),
        "{lines:?}"
    );
    assert_eq!(lines.last().map(String::as_str), Some("age    1h ago"));
    #[cfg(unix)]
    assert!(lines[0].starts_with("local  2023-11-1"), "{lines:?}");
}

#[test]
fn a_fixed_set_value_shows_its_entry_or_says_it_has_none() {
    assert_eq!(
        rows(
            &FieldValue::Glossary {
                field: "backend",
                values: vec!["tmux".into()],
            },
            None
        ),
        ["tmux", "A tmux session on the default socket."]
    );
    assert_eq!(
        rows(
            &FieldValue::Glossary {
                field: "backend",
                values: vec!["screen".into()],
            },
            None
        ),
        ["screen", "no glossary entry for `screen`"]
    );
    assert_eq!(
        glossary_entry("depth", "2").as_deref(),
        Some("2 levels below the pane's root process.")
    );
}

#[test]
fn text_and_lists_show_in_full() {
    let long = "x".repeat(300);
    assert_eq!(rows(&FieldValue::Text(long.clone()), None), [long]);
    assert_eq!(
        rows(&FieldValue::List(vec!["a".into(), "b".into()]), None),
        ["a", "b"]
    );
}

#[test]
fn every_live_state_word_has_an_entry() {
    use crate::model::live::{LiveConfidence, LiveDetail, LiveSource, LiveState};
    let words = LiveState::ALL
        .iter()
        .map(|state| ("state", state.name()))
        .chain(
            LiveDetail::ALL
                .iter()
                .map(|detail| ("state", detail.name())),
        )
        .chain(
            LiveSource::ALL
                .iter()
                .map(|source| ("source", source.name())),
        )
        .chain(
            LiveConfidence::ALL
                .iter()
                .map(|confidence| ("evidence", confidence.name())),
        );
    for (field, value) in words {
        assert!(
            glossary_entry(field, value).is_some(),
            "no glossary entry for {field} `{value}`"
        );
    }
}
