//! The Codex hook entries in a `config.toml`: install, report, and
//! remove (ADR 0143 category 5, ADR 0206 decisions 2 and 6 to 9).
//!
//! Codex reads hooks from the `hooks` table of `config.toml`: for each
//! event a list of matcher groups, each with a list of handlers.
//! Conspectus owns one group per subscribed event ([`HOOK_EVENTS`]), with
//! no matcher and one command handler that runs `conspectus hook write
//! codex` with a 5 s timeout. A handler is Conspectus's when it is a
//! command hook whose command contains `hook write codex`, and a group
//! is Conspectus's when every handler in it is. Groups of the operator's
//! are never edited, even one holding a Conspectus handler beside their
//! own, and Conspectus's go after them, so an install never moves one of
//! the operator's.
//!
//! The file is edited in place, so comments, layout, and every other key
//! survive, in each form TOML allows for the lists: an array of tables
//! (`[[hooks.SessionStart]]`, the form Codex documents), inline arrays
//! under `[hooks]` or in dotted keys, or an inline `hooks = { ... }`
//! table. A new list takes the form of the table it goes into: inline in
//! an inline or dotted table or in one that already holds an inline
//! list, else an array of tables.
//!
//! - Install adds Conspectus's group to every event that lacks one and
//!   brings an older group up to date (no matcher, a timeout on every
//!   handler), so re-running it upgrades an older install and otherwise
//!   changes nothing. New handlers run the command given with
//!   `--command`, else the command the existing handlers run, else this
//!   binary's; `--command` also moves the existing handlers to it.
//! - Remove takes every Conspectus handler out of every event, then the
//!   groups and lists that leaves empty, and nothing else.
//! - Status reports each subscribed event as installed, outdated, or
//!   missing.
//!
//! A `hooks` value or subscribed event list of the wrong type is an
//! error for install, which writes nothing rather than replace it;
//! remove and status skip it. Keys of the hooks table that hold no list
//! are never touched, `hooks.state` among them, where Codex records the
//! hooks the operator trusted: trust is given in Codex's `/hooks`
//! review, and Conspectus never writes it (ADR 0206 decision 8). A
//! `hooks.json` beside the file is another source Codex reads; it is
//! read for a note and never written.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use super::hook_claude::{EventCoverage, HOOK_TIMEOUT_S};
use crate::discovery::harness::codex::HOOK_EVENTS;

/// The handler timeout, as the TOML integer Codex reads in seconds.
const TIMEOUT_S: i64 = HOOK_TIMEOUT_S as i64;

/// What `hook init codex` says after every run: current Codex runs a hook
/// it does not manage only once the operator has trusted it, and every
/// change to an entry needs that review again.
pub(super) const REVIEW_HINT: &str = "Recent Codex versions skip a new or changed hook until you \
     trust it: open /hooks in Codex and review the Conspectus entries.";

/// What `hook remove codex` says when an entry of the operator's moved up
/// a place in its list.
pub(super) const MOVED_NOTE: &str = "note: Codex records the trust you gave a hook by its place \
     in the list, so it may ask you to review the hooks that came after the removed entries \
     again in /hooks";

/// What an install changed, in subscribed events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CodexInstall {
    /// Events that got Conspectus's group.
    pub(super) added: usize,
    /// Events whose group of Conspectus's was brought up to date.
    pub(super) updated: usize,
}

impl CodexInstall {
    pub(super) fn changed(self) -> bool {
        self.added + self.updated > 0
    }
}

/// What a removal changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CodexRemoval {
    /// A Conspectus handler was taken out.
    pub(super) changed: bool,
    /// An entry of the operator's came after one that was taken out, so
    /// its place in its list moved up.
    pub(super) moved: bool,
}

/// The command `hook init` uses to tell its own handlers from others.
pub(super) fn is_conspectus_hook_command(command: &str) -> bool {
    command.contains("hook write codex")
}

