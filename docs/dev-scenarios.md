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
