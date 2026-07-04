# Provider Adapter Guide

This guide is the end-to-end checklist for adding a new
provider adapter to Conspectus, keyed to the four entity
families the H-EXT stream established:

- **Harness adapters** (`discovery/harness/*`) — new agent
  harnesses (aider-shape, codex-shape, etc.).
- **Mux backends** (`discovery/tmux/*` + `discovery/zellij/*`)
  — new terminal multiplexers.
- **Forge adapters** (`discovery/forge/*`) — new pull-request
  forges.
- **Orchestrator adapters** (`discovery/agent_deck/*` +
  `discovery/orchestrator.rs` registry) — new workflow
  orchestrators.

Every family follows the same pattern:

1. Ship a module that implements the family's trait(s).
2. Register the module in the family's registry.
3. Rely on the shared driver to route calls to your adapter.

Once you register, everything downstream — CLI dispatch, TUI
badge rendering, provenance stamping, warm-start cache gating,
freshness classification — reads through the registry and picks
up your adapter without further edits.

## Harness adapters

**Trait:** `discovery::harness::HarnessAdapter`.

**Required methods:**
- `harness_key(&self) -> &'static str` — stable identifier
  (e.g. `"codex"`); appears on `AgentSessionId::harness_key`.
- `discover(&self, ctx: &DiscoveryContext) -> Result<GraphFragment>`
  — read the harness's state root and emit
  `AgentSessionNode`s.
- `runtime_signature(&self) -> &'static RuntimeSignature` —
  process command basenames, fd-path patterns, session-key
  extraction fn, subagent/background heuristics. Point
  `harness_key` on the signature at the same key.

**Optional methods with defaults:**
- `display_label() -> &'static str` — short row label
  (defaults to `harness_key`; override when the key is longer
  than the display budget).
- `launch_argv() -> Vec<OsString>` — default spawn argv.
- `launch_options() -> &'static [HarnessLaunchOption]` — pin
  editor toggle set.
- `resume_argv(session_id, cwd) -> Option<Vec<OsString>>` —
  single-command resume.
- `hook_record_from_payload(payload, ...) -> Result<HookRecord>`
  — SessionStart hook payload → sidecar record (default
  reads canonical `session_id` string).
- `transcript_source(session) -> Option<SessionLocator>` —
  where the native transcript lives.
- `transcript_parser() -> Option<&'static dyn HarnessParser>`
  — how to read it.
- `apply_aux_attribution(snapshot, ctx)` — ADR 0048's
  state/log DB reader shape (codex-log style).

**Registration:**
- Add the adapter to `discovery/harness/mod.rs::REGISTERED_ADAPTERS`
  in the order it should appear in the TUI filter menu.
- Also add it to `HarnessDiscovery::with_default_adapters` if
  the harness has a discovery step (most do).
- Update `discovery/providers.rs::REGISTRY` with a new
  `ProviderDescriptor { key: <YOUR_KEY>, kind:
  Heavy(ProviderClass::Harness) }` entry.

**Worked example:** `discovery/harness/codex.rs` — signature
+ launch options + hook record + transcript source + aux
attribution (codex_log). Compare against `aider.rs` for the
minimal shape (no state DB, no native viewer).

**What the registry provides for free:** TUI filter menu
population (`tui/widgets/controls.rs::harness_options`), row
label rendering (`tui/rows/mod.rs::harness_label`), resume
dispatch (`tui/resume.rs`), hook writer CLI dispatch
(`conspectus hook write <key>`), cross_link attribution
(`discovery/cross_link.rs` iterates `runtime_signature`),
identity-color theming (`tui/theme.rs::harness_color`).

**What needs fixtures:** each harness's `discover` needs
fixtures under `tests/fixtures/<harness>/`. See existing
fixture layouts in `tests/harness_snapshots.rs`.

**What needs an ADR:** the harness's payload schema and any
non-standard attribution shape (see ADR 0013 for opencode's
SQLite path, ADR 0048 for codex's state/log split, ADR 0086
Tier 2 for any transcript-payload access).

## Mux backends

**Trait:** `discovery::tmux::MuxBackend`.

**Required methods:**
- `backend_key(&self) -> &'static str` — stable identifier
  (`"tmux"`, `"zellij"`).
