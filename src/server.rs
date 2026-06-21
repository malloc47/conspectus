//! `conspectus serve` daemon (P7-006).
//!
//! Long-running process that keeps `graph.sqlite` fresh in the
//! background so concurrent one-shot CLI invocations (and the
//! TUI) read warmed-up data on every open. The daemon spawns one
//! worker thread per provider class (harness / mux / git / forge
//! per ADR 0079) and each thread re-runs *only its class's*
//! discovery providers on its own `[server.intervals]` cadence.
//!
//! Layers A, B, and the shutdown half of layer C (this file):
//! per-class scheduling with cycle-level failure isolation and
//! SIGINT/SIGTERM graceful shutdown per ADR 0080. The mutation
//! socket from ADR 0038 lands in the second half of layer C.
//! Today the daemon shares the same writer-lock discipline as
//! the one-shot CLI: both call [`crate::query::persist_snapshot`],
//! which serializes via SQLite's `busy_timeout` (ADR 0037). To
//! keep per-thread cycles atomic across load-prior + evict + run
//! + persist, every thread takes a process-local
//! [`std::sync::Mutex`] before its cycle, so two class threads
//! cannot race on a load/merge/write sequence and silently
//! clobber each other's slice. The mutation socket upgrades that
//! to a dedicated writer thread plus a request queue when it
//! lands.
//!
//! Lifecycle expectations:
//!
//! * The process is user-managed (systemd user unit, launchd
//!   agent, or `conspectus serve &`) per ADR 0038. The CLI
//!   must not auto-spawn it.
//! * SIGINT and SIGTERM flip a shared shutdown flag via
//!   `signal-hook` (ADR 0080). Every scheduler thread polls the
//!   flag between sleeps so a shutdown that arrives mid-tick
//!   still completes the cycle in progress before exiting. The
//!   200ms poll cadence trades a negligible CPU floor for
//!   snappy Ctrl-C response.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::config::ServerIntervals;
use crate::discovery::cache::ProviderClass;
use crate::discovery::{LocalDiscoveryConfig, discover_local_warm_with};
use crate::model::GraphSnapshot;
use crate::query::{load_cached_snapshot, persist_snapshot};
use crate::resolve::resolve_snapshot;

/// Inputs to the daemon main loop. Built by the CLI shell from
/// `[server.intervals]` + `--scan-root` + the discovered cwd.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Discovery scan roots. Empty means "use the process cwd"
    /// just like the one-shot CLI's default.
    pub scan_roots: Vec<PathBuf>,
    /// Per-class warm-start TTL intervals. Each class gets its
    /// own scheduler thread that ticks on its interval.
    pub intervals: ServerIntervals,
}

/// Spawn the per-class scheduler threads and block on the
/// shutdown signal (SIGINT or SIGTERM, registered via
/// `signal-hook` per ADR 0080). Returns once every worker
/// thread has observed the shutdown flag, finished its in-flight
/// cycle, and exited.
pub fn run(config: ServeConfig) -> Result<()> {
    eprintln!(
        "conspectus serve: starting; per-class scheduler; scan roots = {:?}",
        config.scan_roots
    );

    // Single process-local writer lock. Each class thread takes
    // it before its load-prior + evict + run + persist sequence
    // so two threads cannot race on the merge step. Briefly held
    // for the entire cycle; for v1 this trades throughput
    // (forge's minutes-long discovery blocks harness/mux) for
    // correctness. A future layer splits this into a writer
    // thread + a request channel so the load is back-pressured
    // rather than serialized.
    let writer_lock = Arc::new(Mutex::new(()));

    let scan_roots = Arc::new(config.scan_roots);
    let intervals = Arc::new(config.intervals);

    // Shutdown latch per ADR 0080. signal-hook flips this atomic
    // on SIGINT/SIGTERM; every scheduler thread polls it between
    // sleeps so a shutdown that arrives mid-tick still completes
    // the cycle in progress before exiting.
    let shutdown = Arc::new(AtomicBool::new(false));
    register_shutdown_signals(&shutdown).context("install signal handlers for SIGINT/SIGTERM")?;

    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for class in ProviderClass::all() {
        let writer_lock = Arc::clone(&writer_lock);
        let scan_roots = Arc::clone(&scan_roots);
        let intervals = Arc::clone(&intervals);
        let shutdown = Arc::clone(&shutdown);
        let class = *class;
        handles.push(thread::spawn(move || {
            class_loop(class, &scan_roots, &intervals, &writer_lock, &shutdown);
        }));
    }

    // Join every thread. With the shutdown latch in place, each
    // loop exits cleanly when SIGINT/SIGTERM is observed; we
    // wait for all of them so a graceful shutdown surfaces a
    // single "stopped" line on stderr at the end.
    for handle in handles {
        if let Err(panic) = handle.join() {
            eprintln!("conspectus serve: scheduler thread panicked: {panic:?}");
        }
    }

    eprintln!("conspectus serve: stopped");
    Ok(())
}

/// Register SIGINT and SIGTERM against the shared shutdown
/// flag (ADR 0080). Other signals follow the same pattern when
/// they land (SIGHUP for config reload, SIGUSR1 for status
/// dumps).
fn register_shutdown_signals(shutdown: &Arc<AtomicBool>) -> Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    use signal_hook::flag;
    flag::register(SIGINT, Arc::clone(shutdown)).context("register SIGINT handler")?;
    flag::register(SIGTERM, Arc::clone(shutdown)).context("register SIGTERM handler")?;
    Ok(())
}

