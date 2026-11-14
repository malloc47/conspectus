# ADR 0235: Node Joins The Dev Shell, And A CI Job Tests And Type-Checks The Harness Plugins

## Status

Accepted by the operator on 2026-11-13. Amends
[ADR 0049](0049-opencode-hook-plugin-distribution.md) decision 2 ("no CI
build initially") and the consequence in
[ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
that left Node out of the dev shell and CI and a plugin test runner to its
own ADR. Adds a tool to the dev shell and a job to CI, and no process,
socket, signal, or write path of the Conspectus binary (ADR 0117,
ADR 0143).

## Context

Two harness plugins are TypeScript: the opencode plugin
(`plugins/opencode-hook`, a package with a reducer in `src/live.ts` and
glue in `src/index.ts`, `src/v2/server.ts`, and `src/v2/tui.ts`) and the
Pi extension (`plugins/pi-hook/conspectus-hook.ts`, one file with a state
machine and its handlers). Their pure parts are tested with `node --test`
(`just opencode-plugin-test`, `just pi-plugin-test`), against event scripts
the Rust tests replay too. Nothing runs them automatically:

- The dev shell has no Node, so the recipes work only where an operator
  has Node 22.6 or later in their own profile, and say so otherwise. CI
  has no Node step.
- The glue is never compiled. The stories that touched it (CSP-816,
  CSP-846, CSP-847) each record "tsc was not run (no npm install
  offline)". A typing or API drift against `@opencode-ai/plugin` or
  `@opencode/plugin` surfaces only when an operator builds, and the 1.x
  dev dependency (1.14.19) does not even declare the `dispose` hook the
  plugin uses.
- The opencode plugin now holds a stateful reducer and the Pi extension a
  state machine, so a regression there changes what the TUI shows
  without a Rust test failing. ADR 0049 expected this ("if/when the
  plugin grows or breakage becomes a recurring problem") and ADR 0206 left
  it to a decision of its own.

CI today installs each tool from its own action
(`dtolnay/rust-toolchain`, `Swatinem/rust-cache`,
`taiki-e/install-action`) and does not use Nix; the dev shell is how
contributors, and agents through `scripts/dev-env.sh` (ADR 0226), get
their tools.

## Decision

1. **Node is in the dev shell.** `flake.nix` adds `nodejs_24` to the dev
   tools (24.14.1 in the locked nixpkgs), which brings `node` and `npm`.
   TypeScript is not added: the compiler is the opencode plugin's own
   locked dev dependency (`typescript` 5.8.2 in
   `plugins/opencode-hook/package-lock.json`), installed by `npm ci` into
   its ignored `node_modules/`, so the shell and CI compile with the same
   version. Node 24 strips types without a flag; the recipes keep
   `--experimental-strip-types` so they also run on the 22.6 or later the
   plugins support.
2. **One recipe runs every plugin check: `just plugins`.** It runs
   `opencode-plugin-test`, `pi-plugin-test`, and `plugins-typecheck`, the
   last of which runs `npm ci --ignore-scripts` and `npm run typecheck`
   (`tsc --noEmit`) in `plugins/opencode-hook` and `plugins/pi-hook`. The
   test recipes keep their Node version check.
3. **`just check` does not run it.** `check` stays what it was: no Node,
   no network. `npm ci` needs the npm registry, so the plugin checks are a
   recipe of their own, which an operator or agent runs after a change
   under `plugins/` or to the shared event scripts in
   `tests/fixtures/hooks/`, and which CI runs on every change.
4. **CI runs it.** A `plugins` job in `.github/workflows/ci.yml`, beside
   `rust` and `separation-rehearsal`, on the same triggers and not
   path-filtered (the scripts under `tests/fixtures/hooks/` are shared with
   the Rust tests). It checks out, installs Node 24 with
   `actions/setup-node` (caching npm's downloads, keyed on the lock file),
   installs `just` with `taiki-e/install-action` (the action the `rust`
   job uses for nextest), and runs `just plugins`. CI does not use Nix, so
   Node's major version is set in two places, `flake.nix` and the
   workflow, each with a comment naming the other.
5. **The Pi extension is type-checked through a `package.json` that is
   not a package.** The extension is a single file, and `conspectus hook
   init pi` still copies only that file. `plugins/pi-hook/` gains a
   private `package.json`, a `tsconfig.json` (`noEmit`, covering the
   extension), and a lock file, whose only dependencies are dev ones:
   `@earendil-works/pi-coding-agent` 1.5.2 (the version the research read,
   for its types: the extension imports them with `import type`, so
   nothing of Pi's loads at run time), `typescript` and `@types/node` at
   the versions the opencode plugin pins. The lock file's `resolved` URLs
   name `registry.npmjs.org`, which npm replaces with the registry it is
   configured to use; a lock file written through a mirror must be
   rewritten before it is committed.

## Consequences

- A change to either plugin, or to the event scripts, fails CI when a test
  or the opencode type check breaks, instead of when an operator builds.
- The dev shell's closure grows by Node and npm: about 160 MiB over the
  2.6 GiB it was (34 store paths, ICU and OpenSSL's development outputs
  among them). The first environment built after the change fetches
  them from the binary cache: the next `nix develop`, or the next
  `scripts/dev-env.sh` call, whose copy is keyed on `flake.nix` (ADR
  0226), pays that once. Entering the shell costs nothing more, since
  nothing starts Node.
- The type check had never run with the dependencies installed, and its
  first run found one error: `src/v2/server.ts` passed `types` to
  `ctx.event.subscribe`, which the pinned `@opencode/plugin` 2.0.32 does
  not declare (2.2.x does). The options are now built apart from the call,
  so the pinned types accept them and the runtime call is unchanged. Moving
  the pin to a version that declares `types` is a separate choice.
- Installing the Pi extension's types costs 124 packages (about 175 MB in
  `node_modules/`, ignored by git) in CI and on a machine that runs
  `just plugins`, to check one file. Scripts are not run when installing.
- CI gains a job that needs the npm registry, so a registry outage fails
  `plugins` and not `rust`.
- The workflow and the flake pin Node separately, and nothing ties the
  two; a bump of one is a bump of the other, as the comments say.
- A new Pi release can change the types the extension is checked against
  only when someone bumps the dev dependency; the check says nothing about
  Pi versions newer than the pin.

## Alternatives Considered

- **Node in CI only.** `actions/setup-node` alone is the smallest change,
  and the dev shell stays smaller. But the recipes would still need a
  Node an operator or agent happens to have, which is how the type check
  went unrun three times.
- **Tests only, no type check.** Needs no network: `node --test` strips
  types and checks none. It misses the drift the story names, since the
  glue's typing against the SDK packages is the part nothing exercises.
- **Tests only for the Pi extension.** It needs no `package.json`, but it
  leaves `ExtensionAPI` and the event names the extension subscribes to
  unchecked against Pi's own types. The cost above is small, and the
  decision to leave it out is easy to take later.
- **Keep it manual.** The status quo, rejected for the reasons above.
- **Build the plugin's dependencies in Nix** (`buildNpmPackage` or
  `importNpmLock`), so `nix flake check` covers the plugin. It would make
  CI and local checks hermetic, at the price of a hash to refresh on every
  lock change and Nix in CI; `npm ci` against the committed lock is
  reproducible enough for a check.
- **CI through the dev shell** (`nix develop -c just plugins`, as ADR
  0049 sketched). CI has no Nix step, and adding one for a one-minute job
  would pull Nix's install and cache into every run.
- **Run the plugin tests from `just check`.** The tests need no network,
  but the type check does, and a `check` that needs Node and, for one
  step, the network could no longer run offline.
- **`nodejs_22`** (22.22.2 in the lock). Runs the tests too, but the
  plugin tests have been run on Node 24 throughout, 24 is a maintained LTS
  line, and it strips types without a flag.

## Open Questions Answered

- *Does `just check` need Node or the network now?* No; `just plugins`
  does, and is separate.
- *Which Node does CI use?* The one major the flake pins, 24.
- *Is the Pi extension type-checked?* Yes; see decision 5.
