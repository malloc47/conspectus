# ADR 0079: `[server.intervals]` Dual Role As CLI Warm-Start TTL

## Status

Accepted

## Context

P7-003 phase 3 graduates the one-shot CLI's warm-start path from a
backstop merge (phase 2) into per-provider selective refresh: read
the persisted `graph.sqlite`, compare each provider's freshness
against a TTL, evict and re-run only the providers whose data has
aged out, and reuse the rest verbatim.

That requires a per-provider TTL number per invocation. Where it
lives in the config schema is a design fork:

- A new dedicated `[caching]` block, keyed by the granular
  provenance string the per-node sidecar already uses
  (`git::cwd`, `claude-code`, `tmux`, `github`, …).
- Reuse the `[server.intervals]` table ADR 0038 already documents
  for the daemon's per-provider refresh cadence (`harness`, `mux`,
  `git`, `forge`).

`[server.intervals]` is already on the roadmap and already named
in ADR 0038 as the daemon's source of truth. The semantic content
of the two numbers is the same: "the daemon refreshes class X at
this cadence; the CLI considers class X stale after this much
wall-clock time." Recording the same number twice — once for the
daemon, once for the CLI — drifts.

The two existing per-row provenance shapes:

- `node_provenance[node_id].provider` — the granular string the
  adapter emits when discovering a node (`git`, `git::cwd`,
  `claude-code`, `codex`, `tmux`, `github`, …).
- `candidate_links[i].source_metadata.adapter` — the same
  granular string for the link side.

Heavy providers, mutators, and the granularity gap between
"provenance string" and "interval class" all need a written
mapping.

## Decision

`[server.intervals]` is the single per-class refresh-cadence source.
Both the daemon (`P7-006`) and the one-shot CLI's warm-start path
(`P7-003` phase 3) read from it; the daemon treats each value as
"refresh this often," the CLI treats it as "any slice older than
this is stale."

### Schema (unchanged from ADR 0038)

```toml
[server.intervals]
harness = "5s"   # claude-code, codex, opencode, aider state scans
mux     = "5s"   # tmux session enumeration
git     = "30s"  # local git repo discovery + observed-cwd probe
forge   = "5m"   # gh-backed PR/fork lookups
```

Values are duration strings (`"30s"`, `"1m"`, `"500ms"`, …).
Defaults match ADR 0038. An absent block falls back to defaults; an
absent key inside the block falls back to its default; a
malformed value produces a `ConfigDiagnostic` and that key falls
back to its default. The one-shot CLI does not need `socket_path`
and ignores it for warm-start purposes (P7-006 reads it).

### Provenance string → interval class mapping

| Provenance string                                | Class     |
| ------------------------------------------------ | --------- |
| `git`, `git::cwd`                                | `git`     |
| `atelier`, `generic_workspace`, `agent_deck`    | `git`     |
| `tmux`                                           | `mux`     |
| `claude-code`, `codex`, `opencode`, `aider`     | `harness` |
| `github`                                         | `forge`   |

Rationale: `atelier` / `generic_workspace` / `agent_deck` discover
workspace trees on disk via filesystem walks rooted in repos, so
their cost profile matches the `git` adapter family rather than
the per-second `mux` enumeration or the long-tail `forge` HTTP
calls. Any provenance string not in this table maps to **no
class** — see "Always-rerun providers" below.

### Always-rerun providers (mutators + unclassified)

The mutator passes (`cross_link`, `codex_log`, `hook_sidecar`,
`declared`) read the merged snapshot rather than discovering
state independently, so their outputs depend on what every other
provider produced this run. They always re-run; their slices in
the prior snapshot are evicted unconditionally before the merge so
stale mutator-derived nodes/links never linger after upstream
deletion.

Unmapped provenance strings (anything not in the class table
above) join the always-rerun set. The conservative default is
correct for in-development providers whose cost profile has not
been characterized.

### Warm-start algorithm

For each one-shot CLI invocation (default: not `--refresh`):

1. Read the persisted `GraphSnapshot` via
   `query::load_cached_snapshot`. Absent file → cold rebuild.
2. Compute the freshness gate: for each *heavy* provider
   (anything with an interval class), scan
   `node_provenance` + `candidate_links` for entries belonging
   to that provider and find the max `freshness_epoch`. Compare
   `now - max_epoch` against the configured class interval. The
   provider is *fresh* if every slice it owns is within the
   interval; otherwise *stale*. A provider with no entries at
   all is *untested* (treat as stale: run it).