- `list_sessions(&self, format: &str) -> Result<TmuxOutcome>`
  — raw stdout suitable for your backend's discovery
  wrapper.

**Optional methods with defaults (all return
`Unsupported`):**
- `capture_pane(namespace, target)` — preview capture.
- `rename_session(namespace, target, new_name)` — mux
  rename lockstep.
- `new_session(namespace, name, cwd, argv)` — pin launch.
- `attach_session(namespace, name)` — attach dispatch.
- `send_keys(namespace, target, literal, press_enter)` —
  pin-launch send-keys exception.
- `current_session_context() -> Option<MuxSessionContext>` —
  hook-writer probe.

**Registration:**
- Author `discovery/<backend>/mod.rs` with your `MuxBackend`
  impl. Ship a per-backend `DiscoveryProvider` wrapper
  (analogous to `TmuxDiscovery`) that parses the raw
  `list_sessions` stdout format your backend uses.
- Register the backend key in
  `discovery/tmux/mod.rs::KNOWN_MUX_BACKENDS`.
- Add a `ProviderDescriptor { key: <KEY>, kind:
  Heavy(ProviderClass::Mux) }` entry in
  `discovery/providers.rs::REGISTRY`.
- Push a default instance in
  `LocalDiscoveryConfig::from_env` (respecting a
  `CONSPECTUS_DISABLE_<KEY>` opt-out).
- Wire the per-backend discovery wrapper into
  `discover_local_warm_with` using
  `config.take_mux_backend_by_key(YOUR_KEY)`.

**Worked example:** `discovery/zellij/mod.rs` — the H-EXT-010
acceptance test for the H-EXT-008 seam. `SystemZellij` implements
`list_sessions` + `attach_session`; the rest keep their
`Unsupported` defaults. `ZellijDiscovery` parses zellij's
human-formatted output. Zero edits outside the new module +
three registration entries.

**What the registry provides for free:** pin `mux.backend`
validation (`pins.rs` consults `KNOWN_MUX_BACKENDS`), attach
target dispatch (`tui/actions.rs::resolve_attach_target`),
capability-outcome gating (callers check the outcome variant,
not the backend name), warm-start freshness classification
(`cache::provider_class` derives from the registry).

**What needs fixtures:** canned `list-sessions` output for
your backend's format under
`tests/fixtures/<backend>/list-sessions.txt` or inline in
the parser's test module.

**What needs an ADR:** any capability whose semantics diverge
from tmux's (e.g. a rename shape that isn't a single-atom
op, a namespace concept without a socket-file mapping).
ADR 0089 is the shape ADR; supersede or extend it as
needed.

## Forge adapters

**Trait:** `discovery::forge::ForgeAdapter`.

**Required methods:**
- `provider(&self) -> &str` — stable identifier
  (`"github"`, `"gitlab"`).
- `discover(&self, ctx: &DiscoveryContext) -> Result<GraphFragment>`
  — emit `ForgePrNode`s.
- `claims_remote_url(&self, remote_url: &str) -> bool` —
  which git origin URLs this adapter owns.

**Registration:**
- Author `discovery/forge/<forge>.rs` with your
  `ForgeAdapter` impl. If you need a runner seam (shell out
  to a CLI, like `gh` for GitHub), define a
  `<Forge>Runner` trait mirroring `GhRunner` in the same
  module.
- Push a default instance in
  `LocalDiscoveryConfig::from_env` (respect
  `CONSPECTUS_DISABLE_FORGE` + a per-forge opt-in / opt-out
  env var).
- Adapters are drained by `discover_local_warm_with` and
  fanned through `ForgeDiscovery::with_boxed_adapter`; no
  additional wiring needed there.

**Worked examples:**
- `discovery/forge/github.rs` — full implementation shape:
  `GhRunner` trait, `SystemGh` impl, `FakeGh` for tests,
  `GhPullRequestParser`, `GitHubForgeProvider` implementing
  both `DiscoveryProvider` and `ForgeAdapter`.
- `discovery/forge/gitlab.rs` — H-EXT-013 skeleton adapter
  showing the minimal `ForgeAdapter` shape when real
  discovery is blocked pending an identity model ADR
  (H-DESIGN-002 for gitlab).

