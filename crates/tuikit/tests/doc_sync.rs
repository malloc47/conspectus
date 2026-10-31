//! The documentation stays in step with the code.
//!
//! Every public module of the crate has a reference page under
//! `docs/controls/`, built from `docs/controls/_template.md`, and the pages,
//! the module map in `docs/architecture.md`, and the links between the
//! pages are held to the code here. The tests fail when:
//!
//! - a public module (a `pub mod` line in `src/lib.rs`, feature-gated ones
//!   included) has no page, or a page names a module that is not public, or
//!   two pages claim one module. A page names its module first in the
//!   `Module` row of its header table (`tuikit::tree` for `tree_view.md`);
//! - a page lacks a row of the template's header table or one of the
//!   template's `##` sections, in the template's order, or its usage example
//!   has no `rust` block;
//! - the page of a module behind a cargo feature does not name the feature
//!   in its `Cargo feature` row, or the page of an ungated module names one;
//! - a page's "House rules followed" table numbers a rule under another
//!   topic than `docs/house-rules.md` gives it;
//! - a page under `docs/` holds a `rust` block but is not included by
//!   `src/doc_pages.rs` (whose includes make those blocks doctests, so they
//!   compile and run), or an include names a page that is gone;
//! - the module map does not list a public module or link its page, or
//!   lists as a module something that is not one;
//! - a relative link in the README or a page names a file that does not
//!   exist, or a heading the file does not have;
//! - the decision log's index (`docs/decisions/README.md`) and its records
//!   disagree: a record without a row, a row without a record, or a row
//!   whose number, title, status, or origins differ from its record's;
//! - a record lacks its title, its status, one of the record sections in
//!   order, or an `## Origin` section that names its host records
//!   (`- host ADR NNNN ...` items) or says it has none (`None: ...`).
//!
//! While the crate lives in its host's repository the log is also held to
//! the host's decision records (`../../docs/adr` from the crate root): every
//! record number the log names exists there, the host's index marks exactly
//! the records the log restates as framework-owned, and every record in the
//! index's Interactive TUI section is either restated or listed as left out.
//! The host is taken to be present when the manifest two directories up
//! names this crate as a workspace member. When the crate stands alone
//! there is no such manifest, so those checks print a note and pass: the
//! log is then canonical and the host numbers are history.
//!
//! Each check is a pure function over text, so the self-tests at the end
//! feed it a broken page or module list from memory and see it fail; no
//! broken fixture sits on disk. The contribution guide describes the checks
//! (`docs/contributing.md`, "The pages and the code stay in step").

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use tuikit::testkit::source;

/// The page template, relative to the crate root.
const TEMPLATE: &str = "docs/controls/_template.md";
/// Where the reference pages live.
const CONTROLS: &str = "docs/controls/";
/// The module map's page and its section.
const ARCHITECTURE: &str = "docs/architecture.md";
const MODULE_MAP: &str = "## Module map";
/// The house rules' page.
const HOUSE_RULES: &str = "docs/house-rules.md";
/// The file whose includes make the pages doctests.
const DOC_PAGES: &str = "src/doc_pages.rs";
/// Fewest reference pages the checks may find: a tripwire, so a check
/// pointed at the wrong directory fails instead of passing empty. There
/// are 36 today.
const MIN_PAGES: usize = 27;
/// The decision log: its directory and its index.
const DECISIONS: &str = "docs/decisions/";
const DECISIONS_INDEX: &str = "docs/decisions/README.md";
/// The `##` sections every record keeps, in this order.
const RECORD_SECTIONS: &[&str] = &[
    "## Decision",
    "## Rationale",
    "## Consequences",
    "## Alternatives considered",
    "## Origin",
];
/// The section of the record that names where the decision came from.
const ORIGIN: &str = "## Origin";
/// The index's section that lists the host records the log leaves out.
const NOT_INCLUDED: &str = "## Host records not included";
/// Fewest records the checks may find, a tripwire like `MIN_PAGES`. There
/// are 71 today.
const MIN_RECORDS: usize = 52;
/// The host repository, relative to the crate root: the manifest that names
/// this crate as a member, the member path it names, and the host's
/// decision records.
const HOST_MANIFEST: &str = "../../Cargo.toml";
const HOST_MEMBER: &str = "\"crates/tuikit\"";
const HOST_RECORDS: &str = "../../docs/adr";
/// What the host index's status cell says of a record the log restates.
const OWNED_MARK: &str = "framework-owned";
/// The host index's section every record of which the log accounts for.
const HOST_SECTION: &str = "## Interactive TUI";

// ---------------------------------------------------------------------------
// Reading the tree.
// ---------------------------------------------------------------------------

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// A crate-relative path with forward slashes.
fn relative(path: &Path) -> String {
    let rest = path.strip_prefix(root()).unwrap_or(path);
    rest.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every Markdown file of the documentation: the README and everything
/// under `docs/`, as (crate-relative path, text).
fn markdown_files() -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = vec![root().join("README.md")];
    files.extend(
        source::files(&root().join("docs"))
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "md")),
    );
    files
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            (relative(path), text)
        })
        .collect()
}

/// The reference pages: every page under `docs/controls/` but the template.
fn reference_pages(markdown: &[(String, String)]) -> Vec<Page> {
    markdown
        .iter()
        .filter(|(path, _)| path.starts_with(CONTROLS) && path != TEMPLATE)
        .map(|(path, text)| Page::parse(path, text))
        .collect()
}

// ---------------------------------------------------------------------------
// Parsing.
// ---------------------------------------------------------------------------

/// The public modules `lib.rs` declares, each with the cargo feature that
/// gates it, if any.
fn public_modules(lib: &str) -> BTreeMap<String, Option<String>> {
    let mut modules = BTreeMap::new();
    let mut gate: Option<String> = None;
    for line in lib.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[cfg(") {
            gate = feature_of(line);
            continue;
        }
        if line.starts_with("#[") || line.starts_with("#![") {
            continue;
        }
        if let Some(name) = line
            .strip_prefix("pub mod ")
            .and_then(|rest| rest.strip_suffix(';'))
        {
            modules.insert(name.trim().to_string(), gate.take());
        }
        gate = None;
    }
    modules
}

/// The feature a `#[cfg(...)]` line names, if it names one.
fn feature_of(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("feature = \"")?;
    let (feature, _) = rest.split_once('"')?;
    Some(feature.to_string())
}

/// Every `tuikit::name` a piece of text names, by its first path segment.
fn module_paths(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("tuikit::") {
        rest = &rest[at + "tuikit::".len()..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
    }
    found
}

/// The text of a page with its fenced blocks blanked, so a heading, a
/// table, or a link inside an example is not read as the page's own.
fn outside_fences(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            lines.push("");
            continue;
        }
        lines.push(if fenced { "" } else { line });
    }
    lines
}

