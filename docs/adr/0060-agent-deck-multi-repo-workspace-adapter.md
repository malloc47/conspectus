# ADR 0060: Agent-Deck Multi-Repo Workspace Adapter

## Status

Accepted

## Context

`H-AGENTMUX-001` framed a paper audit of agent-mux orchestrators
(agent-deck, dmux, workmux, agent-of-empires) against the MUXPROC
process-tree linker to determine where each tool produces evidence
the linker cannot reproduce. While starting work on agent-deck
specifically, the audit's question collapsed to a single answer:
**workspace composition** — the directory shape that mounts N
git repos under a single working tree via symlinks. The process
tree can see only the cwd; it cannot reveal that the cwd is a
composite of separate checkouts.

This ADR records the decisions made while building the agent-deck
adapter and the surrounding multi-repo surfacing, including the
deferred posture of the other orchestrators per CLAUDE.md's
data-model-first / decisions-as-ADRs rule.

Adjacent work that landed concurrently:

- The agent-table workspace column now renders `repo-a+repo-b+...`
  when the resolver picks ≥2 distinct `WorkspaceContainsRepo`
  members for the underlying workspace (`H-AGENTMUX-003`). This
  surface is shared by atelier (already a multi-repo provider) and
  by the agent-deck adapter introduced here. The default
  `SESSIONS_COLUMNS` set is unchanged; the column is opt-in via
  `--columns ...,workspace,...`.

## Decision

### Adapter shape

A new provider `AgentDeckDiscovery` lives at
`src/discovery/agent_deck.rs`. It is constructed with a fixed
location — typically `~/.agent-deck/multi-repo-worktrees/` — and
on each discovery cycle:

1. Enumerates immediate child directories under that root. Each
   child is a candidate workspace named `<id>`.
2. For each `<id>` directory, enumerates immediate children that
   are **symlinks** (regular directories are ignored — they are
   provider-foreign and may be a developer's transient files
   rather than agent-deck-authored composition).
3. Probes each symlink target with `GitProbe`. Members that do
   not probe as git repos (broken symlinks, non-git targets) are
   silently dropped.
4. If ≥2 distinct resolved members survive, emits one
   `WorkspaceNode { provider: Some("agent-deck"), name: Some(<id>) }`
   plus one `WorkspaceContainsRepo` candidate link per member.
   `<id>` directories with 0 or 1 surviving members emit nothing
   — single-repo workspaces have no composition to surface.

The `Repo` node identity for each member uses the symlink
target's canonical git common dir, so an existing `Repo` node
from an ordinary scan (e.g. `~/src/atelier`) merges with the
agent-deck membership in `merge_fragments`.

Membership links use:

- `Provenance::StrongDiscovered` — agent-deck's manifest is
  explicit composition, not weak filesystem heuristics.
- `Confidence::High`.
- `source_metadata.adapter = "agent_deck"`.
- `source_metadata.evidence = "agent-deck multi-repo-worktrees symlink"`.
- `source_metadata.fields["logical_path"]`,
  `["canonical_checkout_root"]`,
  `["member_path_kind"] = "symlink"` — the same fields the
  generic-workspace adapter populates, so the downstream column
  formatter (`output::agent::fetch_workspace_member_displays`)
  works uniformly across providers.

### Activation: probe-by-default with env opt-out

`LocalDiscoveryConfig` gains a new field `agent_deck_root:
Option<PathBuf>` mirroring the existing `hook_sidecar_root`
pattern:

- `LocalDiscoveryConfig::from_env()` populates it from
  `$HOME/.agent-deck/multi-repo-worktrees` by default.
- `CONSPECTUS_DISABLE_AGENT_DECK` disables it entirely.
- `CONSPECTUS_AGENT_DECK_ROOT` overrides the path.
- `LocalDiscoveryConfig::empty()` leaves it `None` (test default).
- Builder methods `with_agent_deck_root` and `without_agent_deck`
  mirror `with_hook_sidecar_root` / `without_hook_sidecar`.

The adapter only runs when `agent_deck_root` is `Some`; a
missing root directory is silently no-op (a typical operator
without agent-deck installed pays only one `is_dir()` syscall per
discovery cycle).

### Workspace-provider precedence

ADR 0027 already orders provider-specific workspace metadata
above generic inference at the same canonical root. Agent-deck
workspaces sit under `~/.agent-deck/multi-repo-worktrees/<id>/`,
a fixed location the operator does not normally pass to
discovery as a scan root, so the generic-workspace inference
guard (`provider_workspace_claims_root`) does not need extension.
If the operator explicitly scans inside the agent-deck tree, the
agent-deck adapter still emits authoritative provider metadata
and the generic provider will see the path is already a workspace
via the merged fragment. No code change to `workspace.rs` was
required for v1.

### Sessions associate via existing cross-link inference

The adapter emits the workspace and its membership only.
Session ↔ workspace `AssociatedWith` candidates are derived by
the existing `discovery::cross_link::infer` pass based on cwd
prefix matching, which already handles atelier workspaces. No
new cross-link logic was needed.