/// Install Conspectus's hooks in `document`, a `config.toml`; see the
/// module documentation. `command` is the `--command` the operator gave,
/// and `default_command` the one a fresh install runs.
pub(super) fn ensure_codex_hook(
    document: &mut DocumentMut,
    command: Option<&str>,
    default_command: &str,
) -> Result<CodexInstall> {
    if let Some(hooks) = document.get("hooks") {
        let Some(hooks) = hooks.as_table_like() else {
            bail!("`hooks` in the config file is not a table; fix it by hand first");
        };
        for event in HOOK_EVENTS {
            if hooks.get(event).is_some_and(|list| !is_list(list)) {
                bail!("`hooks.{event}` in the config file is not a list; fix it by hand first");
            }
        }
    }
    let template = match command {
        Some(command) => command.to_string(),
        None => first_command(document).unwrap_or_else(|| default_command.to_string()),
    };
    let inline = new_lists_inline(document.get("hooks"));
    let hooks = document
        .entry("hooks")
        .or_insert_with(|| {
            // Implicit, so a file with no hooks gains no empty `[hooks]`.
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_like_mut()
        .expect("checked to be a table");
    let mut install = CodexInstall::default();
    for event in HOOK_EVENTS {
        let Some(list) = hooks.get_mut(event) else {
            hooks.insert(event, new_list(inline, &template));
            install.added += 1;
            continue;
        };
        let groups = tables(list);
        let own = groups
            .iter()
            .position(|group| is_own_group(*group) && matcher_covers(*group))
            .or_else(|| groups.iter().position(|group| is_own_group(*group)));
        if let Some(index) = own {
            let group = tables_mut(list)
                .into_iter()
                .nth(index)
                .expect("the group found above");
            if update_group(group, command) {
                install.updated += 1;
            }
        } else {
            push_group(list, &template);
            install.added += 1;
        }
    }
    Ok(install)
}

/// Whether `document` has a Conspectus handler for any event.
pub(super) fn has_codex_hook(document: &DocumentMut) -> bool {
    hooks_of(document).is_some_and(|hooks| {
        hooks
            .iter()
            .flat_map(|(_, list)| tables(list))
            .any(group_has_conspectus_handler)
    })
}

/// How each subscribed event stands in `document`, in install order.
pub(super) fn codex_hook_coverage(document: &DocumentMut) -> Vec<(&'static str, EventCoverage)> {
    let hooks = hooks_of(document);
    HOOK_EVENTS
        .iter()
        .map(|&event| {
            let groups = hooks
                .and_then(|hooks| hooks.get(event))
                .map(tables)
                .unwrap_or_default();
            let coverage = if groups
                .iter()
                .any(|group| is_own_group(*group) && is_current(*group))
            {
                EventCoverage::Installed
            } else if groups
                .iter()
                .any(|group| group_has_conspectus_handler(*group))
            {
                EventCoverage::Outdated
            } else {
                EventCoverage::Missing
            };
            (event, coverage)
        })
        .collect()
}

/// The label for the install in `document` as a whole, from its
/// `coverage`: `installed` when every event is, `not-installed` when the
/// file has no Conspectus handler at all, else `partial`.
pub(super) fn codex_coverage_label(
    document: &DocumentMut,
    coverage: &[(&str, EventCoverage)],
) -> &'static str {
    if coverage
        .iter()
        .all(|(_, event)| *event == EventCoverage::Installed)
    {
        "installed"
    } else if has_codex_hook(document) {
        "partial"
    } else {
        "not-installed"
    }
}

/// Remove every Conspectus handler from `document`, with the groups and
/// event lists that leaves empty; what was empty before stays, and so
/// does the hooks table itself.
pub(super) fn remove_codex_hook(document: &mut DocumentMut) -> CodexRemoval {
    let Some(hooks) = document.get_mut("hooks").and_then(Item::as_table_like_mut) else {
        return CodexRemoval::default();
    };
    let mut removal = CodexRemoval::default();
    let mut emptied = Vec::new();
    for (event, list) in hooks.iter_mut() {
        if !is_list(list) {
            continue;
        }
        let before = entries(list);
        let (changed, moved) = remove_from_list(list);
        removal.changed |= changed;
        removal.moved |= moved;
        if changed && before > 0 && entries(list) == 0 {
            emptied.push(event.get().to_string());
        }
    }
    for event in emptied {
        hooks.remove(&event);
    }
    removal
}

/// `document` as the text that replaces the file, once parsing it back
/// shows the install as it should be: every event installed after an
/// install, no Conspectus handler left after a removal.
pub(super) fn checked_text(document: &DocumentMut, installed: bool) -> Result<String> {
    let text = document.to_string();
    let reread: DocumentMut = text
        .parse()
        .context("the edited file would not parse; nothing was written")?;
    let as_expected = if installed {
        codex_hook_coverage(&reread)
            .iter()
            .all(|(_, coverage)| *coverage == EventCoverage::Installed)
    } else {
        !has_codex_hook(&reread)
    };
    if !as_expected {
        bail!("the edited file would not read back as intended; nothing was written");
    }
    Ok(text)
}

/// Which command reports a `hooks.json`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NoteFor {
    Init,
    Status,
    Remove,
}

