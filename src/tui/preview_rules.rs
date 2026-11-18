//! The Preview's smart-wrap settings and cleanup rules (ADR 0252).
//!
//! Smart wrap is the framework's (`tuikit::richtext::SmartWrap` and
//! `LineFit`, ADR 0181). What is Conspectus's is the configuration:
//! `[tui.preview]` sets the smart wrap's settings, and each
//! `[[tui.preview.rules]]` entry fits the lines of a pane capture that match
//! its pattern some other way, for an agent's known quirks. The built-in
//! settings and rules are a document in the same format
//! (`preview_defaults.toml`), merged first; the user's config and then the
//! project's merge over it, a setting key by key and a rule by name.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde::Deserialize;
use tuikit::richtext::{LineFit, SmartWrap};

use crate::model::{GraphNode, GraphSnapshot, LinkState, MuxSessionId, NodeId, RelationKind};

/// The built-in settings and rules, in a config file's own format.
const BUILTIN_DOCUMENT: &str = include_str!("preview_defaults.toml");

/// The Preview's smart-wrap settings and the rules that fit lines of a
/// pane capture some other way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewConfig {
    /// `[tui.preview] decoration`, `min_rule_run`, and `shorten_rules`.
    pub smart: SmartWrap,
    /// `[[tui.preview.rules]]`, checked in order: the first that applies
    /// to the pane and matches a line decides how it fits.
    pub rules: Vec<PreviewRule>,
}

impl Default for PreviewConfig {
    /// The built-in document's settings and rules.
    fn default() -> Self {
        BUILTIN.clone()
    }
}

/// One `[[tui.preview.rules]]` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewRule {
    /// What a later layer names to replace or turn off this rule.
    pub name: String,
    /// The harness a pane must run for the rule to apply; `None` applies
    /// it to every pane.
    pub harness: Option<String>,
    /// Matched against a line's text, its styling dropped.
    pub pattern: Pattern,
    /// How a matching line fits the Preview.
    pub fit: LineFit,
}

/// A compiled `match` pattern, compared by its source.
#[derive(Clone, Debug)]
pub struct Pattern(regex::Regex);

impl Pattern {
    /// `source` compiled, or why it doesn't compile.
    pub fn new(source: &str) -> Result<Self, String> {
        regex::Regex::new(source).map(Self).map_err(|err| {
            // The parser's message draws a caret under the error on
            // lines of its own; the last line says what is wrong.
            err.to_string()
                .lines()
                .last()
                .unwrap_or_default()
                .trim_start_matches("error: ")
                .to_string()
        })
    }

    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Pattern {}

impl PreviewConfig {
    /// The rules that apply to a pane running `harnesses`, in order.
    pub fn rules_for(&self, harnesses: &BTreeSet<String>) -> Vec<&PreviewRule> {
        self.rules
            .iter()
            .filter(|rule| {
                rule.harness
                    .as_ref()
                    .is_none_or(|harness| harnesses.contains(harness))
            })
            .collect()
    }
}

/// How `text` fits by the first of `rules` that matches it; smart wrap's
/// own fit when none does.
pub fn line_fit(rules: &[&PreviewRule], text: &str) -> LineFit {
    rules
        .iter()
        .find(|rule| rule.pattern.is_match(text))
        .map_or(LineFit::Smart, |rule| rule.fit)
}

/// The harness keys of what `mux`'s pane runs: the agent sessions linked
/// to it, the agent processes found in it, and a harness its pane command
/// or launch command names.
pub fn pane_harnesses(snapshot: &GraphSnapshot, mux: &MuxSessionId) -> BTreeSet<String> {
    let id = NodeId::MuxSession(mux.clone());
    let mut harnesses = BTreeSet::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        match link.relation {
            RelationKind::LinkedToMux if link.target_node_id() == Some(&id) => {
                if let NodeId::AgentSession(session) = &link.source {
                    harnesses.insert(session.harness_key.clone());
                }
            }
            RelationKind::MuxContainsProcess if link.source == id => {
                if let Some(GraphNode::RuntimeProcess(process)) = link
                    .target_node_id()
                    .and_then(|target| snapshot.find_node(target))
                    && let Some(harness) = &process.harness_key
                {
                    harnesses.insert(harness.clone());
                }
            }
            _ => {}
        }
    }
    if let Some(GraphNode::MuxSession(node)) = snapshot.find_node(&id) {
        for command in [&node.active_pane_command, &node.active_pane_start_command]
            .into_iter()
            .flatten()
        {
            if let Some(adapter) =
                crate::discovery::harness::adapter_in_command(command.split_whitespace())
            {
                harnesses.insert(adapter.harness_key().to_string());
            }
        }
    }
    harnesses
}

