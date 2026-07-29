# ADR 0091: Serve Idle Cost And Class-Gated Mutators

## Status

Accepted. The class-gated process-tree mutator (the primary win) is
implemented in `discover_local_warm_with` / `apply_mutators`
(`src/discovery/mod.rs`) with the gating helpers in
`src/discovery/cache.rs`. The config-reuse micro-optimization was
evaluated and dropped as negligible (see Consequences).

## Context

`conspectus serve` (ADR 0038 / ADR 0079) consumes a nontrivial amount
of CPU (and some disk I/O) even at idle. `H-SERVE-PERF-001` asked for a
diagnosis of what contributes. This ADR records the findings and the
decision on how to reduce the dominant cost.

### How the daemon spends a cycle

The daemon spawns one scheduler thread per provider class (harness,
mux, git, forge), each firing on its own `[server.intervals]` cadence
(`harness=5s`, `mux=5s`, `git=30s`, `forge=5m`) plus a socket-listener
thread. Every class tick calls `try_class_cycle`
(`src/server/mod.rs`), which:

1. Rebuilds `LocalDiscoveryConfig::from_env()`.
2. Evicts the class's provider slices from the prior snapshot and
   calls `discover_local_warm_with`, which re-runs only that class's
   heavy providers (the freshness gate skips the fresh ones per
   ADR 0079).
3. **Runs `apply_mutators` unconditionally**, regardless of which
   class triggered the cycle.
4. Calls `resolve_snapshot` over the whole graph.
5. Calls `publish_snapshot`: re-serializes the entire snapshot to
   `graph.bin` and writes it (atomic rename), plus updates the
   in-memory caches.

### Attributed contributors (idle, in rough order)

1. **Process-tree `/proc` walk on every cycle.** `apply_mutators`
   runs `cross_link::infer` followed by
   `cross_link::active_harness_pids_per_mux(LinuxProcSnapshot)` on
   every discovery, gated only by `process_tree_enabled` (default
   on). Because harness (5s) and mux (5s) tick independently, the
   `/proc` walk runs roughly every 2.5s. On a host with many
   processes this walk — enumerate `/proc`, read `stat`/`cmdline` per
   pid — is the dominant idle CPU cost.

2. **Full re-resolve + re-serialize + `graph.bin` write every cycle.**
   `resolve_snapshot` + `publish_snapshot` run on every class tick,
   even when the triggering class's slice did not change. A byte-level
   "skip write when unchanged" does *not* help here: `node_provenance`
   carries a per-node `freshness_epoch` that the gate restamps each
   cycle, so the serialized bytes differ every time even when the
   graph data is identical.

3. **`LocalDiscoveryConfig::from_env()` rebuilt per cycle** —
   re-instantiates mux backends, forge adapters, and walks the
   orchestrator registry each tick. Minor but repeated.

4. **200ms shutdown-poll across five threads** — each scheduler
   thread wakes ~5×/s to check the shutdown flag between interval
   sleeps. A negligible CPU floor, not worth changing.

## Decision

Reduce idle cost primarily by making the expensive process-tree
mutator **class-gated** instead of always-run, and secondarily by
reusing a cached discovery config.

### Class-gated process-tree mutator

The agent↔pane process-tree linkage
(`cross_link::active_harness_pids_per_mux` and the aux-attribution it
feeds) derives from **mux** and **harness** state. It is meaningless
to recompute it on a git-only or forge-only cycle, where neither mux
sessions nor harness processes were re-observed. The decision:

- Run the `/proc` process-tree walk only when the triggering cycle
  re-ran the mux or harness providers (i.e. those slices were not
  fresh-skipped by the gate). On git/forge-only cycles, skip it.
- To keep the prior agent↔pane links visible across skipped cycles,
  the process-tree contribution moves out of the freshness gate's
  unconditional "always-evict" mutator bucket and into a slice that
  is evicted **only when mux or harness re-runs**. This mirrors how
  ordinary provider slices already survive cycles that don't touch
  them (ADR 0079).
