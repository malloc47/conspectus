//! User-visible text speaks the operator's language. ADR numbers,
//! backlog IDs, and design-document paths are developer artifacts and
//! never reach the screen, `--help`, or an error message; a pin is
//! called a pin, never by its storage word.
//!
//! The scans of source string literals cover the framework's sources
//! (`crates/tuikit/src`) for developer artifacts as well as the host's;
//! the storage-word rule, the rendered screens, and the help pages are
//! host-only. `tests/support/source_tree.rs` explains why, and how each
//! scan is guarded against visiting too few files.

use clap::CommandFactory;

use super::source_tree::{self, FRAMEWORK_SRC, Tree};
use super::*;

/// Documents written for operators, which messages may point at. Every
/// other document under `docs/` (decision records, the design, plans,
/// audits) is for developers.
const USER_GUIDES: &[&str] = &[
    "operations.md",
    "pins-walkthrough.md",
    "worktrees.md",
    "library-api.md",
    "comparison.md",
    "tui-columns.md",
];

/// Words that name developer artifacts.
pub(super) fn artifact_in(text: &str) -> Option<String> {
    for word in text.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '.' || c == '/')) {
        let word = word.trim_matches(|c: char| c == '.' || c == '-');
        if word == "ADR" || word.starts_with("ADR-") || word.starts_with("CSP-") {
            return Some(word.to_string());
        }
        if let Some(doc) = word.strip_prefix("docs/")
            && !USER_GUIDES.contains(&doc)
            && (doc.ends_with('/')
                || Path::new(doc)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md")))
        {
            return Some(word.to_string());
        }
    }
    None
}

/// Whether `text` uses the storage word. A format placeholder such as
/// `{slot}` names a variable, not words the operator reads.
pub(super) fn says_slot(text: &str) -> bool {
    let mut visible = String::with_capacity(text.len());
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => visible.push(ch),
            _ => {}
        }
    }
    visible
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case("slot") || word.eq_ignore_ascii_case("slots"))
}

#[test]
fn the_artifact_detector_finds_what_it_should() {
    assert_eq!(artifact_in("see ADR 0141"), Some("ADR".into()));
    assert_eq!(artifact_in("(ADR 0073 §3)"), Some("ADR".into()));
    assert_eq!(artifact_in("tracked in CSP-123"), Some("CSP-123".into()));
    assert_eq!(artifact_in("docs/design.md"), Some("docs/design.md".into()));
    assert_eq!(
        artifact_in("see docs/adr/0141-pin-slots.md"),
        Some("docs/adr/0141-pin-slots.md".into())
    );
    assert_eq!(artifact_in("see docs/operations.md"), None);
    assert_eq!(artifact_in("an adrenaline rush, a SQUADRON"), None);
    assert!(says_slot("pin slot `x`") && says_slot("Slots"));
    assert!(!says_slot("slotted into place"));
    assert!(
        !says_slot("unpinned `{slot}`"),
        "placeholders are variables"
    );
}

/// Frames of every view with a few rows selected, plus every overlay
/// that can be opened from them.
fn screens() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for view in ALL_VIEWS {
        let mut app = app_in(view);
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        for id in ids.iter().step_by(2).take(8) {
            app.set_selection(id.clone());
            out.push((describe(view, id), frame(&mut app, 160, 40)));
            let mut with_menu = app_in(view);
            with_menu.set_selection(id.clone());
            with_menu.update(Msg::OpenRowMenu);
            out.push((
                format!("{} menu", describe(view, id)),
                frame(&mut with_menu, 160, 40),
            ));
        }
        let mut overlays: Vec<(&str, App)> = Vec::new();
        for (name, open) in [
            ("help", App::open_help_overlay as fn(&mut App)),
            ("controls", App::open_controls_overlay),
            ("messages", App::open_messages_overlay),
            ("search", App::open_search_overlay),
            ("search contents", App::open_content_search_overlay),
            ("search everything", |app: &mut App| {
                app.open_search_overlay();
                super::layout::search_key(app, ratatui::crossterm::event::KeyCode::Tab);
                super::layout::type_into_search(app, "con");
            }),
        ] {
            let mut app = app_in(view);
            open(&mut app);
            overlays.push((name, app));
        }
        let mut mux_menu = app_in(view);
        mux_menu.open_mux_menu(crate::tui::widgets::mux_menu::MuxMenuState::new());
        overlays.push(("mux menu", mux_menu));
        for (name, mut app) in overlays {
            out.push((format!("{view:?} {name}"), frame(&mut app, 160, 60)));
        }
        for (name, msg) in [
            ("clean up", Msg::OpenCleanUp),
            ("launch form", Msg::OpenMuxLaunchForm),
            ("edit prompt", Msg::OpenSlotEdit("conspectus-main".into())),
            (
                "rename prompt",
                Msg::OpenSlotRename("conspectus-main".into()),
            ),
            (
                "retarget prompt",
                Msg::OpenSlotRetarget("conspectus-main".into()),
            ),
        ] {
            let mut app = app_in(view);
            app.update(msg);
            out.push((format!("{view:?} {name}"), frame(&mut app, 160, 40)));
        }
    }
    out
}

