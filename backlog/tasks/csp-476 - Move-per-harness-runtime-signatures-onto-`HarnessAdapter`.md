---
id: CSP-476
title: Move per-harness runtime signatures onto `HarnessAdapter`
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
ordinal: 158000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New
  `crate::discovery::harness::RuntimeSignature` struct
  packages a harness's process / fd / session-key
  attribution surface: `process_command_basenames`,
  `command_substrings`, `fd_path_patterns`,
  `extract_session_keys` fn ptr, `is_background_process`
  fn ptr, `is_subagent_process` fn ptr. `HarnessAdapter`
  gains `runtime_signature(&self) -> &'static
  RuntimeSignature` with a `STUB_RUNTIME_SIGNATURE` default
  for test / future harnesses.
  Every production adapter (codex, claude-code, opencode,
  aider) provides a module-level static
  `<KEY>_RUNTIME_SIGNATURE` const carrying the same
  patterns the pre-H-EXT-004 hand-rolled tables encoded
  inline. `claude-code`'s helper-daemon
  heuristic and opencode's subagent heuristic move onto
  the adapter as free-fn ptrs; the pre-H-EXT-004
  `is_claude_background_process` / `is_opencode_subagent_process`
  methods delegate through a
  `RuntimeProcessRecord::signature_role_check` helper.
  `cross_link.rs`'s six inlined harness surfaces
  (`process_command_harnesses`, `command_harnesses`,
  `session_keys_for_harness_text`, the fd-path branch of
  `session_key_evidence_from_fd_paths`,
  `is_claude_background_process`,
  `is_opencode_subagent_process`) all iterate
  `registered_adapters()` and consult signatures generically.
  `generic_uuid_like_session_keys` and its `is_uuid_like_bytes`
  /`uuid_boundary` helpers moved to
  `discovery::harness` so per-adapter signatures can point
  at them. `opencode_session_key_values` moved to
  `discovery::harness::opencode` under the adapter it
  belongs to.
  Resolver evidence strings (six literals used in ~28 sites
  across cross_link, codex_log, resolve) consolidated into
  a new `crate::resolve::evidence` module.
  `process_identity_evidence_rank` and every producer /
  comparison site references the constants; test literals
  stay as-is since they encode specific expected values.
  Runtime attribution behavior unchanged — the four v1
  harnesses attribute exactly as they did pre-H-EXT-004;
  a fifth adapter added to the registry with a filled-in
  `RuntimeSignature` gains attribution automatically. All
  25 suites (1517 lib tests) pass byte-identically; fmt /
  clippy clean.
- Blockers: `CSP-473` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-004`
