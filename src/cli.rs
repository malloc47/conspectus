use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, Stdio};
use std::str::FromStr;
use std::time::Duration;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use conspectus::aliases::{
    AliasEntry, AliasesDocument, parse_aliases_document, remove_alias_entry, upsert_alias_entry,
};
use conspectus::config::{self, ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{
    DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredStoreKind, DeclaredStoreSelection,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, select_store_for_declaration, upsert_declared_link,
};
use conspectus::discovery::tmux::{SystemTmux, TmuxRenameOutcome, TmuxRunner};
use conspectus::hook::{HookStore, HookTmuxRecord};
use conspectus::model::{GraphLink, GraphSnapshot, LinkEndpoint, NodeId, Provenance, RelationKind};
use conspectus::rename::{MuxNativeRename, RenamePlan, plan_session_rename};

#[derive(Debug, Parser)]
#[command(name = "conspectus", version, about = "AI work graph status tool")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command.unwrap_or(Command::Graph(GraphArgs::default())) {
            Command::Graph(args) => args.run(),
            Command::Table(args) => args.run(),
            Command::Declared(args) => args.run(),
            Command::Node(args) => args.run(),
            Command::Columns(args) => args.run(),
            Command::Tui(args) => args.run(),
            Command::Hook(args) => args.run(),
            Command::Rename(args) => args.run(),
            Command::Alias(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the current work graph.
    Graph(GraphArgs),
    /// Render a tabular projection of the resolved graph.
    Table(TableArgs),
    /// Inspect or author declared graph links.
    Declared(Box<DeclaredArgs>),
    /// Inspect a single node and its surrounding links.
    Node(NodeArgs),
    /// List registered columns for a `conspectus table <ROWS>` row-type.
    Columns(ColumnsArgs),
    /// Open the interactive terminal UI.
    Tui(TuiArgs),
    /// Write or install harness hook integrations.
    Hook(HookArgs),
    /// Rename an agent session or a tmux session.
    Rename(RenameArgs),
    /// Inspect operator-authored session aliases.
    Alias(AliasArgs),
}

#[derive(Debug, Args)]
struct ColumnsArgs {
    /// Row-type whose registered columns to list. Accepts the same
    /// tokens as `conspectus table <ROWS>` (sessions, mux, union,
    /// prs, forks).
    row_type: String,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. `auto` (default) emits ANSI only
    /// when stdout is a TTY (and respects `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces color on;
    /// `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl ColumnsArgs {
    fn run(self) -> Result<()> {
        let projection = match config::Projection::parse(&self.row_type) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let listing = conspectus::output::table::render_columns_listing(projection, color);
        print_paged(
            &listing,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct HookArgs {
    #[command(subcommand)]
    command: HookCommand,
}

impl HookArgs {
    fn run(self) -> Result<()> {
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
    #[command(subcommand)]
    harness: HookWriteHarness,
}

impl HookWriteArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookWriteHarness::ClaudeCode(args) => args.run(),
            HookWriteHarness::Codex(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum HookWriteHarness {
    /// Read Claude Code hook JSON from stdin and write a hook observation.
    ClaudeCode(ClaudeHookWriteArgs),
    /// Read Codex hook JSON from stdin and write a hook observation.
    Codex(CodexHookWriteArgs),
}

#[derive(Debug, Args)]
struct ClaudeHookWriteArgs {
    /// Override hook state root. Primarily useful for tests and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl ClaudeHookWriteArgs {
    fn run(self) -> Result<()> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            bail!("Claude Code hook payload was empty");
        }
        let payload: serde_json::Value =
            serde_json::from_str(&input).context("failed to parse Claude Code hook JSON")?;
        let root = resolve_hook_state_root(self.state_root)?;
        let record = conspectus::hook::claude_code_record_from_payload(
            &payload,
            i64::from(std::process::id()),
            i64::from(parent_pid()),
            tmux_context(),
            std::env::var("CLAUDE_CODE_VERSION").ok(),
            conspectus::hook::current_epoch(),
        )?;
        HookStore::new(root).write_record(&record)?;
        Ok(())
    }
}

#[derive(Debug, Args)]
struct CodexHookWriteArgs {
    /// Override hook state root. Primarily useful for tests and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl CodexHookWriteArgs {
    fn run(self) -> Result<()> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            bail!("Codex hook payload was empty");
        }
        let payload: serde_json::Value =
            serde_json::from_str(&input).context("failed to parse Codex hook JSON")?;
        let root = resolve_hook_state_root(self.state_root)?;
        let record = conspectus::hook::codex_record_from_payload(
            &payload,
            i64::from(std::process::id()),
            i64::from(parent_pid()),
            tmux_context(),
            None,
            conspectus::hook::current_epoch(),
        )?;
        HookStore::new(root).write_record(&record)?;
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

fn parent_pid() -> u32 {
    #[cfg(target_os = "linux")]
    {
        read_linux_parent_pid().unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

#[cfg(target_os = "linux")]
fn read_linux_parent_pid() -> Option<u32> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    let mut fields = after_name.split_whitespace();
    fields.next()?;
    fields.next()?.parse().ok()
}

fn tmux_context() -> Option<HookTmuxRecord> {
    std::env::var_os("TMUX")?;
    let socket_path = std::env::var("TMUX")
        .ok()
        .and_then(|value| value.split_once(',').map(|(socket, _)| socket.to_string()));
    let record = HookTmuxRecord {
        session_name: tmux_value("#{session_name}"),
        native_id: None,
        pane_id: tmux_value("#{pane_id}"),
        socket_path,
    };
    (!record.is_empty()).then_some(record)
}

fn tmux_value(format: &str) -> Option<String> {
    let output = ProcCommand::new("tmux")
        .arg("display-message")
        .arg("-p")
        .arg(format)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
            let cwd = std::env::current_dir()?;
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
            let cwd = std::env::current_dir()?;
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

fn ensure_claude_hook(document: &mut serde_json::Value, command: &str) -> bool {
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

fn has_claude_hook(document: &serde_json::Value) -> bool {
    document
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| entries.iter().any(entry_contains_conspectus_hook))
}

fn remove_claude_hook(document: &mut serde_json::Value) -> bool {
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

#[derive(Debug, Args)]
struct NodeArgs {
    #[command(subcommand)]
    command: NodeCommand,
}

impl NodeArgs {
    fn run(self) -> Result<()> {
        match self.command {
            NodeCommand::Show(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    /// Print a single node, its candidate links, resolved relationships,
    /// source metadata, and any diagnostics touching it.
    Show(NodeShowArgs),
}

#[derive(Debug, Args)]
struct NodeShowArgs {
    /// Node id. Accepts the short content-addressed prefix from the
    /// session table's `ID` column, the full `NodeId` display form
    /// (e.g. `agent_session:codex:/state:session-x`), or the harness/mux
    /// label (e.g. `codex:session-x`, `tmux:editor`).
    id: String,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. See `conspectus table --help` for
    /// the resolution rules.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl NodeShowArgs {
    fn run(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let id = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let rendered = conspectus::output::node_show::render_node_show(&snapshot, &id, color);
        print_paged(
            &rendered,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct GraphArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl Default for GraphArgs {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            scan_roots: Vec::new(),
        }
    }
}

impl GraphArgs {
    fn run(self) -> Result<()> {
        match self.format {
            OutputFormat::Json => {
                let snapshot = if self.scan_roots.is_empty() {
                    conspectus::discovery::discover_local_at_roots([std::env::current_dir()?])?
                } else {
                    conspectus::discovery::discover_local_at_roots(self.scan_roots)?
                };
                let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
                println!("{}", conspectus::output::render_graph_json(&snapshot)?);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Args)]
struct TableArgs {
    #[command(subcommand)]
    command: TableCommand,
}

impl TableArgs {
    fn run(self) -> Result<()> {
        match self.command {
            TableCommand::Sessions(args) => args.run(config::Projection::Agent),
            TableCommand::Mux(args) => args.run(config::Projection::Mux),
            TableCommand::Union(args) => args.run(config::Projection::Union),
            TableCommand::Prs(args) => args.run(config::Projection::Pr),
            TableCommand::Forks(args) => args.run(config::Projection::Fork),
        }
    }
}

#[derive(Debug, Subcommand)]
enum TableCommand {
    /// Agent sessions, one per row.
    Sessions(TableRowsArgs),
    /// Mux (terminal multiplexer) sessions, one per row.
    Mux(TableRowsArgs),
    /// Mixed projection: one row per node, preserving relationship status.
    Union(TableRowsArgs),
    /// Forge pull requests, one per row.
    Prs(TableRowsArgs),
    /// Forks recorded by Atelier or other fork-tracking providers.
    Forks(TableRowsArgs),
}

#[derive(Debug, Args, Default)]
struct TableRowsArgs {
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Force untruncated output even when stdout is a TTY. Conflicts
    /// with `--width`.
    #[arg(long, conflicts_with = "width")]
    wide: bool,
    /// Render at exactly this many columns. Useful for reproducible
    /// captures and snapshot tests.
    #[arg(long, value_name = "N")]
    width: Option<usize>,
    /// Row layout. `columnar` (default) renders one row per line;
    /// `card` renders one column per line with blank lines between
    /// rows, similar to `git log` default formatting.
    #[arg(long, value_enum, default_value_t = LayoutFlag::Columnar)]
    layout: LayoutFlag,
    /// Comma-separated column selection. Tokens: `default` / `all`
    /// reset the running set; `+name` adds; `-name` removes; bare
    /// names switch to explicit-list mode. Unknown names error with
    /// the registered list for the row-type. Overrides the
    /// `[table.<rows>].columns` config when both are present.
    #[arg(long, value_name = "LIST")]
    columns: Option<String>,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    /// Conflicts with `--no-pager`.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. `auto` (default) emits ANSI only
    /// when stdout is a TTY (and respects `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces color on;
    /// `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
    /// Filter / grouping flags (ADR 0031). The flags are recognized
    /// today; `table` will start applying them in F8-010.
    #[command(flatten)]
    filter_args: FilterArgs,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum LayoutFlag {
    #[default]
    Columnar,
    Card,
}

impl TableRowsArgs {
    fn run(self, projection: config::Projection) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let loader = config::ConfigLoader::from_env();
        let outcome = loader.load_from(&cwd);
        for diagnostic in &outcome.diagnostics {
            eprintln!(
                "conspectus: warning: {}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }

        let columns = resolve_columns_selection(
            projection,
            self.columns.as_deref(),
            row_config(projection, &outcome.config).columns.as_deref(),
        );
        let columns = match columns {
            Ok(value) => value,
            Err(err) => {
                eprintln!("conspectus: {err}");
                std::process::exit(2);
            }
        };

        // Resolve the active filter from CLI flags. Future config
        // parity (load from `[table.<rows>].filters` or
        // `[tui.views.sessions]`) lands as a follow-up; for v1 the
        // CLI flags are the only source so the static table matches
        // what the operator typed.
        let cli_filter = self.filter_args.to_row_filter()?;
        let now_epoch = current_unix_epoch_for_table();

        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let render_width = resolve_table_width(self.wide, self.width, &io::stdout());
        let mut options = match (self.layout, render_width) {
            (LayoutFlag::Columnar, Some(w)) => {
                conspectus::output::table::RenderOptions::columnar_width(w)
            }
            (LayoutFlag::Columnar, None) => conspectus::output::table::RenderOptions::wide(),
            (LayoutFlag::Card, Some(w)) => conspectus::output::table::RenderOptions::card_width(w),
            (LayoutFlag::Card, None) => conspectus::output::table::RenderOptions::card(),
        };
        if let Some(columns) = columns {
            options = options.with_columns(columns);
        }
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        options = options
            .with_color(color)
            .with_filter(cli_filter)
            .with_now_epoch(now_epoch);
        let table = conspectus::output::table::render_with(&snapshot, projection, &options);
        print_paged(&table, PagerOptions::from_flags(self.pager, self.no_pager));
        Ok(())
    }
}

fn current_unix_epoch_for_table() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

/// Resolve which column set to render, with CLI overriding config.
/// Returns `Ok(None)` when neither source is set, signalling "use the
/// row-type's registered default set".
fn resolve_columns_selection(
    projection: config::Projection,
    cli_spec: Option<&str>,
    config_names: Option<&[String]>,
) -> Result<Option<Vec<&'static str>>, conspectus::output::table::ColumnsError> {
    if let Some(spec) = cli_spec {
        return conspectus::output::table::parse_columns_spec(projection, spec).map(Some);
    }
    if let Some(names) = config_names {
        return conspectus::output::table::resolve_explicit_columns(projection, names).map(Some);
    }
    Ok(None)
}

fn row_config(projection: config::Projection, config: &config::Config) -> &config::TableRowConfig {
    match projection {
        config::Projection::Agent => &config.table.sessions,
        config::Projection::Mux => &config.table.mux,
        config::Projection::Union => &config.table.union,
        config::Projection::Pr => &config.table.prs,
        config::Projection::Fork => &config.table.forks,
    }
}

/// `--color` flag value. The renderer ultimately consumes a `bool`;
/// the value enum exists to give clap a stable parse surface and so
/// we can document the per-token semantics in `--help`.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ColorFlag {
    /// Auto-detect: color when stdout is a TTY and no env opt-out
    /// is set. See [`resolve_color`] for the full precedence table.
    #[default]
    Auto,
    /// Force color on, even when stdout is not a TTY. Overrides
    /// `NO_COLOR`, matching the cargo/git/ripgrep convention that an
    /// explicit user flag wins over passive env signals.
    Always,
    /// Force color off, regardless of TTY / env.
    Never,
}

/// Pull the env vars [`resolve_color`] cares about from the process
/// environment and dispatch. `stdout_is_tty` lets callers pass an
/// explicit boolean (typically `io::stdout().is_terminal()`) so this
/// function stays trivially testable.
fn resolve_color_from_env(flag: ColorFlag, stdout_is_tty: bool) -> bool {
    resolve_color(
        flag,
        std::env::var("NO_COLOR").ok(),
        std::env::var("CLICOLOR_FORCE").ok(),
        std::env::var("CLICOLOR").ok(),
        std::env::var("TERM").ok(),
        stdout_is_tty,
    )
}

/// Resolve `--color` to a boolean per ADR 0022:
///
/// 1. `--color=never`  ⇒ `false`.
/// 2. `--color=always` ⇒ `true`.
/// 3. `NO_COLOR` set to any non-empty value ⇒ `false`
///    (<https://no-color.org>).
/// 4. `CLICOLOR_FORCE` set to a non-zero value ⇒ `true` (BSD-style
///    force-on; matches the role of `--color=always` for env
///    signals).
/// 5. `TERM=dumb` ⇒ `false`.
/// 6. `CLICOLOR=0` ⇒ `false` (BSD-style opt-out).
/// 7. Otherwise: color iff stdout is a TTY.
///
/// Pure function over its arguments so unit tests can pin every
/// permutation without mutating process-wide env.
fn resolve_color(
    flag: ColorFlag,
    no_color: Option<String>,
    cli_color_force: Option<String>,
    cli_color: Option<String>,
    term: Option<String>,
    stdout_is_tty: bool,
) -> bool {
    match flag {
        ColorFlag::Never => return false,
        ColorFlag::Always => return true,
        ColorFlag::Auto => {}
    }
    if no_color.as_deref().is_some_and(|s| !s.is_empty()) {
        return false;
    }
    if cli_color_force
        .as_deref()
        .is_some_and(|s| !s.is_empty() && s != "0")
    {
        return true;
    }
    if term.as_deref() == Some("dumb") {
        return false;
    }
    if cli_color.as_deref() == Some("0") {
        return false;
    }
    stdout_is_tty
}

/// Decide the render width for `conspectus table <ROWS>`.
///
/// - `--wide` forces `None` (untruncated).
/// - `--width N` forces `Some(N)`.
/// - Otherwise: detect the terminal width when stdout is a TTY, else
///   leave untruncated so piped output stays grep/awk-friendly.
fn resolve_table_width(
    wide: bool,
    width: Option<usize>,
    stdout: &impl IsTerminal,
) -> Option<usize> {
    if wide {
        return None;
    }
    if let Some(w) = width {
        return Some(w);
    }
    if !stdout.is_terminal() {
        return None;
    }
    terminal_size::terminal_size().map(|(w, _)| usize::from(w.0))
}

/// Whether and how to page rendered output (H-TBL-013).
#[derive(Debug, Clone, Copy)]
struct PagerOptions {
    /// `--pager` forces pager even when stdout is not a TTY (useful
    /// for `PAGER=cat` integration tests).
    force_on: bool,
    /// `--no-pager` skips pager even on a TTY.
    force_off: bool,
}

impl PagerOptions {
    fn from_flags(pager: bool, no_pager: bool) -> Self {
        Self {
            force_on: pager,
            force_off: no_pager,
        }
    }

    fn should_page(self, stdout: &impl IsTerminal) -> bool {
        if self.force_off {
            return false;
        }
        if self.force_on {
            return true;
        }
        stdout.is_terminal()
    }
}

/// Print `content` to stdout, optionally through a pager. Falls back
/// to direct print when no pager is configured/available or when
/// `options` disables paging. Git-style behavior: `$PAGER` (when set
/// and non-empty) wins; otherwise `less` (with `LESS=FRX` defaults
/// when the env var is not already set so a single-screen output
/// prints inline and ANSI passes through); otherwise `more`;
/// otherwise direct.
fn print_paged(content: &str, options: PagerOptions) {
    if !options.should_page(&io::stdout()) {
        print!("{content}");
        return;
    }
    for mut cmd in pager_candidates() {
        match cmd.stdin(Stdio::piped()).spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
                return;
            }
            Err(_) => continue,
        }
    }
    print!("{content}");
}

/// Resolve the ordered list of pager commands to try.
///
/// For bare `less` invocations (whether from `$PAGER=less` or the
/// internal fallback) Conspectus passes `-F -R -X` explicitly as
/// command-line arguments. The flags need to apply even when the
/// user has a `$LESS` env value of their own, so setting `LESS=FRX`
/// only when `$LESS` is unset (the original implementation) silently
/// fell back to "no quit-if-one-screen" for users with any `$LESS`
/// set. Command-line args merge cleanly with `$LESS`, so existing
/// `LESS=-R` setups keep their `R` and gain the `F` they need.
fn pager_candidates() -> Vec<ProcCommand> {
    pager_candidates_with_env(std::env::var("PAGER").ok())
}

/// Pure version of [`pager_candidates`] for unit testing — takes the
/// `$PAGER` value explicitly so tests don't need to mutate
/// process-wide environment.
fn pager_candidates_with_env(pager_env: Option<String>) -> Vec<ProcCommand> {
    let mut candidates = Vec::new();

    if let Some(pager) = pager_env.filter(|s| !s.trim().is_empty()) {
        // Crude tokenization on whitespace (no shell-quoting support).
        // Git itself runs PAGER through `sh -c`, but that pulls in a
        // shell dependency we'd rather avoid. Users with quoted args
        // can set $PAGER to a wrapper script.
        let parts: Vec<&str> = pager.split_whitespace().collect();
        if let Some((prog, args)) = parts.split_first() {
            let mut cmd = ProcCommand::new(prog);
            if *prog == "less" && args.is_empty() {
                cmd.args(LESS_DEFAULT_ARGS);
            } else {
                cmd.args(args);
            }
            candidates.push(cmd);
        }
    }

    let mut less = ProcCommand::new("less");
    less.args(LESS_DEFAULT_ARGS);
    candidates.push(less);

    candidates.push(ProcCommand::new("more"));

    candidates
}

/// Default args Conspectus passes to `less` when the user has not
/// supplied any of their own via `$PAGER`.
///
/// - `-F` quit if the entire output fits on one screen.
/// - `-R` pass raw ANSI control sequences through (forward-compatible
///   with any future color story; harmless on plain text).
/// - `-X` skip the terminal init/deinit so the rendered output stays
///   on the user's scrollback instead of being cleared on exit.
const LESS_DEFAULT_ARGS: &[&str] = &["-F", "-R", "-X"];

#[derive(Debug, Args, Default)]
struct TuiArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Initial left-panel organization.
    #[arg(long, value_enum, default_value_t = ViewFlag::Sessions)]
    view: ViewFlag,
    /// Deprecated alias for `--grouping` when `--view sessions` is
    /// active (ADR 0031). Continues to work but emits a one-line
    /// deprecation warning to stderr; `--grouping` overrides on
    /// conflict.
    #[arg(long = "sessions-grouping", value_enum)]
    sessions_grouping: Option<SessionsGroupingFlag>,
    /// Row sort within each group.
    #[arg(long, value_enum, default_value_t = SortFlag::Hierarchy)]
    sort: SortFlag,
    #[command(flatten)]
    filter_args: FilterArgs,
    /// Background graph refresh cadence (e.g. `30s`, `1m`, `500ms`).
    #[arg(
        long = "refresh-interval",
        value_name = "DURATION",
        default_value = "30s"
    )]
    refresh_interval: String,
    /// Selected mux pane capture cadence.
    #[arg(
        long = "mux-preview-interval",
        value_name = "DURATION",
        default_value = "2s"
    )]
    mux_preview_interval: String,
    /// Suppress live extras: mux pane capture and transcript-tail
    /// reads. Graph-resident previews continue to render.
    #[arg(long = "no-live-preview")]
    no_live_preview: bool,
    /// When to colorize the output. `auto` (default) emits ANSI
    /// only when stdout is a TTY (and respects `NO_COLOR`,
    /// `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces
    /// color on; `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ViewFlag {
    #[default]
    Sessions,
    Mux,
    Union,
    Prs,
    Forks,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum SortFlag {
    #[default]
    Hierarchy,
    Recency,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum SessionsGroupingFlag {
    #[default]
    Graph,
    Repo,
    Checkout,
    ScanRoot,
}

impl SessionsGroupingFlag {
    fn to_grouping(self) -> conspectus::tui::Grouping {
        use conspectus::tui::{Grouping, SessionsGrouping};
        match self {
            SessionsGroupingFlag::Graph => Grouping::Sessions(SessionsGrouping::Graph),
            SessionsGroupingFlag::Repo => Grouping::Sessions(SessionsGrouping::Repo),
            SessionsGroupingFlag::Checkout => Grouping::Sessions(SessionsGrouping::Checkout),
            SessionsGroupingFlag::ScanRoot => Grouping::Sessions(SessionsGrouping::ScanRoot),
        }
    }
}

/// Filter / grouping flag surface shared by `conspectus tui` and
/// `conspectus table <ROWS>` (ADR 0031, F8-009). Mount with
/// `#[command(flatten)]` so the host struct picks up every flag
/// without re-declaring them.
///
/// Resolution helpers ([`FilterArgs::to_row_filter`] and
/// [`FilterArgs::to_grouping`]) take the active view so per-view
/// grouping validation can produce actionable errors against the
/// view's enum.
#[derive(Debug, Args, Default, Clone)]
struct FilterArgs {
    /// Narrow to one or more harness keys. Repeatable; values
    /// accumulate into a set. Comparison is case-insensitive and
    /// trim-aware.
    #[arg(long = "harness", value_name = "HARNESS")]
    harness: Vec<String>,
    /// Drop rows whose `last_active_epoch` is older than this
    /// window (e.g. `7d`, `24h`, `30m`).
    #[arg(long = "max-age", value_name = "DURATION")]
    max_age: Option<String>,
    /// Narrow by derived mux state. Comma-separated or repeatable.
    /// Legal values: `attached`, `ambiguous`, `unmuxed`.
    #[arg(long = "mux-state", value_name = "STATE", value_delimiter = ',')]
    mux_state: Vec<String>,
    /// Per-view grouping. Accepted values depend on `--view`; see
    /// `conspectus tui --help` for the per-view list (ADR 0031).
    #[arg(long = "grouping", value_name = "VALUE")]
    grouping: Option<String>,
}

impl FilterArgs {
    /// Convert the raw flag values into a [`conspectus::filter::RowFilter`].
    /// Returns an error when a value fails to parse (max-age
    /// duration, mux-state spelling).
    fn to_row_filter(&self) -> Result<conspectus::filter::RowFilter> {
        use conspectus::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
        let harness = if self.harness.is_empty() {
            None
        } else {
            Some(HarnessFilter::from_values(self.harness.iter()))
        };
        let max_age = match self.max_age.as_deref() {
            Some(raw) => Some(
                parse_filter_duration(raw)
                    .map_err(|err| anyhow!("invalid --max-age `{raw}`: {err}"))?,
            ),
            None => None,
        };
        let mux_state = if self.mux_state.is_empty() {
            None
        } else {
            let mut keys = Vec::with_capacity(self.mux_state.len());
            for raw in &self.mux_state {
                let key = MuxStateKey::from_str_ci(raw).ok_or_else(|| {
                    anyhow!(
                        "invalid --mux-state `{raw}`; expected one of attached, ambiguous, unmuxed"
                    )
                })?;
                keys.push(key);
            }
            Some(MuxStateFilter::from_values(keys))
        };
        Ok(RowFilter {
            harness,
            max_age,
            mux_state,
        })
    }

    /// Convert the `--grouping` flag value into a typed
    /// [`conspectus::tui::Grouping`] for the active view. Returns
    /// `Ok(None)` when the flag wasn't provided; returns an error
    /// when the value isn't valid for `view` so the caller can list
    /// the legal values in the message.
    fn to_grouping(
        &self,
        view: conspectus::tui::View,
    ) -> Result<Option<conspectus::tui::Grouping>> {
        let Some(raw) = self.grouping.as_deref() else {
            return Ok(None);
        };
        conspectus::tui::Grouping::parse_for(view, raw)
            .map(Some)
            .ok_or_else(|| {
                let choices = conspectus::tui::Grouping::values_for(view)
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow!(
                    "invalid --grouping `{raw}` for --view {}; expected one of {choices}",
                    view_flag_label(view)
                )
            })
    }
}

fn view_flag_label(view: conspectus::tui::View) -> &'static str {
    match view {
        conspectus::tui::View::Sessions => "sessions",
        conspectus::tui::View::Mux => "mux",
        conspectus::tui::View::Union => "union",
        conspectus::tui::View::Prs => "prs",
        conspectus::tui::View::Forks => "forks",
    }
}

impl TuiArgs {
    fn run(self) -> Result<()> {
        let refresh_interval = parse_tui_duration(&self.refresh_interval).map_err(|err| {
            anyhow!(
                "invalid --refresh-interval `{}`: {err}",
                self.refresh_interval
            )
        })?;
        let mux_preview_interval =
            parse_tui_duration(&self.mux_preview_interval).map_err(|err| {
                anyhow!(
                    "invalid --mux-preview-interval `{}`: {err}",
                    self.mux_preview_interval
                )
            })?;
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());

        // Resolve scan roots: CLI flags win, then config, then a
        // single-element fallback to the current working directory
        // (the original cwd-scoped v1 behavior).
        let cwd = std::env::current_dir()?;
        let loader = config::ConfigLoader::from_env();
        let outcome = loader.load_from(&cwd);
        for diagnostic in &outcome.diagnostics {
            eprintln!(
                "conspectus: warning: {}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }
        let scan_roots = if !self.scan_roots.is_empty() {
            self.scan_roots
        } else if !outcome.config.tui.scan_roots.is_empty() {
            outcome.config.tui.scan_roots.clone()
        } else {
            vec![cwd.clone()]
        };

        let view = match self.view {
            ViewFlag::Sessions => conspectus::tui::View::Sessions,
            ViewFlag::Mux => conspectus::tui::View::Mux,
            ViewFlag::Union => conspectus::tui::View::Union,
            ViewFlag::Prs => conspectus::tui::View::Prs,
            ViewFlag::Forks => conspectus::tui::View::Forks,
        };

        // Resolve initial filter: CLI flags win over config.
        let cli_filter = self.filter_args.to_row_filter()?;
        let initial_filter = if cli_filter.is_empty() {
            outcome.config.tui.views.for_view(view).filter.clone()
        } else {
            cli_filter
        };

        // Resolve initial grouping with precedence:
        //  1. --grouping (new, per-view, validated)
        //  2. --sessions-grouping (legacy alias; warns; only valid when view=sessions)
        //  3. config `[tui.views.<name>].grouping`
        //  4. Grouping::default_for(view)
        let mut initial_grouping = self.filter_args.to_grouping(view)?;
        if let Some(legacy) = self.sessions_grouping {
            eprintln!(
                "conspectus: warning: --sessions-grouping is deprecated; \
                 use --grouping instead (ADR 0031)"
            );
            if view != conspectus::tui::View::Sessions {
                eprintln!(
                    "conspectus: warning: --sessions-grouping ignored because \
                     --view is not `sessions`"
                );
            } else if initial_grouping.is_none() {
                initial_grouping = Some(legacy.to_grouping());
            }
        }
        let initial_grouping = match initial_grouping {
            Some(g) => g,
            None => outcome
                .config
                .tui
                .views
                .for_view(view)
                .grouping
                .unwrap_or_else(|| conspectus::tui::Grouping::default_for(view)),
        };
        let sessions_grouping = match initial_grouping {
            conspectus::tui::Grouping::Sessions(g) => g,
            // For non-sessions views, the runtime still needs a
            // SessionsGrouping for build_tree_for_view's sessions
            // branch; fall back to the default so a `--view mux
            // --grouping host` launch doesn't accidentally drag a
            // sessions grouping along. F8-003 generalizes this.
            _ => conspectus::tui::SessionsGrouping::Graph,
        };

        let config = conspectus::tui::RunConfig {
            scan_roots,
            cwd: Some(cwd),
            default_view: view,
            default_sort: match self.sort {
                SortFlag::Hierarchy => conspectus::tui::Sort::Hierarchy,
                SortFlag::Recency => conspectus::tui::Sort::Recency,
            },
            sessions_grouping,
            initial_filter,
            refresh_interval,
            mux_preview_interval,
            live_preview_enabled: !self.no_live_preview,
            color,
            current_tmux_session: current_tmux_session_name(),
        };

        conspectus::tui::run(config)
    }
}

fn current_tmux_session_name() -> Option<String> {
    std::env::var_os("TMUX")?;
    let output = ProcCommand::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Parse a duration like [`parse_tui_duration`] but accept a `d`
/// (days) suffix. Used by `--max-age` where day-scale windows are
/// common; `parse_tui_duration` deliberately rejects `d` because
/// day-scale refresh intervals don't make sense.
fn parse_filter_duration(input: &str) -> Result<Duration, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h/d)".to_string())?;
    let (num_str, suffix) = trimmed.split_at(split);
    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{num_str}`"))?;
    let dur = match suffix {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value.saturating_mul(60)),
        "h" => Duration::from_secs(value.saturating_mul(3600)),
        "d" => Duration::from_secs(value.saturating_mul(86_400)),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h/d)")),
    };
    Ok(dur)
}

/// Parse a small subset of duration strings: `<integer><ms|s|m|h>`.
/// Kept in-tree to avoid pulling in `humantime` for the TUI flag
/// surface; revisit if more formats are needed.
fn parse_tui_duration(input: &str) -> Result<Duration, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h)".to_string())?;
    let (num_str, suffix) = trimmed.split_at(split);
    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{num_str}`"))?;
    let dur = match suffix {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value.saturating_mul(60)),
        "h" => Duration::from_secs(value.saturating_mul(3600)),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h)")),
    };
    Ok(dur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(cmd: &ProcCommand) -> String {
        cmd.get_program().to_string_lossy().into_owned()
    }

    fn args(cmd: &ProcCommand) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn parse_tui_duration_accepts_each_supported_unit() {
        assert_eq!(parse_tui_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_tui_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_tui_duration("2m"), Ok(Duration::from_secs(120)));
        assert_eq!(parse_tui_duration("1h"), Ok(Duration::from_secs(3600)));
    }

    #[test]
    fn parse_tui_duration_trims_surrounding_whitespace() {
        assert_eq!(parse_tui_duration("  10s  "), Ok(Duration::from_secs(10)));
    }

    #[test]
    fn parse_tui_duration_rejects_missing_unit() {
        let err = parse_tui_duration("30").unwrap_err();
        assert!(err.contains("missing unit"), "got: {err}");
    }

    #[test]
    fn parse_tui_duration_rejects_unknown_unit() {
        let err = parse_tui_duration("30d").unwrap_err();
        assert!(err.contains("unknown unit"), "got: {err}");
    }

    #[test]
    fn parse_tui_duration_rejects_empty_string() {
        assert!(parse_tui_duration("").is_err());
        assert!(parse_tui_duration("   ").is_err());
    }

    #[test]
    fn parse_tui_duration_rejects_negative_or_non_integer() {
        assert!(parse_tui_duration("-5s").is_err());
        assert!(parse_tui_duration("1.5s").is_err());
    }

    #[test]
    fn pager_candidates_when_pager_env_is_unset_starts_with_less_plus_defaults() {
        let candidates = pager_candidates_with_env(None);
        assert!(candidates.len() >= 2);
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
        assert_eq!(program(&candidates[1]), "more");
    }

    #[test]
    fn pager_candidates_when_pager_is_bare_less_adds_default_flags() {
        // Regression for the original bug: the user had `PAGER=less`
        // (no args) plus `LESS=-R` set, so the original implementation
        // left less without `-F` and dropped into the pager even for
        // short, single-screen tables. The fix passes `-F -R -X` on
        // the command line whenever the resolved pager is plain
        // `less`, so the flags merge with the user's `$LESS` instead
        // of being silently skipped.
        let candidates = pager_candidates_with_env(Some("less".to_string()));
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
    }

    #[test]
    fn pager_candidates_when_pager_is_less_with_explicit_args_respects_user_choice() {
        let candidates = pager_candidates_with_env(Some("less -X".to_string()));
        assert_eq!(program(&candidates[0]), "less");
        // Explicit args are kept verbatim; we do not silently append
        // `-F` because the user opted in to their own less flag set.
        assert_eq!(args(&candidates[0]), vec!["-X"]);
    }

    #[test]
    fn pager_candidates_passes_non_less_pager_through_unchanged() {
        let candidates = pager_candidates_with_env(Some("bat --paging=always".to_string()));
        assert_eq!(program(&candidates[0]), "bat");
        assert_eq!(args(&candidates[0]), vec!["--paging=always"]);
    }

    #[test]
    fn resolve_color_never_wins_against_every_env_signal() {
        assert!(!resolve_color(
            ColorFlag::Never,
            Some("1".into()),
            Some("1".into()),
            Some("1".into()),
            Some("xterm".into()),
            true,
        ));
    }

    #[test]
    fn resolve_color_always_overrides_no_color_and_dumb_term() {
        assert!(resolve_color(
            ColorFlag::Always,
            Some("1".into()),
            None,
            None,
            Some("dumb".into()),
            false,
        ));
    }

    #[test]
    fn resolve_color_auto_honors_no_color() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            Some("1".into()),
            None,
            None,
            None,
            true,
        ));
        // Empty NO_COLOR is treated as unset (per the spec — value
        // matters, not just presence).
        assert!(resolve_color(
            ColorFlag::Auto,
            Some(String::new()),
            None,
            None,
            None,
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_honors_clicolor_force_even_on_non_tty() {
        assert!(resolve_color(
            ColorFlag::Auto,
            None,
            Some("1".into()),
            None,
            None,
            false,
        ));
        // CLICOLOR_FORCE=0 is *not* "force on".
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            Some("0".into()),
            None,
            None,
            false,
        ));
    }

    #[test]
    fn resolve_color_auto_dumb_term_opts_out() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            None,
            Some("dumb".into()),
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_clicolor_zero_opts_out() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            Some("0".into()),
            None,
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_falls_back_to_isatty() {
        assert!(resolve_color(ColorFlag::Auto, None, None, None, None, true));
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            None,
            None,
            false
        ));
    }

    #[test]
    fn resolve_color_from_env_uses_supplied_tty_signal() {
        // The wrapper pulls env vars from the real process; we can
        // only safely pin behavior under the flag values that
        // short-circuit before any env lookup.
        assert!(!resolve_color_from_env(ColorFlag::Never, true));
        assert!(resolve_color_from_env(ColorFlag::Always, false));
    }

    #[test]
    fn pager_candidates_empty_pager_env_falls_back_to_internal_defaults() {
        let candidates = pager_candidates_with_env(Some("   ".to_string()));
        // Whitespace-only `$PAGER` falls back to the internal `less`
        // with default flags.
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
    }

    #[test]
    fn ensure_claude_hook_preserves_existing_hooks() {
        let mut document = serde_json::json!({
            "theme": "dark",
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "startup",
                        "hooks": [
                            { "type": "command", "command": "echo existing" }
                        ]
                    }
                ]
            }
        });

        assert!(ensure_claude_hook(
            &mut document,
            "conspectus hook write claude-code"
        ));
        assert!(has_claude_hook(&document));

        let entries = document["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(entries.len(), 2);
        assert_eq!(document["theme"], "dark");
    }

    #[test]
    fn ensure_claude_hook_is_idempotent() {
        let mut document = serde_json::json!({});

        assert!(ensure_claude_hook(
            &mut document,
            "conspectus hook write claude-code"
        ));
        assert!(!ensure_claude_hook(
            &mut document,
            "conspectus hook write claude-code"
        ));

        let entries = document["hooks"]["SessionStart"].as_array().expect("array");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn remove_claude_hook_preserves_unrelated_hooks_in_same_entry() {
        let mut document = serde_json::json!({
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "startup",
                        "hooks": [
                            { "type": "command", "command": "conspectus hook write claude-code" },
                            { "type": "command", "command": "echo existing" }
                        ]
                    }
                ]
            }
        });

        assert!(remove_claude_hook(&mut document));
        assert!(!has_claude_hook(&document));

        let hooks = document["hooks"]["SessionStart"][0]["hooks"]
            .as_array()
            .expect("hooks");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "echo existing");
    }

    // ---- ADR 0031 / F8-009: FilterArgs ----

    fn filter_args_with(
        harness: Vec<&str>,
        max_age: Option<&str>,
        mux_state: Vec<&str>,
        grouping: Option<&str>,
    ) -> FilterArgs {
        FilterArgs {
            harness: harness.into_iter().map(String::from).collect(),
            max_age: max_age.map(String::from),
            mux_state: mux_state.into_iter().map(String::from).collect(),
            grouping: grouping.map(String::from),
        }
    }

    #[test]
    fn filter_args_empty_produces_empty_row_filter() {
        let args = FilterArgs::default();
        let filter = args.to_row_filter().expect("parse");
        assert!(filter.is_empty());
    }

    #[test]
    fn filter_args_harness_repeats_into_set() {
        let args = filter_args_with(vec!["claude-code", "codex"], None, vec![], None);
        let filter = args.to_row_filter().expect("parse");
        assert_eq!(
            filter
                .harness
                .as_ref()
                .map(|h| h.values().to_vec())
                .unwrap_or_default(),
            vec!["claude-code".to_string(), "codex".to_string()]
        );
    }

    #[test]
    fn filter_args_max_age_parses_duration_suffixes() {
        let args = filter_args_with(vec![], Some("7d"), vec![], None);
        let filter = args.to_row_filter().expect("parse");
        assert_eq!(
            filter.max_age,
            Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        );
    }

    #[test]
    fn filter_args_max_age_reports_actionable_error() {
        let args = filter_args_with(vec![], Some("nope"), vec![], None);
        let err = args.to_row_filter().unwrap_err().to_string();
        assert!(err.contains("invalid --max-age"));
    }

    #[test]
    fn filter_args_mux_state_parses_each_value() {
        let args = filter_args_with(vec![], None, vec!["unmuxed", "Ambiguous"], None);
        let filter = args.to_row_filter().expect("parse");
        let states = filter
            .mux_state
            .as_ref()
            .map(|m| m.values().to_vec())
            .unwrap_or_default();
        use conspectus::filter::MuxStateKey;
        assert!(states.contains(&MuxStateKey::Unmuxed));
        assert!(states.contains(&MuxStateKey::Ambiguous));
    }

    #[test]
    fn filter_args_mux_state_invalid_value_errors_with_choices() {
        let args = filter_args_with(vec![], None, vec!["frobnicated"], None);
        let err = args.to_row_filter().unwrap_err().to_string();
        assert!(err.contains("invalid --mux-state"));
        assert!(err.contains("attached, ambiguous, unmuxed"));
    }

    #[test]
    fn filter_args_grouping_parses_per_view() {
        use conspectus::tui::{Grouping, SessionsGrouping, View};
        let args = filter_args_with(vec![], None, vec![], Some("repo"));
        assert_eq!(
            args.to_grouping(View::Sessions).expect("parse"),
            Some(Grouping::Sessions(SessionsGrouping::Repo))
        );
    }

    #[test]
    fn filter_args_grouping_rejects_value_for_wrong_view() {
        use conspectus::tui::View;
        // `host` is a mux grouping, not a sessions one.
        let args = filter_args_with(vec![], None, vec![], Some("host"));
        let err = args.to_grouping(View::Sessions).unwrap_err().to_string();
        assert!(err.contains("invalid --grouping `host` for --view sessions"));
        assert!(err.contains("graph, repo, checkout, scan-root"));
    }

    #[test]
    fn filter_args_grouping_none_when_flag_omitted() {
        use conspectus::tui::View;
        let args = FilterArgs::default();
        assert!(args.to_grouping(View::Sessions).expect("parse").is_none());
    }
}

