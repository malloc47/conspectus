//! Independence guards for the framework crate.
//!
//! The crate must be liftable into its own repository, so it may know
//! nothing about the program that hosts it. Three guards enforce that, each
//! a pure function over text so the self-tests can feed it a deliberately
//! leaking snippet from memory (no leaking fixture ever sits on disk):
//!
//! 1. **Manifest** (`manifest_scan`): dependencies on the host, and
//!    dependencies off the allowlist.
//! 2. **Vocabulary** (`vocabulary_scan`): host-domain words anywhere in the
//!    crate, this file excepted (`GUARD_FILE`) because it has to contain the
//!    words to test for them.
//! 3. **Process and write** (`capability_scan`): process spawning and file
//!    writes in non-test source.
//!
//! The tree walk, the Rust sanitizer that blanks comments, literals, and
//! test items, and the report format are shared with the crate's other
//! source guards through `support/source_scan.rs`.
//!
//! What each guard checks, how matching works, how the file-count tripwires
//! (`MIN_MANIFESTS`, `MIN_VOCABULARY_FILES`, `MIN_CAPABILITY_FILES`) are
//! chosen, and the process for requesting a reviewed allowlist entry are
//! written down in the crate's contribution guide, `docs/contributing.md`
//! (section "Independence guards"). That guide is the single copy of the
//! rules; change it together with this file. The guide is itself scanned for
//! domain words, so it points at `DOMAIN_TERMS` instead of repeating the
//! list.
//!
//! # Why the manifest is parsed with `toml`
//!
//! Dependency tables come in many spellings (`[dependencies.foo]`, dotted
//! keys, target-specific tables, renames), so a line scan would miss cases.
//! `toml` is already a dependency of the host workspace; this crate adds it
//! as a dev-dependency only, through `[workspace.dependencies]`, so the
//! lockfile gains no package.

use std::path::Path;

#[path = "support/source_scan.rs"]
mod source_scan;

use source_scan::{
    Exception, Needle, Scan, Source, assert_clean, contains_at_word_start, crate_root,
    is_identifier_char, is_rust_file, is_test_path, load_sources, sanitize_rust, stale_exceptions,
    strip_test_items, tripwire, verdict,
};

// ---------------------------------------------------------------------------
// Policy: the reviewed lists. Edit these through the process in
// `docs/contributing.md`; every entry states why it is there.
// ---------------------------------------------------------------------------

/// Crate-relative path of this file. The vocabulary scan skips exactly this
/// path, because the word list and the self-test snippets must contain the
/// words they test for.
const GUARD_FILE: &str = "tests/independence.rs";

/// Fewest manifests the manifest scan may visit.
const MIN_MANIFESTS: usize = 1;
/// Fewest files the vocabulary scan may visit. The manifest, the README,
/// the 43 pages under `docs`, the glyph script, 236 files under `src` (20
/// goldens among them), and six test files besides this one make 288
/// today; the minimum is a little under three quarters of that.
const MIN_VOCABULARY_FILES: usize = 210;
/// Fewest non-test Rust files the process and write scan may visit: 140
/// under `src` today.
const MIN_CAPABILITY_FILES: usize = 100;

/// A dependency the framework may declare, and why.
struct Allowed {
    name: &'static str,
    reason: &'static str,
}

/// Dependencies allowed in `[dependencies]`, optional ones included. Nothing
/// else may appear there.
const RUNTIME_DEPENDENCIES: &[Allowed] = &[
    Allowed {
        name: "ratatui",
        reason: "the rendering library every control draws with",
    },
    Allowed {
        name: "crossterm",
        reason: "key and mouse event types the interaction layer maps",
    },
    Allowed {
        name: "unicode-width",
        reason: "display-width arithmetic for truncation, padding, and wrapping",
    },
    Allowed {
        name: "tui-input",
        reason: "the single-line edit engine behind the text-entry controls",
    },
    Allowed {
        name: "ansi-to-tui",
        reason: "optional rich-text feature: ANSI text into styled lines",
    },
    Allowed {
        name: "tui-markdown",
        reason: "optional rich-text feature: Markdown into styled lines",
    },
    Allowed {
        name: "comfy-table",
        reason: "optional rich-text feature: table layout for rendered content",
    },
];

/// Dependencies allowed only in `[dev-dependencies]` (the runtime
/// allowlist is also allowed there).
const DEV_DEPENDENCIES: &[Allowed] = &[
    Allowed {
        name: "toml",
        reason: "this file parses the crate manifests; already shared by the workspace",
    },
    Allowed {
        name: "insta",
        reason: "snapshot tests that move into the crate with the controls they cover",
    },
    Allowed {
        name: "tui-pantry",
        reason: "the widget preview example that moves in with the controls",
    },
];

/// Reviewed uses of a domain word. Empty: the crate needs none today.
const VOCABULARY_EXCEPTIONS: &[Exception] = &[];