/// The sentences the TUI itself composes for a pin: status hints and
/// the toasts and log lines the reducer produces.
fn composed_text() -> Vec<(String, String)> {
    let snapshot = world();
    let mut out = Vec::new();
    for pin in crate::tui::rows::pin_nodes(&snapshot) {
        if let Some(hint) = crate::tui::actions::pin_status_hint(&snapshot, &pin.id.id) {
            out.push((format!("hint for {}", pin.id.id), hint));
        }
        let mut app = app_in(View::Pinned);
        select(&mut app, &pin.id.id);
        for effect in menu_toasts(&mut app) {
            out.push((format!("toast for {}", pin.id.id), effect));
        }
    }
    out
}

#[test]
fn no_screen_shows_a_developer_artifact() {
    let mut offenders = Vec::new();
    let screens = screens();
    assert!(screens.len() > 80, "{}", screens.len());
    for (name, text) in screens.iter().chain(composed_text().iter()) {
        if let Some(word) = artifact_in(text) {
            offenders.push(format!("{name}: {word}"));
        }
    }
    offenders.dedup();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn no_screen_calls_a_pin_a_slot() {
    let mut offenders = Vec::new();
    for (name, text) in screens().iter().chain(composed_text().iter()) {
        if says_slot(text) {
            let line = text.lines().find(|l| says_slot(l)).unwrap_or_default();
            offenders.push(format!("{name}: {}", line.trim()));
        }
    }
    offenders.dedup();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

fn help_of(command: &clap::Command, path: &str, out: &mut Vec<(String, String)>) {
    let mut command = command.clone();
    let help = command.render_long_help().to_string();
    out.push((path.to_string(), help));
    for sub in command.get_subcommands() {
        help_of(sub, &format!("{path} {}", sub.get_name()), out);
    }
}

#[test]
fn no_help_page_shows_a_developer_artifact() {
    let mut pages = Vec::new();
    help_of(&crate::cli::Cli::command(), "conspectus", &mut pages);
    assert!(
        pages.len() > 30,
        "every subcommand has a page: {}",
        pages.len()
    );
    let offenders: Vec<String> = pages
        .iter()
        .filter_map(|(path, help)| artifact_in(help).map(|word| format!("{path}: {word}")))
        .collect();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// The string literals of `source` outside comments, with their 1-based
/// line numbers. Understands escapes, raw strings, and char literals;
/// stops at the first `#[cfg(test)]` so test code is exempt.
fn string_literals(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let mut at_line_start = true;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            at_line_start = true;
            i += 1;
            continue;
        }
        if at_line_start && !c.is_whitespace() {
            at_line_start = false;
            if chars[i..].starts_with(&['#', '[', 'c', 'f', 'g', '(', 't', 'e', 's', 't', ')']) {
                break;
            }
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == 'r' && matches!(chars.get(i + 1), Some('"' | '#')) {
            let hashes = chars[i + 1..].iter().take_while(|c| **c == '#').count();
            if chars.get(i + 1 + hashes) == Some(&'"') {
                let start = i + hashes + 2;
                let close: Vec<char> = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let mut j = start;
                while j < chars.len() && !chars[j..].starts_with(&close) {
                    j += 1;
                }
                out.push((line, chars[start..j].iter().collect()));
                line += chars[start..j].iter().filter(|c| **c == '\n').count();
                i = j + close.len();
                continue;
            }
        }
        if c == '"' {
            let mut j = i + 1;
            let mut text = String::new();
            let start_line = line;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    j += 1;
                }
                if chars[j] == '\n' {
                    line += 1;
                }
                text.push(chars[j]);
                j += 1;
            }
            out.push((start_line, text));
            i = j + 1;
            continue;
        }
        if c == '\'' {
            // A char literal like '"' or '\n', not a lifetime.
            if chars.get(i + 1) == Some(&'\\') {
                i += 4;
                continue;
            }
            if chars.get(i + 2) == Some(&'\'') {
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[test]
fn the_literal_scanner_skips_comments_and_tests() {
    let source = "let a = \"keep\"; // \"not this\"\n/* \"nor this\" */\nlet b = r#\"raw \"q\" \"#; let c = '\"'; let d = \"e\\\"s\";\n#[cfg(test)]\nlet e = \"test only\";";
    let found: Vec<String> = string_literals(source)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(found, ["keep", "raw \"q\" ", "e\"s"]);
}

/// The host's TUI sources. The tripwire minimum is below the current
/// count (169 files) because code leaves `src/tui` for the framework.
const TUI_SRC: Tree = Tree::new("src/tui", 100);
/// The host's CLI sources (22 files today).
const CLI_SRC: Tree = Tree::new("src/cli", 15);

/// The non-test sources under `trees` with their text. The consistency
/// tests are skipped: they hold the offending words on purpose.
fn source_files(root: &Path, trees: &[Tree]) -> Vec<(PathBuf, String)> {
    source_tree::rust_files(root, trees)
        .into_iter()
        .filter(|path| {
            !source_tree::is_test_file(path)
                && !source_tree::relative(root, path).contains("consistency_tests")
        })
        .filter_map(|path| Some((path.clone(), std::fs::read_to_string(path).ok()?)))
        .collect()
}

/// String literals under `trees` that `bad` flags, as `path:line: ...`
/// findings.
fn literal_offenders(
    root: &Path,
    trees: &[Tree],
    bad: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (path, source) in source_files(root, trees) {
        for (line, text) in string_literals(&source) {
            if let Some(found) = bad(&text) {
                out.push(format!(
                    "{}:{line}: {found} in {:?}",
                    source_tree::relative(root, &path),
                    text.chars().take(70).collect::<String>()
                ));
            }
        }
    }
    out
}

fn slot_in(text: &str) -> Option<String> {
    says_slot(text).then(|| "slot".into())
}

#[test]
fn no_string_in_the_tui_names_a_developer_artifact() {
    let offenders = literal_offenders(source_tree::workspace_root(), &[TUI_SRC], artifact_in);
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn no_string_in_the_cli_names_a_developer_artifact() {
    let offenders = literal_offenders(source_tree::workspace_root(), &[CLI_SRC], artifact_in);
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// The framework's text reaches the operator through the host's screens,
/// so it follows the same rule as the host's own strings.
#[test]
fn no_string_in_the_framework_names_a_developer_artifact() {
    let offenders = literal_offenders(source_tree::workspace_root(), &[FRAMEWORK_SRC], artifact_in);
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// Host only: a pin is a Conspectus word the framework may not name, so a
/// "slot" there is some other generic thing.
#[test]
fn no_string_in_the_tui_calls_a_pin_a_slot() {
    let offenders = literal_offenders(source_tree::workspace_root(), &[TUI_SRC], slot_in);
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn an_artifact_in_a_string_is_caught_in_every_tree_the_scan_covers() {
    let violation = "fn f() { let _ = \"see ADR 0141\"; }\n";
    source_tree::assert_scan_covers_every_tree(
        &[TUI_SRC, CLI_SRC, FRAMEWORK_SRC],
        violation,
        |root, trees| literal_offenders(root, trees, artifact_in),
    );
}

#[test]
fn a_slot_in_a_string_is_caught_in_the_tui_tree() {
    let violation = "fn f() { let _ = \"unpin this slot\"; }\n";
    source_tree::assert_scan_covers_every_tree(&[TUI_SRC], violation, |root, trees| {
        literal_offenders(root, trees, slot_in)
    });
}