/// Per-class scheduler loop. Runs one cycle immediately on
/// startup (so the cache is populated before the first sleep)
/// then ticks at the class's interval. Errors per cycle log to
/// stderr and the loop continues so a transient blip does not
/// silently retire the class's refresh duty. The shutdown
/// latch is polled between cycles and at every chunk of the
/// inter-tick sleep so a Ctrl-C does not have to wait up to a
/// full forge interval (5 minutes) to be observed.
fn class_loop(
    class: ProviderClass,
    scan_roots: &[PathBuf],
    intervals: &ServerIntervals,
    writer_lock: &Mutex<()>,
    shutdown: &AtomicBool,
) {
    let interval = class.ttl_duration(intervals);
    eprintln!(
        "conspectus serve: {} scheduler started; interval = {:?}",
        class.name(),
        interval
    );
    run_cycle(class, scan_roots, intervals, writer_lock);
    while !shutdown.load(Ordering::Relaxed) {
        sleep_with_shutdown(interval, shutdown);
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        run_cycle(class, scan_roots, intervals, writer_lock);
    }
    eprintln!(
        "conspectus serve: {} scheduler stopping after shutdown signal",
        class.name()
    );
}

/// Sleep for up to `total`, polling the shutdown flag every
/// 200ms so a signal that arrives mid-sleep is observed quickly.
/// 200ms is short enough for snappy Ctrl-C response and long
/// enough to keep idle CPU near zero.
fn sleep_with_shutdown(total: Duration, shutdown: &AtomicBool) {
    let poll = Duration::from_millis(200);
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if shutdown.load(Ordering::Relaxed) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(poll));
    }
}

/// Acquire the writer lock and run one class cycle. Errors are
/// logged and swallowed so the calling loop keeps going.
fn run_cycle(
    class: ProviderClass,
    scan_roots: &[PathBuf],
    intervals: &ServerIntervals,
    writer_lock: &Mutex<()>,
) {
    // Poisoned-mutex recovery: a panic in a peer class while it
    // held the lock taints it, but the on-disk graph is durable
    // and re-reading prior on the next acquisition heals any
    // half-finished state. Carry on rather than aborting the
    // daemon.
    let _guard = match writer_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Err(err) = try_class_cycle(class, scan_roots, intervals) {
        eprintln!(
            "conspectus serve: {} cycle failed, retrying next tick: {err:#}",
            class.name()
        );
    }
}

/// Load the prior cache, evict this class's slice so the
/// freshness gate marks it as untested, run discovery (which
/// then runs only this class's providers since every other class
/// is still fresh), merge + resolve + persist. The shared writer
/// lock around `run_cycle` ensures no peer thread reads-old +
/// writes between our load and write.
fn try_class_cycle(
    class: ProviderClass,
    scan_roots: &[PathBuf],
    intervals: &ServerIntervals,
) -> Result<()> {
    let mut prior = match load_cached_snapshot(None) {
        Ok(Some(snap)) => snap,
        Ok(None) => GraphSnapshot::empty(),
        Err(err) => {
            eprintln!(
                "conspectus serve: {} failed to read graph cache: {err:#}",
                class.name()
            );
            GraphSnapshot::empty()
        }
    };
    for provider in class.providers() {
        prior.evict_provider(provider);
    }
    let discovery_config = LocalDiscoveryConfig::from_env();
    let snapshot =
        discover_local_warm_with(scan_roots.to_vec(), discovery_config, prior, intervals)?;
    let snapshot = resolve_snapshot(snapshot);
    persist_snapshot(&snapshot, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn writer_lock_recovers_from_poison() {
        // Simulates the per-class panic case: a thread panics
        // while holding the lock, leaving it poisoned. Subsequent
        // acquisitions must still succeed so the surviving class
        // threads can keep ticking — the on-disk SQLite cache is
        // the durable state, not the in-process lock.
        let lock = Arc::new(Mutex::new(()));
        let panicker = {
            let lock = Arc::clone(&lock);
            thread::spawn(move || {
                let _guard = lock.lock().unwrap();
                panic!("simulated class-thread panic");
            })
        };
        let _ = panicker.join();
        // The same `Err -> into_inner` recovery the scheduler uses.
        let _guard = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
    }

    #[test]
    fn class_intervals_use_server_interval_durations() {
        // Pin the mapping: a future refactor that swaps the
        // class -> interval wiring would silently change the
        // scheduler cadence. The test compares against the
        // ServerIntervals defaults.
        let intervals = ServerIntervals::default();
        assert_eq!(
            ProviderClass::Harness.ttl_duration(&intervals),
            Duration::from_secs(5)
        );
        assert_eq!(
            ProviderClass::Mux.ttl_duration(&intervals),
            Duration::from_secs(5)
        );
        assert_eq!(
            ProviderClass::Git.ttl_duration(&intervals),
            Duration::from_secs(30)
        );
        assert_eq!(
            ProviderClass::Forge.ttl_duration(&intervals),
            Duration::from_secs(300)
        );
    }
}
