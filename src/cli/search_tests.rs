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
        pane_progress: None,
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
    let wide = fit_snippet(text, &range, Some(4), false);
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

/// Where the bold match sits in a fitted snippet: the plain text before
/// it, the match, and the text after it.
fn split_at_match(fitted: &str) -> (&str, &str, &str) {
    let (before, rest) = fitted.split_once("\x1b[1m").expect("a bold match");
    let (matched, after) = rest.split_once("\x1b[0m").expect("the bold ends");
    (before, matched, after)
}

#[test]
fn a_cropped_snippet_keeps_the_match_and_cuts_on_word_edges() {
    let text = "alpha beta gamma delta needle epsilon zeta eta theta iota kappa";
    let words: Vec<&str> = text.split(' ').collect();
    let start = text.find("needle").unwrap();
    let range = start..start + 6;
    for budget in [8, 14, 20, 30, 40, 60] {
        let cropped = fit_snippet(text, &range, Some(budget), false);
        assert!(display_width(&cropped) <= budget, "{budget}: {cropped}");
        assert!(cropped.contains("needle"), "{budget}: {cropped}");
        let inner = cropped.trim_matches('…');
        assert!(
            !inner.starts_with(' ') && !inner.ends_with(' '),
            "{cropped}"
        );
        let shown: Vec<&str> = inner.split(' ').collect();
        let at = shown.iter().position(|word| *word == "needle").unwrap();
        for (i, word) in shown.iter().enumerate() {
            // A side whose only word is too wide for its room shows the
            // part that fits rather than nothing.
            let alone_on_its_side =
                (i == 0 && at == 1) || (i + 1 == shown.len() && at + 2 == shown.len());
            assert!(
                words.contains(word) || (alone_on_its_side && text.contains(word)),
                "{budget}: `{word}` is cut: {cropped}"
            );
        }
        let bold = fit_snippet(text, &range, Some(budget), true);
        assert_eq!(split_at_match(&bold).1, "needle", "{budget}: {bold:?}");
    }
    let cropped = fit_snippet(text, &range, Some(30), false);
    assert!(
        cropped.starts_with('…') && cropped.ends_with('…'),
        "{cropped}"
    );
    assert_eq!(fit_snippet(text, &range, Some(14), false), "…a needle eps…");
    let whole = fit_snippet(text, &range, Some(text.len()), false);
    assert_eq!(whole, text, "a snippet that fits is not cut");
}

#[test]
fn a_cut_gives_the_text_after_the_match_more_room() {
    // The TUI's fit: two thirds of the cells the match leaves go after it,
    // so a cut snippet reads on from the match rather than up to it.
    let text = "one two three four five six seven needle eight nine ten eleven twelve thirteen";
    let start = text.find("needle").unwrap();
    let range = start..start + 6;
    for budget in [24, 30, 40, 50] {
        let bold = fit_snippet(text, &range, Some(budget), true);
        let (before, _, after) = split_at_match(&bold);
        assert!(
            display_width(after) > display_width(before),
            "{budget}: {bold:?}"
        );
    }
    assert_eq!(
        fit_snippet(text, &range, Some(30), false),
        "…seven needle eight nine ten…"
    );
}

#[test]
fn a_wide_character_snippet_is_cropped_by_cells_without_splitting() {
    let text = "日本語のテキスト needle もっと日本語のテキスト";
    let start = text.find("needle").unwrap();
    let range = start..start + 6;
    for budget in [8, 12, 16, 20, 24, 30] {
        let cropped = fit_snippet(text, &range, Some(budget), false);
        assert!(display_width(&cropped) <= budget, "{budget}: {cropped}");
        assert!(cropped.contains("needle"), "{budget}: {cropped}");
        assert!(
            text.contains(cropped.trim_matches('…')),
            "{budget}: a cut falls between characters: {cropped}"
        );
        let bold = fit_snippet(text, &range, Some(budget), true);
        let (before, matched, after) = split_at_match(&bold);
        assert_eq!(matched, "needle", "{budget}: {bold:?}");
        // From 16 cells on there is room for a two-cell character and the
        // blank before the match, and the text on both sides shows.
        if budget >= 16 {
            let context = |side: &str| side.chars().any(|c| !c.is_whitespace() && c != '…');
            assert!(context(before) && context(after), "{budget}: {bold:?}");
        }
    }
    assert_eq!(
        fit_snippet(text, &range, Some(24), false),
        "…スト needle もっと日本…"
    );
    // A wide match: the cut never halves a two-cell character.
    let at = text.find("テキスト").unwrap();
    let wide = at..at + "テキスト".len();
    for budget in [3, 5, 7, 9] {
        let cropped = fit_snippet(text, &wide, Some(budget), false);
        assert!(display_width(&cropped) <= budget, "{budget}: {cropped}");
        assert!(cropped.ends_with('…'), "{budget}: {cropped}");
        assert!(text.contains(cropped.trim_end_matches('…')), "{cropped}");
    }
}

