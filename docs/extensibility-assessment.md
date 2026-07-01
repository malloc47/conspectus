# Provider Extensibility Assessment

Status: draft assessment, 2026-07-01. Companion to `docs/design.md` and the
`H-REF-*` / `H-FUTURE-*` / `H-AGENTMUX-*` backlog sections. This document
inventories how hardcoded each provider entity family is today, measures the
gap between "implement one interface" and "the new provider flows through
CLI, TUI, and graph surfaces," and proposes a chunked plan to close the gap.

Entity families assessed, with the forthcoming extensions that motivated the
audit:

- **Mux backends** — tmux today; zellij next (`H-FUTURE-001`).
- **Orchestrators** — agent-deck today; dmux, herdr, pertmux, workmux next.
- **Agent harnesses** — claude-code, codex, opencode, aider today.
- **Forges** — GitHub today; GitLab/Gitea next (`H-FUTURE-002`).

## Executive Summary

The data plane is in good shape: the graph model is provider-neutral, and the
big consumers (`output/table.rs`, `tui/rows/sessions.rs`, resolver scoring,
graph exports, filters) dispatch on node *fields* (`harness_key`, `backend`,
`provider`) rather than on provider literals. Production code in those files
contains essentially zero hardcoded provider names. That is the architecture
working as designed (ADR 0001/0002/0050) and it should be defended.

The control plane is where hardcoding lives. Each entity family has (or
almost has) a discovery trait, but the *experience* layers — launch, resume,
attach, rename, hooks, transcript viewing, process attribution, theming,
filter option lists, and config/env wiring — bypass the traits with parallel
`match harness_key { ... }` tables, single-slot config fields, and per-provider
struct fields. Implementing today's interface gets a new provider **into the
graph**; it does not get it launched, attached, themed, filtered, viewed, or
hook-attributed.

Gap ranking, smallest to largest:

| Entity | Discovery seam | Experience gap | Verdict |
| --- | --- | --- | --- |
| Forge | `ForgeAdapter` trait ✅ | small: config slot + wiring are gh-shaped | Near-clean; ~2 chunks |
| Orchestrator | `DiscoveryProvider` only (per ADR 0060) | small-medium: bespoke config fields/env per adapter; no shared capability surface | Needs a registry + revisit of ADR 0060 |
| Harness | `HarnessAdapter` trait ✅ | **large**: ~10 side tables across launch/resume/hooks/viewer/cross-link/theme/filter | Biggest count of touch points |
| Mux | `TmuxRunner` trait (tmux-named, tmux-shaped) | **large**: pins, attach, rename, hooks, CLI all assume tmux; "only tmux" gates in UI | Deepest single coupling |

## What Already Works (Preserve This)

These properties are why the fix is tractable, and every chunk below must
keep them true:

- **Nodes carry provider identity as data.** `AgentSessionNode.harness_key`,
  `MuxSessionNode.backend`, `WorkspaceNode.provider`, `ForgePrNode.provider`
  + `host`. Renderers and the resolver read these fields; they do not match
  on known values. `harness_badge` (`src/tui/widgets/badge.rs:37`) renders any
  label and falls back to `harness_unknown` color for unrecognized harnesses.
- **`GraphLink` evidence + resolver** are provenance-driven
  (`Provenance`-tier scoring), not provider-driven, with one soft exception
  noted below.
- **Provider ids are centralized** in `discovery/providers.rs` and every
  cache/eviction/scheduler concern keys off the same strings; the module doc
  explicitly documents the add-a-provider ritual.
- **Per-provider failure isolation and TTL classes** (`cache::provider_class`,
  daemon per-class scheduler) generalize to new providers as long as each new
  key gets a class mapping.
- **The runner seam pattern** (`SystemX` + `FakeX` behind a trait) exists for
  tmux and gh, so offline testing of new external-tool backends has a
  template (`H-REF-004` wants it deduplicated).
- **Server watch paths derive from config** (`config.harness_state_roots`,
  `src/server/mod.rs:1152`), not from hardcoded harness paths.

## Per-Entity Assessment

### 1. Agent Harnesses

**Intended seam:** `HarnessAdapter` (`src/discovery/harness/mod.rs:59`) —
`harness_key()`, `discover()`, `launch_argv()`, `resume_argv()`. The
`HarnessDiscovery` coordinator merges adapter fragments. Discovery itself is
genuinely pluggable.

**The gap.** A new harness ("implement `HarnessAdapter`") today additionally
requires touching all of the following:

