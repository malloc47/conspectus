//! `conspectus serve`, `refresh`, `status` — daemon lifecycle
//! commands (H-REF-006 wave 7).
//!
//! All three consult the daemon socket first (via
//! `conspectus::server::client_*`) and either return that
//! response or fall through to an in-process path.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, ValueEnum};

use conspectus::config;

use super::{
    cache_resolved_snapshot, current_unix_epoch_for_table, warm_start_discover_and_resolve,
};

#[derive(Debug, Args)]
pub(super) struct ServeArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted, mirroring the one-shot
    /// CLI's discovery surface.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl ServeArgs {
    pub(super) fn run(self) -> Result<()> {
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
        let scan_roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };
        conspectus::server::run(conspectus::server::ServeConfig {
            scan_roots,
            intervals: outcome.config.server.intervals,
        })
    }
}

#[derive(Debug, Args, Default)]
pub(super) struct RefreshArgs {
    /// Discovery scan root for the in-process fallback path
    /// (used only when no daemon is running). When omitted the
    /// fallback uses the process cwd.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Refresh only one provider class (`git`, `mux`,
    /// `harness`, or `forge`) instead of the full graph. With
    /// the daemon running, the per-class refresh evicts only
    /// that class's slice and re-runs only its providers. In
    /// the in-process fallback the same constraint applies, so
    /// the wall-clock cost matches a single class's discovery.
    #[arg(long)]
    class: Option<String>,
}

impl RefreshArgs {
    pub(super) fn run(self) -> Result<()> {
        // Validate the --class value up front so we surface a
        // useful error regardless of whether we route through
        // the daemon or the fallback path.
        if let Some(name) = self.class.as_deref()
            && conspectus::discovery::cache::ProviderClass::parse(name).is_none()
        {
            bail!("unknown --class `{name}`; expected one of git, mux, harness, forge");
        }

        // Try the daemon socket first. If a `conspectus serve`
        // process is running it owns the freshest writer
        // discipline and is also the canonical place to
        // coordinate a refresh.
        match conspectus::server::client_refresh(self.class.as_deref()) {
            conspectus::server::ClientOutcome::Ok(epoch) => {
                match self.class.as_deref() {
                    Some(class) => println!("refreshed {class} via daemon (epoch={epoch})"),
                    None => println!("refreshed via daemon (epoch={epoch})"),
                }
                return Ok(());
            }
            conspectus::server::ClientOutcome::DaemonError { code, message } => {
                bail!("daemon refused refresh ({code}): {message}");
            }
            conspectus::server::ClientOutcome::Transport(err) => {
                eprintln!(
                    "conspectus: warning: daemon socket error, falling back to local refresh: {err:#}"
                );
            }
            conspectus::server::ClientOutcome::NoDaemon => {
                // Expected when no daemon is running. Silent
                // fall-through to the local path; an operator
                // who started `conspectus serve` and didn't see
                // a `refreshed via daemon` line will recognize
                // the absence themselves.
            }
        }

        // Fallback: in-process refresh. For a full refresh, do
        // the same cold-rebuild path the table
        // command runs with `--refresh`. For a per-class refresh
        // we load the prior, evict the class, re-run discovery,
        // resolve, and persist, mirroring the daemon's
        // try_class_cycle.
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
        let roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };
        match self.class.as_deref() {
            None => {
                warm_start_discover_and_resolve(
                    roots,
                    true,
                    false,
                    &outcome.config.server.intervals,
                )?;
                println!("refreshed via in-process cold rebuild");
            }
            Some(name) => {
                // `parse` was validated above.
                let class = conspectus::discovery::cache::ProviderClass::parse(name)
                    .expect("class validated above");
                in_process_class_refresh(class, roots, &outcome.config.server.intervals)?;
                println!("refreshed {name} via in-process per-class refresh");
            }
        }
        Ok(())
    }
}

