---
id: CSP-849
title: Decide whether CI builds and tests the harness plugins
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 11:18'
updated_date: '2026-11-14 20:51'
labels:
  - live-state
  - ls-emit
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
- [x] #1 ADR 0235 records the operator's decision of 2026-11-13: Node 24 joins the dev shell, a CI job runs both plugins' tests and type checks, `just check` stays free of Node and the network, with the alternatives and costs; it is indexed in docs/adr/README.md and amends ADR 0049 decision 2 and ADR 0206's Node consequence
- [x] #2 `flake.nix` adds Node to the dev shell and `scripts/dev-env.sh node --version` builds and prints it
- [x] #3 `just plugins` runs both plugin test suites and both plugins' `tsc --noEmit` (after `npm ci`) and passes; `just check` does not run it and needs neither Node nor the network
- [x] #4 A `plugins` job in .github/workflows/ci.yml runs `just plugins` with Node installed by actions/setup-node; the workflow parses as YAML and passes actionlint
- [x] #5 The Pi extension is type-checked in the job (package.json, tsconfig.json, and a lock file under plugins/pi-hook)
- [x] #6 Docs (README, both plugin READMEs, ADR index, live-state plan note) say the plugins are tested and type-checked in CI, and a patch for AGENTS.md is left for the coordinator
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Findings: the registry is reachable only through npm and its mirror from the agent sandbox (curl to registry.npmjs.org fails), so the first `npm ci` ran and the opencode plugin's type check ran for the first time: it found one error (src/v2/server.ts, `types` in `ctx.event.subscribe` options, undeclared by the pinned @opencode/plugin 2.0.32; 2.2.x declares it). Fixed by building the options apart from the call. The Pi lock file came out with the mirror's host in every `resolved` URL; rewritten to registry.npmjs.org before committing (npm ci then passed from a fresh cache). Closed CSP-882 (the Pi type check) as landed here.
The CI job cannot run here: only the workflow's syntax (yq, actionlint 1.7.12) and the same commands through `just plugins` in the dev shell were checked. Watch the first CI run after the push.
AGENTS.md: the exact change is in /tmp/csp-fu/agents-md-849.patch for the coordinator to apply.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Operator decision of 2026-11-13, carried out. ADR 0235 records it (indexed under Process, tooling, and distribution; amends ADR 0049 decision 2 and its live-state item 4, and ADR 0206's Node consequence). `flake.nix` adds `nodejs_24` (24.14.1 in the locked nixpkgs; about 160 MiB over a 2.6 GiB closure) and the banner lines; `scripts/dev-env.sh node --version` builds and prints v24.14.1, and `nix flake check --no-build` evaluates. The justfile gains `plugins` (both test recipes, then `plugins-typecheck`) and `plugins-typecheck` (`npm ci --ignore-scripts --no-audit --no-fund` and `npm run typecheck` in `plugins/opencode-hook` and `plugins/pi-hook`); `check` is unchanged. ci.yml gains a `plugins` job: checkout@v4, setup-node@v4 (Node 24, npm cache keyed on both lock files), `taiki-e/install-action@just`, `just plugins`; CI does not use Nix, so the Node major is set in the flake and the workflow, each with a comment naming the other. The Pi extension is type-checked through a private `package.json`, `tsconfig.json`, and lock under `plugins/pi-hook` (Pi's package 1.5.2 for types only); `hook init pi` still copies one file. The first type check found and fixed one error in `src/v2/server.ts`.

Verified: `just plugins` in the dev shell (opencode 40/40 and Pi 34/34 tests, both `tsc --noEmit` passes), yq and actionlint on the workflow, `/tmp/csp-fu/checks.sh`, and `just check` (5752 passed, 1 skipped). Not verified here: the CI run itself. AGENTS.md is untouched; its change is in /tmp/csp-fu/agents-md-849.patch.
<!-- SECTION:FINAL_SUMMARY:END -->
