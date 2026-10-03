---
id: m-18
title: "Operator Requests 2026-07-27"
---

## Description

A batch of operator-requested items, listed easiest → hardest where
"harder" means more product direction is needed to scope and set the
approach (not raw implementation size). Worked top-to-bottom.

- **CSP-501** Drop the Union view and hide the PRs and Forks views from the TUI
- **CSP-502** Make the column-reflow (narrow → stacked) threshold configurable
- **CSP-503** Surface pins registered in repos outside the active search root
- **CSP-504** Add better recency sort options for the mux view
- **CSP-505** Debug `conspectus serve` resource usage
- **CSP-510** Cache `codex_log` DB scan across cycles
- **CSP-511** Fingerprint-gated `/proc` walk (001a follow-up)
- **CSP-512** Cache `GitProbe::probe` results across cycles keyed on `.git/HEAD` / `config` / `refs/heads/` / `packed-refs` mtimes…
- **CSP-513** TTL-cache `ForgeDiscovery` output (`9a691a3`)
- **CSP-514** Fix `codex::read_session_meta` full-file slurp (`3495367`)
- **CSP-515** Per-file `(mtime, size)` cache for claude + codex session header/tail scans (`9b9eec3`)
- **CSP-516** Refine CSP-511's fingerprint to zero mux `activity_epoch`, `last_attached_epoch`…
- **CSP-517** Extend 008 to also zero `last_message_preview`, `title`, `created_epoch` (`aac498c`)
- **CSP-518** mtime-cache the opencode.db SQL scan (`148763d`)
- **CSP-519** TTL-cache `TmuxDiscovery` + `ZellijDiscovery` output (`03ad86b`)
- Full retrospective for the H-SERVE-PERF chain (001a through 011),
  including per-commit attribution table and remaining deferred
  work (001c resolve/publish deferral, `try_class_cycle` gate
  bypass, empty-fragment gate hole), lives at the bottom of
  `docs/adr/0091-serve-idle-cost-and-class-gated-mutators.md`.
- **CSP-506** Integrate first-class worktree management with pluggable backends

### Operator Requests 2026-08-07

Fresh batch, added alongside the 2026-07-27 items.

- **CSP-525** Pin edit / delete does not resolve a pin when a pinned mux row is selected in the mux view
- **CSP-526** Atelier `exec claude` panes render as "No agent" in the TUI / CLI
- **CSP-580** Preview pane hides quiet panes' output and wraps agent UI decorations (ADR 0106)
- **CSP-583** Returning from tmux redraws the TUI without waiting for the refresh (ADR 0108)
- **CSP-584** Cheaper post-hand-off rescans
- **CSP-527** Create bare tmux sessions from within Conspectus (no pin, no agent)
- **CSP-529** Launch a harness in a fresh mux without persisting a pin (ADR 0096)
- **CSP-530** Extract the shared launch-spec form primitive (ADR 0097)
- **CSP-531** Pin resume drops the pin's launch argv (ADR 0098)

### Worktree Interaction Epic (CSP-520 / CSP-509.02, CSP-521, CSP-522, CSP-523, CSP-524)

Brainstormed 2026-08 across contexts (agent/mux/repo/checkout). Mental
model: a worktree is a *stream of work* — checkout-on-a-branch +
optional mux + optional agent, linked in the graph. Settled action
catalog and decisions:

- Representation (settled): **hybrid** — a context-sensitive worktree
  action menu opened with `w` on the selected node (primary,
  discoverable, gated on mutation-backend availability), plus two hot
  keys: `N` (new stream) and `X` (close down). **Full CLI parity**:
  every mutation mirrored as a `conspectus worktree <verb>`.
- Per-context surfacing: repo → create / new-stream / prune; checkout
  → merge / remove / close-down / lock / new-sibling / launch-here;
  agent → reveal / close-down / merge; mux → attach / close-down /
  merge / remove / new-parallel.
- "Close down a stream" (settled: **full teardown**) = optional merge
  → **kill the mux (+ its agent pane)** → remove worktree + delete
  branch → drop the now-stale pin. Two flavors: merge-&-close and
  discard-&-close. Nuance: Conspectus never types at the agent
  (ADR 0028) and never deletes harness-native session records
  (ADR 0087 prohibition 1) — teardown ends the *running process*, the
  transcript/history stays.
- "New stream" (settled: **both** entry points) = create worktree +
  launch, via a pin-create-form "create worktree for branch" toggle
  AND a standalone repo/checkout "new worktree + launch" action.

Sequencing (settled: **ADR first, then in order**):

- **CSP-520** ADR: sanction `tmux kill-session` as an operator-initiated teardown mutation (ADR 0087 category-3 extension) +…
- **CSP-509.02** TUI worktree **action menu** (`w`) wired to the already-built safe actions (create, remove)…
- **CSP-521** `merge` — `WorktrunkBackend::merge` (`wt -C <wt> merge [target]`) + `WorktreeCaps.can_merge`…
- **CSP-522** `close-down` compound orchestrator (ADR 0093)
- **CSP-523** new-stream: worktree-backed pins realized at launch (ADR 0094)
- **CSP-524** prune + reveal/navigate