/// In-process per-class refresh used by `conspectus refresh
/// --class <name>` when no daemon is available. P11-011a: the
/// prior is mmap'd from `graph.bin` when the file exists (so a
/// peer daemon's recent snapshot still seeds the per-class
/// evict-and-rerun); otherwise empty. Resolved snapshot lands
/// in `graph.bin` via `cache_resolved_snapshot` so a subsequent
/// invocation can warm-start the same way.
fn in_process_class_refresh(
    class: conspectus::discovery::cache::ProviderClass,
    roots: Vec<PathBuf>,
    intervals: &conspectus::config::ServerIntervals,
) -> Result<()> {
    let mut prior = load_prior_from_graph_bin();
    for provider in class.providers() {
        prior.evict_provider(provider);
    }
    let discovery_config = conspectus::discovery::LocalDiscoveryConfig::from_env();
    let snapshot =
        conspectus::discovery::discover_local_warm_with(roots, discovery_config, prior, intervals)?;
    let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
    cache_resolved_snapshot(&snapshot, false);
    Ok(())
}

/// Try to mmap the on-disk `graph.bin` snapshot as the
/// warm-start prior for an in-process refresh. Returns an empty
/// snapshot on missing-file, version-mismatch, validation
/// failure, or any other unhappy path — the caller falls
/// through to cold rebuild semantics.
fn load_prior_from_graph_bin() -> conspectus::model::GraphSnapshot {
    let path = conspectus::snapshot::graph_bin_path();
    match conspectus::snapshot::open_mmap(&path) {
        Ok(handle) => match conspectus::snapshot::deserialize_owned(&handle) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                eprintln!(
                    "conspectus: warning: failed to deserialize {}: {err:#}",
                    path.display()
                );
                conspectus::model::GraphSnapshot::empty()
            }
        },
        Err(conspectus::snapshot::SnapshotError::Io(err))
            if err.kind() == std::io::ErrorKind::NotFound =>
        {
            conspectus::model::GraphSnapshot::empty()
        }
        Err(err) => {
            eprintln!(
                "conspectus: warning: failed to read {}: {err:#}",
                path.display()
            );
            conspectus::model::GraphSnapshot::empty()
        }
    }
}

#[derive(Debug, Args, Default)]
pub(super) struct StatusArgs {
    /// Output format. `human` (default) prints one line per
    /// class with last-tick freshness; `json` emits the
    /// machine-readable shape the daemon returns over the
    /// socket.
    #[arg(long, value_enum, default_value_t = StatusFormatFlag::Human)]
    format: StatusFormatFlag,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum StatusFormatFlag {
    #[default]
    Human,
    Json,
}

impl StatusArgs {
    pub(super) fn run(self) -> Result<()> {
        match conspectus::server::client_status() {
            conspectus::server::ClientOutcome::Ok(classes) => {
                match self.format {
                    StatusFormatFlag::Json => {
                        let value = serde_json::to_value(&classes)?;
                        println!("{}", serde_json::to_string_pretty(&value)?);
                    }
                    StatusFormatFlag::Human => render_status_human(&classes),
                }
                Ok(())
            }
            conspectus::server::ClientOutcome::DaemonError { code, message } => {
                bail!("daemon refused status ({code}): {message}");
            }
            conspectus::server::ClientOutcome::Transport(err) => Err(err),
            conspectus::server::ClientOutcome::NoDaemon => {
                println!("no daemon running");
                Ok(())
            }
        }
    }
}

/// Render the daemon-side ClassState map as a one-line-per-class
/// human-readable block. Empty map prints "no class state yet —
/// daemon may have just started." The "yet" framing avoids
/// surprising the operator who started the daemon a beat ago.
fn render_status_human(
    classes: &std::collections::BTreeMap<String, conspectus::server::ClassState>,
) {
    if classes.is_empty() {
        println!("no class state yet — daemon may have just started");
        return;
    }
    let now = current_unix_epoch_for_table().unwrap_or(0);
    for (class, state) in classes {
        let outcome = state.last_outcome.as_deref().unwrap_or("pending");
        let age = match state.last_completed_epoch {
            Some(epoch) => format!("{}s ago", (now - epoch).max(0)),
            None => "—".to_string(),
        };
        let error = state
            .last_error
            .as_deref()
            .map(|m| format!("; error: {m}"))
            .unwrap_or_default();
        println!("{class:8} {outcome:6} last={age}{error}");
    }
}