/// The language tags of a text's fenced blocks, each with the `##` section
/// it sits in.
fn fences(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut fenced = false;
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !fenced && line.starts_with("## ") {
            section = line.trim_end().to_string();
        }
        if let Some(tag) = trimmed.strip_prefix("```") {
            if !fenced {
                found.push((tag.trim().to_string(), section.clone()));
            }
            fenced = !fenced;
        }
    }
    found
}

/// The cells of a Markdown table row, trimmed; `None` for a line that is
/// not a row. An escaped bar (`\|`) stays inside its cell.
fn cells(line: &str) -> Option<Vec<String>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    out.push(cell.trim().to_string());
    Some(out)
}

/// What a check reads of one page.
struct Page {
    path: String,
    title: Option<String>,
    /// The header table's rows, in order: the row name and its value.
    header: Vec<(String, String)>,
    /// The `##` headings, in order.
    sections: Vec<String>,
    /// Fenced blocks: the language tag and the `##` section holding them.
    fences: Vec<(String, String)>,
    /// The "House rules followed" table: rule number and topic.
    rules: Vec<(u32, String)>,
}

impl Page {
    fn parse(path: &str, text: &str) -> Self {
        let lines = outside_fences(text);
        let title = lines
            .first()
            .and_then(|line| line.strip_prefix("# "))
            .map(str::to_string);
        let mut header = Vec::new();
        let mut in_header = false;
        for line in &lines {
            if line.starts_with("## ") {
                break;
            }
            match cells(line) {
                Some(row) if row.iter().all(|cell| cell.chars().all(|c| c == '-')) => {
                    in_header = true;
                }
                Some(row) if in_header && row.len() >= 2 => {
                    header.push((row[0].clone(), row[1..].join(" | ")));
                }
                Some(_) => {}
                None if in_header => break,
                None => {}
            }
        }
        let sections = lines
            .iter()
            .filter(|line| line.starts_with("## "))
            .map(|line| line.trim_end().to_string())
            .collect();
        let mut rules = Vec::new();
        let mut in_rules = false;
        for line in &lines {
            if line.starts_with("## ") {
                in_rules = line.trim_end() == "## House rules followed";
                continue;
            }
            if !in_rules {
                continue;
            }
            if let Some(row) = cells(line)
                && let (Some(number), Some(topic)) = (row.first(), row.get(1))
                && let Ok(number) = number.parse::<u32>()
            {
                rules.push((number, topic.clone()));
            }
        }
        Self {
            path: path.to_string(),
            title,
            header,
            sections,
            fences: fences(text),
            rules,
        }
    }

    /// The value of a header row.
    fn row(&self, name: &str) -> Option<&str> {
        self.header
            .iter()
            .find(|(row, _)| row == name)
            .map(|(_, value)| value.as_str())
    }

    /// The modules the `Module` row names, the page's own first.
    fn modules(&self) -> Vec<String> {
        self.row("Module").map(module_paths).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// The checks: each returns what is wrong, one line per problem.
// ---------------------------------------------------------------------------

/// A public module with no page, a page naming a module that is not
/// public, a page naming none, and two pages claiming one module.
fn coverage(modules: &BTreeMap<String, Option<String>>, pages: &[Page]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut owners: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for page in pages {
        let named = page.modules();
        let Some(own) = named.first() else {
            problems.push(format!(
                "{}: the `Module` row names no `tuikit::` module",
                page.path
            ));
            continue;
        };
        owners.entry(own.clone()).or_default().push(&page.path);
        for name in &named {
            if !modules.contains_key(name) {
                problems.push(format!(
                    "{}: names `tuikit::{name}`, which `src/lib.rs` does not declare public",
                    page.path
                ));
            }
        }
    }
    for module in modules.keys() {
        match owners.get(module).map(Vec::as_slice) {
            None | Some([]) => problems.push(format!(
                "`tuikit::{module}` has no reference page under {CONTROLS} \
                 (copy {TEMPLATE} and name the module in its `Module` row)"
            )),
            Some([_]) => {}
            Some(several) => problems.push(format!(
                "`tuikit::{module}` is claimed by several pages: {}",
                several.join(", ")
            )),
        }
    }
    problems
}

/// A page that lacks the template's title, header rows, or sections (in
/// the template's order), or a `rust` block in its usage example.
fn shape(template: &Page, page: &Page) -> Vec<String> {
    let mut problems = Vec::new();
    if page.title.is_none() {
        problems.push(format!("{}: should open with a `# ` title", page.path));
    }
    let mut at = 0;
    for (row, _) in &template.header {
        match page.header[at..].iter().position(|(name, _)| name == row) {
            Some(found) => at += found + 1,
            None => problems.push(format!(
                "{}: the header table lacks its `{row}` row (in the template's order)",
                page.path
            )),
        }
    }
    let mut at = 0;
    for heading in &template.sections {
        match page.sections[at..].iter().position(|name| name == heading) {
            Some(found) => at += found + 1,
            None => problems.push(format!(
                "{}: lacks `{heading}` (in the template's order)",
                page.path
            )),
        }
    }
    let example = "## Usage example";
    if !page
        .fences
        .iter()
        .any(|(tag, section)| tag == "rust" && section == example)
    {
        problems.push(format!(
            "{}: its usage example has no `rust` block to compile",
            page.path
        ));
    }
    problems
}

/// The page of a gated module that does not name its feature, and the
/// page of an ungated module that names one.
fn features(modules: &BTreeMap<String, Option<String>>, pages: &[Page]) -> Vec<String> {
    let mut problems = Vec::new();
    for page in pages {
        let Some(own) = page.modules().into_iter().next() else {
            continue;
        };
        let Some(gate) = modules.get(&own) else {
            continue;
        };
        let row = page.row("Cargo feature").unwrap_or_default();
        match gate {
            Some(feature) if !row.contains(&format!("`{feature}`")) => problems.push(format!(
                "{}: `tuikit::{own}` is behind the `{feature}` feature, which its \
                 `Cargo feature` row does not name",
                page.path
            )),
            None if !row.starts_with("none") => problems.push(format!(
                "{}: `tuikit::{own}` is behind no feature, but its `Cargo feature` row \
                 does not start with `none`",
                page.path
            )),
            _ => {}
        }
    }
    problems
}

/// The rule number and topic of each house rule, from the summary table
/// of the house rules' page.
fn house_rules(text: &str) -> BTreeMap<u32, String> {
    let mut rules = BTreeMap::new();
    for line in outside_fences(text) {
        let Some(row) = cells(line) else {
            continue;
        };
        let (Some(number), Some(topic)) = (row.first(), row.get(1)) else {
            continue;
        };
        let Ok(number) = number.parse::<u32>() else {
            continue;
        };
        // The topic is a link to the rule's section: `[Topic](#anchor)`.
        let topic = topic
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .map_or(topic.as_str(), |(topic, _)| topic);
        rules.insert(number, topic.to_string());
    }
    rules
}

/// A page's "House rules followed" row whose number and topic disagree
/// with the house rules' page.
fn rule_topics(rules: &BTreeMap<u32, String>, pages: &[Page]) -> Vec<String> {
    let mut problems = Vec::new();
    for page in pages {
        for (number, topic) in &page.rules {
            match rules.get(number) {
                Some(expected) if expected == topic => {}
                Some(expected) => problems.push(format!(
                    "{}: house rule {number} is `{expected}`, not `{topic}`",
                    page.path
                )),
                None => problems.push(format!("{}: there is no house rule {number}", page.path)),
            }
        }
    }
    problems
}

/// The pages `doc_pages.rs` includes, relative to `docs/`.
fn included_pages(doc_pages: &str) -> BTreeSet<String> {
    doc_pages
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("=> \"")?;
            let (page, _) = rest.split_once('"')?;
            Some(page.to_string())
        })
        .collect()
}