#[derive(Debug, Args)]
struct DeclaredArgs {
    #[command(subcommand)]
    command: DeclaredCommand,
}

impl DeclaredArgs {
    fn run(self) -> Result<()> {
        match self.command {
            DeclaredCommand::List(args) => args.run(),
            DeclaredCommand::Create(args) => args.run(),
            DeclaredCommand::Remove(args) => args.run_remove(),
            DeclaredCommand::Confirm(args) => args.run_confirm(),
            DeclaredCommand::Ignore(args) => args.run(),
            DeclaredCommand::Override(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum DeclaredCommand {
    /// List declared links from discovered config stores.
    List(DeclaredListArgs),
    /// Create a declared link.
    Create(Box<DeclaredCreateArgs>),
    /// Remove a declared link by id.
    Remove(DeclaredIdArgs),
    /// Confirm a discovered relationship as a declared link.
    Confirm(DeclaredIdArgs),
    /// Mark a declared link ignored.
    Ignore(DeclaredIgnoreArgs),
    /// Replace one declared link with another.
    Override(DeclaredOverrideArgs),
}

#[derive(Debug, Args)]
struct DeclaredListArgs {
    /// Limit the list to a declared-link store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local declared-link stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredListArgs {
    fn run(self) -> Result<()> {
        let loader = config::ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records = Vec::new();
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_declared_records(
                &mut records,
                DeclaredStoreFlag::User,
                Provenance::GlobalDeclared,
                path,
            );
        }
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_declared_records(
                    &mut records,
                    DeclaredStoreFlag::Project,
                    Provenance::LocalDeclared,
                    path,
                );
            }
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                declared_record_id(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    declared_record_id(right),
                ))
        });
        for record in records {
            match record.link {
                Ok(link) => println!(
                    "{}",
                    render_declared_record(&record.path, record.store, record.provenance, &link)
                ),
                Err(message) => eprintln!(
                    "conspectus: warning: {}: {}",
                    record.path.display(),
                    message
                ),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredCreateArgs {
    /// Stable id for the declaration.
    #[arg(long)]
    id: String,
    /// Relationship kind, such as linked_to_mux or branch_has_forge_pr.
    #[arg(long, value_parser = parse_relation_kind)]
    relation: RelationKind,
    /// Source endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    source: DeclaredEndpointArg,
    /// Target endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    target: DeclaredEndpointArg,
    /// Human reason stored with the declaration.
    #[arg(long)]
    reason: Option<String>,
    /// Human label stored with the declaration.
    #[arg(long)]
    label: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores for nearest-store selection.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredCreateArgs {
    fn run(self) -> Result<()> {
        let link = DeclaredLink {
            id: self.id,
            relation: self.relation,
            state: DeclaredLinkState::Active,
            source: self.source.0,
            target: self.target.0,
            reason: self.reason,
            overridden_by: None,
            label: self.label,
        };

        let path = resolve_write_store(
            self.store,
            Some(&link.source),
            Some(&link.target),
            &self.scan_roots,
        )?;

        let outcome =
            upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            if outcome.link_count == 1 {
                "wrote"
            } else {
                "updated"
            }
        } else {
            "unchanged"
        };
        println!("{verb} declared link `{}` in {}", link.id, path.display());
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredIdArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIdArgs {
    fn run_remove(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let mut removed_from = None;
        for path in &stores {
            if !path.is_file() {
                continue;
            }
            let outcome =
                remove_declared_link(path, &self.id).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                removed_from = Some(path.clone());
                break;
            }
        }

        match removed_from {
            Some(path) => {
                println!(
                    "removed declared link `{}` from {}",
                    self.id,
                    path.display()
                );
                Ok(())
            }
            None => {
                bail!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                );
            }
        }
    }

    fn run_confirm(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Active,
            None,
            self.store,
            &self.scan_roots,
        )
    }
}

/// Shared implementation for `declared confirm` and `declared ignore`.
///
/// Both commands take a candidate-link id from the current discovered
/// graph and produce a declared link whose source/target/relation
/// mirror the candidate. They only differ in the link state and the
/// optional reason string.
fn run_confirm_or_ignore(
    candidate_id: &str,
    state: DeclaredLinkState,
    reason: Option<String>,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<()> {
    let snapshot = discover_for_store_selection(scan_roots)?;
    let candidate = find_candidate_by_id(&snapshot, candidate_id)?;
    let target_node = match &candidate.target {
        LinkEndpoint::Node { id } => id.clone(),
        LinkEndpoint::Unresolved { .. } => bail!(
            "candidate `{candidate_id}` targets an unresolved endpoint; declare it directly with \
             `conspectus declared create`"
        ),
    };

    let source_endpoint = declared_endpoint_from_node_id(&candidate.source);
    let target_endpoint = declared_endpoint_from_node_id(&target_node);

    let link = DeclaredLink {
        id: candidate_id.to_string(),
        relation: candidate.relation.clone(),
        state,
        source: source_endpoint.clone(),
        target: target_endpoint.clone(),
        reason,
        overridden_by: None,
        label: None,
    };

    let path = resolve_write_store(
        store,
        Some(&source_endpoint),
        Some(&target_endpoint),
        scan_roots,
    )?;

    let outcome =
        upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

    let verb = match (state, outcome.changed) {
        (DeclaredLinkState::Active, true) => "confirmed",
        (DeclaredLinkState::Ignored, true) => "ignored",
        (DeclaredLinkState::Overridden, true) => "overrode",
        (_, false) => "unchanged",
    };
    println!("{verb} declared link `{}` in {}", link.id, path.display());
    Ok(())
}

fn find_candidate_by_id<'a>(snapshot: &'a GraphSnapshot, id: &str) -> Result<&'a GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .find(|link| link.id == id)
        .ok_or_else(|| anyhow!("no candidate link with id `{id}` was discovered"))
}

#[derive(Debug, Args)]
struct DeclaredIgnoreArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Reason the declaration should be ignored.
    #[arg(long)]
    reason: Option<String>,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIgnoreArgs {
    fn run(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Ignored,
            self.reason,
            self.store,
            &self.scan_roots,
        )
    }
}

