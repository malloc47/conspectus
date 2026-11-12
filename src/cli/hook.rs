//! `conspectus hook` subcommand tree.
//!
//! Four subcommands: `write` (records one hook payload; it lives in
//! [`super::hook_write`]), `init` / `remove` (install or remove each
//! harness's hook: settings entries for Claude Code, one per event it
//! reports, in [`super::hook_claude`]; `config.toml` entries for Codex,
//! in [`super::hook_codex`]; an owned extension file for Pi), and
//! `status` (reports whether the hook is installed, then whether it
//! feeds live state; that line comes from [`super::hook_health`]).
//!
//! Every helper this module needs is either hook-specific
//! (moved here alongside the args) or comes from external
//! crates. Nothing shared back through `super`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand, ValueEnum};
use toml_edit::DocumentMut;

use super::hook_claude::{
    claude_hook_coverage, coverage_label, ensure_claude_hook, remove_claude_hook,
};
use super::hook_codex::{
    MOVED_NOTE, NoteFor, REVIEW_HINT, checked_text, codex_coverage_label, codex_hook_coverage,
    ensure_codex_hook, hooks_json_note, remove_codex_hook,
};
use super::hook_health::print_health;
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
    /// Report whether a Conspectus hook is installed (for Claude Code and
    /// Codex, event by event) and whether it feeds live state.
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
    /// Command installed into the harness config. For Claude Code and
    /// Codex it also replaces the command of entries Conspectus installed
    /// before.
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
        let mut document = read_json_document(&path)?;
        let install = ensure_claude_hook(
            &mut document,
            self.command.as_deref(),
            &default_hook_command("claude-code"),
        )
        .with_context(|| format!("can't install the Claude Code hooks in {}", path.display()))?;
        let counts = format!("{} added, {} updated", install.added, install.updated);

        if self.dry_run {
            if install.changed() {
                println!(
                    "would install Claude Code hooks in {} ({counts})",
                    path.display()
                );
            } else {
                println!("already installed Claude Code hooks in {}", path.display());
            }
            return Ok(());
        }
        if install.changed() {
            write_json_document(&path, &document)?;
            println!(
                "installed Claude Code hooks in {} ({counts})",
                path.display()
            );
        } else {
            println!("Claude Code hooks already installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_install(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let mut document = read_toml_document(&path)?;
        let install = ensure_codex_hook(
            &mut document,
            self.command.as_deref(),
            &default_hook_command("codex"),
        )
        .with_context(|| format!("can't install the Codex hooks in {}", path.display()))?;
        let counts = format!("{} added, {} updated", install.added, install.updated);
        let text =
            if install.changed() {
                Some(checked_text(&document, true).with_context(|| {
                    format!("can't install the Codex hooks in {}", path.display())
                })?)
            } else {
                None
            };

        match (self.dry_run, text) {
            (true, Some(_)) => {
                println!("would install Codex hooks in {} ({counts})", path.display());
            }
            (true, None) => println!("already installed Codex hooks in {}", path.display()),
            (false, Some(text)) => {
                replace_settings_file(&path, &text)?;
                println!("installed Codex hooks in {} ({counts})", path.display());
            }
            (false, None) => println!("Codex hooks already installed in {}", path.display()),
        }
        println!("{REVIEW_HINT}");
        if let Some(note) = hooks_json_note(&path, NoteFor::Init) {
            eprintln!("{note}");
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
    /// The install lines, then the live-state health line
    /// (`<health>\t<harness>\t<store file>\t<message>`).
    fn run(self) -> Result<()> {
        let (harness, installed) = match self.harness {
            HookHarnessFlag::ClaudeCode => ("claude-code", self.run_claude_status()?),
            HookHarnessFlag::Codex => ("codex", self.run_codex_status()?),
            HookHarnessFlag::Pi => ("pi", self.run_pi_status()?),
        };
        print_health(harness, installed, self.scope_flag());
        Ok(())
    }

    /// The scope's flag as an install hint spells it.
    fn scope_flag(&self) -> &'static str {
        match self.scope {
            HookScopeFlag::User => "",
            HookScopeFlag::Project => " --scope project",
        }
    }

    /// One line for the install as a whole, then one per subscribed
    /// event, each `<label>\tclaude-code\t<path>\t<event>`. Returns
    /// whether any event is installed.
    fn run_claude_status(&self) -> Result<bool> {
        let path = claude_settings_path(self.scope)?;
        let document = read_json_document(&path)?;
        let coverage = claude_hook_coverage(&document);
        let overall = coverage_label(&document, &coverage);
        println!("{overall}\tclaude-code\t{}", path.display());
        for (event, state) in &coverage {
            println!(
                "{}\tclaude-code\t{}\t{event}",
                state.label(),
                path.display()
            );
        }
        if overall == "partial" {
            eprintln!(
                "some events are missing or outdated; `{} hook init claude-code{}` completes the install",
                crate::invocation::invoked_name(),
                self.scope_flag()
            );
        }
        Ok(overall != "not-installed")
    }

    /// One line for the install as a whole, then one per subscribed
    /// event, each `<label>\tcodex\t<path>\t<event>`. Returns whether any
    /// event is installed.
    fn run_codex_status(&self) -> Result<bool> {
        let path = codex_config_path(self.scope)?;
        let document = read_toml_document(&path)?;
        let coverage = codex_hook_coverage(&document);
        let overall = codex_coverage_label(&document, &coverage);
        println!("{overall}\tcodex\t{}", path.display());
        for (event, state) in &coverage {
            println!("{}\tcodex\t{}\t{event}", state.label(), path.display());
        }
        if overall == "partial" {
            eprintln!(
                "some events are missing or outdated; `{} hook init codex{}` completes the install",
                crate::invocation::invoked_name(),
                self.scope_flag()
            );
        }
        if let Some(note) = hooks_json_note(&path, NoteFor::Status) {
            eprintln!("{note}");
        }
        Ok(overall != "not-installed")
    }

    /// Returns whether a Conspectus extension, current or older, is
    /// installed.
    fn run_pi_status(&self) -> Result<bool> {
        let path = pi_extension_path(self.scope)?;
        let state = pi_extension_state(&path)?;
        let label = match state {
            PiExtensionState::Installed => "installed",
            PiExtensionState::Outdated => "outdated",
            PiExtensionState::Absent => "not-installed",
            PiExtensionState::Foreign => "foreign",
        };
        println!("{label}\tpi\t{}", path.display());
        Ok(matches!(
            state,
            PiExtensionState::Installed | PiExtensionState::Outdated
        ))
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
            println!("{verb} Claude Code hooks in {}", path.display());
            return Ok(());
        }
        if changed {
            write_json_document(&path, &document)?;
            println!("removed Claude Code hooks from {}", path.display());
        } else {
            println!("Claude Code hooks not installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_remove(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let mut document = read_toml_document(&path)?;
        let removal = remove_codex_hook(&mut document);
        let text =
            if removal.changed {
                Some(checked_text(&document, false).with_context(|| {
                    format!("can't remove the Codex hooks from {}", path.display())
                })?)
            } else {
                None
            };

        match (self.dry_run, text) {
            (true, Some(_)) => println!("would remove Codex hooks in {}", path.display()),
            (true, None) => println!("not installed Codex hooks in {}", path.display()),
            (false, Some(text)) => {
                replace_settings_file(&path, &text)?;
                println!("removed Codex hooks from {}", path.display());
            }
            (false, None) => println!("Codex hooks not installed in {}", path.display()),
        }
        if removal.moved {
            eprintln!("{MOVED_NOTE}");
        }
        if let Some(note) = hooks_json_note(&path, NoteFor::Remove) {
            eprintln!("{note}");
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
    let text = serde_json::to_string_pretty(value)? + "\n";
    replace_settings_file(path, &text)
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

/// Replace a harness settings file with `text` through a sibling
/// temporary file. A symlinked file (a dotfile manager's) is written
/// through to its target so the link survives, the file keeps its mode,
/// and a read-only file is refused rather than replaced.
fn replace_settings_file(path: &Path, text: &str) -> Result<()> {
    crate::declared::write_atomic(path, text)
        .with_context(|| format!("failed to write {}", path.display()))
}
