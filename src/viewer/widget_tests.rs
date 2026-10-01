use super::*;
use crate::viewer::model::{
    SessionLocator, TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
};
use std::path::PathBuf;

fn sample_document() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "0b34e59c".to_string(),
            cwd: Some("/p/proj".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "what's up?".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "not much".to_string(),
                timestamp: None,
                aborted: false,
            },
        ],
    }
}

fn unavailable_document() -> TranscriptDocument {
    TranscriptDocument::unavailable(&SessionLocator {
        harness_key: "claude-code".to_string(),
        session_key: "missing".to_string(),
        state_root: PathBuf::from("/x"),
    })
}

fn long_document() -> TranscriptDocument {
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "long".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: Vec::new(),
    };
    for i in 0..30 {
        doc.turns.push(TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: format!("turn {i}"),
            timestamp: None,
            aborted: false,
        });
    }
    doc
}

fn document_with_compaction() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "compact".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "before".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::CompactionSummary,
                body: "summary of prior turns".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "after".to_string(),
                timestamp: None,
                aborted: false,
            },
        ],
    }
}

fn document_with_tools() -> TranscriptDocument {
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tools".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "what files are here?".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolUse,
                body: "Glob: **/*.rs".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolResult,
                body: "src/lib.rs\nsrc/main.rs".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "two Rust files in src/.".to_string(),
                timestamp: None,
                aborted: false,
            },
        ],
    }
}

/// Drop the trailing whitespace each row pads to width — keeps
/// snapshots terminal-width-agnostic to padding noise.
fn snapshot_string(buf: &ratatui::buffer::Buffer) -> String {
    let raw = buffer_to_string(buf);
    raw.lines()
        .map(|l| l.trim_end_matches(' '))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn normal_open_lands_on_last_turn_with_gutter_chips() {
    let doc = sample_document();
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let buf = render_to_buffer(&mut state, &theme, 60, 12);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn jump_to_start_shows_top_of_long_document() {
    let theme = Theme::default();
    let mut state = ViewerState::new(long_document());
    let _ = render_to_buffer(&mut state, &theme, 60, 10);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
    let buf = render_to_buffer(&mut state, &theme, 60, 10);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn empty_document_renders_unavailable_banner() {
    let theme = Theme::default();
    let mut state = ViewerState::new(unavailable_document());
    let buf = render_to_buffer(&mut state, &theme, 60, 8);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn compaction_summary_turn_renders_with_compact_chip() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_compaction());
    let buf = render_to_buffer(&mut state, &theme, 60, 14);
    insta::assert_snapshot!(snapshot_string(&buf));
}

fn document_with_table() -> TranscriptDocument {
    let body = "Three-way comparison:\n\n\
            | Option | Effort | Risk |\n\
            |---|---|---|\n\
            | A | Small | Low |\n\
            | B | Medium | Medium |\n\
            | C | Large | High |\n\n\
            Recommend B.";
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tbl".to_string(),
            cwd: Some("/proj".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
        }],
    }
}