/// A note about the `hooks.json` beside `config`, which Codex reads as
/// well and Conspectus never writes, or `None` when there is nothing to
/// say: no such file, or for a removal, one that runs no Conspectus hook.
pub(super) fn hooks_json_note(config: &Path, note_for: NoteFor) -> Option<String> {
    let (path, conspectus) = hooks_json_beside(config)?;
    let path = path.display();
    match (note_for, conspectus) {
        (NoteFor::Init | NoteFor::Status, false) => Some(format!(
            "note: {path} also holds Codex hooks; Codex runs those as well and may warn at \
             startup that this folder has both files"
        )),
        (NoteFor::Init, true) => Some(format!(
            "note: {path} also runs a Conspectus hook, which Conspectus leaves alone; Codex \
             runs both, so remove that one by hand"
        )),
        (NoteFor::Status, true) => Some(format!(
            "note: {path} also runs a Conspectus hook, which this report leaves out"
        )),
        (NoteFor::Remove, true) => Some(format!(
            "note: {path} still runs a Conspectus hook; Conspectus never edits that file, so \
             remove it there by hand"
        )),
        (NoteFor::Remove, false) => None,
    }
}

/// The `hooks.json` beside `config`, if there is one, and whether a
/// handler in it runs Conspectus's writer. A file that cannot be read or
/// parsed counts as running none.
pub(super) fn hooks_json_beside(config: &Path) -> Option<(PathBuf, bool)> {
    let path = config.with_file_name("hooks.json");
    if !path.exists() {
        return None;
    }
    let conspectus = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|value| json_runs_conspectus(&value));
    Some((path, conspectus))
}

/// Whether any object in `value` is a command hook running Conspectus's
/// writer.
fn json_runs_conspectus(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            (fields.get("type").and_then(serde_json::Value::as_str) == Some("command")
                && fields
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(is_conspectus_hook_command))
                || fields.values().any(json_runs_conspectus)
        }
        serde_json::Value::Array(items) => items.iter().any(json_runs_conspectus),
        _ => false,
    }
}

fn hooks_of(document: &DocumentMut) -> Option<&dyn TableLike> {
    document.get("hooks")?.as_table_like()
}

/// Whether `item` is a list of groups or handlers: an array of tables or
/// an inline array.
fn is_list(item: &Item) -> bool {
    matches!(item, Item::ArrayOfTables(_) | Item::Value(Value::Array(_)))
}

/// How many entries a list holds, tables or not.
fn entries(list: &Item) -> usize {
    match list {
        Item::ArrayOfTables(array) => array.len(),
        Item::Value(Value::Array(array)) => array.len(),
        _ => 0,
    }
}

/// The tables of a list, in order: the tables of an array of tables, or
/// the inline tables of an inline array. Anything else is skipped.
fn tables(list: &Item) -> Vec<&dyn TableLike> {
    match list {
        Item::ArrayOfTables(array) => array.iter().map(|table| table as &dyn TableLike).collect(),
        Item::Value(Value::Array(array)) => array
            .iter()
            .filter_map(Value::as_inline_table)
            .map(|table| table as &dyn TableLike)
            .collect(),
        _ => Vec::new(),
    }
}