- The cheap, in-memory `cross_link::infer` (cwd/evidence-based
  agent↔mux linking, no syscalls) continues to run every cycle; only
  the `/proc`-walking pid attribution is gated.

The gating decision is a pure function of the freshness gate (which
provider keys re-ran this cycle), so it is unit-testable without a
running daemon.

### Config reuse

Build `LocalDiscoveryConfig` once and reuse it across cycles rather
than calling `from_env()` on every tick. The env inputs it reads do
not change within a daemon lifetime.

### Explicitly rejected for now

- **Skip-on-unchanged `graph.bin` publish.** Defeated by the
  per-cycle `freshness_epoch` churn in `node_provenance` (see
  Context). Revisit only if provenance epochs are excluded from the
  publish-equality check, which is a larger change with its own
  correctness questions.
- **Widening the default intervals.** Changes freshness semantics the
  operator relies on; a config knob already exists for operators who
  want it. Not a default change.

## Consequences

- Idle CPU drops because the `/proc` walk runs on the mux/harness
  cadence that actually feeds it, not on every git/forge tick as
  well. On a quiet host where only git (30s) and forge (5m) tick, the
  walk stops running between mux/harness ticks entirely.
- The freshness gate gains one more class-gated slice (the
  process-tree contribution). This is the delicate part: the eviction
  rule for that slice must be tied to mux/harness re-runs, or
  agent↔pane links will either stale out (evicted but not
  recomputed) or go missing. The implementation must land with tests
  covering: git-only cycle preserves prior process-tree links;
  mux/harness cycle recomputes them; process-tree-disabled config is
  unaffected.
- `resolve_snapshot` + `publish_snapshot` still run every cycle. That
  cost is left in place for now; the process-tree walk is the larger
  contributor and the lower-risk win.
- **Config reuse dropped.** `LocalDiscoveryConfig` is *consumed* by
  `discover_local_warm_with` (it drains the boxed mux/forge backends),
  so it must be rebuilt each call regardless; and `from_env()` only
  reads env vars and constructs cheap structs (no I/O). Caching a
  consumed value is awkward for negligible gain, so this
  micro-optimization was not pursued.

## Alternatives Considered

**Land only the safe wins (config reuse, publish skip).** Rejected as
insufficient: the publish skip doesn't fire (freshness-epoch churn),
and config reuse alone is minor. The process-tree walk is the actual
cost and needs the gate change.

**A separate TTL for the process-tree mutator, independent of any
provider class.** Rejected as redundant: the walk's inputs are exactly
the mux + harness slices, which already have TTLs. Tying it to those
re-runs reuses the existing cadence rather than inventing a fourth
knob.

**Move the daemon to a single-thread event loop.** Out of scope. The
per-class thread model (ADR 0038) is not the cost; the work each cycle
does is.

## Related ADRs

- ADR 0038 (serve daemon and per-class scheduler).
- ADR 0079 (warm-start freshness gate and provider classes) — this
  ADR extends the gate with a class-gated mutator slice.
- ADR 0083 (`graph.bin` zero-copy snapshot) — the artifact re-written
  each publish.

## Retrospective (2026-07-28)

The class-gated mutator landed as 001a and dropped CPU from 40% to
5-10%. Operator profiling on a live daemon then surfaced a chain of
additional cost centers that the original ADR did not anticipate.
Each was addressed in turn.

