# ADR 0009: Lightweight Backlog Tracking

## Status

Accepted

## Context

ADR 0008 selected Beads as the near-term work tracker because its
dependency-aware issue graph aligned with Conspectus's graph-first design.
After initialization, the generated repo state proved too heavy for the
project's current stage: it added tracker metadata, generated hooks, Claude
settings, ignore rules, and a long agent-instruction block before there was a
substantial implementation backlog to manage.

Conspectus still needs a local, agent-readable way to turn the design and ADRs
into implementation phases, stories, blockers, and follow-up decision tasks.
The mechanism should stay reviewable and lightweight until the backlog is large
enough to justify a dedicated tool.

## Decision

Use `docs/backlog.md` as the interim source of truth for Conspectus
implementation work tracking.

The backlog should track:

- epics for major implementation phases
- tasks for concrete implementation stories
- bugs for regressions found during development
- dependencies for block/unblock sequencing
- follow-up design questions when an unresolved decision materially affects
  implementation scope

Introduce the Backlog.md tool as a later phase once there is enough active work
to justify CLI querying, structured metadata, dependency operations, or MCP /
agent integration. The first migration target should be the existing
`docs/backlog.md` content, not a parallel issue database.

GitHub Issues may still be useful later for public-facing project management,
but they should not be the default local planning surface during early
implementation.

## Consequences

- Work breakdown stays local, reviewable, and visible without adding tracker
  generated state before it is needed.
- Agent sessions have a single repo-local planning document rather than relying
  on conversation memory.
- Work state can live near the code and travel through git workflows.
- The project loses structured ready-work queries and formal dependency
  operations for now.
- Backlog.md should be evaluated when the manual backlog becomes difficult to
  maintain or when multiple agents need structured coordination.

## Alternatives Considered

- Beads. Strong fit for dependency-aware graph planning, but rejected for now
  because its initialization footprint is too large for the current stage.
- Backlog.md. Strong option for markdown-native board-style planning with CLI
  and AI-assistant integrations. Deferred as an initial dependency so the repo
  can start with a single lightweight planning document, but selected as the
  preferred next-phase tool.
- Beans. Strong option for maximum human readability because it stores issues as
  markdown files in the repo. Deferred because Backlog.md appears to offer a
  better next step for CLI and agent-oriented workflows while staying
  markdown-native.
- Peas. Interesting markdown-plus-GraphQL tracker inspired by Beans and Beads.
  Deferred because it appears less mature and has less established workflow
  evidence.
- GitHub Issues. Useful for public-facing tracking, PR linkage, labels, and
  milestones. Deferred as the primary tracker because it is not local-first and
  is less agent-native for offline or multi-agent local workflows.
- Plain markdown checklists. Accepted temporarily because they are sufficient
  while the project is still turning design into a phased implementation plan.

## Open Questions Answered

- Conspectus implementation work should use `docs/backlog.md` until structured
  tracking becomes worth its overhead.
- Backlog.md is the preferred next-phase tracker when a tool is introduced.
- Beads is too heavy for the current repo stage despite its useful dependency
  graph semantics.
- GitHub Issues are optional later and should not be the default source of truth
  for early implementation planning.