/// The token a fit is spelled with in `[[tui.preview.rules]] fit`.
pub fn fit_token(fit: LineFit) -> &'static str {
    match fit {
        LineFit::Smart => "smart",
        LineFit::Cut => "cut",
        LineFit::Wrap => "wrap",
        LineFit::RightAlign => "right-align",
        LineFit::Drop => "drop",
    }
}

/// The fit `raw` names; `None` for a token no fit has.
pub fn parse_fit(raw: &str) -> Option<LineFit> {
    LineFit::ALL
        .into_iter()
        .find(|fit| fit_token(*fit) == raw.trim())
}

/// `[tui.preview]` as a file holds it: raw values, so a wrong type is a
/// diagnostic rather than a failure of the whole file.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct PreviewFile {
    #[serde(default)]
    decoration: Option<toml::Value>,
    #[serde(default)]
    min_rule_run: Option<toml::Value>,
    #[serde(default)]
    shorten_rules: Option<toml::Value>,
    #[serde(default)]
    rules: Option<toml::Value>,
    /// Keys the schema doesn't name, so a typo is diagnosed.
    #[serde(flatten)]
    unknown: BTreeMap<String, toml::Value>,
}

/// Merge one file's `[tui.preview]` over `config`. A bad setting keeps the
/// earlier value; a bad rule is skipped. Returns what was wrong, one
/// message per problem.
pub(crate) fn merge(config: &mut PreviewConfig, file: PreviewFile) -> Vec<String> {
    let mut problems = Vec::new();
    for key in file.unknown.keys() {
        problems.push(format!(
            "unknown `[tui.preview]` key `{key}`; expected decoration, min_rule_run, \
             shorten_rules, or rules"
        ));
    }
    if let Some(value) = file.decoration {
        match parse_decoration(&value) {
            Ok(decoration) => config.smart.decoration = decoration,
            Err(problem) => problems.push(problem),
        }
    }
    if let Some(value) = file.min_rule_run {
        match value
            .as_integer()
            .and_then(|count| usize::try_from(count).ok())
            .filter(|count| *count >= 2)
        {
            Some(count) => config.smart.min_rule_run = count,
            None => problems.push(format!(
                "invalid `[tui.preview] min_rule_run` value `{}`; expected a whole number \
                 of at least 2",
                shown(&value)
            )),
        }
    }
    if let Some(value) = file.shorten_rules {
        match value.as_bool() {
            Some(shorten) => config.smart.shorten_rules = shorten,
            None => problems.push(format!(
                "invalid `[tui.preview] shorten_rules` value `{}`; expected true or false",
                shown(&value)
            )),
        }
    }
    if let Some(value) = file.rules {
        match value.as_array() {
            Some(entries) => merge_rules(&mut config.rules, entries, &mut problems),
            None => problems.push(
                "invalid `[tui.preview] rules`; expected `[[tui.preview.rules]]` tables"
                    .to_string(),
            ),
        }
    }
    problems
}

/// A string bare, anything else as TOML writes it.
fn shown(value: &toml::Value) -> String {
    match value.as_str() {
        Some(text) => text.to_string(),
        None => value.to_string(),
    }
}

/// `decoration`: a list of single characters and `a..b` ranges.
fn parse_decoration(value: &toml::Value) -> Result<Vec<std::ops::RangeInclusive<char>>, String> {
    let Some(entries) = value.as_array() else {
        return Err(format!(
            "invalid `[tui.preview] decoration` value `{}`; expected a list of characters \
             and ranges like \"a..z\"",
            shown(value)
        ));
    };
    entries
        .iter()
        .map(|entry| {
            entry.as_str().and_then(parse_range).ok_or_else(|| {
                format!(
                    "invalid `[tui.preview] decoration` entry `{}`; expected one character \
                     or a range like \"a..z\"",
                    shown(entry)
                )
            })
        })
        .collect()
}

/// One character, or two around `..` with the first not after the second.
fn parse_range(text: &str) -> Option<std::ops::RangeInclusive<char>> {
    let chars: Vec<char> = text.chars().collect();
    match chars.as_slice() {
        [c] => Some(*c..=*c),
        [from, '.', '.', to] if from <= to => Some(*from..=*to),
        _ => None,
    }
}