/// A page under `docs/` with a `rust` block that no include compiles, and
/// an include that names no page.
fn doctest_coverage(doc_pages: &str, markdown: &[(String, String)]) -> Vec<String> {
    let included = included_pages(doc_pages);
    let mut problems = Vec::new();
    let mut on_disk = BTreeSet::new();
    for (path, text) in markdown {
        let Some(page) = path.strip_prefix("docs/") else {
            continue;
        };
        on_disk.insert(page.to_string());
        let has_rust = fences(text).iter().any(|(tag, _)| tag == "rust");
        if has_rust && !included.contains(page) && path != TEMPLATE {
            problems.push(format!(
                "{path}: has a `rust` block, but {DOC_PAGES} does not include the page, \
                 so the block is never compiled"
            ));
        }
    }
    for page in included.difference(&on_disk) {
        problems.push(format!(
            "{DOC_PAGES}: includes docs/{page}, which does not exist"
        ));
    }
    problems
}

/// The module map's section of the architecture guide.
fn module_map(architecture: &str) -> String {
    let lines = outside_fences(architecture);
    let start = lines.iter().position(|line| line.trim_end() == MODULE_MAP);
    let Some(start) = start else {
        return String::new();
    };
    lines[start + 1..]
        .iter()
        .take_while(|line| !line.starts_with("## "))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// A public module the module map does not list, a page it does not link,
/// and a module it lists that is not public. A row lists a module by its
/// first cell (`` `theme` ``) or as ``module `tree` `` in its text.
fn module_map_coverage(
    architecture: &str,
    modules: &BTreeMap<String, Option<String>>,
    pages: &[Page],
) -> Vec<String> {
    let map = module_map(architecture);
    let mut problems = Vec::new();
    if map.is_empty() {
        return vec![format!("{ARCHITECTURE}: has no `{MODULE_MAP}` section")];
    }
    let mut listed = BTreeSet::new();
    for line in map.lines() {
        let Some(row) = cells(line) else {
            continue;
        };
        if let Some(first) = row.first()
            && let Some(name) = first
                .strip_prefix('`')
                .and_then(|rest| rest.strip_suffix('`'))
            && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        {
            listed.insert(name.to_string());
        }
        let mut rest = line;
        while let Some(at) = rest.find("module `") {
            rest = &rest[at + "module `".len()..];
            if let Some((name, _)) = rest.split_once('`') {
                listed.insert(name.to_string());
            }
        }
    }
    for module in modules.keys() {
        if !listed.contains(module) {
            problems.push(format!(
                "{ARCHITECTURE}: the module map does not list `tuikit::{module}`"
            ));
        }
    }
    for name in &listed {
        if !modules.contains_key(name) {
            problems.push(format!(
                "{ARCHITECTURE}: the module map lists `{name}`, which `src/lib.rs` does not \
                 declare public"
            ));
        }
    }
    for page in pages {
        let link = page.path.trim_start_matches("docs/");
        if !map.contains(&format!("({link})")) {
            problems.push(format!(
                "{ARCHITECTURE}: the module map does not link {}",
                page.path
            ));
        }
    }
    problems
}

/// The anchor GitHub gives a heading: lower case, punctuation other than
/// `-` and `_` dropped, spaces as `-`.
fn slug(heading: &str) -> String {
    heading
        .trim()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// The anchors a page's headings give, a repeated heading numbered as
/// GitHub numbers it.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut found = BTreeSet::new();
    for line in outside_fences(text) {
        let Some(heading) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let heading = heading.trim_start_matches('#');
        if !heading.starts_with(' ') {
            continue;
        }
        let base = slug(heading);
        let count = seen.entry(base.clone()).or_insert(0);
        found.insert(if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        });
        *count += 1;
    }
    found
}

/// The targets of a line's inline links (`[text](target)`) and reference
/// definitions (`[name]: target`), inline code left out.
fn link_targets(line: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            plain.push(c);
        }
    }
    let mut targets = Vec::new();
    let mut rest = plain.as_str();
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        if let Some((target, _)) = rest.split_once(')') {
            targets.push(target.trim().to_string());
        }
    }
    let trimmed = plain.trim_start();
    if trimmed.starts_with('[')
        && let Some((_, target)) = trimmed.split_once("]: ")
    {
        targets.push(target.trim().to_string());
    }
    targets
}