### Deferred orchestrators

The other tools in the original `H-AGENTMUX-001` scope are
deferred without adapters in this ADR:

- **dmux** (`H-AGENTMUX-005`): runtime mapping is largely a
  MUXPROC subset. No persistent multi-repo composition observed.
  Keep as placeholder; revisit if dmux gains workspace-shaped
  state.
- **workmux** (`H-AGENTMUX-006`): per-worktree `.workmux/` and
  `~/.local/state/workmux/agents/` *might* carry resurrect-state
  for exited sessions — the only non-MUXPROC angle. Audit
  remains open behind that specific question; a future ADR
  reopens this if the resurrect-state evidence is genuine.
- **agent-of-empires** (`H-AGENTMUX-007`): container isolation
  is the only theoretical edge; no evidence yet that host-visible
  state captures the in-container agent identity. Deferred.
- **agent-deck `state.db`** (`H-AGENTMUX-004`): the SQLite
  state.db plausibly carries lifecycle metadata for exited /
  paused sessions, but that's read-only enrichment, not
  composition evidence. Deferred behind a follow-up ADR if the
  audit confirms non-overlap.

## Consequences

- Operators with agent-deck installed get multi-repo workspace
  rows immediately on the next `conspectus` invocation, no
  configuration required.
- Operators without agent-deck pay one `is_dir()` check per
  discovery cycle for the missing path.
- The agent-table `workspace` column (opt-in) displays
  `atelier+conspectus`-style joined member names for agent-deck
  workspaces *and* for atelier multi-repo workspaces, exercising
  the shared `output::agent::fetch_workspace_member_displays`
  formatter from `H-AGENTMUX-003`.
- The `AgentMuxAdapter` trait floated in `H-AGENTMUX-001` is
  **not** introduced. With agent-deck collapsing to "emit a
  Workspace + WorkspaceContainsRepo links" — which already maps
  onto `DiscoveryProvider` cleanly — a separate trait would be
  speculative scaffolding. If a future adapter (workmux
  resurrect-state, agent-of-empires container probe) needs
  surface area `DiscoveryProvider` cannot express, that's the
  trigger to introduce the trait.
- `H-AGENTMUX-002` and `H-AGENTMUX-003` close as the
  implementation of this ADR.
- `H-AGENTMUX-001` closes by being subsumed into this ADR
  (the audit's conclusion is recorded here rather than as a
  separate document).

## Alternatives Considered

- **Scan-root-triggered discovery (only when the operator
  explicitly scans into the agent-deck tree).** Rejected
  because typical operators scan `~/src/`, not
  `~/.agent-deck/`. Probe-by-default surfaces agent-deck
  workspaces in the dashboard without configuration; the cost
  is one no-op `is_dir()` per cycle.
- **TOML opt-out under `[agent_deck]` in `.conspectus.toml`.**
  Rejected for v1 because the existing opt-out vocabulary is
  env-based (`CONSPECTUS_DISABLE_TMUX`, `_FORGE`, `_PROCTREE`,
  `_CODEX_LOG`). Adding a one-off TOML toggle for agent-deck
  would diverge from the established pattern. Revisit if a
  broader provider-toggle config table is introduced.
- **Accept directory children, not just symlinks.** Rejected
  because agent-deck's contract is symlink-composition; a real
  subdirectory at that depth is operator scratch, not
  agent-deck-authored membership. Including it would surface
  surprising rows.
- **Introduce a dedicated `AgentMuxAdapter` trait.** Deferred
  as above — premature without a second adapter that doesn't
  fit `DiscoveryProvider`.
- **Promote `workspace` into the default `SESSIONS_COLUMNS`
  set so agent-deck users see the surface without flags.**
  Deferred as a separate UX call. The column is width-heavy and
  the default set was tuned in ADR 0020 / H-TBL-001+. Worth
  revisiting once the joined-name rendering is in use, but not
  bundled with this ADR.

## Open Questions Answered

- **Should the `<id>` workspace name be sanitized?** No — it is
  used verbatim as `WorkspaceNode.name` and as the joined-name
  basename source (via the symlink leaf names, not the `<id>`).
  Operators who name their agent-deck workspaces with odd
  characters get those characters in display surfaces; that's
  consistent with how every other workspace name is treated.
- **Should non-default tmux sockets matter here?** No — agent-deck
  workspace composition is independent of tmux backing. Pin /
  tmux socket handling stays in `H-PIN-F-001` / `-005`.
- **Should the adapter recurse?** No. Membership is one level
  deep by agent-deck's convention; recursing would invent
  composition the operator did not author.
- **Should agent-deck workspaces emit pane ↔ harness evidence?**
  No — MUXPROC owns that. The adapter is composition-only per
  the `H-AGENTMUX-001` audit rule.
- **How does the agent table show multi-repo participants
  without explicit columns?** Via the joined-name rendering in
  the workspace column (`H-AGENTMUX-003`), opt-in for v1. The
  default-column promotion question is intentionally separate.
