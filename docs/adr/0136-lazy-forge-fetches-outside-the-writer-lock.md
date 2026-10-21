# ADR 0136: Lazy Forge Fetches Outside The Writer Lock

## Status

Accepted. Amends ADR 0116 (items 1, 2, 5, and 6: when forge listings are
fetched and by whom) and ADR 0115 (item 5: the concurrency cap).

## Context

ADR 0116 made forge reads rare by caching each remote's listing for the
forge interval. It did not say which cycle fetches. Every daemon cycle
runs forge discovery after the other providers, and the first cycle to
find a listing missing (after a daemon restart) or expired (hourly) runs
the full round of `gh pr list` calls itself. With the enterprise hosts
of ADR 0128 that round takes 9-13 s on the operator machine, and it runs
while the cycle holds the daemon's single writer lock.

After a tmux detach the TUI nudges the daemon to rescan the mux and
harness classes (ADR 0104) and dims the hand-off rows until the result
lands (ADR 0108). The nudge waits on the writer lock: 9.95 s during a
forge fetch against 0.14-0.23 s otherwise. A daemonless TUI ran the same
fetch inside whichever refresh found listings stale, the hand-off's
included.

Forge state changes on a human timescale and the operator usually knows
when it changed. Nothing the operator does in tmux should wait on it.

## Decision

1. **Discovery has a forge fetch policy.** `LocalDiscoveryConfig` and
   the discovery context carry `ForgeFetch::Fetch` (the default: fetch
   missing or expired listings, as before) or `ForgeFetch::CachedOnly`
   (build PRs from cached listings of any age, never run `gh`, and look
   up signed-in hosts only from the cache).
2. **Routine work is cached-only.** The daemon's mux, harness, and git
   cycles, a `refresh` of those classes, and every refresh of a live TUI
   (timer, hand-off, rename, a launch that ends without an attach) use
   `CachedOnly`. They rebuild PR nodes and branch links from the cached
   listings, which is cheap, so local branch changes still show.
3. **Fetching runs off the writer lock, in the background.** The
   daemon's forge class no longer runs a cycle on its interval. It polls
   every 10 s: it reads the published snapshot's repos and fetches the
   listings that are missing or older than the forge interval (or than
   five minutes after a failure, ADR 0116 item 4), without the writer
   lock. When it fetched anything it takes the lock for a short
   cached-only cycle that merges the result. A repo a session just
   reached gets its PRs within the poll period plus its own fetch. A
   separate fetch lock keeps two fetches from running at once.
4. **An explicit refresh still fetches synchronously.** Ctrl-R
   (`refresh --class forge` on the daemon), `conspectus refresh`, and
   `conspectus refresh --class forge` drop the listings and fetch for
   the caller, holding the fetch lock but not the writer lock, then run
   their cycle. The caller waits; nobody else does.
5. **A daemonless TUI has its own background fetch.** While the TUI
   has no daemon, it runs the same fetch on a worker thread at most
   every 10 s, one at a time, and refreshes when the worker fetched
   something. Its Ctrl-R fetches in the refresh it spawns.
6. **One-shot commands are unchanged.** `graph`, `table prs`, `node`,
   and the daemonless `refresh` fallback fetch as before (`Fetch`),
   since the operator asked for output that includes PRs and the
   process has nothing cached. The pin commands keep skipping forge
   discovery entirely (ADR 0135).
7. **The concurrency cap is configurable.** `[forge] concurrency`
   (default 4, ADR 0115) sets how many `gh` calls run at once. Zero or a
   non-integer is a config diagnostic and keeps the default.

## Consequences

- Nudges, hand-off refreshes, and timer refreshes take the time of the
  local rescans regardless of the network, `gh`, or the number of
  remotes.
- After a daemon restart, PRs appear on the first forge poll after the
  other classes publish, about 10 s plus the fetch, instead of during
  the first cycle. The tree is usable meanwhile.
- The forge class's status line reports its last poll. A poll that
  fetched something also ran the merge cycle.
- A fetch can see a published snapshot a cycle old, so a repo found in
  that cycle waits for the next poll.
- `[forge] concurrency` trades fetch time against concurrent requests
  to a forge; a high value can trip a forge's secondary rate limits.

## Alternatives Considered

- **Keep fetching in cycles but raise the concurrency.** Shortens the
  stall to a few seconds but keeps the network behind the writer lock.
- **Release the writer lock around the fetch inside a cycle.** The cycle
  reads its prior from the published snapshot before the fetch, so
  another class's cycle in between would be overwritten.
- **Give live nudges a separate lock.** Two cycles could then publish
  out of order.
- **Persist listings across daemon restarts.** Removes the restart
  fetch but persists data ADR 0116 keeps in memory, and still fetches
  under the lock when listings expire.
