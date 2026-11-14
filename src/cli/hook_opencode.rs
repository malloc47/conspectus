//! `conspectus hook status opencode`: whether opencode's own config lists
//! the Conspectus plugin (`plugins/opencode-hook`).
//!
//! There is no `hook init opencode`: the plugin is built from a checkout
//! and listed by hand (its README), so status only reads the files that
//! list plugins, and nothing else of opencode's (ADR 0230):
//!
//! | Scope | File | Key | What it loads |
//! | :- | :- | :- | :- |
//! | user | `$XDG_CONFIG_HOME/opencode/opencode.json`, `opencode.jsonc`, `config.json` | `plugin` | the 1.x plugin, in each TUI |
//! | user | the same files | `plugins` | the 2.x server entrypoint in the service, and the TUI entrypoint in each client |
//! | user | `$XDG_CONFIG_HOME/opencode/cli.json` | `plugins` | the 2.x TUI entrypoint alone, which reports the sessions its client shows |
//! | project | `opencode.json`, `opencode.jsonc`, and the same under `.opencode/` | `plugin`, `plugins` | as for the user scope |
//!
//! `$XDG_CONFIG_HOME` falls back to `~/.config`. The files are JSONC:
//! comments and trailing commas are dropped before they are parsed. An
//! entry is the plugin's when it names a path with an `opencode-hook`
//! component, or the `@conspectus/opencode-hook` package; a 2.x entry
//! may be a string or `{"package": ...}`. A file that cannot be parsed
//! is reported as such and counts for nothing.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde_json::Value;

/// How a config file lists plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileKind {
    /// `opencode.json[c]` or `config.json`: 1.x `plugin`, 2.x `plugins`.
    Config,
    /// `cli.json`: 2.x TUI-only `plugins`.
    Cli,
}

/// What a listing loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Loads {
    /// The 1.x plugin, from a `plugin` entry.
    V1,
    /// The 2.x server and TUI entrypoints, from a `plugins` entry of a
    /// config file.
    V2,
    /// The 2.x TUI entrypoint alone, from `cli.json`.
    V2Client,
}

impl Loads {
    /// The line's last field.
    pub(super) fn label(self) -> &'static str {
        match self {
            Loads::V1 => "1.x",
            Loads::V2 => "2.x",
            Loads::V2Client => "2.x client only",
        }
    }
}

/// What one config file says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FileStatus {
    /// It lists the plugin, loading each of these.
    Lists(Vec<Loads>),
    /// It exists and lists no Conspectus plugin.
    NotListed,
    /// It exists but is not a JSON(C) object.
    Unreadable(String),
}

/// The plugin config files status reads for a scope, in reporting order.
pub(super) fn config_files(user: bool) -> Result<Vec<(PathBuf, FileKind)>> {
    if user {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .ok_or_else(|| anyhow!("HOME or XDG_CONFIG_HOME is required for --scope user"))?
            .join("opencode");
        Ok(vec![
            (dir.join("opencode.json"), FileKind::Config),
            (dir.join("opencode.jsonc"), FileKind::Config),
            (dir.join("config.json"), FileKind::Config),
            (dir.join("cli.json"), FileKind::Cli),
        ])
    } else {
        let cwd = crate::cwd::for_default_target("--scope user")?;
        let hidden = cwd.join(".opencode");
        Ok(vec![
            (cwd.join("opencode.json"), FileKind::Config),
            (cwd.join("opencode.jsonc"), FileKind::Config),
            (hidden.join("opencode.json"), FileKind::Config),
            (hidden.join("opencode.jsonc"), FileKind::Config),
        ])
    }
}

/// What the file at `path` says, or `None` when there is no file.
pub(super) fn file_status(path: &Path, kind: FileKind) -> Result<Option<FileStatus>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(
                anyhow::Error::new(err).context(format!("failed to read {}", path.display()))
            );
        }
    };
    Ok(Some(status_of(&text, kind)))
}

/// What a config file's `text` says.
pub(super) fn status_of(text: &str, kind: FileKind) -> FileStatus {
    let document: Value = match serde_json::from_str(&strip_jsonc(text)) {
        Ok(document) => document,
        Err(err) => return FileStatus::Unreadable(err.to_string()),
    };
    let Some(object) = document.as_object() else {
        return FileStatus::Unreadable("not a JSON object".to_string());
    };
    let lists = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|entries| entries.iter().any(names_plugin))
    };
    let mut loads = Vec::new();
    match kind {
        FileKind::Config => {
            if lists("plugin") {
                loads.push(Loads::V1);
            }
            if lists("plugins") {
                loads.push(Loads::V2);
            }
        }
        FileKind::Cli => {
            if lists("plugins") {
                loads.push(Loads::V2Client);
            }
        }
    }
    if loads.is_empty() {
        FileStatus::NotListed
    } else {
        FileStatus::Lists(loads)
    }
}

/// Whether a plugin entry, a string or `{"package": ...}`, names the
/// Conspectus plugin.
pub(super) fn names_plugin(entry: &Value) -> bool {
    let spec = match entry {
        Value::String(spec) => spec.as_str(),
        Value::Object(fields) => match fields.get("package") {
            Some(Value::String(spec)) => spec.as_str(),
            _ => return false,
        },
        _ => return false,
    };
    let spec = spec.trim();
    // A scoped package name is the plugin's only under its own scope; a
    // bare name is some other package.
    if spec.starts_with('@') {
        return spec == "@conspectus/opencode-hook"
            || spec.starts_with("@conspectus/opencode-hook@");
    }
    spec.contains(['/', '\\'])
        && spec
            .split(['/', '\\'])
            .any(|component| component == "opencode-hook")
}

/// `text` with JSONC's comments and trailing commas removed, so
/// `serde_json` can read it. Strings are copied as they are.
pub(super) fn strip_jsonc(text: &str) -> String {
    drop_trailing_commas(&drop_comments(text))
}

fn drop_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => while chars.next_if(|next| *next != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn drop_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut index = 0;
    while let Some(&c) = chars.get(index) {
        index += 1;
        if in_string {
            out.push(c);
            match c {
                '\\' => {
                    out.extend(chars.get(index));
                    index += 1;
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        } else if c == ','
            && matches!(
                chars[index..].iter().find(|next| !next.is_whitespace()),
                Some('}' | ']')
            )
        {
            continue;
        }
        out.push(c);
    }
    out
}

/// What `hook status opencode` adds on stderr about where the plugin is
/// listed: nothing, or one note.
pub(super) fn listing_note(loads: &[Loads]) -> Option<&'static str> {
    let client = loads.contains(&Loads::V2Client);
    let server = loads.contains(&Loads::V2);
    match (client, server) {
        (true, false) => Some(
            "the plugin is listed only in cli.json: each opencode client reports the sessions it \
             has shown, and no other; list dist/v2 in opencode.jsonc instead to see every session",
        ),
        (true, true) => Some(
            "the plugin is listed in cli.json and in opencode's config: each client loads its TUI \
             entrypoint twice; remove the cli.json entry",
        ),
        _ => None,
    }
}

#[cfg(test)]
#[path = "hook_opencode_tests.rs"]
mod tests;
