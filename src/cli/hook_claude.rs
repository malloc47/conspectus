//! The Claude Code hook entries in a `settings.json`: install, report,
//! and remove (ADR 0143 category 5, ADR 0206 decisions 6, 7, and 9).
//!
//! Conspectus owns one matcher group per subscribed event, holding the
//! command hook that runs `conspectus hook write claude-code`, with the
//! matcher the event needs and a 5 s timeout. A handler is Conspectus's
//! when its command line (the shell form's `command`, or the exec form's
//! `command` and `args`) contains `hook write claude-code`, and a group is
//! Conspectus's when every handler in it is. Groups of the operator's are
//! never edited, even one holding a Conspectus handler beside their own:
//! Claude Code runs identical commands once, so a group of Conspectus's
//! own next to it costs nothing.
//!
//! - Install adds Conspectus's group to every event that lacks one and
//!   brings an older group up to date (its matcher, a timeout), so
//!   re-running it upgrades a session-start-only install and otherwise
//!   changes nothing. New handlers run the command given with
//!   `--command`, else the command the existing handlers run, else this
//!   binary's; `--command` also moves the existing handlers to it.
//! - Remove takes every Conspectus handler out of every event, then the
//!   groups and event lists that leaves empty, and nothing else.
//! - Status reports each subscribed event as installed, outdated, or
//!   missing.
//!
//! A hooks table or event list of the wrong type is an error for
//! install, which writes nothing rather than replace the operator's
//! value; remove and status skip it.

use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde_json::{Map, Value, json};

use crate::discovery::harness::claude_code::{HOOK_SUBSCRIPTIONS, HookSubscription};

/// The timeout, in seconds, of every handler Conspectus installs. It
/// bounds a writer that cannot finish; the writer bounds itself well
/// inside it.
pub(super) const HOOK_TIMEOUT_S: u64 = 5;

/// What an install changed, in subscribed events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ClaudeInstall {
    /// Events that got Conspectus's group.
    pub(super) added: usize,
    /// Events whose group of Conspectus's was brought up to date.
    pub(super) updated: usize,
}

impl ClaudeInstall {
    pub(super) fn changed(self) -> bool {
        self.added + self.updated > 0
    }
}

/// How one subscribed event stands in a settings file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EventCoverage {
    /// Conspectus's group is there, with a matcher that covers what the
    /// event needs and a timeout on every handler.
    Installed,
    /// A Conspectus handler is there, but not as an install leaves it:
    /// an older matcher, no timeout, or only inside a group of the
    /// operator's.
    Outdated,
    /// No Conspectus handler.
    Missing,
}

impl EventCoverage {
    pub(super) fn label(self) -> &'static str {
        match self {
            EventCoverage::Installed => "installed",
            EventCoverage::Outdated => "outdated",
            EventCoverage::Missing => "missing",
        }
    }
}

/// The label for the install in `document` as a whole, from its
/// `coverage`: `installed` when every event is, `not-installed` when the
/// file has no Conspectus handler at all, else `partial`.
pub(super) fn coverage_label(document: &Value, coverage: &[(&str, EventCoverage)]) -> &'static str {
    if coverage
        .iter()
        .all(|(_, event)| *event == EventCoverage::Installed)
    {
        "installed"
    } else if has_claude_hook(document) {
        "partial"
    } else {
        "not-installed"
    }
}

