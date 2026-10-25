use super::*;

fn info(label: Option<&str>) -> SessionInfo {
    SessionInfo {
        harness: "claude-code".into(),
        session: "abc123".into(),
        label: label.map(str::to_string),
    }
}

fn hit(turn: usize, text: &str, range: std::ops::Range<usize>) -> Hit {
    Hit {
        turn_index: turn,
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        timestamp: DateTime::from_timestamp(1_700_000_000, 0),
        snippet: text.to_string(),
        highlight: range,
    }
}

fn found(group: Grouping, rows: Vec<Row>) -> Found {
    Found {
        group,
        kinds: ContentKinds::default(),
        progress: Progress {
            scanned: 3,
            total: 4,
            skipped: 1,
        },
        parserless: 2,
        rows,
        truncated: Vec::new(),
        limited: None,
    }
}

fn session_row(
    label: Option<&str>,
    count: usize,
    text: &str,
    range: std::ops::Range<usize>,
) -> Row {
    let best = hit(0, text, range);
    Row::Session {
        info: info(label),
        hits: SessionHits {
            count,
            latest: best.timestamp,
            best,
        },
    }
}

const NOW: i64 = 1_700_000_000 + 2 * 3600;

#[test]
fn the_session_table_aligns_columns_and_ends_with_the_snippet() {
    let f = found(
        Grouping::Session,
        vec![
            session_row(Some("Fix the parser"), 12, "found the needle here", 10..16),
            session_row(None, 1, "another needle", 8..14),
        ],
    );
    let text = render_table(&f, None, false, NOW);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("SESSION"), "{text}");
    assert!(lines[0].ends_with("SNIPPET"), "{text}");
    assert!(lines[1].contains("Fix the parser"), "{text}");
    assert!(lines[1].contains("12"), "{text}");
    assert!(lines[1].contains("2h"), "{text}");
    assert!(
        lines[2].contains("abc123"),
        "an unlabelled session shows its id: {text}"
    );
    assert!(lines[0].contains("LATEST"), "{text}");
    let snippet_col = lines[0].find("SNIPPET").unwrap();
    assert_eq!(lines[1].find("found").unwrap(), snippet_col, "{text}");
    assert_eq!(lines[2].find("another").unwrap(), snippet_col, "{text}");
}

#[test]
fn the_message_table_shows_turn_role_kind_and_time() {
    let f = found(
        Grouping::Message,
        vec![Row::Message {
            info: info(Some("Fix the parser")),
            hit: Hit {
                kind: TurnKind::ToolResult,
                ..hit(7, "the needle", 4..10)
            },
        }],
    );
    let text = render_table(&f, None, false, NOW);
    let header = text.lines().next().unwrap();
    for column in [
        "SESSION", "HARNESS", "TURN", "ROLE", "KIND", "TIME", "SNIPPET",
    ] {
        assert!(header.contains(column), "{header}");
    }
    let row = text.lines().nth(1).unwrap();
    assert!(
        row.contains("#8"),
        "turns count from 1 like the viewer: {row}"
    );
    assert!(row.contains(" ai "), "{row}");
    assert!(row.contains("tool output"), "{row}");
}

#[test]
fn no_rows_print_nothing() {
    assert_eq!(
        render_table(&found(Grouping::Session, vec![]), None, false, NOW),
        ""
    );
}

#[test]
fn a_width_cuts_only_the_snippet_and_never_below_a_floor() {
    let long = "x".repeat(200);
    let f = found(
        Grouping::Session,
        vec![session_row(Some("s"), 1, &long, 0..1)],
    );
    let text = render_table(&f, Some(60), false, NOW);
    let row = text.lines().nth(1).unwrap();
    assert!(display_width(row) <= 60, "{row}");
    assert!(row.ends_with('…'), "{row}");
    let tiny = render_table(&f, Some(5), false, NOW);
    let snippet = tiny.lines().nth(1).unwrap().rsplit("  ").next().unwrap();
    assert!(display_width(snippet) >= MIN_SNIPPET_WIDTH, "{tiny}");
}

