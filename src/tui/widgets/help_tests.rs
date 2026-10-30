use super::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tuikit::help::HelpRow;
use tuikit::overlay::Outcome;
use tuikit::testkit::keys::{ctrl, press};

/// Where a description starts: the indent and the key column.
const KEY_COLUMN_WIDTH: usize = 16;

/// The page's lines laid out `width` cells wide (`None` wraps nothing),
/// the footer last, as the modal draws them.
fn body_lines_for_width(theme: &Theme, width: Option<usize>) -> Vec<Line<'static>> {
    let help = open(theme);
    let width = width.unwrap_or(usize::MAX);
    let mut lines = help.content().lines(theme, width);
    lines.extend(help.footer().lines(width, theme));
    lines
}

fn draw(help: &mut KeymapHelp, area: Rect, theme: &Theme) -> Buffer {
    let mut buf = Buffer::empty(area);
    help.draw(area, &mut buf, theme);
    buf
}

fn body_lines(theme: &Theme) -> Vec<Line<'static>> {
    body_lines_for_width(theme, None)
}

/// The keymap page's rows and the keymap's bindings, both ways.
///
/// Table to page: every binding has exactly one row, whose keys are every
/// key it binds and whose description starts with its help text.
///
/// Page to table: every key row is a binding's row (each key it lists
/// reaches that binding in its scope) or an annotation's row that starts
/// with the key of the binding it follows; no annotation names a binding
/// the keymap lacks.
#[test]
fn the_keymap_page_and_the_keymap_agree_both_ways() {
    use tuikit::keys::{ActiveScopes, Binding, Scope};
    let theme = Theme::default();
    let keymap = crate::tui::keybindings::keymap();
    let notes = annotations(&theme);
    assert_eq!(tuikit::help::unanchored(keymap, &notes), Vec::<&str>::new());
    let rows: Vec<(String, String)> = sections(&theme)
        .into_iter()
        .flat_map(|section| section.rows)
        .filter_map(|row| match row {
            HelpRow::Keys {
                keys, description, ..
            } => Some((keys, description.into_owned())),
            HelpRow::Legend { .. } => None,
        })
        .collect();
    let labels = |binding: &Binding<_, _>| -> Vec<String> {
        let mut labels: Vec<String> = Vec::new();
        for key in binding.keys() {
            let label = tuikit::keys::key_label(key);
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
        labels
    };
    // Table to page.
    for binding in keymap.bindings() {
        let joined = labels(binding).join(" / ");
        let found = rows
            .iter()
            .filter(|(keys, description)| {
                *keys == joined && description.starts_with(binding.help())
            })
            .count();
        assert_eq!(found, 1, "`{}` ({joined}) has {found} rows", binding.id());
    }
    // Page to table.
    let after: Vec<(&str, String)> = notes
        .iter()
        .filter_map(|note| match note {
            HelpAnnotation::RowAfter {
                id,
                row: HelpRow::Keys { keys, .. },
            } => Some((*id, keys.clone())),
            _ => None,
        })
        .collect();
    for (keys, description) in &rows {
        let binding = keymap.bindings().iter().find(|binding| {
            labels(binding).join(" / ") == *keys && description.starts_with(binding.help())
        });
        let Some(binding) = binding else {
            let (anchor, _) = after
                .iter()
                .find(|(_, row)| row == keys)
                .unwrap_or_else(|| panic!("`{keys}` is no binding's row: {description}"));
            let first = keymap
                .get(anchor)
                .and_then(|binding| labels(binding).into_iter().next())
                .expect("an anchored binding");
            assert!(
                keys.starts_with(&format!("{first} ")),
                "`{keys}` follows `{anchor}` but does not start with its key `{first}`"
            );
            continue;
        };
        let active = if let Scope::Pane(name) = binding.scope() {
            ActiveScopes::pane(name)
        } else {
            ActiveScopes::global()
        };
        for key in binding.keys() {
            let reached = keymap.resolve(active, key.example()).map(Binding::id);
            assert_eq!(reached, Some(binding.id()), "`{keys}` row: {description}");
        }
    }
}

#[test]
fn help_body_includes_node_kind_icon_legend() {
    // Pressing `?` should surface a built-in
    // legend for the ADR 0073 glyph slate so operators learn
    // the symbol vocabulary without cross-referencing the
    // docs. Every NodeKind in canonical display order must
    // appear with its glyph, its operator-facing display
    // name, and a short blurb.
    let theme = Theme::default();
    let lines = body_lines(&theme);
    let plain_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();
    let body_text = plain_lines.join("\n");
    assert!(
        body_text.contains("Node kind icons"),
        "expected section header in:\n{body_text}",
    );
    for kind in NodeKind::ALL {
        let glyph = node_kind_style(kind, &theme).glyph;
        let name = node_kind_display_name(kind);
        assert!(
            plain_lines
                .iter()
                .any(|line| line.contains(&glyph) && line.contains(name)),
            "expected legend row for {kind:?} ({glyph} {name}) in:\n{body_text}",
        );
    }
}

#[test]
fn help_body_includes_mux_state_legend() {
    let theme = Theme::default();
    let texts: Vec<String> = body_lines(&theme).iter().map(line_text).collect();
    let header = texts
        .iter()
        .position(|t| t == "Link and mux state")
        .expect("mux state section header");
    let legend = &texts[header + 1..];
    for (glyph, name) in [
        ("⊸", "Linked"),
        ("∘", "Not linked"),
        ("⊘", "Stopped"),
        ("?", "Unknown"),
        ("▲", "Ambiguous mux"),
    ] {
        assert!(
            legend
                .iter()
                .any(|line| line.starts_with(&format!("  {glyph} {name}"))),
            "expected mux legend row for {glyph} {name} in:\n{}",
            legend.join("\n"),
        );
    }
    // The half-filled circle once meant ambiguity and now marks a draft
    // pull request; the legend may only use it for that.
    assert!(
        texts
            .iter()
            .filter(|t| t.contains('◐'))
            .all(|t| t.contains("Draft")),
        "◐ may only appear on the draft pull request row",
    );
}

#[test]
fn esc_closes_help_overlay() {
    let mut help = open(&Theme::default());
    assert_eq!(help.handle_key(press(KeyCode::Esc)), Outcome::Close);
}

#[test]
fn q_and_question_mark_close_help_overlay() {
    let mut help = open(&Theme::default());
    assert_eq!(help.handle_key(press(KeyCode::Char('q'))), Outcome::Dismiss);
    assert_eq!(help.handle_key(press(KeyCode::Char('?'))), Outcome::Close);
}

#[test]
fn ctrl_c_closes_help_overlay() {
    let mut help = open(&Theme::default());
    assert_eq!(help.handle_key(ctrl('c')), Outcome::Dismiss);
}

#[test]
fn other_keys_are_swallowed_not_propagated() {
    let mut help = open(&Theme::default());
    assert_eq!(
        help.handle_key(press(KeyCode::Char('j'))),
        Outcome::Consumed
    );
    assert_eq!(help.handle_key(press(KeyCode::Enter)), Outcome::Consumed);
    assert_eq!(
        help.handle_key(press(KeyCode::Char('/'))),
        Outcome::Consumed,
        "the keymap help has no find"
    );
    assert!(!help.takes_text());
}

#[test]
fn body_lines_cover_every_documented_key() {
    let theme = Theme::default();
    let rendered: String = body_lines(&theme)
        .iter()
        .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
        .collect::<Vec<_>>()
        .join(" ");
    // Every key-literal reference used in a bind() call must
    // appear in the rendered help text.
    for needle in [
        "f ",
        "? ",
        "Enter ",
        "a ",
        "i ",
        "R ",
        "v ",
        "r ",
        "S ",
        "q / Ctrl-C",
        "1 ",
        "6 ",
        "] ",
        "[ ",
        "F ",
        "Ctrl-G",
        "/ ",
        "s / Ctrl-/ / Ctrl-_ / Ctrl-7",
        "↑ / k",
        "↓ / j",
        "← / h",
        "→ / l",
        "PgDn",
        "PgUp",
        "Home / g",
        "End / G",
        "Tab",
        "J ",
        "K ",
        "o ",
        "e ",
        "Backspace",
        "E ",
        "o ",
    ] {
        assert!(
            rendered.contains(needle),
            "help text missing `{needle}` reference; full text:\n{rendered}"
        );
    }
    // Every action description must also be visible so the
    // operator knows what each binding does.
    for desc in [
        "Open the controls overlay",
        "This help",
        "Default action on the selected row",
        "Attach to the selected mux",
        "Copy the selected agent or mux session's full id",
        "Rename the selected agent session (alias) or mux (tmux + pin cascade)",
        "Open the selected session's transcript",
        "Refresh discovery now",
        "Resume the selected un-muxed agent session",
        "Quit",
        "Switch directly to the sessions view",
        "Switch directly to the pinned view",
        "Cycle to the next view",
        "Cycle to the previous view",
        "Clear all active filters",
        "Cycle grouping forward",
        "Quick select: find a row of this view",
        "Find anything, whichever view it lives in",
        "Search contents: what was said in the sessions and what is in mux scrollback",
        "Search view: show the previous (older) remembered search",
        "Search view: show the next (newer) remembered search",
        "Move the selection down",
        "Move the selection up",
        "Expand the selected left-tree row",
        "Collapse the selected left-tree row",
        "Page down through the row tree",
        "Jump to the first row",
        "Jump to the last row",
        "Return to where the last reveal started",
        "Cycle focus",
        "Scroll the right-panel preview",
        "Move the explorer cursor",
        "Open or close the Other zone",
        "Back out of the most recent drilldown",
        "Toggle Expanded Node Detail",
        "Toggle edge meta",
        "Open the full untruncated value",
    ] {
        assert!(
            rendered.contains(desc),
            "help text missing description `{desc}`; full text:\n{rendered}"
        );
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn narrow_width_wraps_descriptions_under_a_hanging_indent() {
    let theme = Theme::default();
    let width = 60;
    let lines = body_lines_for_width(&theme, Some(width));
    // The icon legend is a fixed-width table; only the keymap wraps.
    let keymap_end = lines
        .iter()
        .position(|l| line_text(l) == "Node kind icons")
        .expect("legend header");
    for line in &lines[..keymap_end] {
        assert!(
            unicode_width::UnicodeWidthStr::width(line_text(line).as_str()) <= width,
            "line exceeds {width} cells: {:?}",
            line_text(line)
        );
    }
    // The long `w` description continues on an indented line.
    let texts: Vec<String> = lines.iter().map(line_text).collect();
    let w_idx = texts
        .iter()
        .position(|t| t.starts_with("  w "))
        .expect("w binding");
    assert!(texts[w_idx + 1].starts_with(&" ".repeat(KEY_COLUMN_WIDTH)));
    assert!(body_lines(&theme).len() < lines.len());
}

#[test]
fn jumping_to_the_end_shows_the_last_lines_not_blank_space() {
    let theme = Theme::default();
    let mut help = open(&theme);
    help.handle_key(press(KeyCode::Char('G')));
    let area = Rect::new(0, 0, 80, 24);
    let buf = draw(&mut help, area, &theme);
    let window = help.window();
    assert_eq!(window.offset(), window.last_offset(), "the last page");
    let rendered: String = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Esc/q/? close"),
        "end of keymap should be visible after G:\n{rendered}"
    );
    assert!(
        rendered.contains("Enter starts it"),
        "the last legend row, not a blank body:\n{rendered}"
    );
}

/// Widths the help modal is checked at, as terminal widths.
const TERMINAL_WIDTHS: [u16; 4] = [60, 80, 120, 160];

/// The cells the help body's text gets at a terminal width: the modal's
/// inner width, less the scrollbar column.
fn text_width_at(terminal_width: u16) -> usize {
    let modal = HelpPage::SIZE.rect(Rect::new(0, 0, terminal_width, 40), 0);
    usize::from(modal.width.saturating_sub(2)).saturating_sub(1)
}

fn words(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .flat_map(|line| {
            line_text(line)
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn no_help_line_is_wider_than_the_modal_at_any_width() {
    let theme = Theme::default();
    for terminal in TERMINAL_WIDTHS {
        let width = text_width_at(terminal);
        for line in body_lines_for_width(&theme, Some(width)) {
            let text = line_text(&line);
            assert!(
                unicode_width::UnicodeWidthStr::width(text.as_str()) <= width,
                "at {terminal} columns a help line is {} cells in a {width}-cell body: {text:?}",
                unicode_width::UnicodeWidthStr::width(text.as_str())
            );
        }
    }
}

#[test]
fn every_word_of_every_help_page_survives_wrapping() {
    let theme = Theme::default();
    let source = words(&body_lines_for_width(&theme, None));
    assert!(source.len() > 200, "the help page has its text");
    for terminal in TERMINAL_WIDTHS {
        let wrapped = words(&body_lines_for_width(&theme, Some(text_width_at(terminal))));
        assert_eq!(
            wrapped, source,
            "at {terminal} columns wrapping lost, split, or reordered words"
        );
    }
}

#[test]
fn the_rendered_help_modal_shows_every_word_as_you_scroll() {
    let theme = Theme::default();
    let source = words(&body_lines_for_width(&theme, None));
    for terminal in TERMINAL_WIDTHS {
        let area = Rect::new(0, 0, terminal, 24);
        let mut seen = std::collections::BTreeSet::new();
        let mut help = open(&theme);
        for _ in 0..80 {
            // As the app draws it: what the frame showed sizes the next page.
            let buf = draw(&mut help, area, &theme);
            // The text columns only: inside the frame, left of the
            // scrollbar.
            let modal = HelpPage::SIZE.rect(area, 0);
            for y in modal.y + 1..modal.y + modal.height - 1 {
                let row: String = (modal.x + 1..modal.x + modal.width - 2)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect();
                seen.extend(row.split_whitespace().map(str::to_string));
            }
            help.handle_key(press(KeyCode::PageDown));
        }
        let missing: Vec<&String> = source.iter().filter(|word| !seen.contains(*word)).collect();
        assert!(
            missing.is_empty(),
            "at {terminal} columns these words never appear whole: {:?}",
            &missing[..missing.len().min(12)]
        );
    }
}

/// The content search row names the key the search itself binds for
/// listing sessions or messages, not the Search view's.
#[test]
fn the_content_search_row_names_the_key_the_search_binds() {
    use crate::tui::keybindings::overlay::{SEARCH_CONTENT_KEYS, SearchKey};
    use tuikit::keys::{Hint, KeyRow, rows};
    let label = rows(SEARCH_CONTENT_KEYS)
        .find_map(|row| match row {
            KeyRow::Bind {
                action: SearchKey::Granularity,
                hint: Hint::Shown { label, .. },
                ..
            } => Some(*label),
            _ => None,
        })
        .expect("the search shows its sessions/messages key");
    let row = sections(&Theme::default())
        .into_iter()
        .flat_map(|section| section.rows)
        .find_map(|row| match row {
            HelpRow::Keys {
                keys, description, ..
            } if keys.contains("Ctrl-/") => Some(description),
            _ => None,
        })
        .expect("the content search row");
    assert!(
        row.contains(&format!("{label} switches between listing sessions")),
        "{row}"
    );
}

/// The right pane's section is generated from the table the keymap takes
/// in as the right pane's scope, so every key it binds has a row that says
/// what it does, and the page lists no key the pane does not bind.
#[test]
fn the_right_panes_section_is_generated_from_its_table() {
    use crate::tui::keybindings::RIGHT_PANE_KEYS;
    use tuikit::keys::{KeyRow, key_label, rows};
    assert!(tuikit::help::undescribed(RIGHT_PANE_KEYS).is_empty());
    let section = sections(&Theme::default())
        .into_iter()
        .find(|section| section.title == "Detail-pane graph explorer (right focus)")
        .expect("the right pane's section");
    let keys: Vec<String> = section
        .rows
        .iter()
        .filter_map(|row| match row {
            HelpRow::Keys { keys, .. } => Some(keys.clone()),
            HelpRow::Legend { .. } => None,
        })
        .collect();
    for row in rows(RIGHT_PANE_KEYS) {
        if let KeyRow::Bind { key, .. } = row {
            let label = key_label(key);
            assert!(
                keys.iter()
                    .any(|row| row.split(" / ").any(|key| key == label)),
                "`{label}` has no row: {keys:?}"
            );
        }
    }
    // One row per action the table binds, then the program-wide `E`.
    assert_eq!(keys.len(), 14, "{keys:?}");
    assert_eq!(keys.last().map(String::as_str), Some("E"));
}
