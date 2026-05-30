# ADR 0046: Process-Tree Pane Linker

## Status

Accepted

## Context

Conspectus already records tmux active-pane fields and uses them to infer
`AgentSession -> MuxSession` links. The strongest current-session sources are
explicit hook/control-plane evidence and open harness session file descriptors.
Start-command session ids are weaker launch evidence because interactive
harnesses can switch sessions in process after startup.

There is still a gap when a pane starts a shell and the shell launches an agent
without an explicit resume id or open session file that Conspectus can see. In
that case the tmux pane PID identifies the shell, while the actual harness
process lives below it in the process tree. A provider-neutral process-tree
linker can identify that live harness without depending on orchestrator state.

## Decision

Conspectus will implement a Linux-first process-tree linker by reading `/proc`
directly instead of adding a `sysinfo` dependency. The first pass is explicitly
best-effort and has no macOS implementation. Unsupported or unreadable process
state degrades silently.

The linker runs as part of cross-provider inference after tmux and harness
discovery have produced nodes. It consumes each `MuxSession` active-pane PID,
walks the process tree to a bounded depth of four edges, and matches process
commands against the supported harness binary names:

- `claude` and `claude-code` -> `claude-code`
- `codex` -> `codex`
- `opencode` -> `opencode`
- `aider` -> `aider`

The process snapshot boundary is injectable for tests. Production uses a
`LinuxProcSnapshot` implementation that reads numeric `/proc/<pid>` entries,
their parent PIDs, command names, command lines, and current working
directories.

When a matched harness process can be paired with discovered `AgentSession`
nodes by an exact command session key, or by a single harness/cwd match,
Conspectus emits `LinkedToMux` candidates with `match_kind =
"active_pane_process_match"`, `StrongDiscovered` provenance, and high
confidence. If no matching session exists yet, or if same-cwd matching is
ambiguous, Conspectus preserves the evidence as a `MuxSession -> unresolved
AgentSession` candidate carrying the harness key, pane root PID, matched
process PID, command, depth, and cwd. That unresolved edge is diagnostic
evidence only until a concrete session node is discovered in a later run.

The process tree also gates mux cardinality. If Conspectus observes zero or
one non-subagent harness process under the mux active pane, it treats the pane
as controlling at most one human agent session and collapses competing
identity candidates to the freshest non-subagent session. Multiple concrete
session attributions are allowed only when the process tree observes multiple
non-subagent harness processes. openCode subagent worker processes and
sessions are not counted as independent mux occupants.

Resolver ranking treats `active_pane_process_match` as stronger than generic
cwd matching but weaker than explicit current-session evidence from hooks,
control planes, state/file activity, or open fd session matches.

The process-tree linker is disabled when `CONSPECTUS_DISABLE_PROCTREE` is set,
mirroring the tmux and forge provider toggles.

## Consequences

- Plain tmux panes running agent CLIs through an intermediate shell gain
  provider-neutral attachment evidence.
- The implementation avoids a new dependency and keeps platform support honest:
  Linux works through `/proc`; other platforms simply do not contribute this
  evidence yet.
- The linker cannot identify the current session id by itself unless the
  process command carries an exact session key. Otherwise it only proves that a
  harness process is live in a pane and relies on cwd/harness matching or
  stronger sources for exact session attribution.
- Process cwd can be unreadable for permission or lifecycle reasons. Missing
  cwd prevents session pairing but does not fail graph discovery.
- Multiple same-harness sessions in the same cwd remain unresolved or collapse
  to one candidate unless process cardinality shows multiple harness processes
  for the mux.

## Alternatives Considered

- **Depend on `sysinfo`.** Rejected for the first pass. It would add a
  cross-platform dependency before the project has a tested non-Linux
  attribution path, and `/proc` is enough for the current Linux-focused
  workflow.
- **Use tmux pane titles or terminal text.** Rejected as primary evidence
  because titles are convention-heavy and terminal scraping is privacy-hostile
  per ADR 0028.
- **Treat command names as exact session identity.** Rejected. A process-tree
  match proves a live harness in the pane, not which harness-native session is
  active after in-process resume/fork/switch behavior.
- **Fail discovery on unreadable processes.** Rejected because panes, PIDs, and
  permissions are ephemeral. Missing process evidence should not hide other
  graph evidence.

## Open Questions Answered

- The first implementation is Linux-only and dependency-free.
- The descendant walk bound is four edges from the pane PID.
- Process evidence lives in `discovery::cross_link` beside the existing
  active-pane fd and start-command inference.
- Terminal injection remains out of scope for attribution.