#[derive(Debug, Args)]
struct DeclaredOverrideArgs {
    /// Declared-link id to replace.
    #[arg(long)]
    id: String,
    /// Replacement declared-link id.
    #[arg(long = "overridden-by")]
    overridden_by: String,
    /// Reason the old declaration was overridden.
    #[arg(long)]
    reason: Option<String>,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredOverrideArgs {
    fn run(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let (path, existing) = load_declared_link_by_id(&stores, &self.id)
            .map_err(|err| anyhow!(err.to_string()))?
            .ok_or_else(|| {
                anyhow!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                )
            })?;

        let mut replacement = existing;
        replacement.state = DeclaredLinkState::Overridden;
        replacement.overridden_by = Some(self.overridden_by);
        if let Some(reason) = self.reason {
            replacement.reason = Some(reason);
        }

        let outcome = upsert_declared_link(&path, replacement.clone())
            .map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            "overrode"
        } else {
            "unchanged"
        };
        println!(
            "{verb} declared link `{}` in {}",
            replacement.id,
            path.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct RenameArgs {
    #[command(subcommand)]
    command: RenameCommand,
}

impl RenameArgs {
    fn run(self) -> Result<()> {
        match self.command {
            RenameCommand::Session(args) => args.run(),
            RenameCommand::Mux(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum RenameCommand {
    /// Set, change, or clear an agent session's display-name alias.
    /// The linked tmux session is renamed in lockstep by default;
    /// pass `--no-mux` to skip the tmux side.
    Session(RenameSessionArgs),
    /// Rename a tmux session. No alias is written; only the tmux
    /// native name changes (per ADR 0029 mux-id stability rule).
    Mux(RenameMuxArgs),
}

#[derive(Debug, Args)]
struct RenameSessionArgs {
    /// Agent session id. Accepts the short row id, the full
    /// `NodeId` display form, or the `harness:session_key` label
    /// (same forms `conspectus node show` understands).
    id: String,
    /// New display name. Mutually exclusive with `--clear`.
    name: Option<String>,
    /// Skip the lockstep tmux rename. The alias is still written.
    #[arg(long = "no-mux")]
    no_mux: bool,
    /// Remove any existing alias for this session instead of setting one.
    /// Mutually exclusive with `<NAME>`.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Restrict the alias write to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameSessionArgs {
    fn run(self) -> Result<()> {
        let new_display_name = match (self.name, self.clear) {
            (Some(_), true) => bail!("--clear and <NAME> are mutually exclusive"),
            (None, false) => bail!("specify either a new <NAME> or --clear"),
            (Some(name), false) => Some(name),
            (None, true) => None,
        };

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let session_id = match resolved {
            NodeId::AgentSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename session only operates on agent sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        let plan = plan_session_rename(&snapshot, &session_id, new_display_name, self.no_mux)
            .map_err(|err| anyhow!(err.to_string()))?;

        execute_rename_plan(&plan, self.store, &self.scan_roots, &SystemTmux::new())
    }
}

#[derive(Debug, Args)]
struct RenameMuxArgs {
    /// Mux session id. Accepts the short row id, the full `NodeId`
    /// display form, or the `tmux:<native>` label.
    id: String,
    /// New tmux session name. Required because mux aliases are not
    /// stored (per ADR 0029) — only the native tmux name changes.
    name: Option<String>,
    /// Rejected: mux sessions have no Conspectus-owned alias to
    /// clear. Surfaced so the help text documents the constraint.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Root used to discover the running tmux server, if any.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameMuxArgs {
    fn run(self) -> Result<()> {
        if self.clear {
            bail!(
                "mux sessions have no Conspectus-owned alias to clear; \
                 supply a new <NAME> instead"
            );
        }
        let new_name = self
            .name
            .ok_or_else(|| anyhow!("rename mux requires a new <NAME>"))?;
        if new_name.trim().is_empty() {
            bail!("mux rename requires a non-empty <NAME>");
        }

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let mux_id = match resolved {
            NodeId::MuxSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename mux only operates on mux sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        run_mux_rename(
            &MuxNativeRename {
                mux: mux_id,
                new_name,
            },
            &SystemTmux::new(),
        )
    }
}

/// Execute the alias-write side of `plan`, then (when present) the
/// linked tmux rename. Either step can leave the other in a partial
/// state — we surface the error and let the operator decide whether
/// to re-run.
fn execute_rename_plan(
    plan: &RenamePlan,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
    tmux: &dyn TmuxRunner,
) -> Result<()> {
    let endpoint = declared_endpoint_from_node_id(&NodeId::AgentSession(
        plan.agent_alias_write.session.clone(),
    ));
    match &plan.agent_alias_write.display_name {
        Some(display_name) => {
            let path = resolve_alias_store(store, &endpoint, scan_roots)?;
            let entry = AliasEntry {
                node: endpoint,
                display_name: display_name.clone(),
                reason: None,
            };
            let outcome =
                upsert_alias_entry(&path, entry).map_err(|err| anyhow!(err.to_string()))?;
            let verb = if outcome.changed {
                "wrote"
            } else {
                "unchanged"
            };
            println!("{verb} alias `{}` in {}", display_name, path.display());
        }
        None => {
            let stores = alias_candidate_store_paths(store, scan_roots)?;
            let mut removed_from = None;
            for path in &stores {
                if !path.is_file() {
                    continue;
                }
                let outcome =
                    remove_alias_entry(path, &endpoint).map_err(|err| anyhow!(err.to_string()))?;
                if outcome.changed {
                    removed_from = Some(path.clone());
                    break;
                }
            }
            match removed_from {
                Some(path) => println!("removed alias from {}", path.display()),
                None => println!("no alias found for session"),
            }
        }
    }

    if let Some(mux_rename) = &plan.mux_native_rename {
        run_mux_rename(mux_rename, tmux)?;
    }
    Ok(())
}

fn run_mux_rename(rename: &MuxNativeRename, tmux: &dyn TmuxRunner) -> Result<()> {
    let outcome = tmux
        .rename_session(&rename.mux.native_id, &rename.new_name)
        .map_err(|err| anyhow!("tmux rename-session failed: {err}"))?;
    match outcome {
        TmuxRenameOutcome::Renamed => {
            println!(
                "renamed tmux session `{}` to `{}`",
                rename.mux.native_id, rename.new_name
            );
            Ok(())
        }
        TmuxRenameOutcome::NoTarget => bail!(
            "tmux session `{}` not found on this server",
            rename.mux.native_id
        ),
        TmuxRenameOutcome::NameCollision => bail!(
            "tmux refused to rename `{}` to `{}`: name already in use",
            rename.mux.native_id,
            rename.new_name
        ),
        TmuxRenameOutcome::Unavailable(reason) => bail!("tmux unavailable: {}", reason.as_str()),
        TmuxRenameOutcome::Failed { code, message } => bail!(
            "tmux rename-session failed (exit code {:?}): {}",
            code,
            message
        ),
        TmuxRenameOutcome::Unsupported => bail!("tmux runner does not support rename_session"),
    }
}

fn node_kind_label(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}

fn resolve_alias_store(
    store: Option<DeclaredStoreFlag>,
    endpoint: &DeclaredEndpoint,
    scan_roots: &[PathBuf],
) -> Result<PathBuf> {
    match store {
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for alias writes; pick `project` or `user`")
        }
        Some(DeclaredStoreFlag::User) => {
            let loader = ConfigLoader::from_env();
            loader.user_config_path().ok_or_else(|| {
                anyhow!("no user config path available; set $HOME or $XDG_CONFIG_HOME")
            })
        }
        Some(DeclaredStoreFlag::Project) => project_store_path(scan_roots),
        None => {
            let snapshot = discover_for_store_selection(scan_roots)?;
            let loader = ConfigLoader::from_env();
            let selection = select_store_for_declaration(endpoint, endpoint, &snapshot, &loader)
                .ok_or_else(|| {
                    anyhow!("could not pick an alias store; pass --store user or --store project")
                })?;
            Ok(selection.path)
        }
    }
}

fn alias_candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    // Same shape as `candidate_store_paths`: project stores first,
    // then user, so a project alias is removed before the global
    // entry takes over the rendering precedence.
    candidate_store_paths(store, scan_roots)
}

#[derive(Debug, Args)]
struct AliasArgs {
    #[command(subcommand)]
    command: AliasCommand,
}

impl AliasArgs {
    fn run(self) -> Result<()> {
        match self.command {
            AliasCommand::List(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum AliasCommand {
    /// List session aliases from the discovered config stores.
    List(AliasListArgs),
}

#[derive(Debug, Args)]
struct AliasListArgs {
    /// Limit the list to a store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl AliasListArgs {
    fn run(self) -> Result<()> {
        let loader = ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records: Vec<AliasListRecord> = Vec::new();
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in &scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_alias_records(&mut records, DeclaredStoreFlag::Project, path);
            }
        }
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_alias_records(&mut records, DeclaredStoreFlag::User, path);
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                alias_record_key(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    alias_record_key(right),
                ))
        });

        for record in records {
            match record.entry {
                Ok(entry) => println!(
                    "{}\t{}\t{}\t{}",
                    store_label(record.store),
                    record.path.display(),
                    format_alias_endpoint(&entry.node),
                    entry.display_name
                ),
                Err(message) => eprintln!(
                    "conspectus: warning: {}: {}",
                    record.path.display(),
                    message
                ),
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct AliasListRecord {
    store: DeclaredStoreFlag,
    path: PathBuf,
    entry: std::result::Result<AliasEntry, String>,
}

fn alias_record_key(record: &AliasListRecord) -> String {
    match &record.entry {
        Ok(entry) => format_alias_endpoint(&entry.node),
        Err(_) => String::new(),
    }
}

fn append_alias_records(
    records: &mut Vec<AliasListRecord>,
    store: DeclaredStoreFlag,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to read aliases: {err}")),
            });
            return;
        }
    };
    let parsed: AliasesDocument = match parse_aliases_document(&text) {
        Ok(document) => document,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to parse aliases: {err}")),
            });
            return;
        }
    };
    for entry in parsed.entries() {
        records.push(AliasListRecord {
            store,
            path: path.clone(),
            entry: Ok(entry.clone()),
        });
    }
}

fn format_alias_endpoint(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => format!("agent_session:{harness_key}:{state_scope}:{session_key}"),
        DeclaredEndpoint::MuxSession { native_id } => format!("mux_session:{native_id}"),
        DeclaredEndpoint::Repo { common_dir } => format!("repo:{common_dir}"),
        DeclaredEndpoint::Checkout { root, .. } => format!("checkout:{root}"),
        DeclaredEndpoint::Workspace { root } => format!("workspace:{root}"),
        DeclaredEndpoint::Branch { refname, .. } => format!("branch:{refname}"),
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => format!("fork:{provider_source_key}"),
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => format!("forge_pr:{provider}:{host}/{owner}/{repo}#{number}"),
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum DeclaredStoreFlag {
    All,
    Project,
    User,
}

#[derive(Debug)]
struct DeclaredListRecord {
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
    link: std::result::Result<DeclaredLink, String>,
}

fn declared_record_id(record: &DeclaredListRecord) -> &str {
    match &record.link {
        Ok(link) => &link.id,
        Err(_) => "",
    }
}

fn append_declared_records(
    records: &mut Vec<DeclaredListRecord>,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(DeclaredListRecord {
                store,
                provenance,
                path,
                link: Err(format!("failed to read declared config: {err}")),
            });
            return;
        }
    };

    match parse_declared_document(&text) {
        Ok(document) => {
            records.extend(
                document
                    .links()
                    .iter()
                    .cloned()
                    .map(|link| DeclaredListRecord {
                        store,
                        provenance,
                        path: path.clone(),
                        link: Ok(link),
                    }),
            )
        }
        Err(err) => records.push(DeclaredListRecord {
            store,
            provenance,
            path,
            link: Err(format!("failed to parse declared config: {err}")),
        }),
    }
}

fn render_declared_record(
    path: &std::path::Path,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    link: &DeclaredLink,
) -> String {
    [
        store_label(store).to_string(),
        provenance_label(provenance).to_string(),
        state_label(link.state).to_string(),
        link.id.clone(),
        relation_label(&link.relation).to_string(),
        endpoint_label(&link.source),
        endpoint_label(&link.target),
        link.reason.clone().unwrap_or_default(),
        link.overridden_by.clone().unwrap_or_default(),
        link.label.clone().unwrap_or_default(),
        path.display().to_string(),
    ]
    .join("\t")
}

fn store_label(store: DeclaredStoreFlag) -> &'static str {
    match store {
        DeclaredStoreFlag::All => "all",
        DeclaredStoreFlag::Project => "project",
        DeclaredStoreFlag::User => "user",
    }
}