/// Install Conspectus's hooks in `document`, a settings object; see the
/// module documentation. `command` is the `--command` the operator gave,
/// and `default_command` the one a fresh install runs.
pub(super) fn ensure_claude_hook(
    document: &mut Value,
    command: Option<&str>,
    default_command: &str,
) -> Result<ClaudeInstall> {
    let Some(settings) = document.as_object_mut() else {
        bail!("the settings file does not hold a JSON object");
    };
    if let Some(hooks) = settings.get("hooks") {
        let Some(hooks) = hooks.as_object() else {
            bail!("`hooks` in the settings file is not an object; fix it by hand first");
        };
        for subscription in HOOK_SUBSCRIPTIONS {
            if hooks
                .get(subscription.event)
                .is_some_and(|groups| !groups.is_array())
            {
                bail!(
                    "`hooks.{}` in the settings file is not a list; fix it by hand first",
                    subscription.event
                );
            }
        }
    }
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("checked to be an object");
    let template = match command {
        Some(command) => shell_handler(command),
        None => first_handler(hooks).map_or_else(|| shell_handler(default_command), template_of),
    };
    let rewrite = command.map(|_| &template);
    let mut install = ClaudeInstall::default();
    for subscription in &HOOK_SUBSCRIPTIONS {
        let groups = hooks
            .entry(subscription.event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("checked to be a list");
        let own = groups
            .iter()
            .position(|group| is_own_group(group) && matcher_covers(group, subscription))
            .or_else(|| groups.iter().position(is_own_group));
        if let Some(index) = own {
            if update_group(&mut groups[index], subscription, rewrite) {
                install.updated += 1;
            }
        } else {
            groups.push(new_group(subscription, &template));
            install.added += 1;
        }
    }
    Ok(install)
}

/// Whether `document` has a Conspectus handler for any event.
pub(super) fn has_claude_hook(document: &Value) -> bool {
    hooks_of(document).is_some_and(|hooks| {
        hooks
            .values()
            .filter_map(Value::as_array)
            .flatten()
            .any(group_has_conspectus_handler)
    })
}

/// How each subscribed event stands in `document`, in install order.
pub(super) fn claude_hook_coverage(document: &Value) -> Vec<(&'static str, EventCoverage)> {
    let hooks = hooks_of(document);
    HOOK_SUBSCRIPTIONS
        .iter()
        .map(|subscription| {
            let groups = hooks
                .and_then(|hooks| hooks.get(subscription.event))
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice);
            let coverage = if groups
                .iter()
                .any(|group| is_own_group(group) && is_current(group, subscription))
            {
                EventCoverage::Installed
            } else if groups.iter().any(group_has_conspectus_handler) {
                EventCoverage::Outdated
            } else {
                EventCoverage::Missing
            };
            (subscription.event, coverage)
        })
        .collect()
}

/// Remove every Conspectus handler from `document`, with the groups and
/// event lists that leaves empty and a `hooks` table left with nothing;
/// what was empty before stays. Returns whether anything changed.
pub(super) fn remove_claude_hook(document: &mut Value) -> bool {
    let Some(settings) = document.as_object_mut() else {
        return false;
    };
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut changed = false;
    let mut emptied = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        let before = groups.len();
        let mut kept = Vec::with_capacity(before);
        for mut group in groups.drain(..) {
            let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                kept.push(group);
                continue;
            };
            let count = handlers.len();
            handlers.retain(|handler| !hook_is_conspectus_command(handler));
            if handlers.len() == count {
                kept.push(group);
                continue;
            }
            changed = true;
            if !handlers.is_empty() {
                kept.push(group);
            }
        }
        *groups = kept;
        if before > 0 && groups.is_empty() {
            emptied.push(event.clone());
        }
    }
    for event in emptied {
        hooks.remove(&event);
    }
    if changed && hooks.is_empty() {
        settings.remove("hooks");
    }
    changed
}

/// Whether a handler's command line is Conspectus's Claude Code writer.
pub(super) fn is_conspectus_hook_command(command: &str) -> bool {
    command.contains("hook write claude-code")
}

/// Whether `handler` is a command hook that runs Conspectus's writer,
/// in shell form (`command`) or exec form (`command` and `args`).
pub(super) fn hook_is_conspectus_command(handler: &Value) -> bool {
    handler.get("type").and_then(Value::as_str) == Some("command")
        && command_line(handler).is_some_and(|line| is_conspectus_hook_command(&line))
}

/// The command line a handler runs: its `command`, followed in exec form
/// by its `args`.
fn command_line(handler: &Value) -> Option<String> {
    command_line_in(handler.as_object()?)
}

fn command_line_in(handler: &Map<String, Value>) -> Option<String> {
    let command = handler.get("command")?.as_str()?;
    let Some(args) = handler.get("args").and_then(Value::as_array) else {
        return Some(command.to_string());
    };
    let mut line = command.to_string();
    for arg in args.iter().filter_map(Value::as_str) {
        line.push(' ');
        line.push_str(arg);
    }
    Some(line)
}

fn hooks_of(document: &Value) -> Option<&Map<String, Value>> {
    document.get("hooks").and_then(Value::as_object)
}

fn handlers_of(group: &Value) -> &[Value] {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
}

fn group_has_conspectus_handler(group: &Value) -> bool {
    handlers_of(group).iter().any(hook_is_conspectus_command)
}