**What the registry provides for free:** origin-URL routing
(`ForgeDiscovery` fans repos to adapters by
`claims_remote_url`), warm-start freshness classification.

**What needs fixtures:** canned CLI output or REST payload
under `tests/fixtures/<forge>/pr-list.json`.

**What needs an ADR:** the forge's `ForgePr` identity
model (owner/repo/number vs. host/project/iid) if it
differs from GitHub's. Multi-forge identity is currently
unsettled — see H-DESIGN-002.

## Orchestrator adapters

**Registration surface:** `discovery::orchestrator::OrchestratorDescriptor`.

**Required fields on the descriptor:**
- `key: &'static str` — stable identifier
  (`"agent_deck"`).
- `env_root_var: &'static str` — env override for the
  filesystem root (`"CONSPECTUS_AGENT_DECK_ROOT"`).
- `env_disable_var: &'static str` — env-based disable
  (`"CONSPECTUS_DISABLE_AGENT_DECK"`).
- `home_relative_default: Option<&'static str>` —
  `HOME`-relative fallback (`Some(".agent-deck/multi-repo-worktrees")`).
- `build: fn(PathBuf) -> Box<dyn DiscoveryProvider>` —
  turn a resolved root into a discovery provider.

**No shared trait beyond `DiscoveryProvider`.** Orchestrators
have divergent filesystem shapes and no common attribution
rules. ADR 0060 kept this open; H-EXT-014 preserves the
stance.

**Registration:**
- Author `discovery/<orchestrator>/mod.rs` with your
  `DiscoveryProvider` impl.
- Push an `OrchestratorDescriptor` entry into
  `discovery/orchestrator.rs::REGISTRY`.
- Everything downstream (config-table parsing, warm-start
  gating, from-env defaults) picks up the new orchestrator
  automatically.

**Worked example:** `discovery/agent_deck/mod.rs` — the sole
registered orchestrator today. Reads
`<root>/<repo>-<checkout>` directory naming and emits
`WorkspaceNode`s + `AgentSessionNode`s.

**What the registry provides for free:** env-var contract
(each descriptor's `resolve_default_root()` walks
disable → root-var → HOME-fallback uniformly), warm-start
cache slot (freshness gate reads through the descriptor's
provider key).

**What needs fixtures:** each orchestrator's fixture layout
under `tests/fixtures/<orchestrator>/` matching the
discovery pass.

**What needs an ADR:** an orchestrator with a mutation
capability (e.g. rename routing per H-AGENTMUX-008 for
agent-deck) needs H-EXT-015's optional
`OrchestratorMutation` trait shape, which is currently
deferred pending its first consumer.

## Hook plugins

**Not a Rust adapter surface.** Hook plugins live in the
harness's own config (`.claude/settings.json`,
`.codex/config.toml`, `opencode.jsonc`) and shell out to
`conspectus hook write <harness-key>`. See
`plugins/opencode-hook/README.md` for the plugin contract.

## Adapter conformance

Every family has a shared invariant suite at
`tests/adapter_conformance.rs` (H-EXT-016). A new adapter
should pass the family-specific invariant test set out of
the box; failing one means the registration is incomplete
(wrong key, missing provider descriptor, class mismatch).
Run it via `cargo test --test adapter_conformance`.

## Reading order for a new adapter

If you're adding a new adapter, read in this order:

1. **This guide** — the end-to-end checklist above.
2. **The family's ADR** —
   ADR 0088 (provider descriptor registry) for cross-family
   context; ADR 0087 (mutation envelope) for what mutations
   your adapter can perform; the family-specific ADR
   (ADR 0089 for mux, ADR 0060 for orchestrator, no dedicated
   forge ADR yet).
3. **The worked example** — the closest existing adapter's
   module.
4. **The conformance suite** — `tests/adapter_conformance.rs`
   to understand the family invariants.
5. **The family's registration point** — one of
   `REGISTERED_ADAPTERS` (harness), `KNOWN_MUX_BACKENDS` +
   `LocalDiscoveryConfig::from_env` (mux backend),
   `LocalDiscoveryConfig::from_env` (forge), or
   `orchestrator::REGISTRY` (orchestrator).