*Registration / wiring (mechanical, expected):*

1. `discovery/providers.rs` — provider const + `cache::provider_class` arm.
2. `discovery/mod.rs:342` — the keyed-provider list literal
   `&["claude-code", "codex", "opencode", "aider"]` and
   `HarnessDiscovery::with_default_adapters()`.
3. `discovery/mod.rs:450-461` (`LocalDiscoveryConfig::from_env`) — per-harness
   `CONSPECTUS_<X>_STATE` env plumbing and default state root.

*Parallel dispatch tables that bypass the trait (the real problem):*

4. `discovery/harness/mod.rs:93-116, 160-187` — `launch_argv_for`,
   `launch_options_for`, `resume_argv_for`,
   `strip_known_launch_option_fragments` are hardcoded `match harness_key`
   tables and per-harness `const` argv fragments, even though `launch_argv` /
   `resume_argv` are already trait methods. The trait is the interface;
   these free functions are the ones callers actually use.
5. `discovery/cross_link.rs` — the largest leak. Per-harness runtime
   knowledge is inlined: process-command → harness mapping
   (`process_command_harnesses` :1458, `command_harnesses` :1725), fd-path →
   harness patterns (`/.codex/sessions/` etc., :1590), per-harness session-key
   grammars (`session_keys_for_harness_text` :1668, `ses_*` for opencode),
   and role heuristics (`is_opencode_subagent_process` :1250,
   `is_claude_background_process` :1254). A new harness gets zero mux
   attribution until all of these learn about it.
6. `src/hook.rs:150-240` + `src/cli.rs:662` — per-harness hook payload
   normalizers (`claude_code_record_from_payload`, `codex_…`, `opencode_…`)
   and a clap enum (`HookWriteHarness`) with one variant per harness, plus
   per-harness plugin distribution (`plugins/opencode-hook`).
7. `src/viewer/model.rs:36` — `SessionLocator` is a closed enum with one
   variant per harness; `viewer_bridge::locator_for_session`
   (`src/tui/viewer_bridge.rs:24`) matches harness keys to build it, and
   `read_with_appropriate_parser` dispatches to the three `HarnessParser`
   impls. Aider is simply absent. A new harness's transcripts are invisible
   until the enum, the bridge match, and the parser registry all change.
8. `src/tui/resume.rs:42` — resume-target match per harness (partially
   redundant with `resume_argv` on the trait).
9. `src/tui/theme.rs:29-32, 234` — `harness_claude` / `harness_codex` /
   `harness_opencode` / `harness_aider` are individual struct fields with a
   `harness_color(label)` match, plus `[tui.theme]` key parsing in
   `config.rs`. New harness → new field, new config key, new match arm.
   (Note the match keys are display labels — `"claude"` — not harness keys.)
10. `src/tui/widgets/controls.rs:47` — `HARNESS_OPTIONS` hardcodes the filter
    overlay's harness list; a new harness is undiscoverable in the filter UI.
11. `src/tui/rows/mod.rs:431` — `harness_label` special-cases
    `claude-code → claude`.
12. Optional per-harness auxiliary readers are wired as bespoke top-level
    concerns: `codex_log` is its own mutator pass with dedicated
    `LocalDiscoveryConfig` fields (`codex_log_disabled`,
    `codex_log_window_seconds`) and env vars, referenced by name in
    `apply_mutators` (`discovery/mod.rs:399`).

**Count: ~12 locations, 4 of which are genuine feature work (cross-link
signatures, hooks, viewer parser, theme) rather than mechanical registration.**

### 2. Mux Backends

**Intended seam:** `TmuxRunner` (`src/discovery/tmux/mod.rs:32`) — but the
trait is tmux by name and by shape: `list_sessions(format)` takes a tmux
format string, `socket_name` models tmux `-L` server isolation, targets are
`session:window.pane` selectors, and outcomes are `Tmux*Outcome` enums. The
doc comments already anticipate zellij ("per `H-FUTURE-001`") via
`Unsupported` defaults, but a zellij impl of *this* trait would be an
impersonation, not a backend.

**Where tmux is assumed beyond discovery:**

- **Config/wiring:** `LocalDiscoveryConfig.tmux_runner` is a single
  `Option<Box<dyn TmuxRunner>>` slot (`discovery/mod.rs:423`);
  `discover_local_warm_with` wires exactly one `TmuxDiscovery`. There is no
  concept of multiple concurrent mux backends.
