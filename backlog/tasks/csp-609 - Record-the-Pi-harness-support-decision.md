---
id: CSP-609
title: Record the Pi harness support decision
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:01'
updated_date: '2026-10-17 13:28'
labels:
  - h-pi
milestone: m-22
dependencies: []
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/session-format.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/sessions.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/cli.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/environment-variables.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md
documentation:
  - docs/adr/0018-intra-harness-session-lineage.md
  - docs/adr/0049-opencode-hook-plugin-distribution.md
  - docs/adr/0086-payload-privacy-tenet.md
  - docs/adr/0087-mutation-envelope.md
  - docs/provider-adapter-guide.md
priority: medium
type: docs
ordinal: 658000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi (https://pi.dev, `@earendil-works/pi-coding-agent`, evaluated at 1.3.0 / upstream commit 8efd314 on 2026-10-16) is a minimal, extension-first coding-agent harness. `docs/design.md` pins the v1 harness set to claude-code, opencode, codex, and aider and says not to add harnesses "until needed", so adding Pi needs an ADR and a design update before any adapter code lands.

Pi facts the ADR has to settle against the existing model:

- **Session store.** JSONL at `~/.pi/agent/sessions/--<cwd>--/<timestamp>_<session-id>.jsonl`. The directory name encodes cwd lossily (`/`, `\`, `:` all become `-`), so cwd must come from the header line (`{"type":"session","version":3,"id","timestamp","cwd","parentSession"?}`), never the directory. `PI_CODING_AGENT_DIR` moves the agent dir; `PI_CODING_AGENT_SESSION_DIR`, `--session-dir`, and the `sessionDir` setting replace the grouped layout with one flat directory. Files are written lazily (nothing on disk until the first exchange) and are not held open.
- **Session ids.** UUIDv7 by default; `--session-id` accepts custom ids of `[A-Za-z0-9._-]`.
- **Lineage.** `/fork`, `/clone`, and `--fork` write a new file whose header `parentSession` is the parent's *file path*. `/tree` branching stays inside one file (`id`/`parentId` entry tree) and is not cross-session lineage.
- **Names and previews.** `session_info` entries carry the `/name` display name (latest wins). User/assistant text lives in `message` entries.
- **Runtime.** `process.title` is `pi` (`pi-rpc` for RPC mode); `AI_AGENT=pi` and `PI_CODING_AGENT=true` are exported to children, and the bash tool exports `PI_SESSION_ID` / `PI_SESSION_FILE`. `pi` is a short basename, so loose substring matching would collide with `pip`, `api`, and similar.
- **Resume.** `pi --session <path|id>` (searches the current project first), `pi --continue`, `pi --fork <path|id>`.
- **Hooks.** No shell-command hook config. Extensions are TypeScript modules loaded through jiti from `<agent-dir>/extensions/` or `.pi/extensions/` with no build step, and `session_start` fires with `reason` startup/reload/new/resume/fork plus `ctx.sessionManager.getSessionId()/getSessionFile()/getCwd()`.

Scope: new ADR (next free number) plus `docs/design.md` harness-set update. Docs only.

Manual checks: `git diff --check`.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A new ADR in `docs/adr/` records adding `pi` as a supported harness with harness key `pi` and display label `pi`
- [x] #2 The ADR fixes state-root resolution: `CONSPECTUS_PI_STATE` override, then `PI_CODING_AGENT_SESSION_DIR`, then `PI_CODING_AGENT_DIR`/sessions, then `~/.pi/agent/sessions`, and states that both the grouped and the flat layout are scanned and cwd is always read from the header
- [x] #3 The ADR maps `parentSession` to an ADR 0018 `parent_session` candidate with an explicit `lineage_kind`, and states that in-file `/tree` branches emit no cross-session lineage
- [x] #4 The ADR classifies the Pi last-message preview and transcript parser as ADR 0086 Tier 2 readers with a cap and normalize step, and the header/name/timestamp reads as Tier 1
- [x] #5 The ADR decides the runtime signature: exact basenames only (no `pi` substring match), how `pi-rpc` processes are classified, and whether `fd_path_patterns` are useful given Pi does not hold the session file open
- [x] #6 The ADR decides how the Pi hook extension is distributed (following or diverging from ADR 0049) and whether `conspectus hook init pi` may write an extension file into the Pi extensions directory under the ADR 0087 hook-installation category
- [x] #7 `docs/design.md` lists `pi` in the supported harness set and links the new ADR
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Verify Pi facts against upstream docs (sessions, session-format, settings, extensions, env vars) and against the local `~/.pi/agent` (one empty cwd dir, consistent with lazy writes).
2. Draft ADR 0121 covering identity, state root, tiers, lineage, runtime signature, and hook distribution; index it in docs/adr/README.md.
3. Update the docs/design.md v1 harness set.
4. Present the material decisions to the operator for approval before committing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Upstream settings.md: `PI_CODING_AGENT_SESSION_DIR` and `--session-dir` override `sessionDir`; project settings override global; relative `sessionDir` resolves from cwd. The ADR adds an absolute global `sessionDir` as step 3 of the root chain, a refinement of AC #2.
`hook_sidecar::ensure_session` already skips records whose transcript_path does not exist, which covers Pi lazy file creation.
`hook_sidecar::pane_running_harness` is a hardcoded table and needs a `pi` entry in the hook story.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0121 records Pi as a supported harness (`pi`) and is indexed in docs/adr/README.md. docs/design.md lists `pi` in the v1 harness set.

What the ADR decides:
- State root: `CONSPECTUS_PI_STATE`, then `PI_CODING_AGENT_SESSION_DIR`, then an absolute global `sessionDir`, then `<agent-dir>/sessions`. Both layouts are scanned and cwd comes from the header.
- Payload tiers: header, name, and timestamp reads are Tier 1; the preview and parser are Tier 2.
- Lineage: `parent_session` with `lineage_kind=fork` and `lineage_fidelity=native`.
- Runtime signature: exact basenames `pi` and `pi-rpc` with a new exact-token pane-command match, `pi-rpc` treated as Background, and no fd patterns.
- Hook: a single embedded TypeScript extension that `hook init pi` / `hook remove pi` install and remove under ADR 0087 category 5, with marker ownership.

The global `sessionDir` step goes beyond AC #2 and follows Pi's own precedence. The operator approved the decisions on 2026-10-17. Verified with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
