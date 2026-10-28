# Documentation Index

This index tracks the current documentation set for Conspectus.

## `docs/design.md`

Purpose: working design plan for the standalone Conspectus CLI, including
product goals, the entity relationship draft, discovery, persistence, views,
and migration from atelier-adjacent concepts.

## `docs/capabilities.toml`

Purpose: the capability manifest (ADR 0117). Lists every program and
subcommand, socket, signal, and write path Conspectus may use, by context,
with the ADRs that allow each. The source and trace tests check the code
against it.

## `docs/naming.md`

Purpose: historical naming artifact documenting accepted and rejected project
names, collision notes, the rationale for choosing `conspectus`, and its
short name, `csp`.

## `docs/backlog.md`

Purpose: pointer to the work tracker, which moved to Backlog.md task files
under `backlog/` at the repository root (ADR 0109). Keeps the file's
pre-migration preamble.

## `docs/backlog-legacy-ids.md`

Purpose: maps pre-migration story IDs (`P8-014`, `H-PIN-TUI-011`) to their
Backlog.md task IDs, with each story's first landing time and commit.

## `docs/tui-columns.md`

Purpose: living reference for the TUI's left panel: every view and row
kind column by column, the glyphs and state words, age rules, and
rendered example frames. Tests keep it in step with the code.

## `docs/operations.md`

Purpose: user-facing runtime reference covering the environment variables
that gate discovery providers, the harness state-root overrides, the
config-file precedence, the current CLI surface, and session content search
(`conspectus search`).

## `docs/pins-walkthrough.md`

Purpose: teaching-style walkthrough of history, pin slots, and rebuild
covering the three layers, presence (live / gone / unknown), pinning,
editing overrides, rebuilding a gone mux, planned slots, reaping, and
migrating legacy pins, with a dry-run script. Pairs with the
reference-style §"Pin slots, history, and rebuild" in
`docs/operations.md`.

## `docs/worktrees.md`

Purpose: operator entrypoint answering "what worktree operations does
Conspectus support?" — the model (a worktree is a `Checkout`; mutation
is delegated to worktrunk), setup, an at-a-glance capability table
(list / new / rm / merge / close-down / new-stream / prune / reveal)
across CLI and TUI, the close-down and worktree-backed launch and rebuild concepts,
`[worktree]` config keys, and guardrails. Points to ADR 0092 / 0093 /
0094 for rationale.

## `docs/graph-visualization.md`

Purpose: operator guide for `conspectus graph --format {dot,html}`,
covering the DOT export and the self-contained HTML explorer (filter
panel, inspector, search, focus navigation) with debugging recipes.

## `docs/dev-scenarios.md`

Purpose: developer reference for named replay scenarios, including the
hidden debug-only `conspectus dev scenario ...` commands and how to add new
scenario builders, plus the TUI snapshot/fixture workflow, the three
test-world surfaces (`ReplayWorld`, captured fixtures, `dev_scenarios`),
and the `showcase` scenario's coverage.

## `docs/tui-review.md`

Purpose: UX review of the in-development `conspectus tui` sessions view,
focused on fast session switching, spatial density, color, focus treatment,
and follow-up backlog themes.

## `docs/library-api.md`

Purpose: Phase 6 library API inventory describing stable consumer entry
points, pure modules, impure boundaries, and injection seams.

## `docs/atelier-migration.md`

Purpose: migration guide mapping overlapping Atelier observability commands
to Conspectus CLI and library entry points.

## `docs/adr/`

Purpose: architecture decision records. `docs/adr/README.md` indexes all of
them by theme with their status.

## `crates/tuikit/README.md`

Purpose: entry point to the TUI framework crate's own documentation set:
layers and module map (`crates/tuikit/docs/architecture.md`), independence
rules, house rules, and how to add a control
(`crates/tuikit/docs/contributing.md`), the per-control page template, and the
framework-scoped decision index. Written to stand alone so the crate can leave
this repository; ADRs 0150 to 0152 record the decisions behind it.

## `docs/implementation/`

Purpose: phase-by-phase implementation plan for turning the design into
deliverable milestones, including expected behavior, tests, manual checks, and
assumptions for each phase.

## `docs/plans/`

Purpose: tentative redesign plans captured during operator review or design
brainstorming, before they are promoted to ADRs or phase work. Each file
anchors one or more backlog tasks and records the tradeoffs the
later ADR will need to settle.

The TUI framework extraction (`docs/plans/tui-framework/`) is a directory,
not a file: its `README.md` indexes the inventory, control set, gap analysis,
workspace-impact notes, and the research behind them. ADRs 0150 to 0152 record
its decisions, and milestone m-25 carries the stories.
