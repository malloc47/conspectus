# ADR 0068: Fixture Mode For The Snapshot Tool

## Status

Accepted

## Context

ADR 0067 added `conspectus tui --snapshot` against *live* discovery
so agents can iterate on renderer changes without manual
screenshots. That covers "make the operator's current world look
right." It does not cover the iteration-then-regression-test loop:
the agent works against a fixed world, dials in the renderer
behavior, then wants the same world frozen in a unit test so the
fix doesn't regress.

The `GraphSnapshot` type already implements `Serialize` /
`Deserialize`, and `conspectus::output::render_graph_json` already
emits one. The in-tree row-tree tests already build snapshots
programmatically and feed them to `build_sessions_tree`. The
missing piece is a CLI surface that bridges those — capture the
operator's live snapshot to disk, iterate against it, and lift the
same JSON straight into a test.

## Decision

Extend the snapshot tool (ADR 0067) with two file paths:

1. `--snapshot-fixture <PATH>` — read a serialized `GraphSnapshot`
   from a JSON file at `<PATH>` and use it as the input to the
   render pipeline. Live discovery is bypassed entirely; scan
   roots are ignored. The loaded snapshot is run through
   `resolve_snapshot` so resolved relationships are present
   regardless of whether the file on disk already had them.
2. `--snapshot-export-fixture <PATH>` — after the input snapshot
   is produced (live discovery or fixture load) and resolved, but
   before the row tree is built, serialize the snapshot to a JSON
   file at `<PATH>`. Renders proceed normally afterward so the
   agent can iterate on the same invocation that produced the
   fixture.

Both flags are gated by the existing `snapshot` cargo feature and
are mutually compatible: `--snapshot-fixture in.json
--snapshot-export-fixture out.json` round-trips the fixture
through `resolve_snapshot` and writes the result. Either flag
alone is also valid.

### Lift path to in-tree tests

Once a fixture is dialed in:

1. Capture: `conspectus tui --snapshot --snapshot-export-fixture
   tests/fixtures/<name>.json`.
2. Optionally trim the JSON by hand to the minimum nodes and
   links the test cares about. (`render_graph_json` is verbose;
   a focused regression test rarely needs the whole live world.)
3. Write the test:

   ```rust
   let snapshot: GraphSnapshot =
       serde_json::from_str(include_str!("fixtures/<name>.json"))?;
   let tree = build_sessions_tree(SessionsBuildInputs {
       snapshot: &snapshot,
       grouping: SessionsGrouping::Graph,
       ..
   });
   // assertions on `tree.rows`, or render to buffer and assert
   // on the rendered string via the existing test helpers in
   // `src/tui/ui.rs`.
   ```

No new test scaffolding is required — the snapshot tool's fixture
JSON is the same shape `GraphSnapshot::Deserialize` already
accepts, and the row builders / `render_to_buffer` test helpers
already exist.

### Why a graph snapshot, not a richer state capture

A `GraphSnapshot` captures *what was discovered*. It does **not**
capture transient UI state — selection, scroll position, open
overlays, controls cursor, etc. Those are driven by the existing
`--snapshot-keys` prelude. Separating "what world the agent is
looking at" (fixture) from "where the cursor is in that world"
(keys) keeps each layer narrow and replayable.

## Consequences

- **Tight iteration loop.** Agent runs `--snapshot-export-fixture
  foo.json` once to capture a representative world, then re-runs
  `--snapshot-fixture foo.json` (often with different
  `--snapshot-keys` or `--snapshot-pane`) without paying for live
  discovery each iteration.
- **Reproducibility across machines.** A fixture JSON checked
  into the repo is the same input every developer sees, so
  regression tests built on it are deterministic regardless of
  what's on the operator's disk.
- **Same resolve semantics on both paths.** `resolve_snapshot`
  runs on either input, so a fixture missing
  `resolved_relationships` (because it was hand-crafted) still
  renders correctly. The resolver is idempotent for snapshots
  that already carry resolved relationships, so a round-trip
  through `--snapshot-fixture in.json --snapshot-export-fixture
  out.json` produces a stable output.
- **No new model code.** The graph schema, candidate-link
  shapes, and resolver behavior are untouched. The change lives
  in the snapshot module and a small runtime helper that
  populates an `App` from a given snapshot instead of running
  discovery.
- **No UI-state serialization in V1.** Selection, expanded
  rows, and overlay state aren't part of the fixture; if a
  regression test needs them, the key prelude is the place. A
  future ADR can lift UI state to a fixture if the in-tree
  pattern proves insufficient.

## Alternatives Considered

### A. Tie fixtures to the existing `dev_scenarios` system

`src/dev_scenarios.rs` already materializes named worlds (temp
dirs, fake tmux, fake gh) for the debug-only `dev-scenario tui`
command. Rejected because that system is intentionally heavy —
each scenario shells out to git, writes harness fixtures, and
mocks subprocess output. Agents iterating on a renderer change
want a pure JSON read; reusing `dev_scenarios` would couple the
snapshot tool to a build path that exists for replay testing,
not visual iteration.

### B. Capture-and-render in two separate commands

A `conspectus graph dump` subcommand to write a snapshot, and
`conspectus tui --snapshot-fixture` to read it. Rejected for
ergonomics: the operator iterating on a single fix benefits
from a single command line that captures *and* renders, so
they can quickly compare the live frame against the just-
captured fixture.

### C. Include UI state (selection, scroll) in the fixture

Persist `App` state alongside the graph snapshot so a fixture
fully reproduces a screen. Rejected as premature — the key
prelude covers every UI-state delta that a regression test
might need, and serializing the full `App` ties the fixture to
the current `App` shape, which is an internal implementation
detail. Revisit if the prelude proves insufficient.

## Open Questions

- Should the export path emit a trimmed snapshot focused on a
  single visible row or workspace, similar to `git show
  <commit>` vs. the full repo? Out of scope; an operator can
  hand-trim the JSON or run `jq` over the output.
- Should the snapshot tool eventually accept multiple fixture
  files merged into one render? Not today — one snapshot per
  invocation matches the in-tree test pattern.
