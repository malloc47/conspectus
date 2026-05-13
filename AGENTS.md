# Conspectus Agent Instructions

## Project Context

Conspectus is a planned standalone CLI for surveying local AI-agent work across
agent sessions, mux sessions, repos, worktrees, loose workspaces, forks,
branches, and forge PRs.

The project is currently design-first. Before planning or implementing
behavior, read:

- `docs/design.md` for the current north-star product and data-model plan
- `docs/adr/` for accepted Architecture Decision Records
- `README.md` for the project summary

Use `nix develop` for local development. The shell provides Rust tooling,
standard checks, and Beads from `numtide/llm-agents.nix`. The Beads CLI is
available as `bd`.

## Design Guardrails

- Design data-model-first. New features and implementation phases should flow
  from the graph model, not from incidental provider or UI details.
- Keep the core graph provider-neutral. Provider-specific reality belongs in
  adapters, source metadata, evidence, and carefully chosen attributes.
- Treat the graph as sparse by default. Missing links and disconnected cliques
  are normal, not exceptional.
- Use `GraphLink` candidates as the evidence layer and derive typed resolved
  relationships through a resolver.
- Use one polymorphic `Fork` node for fork-like provenance; express effects via
  relation kinds and source metadata.
- Prioritize link inference from on-disk state, provider metadata, and common
  conventions. Federated user overrides are available when needed and take
  precedence over weaker inferred links.
- Keep rebuildable caches outside project trees. Persist user-authored link
  intent near the relevant repo or workspace when the relationship is
  project-rooted.

## Decision Records

- Significant decisions resolved while building features or drafting plans must
  be memorialized as new ADRs under `docs/adr/`.
- Use the existing ADR style: status, context, decision, consequences,
  alternatives considered, and open questions answered when applicable.
- Update `docs/design.md` whenever requirements are introduced or refined, or
  when accepted ADRs change the intended model.
- Keep `docs/design.md` concise enough to remain useful for phase and story
  planning. Put detailed rationale in ADRs.

## Implementation Notes

- Start read-only unless a task explicitly calls for persistence or link CRUD.
- Avoid making Conspectus depend on Atelier command modules directly. Shared
  code should be pure discovery/parsing/model code with a clean boundary.
- Track implementation work with Beads (`bd`) once tracker state is initialized.
- Preserve user changes and avoid rewriting unrelated files.
- For docs-only changes, run `git diff --check`. For code changes, add or run
  the most relevant checks once the project has executable code.
