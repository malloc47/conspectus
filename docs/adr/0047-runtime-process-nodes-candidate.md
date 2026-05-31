# ADR 0047: Runtime Process Nodes

## Status

Accepted

## Context

ADR 0046 added process-tree evidence for mux attribution without introducing a
first-class graph entity for runtime processes. Process data currently lives in
`GraphLink` source metadata on `LinkedToMux` candidates or unresolved
endpoints. That keeps the graph small and avoids modeling ephemeral operating
system state, but it also means important process invariants are enforced
inside cross-link inference rather than represented directly in the graph.

The most important invariant is mux cardinality: if a mux active pane has zero
or one non-subagent agent harness process below it, Conspectus should attribute
at most one human agent session to that mux. Multiple session attributions are
reasonable only when multiple non-subagent harness processes are observed. The
current implementation enforces this with process evidence, but future
features may need a richer explanation surface for why a mux contains one,
zero, or multiple agent sessions.

Process modeling may also matter for openCode subagent filtering, harness
server/proxy modes, control-plane discovery, multi-pane mux sessions, and
diagnostics for stale launch arguments versus current session evidence.

## Decision

Conspectus will add an explicit runtime process layer to the graph before
shipping graph visualization exports. Process evidence has accumulated enough
resolver, mux-cardinality, hook/plugin, subagent, and diagnostic
responsibilities that keeping it only in `GraphLink.source_metadata` would make
the visualization surface misleading: the graph would show agent and mux nodes
without the operational process facts that explain why the resolver accepted or
rejected a mux/session attribution.

The accepted model shape is:

- `RuntimeProcess` node: ephemeral observation of a process under a mux pane.
- `MuxSession -> RuntimeProcess`: the mux contains or owns the observed pane
  process tree member.
- `RuntimeProcess -> AgentSession`: the process is exact, candidate, or
  unresolved evidence for a harness-native session.
- `AgentSession -> MuxSession`: remains the derived user-facing relationship
  selected by the resolver.

`RuntimeProcess` should be provider-neutral and sparse. Candidate attributes
include PID, parent PID, root pane PID, command, cwd, harness key, depth from
the pane root, observed epoch, and a process role such as `human_agent`,
`subagent`, `shell`, or `unknown`. Process nodes should be treated as
observations, not durable identity. They should not imply that PIDs are stable
across runs.

Runtime process nodes should be hidden from default table/TUI views unless the
operator asks for diagnostic detail. Machine-readable graph output and graph
visualization exports should preserve them because they explain resolver
behavior and process-cardinality gates.

## Consequences

- Process cardinality would become a graph fact rather than a hidden
  cross-linking rule.
- Mux attribution diagnostics could show the observed process tree and explain
  why Conspectus did or did not allow multiple session attachments.
- Resolver rules could use explicit process relationships instead of parsing
  process metadata from `LinkedToMux` candidates.
- The graph would become noisier and more operational unless process nodes are
  hidden from default views or constrained to diagnostic surfaces.
- Query persistence, JSON snapshots, fixtures, and TUI/detail views would need
  updates for a new node type and relation set.
- Process node identity is inherently ephemeral. Any implementation must pick
  stable-enough observation ids without pretending that PID identity survives
  process restarts.
- Cross-platform support would remain uneven. Linux `/proc` can populate the
  first implementation; macOS and other platforms may contribute no process
  nodes until a separate provider exists.
- Graph visualization ADRs and implementations should treat runtime process
  nodes as part of the resolved graph shape. DOT and HTML renderers can choose
  subdued styling or default-hidden diagnostic layers, but they should not
  design around a process-free graph.

## Alternatives Considered

- **Keep process evidence only in `GraphLink` metadata.** This is the current
  implementation. It is cheaper, keeps snapshots smaller, and is sufficient
  while process evidence only gates mux attribution.
- **Promote only harness processes, not all processes.** This would reduce
  graph noise and focus on cardinality, but loses shell/intermediate-process
  diagnostics that explain how a pane reached a harness process.
- **Persist process observations outside the graph.** This could keep the
  graph clean, but would make machine-readable graph output incomplete and
  split resolver evidence across two surfaces.
- **Model panes instead of processes first.** Pane nodes could improve
  multi-pane mux semantics, but the cardinality invariant is specifically
  about harness child processes. Pane modeling can be evaluated separately.

## Open Questions Answered

- Process nodes are accepted for implementation as a MUXPROC follow-up before
  graph visualization work begins.
- `AgentSession -> MuxSession` remains the main user-facing relationship.
  Runtime process nodes explain and support that derived relationship; they do
  not replace it.
- Runtime process observations are ephemeral and rebuildable. They may appear in
  JSON, SQLite, node detail, and visualization surfaces, but they are not
  durable identity and should not be persisted as user-authored intent.
- The first implementation should model harness-relevant process observations
  and enough ancestry to explain active-pane attribution. It does not need to
  model every host process.