#[test]
fn table_snapshot_at_80_cols_wide_enough_to_fit_naturally() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_table());
    let buf = render_to_buffer(&mut state, &theme, 80, 18);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn table_snapshot_at_40_cols_forces_dynamic_wrap() {
    // Use a fixture with cell content that exceeds the
    // available content width (terminal 40 minus 10-cell gutter
    // minus the 3-cell ` │ ` separator = 27 cells). comfy-table's
    // ContentArrangement::Dynamic should shrink the widest
    // column and wrap its cells.
    let body = "Comparison of long-form options:\n\n\
            | Option | Description |\n\
            |---|---|\n\
            | Alpha | a sufficiently detailed description that demands wrapping |\n\
            | Beta | shorter |";
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "wrap".to_string(),
            cwd: Some("/proj".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
        }],
    };
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let buf = render_to_buffer(&mut state, &theme, 40, 20);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn tools_visible_at_full_detail_show_call_and_result_chips() {
    let theme = Theme::default();
    let state = ViewerState::new(document_with_tools());
    // Cycle to Full detail (3 t's: Hidden → Sum → Trunc → Full)
    // so the ToolUse + ToolResult turns render with full bodies.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert_eq!(state.tool_detail, ToolDetail::Full);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::JumpToStart);
    let buf = render_to_buffer(&mut state, &theme, 60, 16);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn narrow_terminal_wraps_body_but_keeps_chip_alignment() {
    let theme = Theme::default();
    let mut state = ViewerState::new(TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "wrap".to_string(),
            cwd: Some("/p".to_string()),
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: "one two three four five six seven eight nine ten eleven twelve thirteen"
                .to_string(),
            timestamp: None,
            aborted: false,
        }],
    });
    let buf = render_to_buffer(&mut state, &theme, 40, 12);
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn footer_shows_position_and_toggle_state() {
    let theme = Theme::default();
    let state = ViewerState::new(sample_document());
    // Cycle tool to Summary; toggle thinking on.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleThinking);
    let buf = render_to_buffer(&mut state, &theme, 100, 8);
    let s = buffer_to_string(&buf);
    assert!(
        s.contains("tools·sum"),
        "footer reports tools detail (sum): {s}"
    );
    assert!(s.contains("think·on"), "footer reports thinking state: {s}");
    assert!(s.contains("(t)"), "tools chip carries the t key hint");
    assert!(s.contains("(T)"), "thinking chip carries the T key hint");
    assert!(s.contains("[ "), "footer carries position counter");
}

#[test]
fn help_overlay_renders_keybindings_panel_when_show_help_is_set() {
    let theme = Theme::default();
    let mut state = ViewerState::new(sample_document());
    state.show_help = true;
    let buf = render_to_buffer(&mut state, &theme, 70, 18);
    let s = buffer_to_string(&buf);
    assert!(
        s.contains("viewer keybindings"),
        "panel title visible:\n{s}"
    );
    assert!(s.contains("q / Esc / Ctrl-C"), "close binding visible");
    assert!(s.contains("Toggle this help panel"), "? binding listed");
    assert!(s.contains("Jump to end"), "G/End binding listed");
    insta::assert_snapshot!(snapshot_string(&buf));
}

#[test]
fn render_cache_persists_across_draws_until_toggle_invalidates() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_tools());
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache = state
        .rendered
        .as_ref()
        .expect("cache populated on first draw");
    let cached_lines = cache.lines.len();
    let cached_width = cache.content_width;

    // Second draw at the same width must hit the cache. We can't
    // *prove* it from the outside without instrumentation, but
    // we can at least assert the cache is still populated and
    // the metrics are stable.
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache_after = state.rendered.as_ref().expect("cache still up");
    assert_eq!(cache_after.lines.len(), cached_lines);
    assert_eq!(cache_after.content_width, cached_width);

    // Cycling tool detail must invalidate via the reducer.
    // The next draw rebuilds.
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert!(
        state.rendered.is_none(),
        "CycleToolDetail invalidates cache"
    );
    let _ = render_to_buffer(&mut state, &theme, 60, 14);
    let cache_post = state.rendered.as_ref().expect("cache repopulated");
    assert_ne!(
        cache_post.tool_detail,
        ToolDetail::Hidden,
        "rebuilt cache reflects new tool detail"
    );
    assert!(
        cache_post.lines.len() > cached_lines,
        "non-Hidden tool detail adds tool turns; lines should grow ({} → {})",
        cached_lines,
        cache_post.lines.len()
    );
}