/// Reviewed uses of a process or write API. Empty: the crate needs none
/// today.
const CAPABILITY_EXCEPTIONS: &[Exception] = &[];

/// Words that belong to the host application's domain, lower case and
/// singular. A listed word is matched whole, ignoring case, in any
/// identifier or prose form (see `docs/contributing.md`).
const DOMAIN_TERMS: &[&str] = &[
    // The host's own name and its sanctioned short name.
    "conspectus",
    "csp",
    // Objects in the host's work graph.
    "session",
    "pin",
    "pinned",
    "worktree",
    "checkout",
    "repo",
    "pr",
    "pull request",
    "forge",
    "graph node",
    "graph link",
    "transcript",
    // Multiplexers and orchestrators the host discovers.
    "mux",
    "tmux",
    "zellij",
    "atelier",
    // Agent harnesses and their products. "harness" also catches the pi
    // harness; bare "pi" is too ambiguous to list.
    "harness",
    "claude",
    "codex",
    "opencode",
    "aider",
];

/// What the process and write scan looks for, matched at a word start.
#[rustfmt::skip]
const CAPABILITY_NEEDLES: &[Needle] = &[
    Needle { text: "std::process", why: "starts or controls a process" },
    Needle { text: "process::Command", why: "starts a process" },
    Needle { text: "Command::new", why: "starts a process" },
    Needle { text: "CommandExt", why: "extends process spawning" },
    Needle { text: "File::create", why: "creates or truncates a file" },
    Needle { text: "File::options", why: "opens a file with write options" },
    Needle { text: "OpenOptions", why: "opens a file with write options" },
    Needle { text: "fs::write", why: "writes a file" },
    Needle { text: "fs::copy", why: "writes a file" },
    Needle { text: "fs::rename", why: "moves a file" },
    Needle { text: "fs::*", why: "imports every filesystem function, writes included" },
    Needle { text: "hard_link", why: "creates a file link" },
    Needle { text: "symlink", why: "creates a file link" },
    Needle { text: "set_permissions", why: "changes file permissions" },
    Needle { text: "remove_file", why: "deletes a file" },
    Needle { text: "remove_dir", why: "deletes a directory" },
    Needle { text: "create_dir", why: "creates a directory" },
];

/// Filesystem functions that are flagged when imported by name from a
/// `use std::fs::{...}` group (the full path form is a needle above).
const FS_WRITE_IMPORTS: &[&str] = &["write", "copy", "rename"];

// ---------------------------------------------------------------------------
// Guard 1: the manifest.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Runtime,
    Dev,
    Build,
}

/// Every spelling of a dependency table that Cargo accepts.
const DEPENDENCY_SECTIONS: &[(&str, Section)] = &[
    ("dependencies", Section::Runtime),
    ("dev-dependencies", Section::Dev),
    ("dev_dependencies", Section::Dev),
    ("build-dependencies", Section::Build),
    ("build_dependencies", Section::Build),
];

/// The host application, or any crate named after it.
fn is_host_name(name: &str) -> bool {
    name.to_ascii_lowercase()
        .replace('_', "-")
        .starts_with("conspectus")
}

/// Violations in one manifest's text. A manifest that does not parse is a
/// violation, not a panic.
fn manifest_violations(
    path: &str,
    text: &str,
    runtime: &[Allowed],
    dev: &[Allowed],
) -> Vec<String> {
    let manifest = match text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(error) => return vec![format!("{path}: does not parse as TOML: {error}")],
    };

    let mut tables: Vec<(String, Section, &toml::Table)> = Vec::new();
    for (name, section) in DEPENDENCY_SECTIONS {
        if let Some(table) = manifest.get(*name).and_then(toml::Value::as_table) {
            tables.push(((*name).to_owned(), *section, table));
        }
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (cfg, target) in targets {
            for (name, section) in DEPENDENCY_SECTIONS {
                if let Some(table) = target.get(*name).and_then(toml::Value::as_table) {
                    tables.push((format!("target.{cfg}.{name}"), *section, table));
                }
            }
        }
    }

    let own_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str);
    let mut problems = Vec::new();
    for (location, section, table) in tables {
        for (key, spec) in table {
            let package = spec
                .as_table()
                .and_then(|spec| spec.get("package"))
                .and_then(toml::Value::as_str)
                .unwrap_or(key.as_str());
            if section == Section::Dev && is_own_feature_dependency(own_name, package, spec) {
                continue;
            }
            if is_host_name(key) || is_host_name(package) {
                problems.push(format!(
                    "{path}: [{location}] `{key}` depends on the host application"
                ));
                continue;
            }
            let allowed = match section {
                Section::Runtime => runtime.iter().any(|entry| entry.name == package),
                Section::Dev => runtime.iter().chain(dev).any(|entry| entry.name == package),
                Section::Build => false,
            };
            if !allowed {
                problems.push(format!(
                    "{path}: [{location}] `{key}` (package `{package}`) is not on the \
                     allowlist for this table"
                ));
            }
        }
    }
    problems
}

