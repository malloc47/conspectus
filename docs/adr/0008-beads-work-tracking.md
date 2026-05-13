# ADR 0008: Beads Work Tracking

## Status

Accepted

## Context

Conspectus is moving from design into phased implementation. The project needs
a work-tracking system that is friendly to coding agents, supports dependency
management, works well with local development, and can turn the design and ADRs
into phases, stories, blockers, and follow-up decision tasks.

The tracker should fit the project's existing design posture:

- data-model-first planning
- graph-based reasoning
- local-first workflows
- agent-readable state
- low ceremony for humans and agents
- enough structure to coordinate longer implementation threads

The user is familiar with Beads and Beans, but wanted the ecosystem surveyed
before choosing a tracker.

## Decision

Use Beads as the intended source of truth for Conspectus implementation work
tracking. The concrete package source is `numtide/llm-agents.nix#beads`,
provided through the repository `nix develop` shell. The installed command is
`bd`.

Beads should be used to track:

- epics for major implementation phases
- tasks for concrete implementation stories
- bugs for regressions found during development
- dependencies for block/unblock sequencing
- follow-up design questions when an unresolved decision materially affects
  implementation scope

GitHub Issues may still be useful later for public-facing project management,
but Beads should be the default local/agent-native tracker for near-term
planning and implementation.

The chosen package currently reports `bd version 1.0.4`. The exact version is
pinned by `flake.lock` and should be updated through normal flake update
workflow rather than global package installation.

## Consequences

- Work breakdown can mirror Conspectus's graph-first design by using task
  dependencies and ready-work queries.
- Agent sessions can use a local, structured source of truth rather than
  relying on ad hoc markdown checklists or conversation memory.
- Work state can live near the code and travel through git workflows.
- The team must be intentional about committing tracker metadata because
  repo-local work tracking changes become part of normal diffs.
- The project gains some structure and ceremony compared with plain markdown,
  but the dependency graph should pay for itself during phased implementation.

## Alternatives Considered

- Beans. Strong option for maximum human readability because it stores issues as
  markdown files in the repo. Deferred because Beads' dependency-aware graph is
  a better fit for Conspectus's phased, graph-first implementation planning.
- Backlog.md. Strong option for markdown-native board-style planning with CLI
  and AI-assistant integrations. Deferred because its board/task orientation is
  less directly aligned with dependency-driven graph planning.
- Peas. Interesting markdown-plus-GraphQL tracker inspired by Beans and Beads.
  Deferred because it appears less mature and has less established workflow
  evidence.
- GitHub Issues. Useful for public-facing tracking, PR linkage, labels, and
  milestones. Deferred as the primary tracker because it is not local-first and
  is less agent-native for offline or multi-agent local workflows.
- Plain markdown checklists. Rejected because they do not provide enough
  structure for dependencies, ready-work selection, or durable agent-readable
  task state.

## Open Questions Answered

- Conspectus implementation work should use a local, agent-native tracker.
- Beads is preferred over Beans for this project because dependency-aware graph
  planning is more important than maximum markdown readability.
- GitHub Issues are optional later and should not be the default source of truth
  for early implementation planning.
- The concrete Beads CLI is `bd`, provided by
  `numtide/llm-agents.nix#beads` through `nix develop`.
