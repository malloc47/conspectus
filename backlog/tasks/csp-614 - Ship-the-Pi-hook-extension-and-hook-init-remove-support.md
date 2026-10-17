---
id: CSP-614
title: Ship the Pi hook extension and hook init/remove support
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 14:02'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-609
  - CSP-612
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/environment-variables.md
documentation:
  - docs/adr/0028-hook-sidecar-mux-attribution.md
  - docs/adr/0049-opencode-hook-plugin-distribution.md
  - docs/adr/0087-mutation-envelope.md
  - plugins/opencode-hook/README.md
priority: medium
type: feature
ordinal: 663000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Process-level attribution goes stale when an operator switches sessions inside a running Pi (`/resume`, `/new`, `/fork`, `/clone`), and Pi does not keep its session file open, so fd scans can't correct it. Claude-code and opencode solve this with hook sidecar records (ADR 0028) written by `conspectus hook write <harness>`. Pi has no shell-hook config; its equivalent is a TypeScript extension that Pi loads through jiti with no build step from `<agent-dir>/extensions/` or `.pi/extensions/`.

The extension subscribes to `session_start` (reasons startup, reload, new, resume, fork) and forwards `{session_id, cwd, transcript_path, hook_event_name}` to `conspectus hook write pi`, following ADR 0049's contract: the `CONSPECTUS_HOOK_BIN` override, a silent no-op when the binary is missing, and no destabilizing Pi. `--no-session` runs have no session file, so the payload omits `transcript_path` or the extension skips the write, whichever the Pi ADR chose. `hook init` / `status` / `remove` gain a `pi` target that installs or removes exactly the Conspectus-owned extension file, inside the ADR 0087 hook-installation category.

Manual checks: `conspectus hook init pi`, start `pi` in tmux, `/new` and `/resume` to another session, and confirm the sidecar record and the TUI follow the switch; `conspectus hook remove pi` leaves other extensions untouched.

Blockers: the Pi ADR story (distribution and envelope decision) and the runtime attribution story (`harness_binaries` reads the Pi runtime signature for pid resolution).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `conspectus hook write pi` accepts the extension payload and writes an ADR 0028 record with harness key `pi`, pid/ppid resolved through the Pi runtime signature, and tmux context
- [x] #2 The in-repo extension emits one record per `session_start`, and a `/resume` switch inside one Pi process updates the pane attribution to the new session
- [x] #3 `conspectus hook init pi`, `hook status pi`, and `hook remove pi` install, report, and remove only the Conspectus-owned extension file at user and project scope; any other files in the extensions directory are left untouched
- [x] #4 Any new write path or program is listed in `docs/capabilities.toml` citing ADR 0087, and the capability tests pass
- [x] #5 Tests cover payload parsing (missing or empty `session_id`, absent `transcript_path`), install/remove idempotence, and the stale-attribution demotion under harness key `pi`
- [x] #6 The extension README documents installation, the `CONSPECTUS_HOOK_BIN` override, and how to opt out
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `plugins/pi-hook/conspectus-hook.ts` and a README. On `session_start` in TUI mode with a session file, the extension spawns `$CONSPECTUS_HOOK_BIN || conspectus` with `hook write pi` and the flat ADR 0049 payload, and ignores every failure.
2. Embed the file in `src/cli/hook.rs` and add a `pi` target to `hook init`, `status`, and `remove`: user scope `<PI_CODING_AGENT_DIR|~/.pi/agent>/extensions/conspectus-hook.ts`, project scope `.pi/extensions/conspectus-hook.ts`. The first-line marker decides ownership (installed, outdated, not-installed, foreign), and a temp-file rename does the write.
3. `hook write pi` uses the default payload reader and adds `CONSPECTUS_PI_HOOK_VERSION`.
4. In the hook sidecar, `pane_running_harness` reads the adapter registry instead of a hardcoded table, and Pi gets a state-scope branch for synthesized nodes.
5. Add both paths to `docs/capabilities.toml` `writes.hook` (ADR 0121) and add traced `hook init/remove pi` runs at both scopes.
6. Add Pi env vars to every CLI test isolation helper.
7. Tests: CLI write, payload errors, install/remove idempotence, outdated and foreign handling, scopes, sidecar demotion and switch, helpers. Drive the TS file under Node with a stub API.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped the Pi hook extension and its installer.
- `plugins/pi-hook/conspectus-hook.ts` (with a README) reports each interactive `session_start` to `conspectus hook write pi`. It skips `--no-session` runs and the print, JSON, and RPC modes, honors `CONSPECTUS_HOOK_BIN`, and ignores spawn failures.
- The binary embeds the file, and `conspectus hook init|status|remove pi` manages exactly that file at user scope (following `PI_CODING_AGENT_DIR`) or project scope. A first-line marker decides ownership, so `init` refuses and `remove` skips a foreign file at that path, and an outdated copy is reported and replaced.
- `hook write pi` stamps `CONSPECTUS_PI_HOOK_VERSION`.
- The sidecar reader recognizes `pi` and `pi-rpc` panes through the adapter registry, replacing the hardcoded table.
- The manifest lists both write paths under ADR 0121.
- CLI test helpers now strip the Pi env vars so tests never touch a real Pi directory.

Verification:
- `just check`: 2369 tests, clippy, fmt, rustdoc, and the diff check.
- New `cli_smoke` tests cover the write record and version, a payload without `transcript_path`, missing and empty `session_id`, the init/status/remove lifecycle (dry run, idempotence, outdated replacement, a neighbor file untouched), the foreign file at project scope, and the project-scope and `PI_CODING_AGENT_DIR` paths.
- New `hook_sidecar` tests cover the stale launch-argv demotion under `pi`, a resume switch leaving only the newer session attributed, a record ignored once the pane runs `claude`, `pane_running_harness`, and the Pi state scope.
- The traced `capabilities` test runs `hook init/remove pi` at both scopes under strace. With the Pi paths removed from the manifest it fails, as it should.
- Manual: Node 24 loaded the real `.ts` file with a stubbed `ExtensionAPI`. A TUI `session_start` wrote one record (session, cwd, transcript path, `session_start:resume`, version 1). RPC mode and a session with no file wrote nothing, and a missing binary returned cleanly.

Not verified: Pi isn't installed on this machine, so the live `pi` in tmux with `/new` and `/resume` was not run. ADR 0121's assumption that `/proc/<pid>/comm` reads `pi`, which pid resolution needs, is still unconfirmed. Under Node the pid resolves to none, which is the documented fallback.

During the first stub run, `conspectus hook write pi` reached the operator's running daemon over its socket with a stub record (session `tui-file`, a nonexistent transcript path, pane %4). The daemon holds ingests in memory only, and the sidecar skips records whose transcript file doesn't exist, so it had no effect on attribution. Later runs used `--state-root` with tmux unset.
<!-- SECTION:FINAL_SUMMARY:END -->