/// Merge one file's rules: each replaces an earlier rule of its name, a
/// disabled one only removes it, and the file's rules go ahead of the
/// earlier ones so they are checked first.
fn merge_rules(rules: &mut Vec<PreviewRule>, entries: &[toml::Value], problems: &mut Vec<String>) {
    let mut layer: Vec<PreviewRule> = Vec::new();
    let mut named: BTreeSet<String> = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let (name, rule) = match parse_rule(entry) {
            Ok(parsed) => parsed,
            Err(RuleProblem { name, problem }) => {
                problems.push(match name {
                    Some(name) => {
                        format!("`[[tui.preview.rules]]` rule `{name}` skipped: {problem}")
                    }
                    None => format!(
                        "`[[tui.preview.rules]]` entry {} skipped: {problem}",
                        index + 1
                    ),
                });
                continue;
            }
        };
        if !named.insert(name.clone()) {
            problems.push(format!(
                "`[[tui.preview.rules]]` names `{name}` twice; the later one applies"
            ));
            layer.retain(|earlier| earlier.name != name);
        }
        layer.extend(rule);
    }
    rules.retain(|rule| !named.contains(&rule.name));
    layer.append(rules);
    *rules = layer;
}

struct RuleProblem {
    name: Option<String>,
    problem: String,
}

/// One rule entry: its name, and the rule unless it is turned off.
fn parse_rule(entry: &toml::Value) -> Result<(String, Option<PreviewRule>), RuleProblem> {
    let fail = |name: Option<&str>, problem: String| RuleProblem {
        name: name.map(str::to_string),
        problem,
    };
    let Some(table) = entry.as_table() else {
        return Err(fail(None, "expected a table".to_string()));
    };
    let name = match table.get("name").map(|value| (value, value.as_str())) {
        Some((_, Some(name))) if !name.trim().is_empty() => name.trim().to_string(),
        Some((value, _)) => {
            return Err(fail(
                None,
                format!("its `name` `{}` is not a name", shown(value)),
            ));
        }
        None => return Err(fail(None, "it needs a `name`".to_string())),
    };
    let named = Some(name.as_str());
    // An unknown key could be a misspelled scope; the rule would then
    // apply to every pane, so it is skipped rather than guessed at.
    if let Some(key) = table.keys().find(|key| {
        !matches!(
            key.as_str(),
            "name" | "match" | "fit" | "harness" | "enabled"
        )
    }) {
        return Err(fail(
            named,
            format!("unknown key `{key}`; expected name, match, fit, harness, or enabled"),
        ));
    }
    match table.get("enabled").map(toml::Value::as_bool) {
        None | Some(Some(true)) => {}
        Some(Some(false)) => return Ok((name, None)),
        Some(None) => {
            return Err(fail(
                named,
                "its `enabled` must be true or false".to_string(),
            ));
        }
    }
    let pattern = match table.get("match").map(|value| (value, value.as_str())) {
        Some((_, Some(source))) => {
            Pattern::new(source).map_err(|why| fail(named, format!("its `match` {why}")))?
        }
        Some((value, None)) => {
            return Err(fail(
                named,
                format!("its `match` `{}` is not a pattern", shown(value)),
            ));
        }
        None => return Err(fail(named, "it needs a `match` pattern".to_string())),
    };
    let fit = match table.get("fit") {
        Some(value) => value.as_str().and_then(parse_fit).ok_or_else(|| {
            fail(
                named,
                format!(
                    "its `fit` `{}` is not one of {}",
                    shown(value),
                    LineFit::ALL.map(fit_token).join(", ")
                ),
            )
        })?,
        None => return Err(fail(named, "it needs a `fit`".to_string())),
    };
    let harness = match table.get("harness") {
        None => None,
        Some(value) => {
            let known = crate::discovery::harness::harness_keys();
            match value.as_str().map(str::trim) {
                Some(harness) if known.contains(&harness) => Some(harness.to_string()),
                _ => {
                    return Err(fail(
                        named,
                        format!(
                            "its `harness` `{}` is not one of {}",
                            shown(value),
                            known.join(", ")
                        ),
                    ));
                }
            }
        }
    };
    Ok((
        name.clone(),
        Some(PreviewRule {
            name,
            harness,
            pattern,
            fit,
        }),
    ))
}

static BUILTIN: LazyLock<PreviewConfig> = LazyLock::new(|| {
    builtin_over(PreviewConfig {
        smart: SmartWrap::default(),
        rules: Vec::new(),
    })
    .0
});

/// The built-in document merged over `base`, and what was wrong with it.
/// Tests hold it free of problems.
fn builtin_over(mut base: PreviewConfig) -> (PreviewConfig, Vec<String>) {
    #[derive(Deserialize)]
    struct Document {
        tui: DocumentTui,
    }
    #[derive(Deserialize)]
    struct DocumentTui {
        preview: PreviewFile,
    }
    let problems = match toml::from_str::<Document>(BUILTIN_DOCUMENT) {
        Ok(document) => merge(&mut base, document.tui.preview),
        Err(err) => vec![err.to_string()],
    };
    (base, problems)
}

#[cfg(test)]
#[path = "preview_rules_tests.rs"]
mod tests;