fn provenance_label(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "local_declared",
        Provenance::GlobalDeclared => "global_declared",
        Provenance::StrongDiscovered => "strong_discovered",
        Provenance::Discovered => "discovered",
        Provenance::Convention => "convention",
        Provenance::Cached => "cached",
    }
}

fn state_label(state: DeclaredLinkState) -> &'static str {
    match state {
        DeclaredLinkState::Active => "active",
        DeclaredLinkState::Ignored => "ignored",
        DeclaredLinkState::Overridden => "overridden",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeclaredEndpointArg(DeclaredEndpoint);

impl FromStr for DeclaredEndpointArg {
    type Err = String;

    fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
        parse_endpoint(raw).map(Self)
    }
}

fn parse_relation_kind(raw: &str) -> std::result::Result<RelationKind, String> {
    match raw {
        "associated_with" => Ok(RelationKind::AssociatedWith),
        "belongs_to_repo" => Ok(RelationKind::BelongsToRepo),
        "checked_out_branch" => Ok(RelationKind::CheckedOutBranch),
        "workspace_contains_repo" => Ok(RelationKind::WorkspaceContainsRepo),
        "branch_has_forge_pr" => Ok(RelationKind::BranchHasForgePr),
        "linked_to_mux" => Ok(RelationKind::LinkedToMux),
        "rooted_in" => Ok(RelationKind::RootedIn),
        "forks_workspace" => Ok(RelationKind::ForksWorkspace),
        "forks_repo" => Ok(RelationKind::ForksRepo),
        "created_checkout" => Ok(RelationKind::CreatedCheckout),
        "referenced_checkout" => Ok(RelationKind::ReferencedCheckout),
        "parent_session" => Ok(RelationKind::ParentSession),
        "child_session" => Ok(RelationKind::ChildSession),
        "created_branch" => Ok(RelationKind::CreatedBranch),
        "associated_branch" => Ok(RelationKind::AssociatedBranch),
        "parent_fork" => Ok(RelationKind::ParentFork),
        "rooted_at_path" => Ok(RelationKind::RootedAtPath),
        _ => Err(format!(
            "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
        )),
    }
}