/// `relative` joined onto the directory of `from`, both crate-relative.
fn resolve(from: &str, relative: &str) -> String {
    let mut parts: Vec<String> = Path::new(from)
        .parent()
        .map(|dir| {
            dir.components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    for part in Path::new(relative).components() {
        match part {
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    parts.join("/")
}

/// A relative link that names a file or a heading that does not exist.
/// `exists` says whether a crate-relative path is a file or directory.
fn broken_links(markdown: &[(String, String)], exists: impl Fn(&str) -> bool) -> Vec<String> {
    let texts: BTreeMap<&str, &str> = markdown
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let mut problems = Vec::new();
    for (path, text) in markdown {
        for (number, line) in outside_fences(text).iter().enumerate() {
            for target in link_targets(line) {
                if target.contains("://") || target.starts_with("mailto:") {
                    continue;
                }
                let (file, anchor) = match target.split_once('#') {
                    Some((file, anchor)) => (file, Some(anchor)),
                    None => (target.as_str(), None),
                };
                let file = if file.is_empty() {
                    path.clone()
                } else {
                    resolve(path, file)
                };
                if !exists(&file) {
                    problems.push(format!(
                        "{path}:{}: links {target}, but {file} does not exist",
                        number + 1
                    ));
                    continue;
                }
                let Some(anchor) = anchor else {
                    continue;
                };
                match texts.get(file.as_str()) {
                    Some(target_text) if anchors(target_text).contains(anchor) => {}
                    Some(_) => problems.push(format!(
                        "{path}:{}: links {target}, but {file} has no heading `#{anchor}`",
                        number + 1
                    )),
                    None => {}
                }
            }
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// The decision log.
// ---------------------------------------------------------------------------

/// A four-digit record number at the start of `text`.
fn leading_number(text: &str) -> Option<String> {
    let number: String = text.chars().take_while(char::is_ascii_digit).collect();
    (number.len() == 4).then_some(number)
}

/// Every four-digit number in `text` that stands alone (not part of a
/// longer run of digits), in order.
fn numbers_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut run = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            run.push(c);
            continue;
        }
        if run.len() == 4 {
            found.push(run.clone());
        }
        run.clear();
    }
    found
}

/// The lines of a `##` section, fenced blocks blanked; empty when the text
/// has no such section.
fn section_lines<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    let lines = outside_fences(text);
    let Some(start) = lines.iter().position(|line| line.trim_end() == heading) else {
        return Vec::new();
    };
    lines[start + 1..]
        .iter()
        .take_while(|line| !line.starts_with("## "))
        .copied()
        .collect()
}

/// The host records an `## Origin` section names, or why it cannot be
/// read. An item names one (`- host ADR 0153, "Title": what it decided`,
/// wrapped lines indented under it); a section with no item says why the
/// decision has no host record, in a paragraph that starts with `None:`.
fn origins(text: &str) -> Result<Vec<String>, String> {
    if !outside_fences(text)
        .iter()
        .any(|line| line.trim_end() == ORIGIN)
    {
        return Err(format!("has no `{ORIGIN}` section"));
    }
    let lines = section_lines(text, ORIGIN);
    let mut numbers = Vec::new();
    let mut prose = Vec::new();
    for line in lines {
        if let Some(item) = line.strip_prefix("- ") {
            let number = item.strip_prefix("host ADR ").and_then(leading_number);
            match number {
                Some(number) => numbers.push(number),
                None => {
                    return Err(format!(
                        "its origin item `{}` should start with `host ADR NNNN`",
                        item.trim()
                    ));
                }
            }
        } else if !line.trim().is_empty() && !line.starts_with(' ') {
            prose.push(line.trim());
        }
    }
    let says_none = prose.first().is_some_and(|line| line.starts_with("None:"));
    match (numbers.is_empty(), says_none) {
        (true, true) => Ok(numbers),
        (true, false) => Err(format!(
            "its `{ORIGIN}` names no host record: list `- host ADR NNNN` items, \
             or say why there is none in a paragraph that starts with `None:`"
        )),
        (false, true) => Err(format!(
            "its `{ORIGIN}` both names host records and says it has none"
        )),
        (false, false) => Ok(numbers),
    }
}

/// What the checks read of one record.
struct Record {
    path: String,
    /// The number the file name starts with.
    number: Option<String>,
    page: Page,
    origins: Result<Vec<String>, String>,
}

impl Record {
    fn parse(path: &str, text: &str) -> Self {
        let file = path.rsplit('/').next().unwrap_or(path);
        Self {
            path: path.to_string(),
            number: leading_number(file).filter(|_| file.as_bytes().get(4) == Some(&b'-')),
            page: Page::parse(path, text),
            origins: origins(text),
        }
    }

    /// The title after the number: `# 0001: The title` gives `The title`.
    fn title(&self) -> Option<&str> {
        let title = self.page.title.as_deref()?;
        let number = self.number.as_deref()?;
        title.strip_prefix(number)?.strip_prefix(": ")
    }

    fn status(&self) -> Option<&str> {
        self.page.row("Status")
    }

    fn origin_numbers(&self) -> BTreeSet<String> {
        self.origins
            .as_ref()
            .map(|numbers| numbers.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// The records: every page under `docs/decisions/` but the index.
fn records(markdown: &[(String, String)]) -> Vec<Record> {
    markdown
        .iter()
        .filter(|(path, _)| path.starts_with(DECISIONS) && path != DECISIONS_INDEX)
        .map(|(path, text)| Record::parse(path, text))
        .collect()
}

/// A row of the log's index: `| [0001](0001-title.md) | Title | Status |
/// Origin |`, the origin cell listing host record numbers or `none`.
struct IndexRow {
    number: String,
    target: String,
    title: String,
    status: String,
    origins: BTreeSet<String>,
}

/// The rows of the log's index that link a record.
fn index_rows(index: &str) -> Vec<IndexRow> {
    let mut rows = Vec::new();
    for line in outside_fences(index) {
        let Some(row) = cells(line) else {
            continue;
        };
        let Some(link) = row.first() else {
            continue;
        };
        let Some((number, rest)) = link
            .strip_prefix('[')
            .and_then(|rest| rest.split_once("]("))
        else {
            continue;
        };
        let Some(target) = rest.strip_suffix(')') else {
            continue;
        };
        if leading_number(number).as_deref() != Some(number) {
            continue;
        }
        let cell = |at: usize| row.get(at).cloned().unwrap_or_default();
        rows.push(IndexRow {
            number: number.to_string(),
            target: target.to_string(),
            title: cell(1),
            status: cell(2),
            origins: numbers_in(&cell(3)).into_iter().collect(),
        });
    }
    rows
}

/// The host records the index lists as left out, from the first cell of
/// each row of its `## Host records not included` table.
fn not_included(index: &str) -> BTreeSet<String> {
    section_lines(index, NOT_INCLUDED)
        .iter()
        .filter_map(|line| cells(line))
        .filter_map(|row| row.first().map(|cell| numbers_in(cell)))
        .flatten()
        .collect()
}

/// A record that lacks its title, its status, a record section (in order),
/// or a readable origin.
fn record_shape(record: &Record) -> Vec<String> {
    let mut problems = Vec::new();
    let path = &record.path;
    let Some(number) = &record.number else {
        return vec![format!(
            "{path}: a record's file name is its number and a short title (`0001-title.md`)"
        )];
    };
    if record.title().is_none() {
        problems.push(format!(
            "{path}: should open with `# {number}: ` and the decision's title"
        ));
    }
    match record.status() {
        Some("Proposed" | "Accepted") => {}
        Some(status) if status.starts_with("Superseded by ") => {}
        Some(status) => problems.push(format!(
            "{path}: status `{status}` is not `Proposed`, `Accepted`, or `Superseded by NNNN`"
        )),
        None => problems.push(format!("{path}: the header table lacks its `Status` row")),
    }
    let mut at = 0;
    for heading in RECORD_SECTIONS {
        match record.page.sections[at..]
            .iter()
            .position(|name| name == heading)
        {
            Some(found) => at += found + 1,
            None => problems.push(format!("{path}: lacks `{heading}` (in this order)")),
        }
    }
    if let Err(problem) = &record.origins {
        problems.push(format!("{path}: {problem}"));
    }
    problems
}

/// A record the index does not list, a row that links no record, and a
/// row whose number, title, status, or origins differ from its record's.
fn log_coverage(rows: &[IndexRow], records: &[Record]) -> Vec<String> {
    let mut problems = Vec::new();
    let by_file: BTreeMap<String, &Record> = records
        .iter()
        .map(|record| {
            (
                record.path.trim_start_matches(DECISIONS).to_string(),
                record,
            )
        })
        .collect();
    let mut listed: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows {
        *listed.entry(row.target.as_str()).or_default() += 1;
        let Some(record) = by_file.get(&row.target) else {
            problems.push(format!(
                "{DECISIONS_INDEX}: row {} links {}, which is not a record",
                row.number, row.target
            ));
            continue;
        };
        let path = &record.path;
        if record.number.as_deref() != Some(row.number.as_str()) {
            problems.push(format!(
                "{DECISIONS_INDEX}: row {} links {path}, a record of another number",
                row.number
            ));
        }
        if record.title() != Some(row.title.as_str()) {
            problems.push(format!(
                "{DECISIONS_INDEX}: row {}'s title `{}` is not {path}'s `{}`",
                row.number,
                row.title,
                record.title().unwrap_or_default()
            ));
        }
        if record.status() != Some(row.status.as_str()) {
            problems.push(format!(
                "{DECISIONS_INDEX}: row {}'s status `{}` is not {path}'s `{}`",
                row.number,
                row.status,
                record.status().unwrap_or_default()
            ));
        }
        if record.origins.is_ok() && record.origin_numbers() != row.origins {
            problems.push(format!(
                "{DECISIONS_INDEX}: row {}'s origins {:?} are not {path}'s {:?}",
                row.number,
                row.origins,
                record.origin_numbers()
            ));
        }
    }
    for (file, record) in &by_file {
        match listed.get(file.as_str()) {
            None => problems.push(format!(
                "{}: the index ({DECISIONS_INDEX}) has no row for it",
                record.path
            )),
            Some(1) => {}
            Some(_) => problems.push(format!(
                "{}: the index ({DECISIONS_INDEX}) lists it more than once",
                record.path
            )),
        }
    }
    let mut numbers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for record in records {
        if let Some(number) = &record.number {
            numbers.entry(number).or_default().push(&record.path);
        }
    }
    for (number, paths) in numbers {
        if paths.len() > 1 {
            problems.push(format!(
                "records share the number {number}: {}",
                paths.join(", ")
            ));
        }
    }
    problems
}

/// A row of the host's index: a record number, its status cell, and the
/// `##` section it is listed under.
struct HostRow {
    number: String,
    status: String,
    section: String,
}

/// What the checks read of the host's decision records.
struct Host {
    /// The numbers of the record files.
    files: BTreeSet<String>,
    /// The rows of the host's index.
    rows: Vec<HostRow>,
}

impl Host {
    fn parse(file_names: &[String], index: &str) -> Self {
        let files = file_names
            .iter()
            .filter(|name| name.as_bytes().get(4) == Some(&b'-'))
            .filter_map(|name| leading_number(name))
            .collect();
        let mut rows = Vec::new();
        let mut section = String::new();
        for line in outside_fences(index) {
            if line.starts_with("## ") {
                section = line.trim_end().to_string();
                continue;
            }
            let Some(row) = cells(line) else {
                continue;
            };
            let number = row
                .first()
                .and_then(|cell| cell.strip_prefix('['))
                .and_then(leading_number);
            if let (Some(number), Some(status)) = (number, row.get(2)) {
                rows.push(HostRow {
                    number,
                    status: status.clone(),
                    section: section.clone(),
                });
            }
        }
        Self { files, rows }
    }
}

/// The host's decision records, when the crate is in its host's
/// repository: the manifest two directories up names it as a member.
/// `None` when the crate stands alone.
fn host() -> Option<Host> {
    let manifest = fs::read_to_string(root().join(HOST_MANIFEST)).ok()?;
    if !manifest.contains(HOST_MEMBER) {
        return None;
    }
    let dir = root().join(HOST_RECORDS);
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let index = fs::read_to_string(dir.join("README.md"))
        .unwrap_or_else(|error| panic!("read {}/README.md: {error}", dir.display()));
    Some(Host::parse(&names, &index))
}

/// A host record the log names that the host does not have; one listed
/// both as an origin and as left out; a host index row whose mark
/// disagrees with the log; and a record of the host's section the log
/// neither restates nor lists as left out.
fn host_agreement(records: &[Record], left_out: &BTreeSet<String>, host: &Host) -> Vec<String> {
    let mut problems = Vec::new();
    let mut origins: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for record in records {
        for number in record.origin_numbers() {
            origins.entry(number).or_default().push(&record.path);
        }
    }
    for (number, paths) in &origins {
        if !host.files.contains(number) {
            problems.push(format!(
                "{}: names host ADR {number}, which {HOST_RECORDS} does not have",
                paths.join(", ")
            ));
        }
        if left_out.contains(number) {
            problems.push(format!(
                "{DECISIONS_INDEX}: lists host ADR {number} as left out, but it is restated \
                 by {}",
                paths.join(", ")
            ));
        }
    }
    for number in left_out {
        if !host.files.contains(number) {
            problems.push(format!(
                "{DECISIONS_INDEX}: lists host ADR {number} as left out, which \
                 {HOST_RECORDS} does not have"
            ));
        }
    }
    let marked: BTreeSet<&str> = host
        .rows
        .iter()
        .filter(|row| row.status.to_lowercase().contains(OWNED_MARK))
        .map(|row| row.number.as_str())
        .collect();
    let indexed: BTreeSet<&str> = host.rows.iter().map(|row| row.number.as_str()).collect();
    for (number, paths) in &origins {
        if indexed.contains(number.as_str()) && !marked.contains(number.as_str()) {
            problems.push(format!(
                "{HOST_RECORDS}/README.md: does not mark ADR {number} `{OWNED_MARK}`, but it \
                 is restated by {}",
                paths.join(", ")
            ));
        }
        if !indexed.contains(number.as_str()) && host.files.contains(number) {
            problems.push(format!(
                "{HOST_RECORDS}/README.md: does not list ADR {number}, which is restated by {}",
                paths.join(", ")
            ));
        }
    }
    for number in marked {
        if !origins.contains_key(number) {
            problems.push(format!(
                "{HOST_RECORDS}/README.md: marks ADR {number} `{OWNED_MARK}`, but no record \
                 under {DECISIONS} restates it"
            ));
        }
    }
    for row in host.rows.iter().filter(|row| row.section == HOST_SECTION) {
        if !origins.contains_key(&row.number) && !left_out.contains(&row.number) {
            problems.push(format!(
                "{HOST_RECORDS}/README.md: ADR {} of `{HOST_SECTION}` is neither restated by \
                 a record nor listed under `{NOT_INCLUDED}` in {DECISIONS_INDEX}",
                row.number
            ));
        }
    }
    problems
}

fn assert_none(what: &str, problems: &[String]) {
    assert!(problems.is_empty(), "{what}:\n  {}", problems.join("\n  "));
}

// ---------------------------------------------------------------------------
// The checks, run on the real tree.
// ---------------------------------------------------------------------------

#[test]
fn every_public_module_has_one_page_and_every_page_names_a_public_module() {
    let modules = public_modules(&read("src/lib.rs"));
    let pages = reference_pages(&markdown_files());
    assert!(
        pages.len() >= MIN_PAGES,
        "found {} pages under {CONTROLS}, fewer than {MIN_PAGES}: is the directory right?",
        pages.len()
    );
    assert_none("module and page coverage", &coverage(&modules, &pages));
}

#[test]
fn every_page_keeps_the_template_shape() {
    let markdown = markdown_files();
    let template = Page::parse(TEMPLATE, &read(TEMPLATE));
    assert!(
        !template.header.is_empty() && !template.sections.is_empty(),
        "{TEMPLATE} lost its header table or sections"
    );
    let problems: Vec<String> = reference_pages(&markdown)
        .iter()
        .flat_map(|page| shape(&template, page))
        .collect();
    assert_none("page shape", &problems);
}

#[test]
fn every_page_names_the_feature_its_module_is_behind() {
    let modules = public_modules(&read("src/lib.rs"));
    let pages = reference_pages(&markdown_files());
    assert_none("features", &features(&modules, &pages));
}

#[test]
fn every_page_numbers_the_house_rules_as_the_house_rules_page_does() {
    let rules = house_rules(&read(HOUSE_RULES));
    assert_eq!(
        rules.keys().copied().collect::<Vec<_>>(),
        (1..=18).collect::<Vec<u32>>(),
        "{HOUSE_RULES} should list rules 1 to 18 in its summary table"
    );
    let pages = reference_pages(&markdown_files());
    assert_none("house rule topics", &rule_topics(&rules, &pages));
}

#[test]
fn every_page_with_rust_code_is_compiled_as_a_doctest() {
    assert_none(
        "doctest coverage",
        &doctest_coverage(&read(DOC_PAGES), &markdown_files()),
    );
}

#[test]
fn the_module_map_lists_every_module_and_links_every_page() {
    let modules = public_modules(&read("src/lib.rs"));
    let pages = reference_pages(&markdown_files());
    assert_none(
        "module map",
        &module_map_coverage(&read(ARCHITECTURE), &modules, &pages),
    );
}

#[test]
fn every_relative_link_names_a_file_and_heading_that_exist() {
    let problems = broken_links(&markdown_files(), |path| root().join(path).exists());
    assert_none("links", &problems);
}

#[test]
fn every_record_keeps_the_record_shape_and_names_its_origin() {
    let records = records(&markdown_files());
    assert!(
        records.len() >= MIN_RECORDS,
        "found {} records under {DECISIONS}, fewer than {MIN_RECORDS}: is the directory right?",
        records.len()
    );
    let problems: Vec<String> = records.iter().flat_map(record_shape).collect();
    assert_none("record shape", &problems);
}

#[test]
fn the_decision_index_lists_every_record_as_the_record_reads() {
    let rows = index_rows(&read(DECISIONS_INDEX));
    let records = records(&markdown_files());
    assert_none("decision index", &log_coverage(&rows, &records));
}

#[test]
fn the_log_agrees_with_the_host_records_while_it_has_a_host() {
    let Some(host) = host() else {
        eprintln!(
            "skipped: no host repository ({HOST_MANIFEST} does not name this crate as a \
             member), so the log is checked on its own"
        );
        return;
    };
    let records = records(&markdown_files());
    let left_out = not_included(&read(DECISIONS_INDEX));
    assert!(
        !left_out.is_empty(),
        "{DECISIONS_INDEX}: `{NOT_INCLUDED}` lists no host record: is the heading right?"
    );
    assert!(
        host.rows.iter().any(|row| row.section == HOST_SECTION),
        "{HOST_RECORDS}/README.md has no `{HOST_SECTION}` rows: is the heading right?"
    );
    assert_none("host records", &host_agreement(&records, &left_out, &host));
}

// ---------------------------------------------------------------------------
// Self-tests: each check fails on a broken input fed to it from memory.
// ---------------------------------------------------------------------------

const LIB: &str = "\
//! A crate.
pub mod alpha;
// A comment between an attribute and its module.
#[cfg(feature = \"extra\")]
pub mod beta;
#[cfg(any(test, feature = \"kit\"))]
pub mod kit;
mod private;
";

const TEMPLATE_TEXT: &str = "\
# `Name`

| | |
| --- | --- |
| Module | `tuikit::<module>` |
| Cargo feature | none |

## Purpose

## Usage example

```rust
```

## Seams left open
";

fn page(module: &str, feature: &str, sections: &[&str]) -> String {
    let mut text = format!(
        "# `Page`\n\n| | |\n| --- | --- |\n| Module | `tuikit::{module}` |\n\
         | Cargo feature | {feature} |\n"
    );
    for section in sections {
        text.push_str(&format!("\n{section}\n"));
        if *section == "## Usage example" {
            text.push_str("\n```rust\nlet x = 1;\n```\n");
        }
    }
    text
}

fn pages_of(entries: &[(&str, &str)]) -> Vec<Page> {
    entries
        .iter()
        .map(|(path, text)| Page::parse(path, text))
        .collect()
}

const ALL_SECTIONS: &[&str] = &["## Purpose", "## Usage example", "## Seams left open"];

#[test]
fn modules_are_read_with_their_feature_gates() {
    let modules = public_modules(LIB);
    assert_eq!(
        modules,
        BTreeMap::from([
            ("alpha".to_string(), None),
            ("beta".to_string(), Some("extra".to_string())),
            ("kit".to_string(), Some("kit".to_string())),
        ])
    );
}

#[test]
fn coverage_fails_on_a_module_without_a_page_and_a_page_without_a_module() {
    let modules = public_modules(LIB);
    let alpha = page("alpha", "none", ALL_SECTIONS);
    let beta = page("beta", "`extra`", ALL_SECTIONS);
    let gone = page("gone", "none", ALL_SECTIONS);
    let pages = pages_of(&[
        ("docs/controls/alpha.md", &alpha),
        ("docs/controls/beta.md", &beta),
        ("docs/controls/gone.md", &gone),
    ]);
    let problems = coverage(&modules, &pages);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("`tuikit::kit` has no reference page")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("names `tuikit::gone`")),
        "{problems:?}"
    );
    assert_eq!(problems.len(), 2, "{problems:?}");

    let twice = pages_of(&[
        ("docs/controls/alpha.md", &alpha),
        ("docs/controls/alpha_again.md", &alpha),
    ]);
    assert!(
        coverage(&BTreeMap::from([("alpha".to_string(), None)]), &twice)
            .iter()
            .any(|p| p.contains("claimed by several pages"))
    );
}

#[test]
fn shape_fails_on_a_missing_section_row_or_example() {
    let template = Page::parse(TEMPLATE, TEMPLATE_TEXT);
    let whole = page("alpha", "none", ALL_SECTIONS);
    assert!(shape(&template, &Page::parse("p.md", &whole)).is_empty());

    let no_seams = page("alpha", "none", &["## Purpose", "## Usage example"]);
    let problems = shape(&template, &Page::parse("p.md", &no_seams));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("lacks `## Seams left open`")),
        "{problems:?}"
    );

    let out_of_order = page(
        "alpha",
        "none",
        &["## Usage example", "## Purpose", "## Seams left open"],
    );
    assert!(!shape(&template, &Page::parse("p.md", &out_of_order)).is_empty());

    let no_row = whole.replace("| Cargo feature | none |\n", "");
    let problems = shape(&template, &Page::parse("p.md", &no_row));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("lacks its `Cargo feature` row")),
        "{problems:?}"
    );

    let no_example = whole.replace("```rust", "```text");
    let problems = shape(&template, &Page::parse("p.md", &no_example));
    assert!(
        problems.iter().any(|p| p.contains("no `rust` block")),
        "{problems:?}"
    );
}