- **Attach:** `tui/actions.rs:190` hard-gates
  `AttachDisabled::UnsupportedBackend` with the operator-facing message
  "only tmux" (:442); the CLI builds `tmux [-L sock] attach-session -t`
  argv strings and exec-replaces (`cli.rs`, `format_attach_command`).
- **Pins (ADR 0057/0058):** `PinMux.backend` is validated to literal
  `"tmux"` (`src/pins.rs:260, 326`); `socket_name` is tmux-specific schema;
  pin launch/relaunch call `TmuxRunner::new_session` / `send_keys`
  (`cli.rs:5063-5105`); `resolve/pins.rs` reconstructs `tmux:<socket>:<name>`
  keys; `pin_bindings` sidecars store the same shape.
- **Rename lockstep (ADR 0029):** `rename.rs` and the TUI rename overlay
  call `TmuxRunner::rename_session` directly (`tui/runtime.rs:946`,
  `cli.rs:4102`).
- **Hooks:** `HookTmuxRecord` is a named-tmux structure; `cli.rs:1113`
  (`tmux_context`) shells `tmux display-message` directly to capture mux
  context at hook time; `hook_sidecar` attribution is keyed on tmux
  session/pane identity. `cli.rs:2694` probes `current_tmux_session_name`
  for the "refuse to attach to self" guard.
- **Preview:** the TUI right-pane mux preview calls `capture_pane`
  (`tui/runtime.rs:117` wires `SystemTmux` directly into the run loop).
- **Identity:** mux node ids embed the backend string (`tmux:<name>`,
  `tmux:<socket>:<name>`), which is fine (backend-prefixed ids are the
  ADR 0001 pattern) but the *format* — the socket segment — is tmux-specific
  and is parsed back in several places (`resolve/pins.rs:187`).
- `MuxSessionNode` fields (`active_pane_command`, `active_pane_pid`,
  `active_pane_current_path`, `active_pane_start_command`,
  `client_attached`) are tmux-*flavored* but conceptually generic; zellij
  can populate a subset. Sparse-by-default handles the misses. No model
  change needed beyond documentation.

**Verdict:** deepest coupling of the four. Discovery-only zellij support is
maybe two days of work even today; zellij with attach/pins/rename/preview
parity requires a real `MuxBackend` abstraction. The good news: every call
site already goes through `&dyn TmuxRunner`, so the refactor is a rename +
generalization, not an untangling.

### 3. Forges

**Intended seam:** `ForgeAdapter` (`src/discovery/forge/mod.rs:44`) —
`provider()`, `discover()`. Cleanest of the four. `ForgePrNode` carries
provider-neutral identity (`provider`, `host`, `owner`, `repo`, `number`)
per the ADR 0011 design; output/`tui` PR surfaces consume node fields.

**The gap:**

- `LocalDiscoveryConfig.forge_runner` is `Option<Box<dyn GhRunner>>` — the
  config seam is gh-specific, not adapter-generic, and
  `discover_local_warm_with` (`discovery/mod.rs:356`) instantiates
  `GitHubForgeProvider` inline with the `"github"` key. A GitLab adapter
  has no slot to occupy; `ForgeDiscovery` (the multi-adapter coordinator)
  exists but nothing constructs it.
- `CONSPECTUS_DISABLE_FORGE` treats "forge" as a single toggle.
- The `forge` TTL class is one bucket for all forges (probably fine).
- Which remote belongs to which forge is currently implicit (gh decides).
  A second forge needs an explicit `claims_remote(url) -> bool`-style
  routing question answered once, in the adapter interface.
- `GhRunner`/`GhOutcome`/`GhUnavailableReason` live in `forge/mod.rs`
  rather than `forge/github.rs` — cosmetic, but it makes the gh seam look
  like the forge seam.

**Verdict:** smallest gap. Mostly config-shape + wiring work, already
captured as `H-FUTURE-002` (blocked on `H-REF-004`).

### 4. Orchestrators (agent-deck, dmux, herdr, pertmux, …)

**Current state:** `AgentDeckDiscovery` is a bespoke `DiscoveryProvider`
(`src/discovery/agent_deck.rs`) with its own `LocalDiscoveryConfig` field
(`agent_deck_root`), two dedicated env vars, and inline wiring
(`discovery/mod.rs:346, 598`). ADR 0060 / `H-AGENTMUX-001` *explicitly
declined* an `AgentMuxAdapter` trait: with one surviving adapter, a trait
was speculative, and MUXPROC (process-tree evidence) was judged the primary
tool-agnostic source of agent↔mux links.

