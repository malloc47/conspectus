---
id: CSP-849
title: Decide whether CI builds and tests the harness plugins
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-12 11:18'
updated_date: '2026-11-14 15:04'
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
- [ ] #1 ADR 0235 records the operator's decision of 2026-11-13: Node 24 joins the dev shell, a CI job runs both plugins' tests and a type check of the opencode plugin, `just check` stays free of Node and the network, and the alternatives and costs; it is indexed in docs/adr/README.md and amends ADR 0049 decision 2 and ADR 0206's Node consequence
- [ ] #2 `flake.nix` adds Node to the dev shell and `scripts/dev-env.sh node --version` builds and prints it
- [ ] #3 `just plugins` runs both plugin test suites and the opencode plugin's `tsc --noEmit` (after `npm ci`); `just check` does not run it and needs neither Node nor the network
- [ ] #4 A `plugins` job in .github/workflows/ci.yml runs `just plugins` with Node installed by actions/setup-node; the workflow parses as YAML and passes actionlint
- [ ] #5 The Pi extension's type check is either in the job or filed as a follow-up with the reason, and the ADR and the report say which
- [ ] #6 Docs (README, both plugin READMEs, ADR index, live-state plan note) say the plugins are tested and type-checked in CI, and a patch for AGENTS.md is left for the coordinator
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0235 first (Node in the dev shell, the plugins CI job, alternatives, cost: closure size and cold start), index it, amend ADR 0049 decision 2 and ADR 0206's Node consequence.
2. flake.nix: nodejs_24 in devTools plus banner lines; verify with scripts/dev-env.sh node --version (revert if Nix cannot build it).
3. justfile: plugins and plugins-typecheck recipes beside the two existing test recipes; update their comments.
4. ci.yml: a plugins job (checkout, setup-node, just, just plugins); validate with a YAML parser and actionlint.
5. Docs: README development section, both plugin READMEs, live-state plan note; AGENTS.md patch to /tmp/csp-fu/agents-md-849.patch.
6. The Pi extension has no package.json and no lock file can be made offline (the npm registry is unreachable here): the job tests it only; file the type check as a follow-up.
7. checks.sh and just-check.sh; commit in reviewable commits.
<!-- SECTION:PLAN:END -->
