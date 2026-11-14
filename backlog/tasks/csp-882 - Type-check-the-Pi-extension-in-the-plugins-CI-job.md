---
id: CSP-882
title: Type-check the Pi extension in the plugins CI job
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 15:04'
updated_date: '2026-11-14 20:50'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-849
priority: low
ordinal: 959000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0235 has CI run the Pi extension's `node --test` suite; this task asked for its type check too. `plugins/pi-hook/conspectus-hook.ts` is one file with no `package.json`, and its types come from `@earendil-works/pi-coding-agent` (an `import type`, so Node strips it and the tests never need it). It was filed when the lock file looked impossible to produce, because the public npm registry is unreachable from the agent sandbox; npm reaches a mirror there, so CSP-849 landed it. The lock file's `resolved` URLs had to be rewritten from the mirror's host to `registry.npmjs.org` before committing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `plugins/pi-hook/package.json` (private, no runtime dependencies), `tsconfig.json`, and a committed `package-lock.json` exist, with the Pi package's types as a dev dependency
- [x] #2 `npm ci && tsc --noEmit` passes in `plugins/pi-hook`, with any type errors in the extension fixed or the extension's `import type` kept as the only dependency on Pi
- [x] #3 `just plugins-typecheck` and the CI `plugins` job run it beside the opencode check; `plugins/pi-hook/README.md` and ADR 0235 say the Pi extension is type-checked
- [x] #4 The install flow is unchanged: `conspectus hook init pi` still copies the one file, and `package.json` is not installed or required by Pi
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed with CSP-849 (commit "build(pi-hook): type-check the extension against Pi's types" and "build: Node in the dev shell, `just plugins`, and a plugins CI job"). `plugins/pi-hook/` gains a private `package.json` (dev dependencies only: `@earendil-works/pi-coding-agent` 1.5.2, `typescript` 5.8.2, `@types/node` 22.13.9), a `tsconfig.json` (`noEmit`, the one file), and a committed `package-lock.json` with every `resolved` URL on `registry.npmjs.org`. `npm ci --ignore-scripts && npm run typecheck` passes in the directory from a fresh npm cache, `just plugins-typecheck` runs it beside the opencode plugin's, and the CI `plugins` job runs `just plugins`. `conspectus hook init pi` still embeds only `conspectus-hook.ts` (`include_str!` in `src/cli/hook.rs`), and `just check` passed (5752 tests). The Pi README and ADR 0235 say the extension is type-checked.
<!-- SECTION:FINAL_SUMMARY:END -->
