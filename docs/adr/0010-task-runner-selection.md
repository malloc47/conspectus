# ADR 0010: Task Runner Selection

## Status

Accepted

## Context

Phase 00 introduced a project-level command surface for common checks:
formatting, linting, tests, nextest, and whitespace validation. The initial
implementation used `just` because the development shell already included it,
but that choice had not been compared or recorded.

Conspectus should avoid introducing new project dependencies by habit. A tool
dependency should have an explicit need, a comparison against reasonable
alternatives, and an accepted decision before it becomes part of the workflow.

The immediate need is narrow:

- expose a small set of local and CI-ready check commands
- make those commands discoverable for humans and coding agents
- avoid embedding complex automation in the Rust crate before it exists
- keep the workflow compatible with the existing Nix development shell
- preserve an easy migration path if the command surface outgrows a simple
  runner

Research sources:

- `just`: <https://github.com/casey/just>
- GNU Make: <https://www.gnu.org/software/make/manual/make.html>
- cargo-make: <https://github.com/sagiegurari/cargo-make>
- cargo-xtask pattern: <https://github.com/matklad/cargo-xtask>
- Task: <https://taskfile.dev/>

## Decision

Use `just` as the initial local command runner for Conspectus.

The repo should keep the `justfile` small and limited to developer workflow
entrypoints such as formatting, linting, tests, nextest, and diff checks.
Behavioral build logic, code generation, release automation, or graph-specific
operations should not be added to `justfile` without a fresh review. If the
workflow grows beyond simple command aliases and dependency ordering, revisit
this decision and consider `cargo xtask` or another more structured approach.

Any future nontrivial dependency or workflow tool must be documented before it
is introduced:

- describe the need
- compare reasonable alternatives
- record the chosen option and tradeoffs in an ADR
- update `docs/design.md` only when the decision changes product or
  architecture requirements

## Consequences

- Contributors and agents get one discoverable entrypoint for routine checks:
  `just --list` and `just check`.
- The Nix development shell remains the source of installation, so Conspectus
  does not require global `just` installation for contributors using
  `nix develop`.
- The command runner is intentionally not part of the application runtime or
  Rust dependency graph.
- The repo gains one workflow dependency and one syntax to learn.
- `just` recipes are command aliases, not a substitute for Cargo, CI, or
  complex Rust-coded automation.

## Alternatives Considered

- Plain Cargo commands only. Rejected for the local workflow because it leaves
  agents and contributors to remember several long commands and does not
  provide a discoverable aggregate check target.
- Shell script. Rejected because one growing script tends to become a bespoke
  task runner with weaker command discovery and less clear per-task
  composition.
- GNU Make. Strong default because it is ubiquitous and mature. Rejected for
  this narrow command-runner use because Make's file-target semantics and
  phony-target ceremony are not needed for simple always-run checks.
- cargo-make. Strong Rust-focused task runner and build tool. Rejected for now
  because it is more featureful than the current need and would add a larger
  workflow layer before Conspectus has complex automation.
- cargo-xtask. Strong later option for project-specific automation written in
  Rust. Rejected for Phase 00 because it requires extra workspace structure and
  code for tasks that are currently just direct tool invocations.
- Taskfile. Strong cross-platform task runner with YAML configuration and
  caching features. Rejected for now because the repo already has a Nix shell
  with `just`, and Task's broader feature set is not needed for the current
  check surface.

## Open Questions Answered

- `just` should not have been introduced without an ADR; this ADR records the
  decision retroactively and establishes the guardrail for future additions.
- `justfile` should stay small and limited to check orchestration for now.
- More complex automation should trigger a new review rather than accreting in
  the initial `justfile`.
