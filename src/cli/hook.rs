//! `conspectus hook` subcommand tree.
//!
//! Four subcommands: `write` (records one hook payload; it lives in
//! [`super::hook_write`]), `init` / `remove` (install or remove each
//! harness's hook: a settings entry for Claude Code and Codex, an
//! owned extension file for Pi), and `status` (reports whether the
//! hook is installed).
//!
//! Every helper this module needs is either hook-specific
//! (moved here alongside the args) or comes from external
//! crates. Nothing shared back through `super`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand, ValueEnum};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::hook_write::HookWriteArgs;

#[derive(Debug, Args)]
pub(super) struct HookArgs {
    #[command(subcommand)]
    command: HookCommand,
}

impl HookArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            HookCommand::Write(args) => {
                args.run();
                Ok(())
            }
            HookCommand::Init(args) => args.run(),
            HookCommand::Status(args) => args.run(),
            HookCommand::Remove(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum HookCommand {
    /// Write one hook observation from a harness payload on stdin.
    Write(HookWriteArgs),
    /// Install a Conspectus hook into harness configuration.
    Init(HookInitArgs),
    /// Report whether a Conspectus hook is installed.
    Status(HookStatusArgs),
    /// Remove a Conspectus hook from harness configuration.
    Remove(HookRemoveArgs),
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum HookScopeFlag {
    User,
    Project,
}

#[derive(Debug, Args)]
struct HookInitArgs {
    /// Harness whose hook config should be managed.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to inspect or mutate.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
    /// Print the action without writing files.
    #[arg(long)]
    dry_run: bool,
    /// Command installed into the harness config.
    #[arg(long = "command", value_name = "COMMAND")]
    command: Option<String>,
}

#[derive(Debug, Args)]
struct HookStatusArgs {
    /// Harness whose hook config should be inspected.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to inspect.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
}

#[derive(Debug, Args)]
struct HookRemoveArgs {
    /// Harness whose hook config should be managed.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to mutate.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
    /// Print the action without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum HookHarnessFlag {
    ClaudeCode,
    Codex,
    Pi,
}

impl HookInitArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_install(),
            HookHarnessFlag::Codex => self.run_codex_install(),
            HookHarnessFlag::Pi => self.run_pi_install(),
        }
    }

    fn run_claude_install(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let command = self
            .command
            .unwrap_or_else(|| default_hook_command("claude-code"));
        let mut document = read_json_document(&path)?;
        let changed = ensure_claude_hook(&mut document, &command);

        if self.dry_run {
            let verb = if changed {
                "would install"
            } else {
                "already installed"
            };
            println!("{verb} Claude Code hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_json_document(&path, &document)?;
            println!("installed Claude Code hook in {}", path.display());
        } else {
            println!("Claude Code hook already installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_install(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let command = self
            .command
            .unwrap_or_else(|| default_hook_command("codex"));
        let mut document = read_toml_document(&path)?;
        let changed = ensure_codex_hook(&mut document, &command);

        if self.dry_run {
            let verb = if changed {
                "would install"
            } else {
                "already installed"
            };
            println!("{verb} Codex hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_toml_document(&path, &document)?;
            println!("installed Codex hook in {}", path.display());
        } else {
            println!("Codex hook already installed in {}", path.display());
        }
        Ok(())
    }

    fn run_pi_install(self) -> Result<()> {
        if self.command.is_some() {
            bail!("--command doesn't apply to pi: the extension runs `conspectus hook write pi`");
        }
        let path = pi_extension_path(self.scope)?;
        let state = pi_extension_state(&path)?;
        if state == PiExtensionState::Foreign {
            bail!(
                "{} exists and wasn't written by Conspectus; move it aside to install the hook",
                path.display()
            );
        }
        let changed = state != PiExtensionState::Installed;

        if self.dry_run {
            let verb = if changed {
                "would install"
            } else {
                "already installed"
            };
            println!("{verb} Pi hook extension at {}", path.display());
            return Ok(());
        }
        if changed {
            write_pi_extension(&path)?;
            println!("installed Pi hook extension at {}", path.display());
        } else {
            println!("Pi hook extension already installed at {}", path.display());
        }
        Ok(())
    }
}

impl HookStatusArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_status(),
            HookHarnessFlag::Codex => self.run_codex_status(),
            HookHarnessFlag::Pi => self.run_pi_status(),
        }
    }

    fn run_claude_status(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let document = read_json_document(&path)?;
        if has_claude_hook(&document) {
            println!("installed\tclaude-code\t{}", path.display());
        } else {
            println!("not-installed\tclaude-code\t{}", path.display());
        }
        Ok(())
    }

    fn run_codex_status(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let document = read_toml_document(&path)?;
        if has_codex_hook(&document) {
            println!("installed\tcodex\t{}", path.display());
        } else {
            println!("not-installed\tcodex\t{}", path.display());
        }
        Ok(())
    }

    fn run_pi_status(self) -> Result<()> {
        let path = pi_extension_path(self.scope)?;
        let label = match pi_extension_state(&path)? {
            PiExtensionState::Installed => "installed",
            PiExtensionState::Outdated => "outdated",
            PiExtensionState::Absent => "not-installed",
            PiExtensionState::Foreign => "foreign",
        };
        println!("{label}\tpi\t{}", path.display());
        Ok(())
    }
}