| # | Commit | What it targeted | Effect |
| --- | --- | --- | --- |
| 001a | `47d128d` | class-gate the process-tree mutator (`cross_link` + `codex_log`) so it only fires on cycles that actually re-ran mux/harness | 40% → 5-10% |
| — | `0ec2836` | `class_loop` min-cycle-gap floor; without it a chatty inotify source (Codex's SQLite WAL/SHM) turned watcher-driven cycles into a `wait → run → wait` spin | included above |
| 002 | `7516ff7` | process-lifetime cache for the codex\_log SQLite `LIKE 'pid:<pid>:%'` scan; keyed on `(mtime, size)` of `logs_<N>.sqlite` | speculative, low delta |
| 003 | `a80a7fc` | in-memory mux/harness slice fingerprint gate on the /proc walk; only re-walk when slice content differs from last cycle | small (needed 008/009 refinement) |
| 004 | `d27a987` | per-cwd `GitProbe::probe` cache fingerprinted on `.git/HEAD`, `.git/config`, `.git/refs/heads/`, `.git/packed-refs` mtimes; each probe otherwise spawned 8–15 `git` subprocesses | misattributed as the big win (was small) |
| 005 | `9a691a3` | TTL cache in `ForgeDiscovery`; empty `gh pr list` results produced no `github` stamps, so the freshness gate never marked forge fresh → `gh` respawned every cycle | closes the "empty-fragment providers" hole for forge |
| 006 | `3495367` | `codex::read_session_meta` was `fs::read_to_string` on JSONL files up to ~24 MB just to look at the first line; replaced with `BufReader::read_line` | **167 MB/s → 37 MB/s** |
| 007 | `9b9eec3` | per-file `(mtime, size)` cache for the claude + codex session header/tail scans; on this box 93 of 96 session files were dormant but re-scanned every cycle | file opens: ~900/s → ~zero on dormant files |
| 008 | `70becc5` | `fingerprint_normalized_node` zeros `activity_epoch`, `last_attached_epoch`, `last_active_epoch` — 003's fingerprint was mismatching every cycle on operator boxes with any live tmux | /proc walk: 0.8/s → 0.4/s |
| 009 | `aac498c` | extend 008 to also zero `last_message_preview`, `title`, `created_epoch` — 008 was necessary but not sufficient (any active claude message append still churned the preview) | further /proc walk reduction |
| 010 | `148763d` | mtime cache for `read_sqlite_sessions` on `opencode.db` (25 MB); same shape as 002, opencode uses WAL mode so main-file mtime is stable | **37 MB/s → 549 KB/s** (27× drop, biggest single win) |
| 011 | `03ad86b` | TTL cache in `TmuxDiscovery` and `ZellijDiscovery`; same empty-fragment freshness-gate hole as forge | closes remaining subprocess spawn cost |

**Final state on the operator's box:** ~1% CPU floor with spikes only
on real state changes (session add/remove, pane pid turnover, mux
attach), rchar in the low hundreds of KB/s, from 40% CPU / 617 MB/s
at the start of the investigation. **~40× CPU reduction, ~1100× read
reduction.**

**Method lesson** (learned the hard way): for daemon perf debugging,
run `sudo strace -c -f -p <pid>` on the live process **before**
implementing any speculative fixes. The 001a → 005 sequence was
guided by ADR-time hypotheses about what dominates cost, and every
one of them was misattributed. 006 was found in ~30 seconds by
reading the strace top-3 read sizes (three 24 MB reads matching
codex rollout file sizes). Ground-truth attribution beats hypothesis
for cost work — the profile is not obligated to match the design.

**Not-yet-attempted follow-ups** (deferred pending trigger):

- **001c** — semantic-fingerprint of the resolved graph so
  `graph.bin` isn't re-written every cycle. Deferred because
  `freshness_epoch` churn defeats byte-level equality and a
  semantic fingerprint collides with ADR 0079's freshness gate
  and ADR 0083's warm-restart artifact. The residual cost this
  would address is small after 002–011.
- **`try_class_cycle` per-class evict** (`src/server/mod.rs:1450`)
  bypasses the freshness gate for the local class's providers,
  forcing them to re-run on every class thread wake regardless of
  TTL. Fixing this would let harness genuinely respect its 5s TTL
  (currently runs ~1 Hz throttled) and cut harness cycle rate 4-5×.
  Cosmetic after A-through-011 land the downstream caches.
- **Empty-fragment freshness-gate hole** — the underlying issue that
  005 and 011 both worked around individually. A `provider_last_run`
  sidecar on the snapshot that participates in
  `compute_freshness_gate` would replace both TTL caches with a
  single, correctly-scoped fix. Bigger refactor than either patch.
