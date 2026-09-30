# Developer Scenarios

Conspectus has a debug-only replay scenario registry for inspecting
edge-case graph/TUI behavior without touching real harness state, real
tmux, `/proc`, or network-backed forge discovery.

These commands are hidden from normal help and are compiled only in
debug/test builds:

```sh
cargo run -- dev scenario list
cargo run -- dev scenario graph exact-match
cargo run -- dev scenario table ambiguous-mux sessions --wide
cargo run -- dev scenario node exact-match codex:session-x
cargo run -- dev scenario tui ambiguous-mux --view sessions
```

Release builds do not expose `conspectus dev ...`.

## Inventory

The source of truth is `SCENARIOS` in `src/dev_scenarios.rs`.
The runtime inventory is:

```sh
cargo run -- dev scenario list
```

Current scenarios:

| Name | Purpose |
| ---- | ------- |
| `empty` | Empty non-repo world. |
| `orphan-session` | One agent session with no mux or repo. |
| `exact-match` | One agent session exactly linked to one tmux session. |
| `ambiguous-mux` | One agent session with two plausible tmux candidates. |
| `hook-supersession` | Same-pane hook records where the freshest session wins. |
| `codex-fd-current` | Codex fd evidence beats a stale launch command. |
| `process-cardinality` | One mux pane with two human-agent runtime processes. |
| `workspace-pr` | Git workspace with a fake GitHub pull request. |
| `fork-lineage` | Atelier fork lineage with unresolved harness lineage. |
| `showcase` | Comprehensive world exercising most Conspectus surfaces (ADR 0070); see [The showcase scenario](#the-showcase-scenario). |

## Launching

Render the resolved graph JSON:

```sh
cargo run -- dev scenario graph ambiguous-mux
```

Render a table row-type:

```sh
cargo run -- dev scenario table workspace-pr prs --wide
cargo run -- dev scenario table fork-lineage forks --layout card
```

Inspect a node. The `<id>` accepts the same forms as `conspectus node
show`: full node id, short table id where resolvable, or unique
harness/mux label.

```sh
cargo run -- dev scenario node exact-match codex:session-x
cargo run -- dev scenario node exact-match tmux:editor
```

Open the real TUI on a static scenario graph:

```sh
cargo run -- dev scenario tui ambiguous-mux --view sessions
cargo run -- dev scenario tui ambiguous-mux --grouping none --mux-state ambiguous --sort recency
cargo run -- dev scenario tui fork-lineage --view forks
```

The scenario TUI uses the normal renderer and reducer, but it does not
run live discovery. Pressing `r` reloads the static scenario snapshot.
The controls overlay, grouping cycle, clear-filters action, search, and
view switching work against the pre-materialized graph. Attach/resume
and mutating actions such as rename writes are disabled so scenario runs
cannot affect live tmux or harness state.

Scenario TUI accepts the pure exploration flags from normal `conspectus
tui`:

- `--view {sessions|mux|union|prs|forks}`
- `--grouping VALUE` (valid values depend on `--view`)
- `--harness HARNESS` (repeatable)
- `--mux-state attached,ambiguous,unmuxed`
- `--max-age DURATION`
- `--sort {hierarchy|recency}`
- `--color {auto|always|never}`

Sort state is available to the TUI controls. Some row-tree builders
still have fixed local ordering, so visible sort differences depend on
the active view.

## Detail Explorer Demo

The right-panel detail view is a focused node inspector and graph
relationship explorer (see "TUI Detail Navigation" in
[`design.md`](design.md) and the layout reference in
[`tui-detail-mockup.md`](tui-detail-mockup.md)). Three named scenarios
are the canonical manual-demo surfaces for it:

```sh
cargo run -- dev scenario tui ambiguous-mux --view sessions
cargo run -- dev scenario tui codex-fd-current --view sessions
cargo run -- dev scenario tui process-cardinality --view sessions
```

In each, select the agent session in the left tree and press `Tab` to
move focus into the right pane. From there:

- `j` / `k` walk the explorer cursor through Node fields, Upstream
  groups, then Downstream groups in render order.
- `Enter` drills into the neighbor on a link row and pushes a
  breadcrumb hop, or expands a multi-link group header.
- `e` is the explicit expand/collapse accelerator for multi-link
  group headers (e.g. the two-mux group on `ambiguous-mux`, or the
  two-process group on `process-cardinality`).
- `Backspace` pops the breadcrumb and restores the prior focused node
  along with its cursor and expansion state. Once the stack is empty,
  the first `Backspace` surfaces a confirmation hint and a second
  consecutive `Backspace` shifts focus from the right pane back to the
  left tree, so Backspace reads as a general "go back" key.
- `o` opens the full untruncated value of the cursor row in a
  centered modal — useful on long `cwd`, `command`, transcript path,
  and observation-key rows.

Each scenario highlights a different aspect of the explorer:

- `ambiguous-mux` — one downstream group with two candidate mux
  links; the resolver's preferred candidate sorts first and carries a
  trailing `★`.
- `codex-fd-current` — a single linked-mux downstream group whose
  preview row exposes the fd evidence that won over the stale launch
  command.
- `process-cardinality` — two upstream runtime-process groups so the
  explorer surfaces `process_identifies_session` vs
  `process_candidates_session` separately.

## Snapshot And Fixture Mode

A dev-only `snapshot` cargo feature exposes a one-shot render path and
fixture I/O on the `tui` subcommand (ADRs 0067, 0068, 0069). The flags do
not appear in builds without `--features snapshot`; `just check` and CI
build with `--all-features`, so they cover it. For manual use, run
`cargo run --features snapshot -- tui …` (the examples below abbreviate
that as `conspectus tui …`).

```sh
# Render one frame to stdout with ANSI styling preserved and exit
conspectus tui --snapshot

# Slice to a single pane (header | left | right | status)
conspectus tui --snapshot --snapshot-pane left --snapshot-width 160 --snapshot-height 40

# Drive the UI to a non-default state before snapshotting (vim-style:
# literals + `<Name>` for non-printables, `<C-x>` / `<A-x>` modifiers).
conspectus tui --snapshot --snapshot-keys '2'           # switch to mux view
conspectus tui --snapshot --snapshot-keys 'jjj<Enter>'  # navigate, expand

# Capture the live world to a fixture JSON for later iteration
conspectus tui --snapshot --snapshot-export-fixture world.json

# Re-render from a fixture (skips live discovery, deterministic)
conspectus tui --snapshot --snapshot-fixture world.json --snapshot-pane left

# Explore a fixture interactively in the full TUI. `r` re-reads the
# JSON from disk so you can edit the fixture in another buffer and
# cycle in the new state without leaving the session.
conspectus tui --fixture world.json
```

Snapshot output uses the same row builders and renderer as the
interactive TUI, so what you see is byte-for-byte what an operator
sees. Lift a dialed-in fixture into a regression test with
`serde_json::from_str(include_str!(...))` plus the existing
`render_to_buffer` / `buffer_to_string` helpers in `src/tui/ui.rs`.

Relative ages ("4m", "2d") are computed against the wall clock, so a
fixture rendered long after it was captured shows stale ages. For
illustrative captures, pin the clock, for example with
`TZ=UTC faketime -f '@2026-06-16 12:03:00' conspectus tui --snapshot …`
(`nix shell nixpkgs#libfaketime` provides `faketime`).

## Test Worlds

A fixture is the *resolved* graph fed into the renderer, so it validates
UI behavior but does not exercise the discovery → resolver pipeline. To
test graph building itself, the test corpus offers three complementary
surfaces:

- **`ReplayWorld` (`tests/support/replay.rs`)** — the day-to-day surface
  for programmatic test worlds. Fluently writes harness session JSON, hook
  sidecar records, fake `tmux list-sessions` rows, and `/proc` fd evidence
  into a temp tree, then runs the real `discover_local_with` +
  `resolve_snapshot` pipeline. Use `world.write_snapshot_fixture("path.json")`
  to drop a normalized JSON the snapshot tool's `--fixture` /
  `--snapshot-fixture` flags can consume — discovery tests and renderer
  tests share one fixture format.
- **Captured-fixture corpus (`tests/fixtures/`, `tests/fixture_corpus.rs`)**
  — sanitized real-provider artifacts (codex transcripts, claude sidecars,
  opencode sessions, tmux output, `/proc` snapshots, hook DBs) run through
  the same adapter/parser paths discovery uses. Reach for this when a
  parser bug shows up on real data and you want a regression test against
  that real shape; the file header documents the sanitization workflow.
- **`dev_scenarios` (`src/dev_scenarios.rs`)** — the named curated worlds
  documented above, reachable interactively through `conspectus dev
  scenario tui <name>` for visual inspection of recurring edge cases.

Together they cover programmatic, real-data, and curated paths. A new
graph-build bug typically starts as a `ReplayWorld` test, gets a captured
artifact under `tests/fixtures/` if a real provider's data triggered it,
and becomes a `dev_scenarios` entry if it's recurring enough to deserve a
name.

## The Showcase Scenario

The `showcase` entry (ADR 0070) is the umbrella world that lights up most
surfaces at once: atelier + agent-deck workspaces, three agent harnesses
with sessions (plus aider state on disk),
codex parent → child fork lineage, a bare repo with a linked worktree, an
ambiguous mux, hook supersession, and two PRs. A corresponding
`tests/fixtures/showcase.json` is checked in so the fixture path works
without a debug build:

```sh
# Interactive (rebuilds the world from scratch; debug builds only)
conspectus dev scenario tui showcase

# Interactive, against the checked-in fixture (any build with
# --features snapshot; press `r` to reload after editing the JSON)
conspectus tui --fixture tests/fixtures/showcase.json

# One-shot ANSI snapshot of a single pane against the fixture
conspectus tui --snapshot \
  --snapshot-fixture tests/fixtures/showcase.json \
  --snapshot-pane left

# Regenerate the checked-in fixture after a showcase change
just regen-showcase-fixture
```

Coverage at a glance:

| Layer | Count / contents |
|---|---|
| Workspaces | 2 (atelier + agent-deck, latter named via `state.db`) |
| Repos | 5 |
| Checkouts | 5 (incl. bare-repo linked worktree) |
| Branches | 7 (`main`, `feature/extra`, `feature/bare`, atelier branches…) |
| Agent sessions | 10 — claude-code × 5, codex × 4, opencode × 1 + aider state on disk |
| Mux sessions | 4 (project, ambiguous, bare-work with fd evidence, agent-deck composite) |
| Forks | 1 (atelier alpha) |
| Forge PRs | 2 (open + draft) |
| Resolved relationships | 37 across 11 distinct relation kinds incl. `parent_session` lineage |

## Adding A Scenario

Add scenarios in `src/dev_scenarios.rs`.

1. Add a `ScenarioDef` entry to `SCENARIOS` with a stable kebab-case
   name, a short description, and a builder function.
2. Implement `fn build_<name>(world: &mut ScenarioWorld) -> Result<()>`.
   Use `world.mkdir`, `world.init_repo`, `world.write_codex_session`,
   `world.write_claude_code_session`, `world.add_tmux_row`,
   `world.write_hook_record`, `world.add_fd_paths`,
   `world.write_atelier_config`, and `world.write_fork_index` instead
   of reading from host state.
3. Keep all state rooted under `ScenarioWorld::root()` and prefer fake
   runners (`FakeTmux`, `FakeGh`) through the existing world fields.
   Scenario builders must not read the user's home directory, real tmux
   server, real `/proc`, or the network.
4. Pick scan roots intentionally. Most scenarios can use the default
   root; repo-specific scenarios should set `world.scan_roots` to the
   generated repo or workspace path.
5. Run:

```sh
cargo test dev_scenarios --all-targets
cargo test --test cli_smoke dev_scenario
cargo test scenario_ --lib
```

If the scenario is intended to guard a TUI interaction, add an `App`
reducer/action test in `src/tui/app.rs` or `src/tui/actions.rs` using
`dev_scenarios::materialize`.

## Design Constraints

- Scenario support is a developer/testing surface, not product
  discovery behavior.
- Do not add new dependencies for scenario convenience without an ADR.
- Keep generated directories temporary and disposable.
- Keep names stable once tests or documentation reference them.
- Prefer structured assertions over large terminal snapshots unless the
  layout itself is the risk being tested.
