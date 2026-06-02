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