**Reassessment given the new requirement.** Supporting several orchestrators
changes the calculus that ADR 0060 was decided on, but only partially:

- The *discovery* interface is genuinely already there. An orchestrator
  adapter is a `DiscoveryProvider` that emits `Workspace` / `Fork` /
  candidate-link fragments with a `provider` string. `atelier.rs` and
  `agent_deck.rs` both prove the pattern; consumers (workspace grouping,
  detail pane, graph exports) key on `WorkspaceNode.provider` generically.
  **No new node kinds or trait are needed for read-only orchestrator
  support.** ADR 0060's "MUXPROC first, adapters only for evidence MUXPROC
  can't see" principle remains correct per-orchestrator.
- What does *not* scale is the **registration shape**: one struct field +
  two env vars + one inline wiring block *per orchestrator*. Four more
  orchestrators means four more copies of that pattern, plus four more
  `provider_class` arms.
- What has no home at all is **orchestrator capability beyond discovery**:
  `H-AGENTMUX-008` (route mux renames through agent-deck so its state.db
  stays consistent) is the first example of an orchestrator needing a
  *mutation* hook. Today the rename lockstep path knows only tmux; there is
  no seam for "the mux you are renaming is owned by an orchestrator."

**Verdict:** don't resurrect a heavyweight `AgentMuxAdapter` for discovery —
`DiscoveryProvider` suffices, matching ADR 0060. The work is (a) a generic
registration/config surface so orchestrators are added as data, and (b) a
small optional capability interface for ownership-aware mutations (rename,
maybe launch), introduced when the first mutation lands. A superseding-or-
amending ADR should record the revisit.

## Cross-Cutting Findings

- **Resolver provider leak (soft):** `process_identity_evidence_rank`
  (`src/resolve/mod.rs:741`) hardcodes evidence-string names, two of which
  are provider-flavored (`codex_log_process_thread_match`). Evidence strings
  are a de facto contract between mutator passes and the resolver with no
  type-level registry. Tolerable, but new harness aux readers must know to
  emit exactly these strings to get their scores. Worth a constants module,
  not worth a framework.
- **`SourceMetadata.fields` stringly-typed keys** — already tracked as
  `H-REF-008`; new providers multiply the risk.
