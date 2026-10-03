# Conspectus Agent Instructions

## Project Context

Conspectus is a planned standalone CLI for surveying local AI-agent work across
agent sessions, mux sessions, repos, checkouts, loose workspaces, forks,
branches, and forge PRs.

The project is currently design-first. Before planning or implementing
behavior, read:

- `docs/design.md` for the current north-star product and data-model plan
- `docs/adr/` for accepted Architecture Decision Records
- `README.md` for the project summary

Use `nix develop` for local development. The shell provides Rust tooling and
standard checks.

## Design Guardrails

- Design data-model-first. New features and implementation phases should flow
  from the graph model, not from incidental provider or UI details.
- Keep the core graph provider-neutral. Provider-specific reality belongs in
  adapters, source metadata, evidence, and carefully chosen attributes.
- Treat the graph as sparse by default. Missing links and disconnected cliques
  are normal, not exceptional.
- Use `GraphLink` candidates as the evidence layer and derive typed resolved
  relationships through a resolver.
- Use one polymorphic `Fork` node for fork-like provenance; express effects via
  relation kinds and source metadata.
- Prioritize link inference from on-disk state, provider metadata, and common
  conventions. Federated user overrides are available when needed and take
  precedence over weaker inferred links.
- Keep rebuildable caches outside project trees. Persist user-authored link
  intent near the relevant repo or workspace when the relationship is
  project-rooted.

## Decision Records

- Significant decisions resolved while building features or drafting plans must
  be memorialized as new ADRs under `docs/adr/`.
- Do not introduce new project dependencies, workflow tools, or persistent
  conventions just because they are convenient. First document the need,
  research reasonable alternatives, compare tradeoffs, and record the
  conclusion as an ADR.
- Use the existing ADR style: status, context, decision, consequences,
  alternatives considered, and open questions answered when applicable.
- Update `docs/design.md` whenever requirements are introduced or refined, or
  when accepted ADRs change the intended model.
- Keep `docs/design.md` concise enough to remain useful for phase and story
  planning. Put detailed rationale in ADRs.

## Implementation Notes

- Read-only is the default for discovery and orchestration. Writes are
  bounded by the mutation envelope in ADR 0087: user-intent TOML stores,
  rebuildable observation sidecars under `$XDG_STATE_HOME/conspectus/`,
  operator-initiated mux lifecycle (rename / new-session / attach and the
  narrow pin-launch `send-keys` on Conspectus-constructed argv), and
  Conspectus-owned subprocess launches. Conspectus never mutates
  harness-native state, injects terminal input into live agent panes
  (ADR 0028 absolute), persists payload (ADR 0086), writes shared/system
  locations, performs background mutation, mutates git state, or bypasses
  hooks. Any new write path must cite ADR 0087 and land inside one of
  its sanctioned categories.
- Avoid making Conspectus depend on Atelier command modules directly. Shared
  code should be pure discovery/parsing/model code with a clean boundary.
- Track implementation work as Backlog.md tasks under `backlog/` (ADR 0109);
  see Work Tracking below.
- Code comments explain behavior and rationale; they do not cite backlog IDs
  or narrate history (ADR 0100). Put IDs in commit messages and task final
  summaries. A comment may point at open work as "backlog `ID`";
  `tests/comment_hygiene.rs` enforces this.
- Preserve user changes and avoid rewriting unrelated files.
- For docs-only changes, run `git diff --check`. For code changes, add or run
  the most relevant checks once the project has executable code.
- For TUI / renderer changes with visible output (column alignment, styling,
  layout, color choices, chip placement, overlay shape), validate with
  `conspectus tui --snapshot` (dev-only, ADR 0067) instead of asking the
  operator for a screenshot. The flag renders one frame to stdout with ANSI
  styling preserved; pair it with `--snapshot-pane left|right|header|status` to
  target a region and `--snapshot-keys "..."` (vim-style) to drive the UI into
  a non-default state before the snapshot. Keys are routed by pane focus as
  in the live TUI, so `<Tab>` moves `j`/`k`/`<Enter>`/`<Backspace>` to the
  right-pane explorer (`--snapshot-pane right --snapshot-keys "j<Tab>jjjjj"`
  walks the cursor past the Node fields onto a Related row and shows its
  Preview). Requires `--features snapshot`: `just check` and CI enable it
  through `--all-features`; for manual runs use
  `cargo run --features snapshot -- tui --snapshot ...`.

## Git And Review Conventions

