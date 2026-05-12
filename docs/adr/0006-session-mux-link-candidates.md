# ADR 0006: Session And Mux Link Candidates

## Status

Accepted

## Context

Conspectus links agent sessions and terminal multiplexer sessions, but those
relationships are not inherently one-to-one. A mux session may contain multiple
agent sessions over time. An agent session may have several plausible mux
candidates based on cwd, root path, process or launch evidence, naming
conventions, recency, or user declarations.

Conspectus should still be useful for tools that need a practical default, such
as an agent-session manager or a tool like agent-deck that may want to resume a
session in the best available mux context. At the same time, preserving
candidate mappings lets Conspectus or downstream tools present a mux picker UI
when the mapping is ambiguous.

## Decision

Preserve many mux/session link candidates and let the resolver select a
projection-specific preferred link.

Every plausible `AgentSession -> MuxSession` relationship should enter the
graph as a `GraphLink` candidate. The graph must preserve all non-ignored
candidates, even when a table projection chooses one preferred mux link to show.

Relation kinds or evidence metadata should distinguish why the candidate
exists. Initial categories include:

- explicit declared mux link
- strong process, launch, or provider evidence
- shared cwd or root path
- naming convention match
- recency or activity correlation

Suggested default precedence for resolver-selected links:

1. Non-ignored local declared link
2. Non-ignored global declared link
3. Strong process, launch, or provider evidence
4. Exact cwd or root match between session and mux
5. Naming convention match
6. Recency or activity tie-breaker

Projection behavior:

- Agent projection: render one row per agent session, show the preferred mux
  link, and indicate ambiguity when other candidates exist.
- Mux projection: render one row per mux session and allow zero, one, or many
  linked agent sessions.
- Union projection: preserve both node types and expose relationship status.
- Machine-readable and detailed output: expose all candidate links, evidence,
  provenance, ignored state, and selected/default status.

## Consequences

- Conspectus can provide a useful default mux/session mapping without hiding
  ambiguity.
- Tools like agent-deck can consume the preferred mapping for common workflows
  and still offer a picker when multiple candidates remain.
- User-declared links can override weak discovery without deleting the original
  evidence.
- The resolver must own mux/session scoring and tie-breaking logic.
- Table views need a compact way to show that a preferred mux link was selected
  from multiple candidates.

## Alternatives Considered

- Resolve to one mux link immediately and discard other candidates. Rejected
  because it hides ambiguity and makes incorrect associations hard to diagnose.
- Preserve many raw candidates with no preferred resolution. Rejected because it
  fails to provide the practical default needed by tabular views and downstream
  session-management tools.

## Open Questions Answered

- Session-to-mux and mux-to-session links should be many-candidate
  relationships.
- A projection may select one preferred/default link, but the graph preserves
  all candidate mappings.
- A mux session may contain multiple agent sessions, especially in mux-oriented
  views.
- Preferred mux/session mapping is a resolver decision based on declared links,
  strong evidence, path/root match, convention, and recency.
