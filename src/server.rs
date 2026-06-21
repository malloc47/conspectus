//! `conspectus serve` daemon (P7-006).
//!
//! Long-running process that keeps `graph.sqlite` fresh in the
//! background so concurrent one-shot CLI invocations (and the
//! TUI) read warmed-up data on every open. The daemon spawns one
//! worker thread per provider class (harness / mux / git / forge
//! per ADR 0079) and each thread re-runs *only its class's*
//! discovery providers on its own `[server.intervals]` cadence.
//!
//! Layers A and B (this file): per-class scheduling with
//! cycle-level failure isolation. The mutation socket and
//! graceful shutdown land in layer C per ADR 0038. Today the
//! daemon shares the same writer-lock discipline as the one-shot
//! CLI: both call [`crate::query::persist_snapshot`], which
//! serializes via SQLite's `busy_timeout` (ADR 0037). To keep
//! per-thread cycles atomic across load-prior + evict + run +
//! persist, every thread takes a process-local [`std::sync::Mutex`]
//! before its cycle, so two class threads cannot race on a
//! load/merge/write sequence and silently clobber each other's
//! slice. The mutation socket upgrades that to a dedicated
//! writer thread plus a request queue when it lands.
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
//!   socket since that is where in-flight state (open client
//!   connections, the writer transaction queue) actually needs
//!   draining.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::Result;

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

/// Spawn the per-class scheduler threads and block forever
/// (until the process is signalled). Returns only if every
/// worker thread exits — which they currently don't, since each
/// runs an infinite loop. Layer C will replace the loops with a
/// shared [`std::sync::atomic::AtomicBool`] shutdown latch.
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
    // correctness. Layer C splits this into a writer thread + a
    // request channel so the load is back-pressured rather than
    // serialized.
    let writer_lock = Arc::new(Mutex::new(()));

    let scan_roots = Arc::new(config.scan_roots);
    let intervals = Arc::new(config.intervals);

    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for class in ProviderClass::all() {
        let writer_lock = Arc::clone(&writer_lock);
        let scan_roots = Arc::clone(&scan_roots);
        let intervals = Arc::clone(&intervals);
        let class = *class;
        handles.push(thread::spawn(move || {
            class_loop(class, &scan_roots, &intervals, &writer_lock);
        }));
    }

    // Join every thread. If a thread panics (rather than the
    // typical infinite-loop body), its panic is reported on
    // stderr and the other threads keep running.
    for handle in handles {
        if let Err(panic) = handle.join() {
            eprintln!("conspectus serve: scheduler thread panicked: {panic:?}");
        }
    }

    Ok(())
}

/// Per-class scheduler loop. Runs one cycle immediately on
/// startup (so the cache is populated before the first sleep)
/// then ticks at the class's interval. Errors per cycle log to
/// stderr and the loop continues so a transient blip does not
/// silently retire the class's refresh duty.
fn class_loop(
    class: ProviderClass,
    scan_roots: &[PathBuf],
    intervals: &ServerIntervals,
    writer_lock: &Mutex<()>,
) {
    let interval = class.ttl_duration(intervals);
    eprintln!(
        "conspectus serve: {} scheduler started; interval = {:?}",
        class.name(),
        interval
    );
    run_cycle(class, scan_roots, intervals, writer_lock);
    loop {
        thread::sleep(interval);
        run_cycle(class, scan_roots, intervals, writer_lock);
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