fn tables_mut(list: &mut Item) -> Vec<&mut dyn TableLike> {
    match list {
        Item::ArrayOfTables(array) => array
            .iter_mut()
            .map(|table| table as &mut dyn TableLike)
            .collect(),
        Item::Value(Value::Array(array)) => array
            .iter_mut()
            .filter_map(Value::as_inline_table_mut)
            .map(|table| table as &mut dyn TableLike)
            .collect(),
        _ => Vec::new(),
    }
}

/// Keep the entries of a list `keep` accepts; it sees `None` for an
/// entry that is not a table.
fn retain_entries(list: &mut Item, mut keep: impl FnMut(Option<&dyn TableLike>) -> bool) {
    match list {
        Item::ArrayOfTables(array) => array.retain(|table| keep(Some(table))),
        Item::Value(Value::Array(array)) => {
            array
                .retain(|value| keep(value.as_inline_table().map(|table| table as &dyn TableLike)));
        }
        _ => {}
    }
}

fn handlers_of(group: &dyn TableLike) -> Vec<&dyn TableLike> {
    group.get("hooks").map(tables).unwrap_or_default()
}

/// Whether `handler` is a command hook that runs Conspectus's writer.
fn is_conspectus_handler(handler: &dyn TableLike) -> bool {
    handler.get("type").and_then(Item::as_str) == Some("command")
        && handler
            .get("command")
            .and_then(Item::as_str)
            .is_some_and(is_conspectus_hook_command)
}

fn group_has_conspectus_handler(group: &dyn TableLike) -> bool {
    handlers_of(group).into_iter().any(is_conspectus_handler)
}

/// Whether `group` is Conspectus's own: it has handlers, and each runs
/// Conspectus's writer.
fn is_own_group(group: &dyn TableLike) -> bool {
    let handlers = handlers_of(group);
    !handlers.is_empty() && handlers.into_iter().all(is_conspectus_handler)
}

/// Whether `group` fires on every occurrence of its event. Codex reads a
/// matcher as a regular expression, and only an absent or empty one is
/// sure to match everything.
fn matcher_covers(group: &dyn TableLike) -> bool {
    group
        .get("matcher")
        .is_none_or(|matcher| matcher.as_str() == Some(""))
}

fn has_timeout(handler: &dyn TableLike) -> bool {
    handler.get("timeout").is_some_and(|timeout| {
        timeout.as_integer().is_some_and(|seconds| seconds > 0)
            || timeout.as_float().is_some_and(|seconds| seconds > 0.0)
    })
}

/// Whether `group` is as an install leaves it.
fn is_current(group: &dyn TableLike) -> bool {
    matcher_covers(group) && handlers_of(group).into_iter().all(has_timeout)
}

/// The command of the first Conspectus handler in install order, which
/// new handlers copy.
fn first_command(document: &DocumentMut) -> Option<String> {
    let hooks = hooks_of(document)?;
    HOOK_EVENTS
        .iter()
        .filter_map(|event| hooks.get(event))
        .flat_map(tables)
        .flat_map(handlers_of)
        .find(|handler| is_conspectus_handler(*handler))
        .and_then(|handler| handler.get("command")?.as_str().map(str::to_string))
}

/// Whether a new event list goes inline into `hooks`: an inline table can
/// hold nothing else, a dotted one is written as keys, and a table that
/// already holds an inline list keeps to that form.
fn new_lists_inline(hooks: Option<&Item>) -> bool {
    match hooks {
        Some(Item::Value(Value::InlineTable(_))) => true,
        Some(Item::Table(table)) => {
            table.is_dotted()
                || table
                    .iter()
                    .any(|(_, item)| matches!(item, Item::Value(Value::Array(_))))
        }
        _ => false,
    }
}

/// Conspectus's handler. It runs synchronously, Codex's default: an
/// asynchronous hook may finish after the next event's, and the store
/// relies on a session's events arriving in order.
fn handler(command: &str) -> Value {
    let mut handler = InlineTable::new();
    handler.insert("type", Value::from("command"));
    handler.insert("command", Value::from(command));
    handler.insert("timeout", Value::from(TIMEOUT_S));
    Value::InlineTable(handler)
}