impl HookRemoveArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_remove(),
            HookHarnessFlag::Codex => self.run_codex_remove(),
            HookHarnessFlag::Pi => self.run_pi_remove(),
        }
    }

    fn run_claude_remove(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let mut document = read_json_document(&path)?;
        let changed = remove_claude_hook(&mut document);

        if self.dry_run {
            let verb = if changed {
                "would remove"
            } else {
                "not installed"
            };
            println!("{verb} Claude Code hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_json_document(&path, &document)?;
            println!("removed Claude Code hook from {}", path.display());
        } else {
            println!("Claude Code hook not installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_remove(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let mut document = read_toml_document(&path)?;
        let changed = remove_codex_hook(&mut document);

        if self.dry_run {
            let verb = if changed {
                "would remove"
            } else {
                "not installed"
            };
            println!("{verb} Codex hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_toml_document(&path, &document)?;
            println!("removed Codex hook from {}", path.display());
        } else {
            println!("Codex hook not installed in {}", path.display());
        }
        Ok(())
    }

    fn run_pi_remove(self) -> Result<()> {
        let path = pi_extension_path(self.scope)?;
        let state = pi_extension_state(&path)?;
        if state == PiExtensionState::Foreign {
            println!(
                "{} wasn't written by Conspectus; leaving it in place",
                path.display()
            );
            return Ok(());
        }
        let installed = state != PiExtensionState::Absent;

        if self.dry_run {
            let verb = if installed {
                "would remove"
            } else {
                "not installed"
            };
            println!("{verb} Pi hook extension at {}", path.display());
            return Ok(());
        }
        if installed {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            println!("removed Pi hook extension from {}", path.display());
        } else {
            println!("Pi hook extension not installed at {}", path.display());
        }
        Ok(())
    }
}

fn claude_settings_path(scope: HookScopeFlag) -> Result<PathBuf> {
    match scope {
        HookScopeFlag::User => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("HOME is required for --scope user"))?;
            Ok(home.join(".claude").join("settings.json"))
        }
        HookScopeFlag::Project => {
            let cwd = crate::cwd::for_default_target("--scope user")?;
            Ok(cwd.join(".claude").join("settings.json"))
        }
    }
}

fn codex_config_path(scope: HookScopeFlag) -> Result<PathBuf> {
    match scope {
        HookScopeFlag::User => {
            let home = if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
                PathBuf::from(codex_home)
            } else {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow!("HOME or CODEX_HOME is required for --scope user"))?
                    .join(".codex")
            };
            Ok(home.join("config.toml"))
        }
        HookScopeFlag::Project => {
            let cwd = crate::cwd::for_default_target("--scope user")?;
            Ok(cwd.join(".codex").join("config.toml"))
        }
    }
}

