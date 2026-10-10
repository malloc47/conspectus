//! `conspectus hook` subcommand tree.
//!
//! Four subcommands: `write` (records a session-start hook
//! payload into the sidecar store), `init` / `remove` (wire
//! the hook binding into claude / codex config files), and
//! `status` (reports which config files carry the hook).
//!
//! Every helper this module needs is either hook-specific
//! (moved here alongside the args) or comes from external
//! crates. Nothing shared back through `super`.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand, ValueEnum};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::hook::{HookStore, HookTmuxRecord};

#[derive(Debug, Args)]
pub(super) struct HookArgs {
    #[command(subcommand)]
    command: HookCommand,
}

impl HookArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            HookCommand::Write(args) => args.run(),
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

#[derive(Debug, Args)]
struct HookWriteArgs {
    /// Registered harness key (e.g. `claude-code`, `codex`,
    /// `opencode`), as in `conspectus hook write claude-code`.
    harness: String,
    /// Override hook state root. Primarily useful for tests
    /// and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl HookWriteArgs {
    fn run(self) -> Result<()> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            bail!("{} hook payload was empty", self.harness);
        }
        let payload: serde_json::Value = serde_json::from_str(&input)
            .with_context(|| format!("failed to parse {} hook JSON", self.harness))?;
        let (pid, ppid) = harness_pid_pair(&self.harness);
        let harness_version = harness_version_env(&self.harness);
        let record = crate::hook::hook_record_from_payload(
            &self.harness,
            &payload,
            pid,
            ppid,
            tmux_context(),
            harness_version,
            crate::hook::current_epoch(),
        )?;
        write_or_ingest_hook_record(&record, self.state_root)?;
        Ok(())
    }
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
}

impl HookInitArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_install(),
            HookHarnessFlag::Codex => self.run_codex_install(),
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
}

impl HookStatusArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_status(),
            HookHarnessFlag::Codex => self.run_codex_status(),
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
}

impl HookRemoveArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_remove(),
            HookHarnessFlag::Codex => self.run_codex_remove(),
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
}

fn resolve_hook_state_root(override_root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = override_root {
        return Ok(root);
    }
    HookStore::from_env()
        .map(|store| store.root().to_path_buf())
        .ok_or_else(|| {
            anyhow!("no hook state root available; set HOME or CONSPECTUS_HOOK_SIDECAR_STATE")
        })
}

