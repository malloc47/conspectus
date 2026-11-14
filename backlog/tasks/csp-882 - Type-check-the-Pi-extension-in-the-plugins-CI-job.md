---
id: CSP-882
title: Type-check the Pi extension in the plugins CI job
status: To Do
assignee: []
created_date: '2026-11-14 15:04'
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
ADR 0235 has CI run the Pi extension's `node --test` suite but not a type check: `plugins/pi-hook/conspectus-hook.ts` is one file with no `package.json`, and its types come from `@earendil-works/pi-coding-agent` (an `import type`, so Node strips it and the tests never need it). The smallest check is a `package.json` and `tsconfig.json` under `plugins/pi-hook/` with a dev dependency on that package and the TypeScript the opencode plugin pins, and a committed `package-lock.json`. The session that wrote ADR 0235 could not reach the npm registry, so it could not produce the lock file.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `plugins/pi-hook/package.json` (private, no runtime dependencies), `tsconfig.json`, and a committed `package-lock.json` exist, with the Pi package's types as a dev dependency
- [ ] #2 `npm ci && tsc --noEmit` passes in `plugins/pi-hook`, with any type errors in the extension fixed or the extension's `import type` kept as the only dependency on Pi
- [ ] #3 `just plugins-typecheck` and the CI `plugins` job run it beside the opencode check; `plugins/pi-hook/README.md` and ADR 0235 say the Pi extension is type-checked
- [ ] #4 The install flow is unchanged: `conspectus hook init pi` still copies the one file, and `package.json` is not installed or required by Pi
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
