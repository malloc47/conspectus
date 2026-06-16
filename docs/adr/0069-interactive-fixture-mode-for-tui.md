# ADR 0069: Interactive Fixture Mode For The TUI

## Status

Accepted

## Context

ADR 0067 added `conspectus tui --snapshot` for one-shot render
validation against live state. ADR 0068 added `--snapshot-fixture
<PATH>` so the snapshot tool can read a serialized `GraphSnapshot`
instead of running discovery. Both surfaces are non-interactive.

The runtime already has a separate non-interactive-discovery
interactive entry point — `runtime::run_static(config, snapshot)`
and its `static_event_loop` — gated on `cfg(any(test,
debug_assertions))`. Today it's reachable through `conspectus
dev-scenario tui --name <X>`, which only accepts the hand-coded
named scenarios in `src/dev_scenarios.rs` (empty, orphan-session,
ambiguous-mux, …). Loading an arbitrary fixture file through that
interactive surface isn't possible without adding a new entry
point.

The natural use case is: capture live state into a fixture via
`--snapshot-export-fixture`, iterate the JSON by hand, then
*navigate* the result interactively to confirm the world looks
right before lifting it into a regression test.

## Decision

Add `--fixture <PATH>` to `conspectus tui`. When set:

1. Live discovery is bypassed entirely. `--scan-root` is ignored.
2. The runtime reads `<PATH>` as a serialized `GraphSnapshot`,
   runs it through `resolve_snapshot` for parity with the live
   discovery path, and hands the result to a fixture-flavored
   variant of `static_event_loop`.
3. The event loop is the same one `dev-scenario tui` uses. No
   background discovery worker; no `r`-triggered re-discovery.
4. The `r` accelerator (`Action::Refresh`) re-reads the JSON
   from disk instead of replaying the in-memory snapshot. The
   operator can edit the fixture file in another buffer, press
   `r` in the TUI, and see the new state without leaving the
   session.
5. Parse / read errors during a manual refresh surface in the
   status bar; the previously loaded fixture stays active so a
   transient typo doesn't blank the screen.

`run_static` and `static_event_loop` are re-gated from
`cfg(any(test, debug_assertions))` to `cfg(any(test,
debug_assertions, feature = "snapshot"))` so the production
release builds (no feature) still don't carry them, but the dev
shell builds (which already enable `snapshot` for the snapshot
tool) make them callable from the new `--fixture` path.

`--fixture` is mutually exclusive with `--snapshot`. The snapshot
tool's `--snapshot-fixture` is for one-shot rendering;
`--fixture` is for interactive exploration. Two flags, two
surfaces, same underlying snapshot format.

## Consequences

- **Closes the explore-then-regress loop.** An agent can dump
  live state to a fixture, edit it down to the case under test,
  navigate that case interactively to confirm it reproduces, then
  lift the same JSON into an in-tree test (per ADR 0068's lift
  path) without re-capturing.
- **Edit-and-`r` workflow.** The operator can keep the TUI open
  while iterating on a fixture file in another editor. `r`
  triggers a disk re-read; errors land in the status bar. No
  process restart needed.
- **No discovery surprises.** Because the static event loop has
  no background worker, the fixture stays put until the operator
  explicitly refreshes. Useful for screen-recording, demos, and
  iteration.
- **dev-scenarios stay separate.** `conspectus dev-scenario tui`
  continues to operate on curated named worlds; `conspectus tui
  --fixture` operates on arbitrary captured worlds. Each surface
  has a distinct intent.
- **Feature gate doubles as the gate for `--snapshot-fixture`.**
  No new cargo feature; both fixture surfaces share the `snapshot`
  feature established by ADR 0067.

## Alternatives Considered

### A. Promote `--fixture` to a new top-level subcommand

`conspectus fixture-tui <PATH>` keeps the flag surface clean on
`tui` but creates a new command name to remember. Rejected
because the underlying experience is just "the TUI, but driven
from a file"; making it a flag on `tui` keeps the discovery
ergonomics consistent with `--snapshot-fixture`.

### B. Reload through `dev-scenarios::ScenarioWorld`

Wrap the loaded fixture in a synthetic `ScenarioWorld` so the
existing `dev-scenario tui` plumbing handles it. Rejected as
overkill — `ScenarioWorld` mocks tmux runners, fake gh, temp dirs.
Fixture playback just needs a snapshot in hand.

### C. Hot-reload on file change instead of manual `r`

Watch the fixture file with `notify` and re-read on every save.
Rejected for V1 — adds a watcher dependency and a runtime
side-effect surface (file watchers can fire mid-edit). Manual `r`
is explicit, deterministic, and matches existing muscle memory.
Revisit if a real demand surfaces.

## Open Questions

- Should manual `r` on a fixture-loaded TUI also accept a new
  fixture path (`Action::SwitchFixture`) so the operator can
  swap fixtures without leaving the session? Out of scope for
  V1; quit-and-relaunch covers the case.
- Should the fixture mode persist a "last loaded fixture" path
  somewhere so the next `conspectus tui --fixture` (no path)
  reopens the previous one? Defer until iteration patterns make
  the demand concrete.
