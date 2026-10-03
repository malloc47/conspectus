---
id: CSP-477
title: Normalize hook payloads through the adapter
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-474
ordinal: 159000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. `HarnessAdapter` gains
  `hook_record_from_payload(payload, pid, ppid, tmux,
  version, epoch) -> Result<HookRecord>` with a default
  implementation that reads ADR 0028's canonical
  `session_id` string and stamps the record with
  `self.harness_key()`. Adapters override only when their
  payload shape diverges (none do today, so all four
  inherit the default).
  Three pre-H-EXT-005 per-harness writers
  (`claude_code_record_from_payload`,
  `codex_record_from_payload`,
  `opencode_record_from_payload`) are deleted from
  `src/hook.rs`. A single free
  `hook::hook_record_from_payload(harness_key, ...)` dispatches
  through the registry; unknown keys yield an error listing
  the registered set so misconfigured hooks fail loudly
  instead of writing a record the discovery pipeline would
  silently ignore. `hook::optional_string` promoted to
  `pub hook::optional_payload_string` so the adapter's
  default trait impl can reuse it.
  CLI shape: `HookWriteHarness` clap enum (three subcommands
  each with their own `--state-root` flag) collapsed to a
  single `HookWriteArgs { harness: String, state_root:
  Option<PathBuf> }` positional. Operator configs installed
  via `conspectus hook install claude-code` continue to
  invoke `conspectus hook write <harness>` unchanged — the
  harness key IS the same positional argument.
  `cli::harness_binaries` migrated from a per-harness match
  to iterate `registered_adapters()` reading
  `RuntimeSignature::process_command_basenames`. Codex's
  signature gains `codex-rs` as an alternate binary name to
  preserve pre-H-EXT-005 CLI behavior.
  `cli::harness_version_env` introduced as a helper for the
  per-harness `HARNESS_VERSION` env-var mapping (still
  hardcoded — the mapping is a legacy quirk of the two
  variables `CLAUDE_CODE_VERSION` and
  `CONSPECTUS_OPENCODE_HOOK_VERSION` and doesn't warrant a
  trait method yet).
  Fixture-corpus and hook unit tests migrated to the single
  dispatch entry point. All 25 suites pass byte-identically;
  fmt / clippy clean.
- Blockers: `CSP-474` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-005`
