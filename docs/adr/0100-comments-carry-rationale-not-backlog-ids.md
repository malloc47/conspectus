# ADR 0100: Comments Carry Rationale, Not Backlog IDs

## Status

Accepted. Applied across the codebase in `CSP-566`; enforced by
`tests/comment_hygiene.rs`.

## Context

Conspectus is planned in `docs/backlog.md`, and each story has an ID
(`CSP-510`, `CSP-448.01`, `CSP-326`). While stories were being
built, those IDs went into code comments: as prefixes
(`// CSP-476: dispatch to the registered adapter`), parentheticals
(`(CSP-508 / ADR 0093)`), and narration of history ("preserves the
pre-H-EXT-004 if-chain", "wave 2 will move this into the reducer").
About 1,000 comment lines carried one before the 0.1.0 cleanup.

Those comments aged badly:

- A reader has to open the backlog to learn what the ID means, and
  the backlog entry describes the change that was made, not the code
  as it stands.
- "Pre-X" and "wave N" narration describes code that no longer exists.
  Several of these comments had become false. One promised that a
  later wave would move scroll reconciliation into the reducer, which
  had already happened. Another said the daemon's `refresh` "rotates a
  backup per ADR 0037", although the SQLite store it described had
  been retired by ADR 0082.
- The history is already recorded in commit messages and in the
  backlog's outcome notes, which are the right places for it.

ADR references are different. ADRs are durable decision records, and
pointing at one ("per ADR 0085 contract 2") explains why the code has
its shape.

## Decision

- Code comments explain what the code does and why. They do not cite
  backlog IDs as provenance or narrate how the code used to look.
- ADR references are welcome.
- A comment may point at an **open** backlog item when it describes a
  known gap. It says so explicitly, for example "open work (backlog
  `CSP-185`)". These pointers are removed when the item closes.
- `tests/comment_hygiene.rs` scans the Rust sources under `src/`,
  `tests/`, and `examples/`. It fails when a comment line contains a
  backlog ID without the word "backlog" on that line.
- Commit messages and backlog outcome notes keep carrying IDs.
- `AGENTS.md` states the rule for coding agents.

## Consequences

- Comments describe the current code, so they stay true as long as the
  code they sit beside does.
- Pointers to open work remain greppable, and the test makes each one
  deliberate.
- The guard uses a token match, not a parser. A comment that needs to
  show an ID as data (the test's own doc comment, for example) words it
  as "Backlog IDs such as …".

## Alternatives Considered

**Leave the IDs in place.** They are cheap to write and link code to
planning. In practice the link pointed at change descriptions rather
than at current behavior, and the surrounding prose drifted.

**Convention only, without a test.** About 1,000 lines accumulated while
the convention was unwritten. Agents write most of the code here and
copy the style of nearby comments, so an unenforced rule would erode.

**A clippy or rustfmt rule.** Neither has a lint for comment content,
and a custom lint driver is far heavier than a 100-line test.