fn relation_label(relation: &RelationKind) -> &'static str {
    match relation {
        RelationKind::AssociatedWith => "associated_with",
        RelationKind::BelongsToRepo => "belongs_to_repo",
        RelationKind::CheckedOutBranch => "checked_out_branch",
        RelationKind::WorkspaceContainsRepo => "workspace_contains_repo",
        RelationKind::BranchHasForgePr => "branch_has_forge_pr",
        RelationKind::LinkedToMux => "linked_to_mux",
        RelationKind::RootedIn => "rooted_in",
        RelationKind::ForksWorkspace => "forks_workspace",
        RelationKind::ForksRepo => "forks_repo",
        RelationKind::CreatedCheckout => "created_checkout",
        RelationKind::ReferencedCheckout => "referenced_checkout",
        RelationKind::ParentSession => "parent_session",
        RelationKind::ChildSession => "child_session",
        RelationKind::CreatedBranch => "created_branch",
        RelationKind::AssociatedBranch => "associated_branch",
        RelationKind::ParentFork => "parent_fork",
        RelationKind::RootedAtPath => "rooted_at_path",
    }
}

fn parse_endpoint(raw: &str) -> std::result::Result<DeclaredEndpoint, String> {
    let (kind, fields) = raw.split_once(':').ok_or_else(endpoint_syntax_error)?;
    let fields = parse_endpoint_fields(fields)?;
    match kind {
        "repo" => Ok(DeclaredEndpoint::Repo {
            common_dir: required_field(&fields, "common_dir")?,
        }),
        "checkout" => Ok(DeclaredEndpoint::Checkout {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            root: required_field(&fields, "root")?,
        }),
        "workspace" => Ok(DeclaredEndpoint::Workspace {
            root: required_field(&fields, "root")?,
        }),
        "agent_session" => Ok(DeclaredEndpoint::AgentSession {
            harness_key: required_field(&fields, "harness_key")?,
            state_scope: required_field(&fields, "state_scope")?,
            session_key: required_field(&fields, "session_key")?,
        }),
        "mux_session" => Ok(DeclaredEndpoint::MuxSession {
            native_id: required_field(&fields, "native_id")?,
        }),
        "branch" => Ok(DeclaredEndpoint::Branch {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            refname: required_field(&fields, "refname")?,
        }),
        "fork" => Ok(DeclaredEndpoint::Fork {
            provider_source_key: required_field(&fields, "provider_source_key")?,
        }),
        "forge_pr" => Ok(DeclaredEndpoint::ForgePr {
            provider: required_field(&fields, "provider")?,
            host: required_field(&fields, "host")?,
            owner: required_field(&fields, "owner")?,
            repo: required_field(&fields, "repo")?,
            number: required_field(&fields, "number")?
                .parse()
                .map_err(|_| "endpoint field `number` must be an integer".to_string())?,
        }),
        _ => Err(endpoint_syntax_error()),
    }
}