#[test]
fn a_match_wider_than_the_budget_is_itself_cut() {
    let text = format!("{} tail", "n".repeat(50));
    let cropped = fit_snippet(&text, &(0..50), Some(10), false);
    assert!(display_width(&cropped) <= 10, "{cropped}");
    assert!(cropped.ends_with('…'), "{cropped}");
    let bold = fit_snippet(&text, &(0..50), Some(10), true);
    assert_eq!(split_at_match(&bold).1, "n".repeat(9), "{bold:?}");
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

fn pane_row(command: Option<&str>, line: usize, count: usize) -> Row {
    use crate::content_search::{MuxRef, PaneHit, PaneRef};
    Row::Pane {
        pane: Box::new(PaneRef {
            mux: MuxRef {
                backend: "tmux".into(),
                socket: None,
                session: "work".into(),
            },
            pane_id: "%4".into(),
            window_index: "1".into(),
            pane_index: "0".into(),
            command: command.map(str::to_string),
        }),
        count,
        captured_at: 1_700_000_000,
        hit: PaneHit {
            line,
            snippet: "error: retry budget gone".into(),
            highlight: 7..19,
        },
    }
}

#[test]
fn pane_rows_get_a_table_of_their_own_after_the_sessions() {
    let rows = vec![
        session_row(Some("fix parser"), 3, "x retry y", 2..7),
        pane_row(Some("zsh"), 12, 2),
    ];
    let text = render_table(&found(Grouping::Session, rows), None, false, 1_700_000_100);

    let (sessions, panes) = text
        .split_once("\n\n")
        .expect("a blank line between tables");
    assert!(sessions.starts_with("SESSION"), "{text}");
    assert!(panes.starts_with("PANE"), "{text}");
    assert!(panes.contains("work:1.0"), "{text}");
    assert!(panes.contains("zsh"), "{text}");
    assert!(panes.contains("error: retry budget gone"), "{text}");
}

#[test]
fn only_panes_means_only_the_pane_table() {
    let text = render_table(
        &found(Grouping::Message, vec![pane_row(None, 12, 2)]),
        None,
        false,
        1_700_000_100,
    );

    assert!(text.starts_with("PANE"), "{text}");
    assert!(text.contains("LINE"), "{text}");
    assert!(text.contains("L12"), "{text}");
}

#[test]
fn pane_json_rows_name_their_source_and_location() {
    let mut f = found(Grouping::Session, vec![pane_row(Some("zsh"), 12, 2)]);
    f.pane_progress = Some(PaneProgress {
        progress: Progress {
            scanned: 3,
            total: 3,
            skipped: 0,
        },
        deduplicated: 5,
    });
    let json: serde_json::Value = serde_json::from_str(&render_json("retry", &f).unwrap()).unwrap();

    let row = &json["results"][0];
    assert_eq!(row["source"], "pane");
    assert_eq!(row["mux"], "work");
    assert_eq!(row["pane_id"], "%4");
    assert_eq!(row["line"], 12);
    assert_eq!(row["hits"], 2);
    assert_eq!(row["snippet"]["highlight"], serde_json::json!([7, 19]));
    assert_eq!(json["searched"]["panes"]["deduplicated"], 5);
}
