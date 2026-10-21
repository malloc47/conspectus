# ADR 0116: Lazy Forge Cadence And A Hard Refresh

## Status

Accepted. Refines ADR 0115 (forge discovery over every discovered repo) and
ADR 0038 (the forge interval default). Amended by ADR 0136: listings are
fetched in the background off the daemon's writer lock, routine cycles and
TUI refreshes only read the cache, and only an explicit refresh fetches
synchronously.

## Context

ADR 0115 made forge discovery ask about every discovered repo: two `gh pr
list` calls per GitHub remote. Each call is a network round trip of a second
or more. The forge interval was five minutes, but in practice `gh` ran more
often than that:

- The freshness gate skips a class whose provenance stamps are younger than
  its interval. A repo with no PRs leaves no `github` stamps, so the gate
  never skipped forge. Only a five-minute in-memory cache, keyed by the whole
  repo set, stood between every discovery cycle and a full round of `gh`
  calls.
- The cache was keyed by the repo set, so one new session in a new repo
  missed it and every remote was queried again.
- The cache restamped its fragment each cycle, so a PR's provenance showed
  when it was last reused rather than when the forge answered.
- PR previews ran `gh pr view` each time the selection moved onto a PR,
  and again on every `r`.

Pull requests change on a human timescale, and the operator usually knows
when one changed because they just opened or merged it. Forge reads should be
rare by default, with a way to ask for them now.

## Decision

1. **Forge reads run at startup and then once per forge interval, which
   defaults to one hour.** `[server.intervals] forge` keeps its name and
   now defaults to `1h`. A process that runs discovery (the daemon, a
   daemonless TUI, a one-shot CLI command) has no listings when it starts,
   so its first discovery asks the forges.
2. **Listings are cached per remote.** The process keeps each remote's raw
   `gh pr list` answers with the time they arrived. Discovery reuses a
   listing younger than the forge interval and rebuilds the graph fragment
   from it each cycle, so branch links follow the current local branches
   without a refetch. A remote seen for the first time costs only its own
   queries.
3. **PRs carry the time the forge answered.** The freshness stamps on forge
   nodes and links are the listing's fetch time, so the gate and anything
   that reads freshness see the PR's real age.
4. **Failures retry sooner and keep the last answer.** A listing with a
   failed query is reused for at most five minutes, so a laptop that comes
   back online doesn't wait an hour. A failed refetch keeps the PRs the remote
   last returned, with their original fetch time, alongside the failure
   diagnostic.
5. **Ctrl-R is a hard refresh.** In the TUI, `r` rescans the live classes
   and reuses forge listings. `Ctrl-R` also drops the forge listings, asks a
   running daemon to refresh its forge class, and marks cached PR previews
   stale. The status bar shows "Refreshing forges" while it runs. Without a
   daemon, the TUI's own discovery refetches.
6. **Explicit daemon refreshes refetch; scheduled ones don't.** `conspectus
   refresh --class forge` and a full `conspectus refresh` drop the daemon's
   listings first. Scheduled cycles, including the forge class's own hourly
   tick, reuse listings until they expire. That avoids a double fetch at
   startup, when another class's first cycle has already fetched.
7. **PR previews follow the forge cadence.** A loaded PR preview is reused
   across selections and `r` until it is older than the forge interval or a
   hard refresh marks it stale. A preview that failed to load retries the
   next time the PR is selected.

## Consequences

- With the default interval, `gh` runs about once an hour per remote
  instead of every five minutes, whether or not the repos have PRs.
- A PR opened or merged a minute ago appears after Ctrl-R, after the next
  forge interval, or in any freshly started process. The PRs view's empty
  state and status bar point at Ctrl-R.
- An operator who wants the old behavior sets `forge = "5m"`.
- Listings live only in memory for the life of the process, and the daemon
  and a daemonless TUI keep separate copies. Nothing new is persisted
  (ADR 0086), and the listings are not payload.
- One-shot CLI commands without a daemon still query the forges on every
  invocation, since each invocation starts with no listings. Running
  `conspectus serve` avoids that.

## Alternatives Considered

- **Keep the freshness gate as the only cadence.** It can't see forge reads
  that found no PRs, which is the common case for personal repos, and making
  the gate see them would need a stamp with no node behind it.
- **Persist listings to disk so restarts don't refetch.** This would save a
  round of calls per restart, but adds a cache file for data that is cheap
  to refetch once. "On startup" is also when an operator expects current
  data.
- **Clear the listings on every scheduled forge tick.** That makes the tick
  the only clock, but double-fetches at daemon start.
- **Bind the hard refresh to `R`.** `R` is rename. `Ctrl-R` follows the
  browser convention that a modified reload bypasses caches.
