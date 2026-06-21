//! `conspectus serve` daemon (P7-006).
//!
//! Long-running process that keeps `graph.sqlite` fresh in the
//! background so concurrent one-shot CLI invocations (and the
//! TUI) read warmed-up data on every open. The daemon runs the
//! same warm-start cycle the one-shot CLI runs — load prior,
//! discover skipping fresh providers, merge, resolve, persist —
//! on a tick driven by the shortest interval from
//! `[server.intervals]`.
//!
//! Layer A (this file): single-thread tick loop, cycle-level
//! failure isolation. Per-class scheduling, graceful shutdown,
//! and the mutation socket land in subsequent layers per
//! ADR 0038. The daemon currently shares the same writer-lock
//! discipline as the one-shot CLI: both call
//! [`crate::query::persist_snapshot`], which serializes via
//! SQLite's `busy_timeout` (ADR 0037). The mutation socket
//! upgrades that to a dedicated writer connection when it
//! lands.
//!
//! Lifecycle expectations:
//!
//! * The process is user-managed (systemd user unit, launchd
//!   agent, or `conspectus serve &`) per ADR 0038. The CLI
//!   must not auto-spawn it.
//! * SIGTERM / SIGINT currently terminate the process abruptly.
//!   Because SQLite WAL recovery makes the on-disk graph
//!   consistent on every open, the worst case is losing the
//!   in-flight cycle's discovery work — which the next start
//!   redoes anyway. Graceful shutdown lands with the mutation
//!   socket since that's where in-flight state (open client
//!   connections, the writer transaction queue) actually needs
//!   draining.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::config::ServerIntervals;
use crate::discovery::{LocalDiscoveryConfig, discover_local_warm_with};
use crate::query::{load_cached_snapshot, persist_snapshot};
use crate::resolve::resolve_snapshot;

/// Inputs to the daemon main loop. Built by the CLI shell from
/// `[server.intervals]` + `--scan-root` + the discovered cwd.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Discovery scan roots. Empty means "use the process cwd"
    /// just like the one-shot CLI's default.
    pub scan_roots: Vec<PathBuf>,
    /// Per-class warm-start TTL intervals. The shortest of the
    /// four also drives the daemon's tick cadence — running a
    /// full warm-start cycle at the shortest interval refreshes
    /// each class no less often than its individual TTL while
    /// remaining trivially correct under the freshness gate
    /// (classes whose TTL has not expired are skipped on every
    /// cycle until they do).
    pub intervals: ServerIntervals,
}

/// Block the calling thread on the daemon main loop until the
/// process is terminated. Each iteration runs one warm-start
/// cycle and sleeps for the shortest configured interval. Cycle
/// failures log to stderr and the loop continues so a transient
/// provider blip cannot kill the daemon.
///
/// Returns only on irrecoverable error (currently: never —
/// every cycle failure is logged + swallowed).
pub fn run(config: ServeConfig) -> Result<()> {
    let tick = shortest_interval(&config.intervals);
    eprintln!(
        "conspectus serve: starting; tick interval = {tick:?}, scan roots = {:?}",
        config.scan_roots
    );
    loop {
        if let Err(err) = run_one_cycle(&config.scan_roots, &config.intervals) {
            // Cycle-level isolation. The mutator/provider chain
            // already swallows most provider-specific errors
            // into the snapshot's `diagnostics` field; this
            // catches the residual failure modes (e.g. the
            // writer hitting a permission error) without
            // killing the daemon.
            eprintln!("conspectus serve: cycle failed, retrying next tick: {err:#}");
        }
        std::thread::sleep(tick);
    }
}

/// Single warm-start cycle: load the prior cache, run discovery
/// with the freshness gate, resolve, persist. Mirrors the
/// one-shot CLI's pairing of
/// [`crate::discovery::discover_local_warm_with`] with
/// [`crate::query::persist_snapshot`] so the daemon's writes are
/// byte-for-byte equivalent to a one-shot `conspectus table`
/// invocation.
fn run_one_cycle(scan_roots: &[PathBuf], intervals: &ServerIntervals) -> Result<()> {
    let prior = match load_cached_snapshot(None) {
        Ok(Some(snap)) => snap,
        Ok(None) => crate::model::GraphSnapshot::empty(),
        Err(err) => {
            // Read failures are handled the same way the one-
            // shot CLI handles them: warn + fall back to cold.
            // The post-cycle persist will heal the cache if it
            // is corrupt (the writer's move-aside path).
            eprintln!("conspectus serve: failed to read graph cache: {err:#}");
            crate::model::GraphSnapshot::empty()
        }
    };
    let discovery_config = LocalDiscoveryConfig::from_env();
    let snapshot =
        discover_local_warm_with(scan_roots.to_vec(), discovery_config, prior, intervals)?;
    let snapshot = resolve_snapshot(snapshot);
    persist_snapshot(&snapshot, None)?;
    Ok(())
}

/// The shortest of the four configured class intervals. Drives
/// the daemon's tick cadence in layer A; layer B replaces this
/// with per-class independent timers.
fn shortest_interval(intervals: &ServerIntervals) -> Duration {
    [
        intervals.harness,
        intervals.mux,
        intervals.git,
        intervals.forge,
    ]
    .into_iter()
    .min()
    .unwrap_or_else(|| Duration::from_secs(5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_interval_picks_the_min() {
        let intervals = ServerIntervals {
            harness: Duration::from_secs(5),
            mux: Duration::from_secs(3),
            git: Duration::from_secs(30),
            forge: Duration::from_secs(300),
        };
        assert_eq!(shortest_interval(&intervals), Duration::from_secs(3));
    }

    #[test]
    fn shortest_interval_defaults_are_5s() {
        // The default ServerIntervals has harness=mux=5s, so the
        // tick lands at 5s.
        assert_eq!(
            shortest_interval(&ServerIntervals::default()),
            Duration::from_secs(5)
        );
    }
}
