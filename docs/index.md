# Documentation Index

This index tracks the current documentation set for Conspectus.

## `docs/design.md`

Purpose: working design plan for the standalone Conspectus CLI, including
product goals, the entity relationship draft, discovery, persistence, views,
and migration from atelier-adjacent concepts.

## `docs/naming.md`

Purpose: historical naming artifact documenting accepted and rejected project
names, collision notes, and the rationale for choosing `conspectus`.

## `docs/backlog.md`

Purpose: interim work tracker for turning the design and ADRs into phases,
stories, blockers, and follow-up implementation tasks.

## `docs/feature-summary.md`

Purpose: current feature inventory for the CLI, discovery providers,
declared-link flows, known limits, and planned next areas.

## `docs/operations.md`

Purpose: user-facing runtime reference covering the environment variables
that gate discovery providers, the harness state-root overrides, the
config-file precedence, and the current CLI surface.

## `docs/pins-walkthrough.md`

Purpose: teaching-style walkthrough for session pins covering the "why"
behind pinning, the four binding states, the TUI controls for each pin
operation (create, launch, rename, remove, bind, rebind, adopt), the
mux-death continuity flow, and a dry-run script. Pairs with the
reference-style §"Session pins" in `docs/operations.md`.

## `docs/worktrees.md`

Purpose: operator entrypoint answering "what worktree operations does
Conspectus support?" — the model (a worktree is a `Checkout`; mutation
is delegated to worktrunk), setup, an at-a-glance capability table
(list / new / rm / merge / close-down / new-stream / prune / reveal)
across CLI and TUI, the close-down and worktree-backed-pin concepts,
`[worktree]` config keys, and guardrails. Points to ADR 0092 / 0093 /
0094 for rationale.

## `docs/graph-visualization.md`

Purpose: operator guide for `conspectus graph --format {dot,html}`,
covering the DOT export and the self-contained HTML explorer (filter
panel, inspector, search, focus navigation) with debugging recipes.

## `docs/dev-scenarios.md`

Purpose: developer reference for named replay scenarios, including the
hidden debug-only `conspectus dev scenario ...` commands and how to add new
scenario builders.

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

Purpose: accepted architecture decisions, including the Phase 6 library API
surface and distribution policy.

## `docs/implementation/`

Purpose: phase-by-phase implementation plan for turning the design into
deliverable milestones, including expected behavior, tests, manual checks, and
assumptions for each phase.

## `docs/plans/`

Purpose: tentative redesign plans captured during operator review or design
brainstorming, before they are promoted to ADRs or phase work. Each file
anchors one or more `H-*` backlog entries and records the tradeoffs the
later ADR will need to settle.