/// Whether `spec` (named `package`) is the manifest's own crate, by path
/// `.`: the dev-dependency a crate declares on itself to turn its own
/// features on for its integration tests and doctests. It adds no package,
/// so it needs no allowlist entry.
fn is_own_feature_dependency(own_name: Option<&str>, package: &str, spec: &toml::Value) -> bool {
    own_name == Some(package)
        && spec
            .as_table()
            .and_then(|spec| spec.get("path"))
            .and_then(toml::Value::as_str)
            .is_some_and(|path| path == ".")
}

/// Checks every `Cargo.toml` in `sources`.
fn manifest_scan(sources: &[Source], runtime: &[Allowed], dev: &[Allowed]) -> Scan {
    let manifests: Vec<&Source> = sources
        .iter()
        .filter(|source| source.path.rsplit('/').next() == Some("Cargo.toml"))
        .collect();
    Scan {
        visited: manifests.len(),
        violations: manifests
            .iter()
            .flat_map(|source| manifest_violations(&source.path, &source.text, runtime, dev))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Guard 2: the vocabulary.
// ---------------------------------------------------------------------------

/// Splits one alphanumeric token at `camelCase` boundaries. An acronym stays
/// whole (`HTTPServer` is `HTTP`, `Server`) and so does an acronym's plural
/// (`PRs`).
fn split_identifier(token: &str) -> Vec<String> {
    let chars: Vec<char> = token.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0;
    for index in 1..chars.len() {
        let (previous, current) = (chars[index - 1], chars[index]);
        let next = chars.get(index + 1).copied();
        let after = chars.get(index + 2).copied();
        let word_start =
            (previous.is_lowercase() || previous.is_numeric()) && current.is_uppercase();
        let acronym_end = previous.is_uppercase()
            && current.is_uppercase()
            && next.is_some_and(char::is_lowercase)
            && !(next == Some('s') && after.is_none_or(|c| !c.is_lowercase()));
        if word_start || acronym_end {
            parts.push(chars[start..index].iter().collect());
            start = index;
        }
    }
    parts.push(chars[start..].iter().collect());
    parts
}

/// Lower-case words of `text` with their 1-based line numbers.
fn words(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for token in line
            .split(|c: char| !c.is_alphanumeric())
            .filter(|token| !token.is_empty())
        {
            for part in split_identifier(token) {
                out.push((index + 1, part.to_lowercase()));
            }
        }
    }
    out
}

fn plural(word: &str) -> String {
    if word.ends_with(['s', 'x', 'z']) || word.ends_with("ch") || word.ends_with("sh") {
        format!("{word}es")
    } else {
        format!("{word}s")
    }
}

/// Whether `terms` (lower case) match the words at the front of `stream`,
/// the last term word also matching in its plural form.
fn phrase_at(stream: &[(usize, String)], terms: &[String]) -> bool {
    let Some((last, head)) = terms.split_last() else {
        return false;
    };
    if stream.len() <= head.len() {
        return false;
    }
    let head_matches = head
        .iter()
        .zip(stream)
        .all(|(term, (_, word))| term == word);
    let word = &stream[head.len()].1;
    head_matches && (word == last || *word == plural(last))
}

/// Each `(line, term)` where `text` uses a listed word, once per line.
fn term_hits(text: &str, listed: &[&'static str]) -> Vec<(usize, &'static str)> {
    let compiled: Vec<(&'static str, Vec<String>)> = listed
        .iter()
        .map(|term| {
            (
                *term,
                term.split_whitespace().map(str::to_lowercase).collect(),
            )
        })
        .collect();
    let stream = words(text);
    let mut hits = Vec::new();
    for start in 0..stream.len() {
        for (term, parts) in &compiled {
            if phrase_at(&stream[start..], parts) {
                hits.push((stream[start].0, *term));
            }
        }
    }
    hits.sort_unstable();
    hits.dedup();
    hits
}

/// Scans every file except `GUARD_FILE` for the listed words.
fn vocabulary_scan(sources: &[Source], listed: &[&'static str], exceptions: &[Exception]) -> Scan {
    let mut used = vec![false; exceptions.len()];
    let mut violations = Vec::new();
    let mut visited = 0;
    for source in sources.iter().filter(|source| source.path != GUARD_FILE) {
        visited += 1;
        for (line, term) in term_hits(&source.text, listed) {
            let excused = exceptions
                .iter()
                .position(|entry| entry.path == source.path && entry.term == term);
            if let Some(index) = excused {
                used[index] = true;
            } else {
                violations.push(format!(
                    "{}:{line}: uses the host-domain word `{term}`",
                    source.path
                ));
            }
        }
    }
    violations.extend(stale_exceptions("vocabulary", exceptions, &used));
    Scan {
        visited,
        violations,
    }
}

// ---------------------------------------------------------------------------
// Guard 3: no processes, no writes.
// ---------------------------------------------------------------------------

/// Each `(line, needle)` where sanitized, test-free source uses a process or
/// write API.
fn capability_hits(code: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    for (index, line) in code.lines().enumerate() {
        for needle in CAPABILITY_NEEDLES {
            if contains_at_word_start(line, needle.text) {
                hits.push((index + 1, needle.text.to_owned()));
            }
        }
    }
    // `use std::fs::{self, write}` names the function without `fs::` in front.
    for (start, _) in code.match_indices("fs::{") {
        let group_start = start + "fs::{".len();
        let group = code[group_start..].split('}').next().unwrap_or_default();
        let line = code[..start].matches('\n').count() + 1;
        for item in group.split(|c: char| !is_identifier_char(c)) {
            if FS_WRITE_IMPORTS.contains(&item) {
                hits.push((line, format!("fs::{item}")));
            }
        }
    }
    hits.sort_unstable();
    hits.dedup();
    hits
}

/// Scans the non-test Rust source in `sources` for process and write APIs.
fn capability_scan(sources: &[Source], exceptions: &[Exception]) -> Scan {
    let mut used = vec![false; exceptions.len()];
    let mut violations = Vec::new();
    let mut visited = 0;
    for source in sources
        .iter()
        .filter(|source| is_rust_file(&source.path) && !is_test_path(&source.path))
    {
        visited += 1;
        let code = strip_test_items(&sanitize_rust(&source.text));
        for (line, needle) in capability_hits(&code) {
            let excused = exceptions
                .iter()
                .position(|entry| entry.path == source.path && entry.term == needle);
            if let Some(index) = excused {
                used[index] = true;
            } else {
                let why = CAPABILITY_NEEDLES
                    .iter()
                    .find(|candidate| candidate.text == needle)
                    .map_or("writes a file", |candidate| candidate.why);
                violations.push(format!(
                    "{}:{line}: `{needle}` {why}; the framework starts no process and \
                     writes no file",
                    source.path
                ));
            }
        }
    }
    violations.extend(stale_exceptions("process and write", exceptions, &used));
    Scan {
        visited,
        violations,
    }
}

// ---------------------------------------------------------------------------
// The guards, run on the real tree.
// ---------------------------------------------------------------------------

#[test]
fn manifest_names_no_host_crate_and_only_allowlisted_dependencies() {
    let scan = manifest_scan(
        &load_sources(crate_root()),
        RUNTIME_DEPENDENCIES,
        DEV_DEPENDENCIES,
    );
    assert_clean("manifest", &scan, MIN_MANIFESTS);
}

#[test]
fn no_file_uses_host_domain_vocabulary() {
    let scan = vocabulary_scan(
        &load_sources(crate_root()),
        DOMAIN_TERMS,
        VOCABULARY_EXCEPTIONS,
    );
    assert_clean("vocabulary", &scan, MIN_VOCABULARY_FILES);
}

#[test]
fn non_test_source_starts_no_process_and_writes_no_file() {
    let scan = capability_scan(&load_sources(crate_root()), CAPABILITY_EXCEPTIONS);
    assert_clean("process and write", &scan, MIN_CAPABILITY_FILES);
}

// ---------------------------------------------------------------------------
// Self-tests: each guard must fail on a leak fed to it from memory.
// ---------------------------------------------------------------------------

const CLEAN_MANIFEST: &str = r#"
[package]
name = "tuikit"

[dependencies]
ratatui = { workspace = true }
unicode-width.workspace = true

[dev-dependencies]
toml = { workspace = true }
ratatui = { workspace = true }
"#;

fn manifest_problems(text: &str) -> Vec<String> {
    manifest_violations("Cargo.toml", text, RUNTIME_DEPENDENCIES, DEV_DEPENDENCIES)
}

#[test]
fn manifest_guard_passes_an_allowed_manifest() {
    assert_eq!(manifest_problems(CLEAN_MANIFEST), Vec::<String>::new());
}

#[test]
fn manifest_guard_rejects_the_host_crate_in_every_spelling() {
    let leaks = [
        "[dependencies]\nconspectus = { path = \"../..\" }\n",
        "[dev-dependencies]\nconspectus = { workspace = true }\n",
        "[dependencies]\nconspectus-viewer = \"0.1\"\n",
        "[dependencies]\nconspectus_core = \"0.1\"\n",
        "[dependencies]\nratatui = { package = \"conspectus\", path = \"../..\" }\n",
        "[dependencies.conspectus]\npath = \"../..\"\n",
        "[target.'cfg(unix)'.dependencies]\nconspectus = \"0.1\"\n",
        "[build-dependencies]\nconspectus = \"0.1\"\n",
    ];
    for leak in leaks {
        let problems = manifest_problems(leak);
        assert!(
            problems.iter().any(|p| p.contains("host application")),
            "no host-dependency violation for:\n{leak}\ngot: {problems:?}"
        );
    }
}

#[test]
fn manifest_guard_rejects_dependencies_outside_the_allowlist() {
    let leaks = [
        "[dependencies]\nserde = \"1\"\n",
        "[dependencies.serde]\nversion = \"1\"\n",
        "[dependencies]\nserde.workspace = true\n",
        "[dependencies]\nserde = { package = \"ratatui2\", version = \"1\" }\n",
        // Dev-only crates may not be runtime dependencies.
        "[dependencies]\ntoml = \"0.8\"\n",
        "[dev-dependencies]\nserde = \"1\"\n",
        "[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
        "[target.'cfg(unix)'.dev-dependencies]\nlibc = \"0.2\"\n",
        "[build-dependencies]\nratatui = \"0.30\"\n",
        "[dev_dependencies]\nserde = \"1\"\n",
    ];
    for leak in leaks {
        let problems = manifest_problems(leak);
        assert!(
            problems.iter().any(|p| p.contains("not on the allowlist")),
            "no allowlist violation for:\n{leak}\ngot: {problems:?}"
        );
    }
}

#[test]
fn manifest_guard_accepts_the_crate_turning_on_its_own_features() {
    let own = r#"
[package]
name = "tuikit"

[dev-dependencies]
tuikit = { path = ".", features = ["testkit"] }
"#;
    assert_eq!(manifest_problems(own), Vec::<String>::new());
    // Only as a dev-dependency, only by its own path, and only under its
    // own name.
    let leaks = [
        "[package]\nname = \"tuikit\"\n[dependencies]\ntuikit = { path = \".\" }\n",
        "[package]\nname = \"tuikit\"\n[dev-dependencies]\ntuikit = { path = \"../fork\" }\n",
        "[package]\nname = \"tuikit\"\n[dev-dependencies]\ntuikit = \"0.1\"\n",
        "[package]\nname = \"other\"\n[dev-dependencies]\ntuikit = { path = \".\" }\n",
    ];
    for leak in leaks {
        assert!(
            manifest_problems(leak)
                .iter()
                .any(|p| p.contains("not on the allowlist")),
            "no allowlist violation for:\n{leak}"
        );
    }
}

#[test]
fn manifest_guard_accepts_a_renamed_allowlisted_package() {
    let text = "[dependencies]\nterm = { package = \"ratatui\", version = \"0.30\" }\n";
    assert_eq!(manifest_problems(text), Vec::<String>::new());
}

#[test]
fn manifest_guard_reports_an_unparseable_manifest() {
    let problems = manifest_problems("[dependencies\nratatui = ");
    assert!(problems.iter().any(|p| p.contains("does not parse")));
}

#[test]
fn manifest_scan_checks_nested_manifests_and_counts_them() {
    let sources = [
        Source::new("Cargo.toml", CLEAN_MANIFEST),
        Source::new(
            "examples/demo/Cargo.toml",
            "[dependencies]\nserde = \"1\"\n",
        ),
        Source::new("src/lib.rs", "// not a manifest\n"),
    ];
    let scan = manifest_scan(&sources, RUNTIME_DEPENDENCIES, DEV_DEPENDENCIES);
    assert_eq!(scan.visited, 2);
    assert_eq!(scan.violations.len(), 1);
    assert!(scan.violations[0].starts_with("examples/demo/Cargo.toml"));
}

fn vocabulary_problems(path: &str, text: &str) -> Vec<String> {
    vocabulary_scan(&[Source::new(path, text)], DOMAIN_TERMS, &[]).violations
}

#[test]
fn vocabulary_guard_fails_on_every_listed_word() {
    for term in DOMAIN_TERMS {
        let leak = format!("//! Shows the {term} list.\n");
        assert!(
            !vocabulary_problems("src/lib.rs", &leak).is_empty(),
            "`{term}` did not trip the vocabulary guard"
        );
    }
}

#[test]
fn vocabulary_guard_matches_identifier_prose_and_plural_forms() {
    let leaks = [
        "let session = 1;",
        "struct SessionId;",
        "const MAX_SESSIONS: u8 = 3;",
        "fn pin_row() {}",
        "// Pins stay on top.",
        "// the MUXES",
        "let tmux_pane = 0;",
        "enum PrState {}",
        "struct PRList;",
        "// two PRs open",
        "// a pull request",
        "struct PullRequest;",
        "let pull_request = 0;",
        "// a pull\n// request wraps",
        "struct GraphNode;",
        "// the graph\n// nodes",
        "// two worktrees",
        "// the Harnesses",
        "// ask Claude",
        "// see CSP-720",
        "// conspectus-viewer",
        "kebab-case: opencode",
    ];
    for leak in leaks {
        assert!(
            !vocabulary_problems("src/lib.rs", leak).is_empty(),
            "no vocabulary violation for: {leak}"
        );
    }
}

#[test]
fn vocabulary_guard_ignores_look_alikes() {
    let clean = "// an obsession with spinners, pinch zoom, prefix and expression \
                 handling, a repository, a checkpoint, muxed streams, graphs and nodes, \
                 a pull of the request line, process, Print, PRINT_ALL, notice";
    assert_eq!(
        vocabulary_problems("src/lib.rs", clean),
        Vec::<String>::new()
    );
}

#[test]
fn vocabulary_guard_reports_the_file_line_and_word() {
    let problems = vocabulary_problems("docs/a.md", "fine\nstill fine\nthe session\n");
    assert_eq!(
        problems,
        ["docs/a.md:3: uses the host-domain word `session`"]
    );
}

#[test]
fn vocabulary_scan_skips_only_the_guard_file_by_name() {
    let leak = "// session\n";
    let sources = [
        Source::new(GUARD_FILE, leak),
        Source::new("tests/independence_extra.rs", leak),
        Source::new("tests/other/independence.rs", leak),
    ];
    let scan = vocabulary_scan(&sources, DOMAIN_TERMS, &[]);
    assert_eq!(scan.visited, 2, "the guard file is excluded from the count");
    assert_eq!(scan.violations.len(), 2);
    assert!(scan.violations.iter().all(|p| !p.starts_with(GUARD_FILE)));
}

#[test]
fn vocabulary_exceptions_excuse_one_file_and_one_word() {
    let exception = [Exception {
        path: "src/lib.rs",
        term: "session",
        reason: "test",
    }];
    let sources = [
        Source::new("src/lib.rs", "// session and pin\n"),
        Source::new("src/other.rs", "// session\n"),
    ];
    let scan = vocabulary_scan(&sources, DOMAIN_TERMS, &exception);
    assert_eq!(
        scan.violations,
        [
            "src/lib.rs:1: uses the host-domain word `pin`",
            "src/other.rs:1: uses the host-domain word `session`",
        ]
    );
}

#[test]
fn vocabulary_scan_fails_on_a_stale_exception() {
    let exception = [Exception {
        path: "src/lib.rs",
        term: "session",
        reason: "was needed once",
    }];
    let sources = [Source::new("src/lib.rs", "// clean now\n")];
    let scan = vocabulary_scan(&sources, DOMAIN_TERMS, &exception);
    assert_eq!(scan.violations.len(), 1);
    assert!(scan.violations[0].contains("stale"));
    assert!(scan.violations[0].contains("was needed once"));
}

fn capability_problems(path: &str, text: &str) -> Vec<String> {
    capability_scan(&[Source::new(path, text)], &[]).violations
}

#[test]
fn capability_guard_fails_on_every_needle() {
    for needle in CAPABILITY_NEEDLES {
        let leak = format!("fn leak() {{ let _ = {}; }}\n", needle.text);
        assert!(
            !capability_problems("src/lib.rs", &leak).is_empty(),
            "`{}` did not trip the process and write guard",
            needle.text
        );
    }
}

#[test]
fn capability_guard_catches_realistic_leaks() {
    let leaks = [
        "use std::process::Command;",
        "let out = std::process::Command::new(\"ls\").output();",
        "let child = Command::new(\"git\").spawn();",
        "let f = std::fs::File::create(path)?;",
        "let f = File::create_new(path)?;",
        "let f = OpenOptions::new().append(true).open(path)?;",
        "let f = File::options().write(true).open(path)?;",
        "std::fs::write(path, bytes)?;",
        "fs::write(path, bytes)?;",
        "use std::fs::write;",
        "use std::fs::{self, write};",
        "use std::fs::{\n    self,\n    copy,\n};",
        "use std::fs::rename;",
        "use std::fs::*;",
        "std::fs::remove_file(path)?;",
        "std::fs::remove_dir_all(path)?;",
        "std::fs::create_dir_all(path)?;",
        "std::os::unix::fs::symlink(a, b)?;",
        "fs::hard_link(a, b)?;",
        "fs::set_permissions(path, perms)?;",
        "use std::os::unix::process::CommandExt;",
    ];
    for leak in leaks {
        assert!(
            !capability_problems("src/lib.rs", leak).is_empty(),
            "no process or write violation for: {leak}"
        );
    }
}

#[test]
fn capability_guard_allows_read_only_filesystem_access() {
    let clean = "
use std::fs::{self, File};
use std::io::{Read, Write};
fn read(path: &std::path::Path, sink: &mut dyn Write) -> std::io::Result<()> {
    let mut text = String::new();
    File::open(path)?.read_to_string(&mut text)?;
    let _ = fs::read(path)?;
    let _ = fs::read_to_string(path)?;
    for entry in fs::read_dir(path)? {
        let _ = entry?.metadata()?;
    }
    let _ = fs::metadata(path)?;
    let _ = fs::canonicalize(path)?;
    let _ = path.exists();
    sink.write_all(text.as_bytes())
}
";
    assert_eq!(
        capability_problems("src/lib.rs", clean),
        Vec::<String>::new()
    );
}

#[test]
fn capability_guard_ignores_comments_and_literals() {
    let clean = r##"
// std::process::Command::new("ls") in a line comment
/// File::create in a doc comment
/* fs::write in a /* nested */ block comment */
fn text() -> (&'static str, &'static str, char, char) {
    let _ = "std::process::Command";
    let _ = "an escaped \" quote then File::create";
    let _ = b"fs::remove_file";
    let _ = r#"OpenOptions "quoted" create_dir"#;
    ("fs::write", r"remove_dir", '\'', 'x')
}
"##;
    assert_eq!(
        capability_problems("src/lib.rs", clean),
        Vec::<String>::new()
    );
}

#[test]
fn capability_guard_still_sees_code_after_a_literal() {
    let leak = "fn f<'a>(x: &'a str) { let _ = \"ok\"; std::fs::remove_file(x); }\n";
    assert_eq!(capability_problems("src/lib.rs", leak).len(), 1);
}

#[test]
fn capability_guard_reports_the_file_line_and_api() {
    let problems = capability_problems("src/save.rs", "fn a() {}\nfn b() { fs::write(p, d); }\n");
    assert_eq!(
        problems,
        [
            "src/save.rs:2: `fs::write` writes a file; the framework starts no process and writes no file"
        ]
    );
}

#[test]
fn capability_guard_matches_at_a_word_start() {
    let clean = "fn f() { let _ = try_remove_file_name; let _ = not_symlink_free; }\n";
    assert_eq!(
        capability_problems("src/lib.rs", clean),
        Vec::<String>::new()
    );
}

#[test]
fn capability_guard_skips_test_code_but_not_code_after_it() {
    let source = "
fn live() {}

#[cfg(test)]
mod tests {
    use std::fs::File;
    fn fixture() { File::create(\"x\").unwrap(); let _ = \"}\"; }
}

#[cfg(test)]
#[path = \"live_tests.rs\"]
mod more_tests;

#[cfg(test)]
fn helper(buffer: [u8; 4]) { std::fs::write(\"p\", buffer).unwrap(); }

fn after() { std::fs::remove_file(\"p\").ok(); }
";
    let problems = capability_problems("src/lib.rs", source);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("src/lib.rs:17:"), "{problems:?}");
}

#[test]
fn capability_scan_skips_test_files_and_non_rust_files() {
    let leak = "fn f() { std::fs::write(p, d); }\n";
    let sources = [
        Source::new("src/save_tests.rs", leak),
        Source::new("src/tests.rs", leak),
        Source::new("src/widgets/tests/mod.rs", leak),
        Source::new("tests/smoke.rs", leak),
        Source::new("docs/note.md", leak),
        Source::new("src/save.rs", leak),
    ];
    let scan = capability_scan(&sources, &[]);
    assert_eq!(scan.visited, 1);
    assert_eq!(scan.violations.len(), 1);
    assert!(scan.violations[0].starts_with("src/save.rs:1:"));
}

#[test]
fn capability_exceptions_excuse_one_file_and_one_api_and_go_stale() {
    let exception = [Exception {
        path: "src/lib.rs",
        term: "fs::write",
        reason: "test",
    }];
    let sources = [
        Source::new(
            "src/lib.rs",
            "fn f() { fs::write(p, d); fs::copy(a, b); }\n",
        ),
        Source::new("src/other.rs", "fn f() { fs::write(p, d); }\n"),
    ];
    let scan = capability_scan(&sources, &exception);
    assert_eq!(scan.violations.len(), 2, "{:?}", scan.violations);
    assert!(
        scan.violations
            .iter()
            .any(|p| p.starts_with("src/lib.rs:1: `fs::copy`"))
    );
    assert!(
        scan.violations
            .iter()
            .any(|p| p.starts_with("src/other.rs:1: `fs::write`"))
    );

    let stale = capability_scan(&[Source::new("src/lib.rs", "fn f() {}\n")], &exception);
    assert_eq!(stale.violations.len(), 1);
    assert!(stale.violations[0].contains("stale"));
}

#[test]
fn sanitizer_blanks_literals_and_comments_but_keeps_lines() {
    let text = "let a = \"xy\nz\"; // note\nlet b = 'q'; /* inner\nmore */ let c = 1;\n";
    let clean = sanitize_rust(text);
    assert_eq!(clean.lines().count(), text.lines().count());
    for gone in ["xy", "note", "'q'", "inner", "more"] {
        assert!(!clean.contains(gone), "`{gone}` survived in {clean:?}");
    }
    assert!(clean.contains("let a ="));
    assert!(clean.contains("let c = 1;"));
}

#[test]
fn tripwire_fails_below_the_minimum_and_passes_at_it() {
    assert!(tripwire("demo", 0, 1).is_some());
    assert!(tripwire("demo", 2, 3).is_some());
    assert!(tripwire("demo", 1, 1).is_none());
    assert!(tripwire("demo", 5, 1).is_none());
    let message = tripwire("manifest", 0, 1).expect("trips");
    assert!(message.contains("manifest") && message.contains("minimum of 1"));
}

#[test]
fn every_scan_over_nothing_trips_its_tripwire() {
    let none: [Source; 0] = [];
    let only_guard = [Source::new(GUARD_FILE, "// session\n")];
    assert!(
        tripwire(
            "manifest",
            manifest_scan(&none, &[], &[]).visited,
            MIN_MANIFESTS
        )
        .is_some()
    );
    assert!(
        tripwire(
            "vocabulary",
            vocabulary_scan(&none, DOMAIN_TERMS, &[]).visited,
            MIN_VOCABULARY_FILES
        )
        .is_some()
    );
    assert!(
        tripwire(
            "vocabulary",
            vocabulary_scan(&only_guard, DOMAIN_TERMS, &[]).visited,
            MIN_VOCABULARY_FILES
        )
        .is_some()
    );
    assert!(
        tripwire(
            "process and write",
            capability_scan(&none, &[]).visited,
            MIN_CAPABILITY_FILES
        )
        .is_some()
    );
}

#[test]
fn the_verdict_fails_on_a_violation_and_on_an_empty_scan() {
    let leak = Scan {
        visited: 3,
        violations: vec!["a.rs:1: leak".to_owned()],
    };
    let report = verdict("demo", &leak, 1).expect_err("a violation must fail the guard");
    assert!(report.contains("a.rs:1: leak") && report.contains("docs/contributing.md"));

    let empty = Scan {
        visited: 0,
        violations: Vec::new(),
    };
    let report = verdict("demo", &empty, 1).expect_err("an empty scan must fail the guard");
    assert!(report.contains("minimum of 1"));

    let clean = Scan {
        visited: 1,
        violations: Vec::new(),
    };
    assert_eq!(verdict("demo", &clean, 1), Ok(()));
}

// ---------------------------------------------------------------------------
// Checks on the policy constants themselves.
// ---------------------------------------------------------------------------

#[test]
fn guard_file_constant_names_this_file() {
    assert!(
        Path::new(file!()).ends_with(GUARD_FILE),
        "GUARD_FILE is `{GUARD_FILE}` but this file is `{}`",
        file!()
    );
}

#[test]
fn the_real_tree_is_found_where_the_scans_expect_it() {
    let sources = load_sources(crate_root());
    for expected in ["Cargo.toml", "src/lib.rs", GUARD_FILE] {
        assert!(
            sources.iter().any(|source| source.path == expected),
            "the walk did not find `{expected}`"
        );
    }
    assert!(
        sources
            .iter()
            .all(|source| !source.path.starts_with("target/")),
        "the walk must skip build output"
    );
}

#[test]
fn every_allowlist_entry_states_a_reason_and_none_repeats() {
    let mut names = Vec::new();
    for entry in RUNTIME_DEPENDENCIES.iter().chain(DEV_DEPENDENCIES) {
        assert!(
            !entry.reason.trim().is_empty(),
            "`{}` has no reason",
            entry.name
        );
        assert!(!is_host_name(entry.name), "the host cannot be allowlisted");
        names.push(entry.name);
    }
    for entry in VOCABULARY_EXCEPTIONS.iter().chain(CAPABILITY_EXCEPTIONS) {
        assert!(
            !entry.reason.trim().is_empty(),
            "the exception for `{}` in {} has no reason",
            entry.term,
            entry.path
        );
    }
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total, "a dependency is allowlisted twice");
}

#[test]
fn every_vocabulary_exception_names_a_listed_word_and_every_capability_exception_a_needle() {
    for entry in VOCABULARY_EXCEPTIONS {
        assert!(
            DOMAIN_TERMS.contains(&entry.term),
            "`{}` is not in DOMAIN_TERMS",
            entry.term
        );
    }
    for entry in CAPABILITY_EXCEPTIONS {
        let known = CAPABILITY_NEEDLES
            .iter()
            .any(|needle| needle.text == entry.term)
            || FS_WRITE_IMPORTS
                .iter()
                .any(|name| entry.term == format!("fs::{name}"));
        assert!(known, "`{}` is not a process or write needle", entry.term);
    }
}

#[test]
fn domain_terms_are_lower_case_singular_and_unique() {
    let mut seen = Vec::new();
    for term in DOMAIN_TERMS {
        assert_eq!(*term, term.to_lowercase(), "`{term}` must be lower case");
        assert_eq!(
            *term,
            term.split_whitespace().collect::<Vec<_>>().join(" "),
            "`{term}` has stray spaces"
        );
        assert!(!seen.contains(term), "`{term}` is listed twice");
        seen.push(*term);
    }
}