fn endpoint_label(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => {
            format!("repo:common_dir={common_dir}")
        }
        DeclaredEndpoint::Checkout {
            repo_common_dir,
            root,
        } => {
            format!("checkout:repo_common_dir={repo_common_dir},root={root}")
        }
        DeclaredEndpoint::Workspace { root } => {
            format!("workspace:root={root}")
        }
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => {
            format!(
                "agent_session:harness_key={harness_key},state_scope={state_scope},session_key={session_key}"
            )
        }
        DeclaredEndpoint::MuxSession { native_id } => {
            format!("mux_session:native_id={native_id}")
        }
        DeclaredEndpoint::Branch {
            repo_common_dir,
            refname,
        } => {
            format!("branch:repo_common_dir={repo_common_dir},refname={refname}")
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => {
            format!("fork:provider_source_key={provider_source_key}")
        }
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            format!(
                "forge_pr:provider={provider},host={host},owner={owner},repo={repo},number={number}"
            )
        }
    }
}

fn parse_endpoint_fields(raw: &str) -> std::result::Result<BTreeMap<&str, &str>, String> {
    if raw.is_empty() {
        return Err(endpoint_syntax_error());
    }

    let mut fields = BTreeMap::new();
    for part in raw.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(endpoint_syntax_error)?;
        if key.is_empty() || value.is_empty() {
            return Err(endpoint_syntax_error());
        }
        fields.insert(key, value);
    }
    Ok(fields)
}

