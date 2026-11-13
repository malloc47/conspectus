//! Columns line up: the rows the left pane lays out from one column
//! schema put their lined-up cells in one place on every row, at every
//! width, and a session row lays its cells out alike in every view that
//! lists it. A group header's path and count, and a Pinned or History
//! row's state word and age, are in one column at every depth.
//!
//! The checks read the drawn frame, not the layout code: each row's
//! whitespace-separated tokens (`tuikit::testkit::buffer::tokens`) give
//! the columns its cells start and end in.

use ratatui::buffer::Buffer;
use tuikit::testkit::buffer::{Token, rows, tokens};

use super::*;
use crate::model::{Confidence, GraphLink, LinkEndpoint, LinkState, Provenance, RelationKind};
use crate::tui::Grouping;
use crate::tui::icons::state;
use crate::tui::rows::RowKind;
use tuikit::glyphs::marks;

/// The terminal widths the checks draw at: two side by side (110, 160)
/// and two stacked (60, 80).
const WIDTHS: [u16; 4] = [60, 80, 110, 160];

/// Tall enough that a stacked pane shows every row of these trees.
const HEIGHT: u16 = 100;

/// The shared world plus a mux three agents run in, so the Mux view has
/// a mux whose chips overflow and sessions nested under it.
fn world_with_a_shared_mux() -> GraphSnapshot {
    let mut snapshot = world();
    let mux_id = MuxSessionId::new("tmux:pair");
    let mut mux = MuxSessionNode::new(mux_id.clone(), "tmux", "pair");
    mux.cwd = Some("/home/op/src/conspectus".into());
    mux.backend_session_id = Some("$7".into());
    mux.activity_epoch = Some(NOW - 30);
    snapshot.nodes.push(GraphNode::MuxSession(mux));
    for (n, harness) in ["claude-code", "codex", "opencode"].into_iter().enumerate() {
        let id = AgentSessionId::new(harness, "/state", format!("pair{n:04}"));
        let mut session = AgentSessionNode::new(id.clone(), harness);
        session.cwd = Some("/home/op/src/conspectus".into());
        session.last_active_epoch = Some(NOW - 100 * i64::try_from(n).unwrap_or(0) - 50);
        session.last_message_preview = Some(format!("working on part {n}"));
        snapshot.nodes.push(GraphNode::AgentSession(session));
        snapshot.candidate_links.push(GraphLink {
            id: format!("pair-{n}"),
            source: NodeId::AgentSession(id),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id.clone()),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    resolve_snapshot(snapshot)
}

/// The inner rows of the left pane: the bordered pane whose title is the
/// tab strip, wherever the layout put it.
pub(super) fn left_pane(buffer: &Buffer) -> Vec<String> {
    let lines = rows(buffer);
    let top = lines
        .iter()
        .position(|line| line.contains("sessions · mux"))
        .expect("the left pane's title");
    let title: Vec<char> = lines[top].chars().collect();
    let left = title.iter().position(|c| *c == '┌').expect("a corner");
    let right = left
        + title[left..]
            .iter()
            .position(|c| *c == '┐')
            .expect("a corner");
    lines[top + 1..]
        .iter()
        .take_while(|line| line.chars().nth(left) != Some('└'))
        .map(|line| line.chars().skip(left + 1).take(right - left - 1).collect())
        .collect()
}

/// What kind of row a showing row is, for grouping the rows whose cells
/// must line up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Header,
    Session,
    Mux,
    Pr,
    /// A Pinned or History row.
    Mixed,
}

/// Each showing row's kind and depth beside the text the pane drew for
/// it. The pane is tall enough that its window starts at the first row.
fn drawn_rows(app: &mut App, width: u16) -> Vec<(Kind, u8, String)> {
    let buffer = buffer_of(app, width, HEIGHT);
    let lines = left_pane(&buffer);
    let showing: Vec<(Option<Kind>, u8)> = app
        .visible_rows()
        .iter()
        .map(|row| {
            let kind = match &row.kind {
                RowKind::Group(_) => Some(Kind::Header),
                RowKind::AgentSession(_) => Some(Kind::Session),
                RowKind::MuxSession(_) => Some(Kind::Mux),
                RowKind::Mixed(mixed) if mixed.state.is_some() => Some(Kind::Mux),
                RowKind::Pr(_) => Some(Kind::Pr),
                RowKind::Mixed(_) => Some(Kind::Mixed),
                _ => None,
            };
            (kind, row.depth)
        })
        .collect();
    assert!(
        showing.len() <= lines.len(),
        "the pane shows every row: {} rows, {} lines",
        showing.len(),
        lines.len()
    );
    showing
        .into_iter()
        .zip(lines)
        .filter_map(|((kind, depth), line)| kind.map(|kind| (kind, depth, line)))
        .collect()
}

/// Where a row's lined-up cells sit (see [`grid`]).
type Grid = (Option<usize>, Option<usize>);

fn is_state(token: &str) -> bool {
    [
        state::LINKED,
        state::UNLINKED,
        state::UNKNOWN,
        state::STOPPED,
        state::PR_OPEN,
        state::PR_DRAFT,
        state::PR_MERGED,
        state::PR_CLOSED,
    ]
    .contains(&token)
}

/// The marks a row starts with: the cursor's pointer in the tree's gutter,
/// the disclosure, the kind glyphs, and the pin markers.
fn is_mark(token: &str) -> bool {
    token == marks::POINTER
        || ["▶", "▼", "▦", "◆", "◇", "●", "▣", "⇄", "⌥", "⋎", "◉", "◎"].contains(&token)
}

/// The first of `tokens` after `from` that stands two or more cells from
/// the one before it: where the next cell starts, past a cell whose text
/// has single spaces in it (a label, a state word).
fn next_cell(tokens: &[Token], from: usize) -> Option<usize> {
    (from + 1..tokens.len()).find(|&at| tokens[at].start >= tokens[at - 1].end + 2)
}

/// Where a header's path starts: the cell after its label. `None` for a
/// header with no path, or whose path gave way.
fn header_path(line: &str) -> Option<usize> {
    let tokens = tokens(line);
    let label = tokens.iter().position(|token| !is_mark(&token.text))?;
    let path = next_cell(&tokens, label)?;
    let text = tokens[path].text.as_str();
    let count = text.starts_with('(') && text.ends_with(')');
    (!count && text != marks::WARNING).then_some(tokens[path].start)
}

/// Where a row's lined-up cells sit: for a session or a mux, where its age
/// ends and where its state glyph is; for a header, where its count ends.
/// `None` for a cell the row does not show.
fn grid(kind: Kind, line: &str) -> Grid {
    let tokens: Vec<Token> = tokens(line);
    match kind {
        Kind::Header => (
            tokens
                .iter()
                .find(|token| token.text.starts_with('(') && token.text.ends_with(')'))
                .map(|token| token.end),
            None,
        ),
        Kind::Session | Kind::Mux | Kind::Pr => {
            let age = tokens.iter().position(|token| is_age(&token.text));
            let state = age
                .and_then(|at| tokens.get(at + 1))
                .filter(|token| is_state(&token.text));
            (age.map(|at| tokens[at].end), state.map(|token| token.start))
        }
        // The age, and the state word before it: the cell after the label.
        Kind::Mixed => {
            let age = tokens.iter().position(|token| is_age(&token.text));
            let label = tokens.iter().position(|token| !is_mark(&token.text));
            let word = label.and_then(|at| next_cell(&tokens, at));
            (
                age.map(|at| tokens[at].end),
                word.map(|at| tokens[at].start),
            )
        }
    }
}

/// The column the first token after a mux's glyph and marker starts in:
/// its name.
fn mux_name_column(line: &str) -> Option<usize> {
    tokens(line)
        .into_iter()
        .skip_while(|token| token.text != "▣")
        .skip(1)
        .find(|token| !["◉", "◎"].contains(&token.text.as_str()))
        .map(|token| token.start)
}

/// Every app the checks draw: each grouping of the Sessions view, of the
/// Mux view with every stopped mux listed, and of the PRs, Pinned, and
/// History views.
fn apps() -> Vec<(String, App)> {
    let world = world_with_a_shared_mux();
    let mut apps = Vec::new();
    for view in [
        View::Sessions,
        View::Mux,
        View::Prs,
        View::Pinned,
        View::History,
    ] {
        for grouping in Grouping::values_for(view) {
            let filter = RowFilter {
                show_stopped_muxes: view == View::Mux,
                ..RowFilter::default()
            };
            let mut app = app_over_filtered(view, &world, filter);
            app.update(Msg::SetGrouping(*grouping));
            // Open every group, so nested rows show too.
            let ids: Vec<RowId> = app
                .tree()
                .rows
                .iter()
                .filter(|row| row.expandable)
                .map(|row| row.id.clone())
                .collect();
            for id in ids {
                app.set_selection(id);
                app.update(Msg::ExpandRow);
            }
            apps.push((format!("{view:?}/{grouping:?}"), app));
        }
    }
    apps
}

#[test]
fn the_lined_up_cells_of_every_row_kind_share_their_columns_at_every_width() {
    for (name, mut app) in apps() {
        for width in WIDTHS {
            let drawn = drawn_rows(&mut app, width);
            let mut seen: std::collections::BTreeMap<(Kind, u8), Grid> =
                std::collections::BTreeMap::new();
            let mut counts: Option<usize> = None;
            let mut paths: Option<usize> = None;
            let mut mixed: Option<Grid> = None;
            let mut names: std::collections::BTreeMap<u8, usize> =
                std::collections::BTreeMap::new();
            for (kind, depth, line) in &drawn {
                let (end, glyph) = grid(*kind, line);
                if *kind == Kind::Header {
                    // A header's count is in one column at every depth,
                    // and so is its path.
                    if let Some(end) = end {
                        let first = *counts.get_or_insert(end);
                        assert_eq!(first, end, "{name} at {width}: header counts\n{line}");
                    }
                    if let Some(start) = header_path(line) {
                        let first = *paths.get_or_insert(start);
                        assert_eq!(first, start, "{name} at {width}: header paths\n{line}");
                    }
                    continue;
                }
                if *kind == Kind::Mixed {
                    // A Pinned or History row's age and state word are in
                    // one column at every depth.
                    let first = *mixed.get_or_insert((end, glyph));
                    assert_eq!(
                        first,
                        (end, glyph),
                        "{name} at {width}: Pinned or History rows put their age and state \
                         word in different columns\n{line}"
                    );
                    continue;
                }
                let entry = seen.entry((*kind, *depth)).or_insert((end, glyph));
                assert_eq!(
                    *entry,
                    (end, glyph),
                    "{name} at {width}: {kind:?} rows at depth {depth} put their age and state \
                     in different columns\n{line}"
                );
                if *kind == Kind::Mux {
                    let column = mux_name_column(line).expect("a mux name");
                    let first = *names.entry(*depth).or_insert(column);
                    assert_eq!(first, column, "{name} at {width}: mux names\n{line}");
                }
            }
            assert!(!drawn.is_empty(), "{name} at {width} drew rows");
        }
    }
}

#[test]
fn a_session_row_lays_its_cells_out_alike_in_every_view_that_lists_it() {
    // Measured from the session's own glyph, so the depth a view nests it
    // at does not count.
    let from_glyph = |line: &str| -> Grid {
        let glyph = tokens(line)
            .into_iter()
            .find(|token| token.text == "●")
            .map(|token| token.start)
            .expect("the session glyph");
        let (age, state) = grid(Kind::Session, line);
        (age.map(|at| at - glyph), state.map(|at| at - glyph))
    };
    for width in WIDTHS {
        let mut layouts: Vec<(String, Grid)> = Vec::new();
        for (name, mut app) in apps() {
            for (kind, _, line) in drawn_rows(&mut app, width) {
                if kind == Kind::Session {
                    layouts.push((name.clone(), from_glyph(&line)));
                }
            }
        }
        let nested = layouts
            .iter()
            .filter(|(name, _)| name.starts_with("Mux"))
            .count();
        assert!(
            nested > 0,
            "the Mux view nests sessions under the shared mux"
        );
        let first = layouts[0].1;
        for (name, layout) in &layouts {
            assert_eq!(
                *layout, first,
                "at {width}, a session row in {name} puts its age and state elsewhere"
            );
        }
    }
}

#[test]
fn a_mux_with_more_agents_than_its_chips_show_keeps_its_age_in_line() {
    let world = world_with_a_shared_mux();
    let filter = RowFilter {
        show_stopped_muxes: true,
        ..RowFilter::default()
    };
    let mut app = app_over_filtered(View::Mux, &world, filter);
    let buffer = buffer_of(&mut app, 160, 40);
    let lines = left_pane(&buffer);
    let pair = lines
        .iter()
        .find(|line| line.contains("pair"))
        .expect("the shared mux");
    assert!(
        pair.contains("+1"),
        "two chips and a count of the third: {pair}"
    );
    let ages: Vec<Option<usize>> = lines
        .iter()
        .filter(|line| line.contains('▣'))
        .map(|line| grid(Kind::Mux, line).0)
        .collect();
    assert!(ages.len() > 2, "{lines:#?}");
    assert!(
        ages.windows(2).all(|pair| pair[0] == pair[1]),
        "every mux's age ends in one column: {ages:?}\n{}",
        lines.join("\n")
    );
}