/// Whether `group` is Conspectus's own: it has handlers, and each runs
/// Conspectus's writer.
fn is_own_group(group: &Value) -> bool {
    let handlers = handlers_of(group);
    !handlers.is_empty() && handlers.iter().all(hook_is_conspectus_command)
}

/// Whether `group` is as an install leaves it for `subscription`.
fn is_current(group: &Value, subscription: &HookSubscription) -> bool {
    matcher_covers(group, subscription) && handlers_of(group).iter().all(has_timeout)
}

fn has_timeout(handler: &Value) -> bool {
    handler.as_object().is_some_and(has_timeout_in)
}

fn has_timeout_in(handler: &Map<String, Value>) -> bool {
    handler
        .get("timeout")
        .and_then(Value::as_f64)
        .is_some_and(|timeout| timeout > 0.0)
}

/// Whether `group`'s matcher fires for everything `subscription` needs.
/// A missing, empty, or `*` matcher matches every occurrence, and a
/// matcher of only letters, digits, `_`, and `|` is a list of exact
/// names (Claude Code reads any other matcher as a regular expression,
/// which counts only when it is the expected one).
fn matcher_covers(group: &Value, subscription: &HookSubscription) -> bool {
    let actual = match group.get("matcher") {
        None => "",
        Some(Value::String(matcher)) => matcher.as_str(),
        Some(_) => return false,
    };
    if matches!(actual, "" | "*") {
        return true;
    }
    let Some(expected) = subscription.matcher else {
        return false;
    };
    if actual == expected {
        return true;
    }
    let exact_list = actual
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '|');
    if !exact_list {
        return false;
    }
    let names: BTreeSet<&str> = actual.split('|').collect();
    expected.split('|').all(|name| names.contains(name))
}

/// The handler a fresh install writes: shell form, so a Claude Code too
/// old for the exec form runs it too.
fn shell_handler(command: &str) -> Value {
    json!({ "type": "command", "command": command, "timeout": HOOK_TIMEOUT_S })
}

/// The first Conspectus handler in install order, whose command new
/// entries copy.
fn first_handler(hooks: &Map<String, Value>) -> Option<&Value> {
    HOOK_SUBSCRIPTIONS
        .iter()
        .filter_map(|subscription| hooks.get(subscription.event)?.as_array())
        .flatten()
        .flat_map(handlers_of)
        .find(|handler| hook_is_conspectus_command(handler))
}

/// A new handler running what `handler` runs, in the same form.
fn template_of(handler: &Value) -> Value {
    let mut template = Map::new();
    for key in ["type", "command", "args"] {
        if let Some(value) = handler.get(key) {
            template.insert(key.to_string(), value.clone());
        }
    }
    template.insert("timeout".to_string(), json!(HOOK_TIMEOUT_S));
    Value::Object(template)
}

fn new_group(subscription: &HookSubscription, handler: &Value) -> Value {
    let mut group = Map::new();
    if let Some(matcher) = subscription.matcher {
        group.insert("matcher".to_string(), json!(matcher));
    }
    group.insert("hooks".to_string(), json!([handler]));
    Value::Object(group)
}

/// Bring Conspectus's `group` up to date for `subscription`: the matcher,
/// a timeout on each handler, and, when `rewrite` is given, the command
/// it names. Returns whether anything changed.
fn update_group(
    group: &mut Value,
    subscription: &HookSubscription,
    rewrite: Option<&Value>,
) -> bool {
    let mut changed = false;
    if !matcher_covers(group, subscription) {
        let fields = group
            .as_object_mut()
            .expect("a group with handlers is an object");
        match subscription.matcher {
            Some(matcher) => fields.insert("matcher".to_string(), json!(matcher)),
            None => fields.remove("matcher"),
        };
        changed = true;
    }
    let handlers = group
        .get_mut("hooks")
        .and_then(Value::as_array_mut)
        .expect("a group of Conspectus's has handlers");
    for handler in handlers {
        let fields = handler
            .as_object_mut()
            .expect("a command hook is an object");
        if let Some(template) = rewrite
            && command_line(template) != command_line_in(fields)
        {
            fields.remove("args");
            fields.insert("command".to_string(), template["command"].clone());
            changed = true;
        }
        if !has_timeout_in(fields) {
            fields.insert("timeout".to_string(), json!(HOOK_TIMEOUT_S));
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
#[path = "hook_claude_tests.rs"]
mod tests;