fn required_field(fields: &BTreeMap<&str, &str>, key: &str) -> std::result::Result<String, String> {
    fields
        .get(key)
        .map(|value| (*value).to_string())
        .ok_or_else(|| format!("missing endpoint field `{key}`"))
}

fn endpoint_syntax_error() -> String {
    "invalid endpoint syntax; expected type:key=value,... using declared TOML field names"
        .to_string()
}

/// Resolve which config file a write should target.
///
/// `Some(Project)` / `Some(User)` short-circuit the nearest-store walk;
/// `Some(All)` is rejected because writes have to pick exactly one store.
/// When `store` is `None`, run discovery from the scan roots and ask
/// [`select_store_for_declaration`] to pick the nearest project store,
/// falling back to user config.
fn resolve_write_store(
    store: Option<DeclaredStoreFlag>,
    source: Option<&DeclaredEndpoint>,
    target: Option<&DeclaredEndpoint>,
    scan_roots: &[PathBuf],
) -> Result<PathBuf> {
    match store {
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for write commands; pick `project` or `user`")
        }
        Some(DeclaredStoreFlag::User) => {
            let loader = ConfigLoader::from_env();
            loader.user_config_path().ok_or_else(|| {
                anyhow!("no user config path available; set $HOME or $XDG_CONFIG_HOME")
            })
        }
        Some(DeclaredStoreFlag::Project) => project_store_path(scan_roots),
        None => match (source, target) {
            (Some(source), Some(target)) => {
                let snapshot = discover_for_store_selection(scan_roots)?;
                let loader = ConfigLoader::from_env();
                let selection = select_store_for_declaration(source, target, &snapshot, &loader)
                    .ok_or_else(|| {
                        anyhow!(
                            "could not pick a declared-link store; \
                             pass --store user or --store project"
                        )
                    })?;
                Ok(selection.path)
            }
            _ => bail!(
                "automatic store selection requires both source and target endpoints; \
                 use --store user or --store project"
            ),
        },
    }
}