#[test]
fn color_bolds_the_match_and_a_cut_keeps_it_in_view() {
    let text = "the quick brown needle jumps";
    let range = 16..22;
    let plain = fit_snippet(text, &range, None, false);
    assert_eq!(plain, text);
    let bold = fit_snippet(text, &range, None, true);
    assert!(bold.contains("\x1b[1mneedle\x1b[0m"), "{bold:?}");
    // The cut keeps the match in view, so it is still bold.
    let cut = fit_snippet(text, &range, Some(12), true);
    assert!(cut.contains("\x1b[1mneedle\x1b[0m"), "{cut:?}");
    assert!(cut.starts_with('…') && cut.ends_with('…'), "{cut:?}");
    // A match wider than the line is cut to what fits.
    let (wide, _) = crop_around(text, &range, 4);
    assert!(display_width(&wide) <= 4, "{wide:?}");
    let empty = fit_snippet(text, &(0..0), None, true);
    assert_eq!(empty, text);
}

#[test]
fn json_documents_the_query_grouping_kinds_and_counts() {
    let f = found(
        Grouping::Session,
        vec![session_row(Some("Fix"), 2, "a needle", 2..8)],
    );
    let value: Value = serde_json::from_str(&render_json("  needle ", &f).unwrap()).unwrap();
    assert_eq!(value["query"], "needle");
    assert_eq!(value["group"], "session");
    assert_eq!(value["kinds"], json!(["messages"]));
    assert_eq!(
        value["searched"],
        json!({"sessions": 3, "total": 4, "skipped": 1, "without_parser": 2})
    );
    let result = &value["results"][0];
    assert_eq!(result["hits"], 2);
    assert_eq!(result["label"], "Fix");
    assert_eq!(result["latest"], "2023-11-14T22:13:20Z");
    assert_eq!(result["snippet"]["highlight"], json!([2, 8]));
}

#[test]
fn json_per_message_rows_carry_the_turn() {
    let f = found(
        Grouping::Message,
        vec![Row::Message {
            info: info(None),
            hit: hit(3, "x needle", 2..8),
        }],
    );
    let value: Value = serde_json::from_str(&render_json("needle", &f).unwrap()).unwrap();
    let result = &value["results"][0];
    assert_eq!(result["turn"], 4, "1-based");
    assert_eq!(result["role"], "assistant");
    assert_eq!(result["kind"], "message");
    assert_eq!(result["label"], Value::Null);
    assert!(result.get("hits").is_none());
}

#[test]
fn the_width_flag_wins_and_a_pipe_is_unbounded() {
    assert_eq!(table_width(Some(90), false), Some(90));
    assert_eq!(table_width(Some(90), true), Some(90));
    assert_eq!(table_width(None, false), None);
}

#[test]
fn an_unnamed_session_shows_eight_characters_of_its_key() {
    let mut f = found(
        Grouping::Session,
        vec![session_row(None, 1, "a needle", 2..8)],
    );
    if let Row::Session { info, .. } = &mut f.rows[0] {
        info.session = "0123456789abcdef".into();
    }
    let text = render_table(&f, None, false, NOW);
    assert!(text.contains("01234567"), "{text}");
    assert!(!text.contains("012345678"), "{text}");
}

#[test]
fn a_cropped_snippet_keeps_the_match_and_cuts_on_word_edges() {
    let text = "alpha beta gamma delta needle epsilon zeta eta theta iota kappa";
    let start = text.find("needle").unwrap();
    let range = start..start + 6;
    for budget in [14, 20, 30, 40] {
        let (cropped, hl) = crop_around(text, &range, budget);
        assert!(display_width(&cropped) <= budget, "{budget}: {cropped}");
        assert_eq!(&cropped[hl], "needle", "{budget}: {cropped}");
    }
    let (cropped, _) = crop_around(text, &range, 30);
    assert!(
        cropped.starts_with('…') && cropped.ends_with('…'),
        "{cropped}"
    );
    let inner = cropped.trim_matches('…');
    assert!(!inner.starts_with(' '), "{cropped}");
    let bold = fit_snippet(text, &range, Some(30), true);
    assert!(bold.contains("\x1b[1mneedle\x1b[0m"), "{bold:?}");
}