#[test]
fn features_fail_on_a_page_that_hides_or_invents_its_gate() {
    let modules = public_modules(LIB);
    let silent = page("beta", "none", ALL_SECTIONS);
    let invented = page("alpha", "`extra`", ALL_SECTIONS);
    let problems = features(
        &modules,
        &pages_of(&[("b.md", &silent), ("a.md", &invented)]),
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn rule_topics_fail_on_a_renumbered_rule() {
    let rules = house_rules(
        "| # | Topic | Rule |\n| --- | --- | --- |\n| 1 | [Cursor](#1-cursor) | a |\n\
         | 2 | [Ends](#2-ends) | b |\n",
    );
    assert_eq!(rules.get(&2).map(String::as_str), Some("Ends"));
    let text = "# `P`\n\n## House rules followed\n\n| # | Rule | How |\n| --- | --- | --- |\n\
                | 1 | Cursor | yes |\n| 2 | Cursor | no |\n| 3 | Gone | no |\n";
    let problems = rule_topics(&rules, &pages_of(&[("p.md", text)]));
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn doctest_coverage_fails_on_an_uncompiled_page_and_a_stale_include() {
    let doc_pages = "pages! {\n    Guide => \"guide.md\",\n    Gone => \"controls/gone.md\",\n}\n";
    let markdown = vec![
        (
            "docs/guide.md".to_string(),
            "# G\n\n```rust\n```\n".to_string(),
        ),
        (
            "docs/other.md".to_string(),
            "# O\n\n```rust\n```\n".to_string(),
        ),
        (
            "docs/prose.md".to_string(),
            "# P\n\n```text\n```\n".to_string(),
        ),
    ];
    let problems = doctest_coverage(doc_pages, &markdown);
    assert!(
        problems.iter().any(|p| p.starts_with("docs/other.md")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("docs/controls/gone.md")),
        "{problems:?}"
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn the_module_map_check_fails_on_a_missing_or_stale_row() {
    let modules = public_modules(LIB);
    let pages = pages_of(&[(
        "docs/controls/alpha.md",
        &page("alpha", "none", ALL_SECTIONS),
    )]);
    let architecture = "# A\n\n## Module map\n\n| Module | Provides |\n| --- | --- |\n\
                        | `alpha` | a. Page: [controls/alpha.md](controls/alpha.md) |\n\
                        | `focus` | rules |\n| Kit | the kit (module `kit`) |\n\n## Next\n\n\
                        | `beta` | outside the map |\n";
    let problems = module_map_coverage(architecture, &modules, &pages);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("does not list `tuikit::beta`")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("lists `focus`")),
        "{problems:?}"
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn links_fail_on_a_missing_file_or_heading() {
    let markdown = vec![
        (
            "docs/a.md".to_string(),
            "# A\n\n## The `Thing`: one\n\nSee [b](b.md#the-rule), [here](#the-thing-one), \
             [gone](gone.md), [no](b.md#no-such), and [web](https://example.com).\n\
             `[code](not-a-link.md)`\n\n```text\n[fenced](missing.md)\n```\n"
                .to_string(),
        ),
        ("docs/b.md".to_string(), "# B\n\n### The rule\n".to_string()),
    ];
    let exists = |path: &str| path == "docs/a.md" || path == "docs/b.md";
    let problems = broken_links(&markdown, exists);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("gone.md does not exist")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("no heading `#no-such`")),
        "{problems:?}"
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn slugs_follow_the_heading_anchors_readers_link() {
    assert_eq!(slug("`PaneFrame`"), "paneframe");
    assert_eq!(
        slug("1. List cursor and highlight"),
        "1-list-cursor-and-highlight"
    );
    assert_eq!(
        slug("The screen: `AppFrame` and `Screen`"),
        "the-screen-appframe-and-screen"
    );
    assert_eq!(slug("Seams left open"), "seams-left-open");
    let repeated = anchors("# T\n\n## Rows\n\n### Rows\n");
    assert!(repeated.contains("rows") && repeated.contains("rows-1"));
}

/// A record in the log's shape, with `origin` as its `## Origin` body.
fn record_text(number: &str, title: &str, origin: &str) -> String {
    format!(
        "# {number}: {title}\n\n| | |\n| --- | --- |\n| Status | Accepted |\n\n\
         ## Decision\n\nD.\n\n## Rationale\n\nR.\n\n## Consequences\n\nC.\n\n\
         ## Alternatives considered\n\nA.\n\n## Origin\n\n{origin}\n"
    )
}

fn record_at(file: &str, text: &str) -> Record {
    Record::parse(&format!("{DECISIONS}{file}"), text)
}

#[test]
fn origins_are_read_from_items_or_a_none_paragraph() {
    let two = "- host ADR 0153, \"Theme tokens\": decisions 1 and 2,\n  \
               wrapped under the item.\n- host ADR 0032, \"Theme value\": all of it.";
    let text = record_text("0001", "T", two);
    assert_eq!(origins(&text), Ok(vec!["0153".into(), "0032".into()]));
    let none = record_text("0002", "T", "None: first recorded in this log.");
    assert_eq!(origins(&none), Ok(Vec::new()));

    let missing = record_text("0003", "T", "").replace("## Origin", "## Source");
    assert!(origins(&missing).unwrap_err().contains("no `## Origin`"));
    let empty = record_text("0004", "T", "Some prose.");
    assert!(
        origins(&empty)
            .unwrap_err()
            .contains("names no host record")
    );
    let loose = record_text("0005", "T", "- ADR 0153: no host word.");
    assert!(origins(&loose).unwrap_err().contains("should start with"));
    let both = record_text("0006", "T", "None: nothing.\n\n- host ADR 0153, \"T\": x.");
    assert!(origins(&both).unwrap_err().contains("both"));
}

#[test]
fn record_shape_fails_on_a_missing_title_status_section_or_origin() {
    let whole = record_text("0007", "Title", "- host ADR 0150, \"T\": x.");
    assert!(record_shape(&record_at("0007-title.md", &whole)).is_empty());

    let unnumbered = record_shape(&record_at("title.md", &whole));
    assert!(
        unnumbered[0].contains("file name is its number"),
        "{unnumbered:?}"
    );

    let other_number = record_shape(&record_at("0008-title.md", &whole));
    assert!(
        other_number
            .iter()
            .any(|p| p.contains("should open with `# 0008: `")),
        "{other_number:?}"
    );

    let no_status = whole.replace("| Status | Accepted |\n", "");
    let problems = record_shape(&record_at("0007-title.md", &no_status));
    assert!(
        problems
            .iter()
            .any(|p| p.contains("lacks its `Status` row")),
        "{problems:?}"
    );
    let odd_status = whole.replace("| Accepted |", "| Maybe |");
    assert!(!record_shape(&record_at("0007-title.md", &odd_status)).is_empty());

    let no_rationale = whole.replace("## Rationale\n\nR.\n\n", "");
    let problems = record_shape(&record_at("0007-title.md", &no_rationale));
    assert!(
        problems.iter().any(|p| p.contains("lacks `## Rationale`")),
        "{problems:?}"
    );

    let no_origin = whole.replace("- host ADR 0150, \"T\": x.", "");
    let problems = record_shape(&record_at("0007-title.md", &no_origin));
    assert!(
        problems.iter().any(|p| p.contains("names no host record")),
        "{problems:?}"
    );
}

const INDEX: &str = "\
# Framework decisions

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| [0001](0001-one.md) | One | Accepted | 0150 |
| [0002](0002-two.md) | Two | Accepted | none |

## Host records not included

| Host records | Why |
| --- | --- |
| 0019, 0052 | host content |
| 0118 | the host's name |
";

#[test]
fn the_index_and_its_records_are_read_alike() {
    let rows = index_rows(INDEX);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].target, "0001-one.md");
    assert_eq!(rows[0].origins, BTreeSet::from(["0150".to_string()]));
    assert!(rows[1].origins.is_empty());
    assert_eq!(
        not_included(INDEX),
        BTreeSet::from(["0019".into(), "0052".into(), "0118".into()])
    );
    assert_eq!(numbers_in("0150, 0024 and 12345 or 015"), ["0150", "0024"]);
}

#[test]
fn log_coverage_fails_when_the_index_and_the_records_disagree() {
    let one = record_text("0001", "One", "- host ADR 0150, \"T\": x.");
    let two = record_text("0002", "Two", "None: first recorded here.");
    let records = vec![
        record_at("0001-one.md", &one),
        record_at("0002-two.md", &two),
    ];
    assert_none("coverage", &log_coverage(&index_rows(INDEX), &records));

    let renamed = record_text(
        "0001",
        "Uno",
        "- host ADR 0150, \"T\": x.\n- host ADR 0024, \"U\": y.",
    );
    let three = record_text("0003", "Three", "None: first recorded here.");
    let records = vec![
        record_at("0001-one.md", &renamed),
        record_at("0003-three.md", &three),
    ];
    let problems = log_coverage(&index_rows(INDEX), &records);
    for expected in [
        "title `One` is not",
        "origins {\"0150\"} are not",
        "links 0002-two.md, which is not a record",
        "0003-three.md: the index",
    ] {
        assert!(
            problems.iter().any(|p| p.contains(expected)),
            "{expected}: {problems:?}"
        );
    }
    assert_eq!(problems.len(), 4, "{problems:?}");

    let twice = format!("{INDEX}| [0001](0001-one.md) | One | Accepted | 0150 |\n");
    let records = vec![
        record_at("0001-one.md", &one),
        record_at("0002-two.md", &two),
        record_at(
            "0002-again.md",
            &two.replace("# 0002: Two", "# 0002: Again"),
        ),
    ];
    let problems = log_coverage(&index_rows(&twice), &records);
    assert!(
        problems.iter().any(|p| p.contains("more than once")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("share the number 0002")),
        "{problems:?}"
    );
}

const HOST_INDEX: &str = "\
# Decision records

## Interactive TUI

| ADR | Decision | Status |
| --- | --- | --- |
| [0150](0150-a.md) | A | Accepted; framework-owned |
| [0151](0151-b.md) | B | Accepted (amended by 0152); framework-owned in part |
| [0118](0118-c.md) | C | Accepted |

## Elsewhere

| ADR | Decision | Status |
| --- | --- | --- |
| [0024](0024-d.md) | D | Accepted |
";

fn host_of(files: &[&str], index: &str) -> Host {
    let names: Vec<String> = files.iter().map(|name| (*name).to_string()).collect();
    Host::parse(&names, index)
}

#[test]
fn host_agreement_fails_on_a_missing_number_a_wrong_mark_or_a_record_left_unsorted() {
    let files = [
        "0150-a.md",
        "0151-b.md",
        "0118-c.md",
        "0024-d.md",
        "README.md",
    ];
    let host = host_of(&files, HOST_INDEX);
    assert_eq!(host.files.len(), 4);
    assert_eq!(host.rows.len(), 4);
    let a = record_text("0001", "A", "- host ADR 0150, \"A\": x.");
    let b = record_text("0002", "B", "- host ADR 0151, \"B\": y.");
    let records = vec![record_at("0001-a.md", &a), record_at("0002-b.md", &b)];
    let left_out = BTreeSet::from(["0118".to_string()]);
    assert_none("host", &host_agreement(&records, &left_out, &host));

    // A record that names a host record the host lacks or does not mark,
    // a left-out record that a record restates, a marked record no record
    // restates, and a record of the section that is neither.
    let gone = record_text(
        "0003",
        "C",
        "- host ADR 0999, \"Gone\": x.\n- host ADR 0024, \"D\": y.",
    );
    let records = vec![record_at("0001-a.md", &a), record_at("0003-c.md", &gone)];
    let left_out = BTreeSet::from(["0150".to_string(), "0777".to_string()]);
    let problems = host_agreement(&records, &left_out, &host);
    for expected in [
        "names host ADR 0999, which",
        "lists host ADR 0150 as left out, but",
        "lists host ADR 0777 as left out, which",
        "does not mark ADR 0024 `framework-owned`, but",
        "marks ADR 0151 `framework-owned`, but no record",
        "ADR 0151 of `## Interactive TUI` is neither",
        "ADR 0118 of `## Interactive TUI` is neither",
    ] {
        assert!(
            problems.iter().any(|p| p.contains(expected)),
            "{expected}: {problems:?}"
        );
    }
    assert_eq!(problems.len(), 7, "{problems:?}");
}