#[test]
fn render_cache_invalidates_on_width_change() {
    let theme = Theme::default();
    let mut state = ViewerState::new(sample_document());
    let _ = render_to_buffer(&mut state, &theme, 80, 12);
    let lines_at_80 = state.rendered.as_ref().unwrap().lines.len();
    let _ = render_to_buffer(&mut state, &theme, 40, 12);
    let cache = state.rendered.as_ref().unwrap();
    let inner = compute_gutter_inner_width(&state);
    let content_width_at_40 = 40u16 - leader_width(inner);
    assert_eq!(
        cache.content_width, content_width_at_40,
        "cache reports the latest content_width"
    );
    // Narrower terminals usually produce more wrapped lines but
    // may also be the same for short bodies; just confirm the
    // cache was rebuilt (different width key).
    let _ = lines_at_80;
}

#[test]
fn draw_writes_viewport_height_and_total_lines_to_state() {
    let theme = Theme::default();
    let mut state = ViewerState::new(sample_document());
    let _ = render_to_buffer(&mut state, &theme, 40, 12);
    assert_eq!(
        state.viewport_height, 8,
        "12 total - 4 chrome (header/2 seps/footer)"
    );
    assert!(state.total_lines > 0);
}

#[test]
fn cycle_tool_detail_makes_tool_turns_visible() {
    let theme = Theme::default();
    let mut state = ViewerState::new(document_with_tools());
    let before = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
    assert!(!before.contains("Glob:"), "tools hidden by default");

    // Cycle three times so we land on Full detail.
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let after = buffer_to_string(&render_to_buffer(&mut state, &theme, 60, 14));
    assert!(after.contains("Glob:"), "tool turns visible at Full detail");
    assert!(after.contains("tool  │"), "tool chip label visible");
}

#[test]
fn markdown_table_in_message_body_renders_as_aligned_table() {
    // Real-shape body modeled on a Claude assistant turn: a
    // brief prose intro, a GFM pipe-table, then a prose
    // outro. The table must render as an aligned block with
    // column separators (UTF8_BORDERS_ONLY preset) and the
    // surrounding prose must stay intact.
    let body = "Comparison of options:\n\n\
            | Option | Effort | Risk |\n\
            |---|---|---|\n\
            | A | Small | Low |\n\
            | B | Medium | Medium |\n\
            | C | Large | High |\n\n\
            Recommend B.";
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "tbl".to_string(),
            cwd: None,
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::Assistant,
            kind: TurnKind::Message,
            body: body.to_string(),
            timestamp: None,
            aborted: false,
        }],
    };
    let theme = Theme::default();
    let mut state = ViewerState::new(doc);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 20));
    assert!(s.contains("Comparison of options"), "prose intro: {s}");
    assert!(s.contains("Recommend B"), "prose outro: {s}");
    assert!(
        s.contains("Option") && s.contains("Effort") && s.contains("Risk"),
        "header cells present: {s}"
    );
    assert!(s.contains("Medium"), "body cell present: {s}");
    // UTF8_NO_BORDERS preset uses `┆` as the column separator
    // glyph; presence confirms aligned rendering rather than
    // raw pipe fallthrough. The gutter rule (`│`) appears
    // separately for every line, so we look specifically for the
    // table's dotted column separator.
    assert!(
        s.contains('┆'),
        "column separators rendered (not raw pipes): {s}"
    );
    // And the raw GFM separator row must not survive into the
    // output — that would mean tui-markdown got the table.
    assert!(
        !s.contains("|---|"),
        "raw pipe-separator row should not leak: {s}"
    );
}

#[test]
fn aborted_turns_hidden_by_default_visible_after_toggle() {
    let theme = Theme::default();
    let doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "abrt".to_string(),
            cwd: None,
        },
        turns: vec![
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "first prompt".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::Message,
                body: "first reply".to_string(),
                timestamp: None,
                aborted: false,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "interrupted thought".to_string(),
                timestamp: None,
                aborted: true,
            },
            TranscriptTurn {
                role: TurnRole::User,
                kind: TurnKind::Message,
                body: "replacement prompt".to_string(),
                timestamp: None,
                aborted: false,
            },
        ],
    };
    let mut state = ViewerState::new(doc);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 14));
    assert!(
        s.contains("first prompt"),
        "non-aborted user msg shows: {s}"
    );
    assert!(s.contains("replacement prompt"), "next prompt shows");
    assert!(
        !s.contains("interrupted thought"),
        "aborted user msg hidden by default: {s}"
    );

    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::ToggleAborted);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 80, 14));
    assert!(
        s.contains("interrupted thought"),
        "aborted msg revealed after toggle: {s}"
    );
}