/// The Pi hook extension, embedded so `hook init pi` installs the copy
/// that matches this binary's writer (ADR 0121).
pub(super) const PI_EXTENSION_SOURCE: &str =
    include_str!("../../plugins/pi-hook/conspectus-hook.ts");

/// First-line prefix that marks a Pi extension file as Conspectus's own.
/// `hook init` replaces and `hook remove` deletes only marked files.
const PI_EXTENSION_MARKER: &str = "// conspectus-hook:";

const PI_EXTENSION_FILE: &str = "conspectus-hook.ts";

/// What sits at the Pi extension path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PiExtensionState {
    /// Nothing there.
    Absent,
    /// Conspectus's extension, identical to the embedded copy.
    Installed,
    /// Conspectus's extension from another version.
    Outdated,
    /// A file Conspectus didn't write.
    Foreign,
}

/// `<agent-dir>/extensions/conspectus-hook.ts` for `--scope user`, where
/// `<agent-dir>` is `PI_CODING_AGENT_DIR` or `~/.pi/agent`, and
/// `.pi/extensions/conspectus-hook.ts` under the current directory for
/// `--scope project`.
fn pi_extension_path(scope: HookScopeFlag) -> Result<PathBuf> {
    match scope {
        HookScopeFlag::User => {
            let agent_dir =
                crate::discovery::harness::pi::agent_dir_from_env().ok_or_else(|| {
                    anyhow!("HOME or PI_CODING_AGENT_DIR is required for --scope user")
                })?;
            Ok(agent_dir.join("extensions").join(PI_EXTENSION_FILE))
        }
        HookScopeFlag::Project => {
            let cwd = crate::cwd::for_default_target("--scope user")?;
            Ok(cwd.join(".pi").join("extensions").join(PI_EXTENSION_FILE))
        }
    }
}

pub(super) fn pi_extension_state(path: &Path) -> Result<PiExtensionState> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(PiExtensionState::Absent),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    Ok(if text == PI_EXTENSION_SOURCE {
        PiExtensionState::Installed
    } else if text.starts_with(PI_EXTENSION_MARKER) {
        PiExtensionState::Outdated
    } else {
        PiExtensionState::Foreign
    })
}

fn write_pi_extension(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let tmp = path.with_extension("ts.tmp");
    fs::write(&tmp, PI_EXTENSION_SOURCE)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn default_hook_command(harness: &str) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "conspectus".to_string());
    format!("{} hook write {harness}", shell_quote(&program))
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn read_json_document(path: &Path) -> Result<serde_json::Value> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let value: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse JSON {}", path.display()))?;
            if value.is_object() {
                Ok(value)
            } else {
                bail!("{} must contain a JSON object", path.display());
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_json_document(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)? + "\n";
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn read_toml_document(path: &Path) -> Result<DocumentMut> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .with_context(|| format!("failed to parse TOML {}", path.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_toml_document(path: &Path, value: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = value.to_string();
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

pub(super) fn ensure_claude_hook(document: &mut serde_json::Value, command: &str) -> bool {
    if has_claude_hook(document) {
        return false;
    }

    let object = document
        .as_object_mut()
        .expect("settings document is object");
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() {
        *hooks = serde_json::json!({});
    }
    let hooks_object = hooks.as_object_mut().expect("hooks is object");
    let session_start = hooks_object
        .entry("SessionStart")
        .or_insert_with(|| serde_json::json!([]));
    if !session_start.is_array() {
        *session_start = serde_json::json!([]);
    }
    session_start
        .as_array_mut()
        .expect("SessionStart is array")
        .push(serde_json::json!({
            "matcher": "resume|startup|clear|compact",
            "hooks": [
                {
                    "type": "command",
                    "command": command
                }
            ]
        }));
    true
}

pub(super) fn has_claude_hook(document: &serde_json::Value) -> bool {
    document
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| entries.iter().any(entry_contains_conspectus_hook))
}

pub(super) fn remove_claude_hook(document: &mut serde_json::Value) -> bool {
    let Some(entries) = document
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("SessionStart"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };

    let mut changed = false;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry
            .get_mut("hooks")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let original_len = hooks.len();
        hooks.retain(|hook| !hook_is_conspectus_command(hook));
        changed |= hooks.len() != original_len;
    }
    entries.retain(|entry| {
        entry
            .get("hooks")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|hooks| !hooks.is_empty())
    });
    changed
}

fn entry_contains_conspectus_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|hooks| hooks.iter().any(hook_is_conspectus_command))
}