- **Fixture/test infrastructure is provider-hardcoded in the same way:**
  `HarnessFixture` supports the four harnesses; `FakeTmux` / `FakeGh` are
  per-tool. Fine per adapter, but a documented conformance checklist ("what
  a new adapter's test suite must cover") does not exist yet — `H-DOC-002`
  (provider-adapter contributor guide) is the natural home.
- **Snapshot format:** adding node fields for new backends bumps the rkyv
  `format_version` → cold rebuild (ADR 0083). Cheap, by design; not a
  constraint on any chunk below.
- **Env-var conventions** (`CONSPECTUS_DISABLE_<X>`, `CONSPECTUS_<X>_ROOT`,
  `CONSPECTUS_<X>_STATE`) are consistent but hand-rolled per provider in
  `from_env`; a registry can generate them mechanically.

## Target Shape: One Descriptor Per Entity

The unifying move across all four families is the same: **make the adapter
trait the *only* per-provider code, and derive every side table from a
registry of adapter descriptors.** Concretely, per family:

- **`HarnessAdapter` (extend existing trait):** add the capabilities that
  currently live in side tables — launch options, runtime signature
  (process-command matcher, fd-path patterns, session-key grammar, role
  classifier), hook payload normalizer, transcript locator + parser factory,
  display label, default theme color, default state root. A static
  `HARNESS_REGISTRY: &[&dyn HarnessAdapter]` (or descriptor structs for the
  pure-data parts) replaces `launch_argv_for`-style matches, `HARNESS_OPTIONS`,
  `harness_label`, theme field lookup, and `from_env` state-root plumbing.
- **`MuxBackend` (generalize `TmuxRunner`):** `backend_key()`, `discover()`,
  plus capability methods with `Unsupported` defaults — `attach_argv()`,
  `rename_session()`, `new_session()`, `send_keys()`, `capture_pane()`,
  `current_session_context()` (for hooks / self-attach guard), and a
  `namespace` concept generalizing tmux's socket. Consumers keep their
  capability-outcome handling; "only tmux" gates become "backend lacks this
  capability" gates.
- **`ForgeAdapter` (already right):** add remote-routing
  (`claims_remote_url`) and swap the config slot for a list; route wiring
  through the existing `ForgeDiscovery` coordinator.
- **Orchestrators:** stay `DiscoveryProvider`; add a small
  `OrchestratorDescriptor { key, default_root, build(root) }` registry for
  wiring/config/env generation, and (later, with the first mutation feature)
  an optional `owns_mux()` / `rename_mux()` capability seam.

Provider registration (`providers.rs` consts, `provider_class`, keyed-provider
wiring, env toggles) collapses into one descriptor table consumed by
discovery wiring, the cache gate, the daemon scheduler, and `from_env`.

## Chunked Plan

Ordered so each chunk lands independently, with the mechanical registry work
first (it de-risks everything else) and the deep feature seams later. Sizes:
S ≈ ≤1 day, M ≈ 2–3 days, L ≈ ~1 week. Existing backlog ids referenced where
they overlap; each chunk that changes a convention owes an ADR per the
project guardrails.

Filed in `docs/backlog.md` § Provider Extensibility as `H-EXT-001` through
`H-EXT-017`, in the order listed here (A1 → `H-EXT-001`, …, E2 →
`H-EXT-017`). `H-REF-004`, `H-FUTURE-001`, `H-FUTURE-002`, and `H-DOC-002`
are closed as folded into `H-EXT-008` / `-010` / `-013` / `-017`
respectively; the `H-EXT` entries are the tracking source of truth.

**Phase A — registry backbone (no behavior change)**

- **A1 (M): Provider descriptor registry.** Replace `providers.rs` bare
  consts + `cache::provider_class` match + `discover_local_warm_with`
  hand-wiring + `from_env` env plumbing with a descriptor table
  (`key`, `class`, `enable_env`, `root_env`, constructor). Snapshot strings
  must stay byte-identical (the `canonical_strings_are_stable` test pins
  this). ADR: provider registration convention. Unblocks: every later chunk.
- **A2 (S): Harness registry for pure-data lookups.** Fold `launch_argv_for`,
  `resume_argv_for`, `launch_options_for`, `strip_known_launch_option_fragments`
  (`harness/mod.rs`), `HARNESS_OPTIONS` (controls), `harness_label`
  (rows), and `tui/resume.rs` dispatch into iteration over the registered
  adapters. Deletes four parallel match tables. No new trait methods needed
  except `display_label()` and `launch_options()`.
- **A3 (S): Theme colors keyed by harness key.** Replace the four
  `harness_*` theme fields + `harness_color` match with a map populated from
  adapter defaults, `[tui.theme.harness.<key>]` overrides (keep the old flat
  keys as deprecated aliases per the ADR 0031 precedent). Unknown-harness
  fallback already exists.

**Phase B — harness experience parity**

- **B1 (L): Runtime signatures on `HarnessAdapter`.** Move cross_link's
  per-harness knowledge (command matching, fd-path patterns, session-key
  grammar, subagent/background role classification) onto the adapter as a
  `RuntimeSignature` value. cross_link consumes signatures generically;
  resolver evidence strings move to shared constants (fixes the
  `resolve/mod.rs:741` soft leak). This is the chunk that makes a new
  harness *attributable to muxes* by implementing one interface. Heavy
  snapshot-test surface; do it as its own reviewable branch.
- **B2 (M): Hook normalization on the adapter.** Replace the per-harness
  `*_record_from_payload` functions and the `HookWriteHarness` clap enum with
  `conspectus hook write <harness-key>` resolving through the registry +
  `HarnessAdapter::hook_record_from_payload`. Per-harness hook *plugins*
  (e.g. `plugins/opencode-hook`) stay per-harness by nature; document the
  contract in the H-DOC-002 guide.
- **B3 (M): Viewer locator/parser via the adapter.** Replace the closed
  `SessionLocator` enum + `viewer_bridge` match + fixed parser trio with
  `HarnessAdapter::transcript_source(&AgentSessionNode) -> Option<…>`
  returning an opaque locator plus a `HarnessParser` handle. Keeps the
  renderer 100% harness-neutral (it already is). Aider gains an explicit
  "no transcript source" answer instead of being silently absent.
- **B4 (S): Aux-reader hook.** Generalize the codex_log wiring
  (`codex_log_disabled` / window fields in `LocalDiscoveryConfig`,
  `apply_mutators` special case) into an adapter-provided optional mutator
  pass so the next harness with a state/log DB (ADR 0048-style) doesn't add
  top-level config fields. Small now; painful later if skipped.

**Phase C — mux backend abstraction (supersedes/absorbs `H-REF-004` +
`H-FUTURE-001`)**

- **C1 (M): Extract `MuxBackend` trait from `TmuxRunner`.** Backend-neutral
  method names, `backend_key()`, namespace generalization of `socket_name`,
  neutral outcome enums (the shared external-runner seam from `H-REF-004`
  lands here). `SystemTmux` becomes the first impl; all `&dyn TmuxRunner`
  call sites (runtime, cli rename, pins launch, preview) move to
  `&dyn MuxBackend` resolved *by the node's/pin's `backend` field* through a
  backend registry. `LocalDiscoveryConfig.tmux_runner` →
  `mux_backends: Vec<…>`. ADR required (supersedes parts of 0057's launch
  wording).
- **C2 (S): Capability-gated UI/CLI.** Replace the "only tmux" attach gate
  and pin `backend == "tmux"` validation with registry + capability checks,
  so error messages say "backend `zellij` does not support send-keys" rather
  than naming tmux. Pin schema keeps `backend` as declared data (ADR 0057
  already reserved it).
- **C3 (M): Zellij backend, discovery + attach.** `zellij list-sessions`
  parsing, attach argv, no-namespace semantics; rename/send-keys/capture as
  `Unsupported` initially. Fixture-driven tests per `H-FUTURE-001`. This is
  the acceptance test for C1: it must require *zero* edits outside the new
  module + registry entry.
- **C4 (S): Hook mux context via backend probe.** Generalize
  `tmux_context()` / `current_tmux_session_name()` in `cli.rs` and
  `HookTmuxRecord` naming so hook records carry `(backend, session, pane?)`
  neutrally. Schema-versioned sidecar change.

**Phase D — forge and orchestrator registries**

- **D1 (S): Forge adapter list.** `LocalDiscoveryConfig.forge_runner` →
  forge adapter list wired through the existing `ForgeDiscovery`
  coordinator; move `GhRunner` types into `forge/github.rs`; add
  `claims_remote_url` routing to `ForgeAdapter`. Per-forge disable toggles.
- **D2 (M): Second forge adapter (GitLab or Gitea)** per `H-FUTURE-002`,
  as the D1 acceptance test. Answers the deferred ForgePr identity questions
  (design.md "Forge PR Identity") for the multi-forge case — needs its ADR.
- **D3 (M): Orchestrator registration surface.** Generic
  `[orchestrators.<key>]` config table (root, enable) + descriptor registry
  replacing the bespoke `agent_deck_root` field/env wiring; amend ADR 0060
  to record why discovery stays `DiscoveryProvider`-shaped and what would
  trigger the capability trait. dmux/herdr/pertmux/workmux adapters then
  land as independent S/M chunks each (subject to the per-orchestrator
  evidence audits the `H-AGENTMUX-*` stories already gate on).
- **D4 (S, deferred until first need): Orchestrator mutation capability.**
  `owns_mux` / rename routing seam for `H-AGENTMUX-008`.

**Phase E — conformance and docs**

- **E1 (M): Adapter conformance suites.** A shared test harness per entity
  family asserting the invariants every adapter must satisfy (stable node
  ids, provenance stamping, sparse-input tolerance, registry round-trip,
  capability-outcome behavior), plus fixture-corpus integration so a new
  adapter shows up in `fixture_corpus` / snapshot tests by adding fixtures
  only.
- **E2 (S): Provider-adapter contributor guide** (`H-DOC-002`): the
  end-to-end checklist for each entity family — what to implement, what the
  registry gives you for free, what needs fixtures, what needs an ADR.

### Dependencies at a glance

```
A1 ──► B1, B4, C1, D1, D3
A2 ──► B2, B3
C1 ──► C2 ──► C3, C4
D1 ──► D2
D3 ──► (per-orchestrator adapters), D4
E1/E2 after the first of each family lands
```

### Acceptance criterion (the "single interface" test)

After Phases A–D, adding each entity must reduce to:

- **Harness:** one module implementing `HarnessAdapter` (incl. signature,
  hook normalizer, transcript source) + one registry entry + fixtures.
- **Mux:** one module implementing `MuxBackend` + registry entry + fixtures.
- **Forge:** one module implementing `ForgeAdapter` + registry entry +
  fixtures.
- **Orchestrator:** one `DiscoveryProvider` module + descriptor entry +
  fixtures.

Anything a new provider needs *outside* its module and the registry line is
a regression against this document.