#[test]
fn summary_tool_detail_aggregates_consecutive_tool_runs() {
    let theme = Theme::default();
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "agg".to_string(),
            cwd: None,
        },
        turns: vec![TranscriptTurn {
            role: TurnRole::User,
            kind: TurnKind::Message,
            body: "do the thing".to_string(),
            timestamp: None,
            aborted: false,
        }],
    };
    // Run: Read x2, Bash x4, Edit x2 (each ToolUse + ToolResult).
    let categories = [("Read", 2), ("Bash", 4), ("Edit", 2)];
    for (name, count) in categories {
        for i in 0..count {
            doc.turns.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolUse,
                body: format!("{name}: arg{i}"),
                timestamp: None,
                aborted: false,
            });
            doc.turns.push(TranscriptTurn {
                role: TurnRole::Assistant,
                kind: TurnKind::ToolResult,
                body: format!("result {i}"),
                timestamp: None,
                aborted: false,
            });
        }
    }
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        body: "done".to_string(),
        timestamp: None,
        aborted: false,
    });
    let state = ViewerState::new(doc);
    // Cycle once: Hidden → Summary.
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    assert_eq!(state.tool_detail, ToolDetail::Summary);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    assert!(s.contains("tool  │"), "tool chip present at Summary");
    // The aggregated phrase replaces 16 per-turn lines.
    assert!(
        s.contains("Read 2 files, ran 4 shell commands, edited 2 files"),
        "aggregate phrase visible: {s}"
    );
    // Surrounding non-tool turns still render normally.
    assert!(s.contains("do the thing"), "user message stays");
    assert!(s.contains("done"), "ai closing message stays");
    // No per-call chip body should leak through Summary —
    // operator wouldn't see individual "Bash: arg0" lines.
    assert!(
        !s.contains("Bash: arg"),
        "individual call args hidden in Summary: {s}"
    );
}

#[test]
fn summary_aggregation_separates_runs_around_messages() {
    let theme = Theme::default();
    let mut doc = TranscriptDocument {
        meta: TranscriptMeta {
            harness: "claude-code".to_string(),
            session_key: "split".to_string(),
            cwd: None,
        },
        turns: Vec::new(),
    };
    // Run 1: Read x1.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolUse,
        body: "Read: file.rs".to_string(),
        timestamp: None,
        aborted: false,
    });
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolResult,
        body: "contents".to_string(),
        timestamp: None,
        aborted: false,
    });
    // Message between the runs.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::Message,
        body: "inspecting".to_string(),
        timestamp: None,
        aborted: false,
    });
    // Run 2: Bash x1.
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolUse,
        body: "Bash: ls".to_string(),
        timestamp: None,
        aborted: false,
    });
    doc.turns.push(TranscriptTurn {
        role: TurnRole::Assistant,
        kind: TurnKind::ToolResult,
        body: "out".to_string(),
        timestamp: None,
        aborted: false,
    });
    let state = ViewerState::new(doc);
    let (mut state, _) =
        crate::viewer::input::reduce(state, crate::viewer::input::ViewerMsg::CycleToolDetail);
    let s = buffer_to_string(&render_to_buffer(&mut state, &theme, 100, 14));
    // Both aggregates appear (singular forms).
    assert!(s.contains("Read 1 file"), "first run aggregate: {s}");
    assert!(
        s.contains("Ran 1 shell command"),
        "second run aggregate: {s}"
    );
}