fn write_or_ingest_hook_record(
    record: &crate::hook::HookRecord,
    override_root: Option<PathBuf>,
) -> Result<()> {
    if let Some(root) = override_root {
        HookStore::new(root).write_record(record)?;
        return Ok(());
    }

    match crate::server::client_hook_ingest(record) {
        crate::server::ClientOutcome::Ok(()) => Ok(()),
        crate::server::ClientOutcome::NoDaemon => {
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        crate::server::ClientOutcome::DaemonError { code, message } => {
            if code != "snapshot_unavailable" {
                eprintln!("conspectus: warning: daemon refused hook ingest ({code}): {message}");
            }
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        crate::server::ClientOutcome::Transport(err) => {
            eprintln!(
                "conspectus: warning: daemon hook ingest failed, writing local hook spool: {err:#}"
            );
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn read_linux_parent_pid() -> Option<u32> {
    read_linux_ppid_of(std::process::id())
}

#[cfg(target_os = "linux")]
fn read_linux_ppid_of(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    let mut fields = after_name.split_whitespace();
    fields.next()?;
    fields.next()?.parse().ok()
}

#[cfg(target_os = "linux")]
fn read_linux_comm_of(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Resolve the (pid, ppid) of the harness process (e.g. `claude`,
/// `codex`, `opencode`) that triggered this hook invocation.
///
/// Walks the parent-pid chain from the current process upward, looking
/// for the first ancestor whose `/proc/<pid>/comm` matches one of
/// `expected_binaries`. This is necessary because claude / codex /
/// opencode launch the hook command via a short-lived shell wrapper
/// (e.g. `sh -c 'conspectus hook write …'`), so `std::process::id()`
/// returns the writer's pid — a process that exits within
/// milliseconds. The hook-sidecar discovery layer's liveness check
/// (`src/discovery/hook_sidecar.rs`) treats records with a dead pid
/// as ignored evidence, which would silently disable hook evidence
/// entirely.
///
/// Returns `None` when running on a non-Linux host, when the walk
/// exhausts its depth budget, or when no ancestor matches. Callers
/// must treat `None` as "harness pid unknown" and persist it as a
/// `None` pid in the hook record so the discovery liveness check
/// is skipped instead of failing.
fn resolve_harness_pid(expected_binaries: &[&str]) -> Option<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        let start = read_linux_parent_pid()?;
        resolve_harness_pid_with(start, expected_binaries, |pid| {
            Some((read_linux_comm_of(pid)?, read_linux_ppid_of(pid)?))
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = expected_binaries;
        None
    }
}

/// Pure walker used by `resolve_harness_pid`. Factored out so unit
/// tests can mock the `/proc` reader. Caps the walk at 8 hops to
/// prevent runaway recursion on a corrupted process table.
pub(super) fn resolve_harness_pid_with<F>(
    start_pid: u32,
    expected_binaries: &[&str],
    mut read: F,
) -> Option<(u32, u32)>
where
    F: FnMut(u32) -> Option<(String, u32)>,
{
    let mut pid = start_pid;
    for _ in 0..8 {
        if pid <= 1 {
            return None;
        }
        let (comm, ppid) = read(pid)?;
        if expected_binaries.iter().any(|name| comm == *name) {
            return Some((pid, ppid));
        }
        pid = ppid;
    }
    None
}

/// Process-name set Conspectus expects to see for each harness when
/// walking the parent-pid chain from a hook writer up to the live
/// agent process. Mirrors the harness keys recognized elsewhere in
/// the cross-link and process-tree code.
///
/// Reads from the adapter registry's
/// `RuntimeSignature::process_command_basenames` so a new
/// harness gets pid-pair resolution for free — no cli.rs
/// match-table edit required.
pub(super) fn harness_binaries(harness: &str) -> Vec<&'static str> {
    crate::discovery::harness::registered_adapters()
        .find(|a| a.harness_key() == harness)
        .map(|a| a.runtime_signature().process_command_basenames.to_vec())
        .unwrap_or_default()
}

/// Environment variable Conspectus consults for a harness's
/// version string when writing a hook sidecar record. Read
/// through a helper (rather than inline in `HookWriteArgs::run`)
/// so the per-harness mapping stays in one place.
fn harness_version_env(harness: &str) -> Option<String> {
    match harness {
        "claude-code" => std::env::var("CLAUDE_CODE_VERSION").ok(),
        "opencode" => std::env::var("CONSPECTUS_OPENCODE_HOOK_VERSION").ok(),
        _ => None,
    }
}

/// Resolve the `(pid, ppid)` pair to record on a hook sidecar entry
/// for `harness`. Returns `(None, None)` when the harness pid cannot
/// be identified — the discovery liveness check then skips the pid
/// branch entirely so the record stays Active rather than being
/// marked Ignored against a stillborn writer pid.
fn harness_pid_pair(harness: &str) -> (Option<i64>, Option<i64>) {
    match resolve_harness_pid(&harness_binaries(harness)) {
        Some((pid, ppid)) => (Some(i64::from(pid)), Some(i64::from(ppid))),
        None => (None, None),
    }
}

fn tmux_context() -> Option<HookTmuxRecord> {
    // Iterate the registered mux backends and ask
    // each for its current-session context. The first backend
    // to answer wins. `SystemTmux` reads `$TMUX` and runs `tmux
    // display-message`; other backends supply their own env-var
    // contract via the same trait method.
    let backends: Vec<Box<dyn crate::discovery::tmux::MuxBackend>> = vec![
        Box::new(crate::discovery::tmux::SystemTmux::new()),
        Box::new(crate::discovery::zellij::SystemZellij::new()),
    ];
    for backend in backends {
        if let Some(ctx) = backend.current_session_context() {
            let record = HookTmuxRecord {
                session_name: ctx.session_name,
                native_id: None,
                pane_id: ctx.pane_id,
                socket_path: ctx.namespace,
            };
            if !record.is_empty() {
                return Some(record);
            }
        }
    }
    None
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