fn handlers(command: &str) -> Value {
    let mut handlers = Array::new();
    handlers.push(handler(command));
    Value::Array(handlers)
}

/// Conspectus's group as an element of an array of tables.
fn group_table(command: &str) -> Table {
    let mut group = Table::new();
    group.insert("hooks", Item::Value(handlers(command)));
    group
}

/// Conspectus's group as an element of an inline array.
fn group_inline(command: &str) -> Value {
    let mut group = InlineTable::new();
    group.insert("hooks", handlers(command));
    Value::InlineTable(group)
}

fn new_list(inline: bool, command: &str) -> Item {
    if inline {
        let mut list = Array::new();
        list.push(group_inline(command));
        Item::Value(Value::Array(list))
    } else {
        let mut list = ArrayOfTables::new();
        list.push(group_table(command));
        Item::ArrayOfTables(list)
    }
}

fn push_group(list: &mut Item, command: &str) {
    match list {
        Item::ArrayOfTables(array) => array.push(group_table(command)),
        Item::Value(Value::Array(array)) => array.push(group_inline(command)),
        _ => {}
    }
}

/// Bring Conspectus's `group` up to date: no matcher, a timeout on each
/// handler, and, when `rewrite` is given, the command it names. Returns
/// whether anything changed.
fn update_group(group: &mut dyn TableLike, rewrite: Option<&str>) -> bool {
    let mut changed = false;
    if !matcher_covers(group) {
        group.remove("matcher");
        changed = true;
    }
    if let Some(list) = group.get_mut("hooks") {
        for handler in tables_mut(list) {
            let mut handler_changed = false;
            if let Some(command) = rewrite
                && handler.get("command").and_then(Item::as_str) != Some(command)
            {
                handler.insert("command", toml_edit::value(command));
                handler_changed = true;
            }
            if !has_timeout(&*handler) {
                handler.insert("timeout", toml_edit::value(TIMEOUT_S));
                handler_changed = true;
            }
            if handler_changed {
                // The handler is Conspectus's, so its spacing may be
                // redone around the new key.
                handler.fmt();
                changed = true;
            }
        }
    }
    changed
}

/// Take Conspectus's handlers out of an event's list, and the groups that
/// leaves empty. Returns whether anything went, and whether an entry of
/// the operator's (a group, or a handler in one) came after one that
/// went.
fn remove_from_list(list: &mut Item) -> (bool, bool) {
    let mut goes = Vec::new();
    let mut changed = false;
    let mut moved = false;
    let mut visit = |group: Option<&mut dyn TableLike>| {
        let go = group.is_some_and(|group| {
            let (removed, handler_moved) = strip_handlers(group);
            changed |= removed > 0;
            moved |= handler_moved;
            removed > 0 && group.get("hooks").is_none_or(|list| entries(list) == 0)
        });
        goes.push(go);
    };
    match list {
        Item::ArrayOfTables(array) => {
            for table in array.iter_mut() {
                visit(Some(table as &mut dyn TableLike));
            }
        }
        Item::Value(Value::Array(array)) => {
            for value in array.iter_mut() {
                visit(
                    value
                        .as_inline_table_mut()
                        .map(|table| table as &mut dyn TableLike),
                );
            }
        }
        _ => return (false, false),
    }
    if let Some(first) = goes.iter().position(|go| *go) {
        moved |= goes[first..].iter().any(|go| !go);
    }
    let mut index = 0;
    retain_entries(list, |_| {
        let keep = !goes[index];
        index += 1;
        keep
    });
    (changed, moved)
}

/// Take Conspectus's handlers out of `group`. Returns how many went, and
/// whether a handler of the operator's came after one that went.
fn strip_handlers(group: &mut dyn TableLike) -> (usize, bool) {
    let Some(list) = group.get_mut("hooks") else {
        return (0, false);
    };
    let mut removed = 0;
    let mut moved = false;
    retain_entries(list, |handler| {
        if handler.is_some_and(is_conspectus_handler) {
            removed += 1;
            false
        } else {
            moved |= removed > 0;
            true
        }
    });
    (removed, moved)
}

#[cfg(test)]
#[path = "hook_codex_tests.rs"]
mod tests;
