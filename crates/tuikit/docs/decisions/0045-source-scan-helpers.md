# 0045: The source-scan mechanisms are the test kit's, and the rule sets stay with their repository

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/testkit.md](../controls/testkit.md) |

## Decision

- `testkit::source` exposes the crate's one lexer, which the glyph scanner
  shares: `code`, `without_comments`, `strip_test_items`, `production_code`,
  `string_literals` (escapes resolved), `contains_at_word_start`, and a
  read-only `files` and `rust_files` walk.
- The framework's own guards use it, and so do a host's scans (its tree
  walk, its scans for drawn text, marks, list cursors, and key handling);
  each keeps its own rules.
- The scans a repository runs over every tree it holds, this crate's
  included, are that repository's policy and live in its tests. Their
  mechanisms are here.

## Rationale

The host had several lexers that disagreed (one stopped reading at the first
test line and kept escapes as letters). One lexer gives every scan the same
idea of code, comment, literal, and test item, while the rules (which trees,
which words, which exceptions) stay with whoever owns the code.

## Consequences

- When the crate leaves the host, it keeps its own guards and the
  mechanisms; the host keeps its rule sets and calls the kit.
- Which of the host's scans reach the crate is listed in
  [contributing.md](../contributing.md#repository-wide-hygiene-scans).

## Alternatives considered

- **Moving the host's hygiene tests into the crate.** Their rule sets are
  the repository's policy, not the framework's.
- **Each scan keeping its own reader.** The disagreements stay.

## Origin

- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decision 9.
- host ADR 0150, "The interface framework is a workspace member crate": the
  open question on the hygiene tests (their mechanisms become reusable
  helpers; their rule sets stay repository policy).
