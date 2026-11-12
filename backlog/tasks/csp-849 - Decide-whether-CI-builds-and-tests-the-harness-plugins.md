---
id: CSP-849
title: Decide whether CI builds and tests the harness plugins
status: To Do
assignee: []
created_date: '2026-11-12 11:18'
labels:
  - live-state
  - ls-emit
  - needs-decision
milestone: m-26
dependencies: []
priority: low
type: task
ordinal: 934000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The opencode plugin now holds a stateful reducer, and the Pi extension will (CSP-817). Its pure module is tested with `node --test` by hand (`just opencode-plugin-test`), and its TypeScript glue (`src/index.ts`, `src/v2/server.ts`) is never compiled by CI: ADR 0049 decision 2 keeps the plugin out of CI, and ADR 0206 leaves a plugin test runner to its own ADR. A typing or API drift against `@opencode-ai/plugin` or `@opencode/plugin` surfaces only when an operator builds; the 1.x devDependency (1.14.19) does not even declare the `dispose` hook the plugin uses. Options include Node in the dev shell with CI running the plugin tests and `npm run typecheck` (which needs the npm dependencies in the Nix build), or keeping it manual. A decision for the operator.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records whether and how CI builds and tests the plugins
- [ ] #2 If adopted, CI runs the plugin tests and the type check
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