3. Evict the stale providers' slices from the prior snapshot via
   the P7-005 `evict_provider` primitive. Also evict all
   always-rerun providers' slices.
4. Run the stale + untested heavy providers (skip the fresh
   ones).
5. Merge: `merge_with_prior(fresh, evicted_prior)`. Fresh wins on
   every collision via first-write-wins; the cached fresh slices
   survive as the prior's contribution since the live run did
   not emit them.
6. Run the always-rerun mutators against the merged snapshot.
7. Resolve, render, persist.

`--refresh` bypasses steps 1-4 entirely and runs every provider
cold (phase 2 behavior with prior == empty). `--no-cache`
continues to suppress the writer in step 7.

### Provider keys remain granular

The per-node `NodeProvenance.provider` and per-link
`source_metadata.adapter` strings stay at their existing
granularity (`git::cwd` distinct from `git`,
`claude-code` distinct from `codex`). The class table is the
only place granular strings collapse; the rest of the codebase
keeps the per-emit identity for diagnostics, debugging, and the
P7-006 daemon's per-provider failure isolation.

## Consequences

- The `[server.intervals]` table becomes load-bearing for the
  one-shot CLI even when no `conspectus serve` process is
  running. Operators tuning daemon cadence implicitly tune CLI
  warm-start aggressiveness; the dual role is explicit in this
  ADR so neither path can silently drift from the other.
- The class table lives in code (`discovery::cache::provider_class`)
  rather than as another config knob. Adding a new provider means
  picking a class explicitly, which is the right friction — a
  silent default ("treat as `harness`") would let new heavy
  providers ride a too-short TTL.
- Mutator slices in the prior are always evicted. The CPU cost of
  re-running `cross_link` etc. is paid every invocation; this
  matches the pre-phase-3 baseline (every invocation already
  re-ran every mutator) so no regression.
- Cache hits never extend a provider's freshness_epoch — only
  re-running the provider does. A provider that stays fresh for
  many CLI invocations eventually crosses the TTL boundary and
  gets re-run; there is no "warm-start cascade" risk where the
  cache perpetually re-stamps itself.
- The phase-2 backstop semantics
  (`merge_with_prior(fresh, prior)`) survive intact; phase 3
  upgrades `prior` from "always the full cached snapshot" into
  "the cached snapshot with stale and mutator slices evicted."
  The merge primitive itself is unchanged.

## Alternatives Considered

**Dedicated `[caching]` block keyed by granular provenance
string.** Rejected to avoid recording the same per-class number
twice in two different shapes. Granular per-provider TTL overrides
are a real future need (e.g., bumping a single broken provider's
TTL) but can land later as a class-overrides extension within
`[server.intervals]` or as a separate `[caching.overrides]` block
without invalidating this ADR. The class-level number is enough
for v1.

**Per-provider TTL stored in `provider_state` SQL table.** The
ADR 0037 `provider_state` table records *outcome* and *last run
at*, not *configured TTL*. TTL is a user/operator concern; it
belongs in the config file the operator edits, not the database
the daemon writes. Keeping the two separate also avoids a class
of confusing migrations when the operator wants to bump a TTL
("why is the new TTL not taking effect?"; "because the DB row
hasn't been refreshed yet" is a bad answer).

**Hardcoded TTL constants with no config.** Rejected because the
operator-tuning use case is real: a forge with a flaky `gh` will
want a much shorter TTL than the default; a repo on slow disk
might want a longer one. Shipping with no knob and waiting for
users to ask is the worse default than picking a sensible
five-class shape now.

**No mutator eviction (let mutators accumulate stale links).**
Rejected because deleted upstream state would never disappear
from the visible graph. The CPU cost of re-running mutators
every invocation is small (they read in-memory data) and the
correctness payoff is large.

## Open Questions

- Should `--refresh` accept a comma-separated list of provider
  classes to refresh selectively (e.g.,
  `--refresh=forge,mux`)? Useful for "I just ran `gh pr create`,
  refresh forge but keep my warm caches." Deferred until a real
  request lands; phase 3 ships the all-or-nothing flag.
- Per-class default TTLs: the ADR 0038 starting point
  (`5s/5s/30s/5m`) targets the *daemon's* refresh cadence, which
  may be aggressive for the one-shot CLI. Whether to fork the
  defaults between daemon and CLI (or whether 5s for mux is
  exactly right for both) can be revisited after the daemon
  lands and we have telemetry. The shared table stays whichever
  way the numbers settle.