- Use Conventional Commit subjects, e.g. `feat(resolve): add graph resolver
  tests`, `docs(adr): record workspace detection decision`, or
  `chore(nix): add dev shell tools`.
- Keep commit size proportional to impact. Small docs fixes can be one commit;
  broad model, resolver, or workflow changes should be split into reviewable
  commits by concern.
- Keep commits focused. Separate ADR/design updates, scaffolding, model
  changes, tests, and behavior changes when that makes review easier.
- Prefer short-lived feature branches for multi-step work. Keep `main`
  releasable.
- Prefer squash merges for feature branches so `main` stays story-oriented.
  Use fast-forward only for small linear branches. Avoid merge commits unless
  preserving branch topology is explicitly useful.
- Every PR should include at least one sentence of commentary beyond the title
  that describes the change and surrounding context. Use more detail when the
  change affects the data model, resolver semantics, persistence, or user
  workflows.
- Rebase local feature branches on `main` before merging when practical; do not
  rewrite shared history without coordination.
- Do not commit generated caches, local state, or work tracker scratch data
  unless the file is an intentional project artifact.

## Pre-Main Invariants

Before a feature branch, squash merge, or direct commit lands on `main`:

- Relevant tests pass. Once the Rust crate exists, run at least
  `cargo test --all-targets --all-features`; prefer
  `cargo nextest run --all-targets --all-features` for full validation.
- Formatting and linting pass: `cargo fmt -- --check` and
  `cargo clippy --all-targets --all-features -- -D warnings` when code exists.
- `git diff --check` passes for every change.
- Documentation is updated when behavior, architecture, workflow, or commands
  change.
- Significant decisions are recorded in a new ADR under `docs/adr/`, and
  `docs/design.md` is updated when the north-star model or requirements change.
- Tests cover new resolver/model behavior, including sparse and ambiguous graph
  cases.
- Machine-readable output changes include snapshot or fixture coverage once
  output tests exist.
- The working tree is clean before pushing.

## Work Tracking

Stories are Backlog.md tasks (ADR 0109) in `backlog/tasks/`, done ones
included, with one milestone per phase or workstream in `backlog/milestones/`
whose description keeps that section's planning prose. The `backlog` CLI
comes from `nix develop`. These conventions add to the Backlog.md workflow
below:

- Cite task IDs (`CSP-123`) in commit messages, as before.
- Older commits, PRs, and transcripts cite pre-migration IDs (`P8-014`,
  `H-PIN-TUI-011`). Map them with `docs/backlog-legacy-ids.md` or
  `git grep -w P8-014 -- backlog/`. Keep every task's `Legacy ID:` line.
- Describe a new story as before: scope, tests, manual checks, and
  blockers. Record blockers that are tasks with `--dep` too, and run
  `backlog doctor` after changing dependencies.
- When a story lands, record what landed and how it was verified with
  `--final-summary`, check its Definition of Done items, and set it `Done`.
  Leave it on the board: `backlog task complete` and the browser's cleanup
  move tasks to `backlog/completed/`, which search and milestone progress
  skip.
- Pass text with backticks or apostrophes through a quoted heredoc so the
  shell leaves it alone:

  ```sh
  backlog task edit CSP-123 --final-summary "$(cat <<'EOF'
  Landed `conspectus hook write`; the operator's flow is unchanged.
  EOF
  )"
  ```

<!-- BACKLOG.MD GUIDELINES START -->
<!-- backlog.md-instructions-version: 1.53.0 -->
<CRITICAL_INSTRUCTION>

## Backlog.md Workflow

This project uses Backlog.md for task and project management.

**At the beginning of each conversation in this project, run `backlog instructions overview` before answering or taking action. Re-read it only if you have not read it yet in the current conversation.**

Use the overview to decide whether to search, read, create, or update Backlog tasks.

Before task lifecycle actions, read the matching detailed guide:
- `backlog instructions task-creation` before creating or splitting tasks
- `backlog instructions task-execution` before planning, changing status or assignee, adding a plan or implementation notes, or implementing task work
- `backlog instructions task-finalization` before checking acceptance criteria, writing final summaries, or moving tasks to terminal statuses

Use `backlog <command> --help` before running unfamiliar commands. Help shows options, fields, and examples.

Do not edit Backlog task, draft, document, decision, or milestone markdown files directly. Use the `backlog` CLI so metadata, relationships, and history stay consistent.

</CRITICAL_INSTRUCTION>
<!-- BACKLOG.MD GUIDELINES END -->