fn project_store_path(scan_roots: &[PathBuf]) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let loader = ConfigLoader::from_env();
    let roots = effective_scan_roots(scan_roots, &cwd);

    for root in &roots {
        if let Some(path) = loader.locate_project_config(root) {
            return Ok(path);
        }
    }
    // No existing project config along any scan root: fall back to the
    // first scan root (or cwd) and create one there.
    let fallback = roots.first().cloned().unwrap_or(cwd);
    Ok(fallback.join(PROJECT_CONFIG_FILENAME))
}

/// Effective list of roots used for nearest-store probing. If the caller
/// did not pass any `--scan-root`, we default to the current working
/// directory.
fn effective_scan_roots(scan_roots: &[PathBuf], cwd: &Path) -> Vec<PathBuf> {
    if scan_roots.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        scan_roots.to_vec()
    }
}

fn discover_for_store_selection(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    conspectus::discovery::discover_local_at_roots(roots)
}

/// Candidate stores the read-modify-write helpers should look in when
/// removing or mutating an existing declaration. Order matters: writes
/// stop at the first store that holds a matching id.
fn candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let mut paths = Vec::new();

    let include_project = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::Project)
    );
    let include_user = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::User)
    );

    if include_project {
        let cwd = std::env::current_dir()?;
        let roots = effective_scan_roots(scan_roots, &cwd);
        let mut seen = BTreeSet::new();
        for root in roots {
            if let Some(path) = loader.locate_project_config(root)
                && seen.insert(path.clone())
            {
                paths.push(path);
            }
        }
    }

    if include_user && let Some(path) = loader.user_config_path() {
        paths.push(path);
    }

    Ok(paths)
}

fn store_search_label(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        "any declared-link store".to_string()
    } else {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Borrow checker convenience: lets us reuse the existing
/// [`DeclaredStoreSelection`] type for emitted CLI messages.
fn _selection_display(selection: &DeclaredStoreSelection) -> String {
    let kind = match selection.kind {
        DeclaredStoreKind::Project => "project",
        DeclaredStoreKind::User => "user",
    };
    format!("{kind} {}", selection.path.display())
}