fn hook_is_conspectus_command(hook: &serde_json::Value) -> bool {
    hook.get("type").and_then(serde_json::Value::as_str) == Some("command")
        && hook
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_conspectus_hook_command)
}

fn is_conspectus_hook_command(command: &str) -> bool {
    command.contains("hook write claude-code")
}

fn ensure_codex_hook(document: &mut DocumentMut, command: &str) -> bool {
    if has_codex_hook(document) {
        return false;
    }

    let hooks = document
        .entry("hooks")
        .or_insert_with(|| Item::Table(Table::new()));
    if !hooks.is_table() {
        *hooks = Item::Table(Table::new());
    }
    let hooks_table = hooks.as_table_mut().expect("hooks is table");
    let session_start = hooks_table
        .entry("SessionStart")
        .or_insert_with(|| Item::Value(Value::Array(Array::new())));
    if !session_start.is_array() {
        *session_start = Item::Value(Value::Array(Array::new()));
    }
    session_start
        .as_array_mut()
        .expect("SessionStart is array")
        .push(codex_hook_entry_value(command));
    true
}

fn has_codex_hook(document: &DocumentMut) -> bool {
    document
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(Item::as_array)
        .is_some_and(|entries| entries.iter().any(codex_entry_contains_conspectus_hook))
}

fn remove_codex_hook(document: &mut DocumentMut) -> bool {
    let Some(entries) = document
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("SessionStart"))
        .and_then(Item::as_array_mut)
    else {
        return false;
    };

    let mut changed = false;
    let retained: Vec<Value> = entries
        .iter()
        .filter_map(|entry| {
            let mut entry = entry.clone();
            if remove_codex_hooks_from_entry(&mut entry) {
                changed = true;
            }
            (!codex_entry_hooks_empty(&entry)).then_some(entry)
        })
        .collect();
    if retained.len() != entries.len() {
        changed = true;
    }
    if changed {
        entries.clear();
        for entry in retained {
            entries.push(entry);
        }
    }
    changed
}

fn codex_hook_entry_value(command: &str) -> Value {
    let mut hook = toml_edit::InlineTable::new();
    hook.insert("type", Value::from("command"));
    hook.insert("command", Value::from(command));
    hook.insert("async", Value::from(false));

    let mut hooks = Array::new();
    hooks.push(Value::InlineTable(hook));

    let mut entry = toml_edit::InlineTable::new();
    entry.insert("hooks", Value::Array(hooks));
    Value::InlineTable(entry)
}

fn codex_entry_contains_conspectus_hook(entry: &Value) -> bool {
    entry
        .as_inline_table()
        .and_then(|table| table.get("hooks"))
        .and_then(Value::as_array)
        .is_some_and(|hooks| hooks.iter().any(codex_hook_is_conspectus_command))
}

fn codex_hook_is_conspectus_command(hook: &Value) -> bool {
    hook.as_inline_table().is_some_and(|table| {
        table.get("type").and_then(Value::as_str) == Some("command")
            && table
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.contains("hook write codex"))
    })
}

fn remove_codex_hooks_from_entry(entry: &mut Value) -> bool {
    let Some(hooks) = entry
        .as_inline_table_mut()
        .and_then(|table| table.get_mut("hooks"))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let original_len = hooks.len();
    let retained: Vec<Value> = hooks
        .iter()
        .filter(|hook| !codex_hook_is_conspectus_command(hook))
        .cloned()
        .collect();
    if retained.len() == original_len {
        return false;
    }
    hooks.clear();
    for hook in retained {
        hooks.push(hook);
    }
    true
}

fn codex_entry_hooks_empty(entry: &Value) -> bool {
    entry
        .as_inline_table()
        .and_then(|table| table.get("hooks"))
        .and_then(Value::as_array)
        .is_none_or(Array::is_empty)
}