#[test]
fn a_wide_character_snippet_is_cropped_by_cells_without_splitting() {
    let text = "日本語のテキスト needle もっと日本語のテキスト";
    let start = text.find("needle").unwrap();
    let (cropped, hl) = crop_around(text, &(start..start + 6), 24);
    assert!(display_width(&cropped) <= 24, "{cropped}");
    assert_eq!(&cropped[hl], "needle");
}

#[test]
fn a_match_wider_than_the_budget_is_itself_cut() {
    let text = format!("{} tail", "n".repeat(50));
    let (cropped, _) = crop_around(&text, &(0..50), 10);
    assert!(display_width(&cropped) <= 10, "{cropped}");
}

#[test]
fn json_lists_truncated_sessions() {
    let mut f = found(
        Grouping::Message,
        vec![Row::Message {
            info: info(None),
            hit: hit(0, "x needle", 2..8),
        }],
    );
    f.truncated.push(Truncated {
        info: info(None),
        matches: 900,
        shown: 500,
    });
    let value: Value = serde_json::from_str(&render_json("needle", &f).unwrap()).unwrap();
    assert_eq!(value["truncated"][0]["matches"], 900);
    assert_eq!(value["truncated"][0]["shown"], 500);
}

#[test]
fn a_zero_limit_is_refused() {
    assert!(parse_limit("0").is_err());
    assert_eq!(parse_limit("3"), Ok(3));
    assert!(parse_limit("x").is_err());
}

#[test]
fn the_flat_table_uses_the_message_columns_in_the_given_order() {
    let newer = Hit {
        timestamp: DateTime::from_timestamp(1_700_000_100, 0),
        ..hit(2, "newer needle", 6..12)
    };
    let older = hit(9, "older needle", 6..12);
    let other = SessionInfo {
        session: "zzz999".into(),
        ..info(Some("Other"))
    };
    let f = found(
        Grouping::Flat,
        vec![
            Row::Message {
                info: other,
                hit: newer,
            },
            Row::Message {
                info: info(Some("Fix the parser")),
                hit: older,
            },
        ],
    );
    let text = render_table(&f, None, false, NOW);
    let header = text.lines().next().unwrap();
    for column in ["SESSION", "TURN", "ROLE", "KIND", "TIME", "SNIPPET"] {
        assert!(header.contains(column), "{header}");
    }
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[1].contains("Other") && lines[1].contains("#3"),
        "{text}"
    );
    assert!(
        lines[2].contains("Fix the parser") && lines[2].contains("#10"),
        "rows keep the order they were given, which is newest first: {text}"
    );
}

#[test]
fn json_flat_names_the_group_and_reports_a_limit() {
    let mut f = found(
        Grouping::Flat,
        vec![Row::Message {
            info: info(None),
            hit: hit(0, "x needle", 2..8),
        }],
    );
    f.limited = Some(Limited { shown: 1, total: 5 });
    let value: Value = serde_json::from_str(&render_json("needle", &f).unwrap()).unwrap();
    assert_eq!(value["group"], "flat");
    assert_eq!(value["limited"]["shown"], 1);
    assert_eq!(value["limited"]["total"], 5);
    assert_eq!(value["results"][0]["turn"], 1);
}

#[test]
fn json_has_no_limit_when_nothing_was_cut() {
    let f = found(Grouping::Flat, Vec::new());
    let value: Value = serde_json::from_str(&render_json("needle", &f).unwrap()).unwrap();
    assert_eq!(value["limited"], Value::Null);
}
